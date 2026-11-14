// AppController.Settings.cs: settings the page changes, one command each.
use std::rc::Rc;
use std::time::Duration;

use serde_json::Value;

use super::App;
use super::auto_lock::AUTO_LOCK_CHOICES;
use super::controller::{JsonKind, error_text, has, int32_of, long_of, opt_bool, opt_str};
use super::memory::RAM_LIMIT_CHOICES;
use super::presets::RESTART_ABOVE_CHOICES;
use super::server_hop::HOP_ABOVE_CHOICES;
use crate::core::base::{Fail, Res, Stamp};
use crate::core::{
    anti_afk, l, log, multi_instance, prefs, roblox_flags, roblox_game_settings, roblox_installs, roblox_launcher, roblox_processes, start_with_windows, themes,
    vault, win_kit,
};

/// AppController.Settings.cs declares no instance fields.
#[derive(Default)]
pub struct SettingsState {}

// ───────── Windows' own pieces the C# had from WinForms and .NET ─────────

pub const CSIDL_MYDOCUMENTS: i32 = 0x0005;
pub const CSIDL_MYPICTURES: i32 = 0x0027;
pub const CSIDL_LOCAL_APPDATA: i32 = 0x001c;

/// Environment.GetFolderPath for a CSIDL ("" when Windows has none).
pub fn special_folder(csidl: i32) -> String {
    use windows_sys::Win32::UI::Shell::SHGetFolderPathW;
    let mut buf = [0u16; 260];
    // SAFETY: `buf` holds MAX_PATH characters as SHGetFolderPathW requires; no token, no window.
    let hr = unsafe { SHGetFolderPathW(std::ptr::null_mut(), csidl, std::ptr::null_mut(), 0, buf.as_mut_ptr()) };
    if hr < 0 { String::new() } else { win_kit::from_wide(&buf) }
}

/// A WinForms filter ("Name (*.a)|*.a|…") as OPENFILENAME's: pairs ended by NULs, the list by a second NUL.
fn dialog_filter(filter: &str) -> Vec<u16> {
    let mut out: Vec<u16> = Vec::new();
    for part in filter.split('|') {
        out.extend(part.encode_utf16());
        out.push(0);
    }
    out.push(0);
    out
}

/// The first pattern's extension of a filter ("*.bloxkeeptheme" → "bloxkeeptheme"), for SaveFileDialog's AddExtension.
fn default_extension(filter: &str) -> Option<String> {
    let pattern = filter.split('|').nth(1)?.split(';').next()?;
    let ext = pattern.strip_prefix("*.")?;
    if ext.is_empty() || ext.contains('*') { None } else { Some(ext.to_string()) }
}

fn file_dialog(owner: isize, title: &str, filter: &str, file_name: &str, initial_dir: &str, save: bool) -> Option<String> {
    use windows_sys::Win32::UI::Controls::Dialogs::{
        GetOpenFileNameW, GetSaveFileNameW, OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_HIDEREADONLY, OFN_NOCHANGEDIR, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST, OPENFILENAMEW,
    };
    let filter_w = dialog_filter(filter);
    let title_w = win_kit::wide(title);
    let dir_w = win_kit::wide(initial_dir);
    let ext_w = default_extension(filter).map(|e| win_kit::wide(&e));
    let mut file = vec![0u16; 32 * 1024];
    for (i, ch) in file_name.encode_utf16().take(file.len() - 1).enumerate() {
        file[i] = ch;
    }
    // SAFETY: OPENFILENAMEW is plain data, zeroed then filled; every buffer it points to lives until the call returns.
    unsafe {
        let mut ofn: OPENFILENAMEW = std::mem::zeroed();
        ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
        ofn.hwndOwner = win_kit::hwnd(owner);
        ofn.lpstrFilter = filter_w.as_ptr();
        ofn.nFilterIndex = 1;
        ofn.lpstrFile = file.as_mut_ptr();
        ofn.nMaxFile = file.len() as u32;
        ofn.lpstrTitle = title_w.as_ptr();
        if !initial_dir.is_empty() {
            ofn.lpstrInitialDir = dir_w.as_ptr();
        }
        if save {
            ofn.Flags = OFN_EXPLORER | OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST | OFN_HIDEREADONLY | OFN_NOCHANGEDIR;
            if let Some(ext) = &ext_w {
                ofn.lpstrDefExt = ext.as_ptr();
            }
            if GetSaveFileNameW(&mut ofn) == 0 {
                return None;
            }
        } else {
            ofn.Flags = OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_HIDEREADONLY | OFN_NOCHANGEDIR;
            if GetOpenFileNameW(&mut ofn) == 0 {
                return None;
            }
        }
    }
    Some(win_kit::from_wide(&file))
}

/// OpenFileDialog.ShowDialog(owner): the file chosen, or None when cancelled.
pub fn open_file_dialog(owner: isize, title: &str, filter: &str, initial_dir: &str) -> Option<String> {
    file_dialog(owner, title, filter, "", initial_dir, false)
}

/// SaveFileDialog.ShowDialog(owner), asking before replacing a file: the path chosen, or None when cancelled.
pub fn save_file_dialog(owner: isize, title: &str, filter: &str, file_name: &str, initial_dir: &str) -> Option<String> {
    file_dialog(owner, title, filter, file_name, initial_dir, true)
}

/// Process.Start(new ProcessStartInfo(target) { UseShellExecute = true }): a link or file opened as Explorer would.
pub fn shell_open(target: &str) -> Res<()> {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    let file = win_kit::wide(target);
    // SAFETY: `file` is NUL-terminated and outlives the call; no window, verb, parameters or folder.
    let result = unsafe { ShellExecuteW(std::ptr::null_mut(), std::ptr::null(), file.as_ptr(), std::ptr::null(), std::ptr::null(), 1) };
    let code = result as isize;
    if code > 32 { Ok(()) } else { Err(Fail::other(format!("Windows couldn't open it (os error {code})"))) }
}

/// Application.ExecutablePath.
fn executable_path() -> String {
    std::env::current_exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Path.GetFullPath.
fn full_path(path: &str) -> String {
    std::path::absolute(path).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| path.to_string())
}

/// Collected while the vault's settings change, done once its lock is let go.
#[derive(Default)]
struct Effects {
    installs_dropped: bool,
    trim_reset: bool,
    split: bool,
    active_now: bool,
    multi: Option<bool>,
}

impl App {
    pub fn set_settings(&self, a: &Value) -> Res<Value> {
        // Sent while typing in the game box or on a list-look change: the page already shows it, so just remember it.
        let quick = a.as_object().is_some_and(|o| !o.is_empty() && o.keys().all(|k| matches!(k.as_str(), "lastPlaceInput" | "lastJobId" | "layout" | "sort")));
        let mut fx = Effects::default();
        // The disk scan for other launchers before the vault's lock, not under it: every other command would wait on it.
        let others: Option<Vec<String>> = opt_str(a, "launchVia").and_then(|v| roblox_launcher::other_name(&v)).map(|_| roblox_installs::find_other_bootstrappers());
        self.vault_mut(|d| {
            let s = &mut d.settings;
            if let Some(place) = opt_str(a, "lastPlaceInput") {
                s.last_place_input = place.trim().to_string();
            }
            if let Some(job) = opt_str(a, "lastJobId") {
                s.last_job_id = job.trim().to_string();
            }
            if let Some(layout) = opt_str(a, "layout").filter(|l| l == "cards" || l == "list") {
                s.layout = layout;
            }
            if let Some(sort) = opt_str(a, "sort").filter(|v| matches!(v.as_str(), "name" | "status" | "recent" | "order")) {
                s.sort = sort;
            }
            if quick {
                return;
            }
            if let Some(rename) = opt_bool(a, "renameWindows") {
                s.rename_windows = rename;
            }
            if let Some(pictures) = opt_bool(a, "windowPictures") {
                s.window_pictures = Some(pictures);
            }
            if let Some(groups) = opt_str(a, "taskbarGroups").filter(|g| matches!(g.as_str(), "roblox" | "account" | "preset")) {
                s.taskbar_groups = groups;
            }
            if let Some(window_layout) = opt_str(a, "windowLayout").filter(|w| roblox_processes::is_layout(w)) {
                s.window_layout = window_layout;
            }
            if let Some(width) = int32_of(a, "windowWidth") {
                s.window_width = width.clamp(roblox_processes::LEAST_WINDOW.width, roblox_processes::MOST_WINDOW.width);
            }
            if let Some(height) = int32_of(a, "windowHeight") {
                s.window_height = height.clamp(roblox_processes::LEAST_WINDOW.height, roblox_processes::MOST_WINDOW.height);
            }
            // "custom" needs a launcher picked with PickLauncher; the page never supplies a path itself.
            // "other:<name>" only for a launcher actually found here, so the page can't name an arbitrary program.
            if let Some(via) = opt_str(a, "launchVia") {
                let ok = roblox_launcher::is_valid_option(&via)
                    && (via != "custom" || roblox_launcher::custom_launcher_ready(&s.custom_launcher))
                    && roblox_launcher::other_name(&via).is_none_or(|other| others.as_ref().is_some_and(|o| o.contains(&other)));
                if ok {
                    s.launch_via = via;
                    fx.installs_dropped = true; // the client (and its version) depends on the option
                } else {
                    let why = if via == "custom" {
                        "the custom launcher isn't ready"
                    } else if roblox_launcher::is_valid_option(&via) {
                        "that launcher isn't found on this PC"
                    } else {
                        "unknown option"
                    };
                    log::note(&format!("Settings: launch option not changed ({why})."));
                }
            }
            if let Some(warnings) = opt_bool(a, "memoryWarnings") {
                s.memory_warnings = warnings;
            }
            if let Some(anti_afk) = opt_bool(a, "antiAfk") {
                s.anti_afk = anti_afk;
            }
            if has(a, "robloxConfig", JsonKind::Object) {
                let rc = &a["robloxConfig"];
                let c = &mut s.roblox_config;
                if let Some(mode) = opt_str(rc, "mode").filter(|m| matches!(m.as_str(), "off" | "all" | "marked")) {
                    c.mode = mode;
                }
                if has(rc, "graphics", JsonKind::Number) {
                    c.graphics = long_of(rc, "graphics").clamp(0, 10) as i32;
                }
                if has(rc, "fps", JsonKind::Number) {
                    let fps = i32::try_from(long_of(rc, "fps")).unwrap_or(-1);
                    c.fps = if roblox_game_settings::FPS_CHOICES.contains(&fps) { fps } else { 0 };
                }
                if has(rc, "volume", JsonKind::Number) {
                    c.volume = long_of(rc, "volume").clamp(-1, 100) as i32;
                }
            }
            if let Some(trim) = opt_bool(a, "trimBackground") {
                s.trim_background = trim;
                fx.trim_reset = true;
            }
            if let Some(auto_update) = opt_bool(a, "autoUpdateRoblox") {
                s.auto_update_roblox = auto_update;
            }
            if let Some(when) = opt_str(a, "robloxUpdateWhen").filter(|w| w == "idle" || w == "now" || w == "keep") {
                s.roblox_update_when = when;
            }
            if let Some(fast_flags) = opt_bool(a, "fastFlags") {
                s.fast_flags = fast_flags;
            }
            if has(a, "hopAbovePlayers", JsonKind::Number)
                && let Ok(v) = i32::try_from(long_of(a, "hopAbovePlayers"))
                && HOP_ABOVE_CHOICES.contains(&v)
            {
                s.hop_above_players = v;
            }
            if has(a, "ramLimitMb", JsonKind::Number)
                && let Ok(v) = i32::try_from(long_of(a, "ramLimitMb"))
                && RAM_LIMIT_CHOICES.contains(&v)
            {
                s.ram_limit_mb = v;
            }
            if has(a, "restartAboveMb", JsonKind::Number)
                && let Ok(v) = i32::try_from(long_of(a, "restartAboveMb"))
                && RESTART_ABOVE_CHOICES.contains(&v)
            {
                s.restart_above_mb = v;
            }
            if let Some(split) = opt_bool(a, "splitCores") {
                s.split_cores = split;
                fx.split = true; // at once, not at the next scan
            }
            if has(a, "launchDelaySeconds", JsonKind::Number) {
                s.launch_delay_seconds = long_of(a, "launchDelaySeconds").clamp(0, 120) as i32;
            }
            // Only the choices Settings offers; choosing counts as using the PC (the wait starts from now).
            if has(a, "autoLockMinutes", JsonKind::Number) && let Ok(minutes) = i32::try_from(long_of(a, "autoLockMinutes"))
                && AUTO_LOCK_CHOICES.contains(&minutes)
            {
                s.auto_lock_minutes = minutes;
                fx.active_now = true;
            }
            if let Some(multi) = opt_bool(a, "multiInstance").filter(|m| *m != s.multi_instance) {
                s.multi_instance = multi;
                fx.multi = Some(multi);
            }
            // Both take effect at the next launch; the person's own fullscreen setting is put back by the process check.
            if let Some(windowed) = opt_bool(a, "multiWindowed") {
                s.multi_instance_windowed = windowed;
            }
            if let Some(tile) = opt_bool(a, "multiTile") {
                s.multi_instance_tile = tile;
                if tile {
                    s.maximize_on_open = false;
                }
            }
            if let Some(maximize) = opt_bool(a, "maximizeOnOpen") {
                s.maximize_on_open = maximize;
                if maximize {
                    s.multi_instance_tile = false;
                }
            }
        })?;
        if quick {
            self.persist_soon();
            return Ok(Value::Null);
        }
        if fx.installs_dropped {
            *self.installs.installs.borrow_mut() = None;
        }
        if fx.trim_reset {
            self.controller.trim_refused_shown.set(false);
        }
        if fx.split {
            self.cores.cores_refused_shown.set(false);
            let pids = self.cores.core_pids.borrow().clone();
            self.split_cores(&pids);
        }
        if fx.active_now {
            self.auto_lock.active_at.set(Stamp::utc_now());
        }
        if let Some(multi) = fx.multi {
            if multi {
                multi_instance::enable();
            } else {
                multi_instance::disable();
            }
            self.controller.last_armed.set(multi_instance::armed());
            log::note(&format!("Settings: Multi-Roblox turned {} (armed now: {}).", if multi { "on" } else { "off" }, if multi_instance::armed() { "True" } else { "False" }));
            App::keep_multi_at_start(multi);
        }
        self.persist();
        self.push_state();
        Ok(Value::Null)
    }

    /// The page is replaced while this runs, so its reply may never arrive; the new page reads the state.
    pub async fn set_rendering(self: &Rc<Self>, gpu: bool) -> Value {
        let set = self.controller.set_safe_rendering.borrow().clone();
        if let Some(set) = set
            && gpu == prefs::lock().safe_rendering
        {
            set(!gpu).await;
            // The switch shows the saved choice.
            self.push_state();
        }
        Value::Null
    }

    pub fn disable_tray_autostart(&self) -> Res<Value> {
        self.controller.registry.set(None);
        if let Some(tray) = multi_instance::find_tray_autostart().ok().flatten().filter(|t| t.enabled)
            && let Err(e) = multi_instance::disable_tray_autostart(&tray)
        {
            log::note("Settings: another tray app's autostart couldn't be turned off (Exception).");
            return Err(Fail::user(l::f("Couldn't turn it off: {0}", &[&error_text(&e)])));
        }
        self.push_state();
        Ok(Value::Null)
    }

    pub fn open_url(url: &str) -> Res<Value> {
        shell_open(url)?;
        Ok(Value::Null)
    }

    /// "Choose a launcher…": started with the roblox-player: link as its first argument, as Windows does for the link's handler.
    pub async fn pick_launcher(&self) -> Res<Value> {
        let current = self.vault(|d| d.settings.custom_launcher.clone())?;
        let token = self.vault_token();
        let owner = self.controller.owner.get();
        let title = l::t("Choose the program that opens Roblox");
        let filter = l::t("Programs") + " (*.exe)|*.exe";
        let initial = if roblox_launcher::custom_launcher_ready(&current) {
            std::path::Path::new(&current).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
        } else {
            special_folder(CSIDL_LOCAL_APPDATA)
        };
        // On a worker thread, so the core keeps answering (the window's Quit, timers) while the dialog is open.
        let chosen = crate::core::rt::blocking(move || crate::core::ui::modal(|| open_file_dialog(owner, &title, &filter, &initial))).await;
        if let Some(chosen) = chosen {
            // The core kept running while the box was open: locked, or another vault opened, the program isn't put
            // into a vault it wasn't chosen for.
            if self.vault_token() != token {
                return Err(Fail::user(l::t("Bloxkeep was locked.")));
            }
            if full_path(&chosen).to_lowercase() == full_path(&executable_path()).to_lowercase() {
                return Err(Fail::user(l::t("Pick the program that opens Roblox, not Bloxkeep itself.")));
            }
            if !roblox_launcher::is_local_program(&chosen) {
                return Err(Fail::user(l::t("Pick a launcher on this PC: Bloxkeep doesn't open programs from a network folder.")));
            }
            self.vault_mut(|d| {
                d.settings.custom_launcher = chosen.clone();
                d.settings.launch_via = "custom".into();
            })?;
            *self.installs.installs.borrow_mut() = None;
            self.persist();
        }
        // Also when cancelled: the page put its list back on the previous choice and waits for this.
        self.push_state();
        Ok(Value::Null)
    }

    pub fn log_path() -> String {
        vault::data_dir().join("error.log").to_string_lossy().into_owned()
    }

    /// The folder Bloxkeep.exe runs from (an installed copy's current version, or a portable copy's own folder).
    pub fn app_folder() -> String {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_string_lossy().into_owned()))
            .map(|d| d.trim_end_matches(['\\', '/']).to_string())
            .unwrap_or_default()
    }

    /// Settings' Show buttons: the vault file or error log selected in Explorer (its folder if not there yet), or the
    /// program's folder. The page never names a path to open.
    pub fn show_path(&self, which: &str) -> Res<Value> {
        use std::os::windows::process::CommandExt;
        let file = match which {
            "vault" => self.controller.vault.borrow().as_ref().map(|v| v.file_path().to_string_lossy().into_owned()),
            "log" => Some(App::log_path()),
            _ => None,
        };
        let mut start = std::process::Command::new("explorer.exe");
        if let Some(file) = file.filter(|f| std::path::Path::new(f).is_file()) {
            start.raw_arg(format!("/select,\"{file}\""));
        } else if which == "vault" || which == "log" {
            start.arg(vault::data_dir());
        } else if which == "app" {
            start.arg(App::app_folder());
        } else {
            return Ok(Value::Null);
        }
        start.spawn()?;
        Ok(Value::Null)
    }

    pub fn open_data_folder() -> Res<Value> {
        std::fs::create_dir_all(vault::data_dir())?;
        std::process::Command::new("explorer.exe").arg(vault::data_dir()).spawn()?;
        Ok(Value::Null)
    }

    pub fn dispose(self: &Rc<Self>) {
        if self.controller.disposed.get() {
            return;
        }
        self.controller.disposed.set(true);
        // Each step on its own: one that fails is logged and the rest still run, so the save, the update's install and
        // the vault's closing never depend on the others (review 2026-11-03). The save comes first for the same reason.
        // A step that panics is caught (logged) and the rest still run, as the C#'s Safe(…) did for a thrown exception.
        fn step<R>(f: impl FnOnce() -> R) {
            if crate::core::program::catch_unwind(std::panic::AssertUnwindSafe(f)).is_err() {
                log::note("A closing step failed (see above); the rest still run.");
            }
        }
        step(|| self.save_before_closing()); // normally done already, before the window agreed to close
        step(|| self.give_cores_back());
        step(|| self.lift_ram_limits());
        step(|| self.give_memory_priority_back()); // no Roblox window stays at low RAM priority without Bloxkeep to raise it again
        // A launch cut short: the next Roblox opened by hand gets the person's own file. Also one that couldn't be put
        // back earlier (held, read-only): still noted, tried once more.
        step(|| {
            if self.controller.roblox_flags_pending.get() || roblox_flags::pending() {
                roblox_flags::restore();
            }
        });
        // Quit within seconds of a launch (or during one): the process check never put the person's own in-game settings
        // back, so their next Roblox would start windowed, muted, at Light AFK's FPS (2026-10-30 review). With Roblox
        // still open the copy is kept, for the next start to finish the job. Also one that couldn't be written earlier.
        step(|| {
            if self.controller.roblox_settings_pending.get() || roblox_game_settings::has_baseline() {
                roblox_game_settings::restore(roblox_processes::get_player_pids().is_empty());
            }
        });
        step(|| self.stop_watching_drags());
        step(|| self.give_icons_back());
        // A macro's held keys and buttons let go now, before the windows go back: its own endings never run once this
        // thread closes, and W stayed down until the person pressed it (B5).
        step(|| {
            if let Some(player) = self.macros.macro_player.borrow().as_ref() {
                player.release_all("closing");
            }
        });
        // Quitting during a nudge or a /pc screenshot left that window see-through and click-through until Bloxkeep ran
        // again (review 2026-10-30). Not waited for: both go on on this thread, which is closing; one still running
        // stops at its next window (disposed) and puts back only what it changed.
        step(|| {
            let windows: Vec<isize> = roblox_processes::visible_windows(&roblox_processes::get_player_pids()).into_values().flatten().collect();
            anti_afk::recover(&windows);
        });
        // PORT: SystemEvents/NetworkChange come from the window and the network watcher (see `init`); the updater's
        // report is unhooked.
        step(|| self.updates.updater.borrow().set_on_changed(None));
        step(|| self.discord_presence.discord_presence.dispose()); // the profile stops showing Bloxkeep
        step(|| {
            if let Some(input) = self.anti_afk.roblox_input.borrow_mut().as_mut() {
                input.dispose();
            }
        });
        step(|| self.say_offline_on_exit()); // no longer counted as open (not for an update in the background, back within seconds)
        step(|| self.stop_remote()); // Discord says the PC isn't connected
        step(|| {
            if let Some(feeds) = self.feeds.feeds.borrow().as_ref() {
                feeds.dispose(); // no more game news
            }
        });
        // _procTimer.Dispose(): the timer's loop ends at its next turn (disposed).
        step(|| {
            let home = self.tray.home_session.borrow_mut().take();
            if let Some(mut home) = home {
                home.dispose();
            }
        });
        step(|| self.save_before_closing()); // anything the steps above changed
        step(|| {
            if let Some(timer) = self.backup.backup_timer.borrow_mut().take() {
                timer.cancel();
            }
        });
        step(|| {
            let _ = self.drop_vault_import(false);
        });
        step(|| self.close_websites(None));
        // A downloaded update installs once the app has closed (started again at once when this quit is the background update).
        let relaunch_args = self.relaunch.relaunch_args.borrow().clone();
        step(|| self.updates.updater.borrow().apply_on_exit(relaunch_args.as_deref()));
        step(crate::core::ui::clear_secret_clipboard);
        if let Some(c) = self.controller.launch_cts.borrow().as_ref() {
            c.cancel();
        }
        if let Some(c) = self.controller.rejoin_cts.borrow().as_ref() {
            c.cancel();
        }
        // The Discord status message says Bloxkeep closed (if Discord answers quickly), unless this quit is a background update.
        step(|| {
            if relaunch_args.is_none()
                && let Some(sent) = self.final_discord_board(true)
            {
                let _ = sent.recv_timeout(Duration::from_secs(2)); // best effort
            }
        });
        let vault = self.controller.vault.borrow_mut().take();
        if let Some(v) = vault {
            step(|| v.dispose());
        }
    }

    pub fn set_language(&self, language: &str) -> Value {
        l::set(language);
        self.push_state();
        Value::Null
    }

    pub fn set_theme(&self, theme: &str) -> Value {
        prefs::lock().theme = themes::normalize(Some(theme));
        prefs::save();
        let changed = self.controller.appearance_changed.borrow().clone();
        if let Some(changed) = changed {
            changed();
        }
        self.push_state();
        Value::Null
    }

    pub fn control_window(&self, action: &str) -> Res<Value> {
        if !matches!(action, "close" | "minimize" | "zoom") {
            return Err(Fail::user(l::t("Unknown window action.")));
        }
        let control = self.relaunch.window_control.borrow().clone();
        if let Some(control) = control {
            control(action);
        }
        Ok(Value::Null)
    }

    pub fn set_hotkeys(&self, enabled: bool) -> Value {
        prefs::lock().global_hotkeys = enabled;
        prefs::save();
        let changed = self.hotkeys.hotkeys_changed.borrow().clone();
        if let Some(changed) = changed {
            changed();
        }
        self.push_state();
        Value::Null
    }

    pub fn set_start_with_windows(&self, enabled: bool) -> Res<Value> {
        start_with_windows::set(enabled)?;
        self.push_state();
        Ok(Value::Null)
    }

    pub fn set_close_to_tray(&self, enabled: bool) -> Value {
        prefs::lock().close_to_tray = enabled;
        prefs::save();
        let changed = self.controller.appearance_changed.borrow().clone();
        if let Some(changed) = changed {
            changed();
        }
        self.push_state();
        Value::Null
    }
}
