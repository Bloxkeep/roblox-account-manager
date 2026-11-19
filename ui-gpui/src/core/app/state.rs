// AppController.State.cs: the state the page is drawn from: the app's whole state, each account's and game's part of it
// (what is installed: installs.rs).
//
// The JSON is the C#'s (JsonSerializerDefaults.Web): camelCase names, null written for a missing value.
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use serde_json::{Value, json};

use super::App;
use super::controller::{DISCONNECT_GRACE, PendingLaunch};
use super::notify::NOTIFY_KINDS;
use crate::core::base::{Guid, Stamp};
use crate::core::models::{Account, AppSettings, RecentGame};
use crate::core::roblox_api::{JoinAnswer, Presence};
use crate::core::{l, multi_instance, prefs, rt, start_with_windows, themes, vault, win_kit};

/// How long a teleport is shown as one: Roblox's took 0.5-3.6 s here (2026-10-21); one that failed leaves the window where it was.
pub const TELEPORT_SHOWN: Duration = Duration::from_secs(60);
pub const STALE_PRESENCE: Duration = Duration::from_secs(15 * 60);

/// The instance fields AppController.State.cs declares.
#[derive(Default)]
pub struct StateState {
    /// The page has something closing the browser would lose (a form, a sign-in): the window keeps it alive.
    pub page_busy: Cell<bool>,
    /// Clients that ended here, with their game and server. Roblox keeps reporting the account in that game long after:
    /// on 2026-10-13 a clean leave at 00:02:19 UTC was still "in game" at 00:06, flipping with offline. Until Roblox says
    /// otherwise, the same server (or game, when no server is given) is taken as stale, not "playing elsewhere". Kept in
    /// the vault (NoteOpenSessions) so a restart to update right after, or a lock, doesn't forget them.
    pub ended_here: RefCell<HashMap<Guid, (Option<String>, Option<i64>, Stamp)>>,
}

/// What a client's log line says, as `connection` names it.
pub fn log_connection(status: &str) -> &'static str {
    match status {
        "ingame" => "ingame",
        "joining" => "joining",
        "left" => "menu",
        "signedout" => "signedout",
        _ => "disconnected",
    }
}

fn secs(d: Duration) -> f64 {
    d.as_secs_f64()
}

/// What build_state asks Windows for: the start-with-Windows entry, the monitors and whether the vault file is there.
#[derive(Clone)]
struct Probes {
    start_with_windows: bool,
    monitors: Vec<Value>,
    vault_exists: bool,
}

/// How long the timers' pushes reuse those answers (a command's push reads them fresh).
const PROBES_KEPT: Duration = Duration::from_secs(2);

thread_local! {
    static PROBES: RefCell<Option<(std::time::Instant, Probes)>> = const { RefCell::new(None) };
    static TIMER_PUSH: Cell<bool> = const { Cell::new(false) };
}

/// Runs `f` (a timer's build_state) with the answers of the last 2 s reused instead of the registry, the monitors and
/// the disk being asked again every tick.
pub(super) fn with_kept_probes<R>(f: impl FnOnce() -> R) -> R {
    struct Off;
    impl Drop for Off {
        fn drop(&mut self) {
            TIMER_PUSH.with(|t| t.set(false));
        }
    }
    TIMER_PUSH.with(|t| t.set(true));
    let _off = Off;
    f()
}

fn probes() -> Probes {
    let reuse = TIMER_PUSH.with(Cell::get);
    if reuse && let Some(kept) = PROBES.with(|p| p.borrow().as_ref().filter(|(at, _)| at.elapsed() < PROBES_KEPT).map(|(_, k)| k.clone())) {
        return kept;
    }
    let fresh = Probes {
        start_with_windows: start_with_windows::is_on(),
        monitors: win_kit::all_screens()
            .into_iter()
            .enumerate()
            .map(|(i, s)| json!({ "index": i, "primary": s.primary, "width": s.bounds.width, "height": s.bounds.height }))
            .collect(),
        vault_exists: vault::exists(),
    };
    PROBES.with(|p| *p.borrow_mut() = Some((std::time::Instant::now(), fresh.clone())));
    fresh
}

impl App {
    /// The page's first request; says once whether it replaces an earlier page of this session.
    pub fn initial_state(self: &Rc<Self>) -> Value {
        let resume = self.controller.resuming_page.get();
        self.controller.resuming_page.set(false);
        self.controller.page_attached.set(true);
        self.forget_sleep(); // a new page is awake and gets everything here
        self.state.page_busy.set(false); // a fresh page has nothing open yet
        // A new page has nothing yet; send both again.
        *self.controller.last_metrics.borrow_mut() = None;
        *self.controller.last_state.borrow_mut() = None;
        if self.controller.vault.borrow().is_some() && self.controller.boot.borrow().is_none() {
            // A page made again (after the browser was closed in the background) gets ready too.
            let app = self.clone();
            rt::spawn(async move {
                let _ = app.settled().await;
            });
        }
        let view = if resume && self.open().is_some() { self.controller.page_view.borrow().clone() } else { None };
        *self.controller.page_view.borrow_mut() = None;
        json!({ "resume": resume, "view": view, "state": self.build_state() })
    }

    /// What the page had on screen (selection, filter, search, scroll), kept in memory while the browser is closed in
    /// the background so a recreated page looks the same. On disk only inside an update's encrypted hand-over (see Relaunch).
    /// `restart`: sent right before the page asks for a browser restart (GPU switch), while the window is in front.
    pub fn keep_view(&self, view: &str, restart: bool) -> Value {
        // One that arrives after the window is already back (a quick minimise and restore) is stale.
        if !self.controller.background.get() && !restart {
            return Value::Null;
        }
        // C#'s view.Length counts UTF-16 units.
        *self.controller.page_view.borrow_mut() = if view.encode_utf16().count() <= 256 * 1024 { Some(view.to_string()) } else { None };
        Value::Null
    }

    /// The window came back with its page still open: what the page kept is out of date from here on.
    pub fn drop_kept_view(&self) {
        // Not before any page has come: a view handed over by an update is for the first one.
        if self.controller.page_attached.get() {
            *self.controller.page_view.borrow_mut() = None;
        }
    }

    pub fn page_busy(&self) -> bool {
        self.state.page_busy.get()
    }

    pub fn set_page_busy(&self, busy: bool) -> Value {
        self.state.page_busy.set(busy);
        Value::Null
    }

    /// The page is going away (browser released or recreated): nothing it was doing survives, least of all a
    /// sign-in prompt holding a password or a Quick Login code waiting for approval.
    pub fn page_gone(&self) {
        self.controller.page_attached.set(false); // the next page asks for everything with getState
        self.forget_sleep();
        self.state.page_busy.set(false);
        // The next page says again where its monitor tab is, if it shows it.
        let set_area = self.monitor.set_monitor_area.borrow().clone();
        if let Some(set_area) = set_area {
            set_area(None);
        }
        let _ = self.login_cancel();
        let _ = self.quick_login_cancel();
        let _ = self.drop_vault_import(false);
    }

    /// Answered once the window's cover is off (the start logo between hops): the page's opening starts then.
    pub async fn on_page_shown(self: &Rc<Self>) -> Value {
        let shown = self.controller.page_shown.borrow().clone();
        if let Some(shown) = shown {
            shown().await;
        }
        Value::Null
    }

    pub fn build_state(&self) -> Value {
        let (theme, close_to_tray, global_hotkeys, hide_names, notifications, notify_off_set, daily_sign_in_check, last_sign_in_check, mini_open, safe_rendering, count_online, board_name, multi_at_start) = {
            let p = prefs::lock();
            (
                p.theme.clone(),
                p.close_to_tray,
                p.global_hotkeys,
                p.hide_names,
                p.notifications,
                p.notify_off.clone(),
                p.daily_sign_in_check,
                p.last_sign_in_check,
                p.mini_open,
                p.safe_rendering,
                p.count_online,
                p.board_name.clone(),
                p.multi_roblox_at_start,
            )
        };
        let mut notify_off: Vec<String> = notify_off_set.into_iter().filter(|k| NOTIFY_KINDS.contains(&k.as_str())).collect();
        notify_off.sort();
        let vault = self.controller.vault.borrow().clone();
        let open = self.open();
        let open_settings: Option<AppSettings> = open.as_ref().map(|v| v.data().settings.clone());
        let open_accounts: Vec<Account> = open.as_ref().map(|v| v.data().accounts.clone()).unwrap_or_default();
        let updater = self.updates.updater.borrow().clone();
        let updated_from = self.updates.updated_from.borrow().clone();
        let screen_locked = self.auto_lock.screen_locked.get();
        let installs = self.installs();
        let registry = self.registry_info();
        let Probes { start_with_windows: start_on, monitors, vault_exists } = probes();
        json!({
            "language": l::language(),
            "theme": theme,
            "themeApplied": themes::current().id,
            "themeLight": prefs::lock().system_light.clone(),
            "themeDark": prefs::lock().system_dark.clone(),
            "customTheme": App::custom_theme_dto(),
            "savedThemes": App::saved_themes_dto(),
            "closeToTray": close_to_tray,
            "startWithWindows": start_on,
            "hotkeys": { "on": global_hotkeys, "taken": self.hotkeys.hotkeys_taken.borrow().clone(), "typing": self.hotkeys.hotkeys_typing.borrow().clone() },
            "hideNames": hide_names,
            "notifications": notifications,
            "notifyOff": notify_off,
            "dailySignInCheck": daily_sign_in_check,
            "skipConfirm": App::skipped_confirms(),
            "lastSignInCheck": last_sign_in_check, // shown next to the switch, like the update check's time
            "miniOpen": mini_open,
            "backup": self.backup_dto(),
            "discord": self.discord_dto(),
            "phone": self.phone_dto(),
            "feeds": self.feeds_dto(),
            "wikis": App::wikis_dto(),
            "safeRendering": safe_rendering,
            "update": updater.dto(),
            // An update found before the unlock, which the lock screen gives its place to; go: its answer (null: not yet).
            "lockUpdate": if self.update_offers_at_lock() { json!({ "go": self.relaunch.update_at_lock.get() }) } else { Value::Null },
            "countOnline": count_online,
            "boardName": board_name,
            "community": self.community_dto(),
            "remote": self.remote_dto(),
            "robloxVersion": self.roblox_version_dto(),
            "robloxUpdates": self.roblox_updates_dto(),
            "whatsNew": match (&updated_from, &open) {
                (Some(from), Some(_)) => json!({ "from": from, "releases": super::updates::releases_json(&crate::core::whats_new::between(Some(from), &updater.current_version())) }),
                _ => Value::Null,
            },
            "vaultExists": vault_exists,
            "vaults": self.vaults_dto(),
            "multiAtStart": multi_at_start,
            "locked": open.is_none(),
            // The lock screen after a while without use: Bloxkeep is open behind it, Roblox and the background carry on.
            "screenLock": if screen_locked { json!({ "minutes": vault.as_ref().map(|v| v.data().settings.auto_lock_minutes).unwrap_or(0) }) } else { Value::Null },
            "dataPath": vault::data_dir().to_string_lossy(),
            "paths": {
                "vault": vault.as_ref().map(|v| v.file_path().to_string_lossy().into_owned()),
                "log": App::log_path(),
                "app": App::app_folder(),
            },
            // Not only the roblox-player: links: another Roblox's uninstaller takes them with it (Roblox VNG's, 2026-10-28)
            // while a client, a launcher or the Store edition is still here, and Default opens that one (FallbackLauncher).
            "robloxInstalled": registry.0 || installs.direct || installs.store || installs.bloxstrap || installs.fishstrap || installs.voidstrap || !installs.others.is_empty(),
            "launching": self.controller.launch_cts.borrow().is_some(),
            "checking": self.controller.checking.get(),
            "boot": if open.is_none() { Value::Null } else { self.controller.boot.borrow().as_ref().map(|b| json!({ "stage": b.stage, "percent": b.percent })).unwrap_or(Value::Null) },
            "multi": {
                "enabled": open_settings.as_ref().map(|s| s.multi_instance).unwrap_or(false),
                "armed": multi_instance::armed(),
                "windowed": open_settings.as_ref().map(|s| s.multi_instance_windowed).unwrap_or(true),
                "tile": open_settings.as_ref().map(|s| s.multi_instance_tile).unwrap_or(false),
                "trayAutostart": registry.1,
            },
            "installs": installs.to_json(),
            "monitors": monitors,
            "settings": match &open_settings { Some(s) => self.settings_dto(s), None => Value::Null },
            "accounts": open_accounts.iter().map(|a| self.account_dto(a)).collect::<Vec<_>>(),
            // Macros (Beta), AppController.Macros.cs: nothing of them while locked.
            "macroRuns": self.macro_runs_dto(),
            "macroRecording": self.macro_recording_dto(),
            "macroVars": self.macro_vars_dto(),
            "macroSweep": self.macro_sweep_dto(),
            "macroCanRun": self.macro_can_run_dto(),
            "panelsShown": self.panels_shown_dto(),
        })
    }

    pub fn account_dto(&self, a: &Account) -> Value {
        let run = self.controller.running.borrow().get(&a.id).cloned();
        let launching = self
            .controller
            .pending
            .borrow()
            .iter()
            .any(|p| p.account_id == a.id && !self.controller.close_when_claimed.borrow().contains(&super::controller::LaunchRef(p.clone())));
        let state = if run.is_some() {
            "running"
        } else if launching {
            "launching"
        } else {
            "idle"
        };
        let session = match &run {
            Some(run) => {
                let (pid, launch) = (run.0, &run.1);
                let log = self.controller.logs.borrow().state_of(pid);
                let disconnected = log.as_ref().filter(|s| s.status == "disconnected");
                let phase = self.phase(pid);
                let banned_window = self.banned_window(a, run);
                json!({
                    "pid": pid,
                    "since": launch.started_at.get(),
                    "game": self.session_game_dto(pid, launch),
                    "connection": self.connection(a, pid, launch),
                    "phase": phase,
                    "queuePosition": if phase.as_deref() == Some("full") { self.join_answer_of(pid).and_then(|j| j.queue_position) } else { None },
                    "reason": disconnected.and_then(|s| s.reason),
                    // The game's words with a kick or ban, and whether it was a ban (both come with Roblox's code 267), or
                    // the window is stuck on Roblox's Join Error 600 for a game that banned the account (its log says nothing).
                    "kickMessage": match disconnected {
                        Some(said) => said.kick_message.clone(),
                        None => banned_window.as_ref().and_then(|stuck| stuck.message.clone()),
                    },
                    "bannedByGame": disconnected.is_some_and(|s| s.banned_by_game) || banned_window.is_some(),
                    "jobId": log.as_ref().and_then(|s| s.job_id.clone()),
                    "jobPlaceId": log.as_ref().and_then(|s| s.place_id),
                    "hidden": crate::core::roblox_processes::has_hidden(pid),
                    "fpsLimit": launch.fps_limit,
                    "robloxVersion": self.roblox_updates.client_builds.borrow().get(&pid).cloned().flatten().map(|b| b.version),
                    "oldBuild": self.runs_old_build(a, pid),
                    // The preset that opened it may turn auto-rejoin on or off for this client.
                    "autoRejoin": self.rejoin_on(a, launch.preset_id),
                    // What Bloxkeep does next with it, and when: the card says so (the owner, 2026-11-15: "ở thẻ cần
                    // hiển thị sẽ làm gì tiếp theo, ví dụ rejoin thì hiện thời gian đếm ngược").
                    "next": self.next_step(a, launch, log.as_ref()),
                })
            }
            None => Value::Null,
        };
        // A rejoin waiting its turn (auto-rejoin's wait, or a preset's restart): the card counts down and can call it off.
        // Also one under way (out of the queue, its window not asked for yet): "Rejoining…", and Cancel and Close still stop it.
        let queued = self.controller.rejoin_queue.borrow().iter().find(|r| r.account_id == a.id).cloned().or_else(|| {
            self.rejoin.rejoin_in_flight.borrow().as_ref().filter(|r| r.account_id == a.id).map(|r| (**r).clone())
        });
        let rejoin = match queued {
            Some(q) => json!({ "at": q.due, "restart": q.restart, "game": self.game_dto(Some(q.target.place_id)) }),
            None => Value::Null,
        };
        json!({
            "id": a.id,
            "userId": a.user_id,
            "username": a.username,
            "displayName": a.display_name,
            "alias": a.alias,
            "label": a.label(),
            "group": a.group,
            "note": a.note,
            "hasPassword": !a.password.is_empty(),
            "hasAuthenticator": a.authenticator.is_some(),
            "banned": a.banned == Some(true),
            "bannedSince": a.banned_since,
            "realm": a.realm,
            "cookieValid": a.cookie_valid,
            "lastUsed": a.last_used,
            "autoRejoin": a.auto_rejoin,
            "useRobloxConfig": a.use_roblox_config,
            "fpsCap": a.fps_cap,
            "isMain": a.is_main,
            "avatarUrl": self.controller.avatar_urls.borrow().get(&a.user_id).cloned(),
            "robux": self.robux_of(a),
            "robuxWait": self.robux_waiting(a),
            "state": state,
            "presence": self.presence_dto(a),
            "session": session,
            "rejoin": rejoin,
        })
    }

    /// What comes next for a window, for its card: { kind: "rejoin", at } (dropped: closed and rejoined then),
    /// "waitDevice" (another device took the account: rejoined when it leaves), "checking" (a teleport taking too
    /// long: Roblox's status is being asked). None when nothing is planned (rejoin off, a code Roblox won't reconnect
    /// after, a ban).
    fn next_step(&self, a: &Account, launch: &PendingLaunch, log: Option<&crate::core::roblox_log_watcher::ClientState>) -> Option<Value> {
        use super::rejoin::{roblox_wont_reconnect, signed_in_elsewhere};
        let log = log?;
        if !self.rejoin_on(a, launch.preset_id) || a.banned == Some(true) {
            return None;
        }
        let moving_long = (log.status == "ingame" || log.status == "joining") && log.teleport_started.is_some_and(|t| Stamp::utc_now().secs_since(t) >= super::presets::secs(super::controller::TELEPORT_GIVE_UP));
        if moving_long {
            return Some(json!({ "kind": "checking" }));
        }
        if log.status != "disconnected" || log.banned_by_game || roblox_wont_reconnect(log.reason) {
            return None;
        }
        if signed_in_elsewhere(log.reason) {
            return Some(json!({ "kind": "waitDevice" }));
        }
        let wait = self.vault(|d| d.settings.rejoin_wait_seconds).unwrap_or(0).max(3) as f64;
        Some(json!({ "kind": "rejoin", "at": log.at.plus_secs(wait).to_iso() }))
    }

    /// What a client here is doing: "ingame", "joining" (just started or not known), "menu" (no game, or left it),
    /// "disconnected" (kicked or lost connection; the window stays on Roblox's error screen). Its log says so at once;
    /// without one, Roblox's status for the account is used.
    pub fn connection(&self, a: &Account, pid: i32, launch: &PendingLaunch) -> &'static str {
        if let Some(log) = self.controller.logs.borrow().state_of(pid) {
            return log_connection(&log.status);
        }
        let p = self.controller.presence.borrow().get(&a.user_id).cloned();
        let reliable = self.controller.presence_reliable.borrow().contains(&a.user_id);
        if let Some(p) = &p {
            if p.r#type == 2 {
                return "ingame";
            }
            // Online but not in a game: in Roblox's menu (no log line said otherwise), not cut off.
            if p.r#type == 1 && reliable {
                return "menu";
            }
        }
        if p.is_none() || !reliable || Stamp::utc_now().secs_since(launch.started_at.get()) < secs(DISCONNECT_GRACE) {
            return "joining";
        }
        if launch.target.is_none() { "menu" } else { "disconnected" }
    }

    /// Roblox's answer about a window that hasn't got in (CheckJoinAsync), once it came.
    pub fn join_answer_of(&self, pid: i32) -> Option<JoinAnswer> {
        let asked = self.rejoin.join_checks.borrow().get(&pid).cloned()?;
        let answer = asked.answer.borrow().clone();
        match answer {
            Some(Some(answer)) => Some(answer),
            // Asked again while in a full game's line: the place it had, until the new answer comes.
            None => self.rejoin.in_line.borrow().get(&pid).cloned(),
            Some(None) => None,
        }
    }

    /// What a client's log says beyond `connection`: "teleporting"; "waiting" (no server yet: normally a second, longer
    /// in a full server's line or on a join error, which the log doesn't tell apart); "full" (Roblox's join answer says
    /// so); "joinFailed" (left before any server took it); or None.
    pub fn phase(&self, pid: i32) -> Option<String> {
        let log = self.controller.logs.borrow().state_of(pid)?;
        let joining = log.status == "joining";
        if (log.status == "ingame" || joining)
            && let Some(t) = log.teleport_started
            && Stamp::utc_now().secs_since(t) < secs(TELEPORT_SHOWN)
        {
            return Some("teleporting".into());
        }
        if joining && log.job_id.is_none() {
            if self.controller.logs.borrow().join_challenged_at(pid).is_some() {
                return Some("challenged".into());
            }
            if self.join_answer_of(pid).is_some_and(|j| j.full()) {
                return Some("full".into());
            }
            return Some("waiting".into());
        }
        if log.status == "left" && log.never_joined {
            return Some("joinFailed".into());
        }
        None
    }

    pub async fn name_logged_places(self: &Rc<Self>, place_ids: Vec<i64>) {
        let _ = self.load_games(place_ids.iter().map(|id| (*id, 0i64)).collect(), false).await;
        // Nothing came back (offline, rate-limited): the page would show the same thing.
        let any = place_ids.iter().any(|id| self.controller.games.borrow().get(id).is_some_and(Option::is_some));
        if any {
            self.push_state_if_changed();
        }
    }

    /// The game a client here is in: the place its log reports once that place is named, else the game it was opened
    /// with (games often move players to another of their places right after they join).
    pub fn session_game_dto(&self, pid: i32, launch: &PendingLaunch) -> Value {
        let logged = self.controller.logs.borrow().state_of(pid).and_then(|s| s.place_id);
        let opened = launch.target.as_ref().map(|t| t.place_id);
        let logged_named = logged.is_some_and(|l| self.controller.games.borrow().get(&l).and_then(|g| g.as_ref()).is_some_and(|g| !g.name.is_empty()));
        self.game_dto(if logged_named || opened.is_none() { logged.or(opened) } else { opened })
    }

    pub fn game_dto(&self, place_id: Option<i64>) -> Value {
        match place_id {
            Some(id) => {
                let info = self.controller.games.borrow().get(&id).cloned().flatten();
                json!({
                    "placeId": id,
                    "name": info.as_ref().map(|g| g.name.clone()).unwrap_or_default(),
                    "iconUrl": info.and_then(|g| g.icon_url),
                })
            }
            None => Value::Null,
        }
    }

    pub fn is_stale_in_game(&self, a: &Account, p: &Presence) -> bool {
        if p.r#type != 2 || self.controller.running.borrow().contains_key(&a.id) {
            return false;
        }
        let Some((ended_job, ended_place, ended_at)) = self.state.ended_here.borrow().get(&a.id).cloned() else {
            return false;
        };
        if Stamp::utc_now().secs_since(ended_at) >= secs(STALE_PRESENCE) {
            return false;
        }
        if let (Some(job), Some(ended_job)) = (&p.job_id, &ended_job) {
            return job.to_lowercase() == ended_job.to_lowercase();
        }
        // No server id to compare: the same game counts, and so does no game at all (privacy hides it).
        let place = p.root_place_id.or(p.place_id);
        place.is_none() || ended_place.is_none() || place == ended_place || p.place_id == ended_place
    }

    pub fn presence_dto(&self, a: &Account) -> Value {
        let Some(p) = self.controller.presence.borrow().get(&a.user_id).cloned() else {
            return Value::Null;
        };
        if self.is_stale_in_game(a, &p) {
            let at = self.state.ended_here.borrow().get(&a.id).map(|e| e.2);
            return json!({ "type": "offline", "game": null, "placeId": null, "jobId": null, "joinPlaceId": null, "iconUrl": null, "lastOnline": at });
        }
        let place_id = p.root_place_id.or(p.place_id);
        let info = place_id.and_then(|id| self.controller.games.borrow().get(&id).cloned().flatten());
        json!({
            "type": match p.r#type { 0 => "offline", 1 => "online", 2 => "ingame", 3 => "studio", 4 => "invisible", _ => "unknown" },
            "game": match info.as_ref().map(|g| g.name.clone()) { Some(name) if !name.is_empty() => Some(name), _ => p.location.clone() },
            "placeId": place_id,
            "jobId": p.job_id,
            // The place that server belongs to (placeId above is the game's start place, for its name and icon).
            "joinPlaceId": p.place_id,
            "iconUrl": info.and_then(|g| g.icon_url),
            "lastOnline": p.last_online,
        })
    }

    pub fn settings_dto(&self, s: &AppSettings) -> Value {
        let c = &s.roblox_config;
        json!({
            "autoLockMinutes": s.auto_lock_minutes,
            "lastPlaceInput": s.last_place_input,
            "lastJobId": s.last_job_id,
            "renameWindows": s.rename_windows,
            "windowPictures": s.window_pictures.unwrap_or(s.rename_windows),
            "taskbarGroups": s.taskbar_groups,
            "windowLayout": s.window_layout,
            "windowWidth": s.window_width,
            "windowHeight": s.window_height,
            "maximizeOnOpen": s.maximize_on_open,
            "launchVia": s.launch_via,
            "customLauncher": if s.custom_launcher.is_empty() { None } else { Some(win_kit::file_stem(&s.custom_launcher)) },
            "layout": s.layout,
            "sort": s.sort,
            "launchDelaySeconds": s.launch_delay_seconds,
            // (Missing in the C# too, whose page kept its own copy: the chips here read the state, and went back to Off.)
            "hopAbovePlayers": s.hop_above_players,
            "rejoinWaitSeconds": s.rejoin_wait_seconds,
            "memoryWarnings": s.memory_warnings,
            "trimBackground": s.trim_background,
            "autoUpdateRoblox": s.auto_update_roblox,
            "ownRobloxLinks": crate::core::roblox_links::shown(),
            "robloxUpdateWhen": s.roblox_update_when,
            "fastFlags": s.fast_flags,
            "ramLimitMb": s.ram_limit_mb,
            "restartAboveMb": s.restart_above_mb,
            "splitCores": s.split_cores,
            "cores": App::cores_dto(),
            "antiAfk": s.anti_afk,
            "robloxConfig": { "mode": c.mode, "graphics": c.graphics, "fps": c.fps, "volume": c.volume },
            "recentGames": s.recent_games.iter().map(|g| self.saved_game_dto(g)).collect::<Vec<_>>(),
            "pinnedGames": s.pinned_games.iter().map(|g| self.saved_game_dto(g)).collect::<Vec<_>>(),
            "savedServers": s.saved_servers.iter().map(|v| self.saved_server_dto(v)).collect::<Vec<_>>(),
            "recentPlayers": s.recent_players,
            "followJoinedPlayer": s.follow_joined_player,
            "presets": s.presets.iter().map(|p| self.preset_dto(p)).collect::<Vec<_>>(),
            // The macros as they are, in the page's camelCase shape (Web options), unknown fields as written.
            "macros": s.macros.iter().map(|m| {
                let v = serde_json::to_value(m).unwrap_or(Value::Null);
                crate::core::macro_model::macro_rules::page_json::to_page(v, crate::core::macro_model::macro_rules::page_json::Shape::Macro)
            }).collect::<Vec<_>>(),
            "launchOrder": self.launch_order_dto(),
        })
    }

    pub fn saved_game_dto(&self, g: &RecentGame) -> Value {
        let icon = self
            .controller
            .games
            .borrow()
            .get(&g.place_id)
            .cloned()
            .flatten()
            .and_then(|i| i.icon_url)
            .or_else(|| self.game_picker.icon_by_universe.borrow().get(&g.universe_id).cloned());
        json!({
            "placeId": g.place_id,
            "universeId": g.universe_id,
            "name": if g.name.is_empty() { format!("Place {}", g.place_id) } else { g.name.clone() },
            "iconUrl": icon,
        })
    }

    // RequireVault (throws UserError "Bloxkeep is locked.") and RequireAccount (that, or UserError "Account not found.")
    // are mod.rs's `vault`/`account` helpers; FindAccount is mod.rs's `find_account`.
}
