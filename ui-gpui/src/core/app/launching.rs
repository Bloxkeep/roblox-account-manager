//! Opening Roblox for the chosen accounts, one after another, into the game and server asked for.
//! (AppController.Launching.cs)
use std::{
    cell::RefCell,
    collections::HashSet,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use futures::future::LocalBoxFuture;
use serde_json::{Value, json};

use super::{
    App,
    controller::{LaunchRef, BACKGROUND_SWEEP, MAX_OPEN_RETRIES, MAX_RETRY_DELAY, NETWORK_RETRY_DELAY, PENDING_TIMEOUT, PendingLaunch, RejoinRequest, DEFAULT_CLIENT_MB, MB},
    notify::SKIPPABLE_LAUNCH_WARNINGS,
    presets::{AFK_PROFILE, LaunchResult},
};
use crate::core::{
    base::{Cancel, Fail, Guid, Res, Stamp},
    close_plan::{Cause, End, Open, Then, Who},
    game_target::GameTarget,
    l, log,
    models::Account,
    multi_instance, prefs,
    roblox_api::{self, ServerInfo, is_fail_kind, realm},
    roblox_flags, roblox_game_settings, roblox_installs, roblox_launcher,
    roblox_log_watcher::ClientState,
    roblox_processes, rt, system_memory,
    vault::Vault,
    win_kit::{self, Rect, Size},
    window_desk::{Arrangement, Lift, choose_tiles},
    window_report,
};

// ───────── small shared helpers of the launching and session files ─────────

/// A launch and the window it got: (process id, the launch), as `_running` holds them.
pub type Run = (i32, Rc<PendingLaunch>);

/// "Chooses the server once the launch has started": gets the target and how many accounts will open.
pub type PickServer = Box<dyn Fn(GameTarget, i32, Cancel) -> LocalBoxFuture<'static, Res<GameTarget>>>;
/// A server for each account as it opens.
pub type EachServer = Rc<dyn Fn(GameTarget, Cancel) -> LocalBoxFuture<'static, Res<GameTarget>>>;

/// An id as the log writes it: its first 8 characters.
pub(super) fn short(id: Guid) -> String {
    id.to_string()[..8].to_string()
}

/// DateTime.MinValue (0001-01-01), what a DateTime field never set holds.
pub(super) fn min_stamp() -> Stamp {
    Stamp::from_unix_secs(-62_135_596_800)
}

pub(super) fn secs(d: Duration) -> f64 {
    d.as_secs_f64()
}

/// What a client's log says now.
pub(super) fn log_of(app: &App, pid: i32) -> Option<ClientState> {
    app.controller.logs.borrow().state_of(pid)
}

/// `_running`, copied out (no borrow kept).
pub(super) fn running(app: &App) -> Vec<(Guid, Run)> {
    app.controller.running.borrow().iter().map(|(id, r)| (*id, r.clone())).collect()
}

pub(super) fn run_of(app: &App, id: Guid) -> Option<Run> {
    app.controller.running.borrow().get(&id).cloned()
}

pub(super) fn is_running(app: &App, id: Guid) -> bool {
    app.controller.running.borrow().contains_key(&id)
}

pub(super) fn running_pids(app: &App) -> HashSet<i32> {
    app.controller.running.borrow().values().map(|r| r.0).collect()
}

pub(super) fn pending(app: &App) -> Vec<Rc<PendingLaunch>> {
    app.controller.pending.borrow().clone()
}

pub(super) fn pending_count(app: &App) -> usize {
    app.controller.pending.borrow().len()
}

/// The vault open now (`_vault`), compared by reference as the C# does.
pub(super) fn vault_now(app: &App) -> Option<Arc<Vault>> {
    app.controller.vault.borrow().clone()
}

pub(super) fn same_vault(app: &App, vault: &Arc<Vault>) -> bool {
    vault_now(app).is_some_and(|v| Arc::ptr_eq(&v, vault))
}

/// RequireVault(): the open vault, or what it throws when locked.
pub(super) fn require_vault(app: &App) -> Res<Arc<Vault>> {
    app.vault(|_| ())?;
    vault_now(app).ok_or_else(|| Fail::other("locked"))
}

/// Task.Delay(d, ct): Err (TaskCanceledException) when called off first.
pub(super) async fn delay(ct: &Cancel, d: Duration) -> Res<()> {
    if rt::sleep_or(ct, d).await { Ok(()) } else { Err(Fail::other("cancelled")) }
}

/// A token called off with `parent` or after `limit` (CreateLinkedTokenSource + CancelAfter).
pub(super) fn linked(parent: &Cancel, limit: Duration) -> Cancel {
    let child = Cancel::new();
    let (p, c) = (parent.clone(), child.clone());
    // On the core's timers (no thread): every caller is a task of the core's.
    rt::spawn(async move {
        let until = Instant::now() + limit;
        while Instant::now() < until && !p.is_cancelled() && !c.is_cancelled() {
            rt::sleep(Duration::from_millis(25)).await;
        }
        c.cancel();
    });
    child
}

// Collections of launches held by reference (ReferenceEqualityComparer), as lists here.
pub(super) fn has_launch(list: &[Rc<PendingLaunch>], l: &Rc<PendingLaunch>) -> bool {
    list.iter().any(|x| Rc::ptr_eq(x, l))
}
/// HashSet.Add: false when already there.
pub(super) fn add_launch(list: &mut Vec<Rc<PendingLaunch>>, l: &Rc<PendingLaunch>) -> bool {
    if has_launch(list, l) {
        return false;
    }
    list.push(l.clone());
    true
}
pub(super) fn remove_launch(list: &mut Vec<Rc<PendingLaunch>>, l: &Rc<PendingLaunch>) -> bool {
    match list.iter().position(|x| Rc::ptr_eq(x, l)) {
        Some(i) => {
            list.remove(i);
            true
        }
        None => false,
    }
}
// `_closeWhenClaimed` and `_pendingSeenAlive`: keyed by reference (controller::LaunchRef).
pub(super) fn closing_has(app: &App, l: &Rc<PendingLaunch>) -> bool {
    app.controller.close_when_claimed.borrow().contains(&LaunchRef(l.clone()))
}
pub(super) fn closing_add(app: &App, l: &Rc<PendingLaunch>) -> bool {
    app.controller.close_when_claimed.borrow_mut().insert(LaunchRef(l.clone()))
}
pub(super) fn closing_remove(app: &App, l: &Rc<PendingLaunch>) -> bool {
    app.controller.close_when_claimed.borrow_mut().remove(&LaunchRef(l.clone()))
}
pub(super) fn seen_alive(app: &App, l: &Rc<PendingLaunch>) -> Option<Stamp> {
    app.controller.pending_seen_alive.borrow().get(&LaunchRef(l.clone())).copied()
}
pub(super) fn set_seen_alive(app: &App, l: &Rc<PendingLaunch>, at: Stamp) {
    app.controller.pending_seen_alive.borrow_mut().insert(LaunchRef(l.clone()), at);
}
pub(super) fn get_by<V: Clone>(map: &[(Rc<PendingLaunch>, V)], l: &Rc<PendingLaunch>) -> Option<V> {
    map.iter().find(|(k, _)| Rc::ptr_eq(k, l)).map(|(_, v)| v.clone())
}
pub(super) fn set_by<V>(map: &mut Vec<(Rc<PendingLaunch>, V)>, l: &Rc<PendingLaunch>, v: V) {
    match map.iter_mut().find(|(k, _)| Rc::ptr_eq(k, l)) {
        Some(e) => e.1 = v,
        None => map.push((l.clone(), v)),
    }
}
pub(super) fn remove_by<V>(map: &mut Vec<(Rc<PendingLaunch>, V)>, l: &Rc<PendingLaunch>) -> Option<V> {
    let i = map.iter().position(|(k, _)| Rc::ptr_eq(k, l))?;
    Some(map.remove(i).1)
}

/// A Win32Exception or FileNotFoundException: Roblox's program wouldn't start.
pub(super) fn is_start_error(ex: &Fail) -> bool {
    is_fail_kind(ex, "Win32Exception") || is_fail_kind(ex, "FileNotFoundException")
}

/// RobloxApiException { SignInRefused: true }.
/// A launch held back by hold_for_roblox_update (C#'s RobloxUpdateHold, a UserError): a rejoin waits and tries again.
pub const UPDATE_HOLD_TEXT: &str = "Roblox would install an update as this window opens, and that closes every Roblox window open. Close the other Roblox windows first, or use Close, update, reopen under Settings › Roblox version.";

/// Also a launcher's hold ("{0} would update Roblox…"): both end with the same advice.
pub fn is_update_hold(ex: &Fail) -> bool {
    let tail = |s: &str| s.rsplit_once(". ").map(|(_, t)| t.to_string()).unwrap_or_default();
    matches!(ex, Fail::User(m) if *m == l::t(UPDATE_HOLD_TEXT) || (!tail(m).is_empty() && tail(m) == tail(&l::t(UPDATE_HOLD_TEXT))))
}

pub(super) fn sign_in_refused(ex: &Fail) -> bool {
    ex.sign_in_refused()
}

/// RobloxApiException { Moderated: true }.
pub(super) fn moderated(ex: &Fail) -> bool {
    ex.moderated()
}

// ───────── AppController.Launching.cs ─────────

#[derive(Default)]
pub struct LaunchingState {
    /// Launches whose client quit early, waiting for the client it may have handed over to: the pid that quit
    /// (its children are the first choice) and what its log said last.
    pub handed_over: RefCell<Vec<(Rc<PendingLaunch>, (i32, Option<ClientState>))>>,
    /// Launches whose window didn't come within PENDING_TIMEOUT: one appearing within LATE_TIMEOUT more is still
    /// claimed for it (see track_processes).
    pub late: RefCell<Vec<Rc<PendingLaunch>>>,
    /// Launches that took `launch_cts` so far (see launch_accounts).
    pub launch_generation: std::cell::Cell<u64>,
}

/// Servers found closed in one launch before Roblox is left to pick one for each account.
pub const MAX_SERVER_SWAPS: usize = 3;
/// A normal join gets a server about a second after the start (at most 1.8 s in 31 logs, 2026-10-21), but one
/// waiting for room logs nothing (17-38 s for four accounts sent to one server, 2026-10-16).
pub const SERVER_CHECK: Duration = Duration::from_secs(12);
/// Then the launch goes on without an answer (the log not found, the window gone).
pub const SERVER_CHECK_LIMIT: Duration = Duration::from_secs(25);
/// How long after its window a new client may still be closing the one it replaces.
pub const REPLACED_GRACE: Duration = Duration::from_secs(15);
pub const LATE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// How long windows opened into a game are waited for to be in it before they are tiled.
pub const JOIN_WAIT_FOR_TILES: Duration = Duration::from_secs(60);
/// The longest the next account of a launch waits for the one before it to join its game (from that one's start; the
/// wait between accounts, if longer): a window stuck loading or in a queue doesn't hold the rest back.
pub const JOIN_WAIT_MAX: Duration = Duration::from_secs(20);
/// How long after joining its game a window is tiled: Roblox sizes its window again as it joins (maximised again, if
/// it was last closed so), and a tile given before that is undone.
pub const TILE_SETTLE: Duration = Duration::from_secs(3);

/// In the app's language ("1.5 GB", "1,5 ГБ"): lang.rs › mb.
fn format_mb(mb: i64) -> String {
    crate::lang::by_code(&l::language()).unwrap_or_else(crate::lang::cur).mb(mb as f64)
}

/// By the browser tracker id on its command line (roblox_processes::launch_tracker_of): one naming another is someone
/// else's (another launch, the website); one naming none, not read, or a launch without one, may be.
pub(super) fn may_be_launch_of(tracker: Option<&str>, launch: &PendingLaunch) -> bool {
    !matches!(tracker, Some(t) if !t.is_empty()) || launch.tracker.is_none() || tracker == launch.tracker.as_deref()
}

/// It is that launch's, whichever started first.
pub(super) fn is_launch_of(tracker: Option<&str>, launch: &PendingLaunch) -> bool {
    matches!(tracker, Some(t) if !t.is_empty()) && tracker == launch.tracker.as_deref()
}

/// For error.log: an error's kind and Roblox's flags, never its text (a sentence may name an account).
pub fn launch_fail_kind(ex: &Fail) -> String {
    match ex {
        Fail::Roblox { cookie_invalid, rate_limited, server_error, .. } => {
            format!(
                "RobloxApiException{}{}{}{}{}",
                if *cookie_invalid { " cookie-invalid" } else { "" },
                if *rate_limited { " rate-limited" } else { "" },
                if *server_error { " server-error" } else { "" },
                if sign_in_refused(ex) { " sign-in-refused" } else { "" },
                if moderated(ex) { " moderated" } else { "" }
            )
        }
        _ if is_fail_kind(ex, "Win32Exception") => {
            // The native code: "Win32Exception: 5 …", or in brackets as the launcher words it ("ShellExecute failed (2)").
            // A failed spawn ends in Rust's own "(os error 2)": the bracket then starts with words, so skip them to the code.
            let message = ex.message();
            let rest = message.rsplit_once('(').map(|x| x.1.strip_prefix("os error ").unwrap_or(x.1)).or_else(|| message.split_once(':').map(|x| x.1.trim())).unwrap_or("");
            let code: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '-').collect();
            format!("Win32Exception {code}")
        }
        _ => roblox_api::fail_type_name(ex),
    }
}

impl App {
    /// LaunchFailKind, as the other files call it.
    pub fn launch_fail_kind(ex: &Fail) -> String {
        launch_fail_kind(ex)
    }

    pub fn multi_not_ready_text() -> String {
        let mut text = l::t("Multi-Roblox isn't working yet: Roblox was already open before it was turned on.");
        if matches!(multi_instance::find_tray_autostart(), Ok(Some(t)) if t.enabled) {
            text += &format!(" {}", l::t("Roblox also starts by itself in the background when Windows starts."));
        }
        text + " " + &l::t("A new Roblox window may close the old one.")
    }

    pub async fn launch(self: &Rc<Self>, a: &Value) -> Res<Value> {
        let vault = require_vault(self)?;
        let accounts = self.in_launch_order(&Self::ids_of(a)); // several selected: the launch order from the ⋯ menu
        self.check_can_launch(&accounts)?;
        let app_only = Self::bool_of(a, "appOnly");

        let place_input = Self::str_of(a, "placeInput").trim().to_string();
        let job_id = Self::str_of(a, "jobId").trim().to_string();
        self.vault_mut(|d| {
            d.settings.last_place_input = place_input.clone();
            d.settings.last_job_id = job_id.clone();
        })?;
        let mut target = None;
        if !app_only {
            match GameTarget::try_parse(&place_input, &job_id) {
                Ok(t) => target = Some(t),
                Err(error) => return Err(Fail::user(error)),
            }
        }
        let delay_seconds = self.vault(|d| d.settings.launch_delay_seconds)?;
        let result = self.launch_accounts(vault, accounts, target, Self::bool_of(a, "force"), delay_seconds, None, None, None, None).await?;
        Ok(serde_json::to_value(&result)?)
    }

    pub fn check_can_launch(&self, accounts: &[Account]) -> Res<()> {
        if self.controller.launch_cts.borrow().is_some() || self.tray.opening_home.get() {
            return Err(Fail::user(l::t("Already launching — wait a moment or press Stop.")));
        }
        if accounts.is_empty() {
            return Err(Fail::user(l::t("Select at least one account.")));
        }
        Ok(())
    }

    fn banned_not_opened(&self, account: &Account) -> String {
        let n = self.name(account);
        l::f("{0} is banned on Roblox, so it wasn't opened. If the ban is over, Check sign-in clears this.", &[&n])
    }

    /// Callers run check_can_launch first. Without `force`, asks for a confirmation when something looks wrong
    /// (Multi-Roblox, RAM, bans, playability).
    /// `pick_server`: chooses the server once the launch has started, so Stop works meanwhile; gets how many accounts will open.
    /// `each_server`: instead, a server for each account as it opens (every account in one of its own).
    /// `reserved`: the launch taken already (`launch_cts`) by a caller that had to look something up first.
    #[allow(clippy::too_many_arguments)]
    pub async fn launch_accounts(
        self: &Rc<Self>,
        vault: Arc<Vault>,
        mut accounts: Vec<Account>,
        mut target: Option<GameTarget>,
        force: bool,
        delay_seconds: i32,
        pick_server: Option<PickServer>,
        preset_id: Option<Guid>,
        each_server: Option<EachServer>,
        reserved: Option<Cancel>,
    ) -> Res<LaunchResult> {
        // Close, update, reopen or Roblox's own update under way: a window opened now would be closed by it, or make it
        // give up (close_plan.rs O1).
        if let Open::Refuse(no) = self.may_open(Who::Person, None, Stamp::utc_now()) {
            log::note(&format!("Launch: {} account(s) not opened, {}.", accounts.len(), no.words()));
            return Err(Fail::user(App::open_refused_text(no).unwrap_or_default()));
        }
        // Banned accounts aren't opened: their window would only show the ban. Roblox VNG accounts open like the
        // others, with a ticket from Roblox VNG's sign-in (tried from 2026-10-19).
        let left_out: Vec<Guid> = accounts.iter().filter(|acc| acc.banned == Some(true)).map(|acc| acc.id).collect();
        let left_out_text: Vec<String> = accounts.iter().filter(|acc| acc.banned == Some(true)).map(|acc| self.banned_not_opened(acc)).collect();
        if !left_out.is_empty() {
            log::note(&format!("Launch: {} of {} account(s) left out, banned on Roblox.", left_out.len(), accounts.len()));
            if left_out.len() == accounts.len() {
                return Err(Fail::user(left_out_text.join("\n")));
            }
            accounts.retain(|acc| !left_out.contains(&acc.id));
        }
        // Taken before the first await: a second Play meanwhile would pass check_can_launch, and its "switching" list
        // could close the client this one had just opened.
        let launch_start = Stamp::utc_now(); // the windows opened from here are waited for before tiling
        // Asked now: an earlier Close of these accounts no longer holds them (close_plan.rs O2).
        self.close_desk.release_closed(accounts.iter().map(|acc| acc.id));
        let cts = reserved.unwrap_or_default();
        *self.controller.launch_cts.borrow_mut() = Some(cts.clone());
        // PORT: `_launchCts == cts` (a reference compare); base::Cancel doesn't expose its Arc, so a count of the launches
        // that took `launch_cts` stands in: still ours when no launch took it since.
        let generation = self.launching.launch_generation.get() + 1;
        self.launching.launch_generation.set(generation);
        let ct: Cancel = cts.clone();
        let release = |app: &App| {
            let mine = app.launching.launch_generation.get() == generation && app.controller.launch_cts.borrow().is_some();
            if mine {
                *app.controller.launch_cts.borrow_mut() = None;
            }
        };

        let settings = || self.vault(|d| d.settings.clone()).unwrap_or_default();
        // The kinds in SKIPPABLE_LAUNCH_WARNINGS can be answered "Don't ask again" one by one.
        let mut warnings: Vec<(String, String)> = Vec::new();
        let mut switching: Vec<Guid> = Vec::new();
        let mut errors = left_out_text;
        let mut skipped: Vec<String> = Vec::new();
        // Clients a new client closes when Multi-Roblox isn't ready: not drops to rejoin from.
        let mut replaced: Vec<Guid> = Vec::new();
        // Roblox wouldn't sign them in (a warning to accept, a lock): the page offers their roblox.com, which says why.
        let mut refused_sign_in: Vec<Guid> = Vec::new();
        let mut launched: i32 = 0;
        // Past the checks: from then on an error is reported with the result rather than thrown.
        let mut started = false;

        let outcome: Res<Option<LaunchResult>> = async {
            // Resolved before deciding who switches: clients already in that server are compared by its link code.
            if let Some(share_code) = target.as_ref().and_then(|t| t.share_code.clone()) {
                self.progress(Some(&l::t("Reading the share link…")));
                let share_type = target.as_ref().and_then(|t| t.share_type.clone());
                let share = self.resolve_share(&accounts, &share_code, share_type.as_deref(), &ct).await?;
                if share.expired {
                    log::note("Launch: the share link has expired; joining anyway, as the website does.");
                }
                if let Some(t) = target.as_mut() {
                    t.place_id = share.place_id;
                    t.link_code = share.link_code;
                    t.job_id = share.job_id.or(t.job_id.take());
                    t.share_code = None;
                    t.share_type = None;
                    t.launch_data = share.launch_data;
                }
                self.progress(None);
            }
            if let Some(t) = target.clone() {
                self.recheck_game_bans(&accounts, &t, &ct).await?;
            }
            // A preset leaves out the accounts this game banned: nobody may be there to answer a question (tray, schedule).
            if let (Some(_), Some(t)) = (preset_id, target.as_ref()) {
                let game_banned: Vec<Account> = accounts.iter().filter(|acc| self.game_ban_of(acc, Some(t.place_id)).is_some()).cloned().collect();
                if !game_banned.is_empty() {
                    log::note(&format!("Launch: preset left out {} of {} account(s), banned from place {}.", game_banned.len(), accounts.len(), t.place_id));
                    for acc in &game_banned {
                        let n = self.name(acc);
                        errors.push(l::f("{0} was banned from this game, so the preset didn't open it. Play it on its own to try anyway.", &[&n]));
                    }
                    if game_banned.len() == accounts.len() {
                        return Err(Fail::user(errors.join("\n")));
                    }
                    accounts.retain(|acc| !game_banned.iter().any(|b| b.id == acc.id));
                }
            }
            let mut players = roblox_processes::get_players();
            self.apply_watch_names();
            // Only Roblox's notification-area copy runs: the launch ends it anyway (start_session), so it goes first and
            // Multi-Roblox is ready, rather than said not to be.
            if settings().multi_instance
                && (!multi_instance::armed() || multi_instance::stale())
                && !players.is_empty()
                && roblox_processes::visible_windows(&players.iter().map(|p| p.pid).collect::<Vec<_>>()).values().all(|w| w.is_empty())
                && players.iter().all(|p| roblox_processes::is_tray_copy(p.pid, p.created))
                && self.close_tray_copies(&ct).await?
            {
                players = roblox_processes::get_players();
            }
            if settings().multi_instance && (!multi_instance::armed() || multi_instance::stale()) && players.is_empty() {
                multi_instance::enable();
            }
            // Accounts already playing: left alone in the same game, else switched. Roblox kicks a second client of the
            // same account, so the old one is closed first.
            match target.clone() {
                None => {
                    // "Open Roblox" (home screen): clients in a game go back to it.
                    switching = accounts.iter().filter(|acc| run_of(self, acc.id).is_some_and(|run| self.in_game_here(&run))).map(|acc| acc.id).collect();
                    let leaving_game: Vec<Account> =
                        accounts.iter().filter(|acc| switching.contains(&acc.id) && run_of(self, acc.id).is_some_and(|run| self.playing_here(&run))).cloned().collect();
                    if !leaving_game.is_empty() {
                        let n = self.names(&leaving_game);
                        warnings.push(("switch".into(), l::f("{0} will leave the game and go to the Roblox home screen.", &[&n])));
                    }
                }
                Some(t) if accounts.iter().any(|acc| is_running(self, acc.id)) => {
                    let mut places: Vec<(i64, i64)> =
                        accounts.iter().filter(|acc| is_running(self, acc.id)).map(|acc| (self.current_place(acc.id), 0i64)).collect();
                    places.push((t.place_id, 0));
                    places.retain(|p| p.0 > 0);
                    self.load_games(places, false).await?;
                    switching = accounts.iter().filter(|acc| run_of(self, acc.id).is_some_and(|run| self.needs_switch(&run, &t))).map(|acc| acc.id).collect();
                    // Asked only for the ones in a game: one on the home or an error screen loses nothing.
                    let leaving: Vec<Account> = accounts
                        .iter()
                        .filter(|acc| switching.contains(&acc.id) && run_of(self, acc.id).is_some_and(|run| self.leaves_a_game(&run, &t)))
                        .cloned()
                        .collect();
                    if !leaving.is_empty() {
                        let n = self.names(&leaving);
                        warnings.push(("switch".into(), l::f("{0} will leave what they're playing now and open this game instead.", &[&n])));
                    }
                }
                Some(_) => {}
            }
            // Asked as Roblox's own client asks before joining, instead of a window that never gets in. Only on Play: a
            // preset may run with nobody there to answer.
            if let Some(t) = target.clone()
                && preset_id.is_none()
                && !force
            {
                self.progress(Some(&l::t("Asking Roblox whether the accounts can play this game…")));
                let unplayable = self.unplayable(&accounts, t.place_id, &ct).await?;
                self.progress(None);
                // GroupBy keeps the order each reason first came in.
                let mut whys: Vec<(String, Vec<Account>)> = Vec::new();
                for (acc, why) in unplayable {
                    match whys.iter_mut().find(|w| w.0 == why) {
                        Some(w) => w.1.push(acc),
                        None => whys.push((why, vec![acc])),
                    }
                }
                for (why, list) in whys {
                    let n = self.names(&list);
                    warnings.push(("playability".into(), l::f("{0} can't play this game: {1}.", &[&n, &why])));
                }
            }
            // Asked when opened by hand; a preset left them out above.
            if let Some(t) = target.as_ref() {
                let banned_here: Vec<Account> = accounts.iter().filter(|acc| self.game_ban_of(acc, Some(t.place_id)).is_some()).cloned().collect();
                if !banned_here.is_empty() {
                    let n = self.names(&banned_here);
                    warnings.push((
                        "gameBan".into(),
                        l::p(
                            banned_here.len() as i64,
                            "{0} was banned from this game, so Roblox will most likely not let them in (Join Error 600).",
                            "{0} were banned from this game, so Roblox will most likely not let them in (Join Error 600).",
                            &[&n],
                        ),
                    ));
                }
            }
            // The settings as they are past the waits above (the share link, game bans, playability): the C# read its live
            // settings object at each use, so Multi-Roblox or the memory warnings switched meanwhile count.
            let s = settings();
            let roblox_open = roblox_processes::visible_windows(&players.iter().map(|p| p.pid).collect::<Vec<_>>()).values().any(|list| !list.is_empty());
            if (accounts.len() > 1 || roblox_open) && !multi_instance::armed() {
                warnings.push(if s.multi_instance {
                    ("multiNotReady".into(), Self::multi_not_ready_text())
                } else {
                    ("multiOff".into(), l::t("Multi-Roblox is off, so each new Roblox window closes the previous one."))
                });
            } else if accounts.len() > 1 || roblox_open {
                if let Some(watch_warning) = self.watch_warning() {
                    warnings.push(("watch".into(), watch_warning));
                }
            }
            let to_open = accounts.iter().filter(|acc| !is_running(self, acc.id)).count() as i64;
            let memory = system_memory::read();
            let per_client_mb = if !players.is_empty() {
                // Math.Max(400, (long)players.Average(WorkingSet) / MB).
                std::cmp::max(400, (players.iter().map(|p| p.working_set as f64).sum::<f64>() / players.len() as f64) as i64 / MB)
            } else {
                DEFAULT_CLIENT_MB
            };
            let needed_mb = to_open * per_client_mb;
            // The commit limit can run out with RAM to spare when the paging file is small: a client then crashes as it opens.
            let per_client_commit_mb = Self::client_commit_mb(&players);
            if s.memory_warnings && memory.total_mb > 0 && memory.available_mb - needed_mb < memory.total_mb * 8 / 100 {
                let (free, need) = (format_mb(memory.available_mb), format_mb(needed_mb));
                warnings.push((
                    "memory".into(),
                    l::p(
                        to_open,
                        "Only {0} of RAM is free, and {1} more Roblox window needs about {2}. The PC may slow down or Roblox may crash.",
                        "Only {0} of RAM is free, and {1} more Roblox windows need about {2}. The PC may slow down or Roblox may crash.",
                        &[&free, &to_open, &need],
                    ),
                ));
            } else if s.memory_warnings && Self::virtual_memory_short(&memory, to_open as i32, per_client_commit_mb) {
                warnings.push((
                    "memory".into(),
                    l::p(
                        to_open,
                        "RAM is free, but Windows' virtual memory is almost full, so {0} more Roblox window may crash as it opens. Close other apps or increase virtual memory.",
                        "RAM is free, but Windows' virtual memory is almost full, so {0} more Roblox windows may crash as they open. Close other apps or increase virtual memory.",
                        &[&to_open],
                    ),
                ));
            }
            {
                let skip = prefs::lock().skip_confirm.clone();
                warnings.retain(|w| !(SKIPPABLE_LAUNCH_WARNINGS.contains(&w.0.as_str()) && skip.contains(&super::notify::launch_warning_key(&w.0))));
            }
            if !warnings.is_empty() && !force {
                log::note(&format!("Launch: held for a confirmation ({}).", warnings.iter().map(|w| w.0.as_str()).collect::<Vec<_>>().join(", ")));
                return Ok(Some(LaunchResult {
                    needs_confirm: true,
                    message: Some(warnings.iter().map(|w| w.1.as_str()).collect::<Vec<_>>().join("\n\n")),
                    asks: Some(warnings.iter().map(|w| w.0.clone()).collect()),
                    ..Default::default()
                }));
            }

            started = true;
            let where_to = match &target {
                None => "home screen".to_string(),
                Some(t) => format!(
                    "place {}{}",
                    t.place_id,
                    if t.job_id.is_some() {
                        ", a chosen server"
                    } else if t.is_private() {
                        ", private server"
                    } else {
                        ""
                    }
                ),
            };
            log::note(&format!(
                "Launch: {} account(s), {where_to}{}, via {}, Multi-Roblox {}{}{}; windows after opening: {}.",
                accounts.len(),
                preset_id.map(|p| format!(", preset {}", short(p))).unwrap_or_default(),
                s.launch_via,
                if multi_instance::armed() {
                    "ready"
                } else if s.multi_instance {
                    "not ready"
                } else {
                    "off"
                },
                if !switching.is_empty() { format!(", {} switching", switching.len()) } else { String::new() },
                if force { ", confirmed" } else { "" },
                // What Settings' "When a Roblox window opens" (or the preset) says, so error.log shows why windows were
                // or weren't arranged: a user's came up maximised and unarranged with no line to say which was set
                // (2026-11-18).
                match self.preset_for(preset_id).and_then(|p| p.arrange) {
                    Some(true) => "arranged (the preset)",
                    Some(false) => "left as they open (the preset)",
                    None if s.multi_instance && s.multi_instance_tile => "arranged",
                    None if s.maximize_on_open => "maximised",
                    None => "left as Roblox opens them",
                }
            ));
            self.push_state();
            self.wait_for_sweep(&ct).await?;
            let party = accounts.iter().filter(|acc| (!is_running(self, acc.id) || switching.contains(&acc.id)) && !self.still_opening(acc.id)).count() as i32;
            if let (Some(pick), Some(t)) = (pick_server.as_ref(), target.clone()) {
                target = Some(pick(t, party, ct.clone()).await?);
            }
            // Several accounts sent to one server: the first shows whether it takes anyone before the rest follow; an
            // ended or full one is swapped for another with room for them all.
            let mut check_server = party > 1 && each_server.is_none();
            // Compared without regard to case.
            let mut closed_servers: Vec<String> = Vec::new();
            let closed_has = |list: &Vec<String>, job: &str| list.iter().any(|j| j.eq_ignore_ascii_case(job));

            // Arranged as they open: each window takes its tile as it joins its game, the tiles laid out for all the launch's
            // accounts from the first, so none moves when the next one comes (see tile_when_open).
            if self.preset_for(preset_id).and_then(|p| p.arrange).unwrap_or(s.multi_instance && s.multi_instance_tile) {
                if !self.preset_for(preset_id).is_some_and(|p| p.profile == AFK_PROFILE) {
                    *self.controller.tile_plan.borrow_mut() = Some((launch_start, accounts.iter().map(|acc| acc.id).collect()));
                }
                rt::spawn({
                    let app = self.clone();
                    async move { app.tile_when_open(preset_id, launch_start).await }
                });
            }
            let mut i: isize = 0;
            while (i as usize) < accounts.len() && same_vault(self, &vault) {
                ct.check()?;
                self.wait_for_pending(&ct).await?;
                // The account as it is now (C# kept the live object): a cookie renewed or a setting changed while the
                // earlier ones opened is the one used.
                let Some(account) = self.find_account(accounts[i as usize].id) else {
                    log::note(&format!("Launch: account {} skipped, deleted meanwhile.", short(accounts[i as usize].id)));
                    i += 1;
                    continue; // deleted while the earlier ones opened
                };
                if switching.contains(&account.id)
                    && let Some(old) = run_of(self, account.id)
                {
                    // Roblox would refuse the new window (signed out, no connection): the one in the old game stays.
                    if let Some(why) = self.sign_in_refused_now(account.id, &ct).await {
                        log::note_file(&format!("Launch: account {} not switched, was told: {}", short(account.id), self.scrub(&why)));
                        errors.push(format!("{}: {why}", self.name_text(&account)));
                        i += 1;
                        continue;
                    }
                    let n = self.name(&account);
                    self.progress(Some(&l::f("Closing {0}'s Roblox to switch games…", &[&n])));
                    // Not a disconnect: no auto-rejoin into the old game.
                    self.ask_close(Some(account.id), old.0, Cause::Switch, Then::StayClosed, false);
                    let mut waited = 0;
                    let mut wait_result = Ok(());
                    while is_running(self, account.id) && waited < 15_000 {
                        if let Err(e) = delay(&ct, Duration::from_millis(300)).await {
                            wait_result = Err(e);
                            break;
                        }
                        waited += 300;
                    }
                    // Still open: a real drop later must still rejoin.
                    if is_running(self, account.id) {
                        log::note(&format!(
                            "Launch: account {}'s window {} still open after the wait to switch games (15 s at most).",
                            short(account.id),
                            old.0
                        ));
                        self.close_desk.withdraw(old.0, Cause::Switch);
                    }
                    wait_result?;
                }
                if is_running(self, account.id) || self.still_opening(account.id) {
                    let stayed = is_running(self, account.id) && !switching.contains(&account.id);
                    log::note(&format!(
                        "Launch: account {} skipped, {}.",
                        short(account.id),
                        if stayed {
                            "already there"
                        } else if self.still_opening(account.id) {
                            "still opening"
                        } else {
                            "already running"
                        }
                    ));
                    let n = self.name(&account);
                    skipped.push(if stayed && target.is_none() {
                        l::f("{0}: already on the home screen — skipped.", &[&n])
                    } else if stayed {
                        l::f("{0}: already in this game — skipped.", &[&n])
                    } else {
                        l::f("{0}: already running — skipped.", &[&n])
                    });
                    i += 1;
                    continue;
                }

                // Closed by the person (or a schedule's stop) since this launch began, while it waited or opened the
                // ones before: it stays closed (close_plan.rs O2). Asked before the others are
                // marked as replaced by it.
                if let Open::Refuse(no) = self.may_open(Who::Person, Some(account.id), launch_start) {
                    log::note(&format!("Launch: account {} skipped, {}.", short(account.id), no.words()));
                    if let Some(text) = App::open_refused_text(no) {
                        errors.push(format!("{}: {text}", self.name_text(&account)));
                    }
                    i += 1;
                    continue;
                }
                let n = self.name(&account);
                let (at, of) = (i + 1, accounts.len());
                self.progress(Some(&l::f("Launching {0} ({1}/{2})…", &[&n, &at, &of])));
                let started_at = Stamp::utc_now();
                if !multi_instance::armed() {
                    let others: Vec<(Guid, i32)> = self.controller.running.borrow().iter().map(|(id, (pid, _))| (*id, *pid)).collect();
                    for (id, pid) in others {
                        if id != account.id && self.expect_close(Some(id), pid, Cause::Switch, Then::StayClosed) {
                            replaced.push(id);
                        }
                    }
                }
                let mut opened = false;
                let join: Res<Option<GameTarget>> = match (each_server.as_ref(), target.clone()) {
                    (Some(each), Some(t)) => each(t, ct.clone()).await.map(Some),
                    _ => Ok(target.clone()),
                };
                let attempt = match join {
                    Ok(join) => self.start_session(&account, join, &settings().launch_via, &ct, false, preset_id, false, 0).await,
                    Err(e) => Err(e),
                };
                match attempt {
                    Ok(()) => {
                        launched += 1;
                        opened = true;
                        // Closed while its sign-in ticket was fetched (nothing to close then): its window closes as soon as
                        // it shows.
                        if self.close_desk.held_closed.borrow().get(&account.id).is_some_and(|held| *held > launch_start) {
                            log::note(&format!("Launch: account {} closed while it opened; its window closes as it shows.", short(account.id)));
                            for l in pending(self).into_iter().filter(|p| p.account_id == account.id) {
                                self.close_when_it_shows(&l);
                            }
                        }
                    }
                    Err(ex) if !ct.is_cancelled() && (roblox_api::is_expected_error(&ex) || is_start_error(&ex) || matches!(ex, Fail::User(_))) => {
                        log::note(&format!("Launch: account {} didn't open ({}).", short(account.id), launch_fail_kind(&ex)));
                        // Signed out only when the sign-in check agrees: an outage answers 401 for everyone.
                        let refused = ex.cookie_invalid() && !self.confirm_signed_out(account.id, &ct, false).await;
                        ct.check()?; // the check's own call off ends the launch, as its exception did
                        let text = if refused {
                            super::controller::refused_text()
                        } else if is_start_error(&ex) {
                            super::controller::start_error_text(&ex)
                        } else {
                            Self::error_text(&ex)
                        };
                        // The words the person was shown, not just the kind (names scrubbed).
                        log::note_file(&format!("Launch: account {} was told: {}", short(account.id), self.scrub(&text)));
                        errors.push(format!("{}: {}", self.name_text(&account), text));
                        if sign_in_refused(&ex) && realm::normalize(Some(&account.realm)) != realm::VNG {
                            refused_sign_in.push(account.id);
                        }
                    }
                    Err(ex) => return Err(ex),
                }
                self.push_state();

                if (i as usize) < accounts.len() - 1 {
                    let next: Vec<Guid> = accounts[(i as usize + 1)..].iter().map(|acc| acc.id).collect();
                    self.wait_before_next_launch(&account, started_at, delay_seconds, next, &ct).await?;
                    let Some(t) = target.clone() else {
                        i += 1;
                        continue;
                    };
                    let job = match (&t.job_id, t.is_private()) {
                        (Some(job), false) => job.clone(),
                        _ => {
                            i += 1;
                            continue;
                        }
                    };
                    if !opened || !check_server {
                        i += 1;
                        continue;
                    }
                    if self.wait_for_server_answer(&account, &t, &ct).await? != Some("closed") {
                        check_server = false; // it took this account (or its log can't tell): the rest follow
                        i += 1;
                        continue;
                    }
                    if !closed_has(&closed_servers, &job) {
                        closed_servers.push(job.clone());
                    }
                    let other = if closed_servers.len() > MAX_SERVER_SWAPS {
                        GameTarget { job_id: None, ..t.clone() }
                    } else {
                        self.other_server(&t, party, &closed_servers, &ct).await?
                    };
                    // Got in after all (a slow join): it stays, the rest follow.
                    if self.server_answer(account.id) == Some("in") {
                        closed_servers.retain(|j| !j.eq_ignore_ascii_case(&job));
                        check_server = false;
                        i += 1;
                        continue;
                    }
                    let moving = other.job_id.is_some();
                    target = Some(other);
                    check_server = moving;
                    log::note(&format!(
                        "Launch: the party's server was full or closed ({} so far); {}.",
                        closed_servers.len(),
                        if moving { "moving to another with room" } else { "Roblox picks one for each" }
                    ));
                    self.toast(
                        &if moving {
                            l::t("That server is full or has closed, so the accounts go to another one with room for them all.")
                        } else {
                            l::t("That server is full or has closed, and no other server was found with room for all the accounts: Roblox picks one for each.")
                        },
                        "warn",
                    );
                    // Goes again, to the new server; not a drop for auto-rejoin.
                    if let Some(stuck) = run_of(self, account.id) {
                        self.ask_close(Some(account.id), stuck.0, Cause::Switch, Then::StayClosed, true);
                        let mut waited = 0;
                        while is_running(self, account.id) && waited < 15_000 {
                            delay(&ct, Duration::from_millis(300)).await?;
                            waited += 300;
                        }
                        if is_running(self, account.id) {
                            log::note(&format!("Launch: account {}'s window {} wouldn't close to move servers; it stays.", short(account.id), stuck.0));
                            self.close_desk.withdraw(stuck.0, Cause::Switch); // it wouldn't close: it stays, and auto-rejoin still looks after it
                            i += 1;
                            continue;
                        }
                    }
                    launched -= 1;
                    // i stays: the same account goes again.
                    continue;
                }
                i += 1;
            }
            Ok(None)
        }
        .await;

        let mut thrown: Option<Fail> = None;
        let mut early: Option<LaunchResult> = None;
        match outcome {
            Ok(Some(r)) => early = Some(r),
            Ok(None) => {}
            // OperationCanceledException when ct.IsCancellationRequested.
            Err(ex) if ct.is_cancelled() && is_fail_kind(&ex, roblox_api::TASK_CANCELED_EXCEPTION) => {}
            Err(ex) if started && roblox_api::is_expected_error(&ex) => {
                log::note(&format!("Launch: stopped by an error ({}).", launch_fail_kind(&ex)));
                log::note_file(&format!("Launch: was told: {}", self.scrub(&Self::error_text(&ex))));
                errors.push(Self::error_text(&ex));
            }
            Err(ex) => thrown = Some(ex),
        }
        // finally
        release(self);
        if started {
            let replaced_now = replaced.clone();
            rt::spawn({
                let app = self.clone();
                async move { app.forget_replaced(replaced_now).await }
            });
            if launched > 0
                && let Some(t) = target.as_ref()
                && same_vault(self, &vault)
            {
                // Not `?`: this is the finally part, progress and the "didn't open" notice below must still come.
                if let Err(ex) = self.remember_game(t.place_id) {
                    log::write(&ex);
                }
                rt::spawn({
                    let app = self.clone();
                    async move {
                        let _ = app.load_recent_games().await;
                    }
                });
            }
            if launched > 0 {
                self.refresh_presence_soon(Duration::from_secs(20));
            }
            self.persist();
        }
        self.progress(None);
        self.push_state();
        if let Some(ex) = thrown {
            return Err(ex);
        }
        if let Some(r) = early {
            return Ok(r);
        }
        if started {
            log::note(&format!(
                "Launch: done, {launched} opened, {} not opened, {} skipped{}.",
                errors.len(),
                skipped.len(),
                if ct.is_cancelled() { ", stopped" } else { "" }
            ));
        }
        if !errors.is_empty() && !ct.is_cancelled() {
            let count = errors.len();
            self.notify(&l::p(count as i64, "{0} account didn't open.", "{0} accounts didn't open.", &[&count]), "warn", "launch", Guid::nil(), false, None);
        }
        Ok(LaunchResult {
            launched,
            errors: Some(errors),
            cancelled: ct.is_cancelled(),
            skipped: Some(skipped),
            refused: if refused_sign_in.is_empty() { None } else { Some(refused_sign_in) },
            ..Default::default()
        })
    }

    /// "in" once the log names a server, "silent" after SERVER_CHECK without one, else None.
    pub fn server_answer(&self, account_id: Guid) -> Option<&'static str> {
        let run = run_of(self, account_id)?;
        let log = log_of(self, run.0)?;
        if log.job_id.is_some() {
            Some("in")
        } else if log.status == "joining" && log.at.age_secs() >= secs(SERVER_CHECK) {
            Some("silent")
        } else {
            None
        }
    }

    /// "in", or "closed" when the join is silent and Roblox's join answer says the server ended or is full; else None
    /// and the rest follow. Silence alone isn't "closed": a join waiting for room was closed and the party sent away.
    pub(crate) async fn wait_for_server_answer(self: &Rc<Self>, account: &Account, target: &GameTarget, ct: &Cancel) -> Res<Option<&'static str>> {
        let until = Stamp::utc_now().plus_secs(secs(SERVER_CHECK_LIMIT));
        let mut asked: Option<super::rejoin::JoinSlot> = None;
        while Stamp::utc_now() < until {
            let log = self.server_answer(account.id);
            if log == Some("in") {
                return Ok(log);
            }
            if log == Some("silent")
                && asked.is_none()
                && let Some(run) = run_of(self, account.id)
            {
                // Kept with the window: its own join check reads it rather than asking again.
                let slot = self.spawn_check_join(account.clone(), target.clone(), "A party's server, its first join silent", Cancel::new());
                self.rejoin.join_checks.borrow_mut().insert(run.0, super::rejoin::JoinCheck { answer: slot.clone(), at: Stamp::utc_now() });
                asked = Some(slot);
            }
            let closed = asked.as_ref().is_some_and(|slot| matches!(&*slot.borrow(), Some(Some(said)) if said.ended() || said.full()));
            if closed {
                return Ok(Some("closed"));
            }
            let n = self.name(account);
            self.progress(Some(&l::f("Checking that the server lets {0} in…", &[&n])));
            delay(ct, Duration::from_millis(500)).await?;
        }
        Ok(None)
    }

    /// With none, or no answer, Roblox picks one for each account.
    /// Err only when the launch was stopped meanwhile (C#'s cancel escaped too: the window that met the full server stays).
    async fn other_server(self: &Rc<Self>, target: &GameTarget, party: i32, closed: &[String], ct: &Cancel) -> Res<GameTarget> {
        self.progress(Some(&l::t("Finding a server…")));
        let (place, c) = (target.place_id, ct.clone());
        match rt::blocking(move || roblox_api::get_servers(place, true, None, &c)).await {
            Ok((servers, _)) => {
                if let Some(other) = roomy_server(&servers, party, closed) {
                    return Ok(GameTarget { job_id: Some(other.job_id.clone()), ..target.clone() });
                }
                log::note(&format!("Launch: no other server of place {} has room for {party} ({} read).", target.place_id, servers.len()));
            }
            Err(ex) if !ct.is_cancelled() && roblox_api::is_expected_error(&ex) => {
                log::note(&format!("Launch: the server list of place {} couldn't be read ({}).", target.place_id, launch_fail_kind(&ex)));
            }
            Err(ex) if ct.is_cancelled() => return Err(ex),
            // PORT: the C# let any other exception escape here; other_server's caller treats it as "no other server".
            Err(ex) => log::write(&ex),
        }
        ct.check()?;
        Ok(GameTarget { job_id: None, ..target.clone() })
    }

    /// Not one the person closed meanwhile.
    pub fn still_opening(&self, account_id: Guid) -> bool {
        let list = pending(self);
        list.iter().any(|p| p.account_id == account_id && !closing_has(self, p))
    }

    /// Without Multi-Roblox the new client closes the others once it is up, after the launch returned. They stay
    /// "closed on purpose" until then; one still open after wasn't replaced, so a real drop later must rejoin it.
    async fn forget_replaced(self: Rc<Self>, replaced: Vec<Guid>) {
        if replaced.is_empty() {
            return;
        }
        let deadline = Stamp::utc_now().plus_secs(secs(PENDING_TIMEOUT) + secs(REPLACED_GRACE));
        let mut settled: Option<Stamp> = None;
        while !self.controller.disposed.get() && replaced.iter().any(|id| is_running(&self, *id)) && Stamp::utc_now() < deadline {
            settled = if pending_count(&self) == 0 { Some(settled.unwrap_or_else(Stamp::utc_now)) } else { None };
            if let Some(since) = settled
                && since.age_secs() >= secs(REPLACED_GRACE)
            {
                break;
            }
            rt::sleep(Duration::from_millis(1000)).await;
        }
        for id in &replaced {
            if let Some(run) = run_of(&self, *id) {
                self.close_desk.withdraw(run.0, Cause::Switch);
            }
        }
    }

    /// With the first account whose sign-in Roblox accepts, so a dead first account doesn't stop the whole launch; nor
    /// one Roblox holds a notice on ("User is moderated": every call with its sign-in is refused, the link is fine).
    pub(crate) async fn resolve_share(
        self: &Rc<Self>,
        accounts: &[Account],
        share_code: &str,
        share_type: Option<&str>,
        ct: &Cancel,
    ) -> Res<roblox_api::play::ShareTarget> {
        // Roblox's share links are read on apis.roblox.com: a Roblox VNG sign-in has no business there (review, 2026-10-30).
        let global: Vec<&Account> = accounts.iter().filter(|acc| acc.realm == realm::GLOBAL).collect();
        if global.is_empty() {
            return Err(Fail::user(l::t("Share links are read with a Roblox account. Select one that isn't Roblox VNG.")));
        }
        let mut candidates: Vec<Account> = Vec::new();
        let fine = |acc: &&&Account| acc.cookie_valid != Some(false) && !self.moderated_lately(acc.id);
        for acc in global.iter().filter(fine).chain(global.iter().filter(|acc| acc.cookie_valid != Some(false))).chain(global.iter()) {
            if !candidates.iter().any(|c| c.id == acc.id) {
                candidates.push((*acc).clone());
            }
        }
        let mut i = 0;
        loop {
            let (cookie, code, kind, c) = (candidates[i].cookie.clone(), share_code.to_string(), share_type.map(str::to_string), ct.clone());
            match rt::blocking(move || roblox_api::play::resolve_share(&cookie, &code, kind.as_deref(), &c)).await {
                Ok(found) => return Ok(found),
                Err(ex) if ex.cookie_invalid() && i < candidates.len() - 1 => {
                    log::note(&format!("Launch: share link read refused for account {}'s sign-in; trying the next.", short(candidates[i].id)));
                    self.confirm_signed_out(candidates[i].id, ct, false).await;
                    ct.check()?;
                }
                Err(ex) if ex.moderated() => {
                    self.note_moderated(candidates[i].id);
                    if i == candidates.len() - 1 {
                        // No other account to read it with: said of the account, by name (the launch may hold several).
                        let n = self.name(&candidates[i]);
                        return Err(Fail::user(format!("{n}: {}", l::t("Roblox has a notice on this account to accept first. Open its roblox.com and accept it."))));
                    }
                    log::note(&format!("Launch: Roblox holds a notice on account {} (moderated), so it can't read the share link; trying the next.", short(candidates[i].id)));
                }
                Err(ex) => return Err(ex),
            }
            i += 1;
        }
    }

    /// Waits for the client's window, then the rest of the delay: Roblox's launcher downloads an update before the window
    /// appears, and more launchers meanwhile would all download at once and fight over the install.
    /// `next`: the accounts still to open: their own rejoins don't go during the wait.
    pub(crate) async fn wait_before_next_launch(self: &Rc<Self>, account: &Account, started_at: Stamp, delay_seconds: i32, next: Vec<Guid>, ct: &Cancel) -> Res<()> {
        let waiting_for_it = |app: &App| app.controller.pending.borrow().iter().any(|p| p.account_id == account.id);
        while waiting_for_it(self) && Stamp::utc_now().secs_since(started_at) < secs(PENDING_TIMEOUT) {
            if Stamp::utc_now().secs_since(started_at) > (delay_seconds.max(10)) as f64 {
                let version = self.roblox_version_dto();
                let n = self.name(account);
                self.progress(Some(&if version.up_to_date == Some(false) {
                    let latest = version.latest.clone().unwrap_or_default();
                    l::f("Waiting for Roblox to open {0}… (Roblox is updating to {1})", &[&n, &latest])
                } else {
                    l::f("Waiting for Roblox to open {0}… (Roblox may be updating)", &[&n])
                }));
            }
            delay(ct, Duration::from_millis(150)).await?;
        }
        if waiting_for_it(self) {
            log::note(&format!(
                "Launch: account {}'s window didn't come within {} s; the next goes on.",
                short(account.id),
                PENDING_TIMEOUT.as_secs()
            ));
        }
        // Rejoins may go during the wait, or one dropped early in a long launch would wait for all of it. A rejoin
        // started meanwhile counts as a start, so windows still open one every delay_seconds.
        if delay_seconds <= 0 {
            return Ok(());
        }
        let own_start = self.controller.last_session_start.get();
        let mut said: Option<String> = None;
        let mut waited_join = false;
        *self.controller.launch_waiting.borrow_mut() = Some(next.into_iter().collect());
        let result: Res<()> = async {
            loop {
                let last = self.controller.last_session_start.get();
                let from = if last != own_start { last } else { started_at };
                let rest = delay_seconds as f64 - Stamp::utc_now().secs_since(from);
                let opening = self.controller.rejoining.get() || pending_count(self) > 0;
                let joining = self.still_joining_since(account.id, started_at, delay_seconds);
                if rest <= 0.0 && !opening && !joining {
                    if waited_join {
                        let took = Stamp::utc_now().secs_since(started_at).round() as i64;
                        let joined = run_of(self, account.id).and_then(|r| log_of(self, r.0)).is_some_and(|l| l.status == "ingame");
                        log::note(&format!(
                            "Launch: account {} {} {took} s after its start; the next goes on.",
                            short(account.id),
                            if joined { "joined its game" } else { "not in its game" }
                        ));
                    }
                    return Ok(());
                }
                waited_join |= rest <= 0.0 && !opening && joining;
                let text = if rest > 0.0 {
                    let left = rest.ceil() as i64;
                    l::f("Waiting {0}s before the next account…", &[&left])
                } else if opening {
                    l::t("Waiting for the previous Roblox window to open…")
                } else {
                    l::t("Waiting for the previous account to join its game…")
                };
                if said.as_deref() != Some(text.as_str()) {
                    self.progress(Some(&text));
                    said = Some(text);
                }
                let step = if rest <= 0.0 {
                    Duration::from_millis(500)
                } else if rest < 1.0 {
                    Duration::from_secs_f64(rest)
                } else {
                    Duration::from_secs(1)
                };
                delay(ct, step).await?;
            }
        }
        .await;
        *self.controller.launch_waiting.borrow_mut() = None;
        result
    }

    pub fn current_place(&self, account_id: Guid) -> i64 {
        match run_of(self, account_id) {
            Some(run) => log_of(self, run.0).and_then(|l| l.place_id).or(run.1.target.as_ref().map(|t| t.place_id)).unwrap_or(0),
            None => 0,
        }
    }

    /// A client in a game answers the close button with "leave the game?" (seen 2026-10-13), so it is ended straight away.
    pub fn in_game_now(&self, pid: i32) -> bool {
        log_of(self, pid).is_some_and(|l| l.status == "ingame" || l.status == "joining")
    }

    /// Worth asking first. One on the home screen (asked by mistake on 2026-10-21) or stuck on a join (Join Error 600)
    /// gives way without a word.
    pub(crate) fn leaves_a_game(&self, run: &Run, target: &GameTarget) -> bool {
        self.needs_switch(run, target) && self.playing_here(run)
    }

    /// Without a log, by what it was opened with.
    fn in_game_here(&self, run: &Run) -> bool {
        match log_of(self, run.0) {
            Some(log) => log.status == "ingame" || log.status == "joining",
            None => run.1.target.is_some(),
        }
    }

    /// In a server or joining a given one; not a join still without one (a game's ban among them). Without a log, by
    /// what it was opened with.
    fn playing_here(&self, run: &Run) -> bool {
        match log_of(self, run.0) {
            Some(log) => log.status == "ingame" || (log.status == "joining" && log.job_id.is_some()),
            None => run.1.target.is_some(),
        }
    }

    /// Two places of the same game (a game moves players between its places) count as the same.
    pub fn same_game(&self, a: i64, b: i64) -> bool {
        if a == b {
            return true;
        }
        let games = self.controller.games.borrow();
        let ua = games.get(&a).and_then(|g| g.as_ref()).map(|g| g.universe_id);
        let ub = games.get(&b).and_then(|g| g.as_ref()).map(|g| g.universe_id);
        matches!(ua, Some(ua) if ua > 0 && Some(ua) == ub)
    }

    /// Another game, a chosen server it isn't in, a private server it wasn't opened into, or only in the menu or on an
    /// error screen.
    fn needs_switch(&self, run: &Run, target: &GameTarget) -> bool {
        let log = log_of(self, run.0);
        let current = log.as_ref().and_then(|l| l.place_id).or(run.1.target.as_ref().map(|t| t.place_id)).unwrap_or(0);
        if current == 0 || log.as_ref().is_some_and(|l| matches!(l.status.as_str(), "left" | "disconnected" | "signedout")) {
            return true;
        }
        if let Some(job) = &target.job_id {
            return !log.as_ref().and_then(|l| l.job_id.as_deref()).is_some_and(|j| j.eq_ignore_ascii_case(job));
        }
        if target.is_private() || target.share_code.is_some() {
            let opened = run.1.target.as_ref();
            return !target.is_private() || opened.and_then(|t| t.link_code.clone()) != target.link_code || opened.and_then(|t| t.access_code.clone()) != target.access_code;
        }
        !self.same_game(current, target.place_id)
    }

    /// A window naming a launch's tracker id is that launch's and never another's; only one naming none goes by the
    /// order of starts. By order alone, a late launch and the next account's swapped windows, and a website window
    /// could be taken for a launch's.
    /// `tracker_of`: Some("") none, None not read. `updating`: see roblox_processes::updater_running_since.
    /// Returns whether a launch got its window or stopped waiting.
    pub fn claim_windows(
        &self,
        pids: &HashSet<i32>,
        has_window: &dyn Fn(i32) -> bool,
        children_of: &dyn Fn(i32) -> Vec<i32>,
        tracker_of: &dyn Fn(i32) -> Option<String>,
        updating: &dyn Fn(&PendingLaunch) -> bool,
        now: Stamp,
    ) -> bool {
        let mut changed = false;
        let mut claimed = running_pids(self);
        let mut sorted_pids: Vec<i32> = pids.iter().copied().collect();
        sorted_pids.sort();
        if pending_count(self) > 0 {
            // Newest launch first: a new client can only belong to a launch made before it appeared (its Before
            // list lacks it), and a session waiting for a hand-over must not take the next account's window.
            let mut order = pending(self);
            order.sort_by(|a, b| b.started_at.get().cmp(&a.started_at.get()));
            for launch in order {
                // A window named for this launch first, then a child of a handed-over launch's ended process.
                let handed = get_by(&self.launching.handed_over.borrow(), &launch);
                let children: Vec<i32> = match &handed {
                    Some(h) => children_of(h.0),
                    None => Vec::new(),
                };
                let mut candidates: Vec<(bool, bool, i32)> = sorted_pids
                    .iter()
                    .copied()
                    .filter(|p| !launch.before.contains(p) && !claimed.contains(p))
                    .filter(|p| has_window(*p))
                    .filter_map(|p| {
                        let t = tracker_of(p);
                        may_be_launch_of(t.as_deref(), &launch).then(|| (is_launch_of(t.as_deref(), &launch), children.contains(&p), p))
                    })
                    .collect();
                candidates.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
                let pid = candidates.first().map(|c| c.2).unwrap_or(0);
                if pid == 0 {
                    continue;
                }
                claimed.insert(pid);
                remove_launch(&mut self.controller.pending.borrow_mut(), &launch);
                remove_by(&mut self.launching.handed_over.borrow_mut(), &launch);
                let close_it = closing_remove(&self, &launch);
                if close_it {
                    log::note(&format!("Roblox window {pid} came up and was closed, as asked while it opened."));
                    self.ask_close(Some(launch.account_id), pid, Cause::User, Then::StayClosed, false);
                    // It may have reached its server meanwhile, and Roblox's status then names it for minutes: an end
                    // here, so that reads as stale, not "playing on another device". (It never ran as the account's,
                    // so the ended clients' loop doesn't see it.)
                    let t = launch.target.as_ref();
                    self.note_ended_here(launch.account_id, t.and_then(|t| t.job_id.clone()), t.map(|t| t.place_id), now);
                } else {
                    self.controller.running.borrow_mut().insert(launch.account_id, (pid, launch.clone()));
                    self.persist_soon(); // the open clients on disk (see persist)
                    let main = roblox_processes::main_window(pid);
                    // A box in place of a game window (an error at start): what it says.
                    let words = window_report::small_window_words(main).map(|w| format!("; it shows {}", self.scrub(&w))).unwrap_or_default();
                    log::note(&format!("Roblox window {pid} came up: {}{words}.", window_report::describe(main)));
                    self.window_came_up();
                }
                changed = true;
            }
            // Every new client quit without a game window: stop waiting. Unless Roblox's updater started since: the
            // updated client comes from it a while later, and dropping the launch left that window nobody's.
            for launch in pending(self) {
                if let Some(new) = pids.iter().copied().find(|p| !launch.before.contains(p) && !claimed.contains(p)) {
                    set_seen_alive(&self, &launch, now);
                    self.controller.pending_seen_pid.borrow_mut().insert(LaunchRef(launch.clone()), new);
                } else if let Some(seen) = seen_alive(&self, &launch)
                    && now.secs_since(seen) > secs(super::controller::PENDING_GONE_GRACE)
                {
                    if updating(&launch) {
                        set_seen_alive(&self, &launch, now);
                        continue;
                    }
                    log::note(&format!("Launch: account {}'s Roblox quit before its window came.", short(launch.account_id)));
                    let (in_a_row, handoff) = self.note_quit(&launch);
                    // What it left behind, off this thread: Roblox's crash report, Windows' error record with the fault. With
                    // neither, it ended itself (handing its launch on, or giving up as it started).
                    if let Some(pid) = self.controller.pending_seen_pid.borrow_mut().remove(&LaunchRef(launch.clone())) {
                        let ran_for = Duration::from_secs_f64(now.secs_since(launch.started_at.get()).max(0.0));
                        crate::threads::run(move || {
                            let found = crate::core::roblox_crash_info::describe(pid, ran_for);
                            log::note(&format!(
                                "Launch: the Roblox that quit ({pid}, within {} s of the launch) left: {}.",
                                ran_for.as_secs(),
                                found.as_deref().unwrap_or("no crash report, no Windows error record")
                            ));
                        });
                    }
                    // A second quit in a row with other windows open, whatever its log says (most leave none): a Roblox
                    // with no window that is nobody's is ended too, as after a hand-over.
                    let cleared = (handoff || in_a_row >= 2) && self.end_handoff_targets(handoff) > 0;
                    // Opened by hand: said. "Opened 1 account." was the last word with no window to show for it. A rejoin has
                    // its own words (drop_pending), a handed-over one its session's end.
                    if !launch.rejoin
                        && !closing_has(self, &launch)
                        && get_by(&self.launching.handed_over.borrow(), &launch).is_none()
                        && let Some(account) = self.find_account(launch.account_id)
                    {
                        let n = self.name(&account);
                        // (Closing every window is the way out only of a hand-over to a Roblox that couldn't be ended.)
                        let said = if handoff && in_a_row >= 2 && !cleared {
                            l::f("{0}: Roblox closed again before its window opened. If it keeps happening, close all Roblox windows, then open them again.", &[&n])
                        } else {
                            l::f("{0}: Roblox closed before its window opened. Try again.", &[&n])
                        };
                        self.notify(&said, "warn", "launch", account.id, true, None);
                    }
                    self.drop_pending(&launch, false);
                    changed = true;
                }
            }
        }
        // Late launches, oldest first, take a window named for them, else only one no pending launch could be taking and that
        // isn't signed in as someone else: with none pending, any window opened by hand would do (Start menu, no tracker id).
        let mut late = self.launching.late.borrow().clone();
        late.sort_by_key(|l| l.started_at.get());
        for launch in late {
            if now.secs_since(launch.started_at.get()) > secs(PENDING_TIMEOUT) + secs(LATE_TIMEOUT) || is_running(self, launch.account_id) {
                remove_launch(&mut self.launching.late.borrow_mut(), &launch);
                closing_remove(&self, &launch);
                let waiting = self.controller.pending.borrow().iter().any(|p| p.account_id == launch.account_id);
                if !is_running(self, launch.account_id) && !waiting {
                    log::note(&format!("Launch: gave up on account {}'s late window, none came.", short(launch.account_id)));
                    self.forget_scheduled_open(launch.account_id); // no client came of it
                }
                continue;
            }
            let owner = self.find_account(launch.account_id);
            let pending_now = pending(self);
            let mut candidates: Vec<(bool, i32)> = sorted_pids
                .iter()
                .copied()
                .filter_map(|p| {
                    if launch.before.contains(&p) || claimed.contains(&p) || !has_window(p) {
                        return None;
                    }
                    let t = tracker_of(p);
                    if !may_be_launch_of(t.as_deref(), &launch) {
                        return None;
                    }
                    let own = is_launch_of(t.as_deref(), &launch);
                    let free = pending_now.iter().all(|l| l.before.contains(&p)) && owner.as_ref().is_none_or(|o| self.claim_user_matches(p, o));
                    (own || free).then_some((own, p))
                })
                .collect();
            candidates.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            let pid = candidates.first().map(|c| c.1).unwrap_or(0);
            if pid == 0 {
                continue;
            }
            claimed.insert(pid);
            remove_launch(&mut self.launching.late.borrow_mut(), &launch);
            let close_it = closing_remove(&self, &launch);
            log::note(&format!(
                "Roblox window {pid} came up late, {} s after its start{}.",
                now.secs_since(launch.started_at.get()) as i64,
                if close_it { "; closed, as asked meanwhile" } else { "" }
            ));
            if close_it {
                self.ask_close(Some(launch.account_id), pid, Cause::User, Then::StayClosed, false);
                let t = launch.target.as_ref();
                self.note_ended_here(launch.account_id, t.and_then(|t| t.job_id.clone()), t.map(|t| t.place_id), now); // as above
            } else {
                self.controller.running.borrow_mut().insert(launch.account_id, (pid, launch.clone()));
                self.persist_soon();
            }
            changed = true;
        }
        changed
    }

    /// Untagged windows are matched newest launch first (claim_windows), so a client still opening must have its window
    /// before the next starts, or the two could swap. A rejoin getting its ticket is waited for too.
    async fn wait_for_pending(self: &Rc<Self>, ct: &Cancel) -> Res<()> {
        let opening = |app: &App| app.controller.rejoining.get() || pending_count(app) > 0;
        if !opening(self) {
            return Ok(());
        }
        log::note(&format!(
            "Launch: waits for a window still opening ({}).",
            if self.controller.rejoining.get() { "a rejoin".to_string() } else { format!("{} pending", pending_count(self)) }
        ));
        self.progress(Some(&l::t("Waiting for the previous Roblox window to open…")));
        // Bounded: a launch stops pending after PENDING_TIMEOUT, LATE_TIMEOUT more while Roblox's updater has it (expire_pending).
        while opening(self) {
            delay(ct, Duration::from_millis(500)).await?;
        }
        Ok(())
    }

    /// Holds a launch back when it would close every Roblox window open: Roblox's own install with only another channel's
    /// build (newer than the live one Roblox serves, its live build not installed beside it, see roblox_installs::LIVE_BUILD)
    /// checks the live version as it opens, runs Roblox's installer, and that ends every client (a user's PC, 2026-11-13).
    /// So does one behind a build Roblox released while windows are open, unless that build isn't rolled out to this PC
    /// (live_not_here). With no Roblox open it goes on: the update closes nothing. RoALT holds the same launch ("update
    /// hold"). With the update set to wait and play on meanwhile (keeps_running_build) it isn't held: the client returned
    /// opens on the build it has, never updating (roblox_installs::kept_copy); None opens as chosen.
    fn hold_for_roblox_update(&self, account: &Account, launch_via: &str, custom_launcher: &str) -> Res<Option<String>> {
        let Some(live) = self.updates.latest_roblox.borrow().clone() else { return Ok(None) };
        if !roblox_api::starts_with_ci(&live.folder, "version-") {
            return Ok(None);
        }
        let Some(client) = roblox_installs::client_for(launch_via, custom_launcher) else { return Ok(None) };
        if roblox_installs::is_vng_client(&client) {
            return Ok(None);
        }
        let Some(have) = roblox_installs::version_of(Some(&client)) else { return Ok(None) };
        if have.folder.eq_ignore_ascii_case(&live.folder) {
            return Ok(None);
        }
        let open = roblox_processes::get_player_pids().len();
        if open == 0 {
            return Ok(None);
        }
        let keep = self.keeps_running_build();
        // A Bloxstrap-style launcher behind the live build: it updates first, and its update ends every Roblox open (RoALT
        // holds this one too). Its background update, when it would take this one, leaves them.
        if let Some(launcher) = roblox_installs::updater_for(launch_via, custom_launcher).filter(|l| !roblox_installs::is_own_installer(Some(l))) {
            if !roblox_installs::is_behind(Some(&have), Some(&live)) || roblox_installs::updates_in_background(&launcher, &have, &live) {
                return Ok(None);
            }
            let name = std::path::Path::new(&launcher).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            if keep {
                // Its copy started by itself, past the launcher: no updater beside it, so it plays on its build.
                log::note(&format!(
                    "Roblox start: account {} opens on {name}'s Roblox {} ({}) as it is, not the live {} ({}): {open} Roblox open, and the update waits until none is.",
                    short(account.id),
                    have.version,
                    have.folder,
                    live.version,
                    live.folder
                ));
                return Ok(Some(client));
            }
            log::note(&format!(
                "Roblox start: account {} held back, {name}'s Roblox is {} ({}), the live one {} ({}): {name} would update it first and close the {open} Roblox open.",
                short(account.id),
                have.version,
                have.folder,
                live.version,
                live.folder
            ));
            return Err(Fail::User(l::f(
                "{0} would update Roblox as this window opens, and that closes every Roblox window open. Close the other Roblox windows first, or use Close, update, reopen under Settings › Roblox version.",
                &[&name],
            )));
        }
        if !roblox_installs::is_roblox_own(&client) {
            return Ok(None);
        }
        let parse = |v: &str| -> Option<Vec<u64>> { v.split('.').map(|p| p.parse::<u64>().ok()).collect() };
        if matches!((parse(&have.version), parse(&live.version)), (Some(mine), Some(served)) if mine > served) {
            log::note(&format!(
                "Roblox start: account {} held back, Roblox's build {} ({}) is another channel's, the live {} ({}) isn't installed, and opening it would update Roblox and close the {open} Roblox open.",
                short(account.id),
                have.version,
                have.folder,
                live.version,
                live.folder
            ));
        } else if roblox_installs::is_behind(Some(&have), Some(&live)) && !self.live_not_here(&live) {
            // Roblox released a new build while windows are open: Roblox's own client behind the live build runs Roblox's installer
            // as it opens, and that installer ends every client, as the other channel's did. Not when Roblox hasn't rolled that
            // build out to this PC yet (live_not_here): the client opens on its own build then.
            log::note(&format!(
                "Roblox start: account {} held back, Roblox's build {} ({}) is behind the live {} ({}), and opening it would update Roblox and close the {open} Roblox open.",
                short(account.id),
                have.version,
                have.folder,
                live.version,
                live.folder
            ));
        } else {
            return Ok(None);
        }
        if keep {
            log::note(&format!(
                "Roblox start: account {} opens on that build instead, from a copy without Roblox's updater; the update waits until no Roblox is open.",
                short(account.id)
            ));
            return Ok(Some(client));
        }
        Err(Fail::User(l::t(UPDATE_HOLD_TEXT)))
    }

    /// `preset_id`: also on its rejoins and restarts: its options apply.
    /// `restart`: opened again by its preset's planned restart (a rejoin too).
    /// `open_retries`: see RejoinRequest::open_retries.
    #[allow(clippy::too_many_arguments)]
    pub async fn start_session(
        self: &Rc<Self>,
        account: &Account,
        target: Option<GameTarget>,
        launch_via: &str,
        ct: &Cancel,
        rejoin: bool,
        preset_id: Option<Guid>,
        restart: bool,
        open_retries: i32,
    ) -> Res<()> {
        // A launcher installing the new build: a window opened now would be closed by it, or start the old one.
        if self.controller.roblox_updating.get() {
            log::note(&format!("Roblox start: account {} waits for Roblox's update to finish.", short(account.id)));
        }
        while self.controller.roblox_updating.get() {
            self.progress(Some(&l::t("Waiting for Roblox to finish updating…")));
            delay(ct, Duration::from_millis(500)).await?;
        }
        // A window just asked to close may still start Roblox's tray copy, which the close's sweep then ends: a client
        // started meanwhile would hand its launch to that copy and quit (roblox_processes::soft_close_under_way).
        let closing_since = Stamp::utc_now();
        while roblox_processes::soft_close_under_way() && Stamp::utc_now().secs_since(closing_since) < 15.0 {
            delay(ct, Duration::from_millis(250)).await?;
        }
        // Roblox's tray copy would take this launch over and open no window (Bloxstrap: "Roblox failed to launch").
        self.close_tray_copies(ct).await?;
        let chosen_via = launch_via.to_string();
        let launch_via = roblox_installs::updating_option(launch_via, self.roblox_version_dto().up_to_date);
        if launch_via != chosen_via {
            log::note(&format!("Roblox start: through {launch_via} instead of {chosen_via}, so the old Roblox build is updated first."));
        }
        // The live build not known yet (Bloxkeep started in the background, a scheduled launch): asked now, since it picks the client.
        // With Roblox open, asked again unless just asked: one released since would close every window as this one updates.
        let known = self.updates.latest_roblox.borrow().is_some();
        if super::roblox_updates::ask_live_again(known, self.roblox_updates.latest_roblox_at.get(), Stamp::utc_now(), roblox_processes::get_player_pids().len()) {
            let _ = self.refresh_latest_roblox().await;
        }
        // The settings as they are after the waits above (the C# reads its live settings object at each use).
        let custom_launcher = self.vault(|d| d.settings.custom_launcher.clone())?;
        let kept_build = self.hold_for_roblox_update(account, &launch_via, &custom_launcher)?;
        // The account as it is now, not as it was before the waits above (the C# reads the live account): a sign-in renewed
        // meanwhile, or the main account / Apply to / FPS cap changed, goes with this window.
        let Some(now_account) = self.find_account(account.id) else { return Err(Fail::user(l::t("Account not found."))) };
        let account = &now_account;
        // The sign-in ticket last, after every wait above: one fetched first sat through a Roblox update (RoALT gets its
        // ticket right before starting the client, too).
        let ticket = {
            let (cookie, realm_of, c) = (account.cookie.clone(), account.realm.clone(), ct.clone());
            rt::blocking(move || roblox_api::get_auth_ticket(&cookie, &c, &realm_of)).await
        };
        // Roblox holds a notice on the account: the status check and share links are asked with another from now on.
        if ticket.as_ref().is_err_and(|ex| ex.moderated()) {
            self.note_moderated(account.id);
        }
        let ticket = ticket?;
        ct.check()?;
        let Some(now_account) = self.find_account(account.id) else { return Err(Fail::user(l::t("Account not found."))) };
        let account = &now_account;
        // Read after the ticket's wait, as the C# reads them: Fast Flags, Apply to, the window mode changed meanwhile count.
        let settings = self.vault(|d| d.settings.clone())?;
        let before: HashSet<i32> = roblox_processes::get_player_pids().into_iter().collect();
        let config = settings.roblox_config.clone();
        let windowed = settings.multi_instance && settings.multi_instance_windowed;
        // A light-AFK preset's values, whatever the shared setup says; the next client gets the person's own back. The
        // main account (the one played on) gets neither, nor the Fast Flags below: only an FPS limit set on it.
        let main = account.is_main;
        let afk = !main && self.preset_for(preset_id).is_some_and(|p| p.profile == AFK_PROFILE);
        let covered = !main && (config.mode == "all" || (config.mode == "marked" && account.use_roblox_config));
        let own = roblox_game_settings::for_launch(&config, afk, covered, account.fps_cap);
        if (own.is_some() || windowed || roblox_game_settings::has_baseline())
            && roblox_game_settings::prepare_for(own.as_ref().unwrap_or(&config), own.is_some(), windowed)
        {
            self.controller.roblox_settings_pending.set(true);
            self.controller.roblox_settings_written_at.set(Stamp::utc_now());
        }
        // The leanest drawing (Fast Flags) if the person has it on, for the accounts the Roblox settings go to (Apply to)
        // and a light-AFK preset's windows; off, "Nobody", or the main account: the person's own file goes back first.
        if settings.fast_flags && (covered || afk) {
            if roblox_flags::apply(roblox_installs::client_for(&launch_via, &settings.custom_launcher).as_deref()) {
                self.controller.roblox_flags_pending.set(true);
                self.controller.roblox_settings_written_at.set(Stamp::utc_now());
            }
        } else if self.controller.roblox_flags_pending.get() {
            roblox_flags::restore();
            self.controller.roblox_flags_pending.set(false);
        }
        // Mods (Roblox › Mods) laid into Roblox's own builds, and the ones turned off taken out (core/roblox_mods.rs).
        // (Roblox's own folder is never written: a window with mods opens from Bloxkeep's copy of the build.)
        let mods = super::super::roblox_mods::applies_to(&launch_via);
        let mut mod_client = None;
        if mods && kept_build.is_none() {
            let client = roblox_installs::client_for(&launch_via, &settings.custom_launcher);
            mod_client = rt::blocking(move || super::super::roblox_mods::mod_copy(client.as_deref())).await;
        }
        // The build the windows open run on, copied last so its Fast Flags are the ones just written. Not copied: it waits instead.
        let mut kept_client = None;
        if let Some(kept) = kept_build {
            let source = std::path::Path::new(&kept).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
            match rt::blocking(move || roblox_installs::kept_copy(&kept).map_err(|e| Fail::other(e.to_string()))).await {
                Ok(c) => {
                    // (The kept copy shares the build's files by hard links: matched to it, mods and all.)
                    if mods && let Some(dir) = std::path::Path::new(&c).parent().map(|p| p.to_string_lossy().into_owned()).filter(|d| *d != source) {
                        let src = source.clone();
                        let _ = rt::blocking(move || super::super::roblox_mods::apply_in(&dir, &src)).await;
                    }
                    kept_client = Some(c)
                }
                Err(ex) => {
                    log::note(&format!("Roblox start: account {} held back, its build couldn't be copied to open on: {ex}", short(account.id)));
                    return Err(Fail::User(l::t(UPDATE_HOLD_TEXT)));
                }
            }
        }
        // Stopped meanwhile (Stop, Close, a scheduled stop): nothing is opened.
        ct.check()?;
        let custom = self.vault(|d| d.settings.custom_launcher.clone()).unwrap_or_default();
        let tracker = roblox_launcher::launch(&ticket, target.as_ref(), &launch_via, &custom, kept_client.as_deref().or(mod_client.as_deref()))?;
        log::note(&format!(
            "Roblox start: account {} via {launch_via}, {}{}{}, {} Roblox already running.",
            short(account.id),
            match &target {
                None => "home screen".to_string(),
                Some(t) => format!("place {}", t.place_id),
            },
            if restart {
                ", a restart"
            } else if rejoin {
                ", a rejoin"
            } else {
                ""
            },
            if open_retries > 0 { format!(", try {}", open_retries + 1) } else { String::new() },
            before.len()
        ));
        self.controller.last_session_start.set(Stamp::utc_now());
        // (Its window is looked for at once, not after the idle check's seconds.)
        self.controller.proc_interval_ms.set(500);
        self.close_desk.forget_closes(account.id); // opened again: an earlier close of it is no longer under way (a server switch lasts until it is in its game)
        self.forget_late(account.id); // opened again: a window of the earlier try is not this one's
        let place = target.as_ref().map(|t| t.place_id);
        let home_screen = target.is_none();
        let launch = Rc::new(PendingLaunch {
            account_id: account.id,
            before,
            started_at: std::cell::Cell::new(Stamp::utc_now()),
            home_screen,
            target,
            rejoin,
            preset_id,
            tracker: Some(tracker),
            open_retries,
            fps_limit: own.as_ref().map(|o| o.fps).unwrap_or(0),
        });
        self.controller.pending.borrow_mut().push(launch);
        self.note_activity(
            if restart {
                "reopened"
            } else if rejoin {
                "rejoined"
            } else {
                "opened"
            },
            account.id,
            place,
            None,
            None,
            None,
            None,
        ); // Statistics › Activity
        let _ = self.update_account(account.id, |a| {
            a.last_used = Some(Stamp::now());
            App::sign_in_accepted(a);
        });
        Ok(())
    }

    /// Ends Roblox's notification-area copies (roblox_processes::is_tray_copy); true when there were any. One holds
    /// the name Multi-Roblox takes (ROBLOX_singletonEvent), so Multi-Roblox couldn't be ready while it ran and nothing
    /// took the name once it was ended: the launch opened its window unarmed and the next account's closed it (review,
    /// 2026-10-30). With no Roblox left, the name is taken now, before the new client can.
    pub async fn close_tray_copies(self: &Rc<Self>, ct: &Cancel) -> Res<bool> {
        let tray_copies = roblox_processes::close_tray_copies();
        if tray_copies == 0 {
            return Ok(false);
        }
        log::note(&format!("Roblox start: closed {tray_copies} Roblox notification-area copy(ies) first."));
        delay(ct, Duration::from_millis(500)).await?;
        self.arm_multi_if_free(&roblox_processes::get_player_pids());
        Ok(true)
    }

    /// A new client crashed at once in Roblox's SingleInstanceGuard ("Failed to send message …, error: 1400"): it found a
    /// Roblox already running and handed its launch to it ("NoReload Detector: WebPlay NoReload with Tray only!", a real
    /// Roblox log), whose window wasn't there to take it. Every start after it crashed the same way until that Roblox was
    /// gone: a person's error.log, 2026-11-16 to 11-21, nine crashes in four runs, each with "1 Roblox already running"
    /// and each over once Close all ended it. So the Roblox with no window that no account owns and no launch under way
    /// may be starting is ended now, and the next start opens. Each is said, with what it was, so the next report shows
    /// it. The number ended. `handoff`: the quit client's log shows the hand-over; false: clients quit before their
    /// windows twice in a row with other windows open and left no word why (most leave no log at all), and such a Roblox
    /// is ended all the same.
    pub fn end_handoff_targets(&self, handoff: bool) -> usize {
        let why = if handoff { "a new Roblox crashed handing its launch to it" } else { "new Roblox clients keep quitting before their windows" };
        let players = roblox_processes::get_players();
        let windows = roblox_processes::visible_windows(&players.iter().map(|p| p.pid).collect::<Vec<_>>());
        let owned: HashSet<i32> = self.controller.running.borrow().values().map(|(pid, _)| *pid).collect();
        // (A launch under way may own any Roblox started since it began, its window still to come.)
        let opening_since = pending(self).iter().map(|l| l.started_at.get()).min();
        let mut ended = 0;
        for p in &players {
            let has_window = windows.get(&p.pid).is_some_and(|w| !w.is_empty());
            let started = if p.created > 0 { Some(win_kit::filetime_to_stamp(p.created)) } else { None };
            let maybe_opening = match (opening_since, started) {
                (Some(since), Some(at)) => at >= since,
                (Some(_), None) => true,
                (None, _) => false,
            };
            if has_window || owned.contains(&p.pid) || maybe_opening {
                continue;
            }
            let tray = roblox_processes::is_tray_copy(p.pid, p.created);
            log::note(&format!(
                "Launch: Roblox {} (no window, {}, started {}) ended: {why}.",
                p.pid,
                if tray { "its notification-area copy" } else { "no account's" },
                started.map_or("when not known".to_string(), |at| format!("{:.0} s ago", Stamp::utc_now().secs_since(at)))
            ));
            roblox_processes::kill(p.pid);
            ended += 1;
        }
        if ended == 0 {
            log::note(&format!(
                "Launch: {why}; of the {} Roblox running, none without a window to end (each has an account's window, or may be a launch's).",
                players.len()
            ));
        }
        ended
    }

    /// Multi-Roblox takes Roblox's name when it is on and not holding it yet, and no Roblox runs to hold it instead.
    pub fn arm_multi_if_free(&self, players: &[i32]) {
        let on = vault_now(self).is_some() && self.vault(|d| d.settings.multi_instance).unwrap_or(false);
        if !on || (multi_instance::armed() && !multi_instance::stale()) || !players.is_empty() {
            return;
        }
        self.apply_watch_names();
        multi_instance::enable();
    }

    /// Also from the launches to close once claimed: Close all put the late ones there, and one left behind stayed for the
    /// rest of the run (review, 2026-10-30).
    pub fn forget_late(&self, account_id: Guid) {
        let earlier: Vec<Rc<PendingLaunch>> = self.launching.late.borrow().iter().filter(|l| l.account_id == account_id).cloned().collect();
        for e in earlier {
            remove_launch(&mut self.launching.late.borrow_mut(), &e);
            closing_remove(&self, &e);
        }
    }

    pub fn cancel_launch(&self) -> Value {
        let cts = self.controller.launch_cts.borrow().clone();
        if let Some(cts) = cts {
            log::note("Launch: Stop pressed.");
            cts.cancel();
        }
        Value::Null
    }

    /// `force`: end the process at once (a frozen client) instead of asking its windows to close first.
    pub fn close_accounts(&self, ids: &[Guid], force: bool) -> Value {
        self.close_accounts_for(ids, force, Cause::User)
    }

    /// Close (the person's), or a schedule's stop (`Cause::Stop`): closed for good, no auto-rejoin.
    pub fn close_accounts_for(&self, ids: &[Guid], force: bool, cause: Cause) -> Value {
        let mut closed = 0;
        let mut already = 0;
        for &id in ids {
            // A launch still getting this account's ticket (not opening yet, nothing to close) leaves it closed
            // (close_plan.rs O2).
            if matches!(cause, Cause::User | Cause::Stop) {
                self.close_desk.hold_closed(id);
            }
            self.controller.rejoin_queue.borrow_mut().retain(|r| r.account_id != id);
            self.controller.rejoins_in_a_row.borrow_mut().remove(&id); // closed by the person: a client opened later counts its drops from one
            // A rejoin under way (looking for its player, getting its ticket, waiting for Roblox's update): called off.
            if self.cancel_rejoin_under_way(id) {
                closed += 1;
                self.push_state();
            }
            // Still opening, or late (its window may still come): its client is closed as soon as it shows up, rather than
            // staying open untracked or, late, becoming the account's again.
            let mut theirs: Vec<Rc<PendingLaunch>> = pending(self).into_iter().filter(|p| p.account_id == id).collect();
            theirs.extend(self.launching.late.borrow().iter().filter(|p| p.account_id == id).cloned());
            for launch in theirs {
                let added = self.close_when_it_shows(&launch);
                if added {
                    closed += 1;
                    self.push_state();
                }
            }
            let Some(info) = run_of(self, id) else {
                continue;
            };
            // Counted as closed only when asked now; one already closing (a second Close) is said apart.
            match self.ask_close(Some(id), info.0, cause, Then::StayClosed, force) {
                super::close_desk::Asked::Closing => closed += 1,
                super::close_desk::Asked::Refused(crate::core::close_plan::No::AlreadyClosing(_)) => already += 1,
                _ => {}
            }
        }
        log::note(&format!(
            "Close: {closed} window(s) or opening(s) closed for {} account(s){}{}.",
            ids.len(),
            if already > 0 { format!(", {already} already closing") } else { String::new() },
            if force { ", forced" } else { "" }
        ));
        // The person's count: a window already on its way out is still one closing.
        json!(closed + already)
    }

    pub fn close_all(&self, force: bool) -> Value {
        // Also what is still opening: the launch in progress and a rejoin getting its ticket.
        let (launch, rejoin) = (self.controller.launch_cts.borrow().clone(), self.controller.rejoin_cts.borrow().clone());
        if let Some(c) = launch {
            c.cancel();
        }
        if let Some(c) = rejoin {
            c.cancel();
        }
        self.controller.rejoin_queue.borrow_mut().clear();
        self.controller.rejoins_in_a_row.borrow_mut().clear();
        self.launching.handed_over.borrow_mut().clear();
        // Clients still opening would show up later, untracked: closed as soon as they appear instead.
        if pending_count(self) > 0 {
            for l in pending(self) {
                self.close_when_it_shows(&l);
            }
            self.push_state();
        }
        let late = self.launching.late.borrow().clone();
        for l in &late {
            self.close_when_it_shows(l);
        }
        // Roblox starts a windowless background client when its last window closes; sweep those for a moment
        // so "close all" really leaves nothing running and multi-instance can re-arm.
        self.controller.sweep_background_until.set(Stamp::utc_now().plus_secs(secs(BACKGROUND_SWEEP)));
        log::note(&format!(
            "Close all: {} tracked window(s), {} still opening, {} late{}.",
            self.controller.running.borrow().len(),
            pending_count(self),
            late.len(),
            if force { ", forced" } else { "" }
        ));
        json!(self.close_everything(force))
    }

    /// The sweep would kill a new client.
    pub async fn wait_for_sweep(self: &Rc<Self>, ct: &Cancel) -> Res<()> {
        let remaining = self.controller.sweep_background_until.get().secs_since(Stamp::utc_now());
        if remaining <= 0.0 {
            return Ok(());
        }
        log::note(&format!("Launch: waits {} s for Roblox to finish closing.", fmt_0_1(remaining)));
        self.progress(Some(&l::t("Waiting for Roblox to finish closing…")));
        delay(ct, Duration::from_secs_f64(remaining)).await
    }

    /// Tiles every Roblox window on one monitor: tracked accounts first (by name), then any other clients. `auto`:
    /// tiling after a launch: minimised windows stay minimised. A light-AFK preset's, or one the person put away,
    /// gets no tile; one that "When a window joins a game" minimises gets its tile as the place it opens at
    /// (`resting_clients`). `at_mouse`: the hotkey: on the screen the mouse is on, wherever Bloxkeep is. Who takes which
    /// tile is decided here; the window desk carries it out (window_desk.rs `desk_arrange`).
    pub async fn arrange_windows(self: &Rc<Self>, monitor: i32, auto: bool, at_mouse: bool) -> Res<Value> {
        let screens = win_kit::all_screens();
        let owner = self.controller.owner.get();
        let owner_in_view = owner != 0 && window_shown(owner);
        // No monitor picked (after a launch): the one Bloxkeep is on, else, while it's in the tray, the one the mouse is on.
        let screen = if monitor >= 0 && (monitor as usize) < screens.len() {
            screens[monitor as usize].clone()
        } else if at_mouse || !owner_in_view {
            let (x, y) = cursor_position();
            win_kit::screen_from_rect(Rect::new(x, y, 1, 1))
        } else {
            win_kit::screen_from_handle(owner)
        };
        // Clicked: the windows come over every other app, the last one taking the keyboard (after a launch they come over
        // too, but the keyboard stays: the person may be typing elsewhere).
        let app_window = if !auto && owner_in_view { owner } else { 0 };
        let lift = if auto {
            Lift::Quietly
        } else if app_window != 0 || at_mouse {
            Lift::Keyboard
        } else {
            Lift::No
        };
        let area = screen.working_area;
        let laid = self.desk_arrange(|app| app.arrangement(area, auto, lift)).await?;
        // What it came to, as the person sees it: each window in its place, or what's wrong with it (window_report).
        let how = if auto {
            "after opening"
        } else if at_mouse {
            "hotkey"
        } else {
            "button"
        };
        let index = screens.iter().position(|x| *x == screen).map(|i| i as i64).unwrap_or(-1) + 1;
        log::note(&format!(
            "Arrange ({how}, screen {index} of {}, {}{}): {} of {} window(s), before {}; {}{}.",
            screens.len(),
            laid.layout,
            if app_window != 0 { ", Bloxkeep in view" } else { "" },
            laid.done,
            laid.windows,
            if laid.before.is_empty() { "none" } else { &laid.before },
            laid.note,
            laid.placed
        ));
        Ok(json!(laid.done))
    }

    /// Who takes which tile on `area`, read once the input turn is the Arrange's: the accounts' windows by name, then any
    /// other clients; a launch tiling as its windows join keeps tiles empty for its accounts still to come. A main
    /// account's window stays where the person put it; after a launch a light-AFK preset's gets no tile either.
    fn arrangement(&self, area: Rect, auto: bool, lift: Lift) -> Arrangement {
        let mut tracked_runs: Vec<(String, i32)> = running(self)
            .into_iter()
            .map(|(id, run)| (self.find_account(id).map(|a| a.label()).unwrap_or_default(), if auto && self.tile_later(id, &run) { 0 } else { run.0 }))
            .collect();
        // A launch tiling as its windows join (tile_when_open): its accounts still to come keep their tiles empty (pid 0),
        // so the grid is the whole launch's from the first and a window placed stays in its tile.
        if auto && let Some((_, plan)) = self.controller.tile_plan.borrow().as_ref() {
            for id in plan.iter().filter(|id| !is_running(self, **id)) {
                if let Some(a) = self.find_account(*id).filter(|a| !a.is_main) {
                    tracked_runs.push((a.label(), 0));
                }
            }
        }
        tracked_runs.sort_by(|a, b| win_kit::culture_cmp(&a.0, &b.0));
        let tracked: Vec<i32> = tracked_runs.into_iter().map(|t| t.1).collect();
        let mut others: Vec<i32> = roblox_processes::get_player_pids().into_iter().filter(|p| !tracked.contains(p)).collect();
        others.sort();
        let mut afk = if auto { self.afk_clients() } else { HashSet::new() };
        afk.extend(self.main_pids()); // the window played on stays where the person put it
        // The layout as it is once the turn came.
        let s = self.vault(|d| d.settings.clone()).ok();
        let layout = s.as_ref().map(|s| s.window_layout.clone()).unwrap_or_else(|| "fill".into());
        let custom = s.as_ref().map(|s| Size::new(s.window_width, s.window_height)).unwrap_or_default();
        let order: Vec<(i32, isize)> =
            tracked.iter().chain(others.iter()).copied().filter(|p| !afk.contains(p)).map(|p| (p, if p == 0 { 0 } else { roblox_processes::main_window(p) })).collect();
        let resting = if auto { self.resting_clients() } else { Vec::new() };
        let (slots, open) = choose_tiles(&order, auto, &resting);
        Arrangement { windows: order.into_iter().filter(|w| w.0 != 0).collect(), slots, area, layout, custom, open, clicked: !auto, lift }
    }

    /// The clients whose windows "When a window joins a game" minimises (Settings' choice, or their preset's), other
    /// than a light-AFK preset's and the main accounts': minimised or about to be, each still has its place among the
    /// tiles. Only the accounts' own clients: a window opened outside Bloxkeep is never minimised by it, so one put
    /// away there is the person's doing and gets no tile, as before.
    pub fn resting_clients(&self) -> Vec<i32> {
        let mut skip = self.afk_clients();
        skip.extend(self.main_pids());
        running(self).into_iter().map(|(_, r)| (r.0, r.1.preset_id)).filter(|(pid, preset)| !skip.contains(pid) && self.on_join_of(*preset) == "minimise").map(|x| x.0).collect()
    }

    pub fn afk_clients(&self) -> HashSet<i32> {
        running(self)
            .into_iter()
            .filter(|(_, r)| self.preset_for(r.1.preset_id).is_some_and(|p| p.profile == AFK_PROFILE))
            .map(|(_, r)| r.0)
            .collect()
    }

    /// Multi-Roblox's "tile" option or the preset's Arrange windows, started with the launch: each window takes its tile
    /// a moment after it joins its game (the tiles laid out for every account of the launch, `tile_plan`, so a window
    /// once placed stays), then all of them once the last window is up. Tiled only once all had opened, the first
    /// windows stood at Roblox's size for a minute or more and then jumped to another screen.
    /// `preset_id`: its choice decides (asked again at the end, in case it changed).
    async fn tile_when_open(self: Rc<Self>, preset_id: Option<Guid>, launch_start: Stamp) {
        let since = launch_start;
        let mine = |app: &App| app.controller.tile_plan.borrow().as_ref().is_some_and(|(at, _)| *at == launch_start);
        let _plan_ends = crate::core::base::on_drop({
            let app = self.clone();
            move || {
                if mine(&app) {
                    *app.controller.tile_plan.borrow_mut() = None;
                }
            }
        });
        let mut tiled: HashSet<i32> = HashSet::new();
        let mut deadline: Option<Stamp> = None;
        while !self.controller.disposed.get() && (pending_count(&self) > 0 || self.controller.launch_cts.borrow().is_some()) {
            // (The launch's own wait is as long as it takes; once it is done, its last windows get PENDING_TIMEOUT.)
            if self.controller.launch_cts.borrow().is_none() {
                let at = *deadline.get_or_insert_with(|| Stamp::utc_now().plus_secs(secs(PENDING_TIMEOUT)));
                if Stamp::utc_now() >= at {
                    break;
                }
            }
            // Another launch took over (it tiles its own and these with them).
            if !mine(&self) && self.controller.tile_plan.borrow().is_some() {
                return;
            }
            if mine(&self) && vault_now(&self).is_some() && self.tile_joined(since, &mut tiled).await {
                continue;
            }
            rt::sleep(Duration::from_millis(500)).await;
        }
        // Roblox sizes its window again as it joins its game (maximised again, if it was last closed so): tiled 3 s after
        // the last window came up, 6 windows still loading on a busy PC came back full screen one by one. So each window
        // opened into a game is waited for until it is in it (a minute at most).
        let join_by = Stamp::utc_now().plus_secs(secs(JOIN_WAIT_FOR_TILES));
        while !self.controller.disposed.get() && self.controller.launch_cts.borrow().is_none() && Stamp::utc_now() < join_by && self.still_joining(since) {
            rt::sleep(Duration::from_millis(500)).await;
        }
        rt::sleep(Duration::from_millis(3000)).await;
        let launching_again = self.controller.launch_cts.borrow().is_some();
        let arrange = vault_now(&self).is_some()
            && self
                .preset_for(preset_id)
                .and_then(|p| p.arrange)
                .unwrap_or_else(|| self.vault(|d| d.settings.multi_instance && d.settings.multi_instance_tile).unwrap_or(false));
        if self.controller.disposed.get() || vault_now(&self).is_none() || launching_again || !arrange {
            if !self.controller.disposed.get() && vault_now(&self).is_some() {
                log::note(if launching_again {
                    "Arrange after opening: left to the next launch."
                } else {
                    "Arrange after opening: off (Settings' \"When a Roblox window opens\" or the preset)."
                });
            }
            return; // locked, switched off (in Settings or the preset), or another launch started (it tiles when it is done)
        }
        // Two or more tiles, so one window isn't stretched over the whole screen as a grid of one. A window minimised
        // as it joined (`resting_clients`) counts: it takes its tile minimised.
        let mut afk = self.afk_clients();
        afk.extend(self.main_pids());
        let resting = self.resting_clients();
        let tiles = roblox_processes::visible_windows(&roblox_processes::get_player_pids())
            .iter()
            .filter(|(pid, w)| !afk.contains(pid) && !w.is_empty() && (resting.contains(pid) || !roblox_processes::is_minimised(roblox_processes::main_window_of(w))))
            .count();
        let states = window_report::states(
            &roblox_processes::get_player_pids().into_iter().filter(|p| !afk.contains(p)).map(roblox_processes::main_window).collect::<Vec<_>>(),
        );
        log::note(&format!(
            "Arrange after opening: {tiles} window(s) to tile{} ({states}).",
            if self.still_joining(since) { ", some still joining" } else { "" }
        ));
        if tiles >= 2 {
            let _ = self.arrange_windows(-1, true, false).await;
        }
    }

    /// The account's window opened by this launch (started at `started_at` or later) into a game, not in it yet (its log
    /// says nothing, or joining), and up less than `JOIN_WAIT_MAX` (or the wait between accounts, if longer). Many
    /// windows joining at once made each slow to join and some failed (Error 256); the next account waits for this one to
    /// join, as MultiAlt does. A window that ended, failed or opened to Roblox's home screen isn't waited for.
    fn still_joining_since(&self, account_id: Guid, started_at: Stamp, delay_seconds: i32) -> bool {
        let Some((pid, launch)) = run_of(self, account_id) else { return false };
        let cap = secs(JOIN_WAIT_MAX).max(delay_seconds as f64);
        launch.started_at.get() >= started_at
            && launch.target.is_some()
            && Stamp::utc_now().secs_since(started_at) < cap
            && log_of(self, pid).is_none_or(|l| l.status == "joining")
    }

    /// A window of the launch tiling as its windows join that isn't in its game yet (a minute at most): its tile is kept
    /// empty for now, as Roblox sizes its window again as it joins and would undo it.
    fn tile_later(&self, id: Guid, run: &Run) -> bool {
        let now = Stamp::utc_now();
        self.controller.tile_plan.borrow().as_ref().is_some_and(|(_, plan)| plan.contains(&id))
            && now.secs_since(run.1.started_at.get()) < secs(JOIN_WAIT_FOR_TILES)
            && !log_of(self, run.0).is_some_and(|l| l.status == "ingame" && now.secs_since(l.at) >= secs(TILE_SETTLE))
    }

    /// Tiles the windows of the launch started at `since` that joined their game `TILE_SETTLE` ago or more and weren't
    /// tiled yet (into `tiled`); every window takes its tile from the plan, so those already in theirs stay. True when
    /// it arranged.
    async fn tile_joined(self: &Rc<Self>, since: Stamp, tiled: &mut HashSet<i32>) -> bool {
        let mut skip = self.afk_clients();
        skip.extend(self.main_pids());
        let now = Stamp::utc_now();
        let joined: Vec<i32> = running(self)
            .into_iter()
            .filter(|(_, r)| r.1.started_at.get() >= since && r.1.target.is_some() && !skip.contains(&r.0) && !tiled.contains(&r.0))
            .filter(|(_, r)| log_of(self, r.0).is_some_and(|l| l.status == "ingame" && now.secs_since(l.at) >= secs(TILE_SETTLE)))
            .map(|(_, r)| r.0)
            .collect();
        if joined.is_empty() {
            return false;
        }
        tiled.extend(joined.iter().copied());
        log::note(&format!("Arrange as they join: window(s) {} in their game, each to its tile.", joined.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", ")));
        let _ = self.arrange_windows(-1, true, false).await;
        true
    }

    /// A window opened since `since` into a game, not in it yet (its log says nothing, or joining). Light-AFK and main
    /// accounts' windows aren't tiled, so aren't waited for.
    fn still_joining(&self, since: Stamp) -> bool {
        let mut skip = self.afk_clients();
        skip.extend(self.main_pids());
        running(self).into_iter().any(|(_, r)| {
            r.1.started_at.get() >= since && r.1.target.is_some() && !skip.contains(&r.0) && log_of(self, r.0).is_none_or(|l| l.status == "joining")
        })
    }

    /// Stops waiting for a launch; one already running ends its session (auto-rejoin, toast). A rejoin that never got a
    /// window is said, since its account would otherwise be left with no word and no client.
    /// `late`: its window may still come (see `late`): a scheduled stop keeps the account until then.
    pub fn drop_pending(&self, launch: &Rc<PendingLaunch>, late: bool) {
        remove_launch(&mut self.controller.pending.borrow_mut(), launch);
        let closed = closing_remove(&self, launch);
        log::note(&format!(
            "Launch: stopped waiting for account {}'s window{}{}{}.",
            short(launch.account_id),
            if launch.rejoin { " (a rejoin)" } else { "" },
            if late { ", still taken if it comes late" } else { "" },
            if closed { ", closed meanwhile" } else { "" }
        ));
        let handed = remove_by(&mut self.launching.handed_over.borrow_mut(), launch);
        if let Some(handed) = handed
            && !closed
        {
            // Its client handed over and ended long since: whatever closed it then was taken in when it ended.
            self.on_session_ended(launch.account_id, launch, handed.1, End::Unasked);
            return;
        }
        if launch.rejoin
            && !closed
            && let Some(account) = self.find_account(launch.account_id)
        {
            // Its client quit before a window came (crashed as it started): tried again, as a drop in game is, for as
            // long as auto-rejoin is on. Not a late one: its window may still come.
            if !late && self.retry_open(&account, launch) {
                return;
            }
            log::note(&format!("Rejoin: account {} gave up, no Roblox window appeared (try {}).", short(account.id), launch.open_retries + 1));
            let n = self.name(&account);
            self.notify(&l::f("{0} didn't rejoin: no Roblox window appeared.", &[&n]), "warn", "rejoinFail", account.id, true, None);
        }
        if !late && !is_running(self, launch.account_id) {
            self.forget_scheduled_open(launch.account_id); // no client came of it: a scheduled stop has nothing of this account's to close
        }
    }

    /// A rejoin that opened no window goes again later (see RejoinRequest::open_retries) while auto-rejoin is on and
    /// nothing else of the account's is open or waiting: soon at first, then further apart (`open_retry_delay`), with
    /// no last try. Giving up after four left the account closed for good, though a start by hand right after such a
    /// quit opened. Said once, after MAX_OPEN_RETRIES more tries.
    fn retry_open(&self, account: &Account, launch: &PendingLaunch) -> bool {
        let Some(target) = launch.target.clone() else {
            return false;
        };
        if !self.rejoin_on(account, launch.preset_id)
            || account.banned == Some(true)
            || (target.follow.is_none() && self.game_ban_of(account, Some(target.place_id)).is_some())
            || is_running(self, account.id)
            || self.controller.pending.borrow().iter().any(|p| p.account_id == account.id)
            || self.controller.rejoin_queue.borrow().iter().any(|r| r.account_id == account.id)
        {
            return false;
        }
        let delay = open_retry_delay(launch.open_retries);
        if launch.open_retries == MAX_OPEN_RETRIES {
            let n = self.name(account);
            self.notify(
                &l::f("{0} hasn't reopened yet: Roblox keeps closing as it starts. Bloxkeep keeps trying.", &[&n]),
                "warn",
                "rejoinFail",
                account.id,
                true,
                None,
            );
        }
        self.controller.rejoin_queue.borrow_mut().push(RejoinRequest {
            account_id: account.id,
            target: GameTarget { job_id: None, ..target },
            due: Stamp::utc_now().plus_secs(secs(delay)),
            network_retries: 0,
            preset_id: launch.preset_id,
            restart: false,
            open_retries: launch.open_retries + 1,
        });
        log::note(&format!(
            "Rejoin: account {} opened no window; try {} in {} s.",
            short(account.id),
            launch.open_retries + 2,
            delay.as_secs()
        ));
        true
    }
}

/// Before the next try of a rejoin whose client quit before its window: 10 s, 30 s, 1 min, 2 min, then MAX_RETRY_DELAY.
/// The quit itself is known some 20 s after the start, and the try right after one often opens.
pub(super) fn open_retry_delay(tries: i32) -> Duration {
    match tries.max(0) {
        0 => Duration::from_secs(10),
        1 => Duration::from_secs(30),
        2 => Duration::from_secs(60),
        3 => Duration::from_secs(120),
        _ => MAX_RETRY_DELAY,
    }
}

/// min(NETWORK_RETRY_DELAY × (tries + 1), MAX_RETRY_DELAY).
pub(super) fn retry_delay(tries: i32) -> Duration {
    std::cmp::min(NETWORK_RETRY_DELAY * (tries.max(0) as u32 + 1), MAX_RETRY_DELAY)
}

/// The busiest with room for all (a server whose last player leaves closes).
pub(crate) fn roomy_server<'a>(servers: &'a [ServerInfo], party: i32, closed: &[String]) -> Option<&'a ServerInfo> {
    // MaxBy: the first of the busiest.
    let mut best: Option<&ServerInfo> = None;
    for sv in servers.iter().filter(|sv| sv.max_players - sv.playing >= party && !closed.iter().any(|j| j.eq_ignore_ascii_case(&sv.job_id))) {
        if best.is_none_or(|b| sv.playing > b.playing) {
            best = Some(sv);
        }
    }
    best
}

/// "0.#".
fn fmt_0_1(v: f64) -> String {
    let r = (v * 10.0).round() / 10.0;
    if r.fract() == 0.0 { format!("{}", r as i64) } else { format!("{r:.1}") }
}

/// The main window shown, not minimised (WinForms: IsHandleCreated && Visible && WindowState != Minimized).
fn window_shown(h: isize) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{IsIconic, IsWindowVisible};
    // SAFETY: plain calls on a window handle; an invalid one answers 0.
    unsafe { IsWindowVisible(win_kit::hwnd(h)) != 0 && IsIconic(win_kit::hwnd(h)) == 0 }
}

/// Cursor.Position.
fn cursor_position() -> (i32, i32) {
    use windows_sys::Win32::{Foundation::POINT, UI::WindowsAndMessaging::GetCursorPos};
    let mut p = POINT { x: 0, y: 0 };
    // SAFETY: GetCursorPos writes the POINT on the stack.
    unsafe {
        GetCursorPos(&mut p);
    }
    (p.x, p.y)
}
