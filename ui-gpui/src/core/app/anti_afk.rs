// AppController.AntiAfk.cs: anti-AFK: nudging the Roblox windows before Roblox kicks them for being idle, and the idle
// kicks told.
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    fmt,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use indexmap::IndexMap;

use super::App;
use super::presets::secs;
use super::rejoin::{BOT_CHECK_HOLD, IDLE_KICK};
use crate::core::{
    anti_afk::{self, INTERVAL, LAST_CHANCE, PAUSE_NEEDED, USER_IDLE_NEEDED},
    base::Stamp,
    input_turn, l, log,
    roblox_input::RobloxInput,
    roblox_processes, rt,
};

pub const NUDGE_RETRY: Duration = Duration::from_secs(60);
pub const MAX_NUDGE_RETRY: Duration = Duration::from_secs(15 * 60);
/// How often the nudge's "keep going" answer is read again on the core's thread while a nudge runs.
// PORT: the C# lambda read the App from the nudge's own thread; here the core refreshes a flag the nudge reads.
const KEEP_GOING_EVERY: Duration = Duration::from_millis(100);

/// Why anti-AFK last held back from a due window: said if Roblox kicks it for idling anyway.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AfkHeld {
    InUse,
    FullScreen,
    Locked,
    Refused,
    MacroBusy,
}

impl fmt::Display for AfkHeld {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            AfkHeld::InUse => "InUse",
            AfkHeld::FullScreen => "FullScreen",
            AfkHeld::Locked => "Locked",
            AfkHeld::Refused => "Refused",
            AfkHeld::MacroBusy => "MacroBusy",
        })
    }
}

/// When the person's input last reached a process's window; None when not known.
pub type ReachedAt = Box<dyn Fn(i32) -> Option<Stamp>>;

#[derive(Default)]
pub struct AntiAfkState {
    /// Set up on the first process tick.
    pub roblox_input: RefCell<Option<RobloxInput>>,
    pub roblox_input_tried: Cell<bool>,
    /// When the person's input last reached a process's window; None when not known. A field, so the checks can answer.
    pub roblox_input_at: RefCell<Option<ReachedAt>>,
    /// Last time each client (by process id) got input: from the person playing it or from anti-AFK.
    pub last_input: RefCell<HashMap<i32, Stamp>>,
    pub idle_kick_reported: RefCell<HashSet<i32>>,
    /// Windows a nudge couldn't reach (focus refused, PC locked): not tried again before this.
    pub nudge_retry_at: RefCell<HashMap<i32, Stamp>>,
    /// Nudges in a row each window missed: the wait before the next one doubles.
    pub nudge_misses: RefCell<HashMap<i32, i32>>,
    /// Anti-AFK has seen the windows that were already open when Bloxkeep started (or was unlocked).
    pub afk_adopted: Cell<bool>,
    /// Why anti-AFK last held back from a due window: said if Roblox kicks it for idling anyway.
    pub afk_held: RefCell<IndexMap<i32, AfkHeld>>,
    /// What error.log was last told of held windows and a bot check: each logged once until it changes.
    pub afk_held_logged: RefCell<Option<String>>,
    pub afk_bot_check_logged: Cell<bool>,
}

fn hh_mm_ss(at: Stamp) -> String {
    let c = at.local();
    format!("{:02}:{:02}:{:02}", c.hour, c.minute, c.second)
}

impl App {
    /// After Settings' minutes without input (`afk_every`, or since joining), nudges every in-server Roblox window, also
    /// ones opened outside Bloxkeep: at once while the PC is idle, else in a short pause of the person's typing. A preset's own option decides for its windows (`anti_afk_on`).
    pub fn run_anti_afk(self: &Rc<Self>, now: Stamp, windows: &HashMap<i32, Vec<isize>>) {
        let st = &self.anti_afk;
        st.last_input.borrow_mut().retain(|pid, _| windows.contains_key(pid));
        st.nudge_retry_at.borrow_mut().retain(|pid, _| windows.contains_key(pid));
        st.nudge_misses.borrow_mut().retain(|pid, _| windows.contains_key(pid));
        st.afk_held.borrow_mut().retain(|pid, _| windows.contains_key(pid));
        st.idle_kick_reported.borrow_mut().retain(|pid| windows.contains_key(pid));
        // A nudge cut short (Bloxkeep closed in the middle of one) left its window unseen and click-through. Not while a
        // window is being nudged or pictured (/pc screenshot): it is unseen on purpose then.
        if !self.controller.nudging.get() && !self.remote.picturing.get() && !input_turn::busy() {
            let all: Vec<isize> = windows.values().flatten().copied().collect();
            anti_afk::recover(&all);
        }
        if self.controller.vault.borrow().is_none() || windows.is_empty() {
            return;
        }
        let on: HashSet<i32> = windows.keys().copied().filter(|pid| self.anti_afk_on(*pid)).collect();
        if on.is_empty() {
            return;
        }
        self.report_idle_kicks(&on);
        // A window already open when Bloxkeep started may have had no input for nearly 20 minutes, so it is due at once,
        // even while the PC is in use: counting from the start had Roblox kick it first.
        for &pid in &on {
            if st.last_input.borrow().contains_key(&pid) {
                continue;
            }
            let joined = self.controller.logs.borrow().state_of(pid).filter(|s| s.status == "ingame").map(|s| s.at);
            let at = match joined {
                Some(at) => at,
                None if st.afk_adopted.get() => now,
                None => now.plus_secs(-secs(LAST_CHANCE)),
            };
            st.last_input.borrow_mut().insert(pid, at);
        }
        st.afk_adopted.set(true);
        if self.controller.nudging.get() || self.remote.picturing.get() {
            return;
        }
        // A nudge cancelled a bot check being solved twice (2026-10-27): none goes while one is in front or for
        // BOT_CHECK_HOLD after it came. Not for good: an unsolved check left every other window kicked after 20 minutes.
        let front = roblox_processes::foreground_pid();
        let checking = {
            let logs = self.controller.logs.borrow();
            windows.keys().any(|&pid| {
                logs.join_challenged_at(pid).is_some_and(|at| {
                    logs.state_of(pid).is_some_and(|s| s.status == "joining") && (pid == front || now.secs_since(at) < secs(BOT_CHECK_HOLD))
                })
            })
        };
        if checking {
            if !st.afk_bot_check_logged.get() {
                log::note("Anti-AFK: held back while a Roblox bot check is open.");
            }
            st.afk_bot_check_logged.set(true);
            return;
        }
        st.afk_bot_check_logged.set(false);
        let due = self.anti_afk_due(now, &on, windows, anti_afk::locked(), anti_afk::idle_time(), roblox_processes::foreground_pid(), &anti_afk::full_screen_in_front);
        // Why due windows wait, logged once each time it changes.
        let held = {
            let map = st.afk_held.borrow();
            let mut kinds: Vec<AfkHeld> = map.values().copied().collect();
            kinds.sort();
            kinds.dedup();
            kinds.iter().map(|k| format!("{} {}", map.values().filter(|v| *v == k).count(), k)).collect::<Vec<_>>().join(", ")
        };
        if st.afk_held_logged.borrow().as_deref() != Some(held.as_str()) && !held.is_empty() {
            let pids = st.afk_held.borrow().keys().map(|p| p.to_string()).collect::<Vec<_>>().join(", ");
            log::note(&format!("Anti-AFK: due Roblox window(s) held back: {held} (pids {pids})."));
        }
        *st.afk_held_logged.borrow_mut() = Some(held);
        if !due.is_empty() {
            let clients: Vec<(i32, isize)> = due.iter().map(|pid| (*pid, windows[pid][0])).collect();
            let app = self.clone();
            // C#: `_ = NudgeAsync(...)`.
            rt::spawn(async move { app.nudge(clients).await });
        }
    }

    /// The windows to nudge now; why the others due wait is noted in `afk_held`. `idle`: anti-AFK's own nudges don't
    /// count. `full_screen`: asked only when it matters.
    #[allow(clippy::too_many_arguments)]
    pub fn anti_afk_due(
        &self,
        now: Stamp,
        on: &HashSet<i32>,
        windows: &HashMap<i32, Vec<isize>>,
        locked: bool,
        idle: Duration,
        foreground: i32,
        full_screen: &dyn Fn() -> bool,
    ) -> Vec<i32> {
        let st = &self.anti_afk;
        let every = self.afk_every();
        let due = |pid: i32, after: Duration| -> bool {
            windows.get(&pid).is_some_and(|w| !w.is_empty())
                && self.controller.logs.borrow().state_of(pid).is_none_or(|s| s.status == "ingame")
                && !self.on_home_screen(pid)
                && now.secs_since(self.last_input(pid)) >= secs(after)
        };
        let waiting = |pid: i32| -> bool { st.nudge_retry_at.borrow().get(&pid).is_some_and(|retry| now < *retry) };
        // Locked: no window can come to the front (a minimised one would be restored and minimised for nothing).
        if locked {
            for &pid in on.iter().filter(|pid| due(**pid, every)) {
                st.afk_held.borrow_mut().insert(pid, AfkHeld::Locked);
            }
            return Vec::new();
        }
        // Input that really reached a Roblox window (raw input, RobloxInput). Without it, any input with a Roblox window
        // in front counts for it: a mouse on another monitor too, which Roblox never got.
        let reached_at = st.roblox_input_at.borrow();
        if let Some(reached_at) = reached_at.as_ref() {
            for &pid in windows.keys() {
                if let Some(at) = reached_at(pid) {
                    let known = st.last_input.borrow().get(&pid).copied();
                    if known.is_none_or(|k| at > k) && at <= now {
                        st.last_input.borrow_mut().insert(pid, at);
                        st.afk_held.borrow_mut().shift_remove(&pid);
                    }
                }
            }
        } else if idle < USER_IDLE_NEEDED && windows.contains_key(&foreground) {
            st.last_input.borrow_mut().insert(foreground, now);
            st.afk_held.borrow_mut().shift_remove(&foreground);
        }
        drop(reached_at);
        if idle >= USER_IDLE_NEEDED {
            return on.iter().copied().filter(|pid| due(*pid, every) && !waiting(*pid)).collect();
        }
        // PC in use: a due window goes all the same, in a two-second pause (mid-typing, a key would reach Roblox), unless a
        // full-screen app is in front. (It used to wait for an idle minute until 18 minutes: a game that kicks sooner than
        // Roblox, Pet Simulator 99 at about 15, took the window first; a player's log, 2026-11-20.)
        let last: HashSet<i32> = on.iter().copied().filter(|pid| due(*pid, every)).collect();
        for &pid in on.iter().filter(|pid| due(**pid, every)) {
            st.afk_held.borrow_mut().insert(pid, AfkHeld::InUse);
        }
        // The next idle spell tries every other window again from the start.
        let others: Vec<i32> = st.nudge_retry_at.borrow().keys().copied().filter(|pid| !last.contains(pid)).collect();
        for pid in others {
            st.nudge_retry_at.borrow_mut().remove(&pid);
            st.nudge_misses.borrow_mut().remove(&pid);
        }
        if last.is_empty() || idle < PAUSE_NEEDED {
            return Vec::new();
        }
        if !windows.contains_key(&foreground) && full_screen() {
            for &pid in &last {
                st.afk_held.borrow_mut().insert(pid, AfkHeld::FullScreen);
            }
            return Vec::new();
        }
        last.into_iter().filter(|pid| !waiting(*pid)).collect()
    }

    /// How long a window goes without input before it is pressed for: Settings' 5 to 18 minutes (12 before a vault).
    pub fn afk_every(&self) -> Duration {
        let minutes = self.vault(|d| d.settings.anti_afk_minutes).unwrap_or(INTERVAL.as_secs() as i32 / 60);
        Duration::from_secs(minutes.clamp(5, 18) as u64 * 60)
    }

    /// 1 min, doubling to 15, but never past the window's last chance.
    pub fn nudge_retry_at(now: Stamp, misses: i32, last_input: Stamp) -> Stamp {
        let shift = (misses - 1).min(4).max(0) as u32;
        let wait = (secs(NUDGE_RETRY) * f64::from(1u32 << shift)).min(secs(MAX_NUDGE_RETRY));
        let later = now.plus_secs(wait);
        let last_chance = last_input.plus_secs(secs(LAST_CHANCE));
        if later < last_chance {
            later
        } else if last_chance > now.plus_secs(secs(NUDGE_RETRY)) {
            last_chance
        } else {
            now.plus_secs(secs(NUDGE_RETRY))
        }
    }

    /// A window Bloxkeep opened on Roblox's home screen that has been in no game since. Roblox kicks nobody from its
    /// home screen, so it is never nudged: the owner's sat there minimised and was brought up every 12 minutes (their
    /// log, 2026-11-18: "tôi có đang ở trong game đâu mà anti afk vẫn chạy"). A window whose log isn't known for another
    /// reason (open before Bloxkeep started) still counts as in a game, as before.
    fn on_home_screen(&self, pid: i32) -> bool {
        self.controller.running.borrow().values().any(|(of, launch)| *of == pid && launch.home_screen && launch.target.is_none())
            && self.controller.logs.borrow().state_of(pid).is_none()
    }

    pub fn last_input(&self, pid: i32) -> Stamp {
        let now = Stamp::utc_now();
        let last = self.anti_afk.last_input.borrow().get(&pid).copied().unwrap_or(now);
        // Roblox counts idle time from joining a server (a server change starts it over). A time ahead of now was logged
        // before the clock was set back (see move_clock_back): it would hold the nudge back by as much.
        let since = match self.controller.logs.borrow().state_of(pid) {
            Some(log) if log.status == "ingame" && log.at > last && log.at <= now => log.at,
            _ => last,
        };
        if since > now { now } else { since }
    }

    /// A Roblox window is brought forward or shown unseen for a moment right now (a nudge, a picture, a macro's turn): what
    /// moves, sizes or limits windows on a timer leaves them alone meanwhile.
    pub fn windows_in_turn(&self) -> bool {
        self.controller.nudging.get() || self.remote.picturing.get() || input_turn::busy()
    }

    pub async fn nudge(self: &Rc<Self>, clients: Vec<(i32, isize)>) {
        // Cleared when this ends, also by a panic: a stuck flag would hold back every window timer for good.
        let _nudging = crate::core::base::Busy::set(&self.controller.nudging);
        let mut turn: Option<input_turn::Lease> = None;
        // try
        'run: {
            // In turn with macros and /pc screenshot (input_turn.rs). A window a macro played into meanwhile has had its input.
            let asked = Stamp::utc_now();
            // A macro's turn holds them back: an idle kick meanwhile is told as the macro's doing.
            if input_turn::holder().as_deref() == Some("macro") {
                log::note(&format!("Anti-AFK: {} due Roblox window(s) wait for a macro's turn to end.", clients.len()));
                for (pid, _) in &clients {
                    self.anti_afk.afk_held.borrow_mut().insert(*pid, AfkHeld::MacroBusy);
                }
            }
            match rt::blocking(|| input_turn::enter("afk", None)).await {
                Ok(lease) => turn = Some(lease),
                Err(ex) => {
                    // PORT: the C# task ended with the exception unobserved (its finally ran); here it is logged.
                    log::write(&ex);
                    break 'run;
                }
            }
            let had_input = |pid: i32| self.anti_afk.last_input.borrow().get(&pid).is_some_and(|at| *at >= asked);
            for (pid, _) in clients.iter().filter(|c| had_input(c.0)) {
                if self.anti_afk.afk_held.borrow().get(pid) == Some(&AfkHeld::MacroBusy) {
                    self.anti_afk.afk_held.borrow_mut().shift_remove(pid);
                }
            }
            let clients: Vec<(i32, isize)> = clients.iter().copied().filter(|c| !had_input(c.0)).collect();
            if clients.is_empty() {
                log::note("Anti-AFK: nudge dropped, every due window had input meanwhile (a macro played into it).");
                break 'run;
            }
            // Turning anti-AFK off (or locking, or quitting) stops it between two windows.
            let keep_going = |app: &App| !app.controller.disposed.get() && clients.iter().any(|c| app.anti_afk_on(c.0));
            let keep = Arc::new(AtomicBool::new(keep_going(self)));
            let finished = Rc::new(Cell::new(false));
            // Set when the nudge is over, also by a panic, so the loop below always stops.
            struct Finished(Rc<Cell<bool>>);
            impl Drop for Finished {
                fn drop(&mut self) {
                    self.0.set(true);
                }
            }
            let finish = Finished(finished.clone());
            {
                let (app, keep, finished, clients) = (self.clone(), keep.clone(), finished.clone(), clients.clone());
                rt::spawn(async move {
                    while !finished.get() {
                        keep.store(!app.controller.disposed.get() && clients.iter().any(|c| app.anti_afk_on(c.0)), Ordering::SeqCst);
                        rt::sleep(KEEP_GOING_EVERY).await;
                    }
                });
            }
            let handles: Vec<isize> = clients.iter().map(|c| c.1).collect();
            let reached = {
                let keep = keep.clone();
                let key = self.vault(|d| d.settings.anti_afk_key.clone()).unwrap_or_default();
                rt::blocking(move || anti_afk::nudge(&handles, &|| keep.load(Ordering::SeqCst), Some(&|_| anti_afk::press_named(&key)))).await
            };
            drop(finish);
            for (pid, window) in &clients {
                let pid = *pid;
                if reached.contains(window) {
                    self.anti_afk.last_input.borrow_mut().insert(pid, Stamp::utc_now());
                    self.anti_afk.nudge_retry_at.borrow_mut().remove(&pid);
                    self.anti_afk.nudge_misses.borrow_mut().remove(&pid);
                    self.anti_afk.afk_held.borrow_mut().shift_remove(&pid);
                } else {
                    // Not reached: the person came back meanwhile, or Windows kept it in the background.
                    let why = if anti_afk::idle_time() < USER_IDLE_NEEDED {
                        AfkHeld::InUse
                    } else if anti_afk::locked() {
                        AfkHeld::Locked
                    } else {
                        AfkHeld::Refused
                    };
                    self.anti_afk.afk_held.borrow_mut().insert(pid, why);
                    // Backs off, or a minimised window would be restored and minimised again every minute.
                    let misses = {
                        let mut all = self.anti_afk.nudge_misses.borrow_mut();
                        let n = all.get(&pid).copied().unwrap_or(0) + 1;
                        all.insert(pid, n);
                        n
                    };
                    let retry = App::nudge_retry_at(Stamp::utc_now(), misses, self.last_input(pid));
                    self.anti_afk.nudge_retry_at.borrow_mut().insert(pid, retry);
                    log::note(&format!("Anti-AFK: Roblox window {pid} not reached ({why}, miss {misses}); tried again at {}.", hh_mm_ss(retry)));
                }
            }
        }
        // finally
        if let Some(lease) = turn {
            lease.dispose();
        }
    }

    /// An idle kick despite anti-AFK: tells the person why it didn't reach the window, and error.log (no account data).
    pub fn report_idle_kicks(&self, on: &HashSet<i32>) {
        let running: Vec<(crate::core::base::Guid, i32)> = self.controller.running.borrow().iter().map(|(id, r)| (*id, r.0)).collect();
        for (id, pid) in running {
            if !on.contains(&pid) {
                continue;
            }
            let Some(kick) = self.controller.logs.borrow().state_of(pid).filter(|s| s.status == "disconnected" && s.reason == Some(IDLE_KICK)) else {
                continue;
            };
            if !self.anti_afk.idle_kick_reported.borrow_mut().insert(pid) {
                continue;
            }
            let Some(account) = self.find_account(id) else { continue };
            // Kicked before this run (Bloxkeep restarted while the window stayed on Roblox's message): said back then.
            if kick.at < self.relaunch.started.get() {
                continue;
            }
            let held = self.anti_afk.afk_held.borrow().get(&pid).copied();
            let last = self.anti_afk.last_input.borrow().get(&pid).copied();
            log::note(&format!(
                "Idle kick with anti-AFK on: Roblox pid {pid}, last input {}, held back by: {}.",
                last.map_or_else(|| "unknown".to_string(), hh_mm_ss),
                held.map_or_else(|| "nothing noted".to_string(), |h| h.to_string())
            ));
            let name = self.name(&account);
            let text = match held {
                Some(AfkHeld::InUse) => l::f("{0} was kicked for being AFK: the PC was in use without a pause of two seconds, so anti-AFK couldn't step in.", &[&name]),
                Some(AfkHeld::FullScreen) => l::f("{0} was kicked for being AFK: a full-screen app was in front, and anti-AFK doesn't interrupt one.", &[&name]),
                Some(AfkHeld::Locked) => l::f("{0} was kicked for being AFK: the PC was locked, and anti-AFK can't reach Roblox then.", &[&name]),
                Some(AfkHeld::Refused) => l::f("{0} was kicked for being AFK: Windows didn't let Bloxkeep bring its window to the front.", &[&name]),
                Some(AfkHeld::MacroBusy) => l::f("{0} was kicked for being AFK: a macro kept the PC, so anti-AFK had to wait.", &[&name]),
                None => l::f("{0} was kicked for being AFK.", &[&name]),
            };
            self.notify(&text, "warn", "afk", account.id, true, None);
        }
    }
}
