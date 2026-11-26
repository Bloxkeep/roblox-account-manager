//! Updates installed in the background (see crate::core::relaunch): once one is downloaded and nobody is using
//! Bloxkeep, the app quits, Velopack installs and restarts it, and the new run carries on where this one stopped.
//! (AppController.Relaunch.cs)
use std::{
    cell::{Cell, RefCell},
    io::{Read, Seek, SeekFrom},
    rc::Rc,
    time::Duration,
};

use base64::{Engine, engine::general_purpose::STANDARD as B64};
use serde_json::Value;

use super::{
    App,
    launching::{min_stamp, pending_count, secs, vault_now},
    schedule::{Wall, next_run},
};
use crate::core::{
    anti_afk, app_updater,
    base::{Fail, Res, Stamp, new_guid},
    l, log, prefs,
    relaunch::{self, State},
    rt, ui,
    vault::{self, Vault},
    whats_new,
};

/// Not this soon after a start (so a restart can never loop), nor while the PC was used this recently with the app on screen.
pub const RELAUNCH_AFTER_START: Duration = Duration::from_secs(3 * 60);
pub const RELAUNCH_IDLE: Duration = Duration::from_secs(10 * 60);
pub const REJOIN_SOON: Duration = Duration::from_secs(30);
/// A scheduled preset starts within this, or its minute is on and it hasn't run yet: a restart now could carry it past
/// its minute, and a missed minute isn't caught up (a restart at 03:00:10 skipped the 03:00 preset without a word,
/// 2026-10-30 review).
pub const RELAUNCH_BEFORE_SCHEDULE: Duration = Duration::from_secs(3 * 60);

/// (window: "hidden" (in the tray), "minimized" or "shown"; in front; the mini window shown).
pub type WindowNow = Rc<dyn Fn() -> (String, bool, bool)>;

pub struct RelaunchState {
    /// Closes the app for real (the window's Quit), for the update's restart.
    pub quit_for_update: RefCell<Option<Rc<dyn Fn()>>>,
    /// The macOS themes' buttons: "close" (as the title bar's X, so to the tray when that is on), "minimize", "zoom".
    pub window_control: RefCell<Option<Rc<dyn Fn(&str)>>>,
    pub window_now: RefCell<Option<WindowNow>>,
    /// PORT: Application.OpenForms (another window of the app shown: the sign-in browser, a captcha), other than the main
    /// window, the mini window and a panel shown by itself. The shell sets it; None: every visible top-level window of
    /// this process but the main one counts (`other_windows_shown`).
    pub other_windows_open: RefCell<Option<Rc<dyn Fn() -> bool>>>,
    pub relaunch_args: RefCell<Option<Vec<String>>>,
    pub relaunching: Cell<bool>,
    /// Moved back with the clock (see track_processes), like every other start time.
    pub started: Cell<Stamp>,
    /// After an update that didn't take: no new try by itself for a day, so a failing install can't loop every few
    /// minutes. The button still works.
    pub auto_relaunch_paused_until: Cell<Stamp>,
    pub resumed_from: RefCell<Option<State>>,
    pub update_failed_notice: RefCell<Option<String>>,
    /// The lock screen's answer (see Lock.tsx): true, it gave the update its place (nothing typed); false, it kept the
    /// form (something typed, or its link clicked); None, not answered. An unlock or a failed download clears it.
    pub update_at_lock: Cell<Option<bool>>,
    /// Why a ready update last waited, as logged.
    pub update_held_why: RefCell<Option<String>>,
}

impl Default for RelaunchState {
    fn default() -> RelaunchState {
        RelaunchState {
            quit_for_update: RefCell::new(None),
            window_control: RefCell::new(None),
            window_now: RefCell::new(None),
            other_windows_open: RefCell::new(None),
            relaunch_args: RefCell::new(None),
            relaunching: Cell::new(false),
            started: Cell::new(Stamp::utc_now()),
            auto_relaunch_paused_until: Cell::new(min_stamp()),
            resumed_from: RefCell::new(None),
            update_failed_notice: RefCell::new(None),
            update_at_lock: Cell::new(None),
            update_held_why: RefCell::new(None),
        }
    }
}

/// Any visible top-level window of this process other than `owner` (Application.OpenForms' visible forms, roughly).
fn other_windows_shown(owner: isize) -> bool {
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM},
        UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId, IsWindowVisible},
    };
    struct Look {
        owner: isize,
        found: bool,
    }
    unsafe extern "system" fn each(hwnd: HWND, data: LPARAM) -> i32 {
        // SAFETY: `data` is the Look below, alive for the whole enumeration; the calls only read the window.
        unsafe {
            let look = &mut *(data as *mut Look);
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &mut pid);
            // Not a tool window (the mini window, a panel, an overlay, the tray's menu: web_tool_window makes them so) nor
            // a click-through one (anti-AFK's unseen window): the C# skipped the mini window and a panel shown by
            // itself, and with them counted the update waited for a quiet moment forever (review 2026-11-13). A
            // dialog is ui::modals_open's.
            const SKIP: i32 = (windows_sys::Win32::UI::WindowsAndMessaging::WS_EX_TOOLWINDOW | windows_sys::Win32::UI::WindowsAndMessaging::WS_EX_TRANSPARENT) as i32;
            let ex = windows_sys::Win32::UI::WindowsAndMessaging::GetWindowLongW(hwnd, windows_sys::Win32::UI::WindowsAndMessaging::GWL_EXSTYLE);
            if pid == std::process::id() && hwnd as isize != look.owner && IsWindowVisible(hwnd) != 0 && ex & SKIP == 0 {
                look.found = true;
                return 0;
            }
        }
        1
    }
    let mut look = Look { owner, found: false };
    // SAFETY: the callback reads `look` only during the call.
    unsafe { EnumWindows(Some(each), &mut look as *mut Look as LPARAM) };
    look.found
}

/// Velopack's log since the newest "Applying package" says the folder was in use: a program Bloxkeep started before
/// 1.0.6 was still running "in" it.
fn update_folder_was_busy() -> bool {
    let Some(local) = std::env::var_os("LOCALAPPDATA") else {
        return false;
    };
    let path = std::path::Path::new(&local).join("velopack").join("velopack_BloxkeepApp.log");
    // FileShare.ReadWrite | Delete: std opens with all sharing on Windows.
    let read = (|| -> std::io::Result<String> {
        let mut file = std::fs::File::open(&path)?;
        let length = file.metadata()?.len();
        file.seek(SeekFrom::Start(length.saturating_sub(64_000)))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    })();
    match read {
        Ok(tail) => match tail.rfind("Applying package") {
            Some(apply) => tail[apply..].contains("used by another process"),
            None => false,
        },
        // IOException or UnauthorizedAccessException.
        Err(_) => false,
    }
}

impl App {
    fn window_now(&self) -> (String, bool, bool) {
        let f = self.relaunch.window_now.borrow().clone();
        match f {
            Some(f) => f(),
            None => ("hidden".into(), false, false),
        }
    }

    /// An update found before the vault is opened installs there and then (as other apps update when opened); the lock
    /// screen offers its place while it downloads. Found after the unlock, it waits for a quiet moment instead of
    /// cutting in.
    pub fn update_offers_at_lock(&self) -> bool {
        let status = self.updates.updater.borrow().status();
        vault_now(self).is_none()
            && !self.auto_lock.screen_locked.get()
            && prefs::lock().check_updates
            && matches!(status.as_str(), "downloading" | "ready")
            && Stamp::utc_now() >= self.relaunch.auto_relaunch_paused_until.get()
            && !self.relaunch.relaunching.get()
            && !self.updates.applying_update.get()
    }

    pub fn set_update_at_lock(&self, go: bool) -> Value {
        self.relaunch.update_at_lock.set(if self.update_offers_at_lock() { Some(go) } else { None });
        self.push_state();
        Value::Null
    }

    /// Vault closed, and the screen given to the update or the window out of sight: nobody is in the app, so a
    /// downloaded update goes at once.
    pub(crate) fn update_goes_at_lock(&self) -> bool {
        if vault_now(self).is_some() || self.auto_lock.screen_locked.get() {
            return false;
        }
        let (window, _, mini) = self.window_now();
        self.relaunch.update_at_lock.get() == Some(true) || (window != "shown" && !mini)
    }

    pub fn relaunch_if_quiet(self: &Rc<Self>, now: Stamp) {
        let at_lock = self.update_goes_at_lock();
        let status = self.updates.updater.borrow().status();
        let held_for_installer = app_updater::HELD_FOR_INSTALLER.load(std::sync::atomic::Ordering::SeqCst);
        let not_ready = self.relaunch.relaunching.get()
            || self.updates.applying_update.get()
            || self.controller.disposed.get()
            || !prefs::lock().check_updates
            || status != "ready";
        let paused = now < self.relaunch.auto_relaunch_paused_until.get();
        if not_ready || held_for_installer || paused || !self.quiet_for_relaunch(now, at_lock) {
            // A downloaded update waiting: why, logged when that changes.
            let why = if not_ready {
                None
            } else if held_for_installer {
                Some("held for the installer".to_string())
            } else if paused {
                Some("paused a day after a failed install".to_string())
            } else {
                Some(self.update_held_why(now, at_lock))
            };
            self.note_update_held(why);
            return;
        }
        self.note_update_held(None);
        let app = self.clone();
        rt::spawn(async move { app.relaunch(at_lock).await });
    }

    fn note_update_held(&self, why: Option<String>) {
        if why == *self.relaunch.update_held_why.borrow() {
            return;
        }
        *self.relaunch.update_held_why.borrow_mut() = why.clone();
        if let Some(why) = why {
            let version = self.updates.updater.borrow().new_version().unwrap_or_default();
            log::note(&format!("Update {version} ready, waits: {why}."));
        }
    }

    /// Something of the app's under way that a restart would lose (shared by quiet_for_relaunch and update_held_why).
    fn app_busy(&self) -> bool {
        self.accounts.quick_login.borrow().is_some()
            || self.vault_import.vault_import_opening.get()
            || self.vault_import.vault_import_path.borrow().is_some()
            || self.backup_writing()
            || self.health.sign_in_check_running.get()
            || self.discord.sending.get()
            || self.persist.save_failed.get()
            || self.page_busy()
            || ui::modals_open() > 0
    }

    /// Another window of the app (the sign-in browser, a captcha) means someone is in the middle of it.
    fn other_window_open(&self) -> bool {
        let f = self.relaunch.other_windows_open.borrow().clone();
        match f {
            Some(f) => f(),
            None => other_windows_shown(self.controller.owner.get()),
        }
    }

    fn busy_with_roblox(&self, now: Stamp) -> bool {
        self.opening_roblox()
            || self.close_desk.reopening()
            || self.windows_in_turn()
            || self.macros_playing()
            || now < self.controller.sweep_background_until.get()
            || self.schedule_due_soon()
    }

    /// For the log: the first reason quiet_for_relaunch says no, read the same way.
    fn update_held_why(&self, now: Stamp, at_lock: bool) -> String {
        if !at_lock && now.secs_since(self.relaunch.started.get()) < secs(RELAUNCH_AFTER_START) {
            return "just started".into();
        }
        if self.settling() || self.home_sessions_active() {
            return if self.settling() { "the PC just woke" } else { "a Roblox sign-in window is open" }.into();
        }
        if self.busy_with_roblox(now) {
            return if self.opening_roblox() {
                "Roblox opening or rejoining"
            } else if self.close_desk.reopening() {
                "a restart under way"
            } else if self.windows_in_turn() || self.macros_playing() {
                "a macro or window turn"
            } else if self.schedule_due_soon() {
                "a scheduled preset due soon"
            } else {
                "Roblox closing"
            }
            .into();
        }
        if self.app_busy() {
            return if self.persist.save_failed.get() { "the vault couldn't be saved" } else { "something in the app under way" }.into();
        }
        if self.other_window_open() {
            return "another Bloxkeep window open".into();
        }
        "Bloxkeep in use".into()
    }

    /// Nobody is using Bloxkeep and nothing a restart would lose is under way. Running Roblox windows don't count: they
    /// keep running and are followed again after.
    /// `at_lock`: see update_goes_at_lock: no wait after the start, and nobody's use holds the update back.
    pub(crate) fn quiet_for_relaunch(&self, now: Stamp, at_lock: bool) -> bool {
        if (!at_lock && now.secs_since(self.relaunch.started.get()) < secs(RELAUNCH_AFTER_START)) || self.settling() || self.home_sessions_active() {
            return false;
        }
        if self.busy_with_roblox(now) {
            return false;
        }
        if self.app_busy() {
            return false;
        }
        if self.other_window_open() {
            return false;
        }
        if at_lock {
            return true;
        }
        let (window, _, mini) = self.window_now();
        (window != "shown" && !mini) || anti_afk::idle_time() >= RELAUNCH_IDLE
    }

    /// A late window counts too: a restart would forget its account. A rejoin due later than REJOIN_SOON is kept with the
    /// vault and picked up after the restart: on 2026-10-21 a banned account's long rejoin wait kept "Restart to update"
    /// refused while nothing was opening.
    pub fn opening_roblox(&self) -> bool {
        let now = Stamp::utc_now();
        self.tray.opening_home.get()
            || self.controller.launch_cts.borrow().is_some()
            || pending_count(self) > 0
            || !self.launching.late.borrow().is_empty()
            || !self.launching.handed_over.borrow().is_empty()
            || self.controller.rejoin_queue.borrow().iter().any(|r| r.due.secs_since(now) < secs(REJOIN_SOON))
            || self.controller.rejoining.get()
            || self.close_desk.any_own()
    }

    pub fn schedule_due_soon(&self) -> bool {
        if vault_now(self).is_none() {
            return false;
        }
        let local = Wall::now();
        self.vault(|d| {
            d.settings.presets.iter().any(|p| next_run(p.schedule.as_ref()).is_some_and(|at| (at.secs_since(local) as f64) < secs(RELAUNCH_BEFORE_SCHEDULE)))
        })
        .unwrap_or(false)
    }

    /// `at_lock`: the "Updating" window shows (when Bloxkeep's is on screen), as for the button.
    async fn relaunch(self: Rc<Self>, at_lock: bool) {
        self.relaunch.relaunching.set(true);
        self.save_view().await;
        // Something may have started meanwhile: the next quiet moment then.
        let status = self.updates.updater.borrow().status();
        if self.controller.disposed.get()
            || !prefs::lock().check_updates
            || status != "ready"
            || at_lock != self.update_goes_at_lock()
            || !self.quiet_for_relaunch(Stamp::utc_now(), at_lock)
            || !self.save_before_closing()
        {
            log::note("Update: the restart to install was called off, something started meanwhile.");
        } else {
            // Opened to the tray (at sign-in), it updates there without a window coming up.
            let show = at_lock && self.window_now().0 == "shown";
            // catch (IOException or UnauthorizedAccessException or CryptographicException or ObjectDisposedException)
            match self.quit_into_update(show) {
                Ok(()) => {
                    self.closed_for_update().await;
                }
                Err(ex) => {
                    log::write(&ex);
                    relaunch::discard();
                    *self.relaunch.relaunch_args.borrow_mut() = None;
                }
            }
        }
        // finally
        self.relaunch.relaunching.set(self.relaunch.relaunch_args.borrow().is_some());
    }

    /// "Restart to update": the background restart at once, with Bloxkeep's own "Updating" window (Velopack's is a plain
    /// Windows dialog). Roblox windows stay open.
    pub async fn apply_update(self: &Rc<Self>) -> Res<Value> {
        if self.updates.applying_update.get() || self.relaunch.relaunching.get() {
            return Ok(Value::Null);
        }
        // The button can be older than the state (pushed before a failed re-check): say why nothing happens.
        let status = self.updates.updater.borrow().status();
        if status != "ready" {
            return Err(Fail::user(if matches!(status.as_str(), "checking" | "downloading") {
                l::t("The update is still downloading. Try again in a moment.")
            } else {
                l::t("No update is ready to install. Check for updates again.")
            }));
        }
        if self.home_sessions_active() {
            return Err(Fail::user(l::t("Close the separate Roblox sign-in windows before updating Bloxkeep.")));
        }
        if self.opening_roblox() {
            return Err(Fail::user(l::t("Wait until Bloxkeep finishes opening Roblox.")));
        }
        self.updates.applying_update.set(true);
        let tried: Res<()> = async {
            self.save_view().await;
            // Started during that moment (a scheduled preset, a rejoin): the restart would cut it off.
            if self.home_sessions_active() {
                return Err(Fail::user(l::t("Close the separate Roblox sign-in windows before updating Bloxkeep.")));
            }
            if self.opening_roblox() {
                return Err(Fail::user(l::t("Wait until Bloxkeep finishes opening Roblox.")));
            }
            if !self.save_before_closing() {
                return Err(Fail::user(l::t("Couldn't save your changes, so the update waits. Try again in a moment.")));
            }
            self.quit_into_update(true)?;
            if !self.closed_for_update().await {
                log::note("Update: Restart to update didn't restart the app.");
                return Err(Fail::user(l::t("Couldn't restart to update. It installs by itself when you quit Bloxkeep.")));
            }
            Ok(())
        }
        .await;
        let result = match tried {
            Ok(()) => Ok(()),
            // A UserError goes to the page as it is (after the finally below).
            Err(ex @ Fail::User(_)) => Err(ex),
            // catch (IOException or UnauthorizedAccessException or CryptographicException or ObjectDisposedException)
            Err(ex) => {
                log::write(&ex);
                relaunch::discard();
                *self.relaunch.relaunch_args.borrow_mut() = None;
                Ok(())
            }
        };
        // finally
        self.updates.applying_update.set(self.relaunch.relaunch_args.borrow().is_some());
        result?;
        Ok(Value::Null)
    }

    /// A page on screen says what it shows now (one in the background did so as it went there).
    async fn save_view(&self) {
        if !self.controller.page_attached.get() || self.controller.background.get() {
            return;
        }
        self.emit("saveView", Value::Null);
        rt::sleep(Duration::from_millis(700)).await;
    }

    /// Writes the hand-over and quits; Velopack then installs and starts the new version with it (see dispose:
    /// apply_on_exit). If the window refuses to close, the hand-over is removed: the key mustn't wait on disk.
    fn quit_into_update(&self, show_window: bool) -> Res<()> {
        let (window, foreground, _) = self.window_now();
        let updater = self.updates.updater.borrow().clone();
        let mut state = State {
            written_at: Stamp::utc_now(),
            from: Some(updater.current_version().to_string()),
            to: updater.new_version(),
            window: window.clone(),
            foreground,
            view: if self.open().is_some() { self.controller.page_view.borrow().clone() } else { None },
            weak_password: self.backup.weak_password.get(),
            screen_locked: self.auto_lock.screen_locked.get(),
            ..State::default()
        };
        if let Some(vault) = vault_now(self) {
            let (mut key, salt, iterations) = vault.export_key()?;
            state.key = Some(B64.encode(&key));
            state.salt = Some(B64.encode(&salt));
            state.iterations = iterations;
            // CryptographicOperations.ZeroMemory
            for b in key.iter_mut() {
                // SAFETY: a write through a valid &mut, volatile so it isn't left out.
                unsafe { std::ptr::write_volatile(b, 0) };
            }
        }
        let mut args = relaunch::write(&state)?;
        // In the background, a hand-over that comes too late starts it back in the tray (see relaunch::after_update);
        // after the click, the person is waiting for the window.
        if !show_window {
            args.push(relaunch::AFTER_UPDATE_ARG.to_string());
            args.push(updater.new_version().unwrap_or_default());
        }
        *self.relaunch.relaunch_args.borrow_mut() = Some(args);
        log::note(&format!(
            "Installing {} {}, window {window}.",
            updater.new_version().unwrap_or_default(),
            if show_window { "now (Restart to update)" } else { "in the background" }
        ));
        watch_update(updater.new_version().as_deref(), show_window);
        if let Some(c) = self.controller.rejoin_cts.borrow().as_ref() {
            c.cancel();
        }
        ui::clear_secret_clipboard();
        let quit = self.relaunch.quit_for_update.borrow().clone();
        if let Some(quit) = quit {
            quit();
        }
        Ok(())
    }

    /// quit() only posts the close, so the window goes a moment later (C#'s Close() was at once). Still open after a
    /// while (the person kept it open): the hand-over is removed, the key mustn't wait on disk.
    async fn closed_for_update(&self) -> bool {
        for _ in 0..150 {
            if self.controller.disposed.get() {
                return true;
            }
            rt::sleep(Duration::from_millis(100)).await;
        }
        if self.controller.disposed.get() {
            return true;
        }
        log::note("Update: the window didn't close to install; the hand-over was removed.");
        relaunch::discard();
        *self.relaunch.relaunch_args.borrow_mut() = None;
        false
    }

    /// Unlocks with the handed-over key and keeps the page's view. No key (too late, or started by the watch) or one that
    /// doesn't open the vault: the lock screen asks as usual.
    pub fn resume(self: &Rc<Self>, state: State) {
        *self.relaunch.resumed_from.borrow_mut() = Some(state.clone());
        self.vaults.vault_pick.set(false); // the vault that was open comes back, not the picker
        let current = self.updates.updater.borrow().current_version().to_string();
        // Still the version it was meant to leave: the install didn't take.
        if let Some(to) = state.to.as_deref().filter(|t| !t.is_empty())
            && to != current
        {
            self.relaunch.auto_relaunch_paused_until.set(Stamp::utc_now().plus_secs(24.0 * 3600.0));
            let busy = update_folder_was_busy();
            log::note(&format!(
                "The update to {to} didn't install (this is {current}{}); no new try by itself today.",
                if busy { ", its folder was in use" } else { "" }
            ));
            // A quiet fix is tried again tomorrow without a word: it has no button to click.
            *self.relaunch.update_failed_notice.borrow_mut() = if whats_new::is_quiet_update(Some(to), Some(&current)) {
                None
            } else if busy {
                Some(l::f(
                    "Bloxkeep couldn't update to {0}: a program it opened earlier (Bloxstrap, or a Roblox window) was still using its folder. Close those, then click Restart to update again.",
                    &[&to],
                ))
            } else {
                Some(l::f("Bloxkeep couldn't update to {0}. Click Restart to update to try again.", &[&to]))
            };
        }
        if let (Some(key), Some(salt)) = (state.key.as_deref(), state.salt.as_deref()) {
            // An antivirus can hold the vault a moment right after an install.
            let mut attempt = 1;
            while vault_now(self).is_none() {
                // Convert.FromBase64String throws FormatException: caught by the general catch below.
                let opened = match (B64.decode(key), B64.decode(salt)) {
                    (Ok(k), Ok(s)) => Vault::open_with_key(k, s, state.iterations),
                    _ => Err(Fail::other("FormatException: The input is not a valid Base-64 string.")),
                };
                match opened {
                    Ok(vault) => {
                        self.backup.weak_password.set(state.weak_password);
                        self.on_unlocked(std::sync::Arc::new(vault));
                        // Behind the lock screen it stays, though the key came along for what runs in the background.
                        self.auto_lock.screen_locked.set(state.screen_locked);
                        if vault_now(self).is_none() {
                            break; // PORT: on_unlocked always sets the vault in C#; never loop on one that didn't.
                        }
                    }
                    // PORT: an IOException is told by the "(os error N)" std's io errors carry (Fail::Other has no kind).
                    Err(ex) if attempt < 3 && is_io_error(&ex) => {
                        log::write(&ex);
                        std::thread::sleep(Duration::from_millis(400));
                    }
                    Err(ex) => {
                        log::write(&ex);
                        break;
                    }
                }
                attempt += 1;
            }
        }
        if self.open().is_some()
            && let Some(view) = state.view.clone()
        {
            *self.controller.page_view.borrow_mut() = Some(view);
            self.controller.resuming_page.set(true);
        }
        let how = if self.auto_lock.screen_locked.get() {
            "behind the lock screen again"
        } else if vault_now(self).is_some() {
            "unlocked again"
        } else {
            "locked"
        };
        log::note(&format!(
            "{}; {how}, window {}.",
            match &state.from {
                Some(from) => format!("Updated from {from}"),
                None => "Started again after an update".to_string(),
            },
            state.window
        ));
    }

    pub fn say_update_failed(&self) {
        let notice = self.relaunch.update_failed_notice.borrow().clone();
        let Some(notice) = notice else {
            return;
        };
        if !self.controller.page_attached.get() && !self.controller.background.get() {
            return;
        }
        *self.relaunch.update_failed_notice.borrow_mut() = None;
        self.notify(&notice, "warn", "updateFailed", crate::core::base::Guid::nil(), true, None);
    }

    /// Bloxkeep came back locked after an update (a scheduled preset can't run until unlocked); else None.
    pub fn locked_after_update(&self) -> Option<String> {
        let state = self.relaunch.resumed_from.borrow().clone()?;
        if vault_now(self).is_some() {
            return None;
        }
        let current = self.updates.updater.borrow().current_version().to_string();
        Some(match state.to.as_deref().filter(|t| !t.is_empty()) {
            Some(to) if to != current => {
                if whats_new::is_quiet_update(Some(to), Some(&current)) {
                    l::t("Bloxkeep restarted. Unlock it to carry on.")
                } else {
                    l::f("The update to {0} didn't install, so Bloxkeep started again as it was. Unlock it to carry on.", &[&to])
                }
            }
            _ => {
                if whats_new::is_quiet_update(Some(&current), state.from.as_deref()) {
                    l::t("Bloxkeep installed a small fix. Unlock it to carry on.")
                } else {
                    l::f("Bloxkeep was updated to {0}. Unlock it to carry on.", &[&current])
                }
            }
        })
    }
}

/// std's io errors, as Fail::Other carries them: "… (os error N)".
fn is_io_error(ex: &Fail) -> bool {
    matches!(ex, Fail::Other(m) if m.contains("(os error "))
}

/// The watch over the restart: the installer's program, run from a %TEMP% copy since the program folder is about to be
/// replaced. It shows "Updating Bloxkeep to x" (unless quiet); if this process doesn't quit it goes away; if Bloxkeep
/// isn't up again after a minute or so it removes the hand-over (the key) and starts Bloxkeep itself.
fn watch_update(version: Option<&str>, show_window: bool) {
    let result = (|| -> std::io::Result<()> {
        // AppContext.BaseDirectory, without its last separator.
        let app_dir = std::env::current_exe()?.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let source = app_dir.join("BloxkeepUninstall.exe");
        let root = app_dir.parent().map(|p| p.to_path_buf());
        let (true, Some(root)) = (source.is_file(), root) else {
            log::note("Update: no watch over the restart, BloxkeepUninstall.exe not found beside the app.");
            return Ok(());
        };
        let dir = std::env::temp_dir().join(format!("Bloxkeep-Updating-{}", &new_guid().simple().to_string()[..8]));
        std::fs::create_dir_all(&dir)?;
        let copy = dir.join("BloxkeepUpdating.exe");
        std::fs::copy(&source, &copy)?;
        let mut command = std::process::Command::new(&copy);
        command.current_dir(&dir);
        command.args(["--updating", version.unwrap_or(""), "--lang", &l::language(), "--wait-pid", &std::process::id().to_string()]);
        command.arg("--app").arg(root.join("Bloxkeep.exe"));
        command.arg("--data").arg(vault::data_dir());
        if !show_window {
            command.arg("--quiet");
        }
        command.spawn()?; // Process.Start(info)?.Dispose(): not waited for
        Ok(())
    })();
    // IOException or UnauthorizedAccessException or Win32Exception.
    if let Err(ex) = result {
        log::write(&ex);
    }
}
