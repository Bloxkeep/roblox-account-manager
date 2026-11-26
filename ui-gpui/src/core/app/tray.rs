// AppController.Tray.cs: what the tray's quick panel and menu (TrayWindow) show and do: Multi-Roblox's state, a new Roblox
// window, accounts and presets to open, and closing every Roblox, as the page does. With Multi-Roblox only (no vault, see
// `multi_only`) Multi-Roblox turns on and off here too.
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use serde_json::{Value, json};

use super::App;
use super::controller::{MB, error_text};
use crate::core::base::{Fail, Guid, Res, Stamp};
use crate::core::close_plan::{Open, Who};
use crate::core::forms::dialog;
use crate::core::game_target::GameTarget;
use crate::core::models::stamp_min;
use crate::core::roblox_api::is_fail_kind;
use crate::core::roblox_home_session::RobloxHomeSession;
use crate::core::roblox_processes::{self, PlayerProcess};
use crate::core::{l, log, multi_instance, prefs, roblox_installs, rt, ui, vault, win_kit};

/// `open`: a vault is open and the screen isn't locked: accounts, presets and closing Roblox are offered.
/// `multi`: "off", "ready" (Multi-Roblox holds Roblox's lock) or "notReady".
/// `accounts`: the accounts to open, most recently used first (at most 12), and whether each is open.
/// `clients`: the accounts' Roblox windows open now, in the launch order (the quick panel's list).
/// `ram_mb`: what every Roblox window on this PC holds in RAM together.
#[derive(Clone, Debug)]
pub struct TrayInfo {
    pub open: bool,
    pub multi: String,
    pub roblox_count: i32,
    pub presets: Vec<(Guid, String)>,
    pub accounts: Option<Vec<(Guid, String, bool)>>,
    pub multi_only: bool,
    pub clients: Option<Vec<TrayClient>>,
    pub ram_mb: i64,
    /// (id, name, shown, state)
    pub macros: Option<Vec<(Guid, String, bool, String)>>,
}

#[derive(Clone, Debug)]
pub struct TrayClient {
    pub id: Guid,
    pub name: String,
    pub game: String,
    pub pid: i32,
    pub ram_mb: i64,
}

pub const TRAY_ACCOUNTS: usize = 12;

/// Where the separate sign-in windows' junctions go (C#'s static readonly HomeAliases).
pub fn home_aliases() -> PathBuf {
    vault::cache_dir().join("roblox-client-paths")
}

/// The instance fields AppController.Tray.cs declares.
pub struct TrayState {
    pub home_session: RefCell<Option<RobloxHomeSession>>,
    pub opening_home: Cell<bool>,
    /// The "separate sign-in windows are experimental" question is open: another click waits for it, nothing else does.
    pub asking_home: Cell<bool>,
    pub home_started_at: Cell<Stamp>,
}

impl Default for TrayState {
    fn default() -> Self {
        TrayState { home_session: RefCell::new(None), opening_home: Cell::new(false), asking_home: Cell::new(false), home_started_at: Cell::new(stamp_min()) }
    }
}

/// The failures the C# caught here: Win32Exception, IOException (and its kinds) and UnauthorizedAccessException.
fn is_io_failure(fail: &Fail) -> bool {
    ["Win32Exception", "IOException", "FileNotFoundException", "DirectoryNotFoundException", "PathTooLongException", "UnauthorizedAccessException"]
        .iter()
        .any(|k| is_fail_kind(fail, k))
}

/// Only IOException (and its kinds): what RobloxHomeSession's constructor throws while Roblox holds a file.
fn is_io_exception(fail: &Fail) -> bool {
    ["IOException", "FileNotFoundException", "DirectoryNotFoundException", "PathTooLongException"].iter().any(|k| is_fail_kind(fail, k))
}

/// Multi-Roblox turned on while a Roblox client runs: Roblox already holds the name Multi-Roblox needs, so it starts
/// only once every client is closed (the process check takes it then). Many didn't know; said as a toast and a Windows
/// notification, since someone using only their browser may never look at Bloxkeep.
pub const MULTI_CLOSE_FIRST: &str = "Roblox is open, so Multi-Roblox can't start yet. Close every Roblox window (and Roblox in the tray): it turns on by itself.";

impl App {
    pub fn tray_state(&self) -> TrayInfo {
        let vault = if self.auto_lock.screen_locked.get() { None } else { self.controller.vault.borrow().clone() };
        let open = vault.is_some();
        let on = match &vault {
            Some(v) => v.data().settings.multi_instance,
            None => prefs::lock().multi_roblox_at_start,
        };
        let (presets, picked) = match &vault {
            Some(v) => {
                let d = v.data();
                let presets: Vec<(Guid, String)> = d.settings.presets.iter().map(|p| (p.id, p.name.clone())).collect();
                let mut accounts: Vec<_> = d.accounts.iter().filter(|a| a.user_id > 0 && a.banned != Some(true)).cloned().collect();
                // OrderByDescending(LastUsed ?? DateTime.MinValue): a stable sort, as LINQ's.
                accounts.sort_by_key(|a| std::cmp::Reverse(a.last_used.unwrap_or_else(stamp_min).unix_ms()));
                accounts.truncate(TRAY_ACCOUNTS);
                (presets, accounts)
            }
            None => (Vec::new(), Vec::new()),
        };
        let accounts: Vec<(Guid, String, bool)> = picked
            .iter()
            .map(|a| {
                let running = self.controller.running.borrow().contains_key(&a.id);
                (a.id, self.name_text(a), running)
            })
            .collect();
        let clients: Vec<TrayClient> = if open {
            self.running_clients().into_iter().map(|c| TrayClient { id: c.id, name: c.label, game: c.game, pid: c.pid, ram_mb: c.ram_mb }).collect()
        } else {
            Vec::new()
        };
        let multi = if !on {
            "off"
        } else if multi_instance::armed() {
            "ready"
        } else {
            "notReady"
        };
        let ram: i64 = self.mini.client_ram.borrow().values().sum();
        TrayInfo {
            open,
            multi: multi.into(),
            roblox_count: self.roblox_count(),
            presets,
            accounts: Some(accounts),
            multi_only: !vault::exists(),
            clients: Some(clients),
            ram_mb: ram / MB,
            macros: Some(self.tray_macros()),
        }
    }

    /// The account's picture for the tray's page: Roblox's address for it, once known (its initials meanwhile),
    /// names hidden or not (hiding names keeps the pictures, as Discord's streamer mode does).
    pub fn tray_picture_url(&self, id: Guid) -> Option<String> {
        let account = self.find_account(id).filter(|a| a.user_id > 0)?;
        self.controller.avatar_urls.borrow().get(&account.user_id).cloned()
    }

    pub fn tray_custom_theme(&self) -> Value {
        App::custom_theme_dto()
    }

    /// The quick panel's eye: the account's Roblox window in front.
    pub fn tray_show_client(&self, pid: i32) {
        log::note(&format!("Tray: Show on Roblox window {pid}."));
        roblox_processes::focus_in_turn(pid);
    }

    /// The menu's Multi-Roblox item: only off while on, like the sidebar's switch (turning on asks first, in the
    /// page). With Multi-Roblox only, both ways (its warning was answered when that was chosen).
    pub fn tray_multi(self: &Rc<Self>, on: bool) {
        if !vault::exists() {
            // Vault.Exists was just checked, so MultiOnly's refusal can't come.
            let _ = self.multi_only(on);
            return;
        }
        if self.controller.vault.borrow().is_none() || self.auto_lock.screen_locked.get() {
            log::note("Tray: Multi-Roblox item did nothing (locked).");
            return;
        }
        if on {
            // Turning on is asked in Bloxkeep: its window is up (the tray's "multiAsk"), and the page asks Multi-Roblox's
            // question there (or turns it on at once when the person said not to ask again).
            log::note("Tray: Multi-Roblox turned on from the tray, asked in Bloxkeep.");
            self.emit("askMulti", Value::Null);
            return;
        }
        // The C# let SetSettings' exception go up into the tray's click; here it is written to the log.
        if let Err(e) = self.set_settings(&json!({ "multiInstance": false })) {
            log::write(&e);
        }
        self.push_state();
    }

    /// First start › "Just use Multi-Roblox": no vault or account, for someone who signs in to Roblox only in their
    /// browser. Kept on from start-up (prefs' multi_roblox_at_start); a vault made later takes the setting over.
    pub fn multi_only(&self, on: bool) -> Res<Value> {
        if vault::exists() {
            return Err(Fail::user(l::t("Bloxkeep has a vault on this PC: Multi-Roblox is turned on or off inside it.")));
        }
        prefs::lock().multi_roblox_at_start = on;
        prefs::save();
        if on {
            multi_instance::enable();
        } else {
            multi_instance::disable();
        }
        self.controller.last_armed.set(multi_instance::armed());
        if on && !multi_instance::armed() {
            self.warn(&l::t(MULTI_CLOSE_FIRST));
        }
        self.push_state();
        Ok(Value::Null)
    }

    /// What the tray menu couldn't do, as a toast and a Windows notification (nothing once closed).
    pub fn warn(&self, text: &str) {
        if self.controller.disposed.get() {
            return;
        }
        self.toast(text, "warn");
        let show = self.notify.show_notification.borrow().clone();
        if let Some(show) = show {
            show(text, "warn");
        }
    }

    /// A new sign-in window without an account or ticket from Bloxkeep: the installed client opened through a
    /// junction, keeping its mods; shared sign-in files stay locked until clients using these aliases close.
    /// (C#'s `async void`: the tray spawns it.)
    pub async fn tray_new_roblox(self: &Rc<Self>) {
        let disposed = self.controller.disposed.get();
        if disposed || self.tray.opening_home.get() || self.tray.asking_home.get() || self.updates.applying_update.get() || self.relaunch.relaunching.get() {
            let why = if disposed {
                "closing"
            } else if self.tray.opening_home.get() {
                "one is opening"
            } else if self.tray.asking_home.get() {
                "its question is open"
            } else {
                "an update or restart is under way"
            };
            log::note(&format!("Tray: New Roblox did nothing ({why})."));
            return;
        }
        if self.opening_roblox() {
            self.warn(&l::t("Wait until Bloxkeep finishes opening Roblox."));
            return;
        }
        let warn = |text: &str| self.warn(text);
        if !self.home_launch_ready(&warn) {
            return;
        }
        let (via, custom) = match self.controller.vault.borrow().as_ref() {
            Some(v) => {
                let d = v.data();
                (d.settings.launch_via.clone(), d.settings.custom_launcher.clone())
            }
            None => ("auto".to_string(), String::new()),
        };
        // PORT: the C# caught Win32Exception/IOException/UnauthorizedAccessException from ClientFor here and said
        // "Couldn't open a separate Roblox sign-in window…"; roblox_installs::client_for answers None instead.
        let Some(client) = roblox_installs::client_for(&via, &custom) else {
            self.warn(&l::t("Roblox isn't installed on this PC."));
            return;
        };
        if roblox_installs::is_store_client(&client) {
            self.warn(&l::t("Separate sign-in windows currently need Roblox from roblox.com, rather than the Microsoft Store edition."));
            return;
        }
        // The question holds nothing up: auto-rejoin, schedules and the rest carry on while it waits for an answer
        // (a window that lost its game while it was open was closed for a rejoin that then never came).
        if self.tray.home_session.borrow().is_none() {
            let asking = crate::core::base::Busy::set(&self.tray.asking_home);
            let owner = self.controller.owner.get();
            let text = l::t("Separate sign-in windows are experimental. All Roblox windows temporarily stop saving sign-ins and some settings. Your usual saved sign-in may expire. If Bloxkeep quits or crashes while a separate window stays open, that window may replace the saved sign-in. Close separate windows before quitting Bloxkeep. Long sessions have not been verified.");
            let (yes, no) = (l::t("Open separate window"), l::t("Cancel"));
            // PORT: Dialog.Confirm ran a nested message loop on the UI thread, so the rest went on meanwhile; here the
            // box waits on a worker thread and the core's thread carries on.
            let ok = rt::blocking(move || ui::modal(|| dialog::confirm(Some(owner), &text, &yes, &no, true))).await;
            drop(asking);
            if !ok {
                return;
            }
        }
        if self.controller.disposed.get() {
            return;
        }
        // Something may have started opening Roblox while the question was open (a rejoin): it goes first.
        if self.opening_roblox() {
            self.warn(&l::t("Wait until Bloxkeep finishes opening Roblox."));
            return;
        }
        // Roblox updating (close_plan.rs O1): the window would be closed by it, or make it give up.
        if let Open::Refuse(no) = self.may_open(Who::Person, None, Stamp::utc_now()) {
            log::note(&format!("Tray: New Roblox did nothing ({}).", no.words()));
            self.warn(&App::open_refused_text(no).unwrap_or_default());
            return;
        }
        let mut acquired = false;
        let mut started = false;
        let opening = crate::core::base::Busy::set(&self.tray.opening_home);
        let outcome = self.open_home_window(&client, &mut acquired, &mut started).await;
        if let Err(e) = outcome {
            if is_io_failure(&e) {
                if acquired && !started {
                    let session = self.tray.home_session.borrow_mut().take();
                    if let Some(mut s) = session {
                        s.dispose();
                    }
                }
                log::write(&e);
                self.warn(&l::t("Couldn't open a separate Roblox sign-in window. Try again, or close Roblox and retry."));
            } else {
                // PORT: any other exception went up into `async void` (the app's unhandled-exception handler).
                log::write(&e);
            }
        }
        drop(opening);
        if !self.controller.disposed.get() {
            self.push_state();
        }
    }

    /// TrayNewRoblox's try block: Ok(()) on every `return` there, Err on what its catch takes.
    async fn open_home_window(self: &Rc<Self>, client: &str, acquired: &mut bool, started: &mut bool) -> Res<()> {
        let warn = |text: &str| self.warn(text);
        if !self.home_launch_ready(&warn) {
            return Ok(());
        }
        if roblox_processes::close_tray_copies() > 0 {
            rt::sleep(Duration::from_millis(250)).await;
        }
        let aliases = home_aliases().to_string_lossy().into_owned();
        if self.tray.home_session.borrow().is_none() {
            if roblox_processes::get_player_pids().is_empty() {
                RobloxHomeSession::cleanup_aliases(&aliases);
            }
            let storage = win_kit::local_app_data().join("Roblox").join("LocalStorage").to_string_lossy().into_owned();
            // Roblox can have one of these files open briefly while saving. Each failed attempt releases both.
            let mut attempt = 0;
            loop {
                if !self.home_launch_ready(&warn) {
                    return Ok(());
                }
                match RobloxHomeSession::new(&storage, &aliases) {
                    Ok(session) => {
                        *self.tray.home_session.borrow_mut() = Some(session);
                        *acquired = true;
                        break;
                    }
                    Err(e) if is_io_exception(&e) && attempt < 9 => {
                        rt::sleep(Duration::from_millis(100)).await;
                    }
                    Err(e) => return Err(e),
                }
                attempt += 1;
            }
        }
        if !self.home_launch_ready(&warn) {
            if *acquired {
                let session = self.tray.home_session.borrow_mut().take();
                if let Some(mut s) = session {
                    s.dispose();
                }
            }
            return Ok(());
        }
        self.tray.home_started_at.set(Stamp::utc_now());
        {
            let mut session = self.tray.home_session.borrow_mut();
            // Not None here: either it was before, or it was just made above.
            if let Some(s) = session.as_mut() {
                s.start(client)?;
            }
        }
        *started = true;
        self.processes.last_scan_found_none.set(false);
        self.processes.last_scan.set(stamp_min());
        if *acquired {
            let text = l::t("A separate Roblox sign-in window is opening. Keep Bloxkeep running; these windows don't save their sign-ins when closed.");
            self.toast(&text, "info");
            let show = self.notify.show_notification.borrow().clone();
            if let Some(show) = show {
                show(&text, "info");
            }
        }
        Ok(())
    }

    /// No automatic or manual update may drop the file locks while separate sign-ins are in use.
    pub fn home_sessions_active(&self) -> bool {
        self.tray.home_session.borrow().is_some()
    }

    pub fn home_launch_ready(&self, tell: &dyn Fn(&str)) -> bool {
        if self.controller.disposed.get() {
            return false;
        }
        let state = self.tray_state();
        if state.multi == "ready" {
            return true;
        }
        tell(&if state.multi == "off" { l::t("Turn on Multi-Roblox before opening a separate Roblox sign-in window.") } else { App::multi_not_ready_text() });
        false
    }

    pub fn release_home_session_if_idle(&self, scan: bool, players: &[PlayerProcess], trays: &HashSet<i32>, now: Stamp) {
        if !scan || self.tray.home_session.borrow().is_none() || self.opening_roblox() || now.secs_since(self.tray.home_started_at.get()) < 15.0 {
            return;
        }
        let (pids, uncertain) = match self.tray.home_session.borrow_mut().as_mut() {
            Some(s) => s.find_clients(players, None),
            None => return,
        };
        // Only our own windowless copies are ended. A normal client or tray copy never extends this session's life.
        for pid in pids.iter().filter(|p| trays.contains(p)) {
            roblox_processes::kill(*pid);
        }
        // Even a tray we just ended stays counted until a later snapshot confirms it really exited.
        if !pids.is_empty() || uncertain {
            return;
        }
        let session = self.tray.home_session.borrow_mut().take();
        if let Some(mut s) = session {
            s.dispose();
        }
        RobloxHomeSession::cleanup_aliases(&home_aliases().to_string_lossy());
    }

    /// An account from the menu, as its card's Play: into the dock's chosen game, else Roblox's home screen. A
    /// launch with something to ask first isn't opened from here; that is said, to be done from Bloxkeep.
    /// (C#'s `async void`: the tray spawns it.)
    pub async fn tray_open_account(self: &Rc<Self>, id: Guid) {
        let short = id.to_string()[..8].to_string();
        match self.tray_open_account_run(id, &short).await {
            Ok(()) => {}
            Err(Fail::User(m)) => {
                log::note(&format!("Tray: account {short} not opened (refused; the person was told why)."));
                self.warn(&m);
            }
            Err(e) => self.warn(&error_text(&e)),
        }
    }

    async fn tray_open_account_run(self: &Rc<Self>, id: Guid, short: &str) -> Res<()> {
        let vault = if self.auto_lock.screen_locked.get() { None } else { self.controller.vault.borrow().clone() };
        let account = if vault.is_some() { self.find_account(id) } else { None };
        let (Some(vault), Some(account)) = (vault, account) else {
            let why = if self.controller.vault.borrow().is_none() || self.auto_lock.screen_locked.get() { "locked" } else { "no such account" };
            log::note(&format!("Tray: account {short} not opened ({why})."));
            return Ok(());
        };
        let (place_input, job_id, delay) = {
            let d = vault.data();
            (d.settings.last_place_input.clone(), d.settings.last_job_id.clone(), d.settings.launch_delay_seconds)
        };
        let mut target: Option<GameTarget> = None;
        if !place_input.is_empty() {
            match GameTarget::try_parse(&place_input, &job_id) {
                Ok(t) => target = Some(t),
                Err(error) => return Err(Fail::user(error)),
            }
        }
        self.check_can_launch(std::slice::from_ref(&account))?;
        // ControllerState keeps the vault as an Arc<Vault> (as accounts.rs, presets.rs and schedule.rs take it);
        // launching.rs's launch_accounts is written for an Rc<Vault> and needs the same type.
        let result = self.launch_accounts(vault, vec![account], target, false, delay, None, None, None, None).await?;
        if result.needs_confirm {
            log::note(&format!("Tray: account {short} not opened, its launch asks something first."));
            self.warn(&(l::t("Opening this account asks something first. Open it from Bloxkeep.") + "\n\n" + result.message.as_deref().unwrap_or("")));
        }
        Ok(())
    }

    /// A preset from the menu, as the Presets tab's Run; one that would ask first is refused as in `tray_open_account`.
    /// (C#'s `async void`: the tray spawns it.)
    pub async fn tray_run_preset(self: &Rc<Self>, id: Guid) {
        let short = id.to_string()[..8].to_string();
        if self.controller.vault.borrow().is_none() || self.auto_lock.screen_locked.get() {
            log::note(&format!("Tray: preset {short} not run (locked)."));
            return;
        }
        match self.start_preset(id, false).await {
            Ok(result) => {
                if result.needs_confirm {
                    log::note(&format!("Tray: preset {short} not run, it asks something first."));
                    self.warn(&(l::t("This preset asks something before it opens. Run it from Bloxkeep's Presets tab.") + "\n\n" + result.message.as_deref().unwrap_or("")));
                }
            }
            Err(Fail::User(m)) => {
                log::note(&format!("Tray: preset {short} not run (refused; the person was told why)."));
                self.warn(&m);
            }
            Err(e) => self.warn(&error_text(&e)),
        }
    }

    /// The vault's Multi-Roblox setting, copied for the next start (prefs' multi_roblox_at_start).
    pub fn keep_multi_at_start(on: bool) {
        {
            let mut p = prefs::lock();
            if p.multi_roblox_at_start == on {
                return;
            }
            p.multi_roblox_at_start = on;
        }
        prefs::save();
    }
}
