// AppController.RobloxUpdates.cs: Settings › Roblox updates: Roblox builds seen on this PC (in prefs.json, so it works while
// locked), and noticing when Roblox closes its other windows as one more opens (an update breaking Multi-Roblox), beside
// what the Roblox watch found in each build (roblox_watch) and the builds two windows played together on.
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use serde_json::{Value, json};
use windows_sys::Win32::Foundation::{CloseHandle, FILETIME};
use windows_sys::Win32::System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

use super::App;
use super::controller::{INSTALL_CACHE_LIFETIME, PendingLaunch};
use super::installs::{InstallsInfo, net_version, read_installs};
use crate::core::base::{Cancel, Fail, Guid, Res, Stamp};
use crate::core::models::{Account, stamp_min};
use crate::core::roblox_api::{self, realm};
use crate::core::roblox_installs::{self, ClientVersion};
use crate::core::roblox_processes::{self, PlayerProcess};
use crate::core::roblox_watch::{self, Build};
use crate::core::{l, log, multi_instance, prefs, rt, win_kit};

pub const MAX_ROBLOX_HISTORY: usize = 30;
/// A client that ends this soon after another one started may have been closed by it.
pub const NEW_CLIENT_WINDOW: Duration = Duration::from_secs(20);
/// Without the log line: two or more tracked clients ending this close together.
pub const CLOSE_BURST: Duration = Duration::from_secs(5);
/// The notice waits this long after the first close, so it can count the windows closed with it.
pub const CLOSED_NOTICE_DELAY: Duration = Duration::from_secs(6);
pub const CLOSED_WARNING_SHOWN: Duration = Duration::from_secs(24 * 3600);
pub const RECENT_ROBLOX_UPDATE: Duration = Duration::from_secs(48 * 3600);
pub const RELEASE_NOTES_INDEX: &str = "https://create.roblox.com/docs/en-us/releases";
pub const MEMORY_CRASH_WINDOW: Duration = Duration::from_secs(30);
/// Two clients in a game together this long, none closed by Roblox since: Multi-Roblox works again.
pub const WORKING_AGAIN: Duration = Duration::from_secs(10);
pub const UPDATE_REOPEN_CLOSE_LIMIT: Duration = Duration::from_secs(30);
/// How long before the cached read runs out the next one starts, on a worker.
pub const INSTALL_READ_AHEAD: Duration = Duration::from_secs(20);

/// A read of what is installed done on a worker: the read, the launcher that can update, when the cached read it
/// replaces was made, and when this one was.
pub type InstallsRead = (InstallsInfo, Option<String>, Stamp, Stamp);

/// The instance fields AppController.RobloxUpdates.cs declares.
pub struct RobloxUpdatesState {
    /// Roblox clients running at the last tick, and when each one that started since then first appeared.
    pub known_clients: RefCell<HashSet<i32>>,
    pub client_starts: RefCell<HashMap<i32, Stamp>>,
    pub clients_seen: Cell<bool>,
    /// Accounts whose client Roblox closed: no auto-rejoin (it would close the next window again).
    pub closed_by_roblox: RefCell<HashSet<Guid>>,
    /// Tracked clients that ended right after another one started, without the log line saying why: (account, at, counted).
    pub suspect_ends: RefCell<Vec<(Guid, Stamp, bool)>>,
    pub closed_pending: Cell<i32>,
    pub closed_notice_due: Cell<Option<Stamp>>,
    /// When Windows' commit limit was last all but used up (read every tick, also while locked): clients ending soon after
    /// it ran out of memory.
    pub memory_short_at: Cell<Option<Stamp>>,
    /// Since when two or more clients have been in a game at once (Multi-Roblox armed); None after a close by Roblox or under two.
    pub together_since: Cell<Option<Stamp>>,
    /// The build each client runs, read once from its file by the path it started from; None when Windows wouldn't say.
    pub client_builds: RefCell<HashMap<i32, Option<ClientVersion>>>,
    /// The newest build the automatic update was tried for: once each, so a launcher that can't doesn't run every tick.
    pub auto_updated_for: RefCell<Option<String>>,
    /// The build the automatic update is waiting for windows over (said once in the log).
    pub auto_update_waits_for: RefCell<Option<String>>,
    /// When "Close, update, reopen" closed the windows; None when none is under way. Rejoins wait for it.
    pub update_reopen_since: Cell<Option<Stamp>>,
    pub update_reopen_updating: Cell<bool>,
    /// A read of what is installed under way on a worker (C#'s Task): its answer comes on this channel; a worker that
    /// failed hangs up without one.
    pub installs_reading: RefCell<Option<Receiver<InstallsRead>>>,
    /// When Roblox's live build was last asked; a launch with Roblox open asks again past LIVE_RECHECK.
    pub latest_roblox_at: Cell<Stamp>,
    /// When the live build in latest_roblox was first seen: a client started after it asked Roblox for that build.
    pub latest_roblox_since: Cell<Stamp>,
    /// Roblox's own clients started behind the live build after it was listed, with the build they asked for: each one that
    /// gets into a game still on its old build shows Roblox hasn't rolled the build out to this PC (live_not_here).
    pub rollout_probes: RefCell<HashMap<i32, String>>,
    /// A live build Roblox lists but hasn't rolled out to this PC, and when that was seen.
    pub live_not_here: RefCell<Option<(String, Stamp)>>,
}

/// How long a "not rolled out here" holds: a staged rollout can reach this PC later (RoALT trusts it 30 minutes too).
const NOT_HERE_FOR: Duration = Duration::from_secs(30 * 60);
/// A launch with Roblox open asks for the live build again when the last answer is older than this: Roblox can release one
/// while windows are open, and a client behind it would update and close them all (RoALT asks before every launch).
const LIVE_RECHECK: Duration = Duration::from_secs(30);

/// Whether a launch asks Roblox for its live build first: never asked, or Roblox is open and the answer is old.
pub fn ask_live_again(known: bool, asked_at: Stamp, now: Stamp, open: usize) -> bool {
    !known || (open > 0 && now.secs_since(asked_at) > secs(LIVE_RECHECK))
}

/// Roblox lists a build before it reaches every PC (staged rollouts). A client that asked for `asked_for` and got into a
/// game on another build wasn't updated by Roblox, so that build isn't this PC's update yet (as RoALT reads it).
pub fn not_rolled_out(asked_for: &str, runs: Option<&ClientVersion>, in_game: bool) -> bool {
    in_game && runs.is_some_and(|r| !r.folder.eq_ignore_ascii_case(asked_for))
}

/// DateTime.FromFileTimeUtc.
fn from_file_time(created: i64) -> Stamp {
    Stamp::from_unix_ms((created - 116_444_736_000_000_000) / 10_000)
}

impl Default for RobloxUpdatesState {
    fn default() -> Self {
        RobloxUpdatesState {
            known_clients: RefCell::new(HashSet::new()),
            client_starts: RefCell::new(HashMap::new()),
            clients_seen: Cell::new(false),
            closed_by_roblox: RefCell::new(HashSet::new()),
            suspect_ends: RefCell::new(Vec::new()),
            closed_pending: Cell::new(0),
            closed_notice_due: Cell::new(None),
            memory_short_at: Cell::new(None),
            together_since: Cell::new(None),
            client_builds: RefCell::new(HashMap::new()),
            auto_updated_for: RefCell::new(None),
            auto_update_waits_for: RefCell::new(None),
            update_reopen_since: Cell::new(None),
            update_reopen_updating: Cell::new(false),
            installs_reading: RefCell::new(None),
            latest_roblox_at: Cell::new(crate::core::models::stamp_min()),
            latest_roblox_since: Cell::new(crate::core::models::stamp_min()),
            rollout_probes: RefCell::new(HashMap::new()),
            live_not_here: RefCell::new(None),
        }
    }
}

fn secs(d: Duration) -> f64 {
    d.as_secs_f64()
}

/// How many of the Roblox windows open would stay closed after "Close, update, reopen": those not among the ones
/// that can be opened again.
pub fn would_stay_closed(open: impl IntoIterator<Item = i32>, reopenable: impl IntoIterator<Item = i32>) -> i32 {
    let again: HashSet<i32> = reopenable.into_iter().collect();
    open.into_iter().filter(|pid| !again.contains(pid)).count() as i32
}

/// Whether the windows told to close have had their time. A start ahead of the tick's clock counts from now: the
/// automatic update starts the wait inside a tick, after the tick's clock was read, and the old check ("now is before the
/// start") gave up at once: the windows had been closed, and nothing was updated or opened again (2026-11-11).
pub fn close_time_up(now: Stamp, since: &mut Stamp, limit: Duration) -> bool {
    if now < *since {
        *since = now;
    }
    now.secs_since(*since) >= secs(limit)
}

/// A Stamp as a FILETIME (UTC), as DateTime.ToFileTime.
fn to_file_time(at: Stamp) -> i64 {
    at.unix_ms() * 10_000 + 116_444_736_000_000_000
}

/// A process's start (GetProcessTimes' creation time, a FILETIME); None when it ended meanwhile or isn't ours to read.
fn process_start(pid: i32) -> Option<i64> {
    // SAFETY: the handle is checked and closed here; four out values on the stack.
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid as u32);
        if h.is_null() {
            return None;
        }
        let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        let (mut created, mut exit, mut kernel, mut user) = (zero, zero, zero, zero);
        let ok = GetProcessTimes(h, &mut created, &mut exit, &mut kernel, &mut user) != 0;
        CloseHandle(h);
        if ok { Some(((created.dwHighDateTime as i64) << 32) | created.dwLowDateTime as i64) } else { None }
    }
}

/// A process of this launcher started at or after this time (not its watchers of windows already open).
pub fn launcher_running_since(launcher: &str, since: Stamp) -> bool {
    let after = to_file_time(since.plus_secs(-1.0));
    // Ended meanwhile, or not ours to read: skipped (InvalidOperationException, Win32Exception).
    win_kit::pids_by_name(launcher).unwrap_or_default().into_iter().any(|pid| process_start(pid).is_some_and(|start| start >= after))
}

/// Closes the Roblox app Roblox's installer opened since `since` (no game in it), looking a few seconds in case it is
/// still starting.
pub async fn close_installer_app(since: Stamp) {
    let after = to_file_time(since.plus_secs(-1.0));
    for _ in 0..6 {
        let closed = rt::blocking(move || {
            let mut closed = false;
            for p in roblox_processes::get_players() {
                if p.created >= after && !roblox_processes::is_closing(p.pid) && roblox_processes::opened_by_installer(p.pid, p.created) {
                    log::note(&format!("Closed the Roblox app Roblox's installer opened after updating (pid {}).", p.pid));
                    roblox_processes::close(p.pid, false);
                    closed = true;
                }
            }
            closed
        })
        .await;
        if closed {
            return;
        }
        rt::sleep(Duration::from_millis(500)).await;
    }
}

impl App {
    /// Notes the Roblox clients that appeared since the last tick: not those running when Bloxkeep started, nor the
    /// windowless copy a client starts in the notification area as it closes (its parent is that client) — counted as new,
    /// two closing at once looked as if each had closed the other.
    /// `parent_of`: the process that started a client (0 when unknown); asked only for a client not seen before.
    pub fn note_client_starts(&self, pids: &HashSet<i32>, now: Stamp, parent_of: &dyn Fn(i32) -> i32) {
        let seen = self.roblox_updates.clients_seen.get();
        let known: HashSet<i32> = self.roblox_updates.known_clients.borrow().clone();
        {
            let mut starts = self.roblox_updates.client_starts.borrow_mut();
            for &pid in pids {
                if known.contains(&pid) || !seen {
                    continue;
                }
                let parent = parent_of(pid);
                if parent == 0 || !(pids.contains(&parent) || known.contains(&parent)) {
                    starts.insert(pid, now);
                }
            }
            starts.retain(|_, at| !(now.secs_since(*at) > secs(NEW_CLIENT_WINDOW) * 3.0 || now < *at));
        }
        *self.roblox_updates.known_clients.borrow_mut() = pids.clone();
        self.roblox_updates.clients_seen.set(true);
    }

    pub fn forget_client_start(&self, pid: i32) {
        self.roblox_updates.client_starts.borrow_mut().remove(&pid);
    }

    /// A tracked client ended (read before auto-rejoin decides): did Roblox close it because another opened? Its log saying
    /// so with another client started in the last 20 s is enough; without that line, two or more tracked clients that closed
    /// in order (`in_order`) within 5 s of each other right after a new one started. Crashes, killed processes
    /// (2026-10-22: three windows out of memory at once were taken for this) and Bloxkeep's own closes never count.
    /// `in_order`: it closed in order: its log says it shut down, or it left its notification-area copy.
    /// `children_of`: child processes by parent: the ended client's own notification-area copy isn't a new window.
    pub fn check_closed_by_roblox(&self, account: Guid, pid: i32, started_at: Stamp, now: Stamp, in_order: bool, children_of: &dyn Fn(i32) -> Vec<i32>) -> bool {
        // Without Multi-Roblox holding Roblox's single-window lock, Roblox closing the older window is how it always works.
        if !multi_instance::armed() || self.controller.user_closed.borrow().contains(&account) || self.rejoin.closing_for_rejoin.borrow().contains(&pid) {
            return false;
        }
        let starts: Vec<(i32, Stamp)> = self.roblox_updates.client_starts.borrow().iter().map(|(k, v)| (*k, *v)).collect();
        let mut new_client = false;
        let mut children: Option<Vec<i32>> = None;
        for (other, at) in starts {
            if other == pid || at <= started_at || now.secs_since(at) > secs(NEW_CLIENT_WINDOW) {
                continue;
            }
            if children.get_or_insert_with(|| children_of(pid)).contains(&other) {
                continue;
            }
            new_client = true;
            break;
        }
        if !new_client {
            return false;
        }
        let (closed_line, out_of_memory) = {
            let logs = self.controller.logs.borrow();
            (logs.closed_by_new_window_at(pid).is_some(), logs.out_of_memory_at(pid).is_some())
        };
        if closed_line {
            log::note(&format!("Multi-Roblox: window {pid} closed by Roblox as another opened (its log says so); not rejoined."));
            self.roblox_updates.closed_by_roblox.borrow_mut().insert(account);
            self.count_closed_by_roblox(1, now);
            return true;
        }
        // Out of memory (its log says Windows refused it memory, or the commit limit was nearly used up just now): a crash, as on
        // 2026-10-22 (6 windows, Windows' low-virtual-memory event at 99.7 %), not Roblox closing them; Multi-Roblox is fine.
        let memory_short = self.roblox_updates.memory_short_at.get().is_some_and(|short_at| now >= short_at && now.secs_since(short_at) <= secs(MEMORY_CRASH_WINDOW));
        if !in_order || out_of_memory || memory_short {
            if in_order {
                log::note(&format!("Multi-Roblox: window {pid} ended as another opened, but out of memory; not counted as closed by Roblox."));
            }
            return false;
        }
        let mut called_off: Vec<Guid> = Vec::new();
        let counted = {
            let mut ends = self.roblox_updates.suspect_ends.borrow_mut();
            ends.retain(|e| !(now.secs_since(e.1) > secs(CLOSE_BURST) || now < e.1 || e.0 == account));
            ends.push((account, now, false));
            if ends.len() < 2 {
                return false;
            }
            let mut counted = 0;
            for end in ends.iter_mut() {
                if end.2 {
                    continue;
                }
                end.2 = true;
                counted += 1;
                // One that ended a moment earlier may have a rejoin queued already: call it off.
                if end.0 != account {
                    called_off.push(end.0);
                }
            }
            counted
        };
        if !called_off.is_empty() {
            self.controller.rejoin_queue.borrow_mut().retain(|r| !called_off.contains(&r.account_id));
        }
        self.roblox_updates.closed_by_roblox.borrow_mut().insert(account);
        log::note(&format!("Multi-Roblox: {counted} window(s) ended together right after another opened (last {pid}); taken as closed by Roblox, not rejoined."));
        self.count_closed_by_roblox(counted, now);
        true
    }

    pub fn count_closed_by_roblox(&self, windows: i32, now: Stamp) {
        if windows <= 0 {
            return;
        }
        let pending = self.roblox_updates.closed_pending.get() + windows;
        self.roblox_updates.closed_pending.set(pending);
        if self.roblox_updates.closed_notice_due.get().is_none() {
            self.roblox_updates.closed_notice_due.set(Some(now.plus_secs(secs(CLOSED_NOTICE_DELAY))));
        }
        self.roblox_updates.together_since.set(None);
        {
            let mut p = prefs::lock();
            p.roblox_closed_at = Some(Stamp::utc_now());
            p.roblox_closed_count = pending;
        }
        prefs::save();
    }

    /// Settings › Roblox version › Update now: this Roblox copy's launcher installs the newest build without opening
    /// it (roblox_installs::updater_for). Bloxstrap does it in the background, leaving windows open, when it can
    /// (its own setting, at most one version behind, 5 GB free); otherwise it closes every Roblox window, so the page asks
    /// first. The result shows as "Roblox updated to …"; a launcher ending without a new build is said too.
    /// `quiet`: Bloxkeep's own idea, not a click: a launcher that found nothing new isn't worth saying.
    pub fn update_roblox(self: &Rc<Self>, quiet: bool) -> Res<Value> {
        if self.controller.roblox_updating.get() {
            return Ok(Value::Null);
        }
        self.controller.installs_read_at.set(stamp_min()); // the launcher and the client as they are now
        let before = self.installs().client.map(|c| c.folder);
        let exe = match self.installs.updater_exe.borrow().clone() {
            Some(exe) if Path::new(&exe).is_file() => exe,
            _ => {
                log::note("Roblox update: no launcher here can update Roblox; it updates as it opens.");
                return Err(Fail::user(l::t("No launcher here can update Roblox by itself. Roblox updates the next time it opens.")));
            }
        };
        let own = roblox_installs::is_own_installer(Some(&exe));
        // Roblox's installer would close the windows open without a word: they close as Close, update, reopen's do, and
        // open again after (it comes back here once they are gone).
        // Not while Close, update, reopen itself calls this: it would turn the call down and nothing would update.
        if own && self.roblox_updates.update_reopen_since.get().is_none() && !roblox_processes::get_player_pids().is_empty() {
            return self.update_and_reopen();
        }
        let file_name = Path::new(&exe).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        log::note(&format!(
            "Roblox update: started {file_name}{}, from build {}.",
            if quiet { ", automatically" } else { "" },
            before.as_deref().unwrap_or("unknown")
        ));
        let started_at = Stamp::utc_now(); // C#'s DateTime.Now: compared with process start times, the same moment
        let mut cmd = std::process::Command::new(&exe);
        if let Some(dir) = Path::new(&exe).parent() {
            cmd.current_dir(dir);
        }
        if !own {
            cmd.arg("-player").arg("-nolaunch");
        }
        // Process.Start's Win32Exception goes up to the page, as in the C#.
        cmd.spawn().map_err(|e| roblox_api::fail_of("Win32Exception", e))?;
        self.controller.roblox_updating.set(true);
        self.push_state();
        let app = self.clone();
        let launcher = win_kit::file_stem(&exe);
        rt::spawn(async move { app.watch_roblox_update(launcher, before, started_at, quiet, own).await });
        Ok(Value::Null)
    }

    /// Settings › Roblox version › When: "keep": the update waits until no Roblox is open, and windows opened meanwhile play
    /// on the build already running (hold_for_roblox_update) instead of waiting for it.
    pub fn keeps_running_build(&self) -> bool {
        self.controller.vault.borrow().as_ref().is_some_and(|v| {
            let s = &v.data().settings;
            s.auto_update_roblox && s.roblox_update_when == "keep"
        })
    }

    pub fn live_not_here(&self, live: &ClientVersion) -> bool {
        self.roblox_updates
            .live_not_here
            .borrow()
            .as_ref()
            .is_some_and(|(folder, at)| folder.eq_ignore_ascii_case(&live.folder) && Stamp::utc_now().secs_since(*at) < secs(NOT_HERE_FOR))
    }

    pub fn note_client_builds(&self, players: &[PlayerProcess]) {
        let live = self.updates.latest_roblox.borrow().clone();
        let since = self.roblox_updates.latest_roblox_since.get();
        {
            let mut builds = self.roblox_updates.client_builds.borrow_mut();
            for player in players {
                if builds.contains_key(&player.pid) {
                    continue;
                }
                let path = roblox_processes::launch_path(player.pid, player.created);
                let build = roblox_installs::version_of(path.as_deref());
                if let (Some(path), Some(live)) = (path.as_deref(), live.as_ref())
                    && roblox_installs::is_roblox_own(path)
                    && !roblox_installs::is_vng_client(path)
                    && roblox_installs::is_behind(build.as_ref(), Some(live))
                    && player.created > 0
                    && from_file_time(player.created) > since
                {
                    self.roblox_updates.rollout_probes.borrow_mut().insert(player.pid, live.folder.clone());
                }
                builds.insert(player.pid, build);
            }
            builds.retain(|pid, _| players.iter().any(|p| p.pid == *pid));
        }
        // The builds kept to play on go once no Roblox is open or opening: the update can go in then.
        if players.is_empty()
            && !self.opening_roblox()
            && self.controller.pending.borrow().is_empty()
            && std::path::Path::new(&roblox_installs::kept_root()).is_dir()
            && roblox_installs::forget_kept_copies().is_ok()
        {
            log::note("Roblox update: no Roblox open, the build kept to play on is removed.");
        }
        let probes: Vec<(i32, String)> = self.roblox_updates.rollout_probes.borrow().iter().map(|(p, f)| (*p, f.clone())).collect();
        for (pid, asked) in probes {
            let in_game = self.controller.logs.borrow().state_of(pid).is_some_and(|s| s.status == "ingame");
            let runs = self.roblox_updates.client_builds.borrow().get(&pid).cloned();
            if runs.is_none() || in_game {
                self.roblox_updates.rollout_probes.borrow_mut().remove(&pid);
            }
            let runs = runs.flatten();
            let now_live = self.updates.latest_roblox.borrow().clone();
            if not_rolled_out(&asked, runs.as_ref(), in_game)
                && let Some(now_live) = now_live.filter(|n| n.folder == asked)
                && !self.live_not_here(&now_live)
                && let Some(runs) = runs
            {
                *self.roblox_updates.live_not_here.borrow_mut() = Some((asked.clone(), Stamp::utc_now()));
                log::note(&format!(
                    "Roblox update: Roblox lists {} ({asked}), but a Roblox started for it got into a game on {} ({}): not rolled out to this PC yet, so launches with Roblox open aren't held for it.",
                    now_live.version, runs.version, runs.folder
                ));
                self.push_state();
            }
        }
    }

    /// A window on a build older than Roblox's newest. Roblox VNG has its own builds, so it's never compared.
    pub fn runs_old_build(&self, account: &Account, pid: i32) -> bool {
        if account.realm == realm::VNG {
            return false;
        }
        let build = self.roblox_updates.client_builds.borrow().get(&pid).cloned().flatten();
        let latest = self.updates.latest_roblox.borrow().clone();
        roblox_installs::is_behind(build.as_ref(), latest.as_ref())
    }

    /// In a game, and auto-rejoin's path would open it again (see restart_due_clients).
    pub fn can_reopen(&self, id: Guid, run: &(i32, Rc<PendingLaunch>)) -> bool {
        run.1.target.is_some()
            && self.controller.logs.borrow().state_of(run.0).is_some_and(|s| s.status == "ingame")
            && self.find_account(id).is_some_and(|a| a.banned != Some(true) && a.cookie_valid != Some(false))
    }

    /// The newest build goes in by itself once no window could be cut off by it (a launcher's copy: Roblox's own updates
    /// itself as it opens), or at once with "now": as Close, update, reopen, the windows in a game opening again after (the
    /// owner, 2026-11-07). AppSettings' auto_update_roblox.
    pub fn auto_update_roblox(self: &Rc<Self>, players: &[PlayerProcess]) {
        let settings = self.controller.vault.borrow().as_ref().map(|v| {
            let d = v.data();
            (d.settings.auto_update_roblox, d.settings.roblox_update_when.clone())
        });
        let now = settings.as_ref().is_some_and(|s| s.1 == "now");
        if settings.as_ref().map(|s| s.0) != Some(true)
            || (!players.is_empty() && !now)
            || self.controller.roblox_updating.get()
            || self.roblox_updates.update_reopen_since.get().is_some()
            || self.opening_roblox()
            || self.controller.rejoin_queue.borrow().iter().any(|r| !self.rejoin.update_held_told.borrow().contains(&r.account_id))
            || self.settling()
            || self.schedule_due_soon()
        {
            return;
        }
        let version = self.roblox_version_dto();
        if version.up_to_date != Some(false) || version.updater.is_none() || version.latest == *self.roblox_updates.auto_updated_for.borrow() {
            return;
        }
        // By itself it closes only what it can open again. A window not in a game, one whose log isn't read yet (a second
        // after unlocking, when the windows open before are picked up) or one Bloxkeep didn't open would stay closed: the
        // owner's window did, 2026-11-11 01:25 ("closing 1 tracked … 0 to reopen"). It waits for them instead.
        let running: Vec<(Guid, (i32, Rc<PendingLaunch>))> = self.controller.running.borrow().iter().map(|(k, v)| (*k, v.clone())).collect();
        let reopenable: Vec<i32> = running.iter().filter(|(id, run)| self.can_reopen(*id, run)).map(|(_, run)| run.0).collect();
        let stay = would_stay_closed(players.iter().map(|p| p.pid), reopenable);
        let latest = version.latest.clone();
        if stay > 0 {
            if *self.roblox_updates.auto_update_waits_for.borrow() != latest {
                *self.roblox_updates.auto_update_waits_for.borrow_mut() = latest.clone();
                log::note(&format!(
                    "Roblox update: automatic, to {}, waits: {stay} of {} Roblox window(s) open couldn't be opened again after it (not in a game yet, or not opened by Bloxkeep).",
                    latest.as_deref().unwrap_or(""),
                    players.len()
                ));
            }
            return;
        }
        *self.roblox_updates.auto_updated_for.borrow_mut() = latest.clone();
        log::note(&format!(
            "Roblox update: automatic, to {}{}.",
            latest.as_deref().unwrap_or(""),
            if players.is_empty() { ", no Roblox open".to_string() } else { format!(", closing and reopening {} Roblox", players.len()) }
        ));
        // The C# let these throw up into the tick (its step logs it); here they are logged the same way.
        if players.is_empty() {
            if let Err(e) = self.update_roblox(true) {
                log::write(&e);
            }
            return;
        }
        // As Close, update, reopen: the windows in a game open again after (an account farming overnight isn't left closed).
        if let Err(e) = self.update_and_reopen() {
            log::write(&e);
        }
    }

    /// Settings › Roblox version › Close, update, reopen: every Roblox window closes, the launcher updates Roblox (with none,
    /// Roblox does as the first window opens), then the ones that were in a game open again, like a preset's restart.
    pub fn update_and_reopen(&self) -> Res<Value> {
        if self.controller.vault.borrow().is_none() || self.controller.roblox_updating.get() || self.roblox_updates.update_reopen_since.get().is_some() {
            return Ok(Value::Null);
        }
        // A launch or rejoin under way would keep opening windows, and the update would give up: "Some Roblox windows didn't close".
        if self.opening_roblox() {
            log::note("Close, update, reopen: not now, Roblox is still opening.");
            return Err(Fail::user(l::t("Wait until Bloxkeep finishes opening Roblox.")));
        }
        let mut tracked: HashSet<i32> = HashSet::new();
        let running: Vec<(Guid, (i32, Rc<PendingLaunch>))> = self.controller.running.borrow().iter().map(|(k, v)| (*k, v.clone())).collect();
        for (id, run) in running {
            tracked.insert(run.0);
            if self.can_reopen(id, &run) {
                let target = self.restart_target(run.0, &run.1);
                self.presets.restarts.borrow_mut().insert(id, (target, Stamp::utc_now()));
            } else {
                self.controller.user_closed.borrow_mut().insert(id);
            }
            self.rejoin.closing_for_rejoin.borrow_mut().insert(run.0); // Bloxkeep's own close: neither a drop nor closed by Roblox
            roblox_processes::close(run.0, self.in_game_now(run.0));
        }
        let mut untracked = 0;
        for other in roblox_processes::get_player_pids().into_iter().filter(|pid| !tracked.contains(pid)) {
            roblox_processes::close(other, false);
            untracked += 1;
        }
        log::note(&format!(
            "Close, update, reopen: closing {} tracked and {untracked} other Roblox window(s), {} to reopen.",
            tracked.len(),
            self.presets.restarts.borrow().len()
        ));
        self.roblox_updates.update_reopen_since.set(Some(Stamp::utc_now()));
        self.roblox_updates.update_reopen_updating.set(false);
        self.push_state();
        Ok(Value::Null)
    }

    pub fn tick_update_reopen(self: &Rc<Self>, now: Stamp) {
        let Some(mut since) = self.roblox_updates.update_reopen_since.get() else {
            return;
        };
        if !self.roblox_updates.update_reopen_updating.get() {
            roblox_processes::close_tray_copies();
            if !roblox_processes::get_player_pids().is_empty() {
                if !close_time_up(now, &mut since, UPDATE_REOPEN_CLOSE_LIMIT) {
                    self.roblox_updates.update_reopen_since.set(Some(since));
                    return;
                }
                log::note(&format!(
                    "Close, update, reopen: {} Roblox still open after {} s; not updated.",
                    roblox_processes::get_player_pids().len(),
                    UPDATE_REOPEN_CLOSE_LIMIT.as_secs()
                ));
                self.notify(&l::t("Some Roblox windows didn't close, so Roblox wasn't updated."), "warn", "launch", Guid::nil(), true, None);
                self.roblox_updates.update_reopen_since.set(None);
                self.push_state();
                return;
            }
            self.roblox_updates.update_reopen_updating.set(true);
            self.controller.installs_read_at.set(stamp_min());
            // Installs() is never null in the C#: the read is what fills the updater.
            let _ = self.installs();
            let exe = self.installs.updater_exe.borrow().clone();
            if exe.is_some_and(|e| Path::new(&e).is_file()) {
                // The C# let UpdateRoblox's exception go up into the tick (its step logs it).
                if let Err(e) = self.update_roblox(true) {
                    log::write(&e);
                }
            } else {
                log::note("Close, update, reopen: no launcher to update with; Roblox updates as the windows reopen.");
            }
        }
        if self.controller.roblox_updating.get() {
            return;
        }
        log::note(&format!("Close, update, reopen: done after {} s; the windows reopen.", now.secs_since(since) as i64));
        self.roblox_updates.update_reopen_since.set(None); // the windows' rejoins go now
        self.push_state();
    }

    /// Until the client's build changes or the launcher's processes for it have all ended, at most 15 minutes.
    /// Roblox's installer (`own`) is waited for to the end, then the Roblox app it opened is closed: closed
    /// sooner, while the installer still waits for it to start, the installer would call the launch failed.
    pub async fn watch_roblox_update(self: &Rc<Self>, launcher: String, before: Option<String>, started_at: Stamp, quiet: bool, own: bool) {
        // Set before the task started; cleared however this ends.
        let updating = crate::core::base::Busy::set(&self.controller.roblox_updating);
        let until = Stamp::utc_now().plus_secs(15.0 * 60.0);
        rt::sleep(Duration::from_secs(if own { 2 } else { 5 })).await;
        let mut finished = false;
        while !self.controller.disposed.get() && Stamp::utc_now() < until {
            // Read again on a worker (up to a second on a busy PC), compared with the last read (its "Roblox updated to …").
            let (via, custom) = self.launch_option();
            let (installs, updater) = rt::blocking(move || read_installs(&via, &custom)).await;
            let updated = self.take_installs(installs, updater, Stamp::utc_now()).client.map(|c| c.folder) != before;
            let running = {
                let launcher = launcher.clone();
                rt::blocking(move || launcher_running_since(&launcher, started_at)).await
            };
            if own && !running {
                close_installer_app(started_at).await;
            }
            if updated && !(own && running) {
                log::note(&format!("Roblox update: done, now build {}.", self.installs().client.map(|c| c.folder).unwrap_or_else(|| "unknown".into())));
                finished = true; // Installs said "Roblox updated to …"
                break;
            }
            if !running {
                log::note(&format!("Roblox update: {launcher} ended without a newer build."));
                if !quiet {
                    let who = if own { "Roblox".to_string() } else { launcher.clone() };
                    self.toast(&l::f("{0} found no newer Roblox to install.", &[&who]), "info");
                }
                finished = true;
                break;
            }
            rt::sleep(Duration::from_secs(if own { 1 } else { 3 })).await;
        }
        if !finished && !self.controller.disposed.get() {
            log::note(&format!("Roblox update: {launcher} still running after 15 min; stopped watching it."));
        }
        // finally
        drop(updating);
        if !self.controller.disposed.get() {
            self.push_state();
        }
    }

    /// From the process tick: looks for a new Roblox build every 3 minutes (also from the tray) and sends the "Roblox
    /// closed windows" notice, at most one a day. The warning clears once two clients have been in a game together for a few
    /// seconds with none closed by Roblox since, or once a newer build is installed (what it said may not hold for it).
    /// `in_game`: Roblox clients in a game now, by their logs.
    pub fn tick_roblox_updates(&self, now: Stamp, in_game: i32) {
        // What is installed is read again on a worker a little before the cached read runs out, and taken in at the next tick.
        // A read started before a vault or launch option change (which drops the cache) is left unused.
        let taken = {
            let reading = self.roblox_updates.installs_reading.borrow();
            match reading.as_ref().map(|r| r.try_recv()) {
                None | Some(Err(TryRecvError::Empty)) => None,
                Some(Ok(read)) => Some(Some(read)),
                Some(Err(TryRecvError::Disconnected)) => Some(None),
            }
        };
        if let Some(result) = taken {
            *self.roblox_updates.installs_reading.borrow_mut() = None;
            match result {
                Some((installs, updater, since, at)) => {
                    let current = self.installs.installs.borrow().as_ref().map(|i| i.client.as_ref().map(|c| c.folder.clone()));
                    if since == self.controller.installs_read_at.get()
                        && let Some(before) = current
                        && self.take_installs(installs, updater, at).client.map(|c| c.folder) != before
                    {
                        self.push_state_if_changed();
                    }
                }
                // PORT: the worker's exception (reading.Exception) was logged; a Rust worker that panicked has written
                // its panic to stderr already and only hangs up.
                None => log::note("Roblox installs: the read on a worker failed."),
            }
        }
        let has_installs = self.installs.installs.borrow().is_some();
        if !has_installs || now < self.controller.installs_read_at.get() {
            let before = self.installs.installs.borrow().as_ref().and_then(|i| i.client.as_ref().map(|c| c.folder.clone()));
            if self.installs().client.map(|c| c.folder) != before {
                self.push_state_if_changed();
            }
        } else if self.roblox_updates.installs_reading.borrow().is_none()
            && now.secs_since(self.controller.installs_read_at.get()) >= secs(INSTALL_CACHE_LIFETIME) - secs(INSTALL_READ_AHEAD)
        {
            let (via, custom) = self.launch_option();
            let read_before = self.controller.installs_read_at.get();
            let (tx, rx) = std::sync::mpsc::channel::<InstallsRead>();
            std::thread::spawn(move || {
                let (installs, updater) = read_installs(&via, &custom);
                let _ = tx.send((installs, updater, read_before, Stamp::utc_now()));
            });
            *self.roblox_updates.installs_reading.borrow_mut() = Some(rx);
        }
        let together_before = self.roblox_updates.together_since.get();
        if in_game < 2 || !multi_instance::armed() || together_before.is_some_and(|t| now < t) {
            self.roblox_updates.together_since.set(None);
        } else if together_before.is_none() {
            self.roblox_updates.together_since.set(Some(now));
        }
        // Two windows in a game together for a while: Multi-Roblox works on this build, whatever the Roblox watch said.
        let worked_now = match self.roblox_updates.together_since.get() {
            Some(together) if now.secs_since(together) >= secs(WORKING_AGAIN) => self.installs().client.map(|c| c.folder).filter(|f| !f.is_empty()),
            _ => None,
        };
        let known = worked_now.as_ref().is_some_and(|f| prefs::lock().multi_worked.contains(f));
        if let Some(folder) = worked_now
            && !known
        {
            log::note(&format!("Multi-Roblox: two windows played together on build {folder}; it works there."));
            {
                let mut p = prefs::lock();
                p.multi_worked.push(folder);
                let count = p.multi_worked.len();
                if count > MAX_ROBLOX_HISTORY {
                    p.multi_worked.drain(0..count - MAX_ROBLOX_HISTORY);
                }
            }
            prefs::save();
            self.push_state_if_changed();
        }
        let clear = {
            let p = prefs::lock();
            match p.roblox_closed_at {
                Some(closed_at) if self.roblox_updates.closed_notice_due.get().is_none() => {
                    self.roblox_updates.together_since.get().is_some_and(|since| now.secs_since(since) >= secs(WORKING_AGAIN) && closed_at < since)
                        || p.roblox_history.iter().any(|h| !h.first && h.at > closed_at)
                }
                _ => false,
            }
        };
        if clear {
            self.roblox_updates.together_since.set(None);
            self.hide_roblox_closed();
        }
        let Some(due) = self.roblox_updates.closed_notice_due.get() else {
            return;
        };
        if now < due {
            return;
        }
        let windows = self.roblox_updates.closed_pending.get();
        self.roblox_updates.closed_pending.set(0);
        self.roblox_updates.closed_notice_due.set(None);
        let told_today = prefs::lock().roblox_closed_notified.is_some_and(|told| {
            let (a, b) = (told.local(), Stamp::utc_now().local());
            (a.year, a.month, a.day) == (b.year, b.month, b.day)
        });
        if told_today {
            log::note(&format!("Multi-Roblox: Roblox closed {windows} window(s); not said again today."));
            self.push_state_if_changed();
            return;
        }
        prefs::lock().roblox_closed_notified = Some(Stamp::utc_now());
        prefs::save();
        self.notify(
            &l::f("Roblox closed {0} window(s) when another one opened — Roblox may have blocked running several windows. Multi-Roblox may need an update.", &[&windows]),
            "warn",
            "launch",
            Guid::nil(),
            true,
            None,
        );
        self.push_state_if_changed();
    }

    /// Adds a client build not seen before: the first as "first seen", later ones timed by when their folder was
    /// made. A build older than the last one (another launcher's copy) isn't an update.
    pub fn note_roblox_build(client: Option<&ClientVersion>) {
        let Some(client) = client else {
            return;
        };
        if client.folder.is_empty() || client.version.is_empty() {
            return;
        }
        let note = {
            let mut p = prefs::lock();
            let history = &mut p.roblox_history;
            if history.iter().any(|h| h.folder.eq_ignore_ascii_case(&client.folder)) {
                return;
            }
            let last = history.last().cloned();
            if let Some(last) = &last
                && let (Some(have), Some(had)) = (net_version(Some(&client.version)), net_version(Some(&last.version)))
                && have < had
            {
                return;
            }
            let now = Stamp::utc_now();
            let mut at = now;
            if let Some(last) = &last
                && let Some(exe) = &client.exe
            {
                // IOException, UnauthorizedAccessException, ArgumentException, NotSupportedException: the time stays now.
                let dir = Path::new(exe).parent().map(|d| d.to_path_buf()).unwrap_or_default();
                if let Ok(made) = std::fs::metadata(&dir).and_then(|m| m.created())
                    && let Ok(since_epoch) = made.duration_since(std::time::UNIX_EPOCH)
                {
                    let made = Stamp::from_unix_ms(since_epoch.as_millis() as i64);
                    if made > last.at && made <= now {
                        at = made;
                    }
                }
            }
            let source = client.source.clone().unwrap_or_default();
            history.push(prefs::RobloxBuild { at, version: client.version.clone(), folder: client.folder.clone(), source, first: last.is_none() });
            let count = history.len();
            if count > MAX_ROBLOX_HISTORY {
                history.drain(0..count - MAX_ROBLOX_HISTORY);
            }
            format!(
                "Roblox build {} ({}, {}) {}.",
                client.version,
                client.folder,
                client.source.as_deref().unwrap_or("?"),
                match &last {
                    None => "first seen".to_string(),
                    Some(last) => format!("installed, after {}", last.version),
                }
            )
        };
        log::note(&note);
        prefs::save();
    }

    pub fn roblox_updates_dto(&self) -> Value {
        let installed = self.installs().client;
        let now = Stamp::utc_now();
        let watch = self.watch_dto();
        let p = prefs::lock();
        // Only an update before the windows were closed can explain it; one after it is no reason.
        let last_update = p.roblox_closed_at.and_then(|closed_at| p.roblox_history.iter().rev().find(|h| !h.first && h.at <= closed_at).cloned());
        let history: Vec<Value> = p
            .roblox_history
            .iter()
            .rev()
            .map(|h| {
                json!({
                    "at": h.at,
                    "version": h.version,
                    "build": h.folder,
                    "source": h.source,
                    "first": h.first,
                    "check": roblox_watch::for_hash(Some(&h.folder)).map(|b| b.verdict),
                    "worked": p.multi_worked.contains(&h.folder),
                })
            })
            .collect();
        let closed = match p.roblox_closed_at {
            Some(at) if now.secs_since(at) < secs(CLOSED_WARNING_SHOWN) && at <= now.plus_secs(5.0 * 60.0) => json!({
                "at": at,
                "count": p.roblox_closed_count.max(1),
                "updatedAt": last_update.filter(|u| now.secs_since(u.at) < secs(RECENT_ROBLOX_UPDATE)).map(|u| u.at),
            }),
            _ => Value::Null,
        };
        json!({
            "installed": installed.map(|i| json!({ "version": i.version, "build": i.folder, "source": i.source })),
            "history": history,
            "watch": watch,
            "closed": closed,
        })
    }

    /// Roblox's notes for a version's release ("0.742.0.…" is release 742), in the default browser.
    pub fn open_release_notes(version: &str) -> Res<Value> {
        let parts: Vec<&str> = version.split('.').collect();
        let release = parts
            .get(1)
            .filter(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
            .and_then(|p| p.parse::<i32>().ok())
            .filter(|r| *r > 0 && *r < 100000);
        match release {
            Some(release) if parts.len() >= 2 => App::open_url(&format!("https://create.roblox.com/docs/release-notes/release-notes-{release}")),
            _ => App::open_url(RELEASE_NOTES_INDEX),
        }
    }

    pub fn hide_roblox_closed(&self) -> Value {
        prefs::lock().roblox_closed_at = None;
        prefs::save();
        self.push_state();
        Value::Null
    }

    /// Reads what Bloxkeep's Roblox watch found (roblox_watch), at the Roblox version check's pace.
    pub async fn refresh_roblox_watch(self: &Rc<Self>) {
        match rt::blocking(|| roblox_watch::refresh(&Cancel::new())).await {
            Ok(changed) => {
                if changed {
                    self.apply_watch_names();
                }
                self.push_state_if_changed();
            }
            // HttpRequestException, TaskCanceledException, IOException: shown as "couldn't check"; tried again next time.
            Err(e) if roblox_api::is_expected_error(&e) || roblox_api::is_fail_kind(&e, "IOException") => {}
            // PORT: any other exception went up into the fire-and-forget task, unobserved; here it is logged.
            Err(e) => log::write(&e),
        }
    }

    /// The lock names the watch says to hold besides Multi-Roblox's own, for the next time it takes them.
    pub fn apply_watch_names(&self) {
        let folder = self.installs().client.map(|c| c.folder);
        multi_instance::set_extra_names(&roblox_watch::extra_names(folder.as_deref(), multi_instance::EVENT_NAME));
    }

    /// The build Multi-Roblox meets next and what the watch says about it: the installed one, or the live one when a
    /// launcher updates to it before starting Roblox (an older build here, not started directly).
    /// Returns (version, hash, check, worked); worked: two windows played together on that build here, which outweighs
    /// anything the watch says.
    pub fn watched_build(&self) -> (Option<String>, Option<String>, Option<Build>, bool) {
        let installed = self.installs().client;
        let rv = self.roblox_version_dto();
        let live = roblox_watch::current().and_then(|c| c.live.clone());
        let updates_first = rv.up_to_date == Some(false) && !rv.direct && live.is_some();
        let (hash, version) = match (&live, updates_first) {
            (Some(live), true) => (Some(live.hash.clone()), Some(live.version.clone())),
            _ => (installed.as_ref().map(|i| i.folder.clone()), installed.as_ref().map(|i| i.version.clone())),
        };
        let check = roblox_watch::for_hash(hash.as_deref());
        let worked = hash.as_ref().is_some_and(|h| prefs::lock().multi_worked.contains(h));
        (version, hash, check, worked)
    }

    /// Said before opening several windows when the Roblox watch found the single-window lock changed in their build
    /// and two windows haven't played together on it here since.
    pub fn watch_warning(&self) -> Option<String> {
        let (version, _, check, worked) = self.watched_build();
        let check = check?;
        if worked {
            return None;
        }
        let version = version.unwrap_or_default();
        match check.verdict.as_str() {
            "changed" => Some(l::f("Bloxkeep's Roblox watch found that Roblox {0} changed its single-window code, so Multi-Roblox may not work. If Roblox closes a window when the next one opens, Bloxkeep will say so.", &[&version])),
            "renamed" => Some(l::f("Roblox {0} renamed its single-window lock. Bloxkeep holds the new name as well, but that isn't proven yet. If Roblox closes a window when the next one opens, Bloxkeep will say so.", &[&version])),
            "gone" => Some(l::f("Roblox {0} no longer has the single-window lock Multi-Roblox holds, so Roblox will probably close each window when the next one opens.", &[&version])),
            _ => None,
        }
    }

    pub fn watch_dto(&self) -> Value {
        let (version, hash, check, worked) = self.watched_build();
        let next = roblox_watch::current().and_then(|c| c.next.clone());
        let installed_folder = self.installs().client.map(|c| c.folder);
        json!({
            "readAt": roblox_watch::read_at(),
            "version": version,
            "updatesFirst": hash.is_some() && hash != installed_folder,
            "check": check.map(|c| c.verdict),
            "worked": worked,
            "next": next.map(|n| json!({ "version": n.version, "check": n.verdict })),
            "held": multi_instance::extra_held(),
        })
    }
}
