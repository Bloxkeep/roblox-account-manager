//! Prefs.cs: settings needed before the vault is unlocked (language, theme, safe rendering), in prefs.json. Nothing here
//! is secret.
//!
//! The C# static properties live in one `State` behind `lock()`; the language has its own lock (`language()`), since
//! every sentence (`l::t`) reads it. Never call `l::t`, `vaults::current` or `save` while holding `lock()`.
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, MutexGuard, RwLock};
use std::time::Duration;

use indexmap::IndexMap;
use serde_json::{Map, Value, json};

use crate::core::base::Stamp;
use crate::core::log;
use crate::core::themes;
use crate::core::vault;
use crate::core::vaults;

fn prefs_path() -> PathBuf {
    vault::data_dir().join("prefs.json")
}

static LANGUAGE: LazyLock<RwLock<String>> = LazyLock::new(|| RwLock::new("en".into()));

/// "en", "vi" or another of lang.rs's codes.
pub fn language() -> String {
    LANGUAGE.read().unwrap_or_else(|e| e.into_inner()).clone()
}

pub fn set_language(language: &str) {
    *LANGUAGE.write().unwrap_or_else(|e| e.into_inner()) = language.to_string();
}

/// Whether the language was picked by someone (the first open's language step, the lock screen's switch, Settings),
/// not only taken from Windows: until it is, the first open asks (pages/lock.rs).
static LANGUAGE_CHOSEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn language_chosen() -> bool {
    LANGUAGE_CHOSEN.load(Ordering::Relaxed)
}

pub fn mark_language_chosen() {
    LANGUAGE_CHOSEN.store(true, Ordering::Relaxed);
}

/// Windows' display language as one of Bloxkeep's: the first of the user's display languages (Settings › Time &
/// language, in order) that Bloxkeep has ("pt-BR" → "pt"), "en" when none: what the first open starts in, before
/// anyone picks. Read as names, never through a language number (LCID): a number with no culture behind it (the C#
/// app's crash on a Portuguese keyboard, 1046, 2026-11-06) can't break it, and the keyboard's language plays no part.
pub fn system_language() -> &'static str {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Globalization::GetUserPreferredUILanguages;
        const MUI_LANGUAGE_NAME: u32 = 0x8;
        let (mut count, mut len) = (0u32, 0u32);
        // SAFETY: asked for the length first, then given a buffer that long; Windows writes names ending in 0, then 0.
        if unsafe { GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, std::ptr::null_mut(), &mut len) } != 0 && len > 0 && len < 4096 {
            let mut names = vec![0u16; len as usize];
            if unsafe { GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, names.as_mut_ptr(), &mut len) } != 0 {
                let names = String::from_utf16_lossy(&names[..(len as usize).min(names.len())]);
                if let Some(l) = names.split('\0').filter(|n| !n.is_empty()).find_map(crate::lang::by_code) {
                    return l.code;
                }
            }
        }
    }
    "en"
}

/// Settings › Appearance › Custom: page colour and accent (#rrggbb), how much colour lies over the picture (0 to
/// 90 %), the picture's or video's file in the data folder (theme-background-*.png…), and its blur (0 to 30 px).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomTheme {
    pub background: String,
    pub accent: String,
    pub fade: i32,
    pub picture: Option<String>,
    pub blur: i32,
}

/// A custom theme saved under a name (Settings › Appearance › Your themes): picked again, shared as a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedTheme {
    pub id: String,
    pub name: String,
    pub look: CustomTheme,
}

/// A vault on this PC (the lock screen's picker, `vaults`): its own file and master password. Names and pictures show
/// before anything is unlocked; `remote_key` is the one secret (hidden from Debug).
#[derive(Clone, Default)]
pub struct VaultEntry {
    pub id: String,
    /// vault.rmv for the first, vault-<id>.rmv for the others.
    pub file: String,
    pub name: String,
    /// A picture copied into the data folder's pictures (vault-picture-<id>-<time>.png…), or none: the name's first letter shows.
    pub picture: Option<String>,
    pub last_backup: Option<Stamp>,
    pub backup_pending: bool,
    /// This vault's Control from Discord (`State::remote_control`) and its secret for the bot (`State::remote_key`,
    /// encrypted for this Windows user, so not readable from the file).
    pub remote_control: bool,
    pub remote_key: Option<String>,
}

impl std::fmt::Debug for VaultEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VaultEntry")
            .field("id", &self.id)
            .field("file", &self.file)
            .field("name", &self.name)
            .field("picture", &self.picture)
            .field("last_backup", &self.last_backup)
            .field("backup_pending", &self.backup_pending)
            .field("remote_control", &self.remote_control)
            .field("remote_key", &self.remote_key.as_ref().map(|_| super::base::Redacted))
            .finish()
    }
}

/// `at`: when the build was installed (its folder's time), or first seen for the first entry. `source`: whose copy
/// ("Roblox", "Bloxstrap", …). `first`: the first build Bloxkeep saw here, not an update it saw happen.
#[derive(Clone, Debug, PartialEq)]
pub struct RobloxBuild {
    pub at: Stamp,
    pub version: String,
    pub folder: String,
    pub source: String,
    pub first: bool,
}

/// The Game news tab (AppController.Feeds.cs): games followed, how far each was read, the games seen in the
/// list already (a new one is marked), and when to be told of news. Per PC, not per vault: nothing here is personal.
#[derive(Clone, Debug, Default)]
pub struct FeedPrefs {
    pub follow: Vec<String>,
    /// A game's newest news read (its time, ms).
    pub read: IndexMap<String, i64>,
    pub seen: Vec<String>,
    pub alerts: IndexMap<String, FeedAlert>,
}

/// "off", "all" (every piece of news) or "words" (news with one of `words` in it).
#[derive(Clone, Debug)]
pub struct FeedAlert {
    pub mode: String,
    pub words: Vec<String>,
}

impl Default for FeedAlert {
    fn default() -> FeedAlert {
        FeedAlert { mode: "off".into(), words: Vec::new() }
    }
}

/// The Games tab's Wiki part (AppController.Wiki.cs): the wiki picked for each game (and games added for their
/// wiki alone), pages saved, pages seen lately. Per PC, kept here so nobody signs in anywhere to have them.
#[derive(Clone, Debug, Default)]
pub struct WikiPrefs {
    pub games: Vec<WikiPick>,
    pub saved: Vec<WikiMark>,
    pub recent: Vec<WikiMark>,
}

/// A game's wiki: `id` the game's in the tab (the bot's, or "w:" and the wiki's host for one added here), `place` its
/// Roblox place when known (for its look).
#[derive(Clone, Debug, Default)]
pub struct WikiPick {
    pub id: String,
    pub name: String,
    pub site: String,
    pub place: i64,
    pub img: Option<String>,
    pub added: bool,
}

#[derive(Clone, Debug, Default)]
pub struct WikiMark {
    pub site: String,
    pub title: String,
    pub game: String,
    pub at: i64,
}

/// Every setting in prefs.json but the language.
pub struct State {
    /// A theme id from `themes`.
    pub theme: String,
    /// The default theme's change from Dark to Sunset has been seen to here: a prefs.json from before it that still
    /// said Dark was moved to Sunset, once. Kept, so Dark picked again after that stays Dark. A file can't say
    /// whether its Dark was picked or only never changed: both are moved that once.
    pub sunset_default: bool,
    /// With Sync with Windows: the theme shown while Windows' apps are light, and while they are dark (a theme id each,
    /// never Sync with Windows or Custom; "light" and "dark" until chosen).
    pub system_light: String,
    pub system_dark: String,
    pub custom: CustomTheme,
    pub saved_themes: Vec<SavedTheme>,
    /// The close button hides the window to the notification area; the app quits from the tray icon.
    pub close_to_tray: bool,
    /// Ctrl+Alt+H (hide or show every Roblox window) and Ctrl+Alt+A (arrange them), from any app.
    pub global_hotkeys: bool,
    /// Accounts show as "Account 1", "Account 2"… instead of their names (see AppController.Names.cs).
    pub hide_names: bool,
    /// The "still running in the tray" notice was shown once.
    pub tray_hint_shown: bool,
    /// Draw the interface on the CPU instead of the graphics card.
    pub safe_rendering: bool,
    /// Look for new versions by itself (a manual check is always possible).
    pub check_updates: bool,
    /// Updates include beta versions (GitHub pre-releases), automatic or not; off until the person turns it on.
    pub beta_updates: bool,
    /// Settings › Discord › Show on my Discord profile: off until the person turns it on (AppController.DiscordPresence.cs).
    pub discord_presence: bool,
    /// Settings › Bloxkeep › Add me to the online count: on until the person turns it off (AppController.Online.cs).
    pub count_online: bool,
    /// The online count's id for this PC, made from the PC at its first beat (the same after a reinstall) and linked
    /// to nothing else (AppController.Online.cs). Forgotten when the count is turned off.
    pub online_id: Option<String>,
    /// Statistics › Most alts now: this PC's name there (None: not on it). Goes with the online count's beats, which
    /// also say how many accounts run here (AppController.Online.cs). A made-up name at the first beat until one is chosen.
    pub board_name: Option<String>,
    /// The board name was given (made up or chosen, emptied too): no made-up one again.
    pub board_name_chosen: bool,
    /// The bot was told this copy's first "open" (a new install, AppController.Online.cs): told once.
    pub first_open_told: bool,
    remote_control_pc: bool,
    remote_key_pc: Option<String>,
    /// Keys (encrypted) of links turned off or of vaults gone, until the bot has said it forgot them: told only
    /// while connected, it kept a PC nobody could reach under the person's /pc (2026-10-30 review).
    pub remote_gone: Vec<String>,
    /// The version that last ran here; a newer one means the app was updated (see `whats_new`).
    pub last_version: Option<String>,
    /// Important events reach the person as Windows notifications while the window is hidden or in the background.
    pub notifications: bool,
    /// Event kinds turned off one by one under that switch (AppController.Notify.cs). Names this build doesn't know
    /// (a newer one wrote them) are kept.
    pub notify_off: HashSet<String>,
    /// Questions answered with "Don't ask again" (see AppController.SkippableConfirms): their yes is assumed.
    pub skip_confirm: HashSet<String>,
    /// Where copies of the vault go (see `backup`); None until chosen. Kept here so restoring works before unlocking.
    pub backup_folder: Option<String>,
    pub backup_enabled: bool,
    last_backup_pc: Option<Stamp>,
    backup_pending_pc: bool,
    pub vault_list: Vec<VaultEntry>,
    /// The vault last opened or picked: the one the lock screen offers at the next start.
    pub last_vault: Option<String>,
    /// The lock screen starts on the picker, Steam-like ("Ask which vault to use each time Bloxkeep starts").
    pub ask_vault: bool,
    /// Every saved sign-in is verified once a day (see AppController.Health.cs).
    pub daily_sign_in_check: bool,
    pub last_sign_in_check: Option<Stamp>,
    /// The mini window (MiniWindow) was open when the app last ran, so it comes back at start.
    pub mini_open: bool,
    /// The mini window's last place and size, "x,y,w,h" in screen pixels; None until it was moved.
    pub mini_window: Option<String>,
    /// Each macro panel's place (PanelForm.cs) by the macro's id: "x,y" in screen pixels.
    pub panel_windows: IndexMap<String, String>,
    /// The main window's place and size when not maximised, "x,y,w,h" in screen pixels; None until it was first moved, resized or closed.
    pub main_window: Option<String>,
    pub main_maximized: bool,
    /// The Roblox client builds seen on this PC, oldest first (see AppController.RobloxUpdates.cs).
    pub roblox_history: Vec<RobloxBuild>,
    /// The last time Roblox closed windows when another one opened, how many, and the day it was told.
    pub roblox_closed_at: Option<Stamp>,
    pub roblox_closed_count: i32,
    pub roblox_closed_notified: Option<Stamp>,
    /// Roblox builds (folder names) on which two windows played together here with Multi-Roblox on, oldest first,
    /// so a Roblox watch warning about that build no longer applies.
    pub multi_worked: Vec<String>,
    /// Ids of vaults unlocked here (`Vault::id`, a hash of the salt), old ids included: a restored backup with an id
    /// not in here was made somewhere else.
    pub known_vault_ids: HashSet<String>,
    /// A vault restored from another PC's backup, until first unlocked: its Roblox launcher and Discord links are
    /// dropped then (AppController.RestoreBackup).
    pub foreign_restore: Option<String>,
    /// A copy of the last vault's Multi-Roblox switch, taken at start before the master password so Roblox windows
    /// opened any way run side by side even at the lock screen. Kept in step by AppController.
    pub multi_roblox_at_start: bool,
    /// A Roblox client seen running on this PC (its file): see roblox_installs::note_running.
    pub seen_roblox: Option<String>,
    pub game_feeds: FeedPrefs,
    pub wikis: WikiPrefs,
    /// Keys this build doesn't know (a newer one wrote them): saved back untouched rather than dropped.
    extra: IndexMap<String, Value>,
    pub(crate) unreadable: bool,
    save_failed: bool,
    save_failure_logged: bool,
}

impl Default for State {
    fn default() -> State {
        State {
            theme: themes::DEFAULT.into(),
            // (A new Bloxkeep starts on Sunset: nothing to move.)
            sunset_default: true,
            system_light: "light".into(),
            system_dark: "dark".into(),
            custom: CustomTheme { background: "#141a26".into(), accent: "#22c55e".into(), fade: 45, picture: None, blur: 0 },
            saved_themes: Vec::new(),
            close_to_tray: true,
            global_hotkeys: true,
            hide_names: false,
            tray_hint_shown: false,
            safe_rendering: false,
            check_updates: true,
            beta_updates: false,
            discord_presence: false,
            count_online: true,
            online_id: None,
            board_name: None,
            board_name_chosen: false,
            first_open_told: false,
            remote_control_pc: false,
            remote_key_pc: None,
            remote_gone: Vec::new(),
            last_version: None,
            notifications: true,
            notify_off: HashSet::new(),
            skip_confirm: HashSet::new(),
            backup_folder: None,
            backup_enabled: true,
            last_backup_pc: None,
            backup_pending_pc: false,
            vault_list: Vec::new(),
            last_vault: None,
            ask_vault: false,
            daily_sign_in_check: true,
            last_sign_in_check: None,
            mini_open: false,
            mini_window: None,
            panel_windows: IndexMap::new(),
            main_window: None,
            main_maximized: false,
            roblox_history: Vec::new(),
            roblox_closed_at: None,
            roblox_closed_count: 0,
            roblox_closed_notified: None,
            multi_worked: Vec::new(),
            known_vault_ids: HashSet::new(),
            foreign_restore: None,
            multi_roblox_at_start: false,
            seen_roblox: None,
            game_feeds: FeedPrefs::default(),
            wikis: WikiPrefs::default(),
            extra: IndexMap::new(),
            unreadable: false,
            save_failed: false,
            save_failure_logged: false,
        }
    }
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(|| Mutex::new(State::default()));

/// The settings, to read or change (then `save`). Let go of it before calling anything that takes it again.
pub fn lock() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

impl State {
    /// `Vaults.Current`'s place in `vault_list`: the vault in use, or the one the lock screen offers.
    pub fn current_index(&self) -> Option<usize> {
        let active = vault::active_file();
        self.vault_list.iter().position(|v| v.file.eq_ignore_ascii_case(&active))
    }

    /// Settings › Discord › Control from Discord: off until the person turns it on (AppController.Remote.cs). Each
    /// vault has its own (`VaultEntry::remote_control`): vaults are for different people, and one person's /pc
    /// must never run another's presets or picture their accounts (2026-10-30 review).
    pub fn remote_control(&self) -> bool {
        match self.current_index() {
            Some(i) => self.vault_list[i].remote_control,
            None => self.remote_control_pc,
        }
    }

    pub fn set_remote_control(&mut self, value: bool) {
        match self.current_index() {
            Some(i) => self.vault_list[i].remote_control = value,
            None => self.remote_control_pc = value,
        }
    }

    /// The secret the vault in use proves itself with to Bloxkeep's Discord bot (a PC of its own there), made when
    /// Control from Discord is turned on and dropped when it's turned off; encrypted for this Windows user
    /// (AppController.Remote.cs). A new one is a new PC for the bot, linked to nobody.
    pub fn remote_key(&self) -> Option<String> {
        match self.current_index() {
            Some(i) => self.vault_list[i].remote_key.clone(),
            None => self.remote_key_pc.clone(),
        }
    }

    pub fn set_remote_key(&mut self, value: Option<String>) {
        match self.current_index() {
            Some(i) => self.vault_list[i].remote_key = value,
            None => self.remote_key_pc = value,
        }
    }

    pub fn forget_remote_key(&mut self, key: Option<&str>) {
        let Some(key) = key.filter(|k| !k.is_empty()) else { return };
        if self.remote_gone.iter().any(|k| k == key) {
            return;
        }
        self.remote_gone.push(key.to_string());
        if self.remote_gone.len() > 20 {
            self.remote_gone.remove(0);
        }
    }

    /// Control from Discord as builds before 2026-10-30 kept it, one for the whole PC: it goes to the vault the lock
    /// screen offers (the one last used), unless that vault has its own already. `entry`: its place in `vault_list`.
    pub(crate) fn move_remote_to(&mut self, entry: Option<usize>) {
        let Some(i) = entry else { return };
        if !self.remote_control_pc && self.remote_key_pc.is_none() {
            return;
        }
        let e = &mut self.vault_list[i];
        if !e.remote_control && e.remote_key.is_none() {
            (e.remote_control, e.remote_key) = (self.remote_control_pc, self.remote_key_pc.clone());
        }
        (self.remote_control_pc, self.remote_key_pc) = (false, None);
    }

    /// When the vault in use (`vaults`) was last copied to the backup folder; each vault has its own.
    pub fn last_backup(&self) -> Option<Stamp> {
        match self.current_index() {
            Some(i) => self.vault_list[i].last_backup,
            None => self.last_backup_pc,
        }
    }

    pub fn set_last_backup(&mut self, value: Option<Stamp>) {
        match self.current_index() {
            Some(i) => self.vault_list[i].last_backup = value,
            None => self.last_backup_pc = value,
        }
    }

    /// A change worth a backup was saved and not copied yet; still set if the app closed first (killed, or the copy
    /// on exit took too long), so the next unlock makes it. Each vault has its own.
    pub fn backup_pending(&self) -> bool {
        match self.current_index() {
            Some(i) => self.vault_list[i].backup_pending,
            None => self.backup_pending_pc,
        }
    }

    pub fn set_backup_pending(&mut self, value: bool) {
        match self.current_index() {
            Some(i) => self.vault_list[i].backup_pending = value,
            None => self.backup_pending_pc = value,
        }
    }

    /// The last Save couldn't write prefs.json (until one does).
    pub fn save_failed(&self) -> bool {
        self.save_failed
    }
}

/// Eight lower-case hex digits (`Vault::id`).
pub fn is_vault_id(id: Option<&str>) -> bool {
    matches!(id, Some(id) if id.len() == 8 && id.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)))
}

const KNOWN: &[&str] = &[
    "remoteGone",
    "language", "languageChosen", "theme", "sunsetDefault", "themeLight", "themeDark", "closeToTray", "hideNames", "trayHintShown", "safeRendering", "checkUpdates", "betaUpdates", "lastVersion",
    "notifications", "notifyOff", "backupFolder", "backupEnabled", "lastBackup", "backupPending", "dailySignInCheck", "lastSignInCheck", "miniOpen", "miniWindow", "panelWindows",
    "mainWindow", "mainMaximized", "robloxHistory", "robloxClosedAt", "robloxClosedCount", "robloxClosedNotified", "skipConfirm", "multiWorked",
    "multiRobloxAtStart", "seenRoblox", "discordPresence", "customTheme", "vaults", "lastVault", "askVault", "countOnline", "onlineId",
    "knownVaultIds", "foreignRestore", "savedThemes", "gameFeeds", "wikis",
    // Read and written here, but not in the C#'s list: kept out of `extra`, where a stale copy (remoteKey, a secret) sat.
    "globalHotkeys", "boardName", "boardNameChosen", "firstOpenTold", "remoteControl", "remoteKey",
];

/// String length as C# counts it (UTF-16 units).
fn len16(s: &str) -> usize {
    s.encode_utf16().count()
}

fn text<'a>(e: &'a Value, name: &str) -> Option<&'a str> {
    e.get(name).and_then(Value::as_str)
}

fn is_true(e: &Value, name: &str) -> bool {
    matches!(e.get(name), Some(Value::Bool(true)))
}

fn is_false(e: &Value, name: &str) -> bool {
    matches!(e.get(name), Some(Value::Bool(false)))
}

/// TryGetInt32 on a JSON number: a whole number in int's range.
fn int32(v: &Value) -> Option<i32> {
    v.as_i64().and_then(|n| i32::try_from(n).ok())
}

/// A string property read as a date (TryGetDateTime).
fn date(e: &Value, name: &str) -> Option<Stamp> {
    e.get(name).and_then(Value::as_str).and_then(Stamp::parse)
}

/// A custom theme as saved (its colours checked, the numbers kept in range); what is missing or wrong from `fallback`.
fn read_custom(e: &Value, fallback: &CustomTheme) -> CustomTheme {
    let number = |name: &str| e.get(name).filter(|v| v.is_number()).and_then(int32);
    CustomTheme {
        background: themes::hex(text(e, "background")).unwrap_or_else(|| fallback.background.clone()),
        accent: themes::hex(text(e, "accent")).unwrap_or_else(|| fallback.accent.clone()),
        fade: number("fade").map(|f| f.clamp(0, 90)).unwrap_or(fallback.fade),
        picture: text(e, "picture").filter(|p| themes::is_theme_media_name(p)).map(String::from),
        blur: number("blur").map(|b| b.clamp(0, themes::MAX_BLUR)).unwrap_or(0),
    }
}

/// A future time (the clock was set back) is now.
fn not_future(at: Stamp) -> Stamp {
    let utc = at.to_utc();
    let now = Stamp::utc_now();
    if utc > now { now } else { utc }
}

pub fn load() {
    // (No prefs.json yet, or one without a language: Windows' own, until someone picks.)
    set_language(system_language());
    let raw = match read_prefs() {
        Ok(raw) => raw,
        Err(e) => {
            // Missing or unreadable prefs: defaults. Held by another program: nothing is known to be wrong with it, so
            // this run's defaults don't overwrite it.
            if prefs_path().exists() {
                let mut s = lock();
                s.unreadable = true;
                // What this run starts from, so a later save that can read the file again knows what changed since.
                *UNREAD_START.lock().unwrap_or_else(|e| e.into_inner()) = Some(snapshot(&s, &language()));
                drop(s);
                log::note(&format!("prefs.json couldn't be opened ({:?}); this run's settings won't be saved over it until it can be read.", e.kind()));
            }
            return;
        }
    };
    let root: Value = match serde_json::from_str(raw.trim_start_matches('\u{feff}')) {
        Ok(root) => root,
        Err(_) => {
            // One that isn't JSON any more is kept aside first.
            keep_damaged();
            return;
        }
    };
    let Some(obj) = root.as_object() else {
        keep_damaged();
        return;
    };
    let board_name = crate::core::app::App::board_name_of(text(&root, "boardName"));
    let wiki_parsed = root.get("wikis").filter(|w| w.is_object()).map(parse_wikis);
    let mut s = lock();
    for (name, value) in obj {
        if !KNOWN.contains(&name.as_str()) {
            s.extra.insert(name.clone(), value.clone());
        }
    }
    if let Some(l) = text(&root, "language").and_then(crate::lang::by_code) {
        set_language(l.code);
    }
    // (A file from before the first open asked has its language and no mark: that language was someone's pick.)
    let chosen = match root.get("languageChosen") {
        Some(Value::Bool(b)) => *b,
        _ => text(&root, "language").is_some(),
    };
    LANGUAGE_CHOSEN.store(chosen, Ordering::Relaxed);
    if let Some(theme) = text(&root, "theme") {
        s.theme = themes::normalize(Some(theme));
    }
    // A file from before Sunset became the default: its Dark, the default it was written under, becomes Sunset, this
    // once (the mark goes into the file at its next save, and is in the state from here on).
    if !is_true(&root, "sunsetDefault") && s.theme == themes::OLD_DEFAULT {
        s.theme = themes::DEFAULT.into();
        crate::core::log::note("Settings: the theme was the old default (Dark); it is Sunset now, the new default. Dark can be picked again in Settings › Appearance.");
    }
    s.sunset_default = true;
    s.system_light = themes::plain(text(&root, "themeLight"), "light");
    s.system_dark = themes::plain(text(&root, "themeDark"), "dark");
    if let Some(Value::Array(list)) = root.get("vaults") {
        for v in list {
            if !v.is_object() {
                continue;
            }
            let (Some(vid), Some(vfile)) = (text(v, "id"), text(v, "file")) else { continue };
            if !vaults::is_id(vid) || !vaults::is_file(vfile) || s.vault_list.iter().any(|e| e.id == vid || e.file == vfile) {
                continue;
            }
            let entry = VaultEntry {
                id: vid.to_string(),
                file: vfile.to_string(),
                name: vaults::clean_name(text(v, "name")).unwrap_or_else(|| "Vault".into()),
                picture: text(v, "picture").filter(|p| themes::is_picture_name(p)).map(String::from),
                last_backup: date(v, "lastBackup").map(not_future),
                backup_pending: is_true(v, "backupPending"),
                remote_control: is_true(v, "remoteControl"),
                remote_key: text(v, "remoteKey").map(String::from),
            };
            s.vault_list.push(entry);
        }
    }
    if let Some(Value::Array(gone)) = root.get("remoteGone") {
        for key in gone {
            if let Some(k) = key.as_str() {
                s.forget_remote_key(Some(k));
            }
        }
    }
    s.last_vault = text(&root, "lastVault").map(String::from);
    s.ask_vault = is_true(&root, "askVault");
    if let Some(custom) = root.get("customTheme").filter(|c| c.is_object()) {
        let fallback = s.custom.clone();
        s.custom = read_custom(custom, &fallback);
    }
    if let Some(Value::Array(saved)) = root.get("savedThemes") {
        let fallback = s.custom.clone();
        let mut list: Vec<SavedTheme> = Vec::new();
        for t in saved {
            if !t.is_object() {
                continue;
            }
            let Some(id) = text(t, "id").filter(|id| (1..=40).contains(&len16(id))) else { continue };
            let Some(name) = themes::theme_name(text(t, "name")) else { continue };
            // DistinctBy(Id): the first of an id is kept.
            if list.iter().any(|x| x.id == id) {
                continue;
            }
            list.push(SavedTheme { id: id.to_string(), name, look: read_custom(t, &fallback) });
        }
        list.truncate(themes::MAX_SAVED);
        s.saved_themes = list;
    }
    if is_false(&root, "closeToTray") {
        s.close_to_tray = false;
    }
    if is_false(&root, "globalHotkeys") {
        s.global_hotkeys = false;
    }
    if is_true(&root, "hideNames") {
        s.hide_names = true;
    }
    if is_true(&root, "trayHintShown") {
        s.tray_hint_shown = true;
    }
    if is_true(&root, "safeRendering") {
        s.safe_rendering = true;
    }
    if is_false(&root, "checkUpdates") {
        s.check_updates = false;
    }
    if is_true(&root, "betaUpdates") {
        s.beta_updates = true;
    }
    if let Some(last) = text(&root, "lastVersion") {
        s.last_version = Some(last.to_string());
    }
    s.discord_presence = is_true(&root, "discordPresence");
    if is_false(&root, "countOnline") {
        s.count_online = false;
    }
    // Guid.TryParseExact(…, "N"): 32 hex digits, no dashes.
    if let Some(online_id) = text(&root, "onlineId").filter(|id| id.len() == 32 && id.chars().all(|c| c.is_ascii_hexdigit())) {
        s.online_id = Some(online_id.to_string());
    }
    s.board_name = board_name;
    s.board_name_chosen = is_true(&root, "boardNameChosen");
    // From before each vault had its own: moved to a vault by vaults::load (move_remote_to).
    s.remote_control_pc = is_true(&root, "remoteControl");
    s.first_open_told = is_true(&root, "firstOpenTold");
    s.remote_key_pc = text(&root, "remoteKey").map(String::from);
    if is_false(&root, "notifications") {
        s.notifications = false;
    }
    if let Some(Value::Array(off)) = root.get("notifyOff") {
        for kind in off {
            if let Some(name) = kind.as_str().filter(|n| (1..=40).contains(&len16(n))) {
                s.notify_off.insert(name.to_string());
            }
        }
    }
    if let Some(Value::Array(skip)) = root.get("skipConfirm") {
        for question in skip {
            if let Some(name) = question.as_str().filter(|n| (1..=40).contains(&len16(n))) {
                s.skip_confirm.insert(name.to_string());
            }
        }
    }
    if let Some(folder) = text(&root, "backupFolder") {
        s.backup_folder = if folder.is_empty() { None } else { Some(folder.to_string()) };
    }
    if is_false(&root, "backupEnabled") {
        s.backup_enabled = false;
    }
    if let Some(at) = date(&root, "lastBackup") {
        // A time in the future (the clock was set back) would hold the next backup off for days.
        s.last_backup_pc = Some(not_future(at));
    }
    if is_true(&root, "backupPending") {
        s.backup_pending_pc = true;
    }
    if is_false(&root, "dailySignInCheck") {
        s.daily_sign_in_check = false;
    }
    if let Some(checked_at) = date(&root, "lastSignInCheck") {
        // Likewise: a time in the future would hold the next check off.
        s.last_sign_in_check = Some(not_future(checked_at));
    }
    if is_true(&root, "miniOpen") {
        s.mini_open = true;
    }
    if is_true(&root, "multiRobloxAtStart") {
        s.multi_roblox_at_start = true;
    }
    if let Some(seen) = text(&root, "seenRoblox").filter(|p| len16(p) <= 1024) {
        s.seen_roblox = Some(seen.to_string());
    }
    if let Some(mini) = text(&root, "miniWindow") {
        s.mini_window = Some(mini.to_string());
    }
    if let Some(Value::Object(panels)) = root.get("panelWindows") {
        s.panel_windows = panels.iter().filter(|(name, v)| v.is_string() && len16(name) <= 40).take(200).map(|(name, v)| (name.clone(), v.as_str().unwrap_or_default().to_string())).collect();
    }
    if let Some(main) = text(&root, "mainWindow") {
        s.main_window = Some(main.to_string());
    }
    if is_true(&root, "mainMaximized") {
        s.main_maximized = true;
    }
    if let Some(Value::Array(history)) = root.get("robloxHistory") {
        for h in history {
            if !h.is_object() {
                continue;
            }
            let Some(version) = text(h, "version").filter(|v| (1..=40).contains(&len16(v))) else { continue };
            let Some(build_folder) = text(h, "folder").filter(|f| (1..=80).contains(&len16(f))) else { continue };
            let Some(built_at) = date(h, "at") else { continue };
            s.roblox_history.push(RobloxBuild {
                at: built_at.to_utc(),
                version: version.to_string(),
                folder: build_folder.to_string(),
                source: text(h, "source").unwrap_or("").to_string(),
                first: is_true(h, "first"),
            });
        }
    }
    if let Some(closed) = date(&root, "robloxClosedAt") {
        s.roblox_closed_at = Some(closed.to_utc());
    }
    if let Some(count) = root.get("robloxClosedCount").filter(|v| v.is_number()).and_then(int32) {
        s.roblox_closed_count = count.max(0);
    }
    if let Some(notified) = date(&root, "robloxClosedNotified") {
        s.roblox_closed_notified = Some(notified.to_utc());
    }
    if let Some(Value::Array(worked)) = root.get("multiWorked") {
        for build in worked {
            if let Some(folder) = build.as_str().filter(|f| (1..=80).contains(&len16(f))) {
                s.multi_worked.push(folder.to_string());
            }
        }
    }
    if let Some(Value::Array(known)) = root.get("knownVaultIds") {
        for id in known {
            if let Some(vault_id) = id.as_str().filter(|id| is_vault_id(Some(id))) {
                s.known_vault_ids.insert(vault_id.to_string());
            }
        }
    }
    if let Some(feeds) = root.get("gameFeeds").filter(|f| f.is_object()) {
        // A shape it can't read (JsonException): nothing from it.
        s.game_feeds = parse_feeds(feeds).unwrap_or_default();
    }
    if let Some(wikis) = wiki_parsed {
        s.wikis = wikis;
    }
    if let Some(restored) = text(&root, "foreignRestore").filter(|r| is_vault_id(Some(r))) {
        s.foreign_restore = Some(restored.to_string());
    }
}

/// A property by name, any case (PropertyNameCaseInsensitive).
fn prop_ci<'a>(o: &'a Map<String, Value>, name: &str) -> Option<&'a Value> {
    o.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v)
}

/// A list of strings as System.Text.Json reads it into List<string>; Err for any other shape. A null element can't be
/// held by a Rust String and is left out.
fn strings(v: Option<&Value>) -> Result<Option<Vec<String>>, ()> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(items)) => {
            let mut out = Vec::new();
            for item in items {
                match item {
                    Value::String(s) => out.push(s.clone()),
                    Value::Null => {}
                    _ => return Err(()),
                }
            }
            Ok(Some(out))
        }
        _ => Err(()),
    }
}

fn string_prop(v: Option<&Value>) -> Result<Option<String>, ()> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        _ => Err(()),
    }
}

fn long_prop(v: Option<&Value>) -> Result<Option<i64>, ()> {
    match v {
        None => Ok(None),
        Some(Value::Number(n)) => n.as_i64().map(Some).ok_or(()),
        _ => Err(()),
    }
}

fn bool_prop(v: Option<&Value>) -> Result<Option<bool>, ()> {
    match v {
        None => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        _ => Err(()),
    }
}

/// `gameFeeds` as `Deserialize<FeedPrefs>` (case-insensitive) reads it, then tidied as Load does: Err where System.Text.Json
/// throws JsonException.
fn parse_feeds(v: &Value) -> Result<FeedPrefs, ()> {
    let o = v.as_object().ok_or(())?;
    let mut feeds = FeedPrefs { follow: strings(prop_ci(o, "follow"))?.unwrap_or_default(), seen: strings(prop_ci(o, "seen"))?.unwrap_or_default(), ..FeedPrefs::default() };
    match prop_ci(o, "read") {
        None | Some(Value::Null) => {}
        Some(Value::Object(read)) => {
            for (k, n) in read {
                feeds.read.insert(k.clone(), long_prop(Some(n))?.ok_or(())?);
            }
        }
        _ => return Err(()),
    }
    match prop_ci(o, "alerts") {
        None | Some(Value::Null) => {}
        Some(Value::Object(alerts)) => {
            for (k, a) in alerts {
                match a {
                    // Alerts with no value are dropped.
                    Value::Null => {}
                    Value::Object(ao) => {
                        let mut alert = FeedAlert::default();
                        match prop_ci(ao, "mode") {
                            None => {}
                            // PORT: C# keeps a null Mode (a Rust String can't): it reads as "" here.
                            Some(Value::Null) => alert.mode = String::new(),
                            Some(m) => alert.mode = string_prop(Some(m))?.unwrap_or_default(),
                        }
                        alert.words = strings(prop_ci(ao, "words"))?.unwrap_or_default();
                        feeds.alerts.insert(k.clone(), alert);
                    }
                    _ => return Err(()),
                }
            }
        }
        _ => return Err(()),
    }
    Ok(feeds)
}

/// `wikis` read as `Deserialize<WikiPrefs>` (case-insensitive), then only the entries with a known wiki kept; a shape it
/// can't read gives an empty one.
fn parse_wikis(v: &Value) -> WikiPrefs {
    fn picks(v: Option<&Value>) -> Result<Vec<WikiPick>, ()> {
        match v {
            None | Some(Value::Null) => Ok(Vec::new()),
            Some(Value::Array(items)) => {
                let mut out = Vec::new();
                for item in items {
                    match item {
                        Value::Null => {}
                        Value::Object(o) => out.push(WikiPick {
                            // A null Id or Site made C# throw a NullReferenceException at `.Length` / Site(): read as "".
                            id: string_prop(prop_ci(o, "id"))?.unwrap_or_default(),
                            name: string_prop(prop_ci(o, "name"))?.unwrap_or_default(),
                            site: string_prop(prop_ci(o, "site"))?.unwrap_or_default(),
                            place: long_prop(prop_ci(o, "place"))?.unwrap_or(0),
                            img: string_prop(prop_ci(o, "img"))?,
                            added: bool_prop(prop_ci(o, "added"))?.unwrap_or(false),
                        }),
                        _ => return Err(()),
                    }
                }
                Ok(out)
            }
            _ => Err(()),
        }
    }
    fn marks(v: Option<&Value>) -> Result<Vec<WikiMark>, ()> {
        match v {
            None | Some(Value::Null) => Ok(Vec::new()),
            Some(Value::Array(items)) => {
                let mut out = Vec::new();
                for item in items {
                    match item {
                        Value::Null => {}
                        Value::Object(o) => out.push(WikiMark {
                            site: string_prop(prop_ci(o, "site"))?.unwrap_or_default(),
                            title: string_prop(prop_ci(o, "title"))?.unwrap_or_default(),
                            game: string_prop(prop_ci(o, "game"))?.unwrap_or_default(),
                            at: long_prop(prop_ci(o, "at"))?.unwrap_or(0),
                        }),
                        _ => return Err(()),
                    }
                }
                Ok(out)
            }
            _ => Err(()),
        }
    }
    let parsed = (|| -> Result<WikiPrefs, ()> {
        let o = v.as_object().ok_or(())?;
        Ok(WikiPrefs { games: picks(prop_ci(o, "games"))?, saved: marks(prop_ci(o, "saved"))?, recent: marks(prop_ci(o, "recent"))? })
    })();
    let mut wikis = parsed.unwrap_or_default();
    let known = |site: &str| crate::core::wiki_net::site(Some(site)).is_some();
    wikis.games.retain(|g| !g.id.is_empty() && known(&g.site));
    wikis.saved.retain(|m| known(&m.site) && !m.title.is_empty());
    wikis.recent.retain(|m| known(&m.site) && !m.title.is_empty());
    wikis
}

/// A moment's wait for an antivirus or sync program holding the file as Bloxkeep starts.
fn read_prefs() -> std::io::Result<String> {
    let mut tries = 1;
    loop {
        match std::fs::read(prefs_path()) {
            // File.ReadAllText: UTF-8, a byte order mark dropped (by `load`), anything not UTF-8 replaced.
            Ok(bytes) => return Ok(String::from_utf8_lossy(&bytes).into_owned()),
            Err(e) if tries < 5 && e.kind() != std::io::ErrorKind::NotFound => {
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(e) => return Err(e),
        }
        tries += 1;
    }
}

/// A prefs.json that can't be read is copied to prefs.json.damaged-<time> before the next save writes the
/// defaults over it: the vault list, backup folder and the rest can be got back from it (review, 2026-10-30).
fn keep_damaged() {
    let c = Stamp::now().local();
    let mut target = prefs_path().into_os_string();
    target.push(format!(".damaged-{:04}{:02}{:02}-{:02}{:02}{:02}", c.year, c.month, c.day, c.hour, c.minute, c.second));
    let target = PathBuf::from(target);
    // Already kept this second, or the folder won't take it: the defaults apply all the same.
    if target.exists() {
        return;
    }
    if std::fs::copy(prefs_path(), &target).is_ok() {
        log::note("prefs.json couldn't be read; a copy was kept beside it and the settings start from their defaults.");
    }
}

/// The last `save` couldn't write prefs.json (until one does).
pub fn save_failed() -> bool {
    lock().save_failed
}

const SAVE_TRIES: u32 = 3;
const SAVE_RETRY_WAIT: Duration = Duration::from_millis(150);

/// Strings in .NET's ordinal order (UTF-16 units).
fn ordinal_sorted(set: &HashSet<String>) -> Vec<String> {
    let mut list: Vec<String> = set.iter().cloned().collect();
    list.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    list
}

/// Numbers each save's snapshot, taken under the prefs lock, so they count in the order the state was read.
static SNAPSHOTS: AtomicU64 = AtomicU64::new(0);
/// Writes one at a time: the newest snapshot tried, and whether it was written. An older one coming late is
/// never written over it.
static WRITTEN: Mutex<(u64, bool)> = Mutex::new((0, true));

/// The settings as this run started, when prefs.json couldn't be read then.
static UNREAD_START: Mutex<Option<Map<String, Value>>> = Mutex::new(None);

/// prefs.json couldn't be read at start: read again before a save. Once it can be, what it holds comes back, with
/// this run's changes over it (a vault the file doesn't list added), and saving works again. False while still held.
fn read_again() -> bool {
    if !lock().unreadable {
        return true;
    }
    // One save at a time does this; the others find it done.
    static AGAIN: Mutex<()> = Mutex::new(());
    let _one = AGAIN.lock().unwrap_or_else(|e| e.into_inner());
    if !lock().unreadable {
        return true;
    }
    let file = match std::fs::read(prefs_path()) {
        Ok(bytes) => serde_json::from_str::<Value>(String::from_utf8_lossy(&bytes).trim_start_matches('\u{feff}')).ok(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => return false,
    };
    let Some(Value::Object(mut merged)) = file else {
        // Gone, or not JSON (kept aside first, as at start): nothing of the person's to keep, this run's settings saved.
        if prefs_path().exists() {
            keep_damaged();
        }
        lock().unreadable = false;
        log::note("prefs.json can be saved again.");
        return true;
    };
    let start = UNREAD_START.lock().unwrap_or_else(|e| e.into_inner()).take().unwrap_or_default();
    let now = snapshot(&lock(), &language());
    for (key, value) in now {
        if start.get(&key) == Some(&value) {
            continue; // not changed this run: the file's own stays
        }
        if key == "vaults"
            && let (Some(Value::Array(theirs)), Value::Array(ours)) = (merged.get_mut("vaults"), &value)
        {
            let file_of = |v: &Value| v.get("file").and_then(Value::as_str).map(str::to_ascii_lowercase);
            let listed: HashSet<String> = theirs.iter().filter_map(file_of).collect();
            theirs.extend(ours.iter().filter(|v| file_of(v).is_some_and(|f| !listed.contains(&f))).cloned());
            continue;
        }
        merged.insert(key, value);
    }
    let Ok(text) = serde_json::to_string(&Value::Object(merged)) else { return false };
    if crate::core::durable_file::write_text(prefs_path(), &text).is_err() {
        *UNREAD_START.lock().unwrap_or_else(|e| e.into_inner()) = Some(start);
        return false;
    }
    // Read back as at start, over fresh settings: the file's values in memory too, so later saves keep them.
    *lock() = State::default();
    load();
    log::note("prefs.json could be read again; its settings are back, with this run's changes kept.");
    !lock().unreadable
}

/// Written to a temp file first, so a crash mid-write never leaves an empty prefs.json (and defaults).
/// false when it couldn't be written (`save_failed`); what was set still applies for this session.
pub fn save() -> bool {
    if !read_again() {
        lock().save_failed = true; // logged when it was found; nothing is written over a file that couldn't be read
        return false;
    }
    let language = language();
    let (text, number) = {
        let mut s = lock();
        let values = snapshot(&s, &language);
        match crate::core::models::to_net_json_string(&values) {
            Ok(text) => (text, SNAPSHOTS.fetch_add(1, Ordering::SeqCst) + 1),
            Err(e) => return failed(&mut s, &format!("prefs.json couldn't be saved ({e}); tried {SAVE_TRIES} times. The settings apply until Bloxkeep closes, and are kept by the next save that works.")),
        }
    };
    // The disk work (fsync, rename, waits between tries) outside the prefs lock, which every part of the app reads.
    let mut last = WRITTEN.lock().unwrap_or_else(|e| e.into_inner());
    if number < last.0 {
        return last.1; // a newer snapshot went first: this one's changes are in it
    }
    last.0 = number;
    // Flushed to disk before it replaces the old file: a power cut can't leave it empty (language, theme and
    // the backup folder would quietly reset, and backups stop).
    let result = std::fs::create_dir_all(vault::data_dir()).map_err(|e| format!("{:?}", e.kind())).and_then(|()| {
        // An antivirus or sync program holding the file lets go within a moment, as for the vault: tried a few times.
        let mut attempt = 1;
        loop {
            match crate::core::durable_file::write_text(&prefs_path(), &text) {
                Ok(()) => return Ok(()),
                Err(_) if attempt < SAVE_TRIES => std::thread::sleep(SAVE_RETRY_WAIT),
                Err(e) => return Err(e.to_string()),
            }
            attempt += 1;
        }
    });
    last.1 = result.is_ok();
    // Still under WRITTEN, so a later save's outcome can't be overtaken by this one's.
    match result {
        Ok(()) => {
            lock().save_failed = false;
            true
        }
        // The choice still applies for this session, and the next Save that works writes it all.
        Err(why) => failed(
            &mut lock(),
            &format!("prefs.json couldn't be saved ({why}); tried {SAVE_TRIES} times. The settings apply until Bloxkeep closes, and are kept by the next save that works."),
        ),
    }
}

/// prefs.json's content from the state now.
fn snapshot(s: &State, language: &str) -> Map<String, Value> {
    let first = s.vault_list.iter().find(|v| v.file == vaults::FIRST_FILE);
    let mut values = Map::new();
    let mut put = |key: &str, value: Value| {
        values.insert(key.to_string(), value);
    };
    put("language", json!(language));
    put("languageChosen", json!(language_chosen()));
    put("theme", json!(s.theme));
    put("sunsetDefault", json!(s.sunset_default));
    put("themeLight", json!(s.system_light));
    put("themeDark", json!(s.system_dark));
    put("closeToTray", json!(s.close_to_tray));
    put("globalHotkeys", json!(s.global_hotkeys));
    put("hideNames", json!(s.hide_names));
    put("trayHintShown", json!(s.tray_hint_shown));
    put("safeRendering", json!(s.safe_rendering));
    put("checkUpdates", json!(s.check_updates));
    put("betaUpdates", json!(s.beta_updates));
    put("lastVersion", json!(s.last_version));
    put("discordPresence", json!(s.discord_presence));
    put("countOnline", json!(s.count_online));
    put("onlineId", json!(s.online_id));
    put("boardName", json!(s.board_name));
    put("boardNameChosen", json!(s.board_name_chosen));
    // Written (empty once moved to a vault) so the old keys never come back from what this build doesn't know.
    put("remoteControl", json!(s.remote_control_pc));
    put("remoteKey", json!(s.remote_key_pc));
    put("remoteGone", json!(s.remote_gone));
    put("firstOpenTold", json!(s.first_open_told));
    let c = &s.custom;
    put("customTheme", json!({ "background": c.background, "accent": c.accent, "fade": c.fade, "picture": c.picture, "blur": c.blur }));
    put(
        "savedThemes",
        Value::Array(
            s.saved_themes
                .iter()
                .map(|t| {
                    json!({ "id": t.id, "name": t.name, "background": t.look.background, "accent": t.look.accent, "fade": t.look.fade, "picture": t.look.picture, "blur": t.look.blur })
                })
                .collect(),
        ),
    );
    put("notifications", json!(s.notifications));
    put("notifyOff", json!(ordinal_sorted(&s.notify_off)));
    // The first vault's backup state also where builds before several vaults read it.
    put("backupFolder", json!(s.backup_folder));
    put("backupEnabled", json!(s.backup_enabled));
    put("lastBackup", json!(first.and_then(|v| v.last_backup).or(s.last_backup_pc)));
    put("backupPending", json!(first.map(|v| v.backup_pending).unwrap_or(s.backup_pending_pc)));
    put(
        "vaults",
        Value::Array(
            s.vault_list
                .iter()
                .map(|v| {
                    json!({ "id": v.id, "file": v.file, "name": v.name, "picture": v.picture, "lastBackup": v.last_backup, "backupPending": v.backup_pending,
                        "remoteControl": v.remote_control, "remoteKey": v.remote_key })
                })
                .collect(),
        ),
    );
    put("lastVault", json!(s.last_vault));
    put("askVault", json!(s.ask_vault));
    put("dailySignInCheck", json!(s.daily_sign_in_check));
    put("lastSignInCheck", json!(s.last_sign_in_check));
    put("miniOpen", json!(s.mini_open));
    put("miniWindow", json!(s.mini_window));
    put("panelWindows", json!(s.panel_windows));
    put("mainWindow", json!(s.main_window));
    put("mainMaximized", json!(s.main_maximized));
    put(
        "robloxHistory",
        Value::Array(s.roblox_history.iter().map(|h| json!({ "at": h.at, "version": h.version, "folder": h.folder, "source": h.source, "first": h.first })).collect()),
    );
    put("robloxClosedAt", json!(s.roblox_closed_at));
    put("robloxClosedCount", json!(s.roblox_closed_count));
    put("robloxClosedNotified", json!(s.roblox_closed_notified));
    put("skipConfirm", json!(ordinal_sorted(&s.skip_confirm)));
    put("multiWorked", json!(s.multi_worked));
    put("multiRobloxAtStart", json!(s.multi_roblox_at_start));
    put("seenRoblox", json!(s.seen_roblox));
    put("knownVaultIds", json!(ordinal_sorted(&s.known_vault_ids)));
    put("foreignRestore", json!(s.foreign_restore));
    let feeds = &s.game_feeds;
    let alerts: Map<String, Value> = feeds.alerts.iter().map(|(k, a)| (k.clone(), json!({ "mode": a.mode, "words": a.words }))).collect();
    put("gameFeeds", json!({ "follow": feeds.follow, "read": feeds.read, "seen": feeds.seen, "alerts": alerts }));
    let w = &s.wikis;
    let marks = |list: &Vec<WikiMark>| Value::Array(list.iter().map(|m| json!({ "site": m.site, "title": m.title, "game": m.game, "at": m.at })).collect());
    put(
        "wikis",
        json!({
            "games": w.games.iter().map(|g| json!({ "id": g.id, "name": g.name, "site": g.site, "place": g.place, "img": g.img, "added": g.added })).collect::<Vec<_>>(),
            "saved": marks(&w.saved),
            "recent": marks(&w.recent),
        }),
    );
    // Never over a known key: a pref missing from Known would otherwise revert to its loaded value every restart.
    for (key, value) in &s.extra {
        if !values.contains_key(key) {
            values.insert(key.clone(), value.clone());
        }
    }
    values
}

/// A failed Save: the flag the app shows a notice for (AppController.AutoLock.cs), and one line in the log per session.
fn failed(s: &mut State, note: &str) -> bool {
    s.save_failed = true;
    if !s.save_failure_logged {
        s.save_failure_logged = true;
        log::note(note);
    }
    false
}
