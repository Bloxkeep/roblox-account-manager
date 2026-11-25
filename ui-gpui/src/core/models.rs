//! Models.cs: what the vault holds.
//!
//! Each vault object keeps fields it doesn't know (Extra), so an older Bloxkeep run after a newer one saves them back
//! untouched instead of silently dropping them.
//!
//! The JSON is System.Text.Json's: property names as written in C# (PascalCase), Guid as a string, DateTime through
//! `base::Stamp`, null for an empty optional, the unknown fields last. `to_net_json` writes it byte for byte as .NET's
//! JsonSerializer does (its default encoder escapes every non-ASCII character and `<>&'+\``).
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::core::base::{Extra, Guid, Res, Stamp, new_guid};
use crate::core::game_target::GameTarget;
use crate::core::macro_model::Macro;
use crate::core::roblox_game_settings::RobloxConfig;
use crate::core::totp::TotpKey;

/// C#'s `default(DateTime)`: 0001-01-01T00:00:00, no zone (what a DateTime property nobody set is written as).
pub fn stamp_min() -> Stamp {
    Stamp::parse("0001-01-01T00:00:00").expect("a fixed date")
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Account {
    pub id: Guid,
    pub user_id: i64,
    pub username: String,
    pub display_name: String,
    pub alias: String,
    pub group: String,
    pub note: String,
    pub password: String,
    pub cookie: String,
    /// `realm::GLOBAL` (roblox.com) or `realm::VNG` (Roblox VNG, Vietnam).
    pub realm: String,
    pub created_at: Stamp,
    pub last_used: Option<Stamp>,
    pub last_checked: Option<Stamp>,
    pub cookie_valid: Option<bool>,
    /// When Roblox last renewed `cookie` (see AppController.SignIns.cs); None: not since Bloxkeep watches for it.
    pub cookie_renewed: Option<Stamp>,
    pub auto_rejoin: bool,
    /// Auto-rejoin was switched off because Roblox ended the sign-in: on again once it is signed in again.
    pub rejoin_off_by_sign_out: bool,
    /// Opened with the shared Roblox settings when those apply to chosen accounts only.
    pub use_roblox_config: bool,
    /// This account's own frame rate limit in Roblox (see RobloxGameSettings.ForLaunch); 0: none.
    pub fps_cap: i32,
    /// The account the person plays on: left out of Select all, and its window of Save RAM, the RAM limit, Arrange
    /// windows and the background cores (it always gets the fast ones).
    pub is_main: bool,
    /// The authenticator (2-step) key, None when none. A secret like `password`: only its current code leaves the app.
    pub authenticator: Option<TotpKey>,
    /// What Roblox's public profile of the account said at the last check (see AppController.Health.cs); None until asked.
    pub banned: Option<bool>,
    pub banned_since: Option<Stamp>,
    /// Games that banned this account (the game's own ban, not Roblox's), as Bloxkeep saw them: no auto-rejoin into those.
    pub game_bans: Vec<GameBan>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for Account {
    fn default() -> Account {
        Account {
            id: new_guid(),
            user_id: 0,
            username: String::new(),
            display_name: String::new(),
            alias: String::new(),
            group: String::new(),
            note: String::new(),
            password: String::new(),
            cookie: String::new(),
            realm: crate::core::roblox_api::realm::GLOBAL.to_string(),
            created_at: Stamp::now(),
            last_used: None,
            last_checked: None,
            cookie_valid: None,
            cookie_renewed: None,
            auto_rejoin: false,
            rejoin_off_by_sign_out: false,
            use_roblox_config: false,
            fps_cap: 0,
            is_main: false,
            authenticator: None,
            banned: None,
            banned_since: None,
            game_bans: Vec::new(),
            extra: Extra::new(),
        }
    }
}

impl Account {
    /// The alias when there is one (trimmed), else the username. Never written to the vault ([JsonIgnore]).
    pub fn label(&self) -> String {
        if !self.alias.trim().is_empty() { self.alias.trim().to_string() } else { self.username.clone() }
    }
}

/// `password`, `cookie` and `authenticator` left out: `{:?}` can end up in a log.
impl std::fmt::Debug for Account {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use crate::core::base::Redacted;
        f.debug_struct("Account")
            .field("id", &self.id)
            .field("user_id", &self.user_id)
            .field("username", &self.username)
            .field("display_name", &self.display_name)
            .field("alias", &self.alias)
            .field("group", &self.group)
            .field("note", &self.note)
            .field("password", &Redacted)
            .field("cookie", &Redacted)
            .field("realm", &self.realm)
            .field("created_at", &self.created_at)
            .field("last_used", &self.last_used)
            .field("last_checked", &self.last_checked)
            .field("cookie_valid", &self.cookie_valid)
            .field("cookie_renewed", &self.cookie_renewed)
            .field("auto_rejoin", &self.auto_rejoin)
            .field("rejoin_off_by_sign_out", &self.rejoin_off_by_sign_out)
            .field("use_roblox_config", &self.use_roblox_config)
            .field("fps_cap", &self.fps_cap)
            .field("is_main", &self.is_main)
            .field("authenticator", &self.authenticator.as_ref().map(|_| Redacted))
            .field("banned", &self.banned)
            .field("banned_since", &self.banned_since)
            .field("game_bans", &self.game_bans)
            .field("extra", &self.extra)
            .finish()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct AppSettings {
    pub last_place_input: String,
    pub last_job_id: String,
    /// Off until the user opts in: Roblox does not officially support several clients.
    pub multi_instance: bool,
    /// With Multi-Roblox on, open every client in a window (Roblox's fullscreen switch is shared by all of them). On by default.
    pub multi_instance_windowed: bool,
    /// With Multi-Roblox on, tile the Roblox windows once a launch has opened them.
    pub multi_instance_tile: bool,
    /// Maximise each Roblox window Bloxkeep opens (still a window, not Roblox's shared fullscreen); not with `multi_instance_tile`.
    pub maximize_on_open: bool,
    /// What a window does once in its game (a join, a rejoin): "" as it was (a rejoin's window as the account's last
    /// one was), "show" stays up, "minimise" minimised once (Roblox draws less then).
    pub on_join: String,
    /// A window whose player leaves its game (Leave in Roblox's menu) is closed, rather than staying open on Roblox's
    /// home screen: one Roblox per game played (the owner, 2026-11-20). Off unless turned on.
    pub close_after_leave: bool,
    pub rename_windows: bool,
    /// The account's picture as its Roblox window's icon; None (saved before it had its own switch): as `rename_windows`.
    pub window_pictures: Option<bool>,
    /// Taskbar buttons: "roblox" (one for every Roblox window, Windows' own), "account" (one each) or "preset" (one group per preset).
    pub taskbar_groups: String,
    /// How Arrange windows lays them out: "fill" (whole screen, one tile per window); "<cols>x<rows>" ("2x2"…), a
    /// screen grid filled from the top-left whatever the window count; "custom": every window `window_width` ×
    /// `window_height`; or, from older versions, "small" / "medium" / "large".
    pub window_layout: String,
    /// The "custom" layout's window size, in screen pixels (the whole window, title bar too).
    pub window_width: i32,
    pub window_height: i32,
    /// Replaced by `launch_via`; only read to migrate older vaults.
    pub direct_launch: bool,
    pub launch_via: String,
    /// The launcher picked by hand (Settings › Open Roblox with › Choose…), used when LaunchVia is "custom".
    pub custom_launcher: String,
    /// Account list as "cards" or "list", and its order; "" until first chosen.
    pub layout: String,
    pub sort: String,
    pub launch_delay_seconds: i32,
    /// Minutes without keyboard or mouse input anywhere on the PC before the lock screen shows; 0 never. Roblox
    /// windows and background work carry on (see AppController.ScreenLock).
    pub auto_lock_minutes: i32,
    pub memory_warnings: bool,
    pub trim_background: bool,
    /// Clean RAM's areas (clean_ram.rs, one bit each).
    pub clean_areas: u32,
    /// Auto clean: when the PC's RAM use (%) goes above this; 0 off.
    pub auto_clean_at: i32,
    /// Auto clean: every so many minutes; 0 off.
    pub auto_clean_every: i32,
    /// A launcher installs Roblox's newest build by itself while no Roblox window is open.
    pub auto_update_roblox: bool,
    /// When that update goes in: "idle" once no Roblox window is open, "now" closing them all first (like Close all).
    /// When that update goes in: "idle" once no Roblox window is open, "now" closing them all first (like Close all), "keep"
    /// once none is open too, windows opened meanwhile playing on the build already running instead of waiting.
    pub roblox_update_when: String,
    /// Roblox's leanest drawing for the windows Bloxkeep opens, Light AFK presets' too. See RobloxFlags.
    pub fast_flags: bool,
    /// The most RAM a minimized or hidden Roblox window keeps, in MB; the rest waits in virtual memory (0 = no limit,
    /// the default). See AppController.LimitRestingRam.
    pub ram_limit_mb: i32,
    /// A Roblox window whose own memory (commit, which a RAM limit doesn't lower) stays above this many MB is
    /// closed and opened again into its server (0 = never, the default). See AppController.RestartDueClients.
    pub restart_above_mb: i32,
    /// Background Roblox windows run in Windows' efficiency mode (below-normal priority, efficiency cores); the one
    /// in front at full speed (AppController.Cores.cs). Off by default.
    pub split_cores: bool,
    /// Off until the user opts in: it briefly brings Roblox windows to the front while the PC is idle.
    pub anti_afk: bool,
    /// Minutes without input before anti-AFK presses for a window (5 to 18): then, even while the PC is in use, in a pause.
    /// Roblox kicks at 20; some games sooner (a Pet Simulator 99 player's, 2026-11-20, about 15).
    pub anti_afk_minutes: i32,
    /// The key anti-AFK presses: F15 (on no keyboard, used by no game), or one of the macro keys (Space jumps, for a game
    /// that wants the character to move).
    pub anti_afk_key: String,
    /// A window in a public server with more than this many other players moves to a quieter one; 0 = never.
    /// See AppController.ServerHop.cs.
    pub hop_above_players: i32,
    /// How long a window stays disconnected before Bloxkeep closes it to rejoin, in seconds (0: at once, after the 3 s
    /// every drop is given). A game that moves between servers can show a disconnect for a moment and get into the
    /// next server by itself; with a wait, a window back in a game by then is left alone.
    pub rejoin_wait_seconds: i32,
    pub roblox_config: RobloxConfig,
    pub recent_games: Vec<RecentGame>,
    pub pinned_games: Vec<RecentGame>,
    /// Servers saved to join again (the game picker's Saved tab), newest first.
    pub saved_servers: Vec<SavedServer>,
    /// Names last looked up with "Join a player…", newest first (see AppController.JoinPlayer.cs).
    pub recent_players: Vec<String>,
    /// "Join a player…"'s box: the accounts it opens follow that player when they rejoin (on until unticked).
    pub follow_joined_player: bool,
    pub presets: Vec<LaunchPreset>,
    /// Macros (Beta): recorded or built input sequences played into Roblox windows (see Macro.cs, MacroPlayer.cs).
    pub macros: Vec<Macro>,
    /// Accounts in the order Play opens them; accounts missing here (added later) come after, as listed.
    pub launch_order: Vec<Guid>,
    /// Ids this vault had before password changes (`Vault::id`): backups under them are still its to prune.
    pub previous_vault_ids: Vec<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for AppSettings {
    fn default() -> AppSettings {
        AppSettings {
            last_place_input: String::new(),
            last_job_id: String::new(),
            multi_instance: false,
            multi_instance_windowed: true,
            multi_instance_tile: false,
            maximize_on_open: false,
            on_join: String::new(),
            close_after_leave: false,
            rename_windows: true,
            window_pictures: None,
            taskbar_groups: "roblox".into(),
            window_layout: "fill".into(),
            window_width: 640,
            window_height: 480,
            direct_launch: false,
            launch_via: String::new(),
            custom_launcher: String::new(),
            layout: String::new(),
            sort: String::new(),
            launch_delay_seconds: 8,
            auto_lock_minutes: 0,
            memory_warnings: true,
            trim_background: false,
            clean_areas: crate::clean_ram::DEFAULT_AREAS,
            auto_clean_at: 0,
            auto_clean_every: 0,
            auto_update_roblox: true,
            roblox_update_when: "idle".into(),
            fast_flags: false,
            ram_limit_mb: 0,
            restart_above_mb: 0,
            split_cores: false,
            anti_afk: false,
            anti_afk_minutes: 12,
            anti_afk_key: "F15".into(),
            hop_above_players: 0,
            rejoin_wait_seconds: 0,
            roblox_config: RobloxConfig::default(),
            recent_games: Vec::new(),
            pinned_games: Vec::new(),
            saved_servers: Vec::new(),
            recent_players: Vec::new(),
            follow_joined_player: true,
            presets: Vec::new(),
            macros: Vec::new(),
            launch_order: Vec::new(),
            previous_vault_ids: Vec::new(),
            extra: Extra::new(),
        }
    }
}

/// A saved launch: these accounts, in this order, into this game and kind of server.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct LaunchPreset {
    pub id: Guid,
    pub name: String,
    pub account_ids: Vec<Guid>,
    /// As typed in the dock (place id or link); empty opens Roblox without a game.
    pub place_input: String,
    /// The game's name when the preset was saved, for menus.
    pub game_name: String,
    /// "any" (Roblox picks), "most", "fewest", "ping", "random", or "job" (`job_id`).
    pub server: String,
    pub job_id: String,
    /// With `server` "follow": the player its accounts join and follow on every rejoin (their username when saved).
    pub follow_player: String,
    /// That player's user id, which is what Roblox is asked about (a username can change).
    pub follow_user_id: i64,
    /// Seconds between accounts; None uses the global setting.
    pub delay_seconds: Option<i32>,
    /// Runs by itself at set times; None (older presets) or enabled = false when it doesn't.
    pub schedule: Option<PresetSchedule>,
    /// When it last opened accounts (by hand or by schedule), UTC; None if it hasn't since this was added.
    pub last_run_at: Option<Stamp>,
    /// Anti-AFK for its windows; None follows Settings.
    pub anti_afk: Option<bool>,
    /// Auto-rejoin for its windows; None follows each account's own switch.
    pub auto_rejoin: Option<bool>,
    /// Where a dropped window rejoins: "" any server, "any" a server another of its accounts plays in, or an
    /// account id, that account's server (see AppController.TeammateTargetAsync).
    pub rejoin_with: String,
    /// "" (Roblox as set up) or "afk": lowest graphics, 30 FPS, no sound, minimised once in the game.
    pub profile: String,
    /// Each of its windows in a game is closed and opened again after this many hours; 0 = never.
    pub restart_hours: i32,
    /// Its windows are arranged (Settings' layout) once it has opened them; None follows Settings.
    pub arrange: Option<bool>,
    /// What its windows do once in their game ("show", "minimise"); "" follows Settings' `on_join`.
    pub on_join: String,
    /// Its own Discord webhook for its schedule messages, stop summary and window alerts; "" uses the one in
    /// Settings (see AppController.Discord.cs).
    pub discord_webhook: String,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for LaunchPreset {
    fn default() -> LaunchPreset {
        LaunchPreset {
            id: new_guid(),
            name: String::new(),
            account_ids: Vec::new(),
            place_input: String::new(),
            game_name: String::new(),
            server: "any".into(),
            job_id: String::new(),
            follow_player: String::new(),
            follow_user_id: 0,
            delay_seconds: None,
            schedule: None,
            last_run_at: None,
            anti_afk: None,
            auto_rejoin: None,
            rejoin_with: String::new(),
            profile: String::new(),
            restart_hours: 0,
            arrange: None,
            on_join: String::new(),
            discord_webhook: String::new(),
            extra: Extra::new(),
        }
    }
}

/// `discord_webhook` left out: anyone with it can post to the channel.
impl std::fmt::Debug for LaunchPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LaunchPreset")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("account_ids", &self.account_ids)
            // It can be a private server's link (privateServerLinkCode), which lets anyone in: never written out.
            .field("place_input", &crate::core::base::Redacted)
            .field("game_name", &self.game_name)
            .field("server", &self.server)
            .field("job_id", &self.job_id)
            .field("follow_player", &self.follow_player)
            .field("follow_user_id", &self.follow_user_id)
            .field("delay_seconds", &self.delay_seconds)
            .field("schedule", &self.schedule)
            .field("last_run_at", &self.last_run_at)
            .field("anti_afk", &self.anti_afk)
            .field("auto_rejoin", &self.auto_rejoin)
            .field("rejoin_with", &self.rejoin_with)
            .field("profile", &self.profile)
            .field("restart_hours", &self.restart_hours)
            .field("arrange", &self.arrange)
            .field("on_join", &self.on_join)
            .field("discord_webhook", &crate::core::base::Redacted)
            .field("extra", &self.extra)
            .finish()
    }
}

impl LaunchPreset {
    /// A copy, through its JSON: every field, a newer Bloxkeep's too, so a copy can't lose one (a followed player's id
    /// was lost, and the copy could never run). Same id: the caller gives it its own. (C#'s `Clone()`; named so beside
    /// Rust's own `clone`.)
    pub fn json_clone(&self) -> Res<LaunchPreset> {
        Ok(serde_json::from_value(serde_json::to_value(self)?)?)
    }
}

/// When a preset runs on its own (see AppController.Schedule.cs). Times are local wall-clock "HH:mm".
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct PresetSchedule {
    pub enabled: bool,
    pub time: String,
    /// Weekdays it repeats on, as DayOfWeek numbers (0 = Sunday); empty when it runs once.
    pub days: Vec<i32>,
    /// "yyyy-MM-dd" of a one-off run; empty when it repeats.
    pub date: String,
    /// "never", "after" (`stop_hours`) or "at" (`stop_time`).
    pub stop: String,
    pub stop_hours: f64,
    pub stop_time: String,
    /// The scheduled minute that last ran, "yyyy-MM-dd HH:mm", so a clock set back can't run it again.
    pub last_run: String,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for PresetSchedule {
    fn default() -> PresetSchedule {
        PresetSchedule {
            enabled: false,
            time: String::new(),
            days: Vec::new(),
            date: String::new(),
            stop: "never".into(),
            stop_hours: 0.0,
            stop_time: String::new(),
            last_run: String::new(),
            extra: Extra::new(),
        }
    }
}

/// A server saved to join again: a private server's link, or a code a game asks for inside it (Bloxkeep opens
/// the game and copies the code; it never types it in).
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct SavedServer {
    pub id: Guid,
    pub name: String,
    pub place_id: i64,
    pub universe_id: i64,
    pub game_name: String,
    /// The private server's link as pasted (privateServerLinkCode or a share link); empty for a code only.
    pub link: String,
    /// What the game asks for inside it; empty for a link only.
    pub code: String,
    #[serde(flatten)]
    pub extra: Extra,
}

/// `link` and `code` left out: either lets anyone into the private server.
impl std::fmt::Debug for SavedServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SavedServer")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("place_id", &self.place_id)
            .field("universe_id", &self.universe_id)
            .field("game_name", &self.game_name)
            .field("link", &crate::core::base::Redacted)
            .field("code", &crate::core::base::Redacted)
            .field("extra", &self.extra)
            .finish()
    }
}

impl Default for SavedServer {
    fn default() -> SavedServer {
        SavedServer {
            id: new_guid(),
            name: String::new(),
            place_id: 0,
            universe_id: 0,
            game_name: String::new(),
            link: String::new(),
            code: String::new(),
            extra: Extra::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct RecentGame {
    pub place_id: i64,
    pub universe_id: i64,
    pub name: String,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct VaultData {
    pub version: i32,
    pub accounts: Vec<Account>,
    pub settings: AppSettings,
    pub open_sessions: Vec<OpenSession>,
    /// Clients that ended here in the last minutes (see AppController.IsStaleInGame); missing in older vaults.
    pub ended_sessions: Vec<EndedSession>,
    /// Rejoins waiting their turn, picked up again by the next unlock (an update's restart, a lock); missing in older vaults.
    pub waiting_rejoins: Vec<WaitingRejoin>,
    /// Stays in games for the Statistics view (see AppController.Stats.cs); missing in older vaults.
    pub play_sessions: Vec<PlaySession>,
    /// Statistics › Activity, oldest first (see AppController.Activity.cs); missing in older vaults.
    pub activity: Vec<ActivityEntry>,
    /// The minutes each account played on each day, as Roblox's Screen time tells them (it keeps the last 7 days only,
    /// so they are kept here as they come: app/screen_time.rs), by Roblox user id and local day ("2026-11-25");
    /// missing in older vaults.
    pub screen_time: std::collections::BTreeMap<i64, std::collections::BTreeMap<String, i64>>,
    /// Scheduled runs whose stop is still to come when the app closes; armed again at the next unlock.
    pub scheduled_runs: Vec<ScheduledRunState>,
    /// Settings › Discord (see AppController.Discord.cs); kept here because the webhook link is a secret.
    pub discord: DiscordSettings,
    /// Settings › Notifications › Phone (see AppController.Phone.cs); here because whoever knows the topic reads it.
    pub phone: PhoneSettings,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for VaultData {
    fn default() -> VaultData {
        VaultData {
            version: 1,
            accounts: Vec::new(),
            settings: AppSettings::default(),
            open_sessions: Vec::new(),
            ended_sessions: Vec::new(),
            waiting_rejoins: Vec::new(),
            play_sessions: Vec::new(),
            activity: Vec::new(),
            screen_time: Default::default(),
            scheduled_runs: Vec::new(),
            discord: DiscordSettings::default(),
            phone: PhoneSettings::default(),
            extra: Extra::new(),
        }
    }
}

/// Alerts on the phone through ntfy: off until the person turns it on.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct PhoneSettings {
    pub enabled: bool,
    /// An ntfy topic's name (on ntfy.sh), or https://server/topic.
    pub topic: String,
}

/// `topic` left out: whoever knows it reads the alerts.
impl std::fmt::Debug for PhoneSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PhoneSettings").field("enabled", &self.enabled).field("topic", &crate::core::base::Redacted).finish()
    }
}

/// Posting to a Discord channel through a webhook: off until the person turns it on.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct DiscordSettings {
    pub enabled: bool,
    /// https://discord.com/api/webhooks/{id}/{token}; anyone with it can post to the channel.
    pub webhook_url: String,
    /// "gone" after Discord said the webhook no longer exists (sending stopped); None otherwise.
    pub stopped: Option<String>,
    pub username: String,
    pub avatar_url: String,
    /// "app" (follows Bloxkeep's), "en" or "vi".
    pub language: String,
    /// "embed" or "plain".
    pub style: String,
    /// Messages name accounts "Account 1", "Account 2"…, apart from Settings › Appearance's switch for the screen.
    pub hide_names: bool,
    pub summary_daily: bool,
    pub summary_daily_time: String,
    pub summary_weekly: bool,
    /// A DayOfWeek number (0 = Sunday).
    pub summary_weekday: i32,
    pub summary_weekly_time: String,
    pub summary_after_schedule: bool,
    /// 1 (today), 7 or 30.
    pub summary_days: i32,
    /// Of "total", "accounts", "games", "sessions", "kicks".
    pub summary_parts: Vec<String>,
    /// Accounts the summary covers; empty for all.
    pub summary_accounts: Vec<Guid>,
    /// Local "yyyy-MM-dd" the daily / weekly summary last went out, so neither goes twice (a clock set back included).
    pub last_daily: String,
    pub last_weekly: String,
    pub board: bool,
    pub board_minutes: i32,
    /// The status message being edited in place; empty until posted (or after it was deleted).
    pub board_message_id: String,
    /// Event kinds sent (as in AppController.Notify.cs), apart from the Windows notification switches.
    pub events: Vec<String>,
    /// Per message kind ("summary" or an event kind): "none", "here", "everyone", "user:<id>", "role:<id>".
    /// (A Dictionary: written in the order its keys were added, which IndexMap keeps.)
    pub pings: IndexMap<String, String>,
    /// Between these local "HH:mm" times messages still go out, without pings.
    pub quiet: bool,
    pub quiet_from: String,
    pub quiet_to: String,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for DiscordSettings {
    fn default() -> DiscordSettings {
        DiscordSettings {
            enabled: false,
            webhook_url: String::new(),
            stopped: None,
            username: "Bloxkeep".into(),
            avatar_url: String::new(),
            language: "app".into(),
            style: "embed".into(),
            hide_names: false,
            summary_daily: false,
            summary_daily_time: "22:00".into(),
            summary_weekly: false,
            summary_weekday: 0,
            summary_weekly_time: "22:00".into(),
            summary_after_schedule: false,
            summary_days: 1,
            summary_parts: ["total", "accounts", "games", "sessions", "kicks"].map(String::from).to_vec(),
            summary_accounts: Vec::new(),
            last_daily: String::new(),
            last_weekly: String::new(),
            board: false,
            board_minutes: 5,
            board_message_id: String::new(),
            events: ["drop", "schedule", "signin", "launch"].map(String::from).to_vec(),
            pings: IndexMap::new(),
            quiet: false,
            quiet_from: "23:00".into(),
            quiet_to: "07:00".into(),
            extra: Extra::new(),
        }
    }
}

/// `webhook_url` left out: anyone with it can post to the channel.
impl std::fmt::Debug for DiscordSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiscordSettings")
            .field("enabled", &self.enabled)
            .field("webhook_url", &crate::core::base::Redacted)
            .field("stopped", &self.stopped)
            .field("username", &self.username)
            .field("avatar_url", &self.avatar_url)
            .field("language", &self.language)
            .field("style", &self.style)
            .field("hide_names", &self.hide_names)
            .field("summary_daily", &self.summary_daily)
            .field("summary_daily_time", &self.summary_daily_time)
            .field("summary_weekly", &self.summary_weekly)
            .field("summary_weekday", &self.summary_weekday)
            .field("summary_weekly_time", &self.summary_weekly_time)
            .field("summary_after_schedule", &self.summary_after_schedule)
            .field("summary_days", &self.summary_days)
            .field("summary_parts", &self.summary_parts)
            .field("summary_accounts", &self.summary_accounts)
            .field("last_daily", &self.last_daily)
            .field("last_weekly", &self.last_weekly)
            .field("board", &self.board)
            .field("board_minutes", &self.board_minutes)
            .field("board_message_id", &self.board_message_id)
            .field("events", &self.events)
            .field("pings", &self.pings)
            .field("quiet", &self.quiet)
            .field("quiet_from", &self.quiet_from)
            .field("quiet_to", &self.quiet_to)
            .field("extra", &self.extra)
            .finish()
    }
}

/// A scheduled preset run (see AppController.Schedule.cs) still waiting for its stop, as written to the vault.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct ScheduledRunState {
    pub preset_id: Guid,
    /// The preset's name when the run started, for its stop's notice if the preset was deleted since.
    pub name: String,
    /// The accounts the run opened: the ones its stop closes.
    pub opened: Vec<Guid>,
    /// When to stop (UTC), for "after N hours".
    pub stop_at: Option<Stamp>,
    /// Local minute ("yyyy-MM-dd HH:mm") to stop at, for "at a time".
    pub stop_at_minute: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for ScheduledRunState {
    fn default() -> ScheduledRunState {
        ScheduledRunState { preset_id: Guid::nil(), name: String::new(), opened: Vec::new(), stop_at: None, stop_at_minute: None, extra: Extra::new() }
    }
}

/// One stay in a game by a client Bloxkeep opened; still going while `ended_at` is None.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct PlaySession {
    pub account_id: Guid,
    /// The account's name when the stay began, shown once the account is deleted; None in stays from older versions.
    pub account_label: Option<String>,
    /// The client's process, to go on with the stay after Bloxkeep restarts.
    pub pid: i32,
    pub place_id: i64,
    pub universe_id: i64,
    /// The game's name when the stay began (or once looked up), so the view needs no lookups.
    pub game_name: String,
    pub started_at: Stamp,
    pub ended_at: Option<Stamp>,
    /// When the client was last seen still playing: the end of a stay Bloxkeep didn't see end.
    pub last_seen_at: Option<Stamp>,
    /// "left" (by choice), "disconnected" (see `reason_code`), "closed" (the window, by anyone), "sleep" (the PC went to
    /// sleep) or "appClosed" (Bloxkeep wasn't running).
    pub end_reason: Option<String>,
    /// Roblox's disconnect code when it gave one (273 = the same account joined elsewhere, 278 = idle).
    pub reason_code: Option<i32>,
    /// The preset whose window it was (summaries group play by preset); None opened otherwise, or before 1.0.5.
    pub preset_id: Option<Guid>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for PlaySession {
    fn default() -> PlaySession {
        PlaySession {
            account_id: Guid::nil(),
            account_label: None,
            pid: 0,
            place_id: 0,
            universe_id: 0,
            game_name: String::new(),
            started_at: stamp_min(),
            ended_at: None,
            last_seen_at: None,
            end_reason: None,
            reason_code: None,
            preset_id: None,
            extra: Extra::new(),
        }
    }
}

/// One line of Statistics › Activity (see AppController.Activity.cs).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct ActivityEntry {
    /// When it happened (UTC).
    pub at: Stamp,
    /// "opened", "rejoined", "joined", "left", "disconnected", "signedOut", "closed", "closedByRoblox", or a
    /// notification's type (AppController.Notify.cs), whose sentence is in `text`.
    pub kind: String,
    pub account_id: Option<Guid>,
    /// The account's name then, shown once the account is deleted.
    pub account_label: Option<String>,
    pub place_id: Option<i64>,
    /// The game's name, once known (filled in later for a place not looked up yet).
    pub game_name: Option<String>,
    /// Roblox's disconnect code, when it gave one.
    pub code: Option<i32>,
    /// A notification's own sentence, in English and in Vietnamese, so it shows in the language chosen later.
    pub text: Option<String>,
    pub text_vi: Option<String>,
    /// The same in the languages added after Vietnamese, by code ("es", "pt", …; lang.rs).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_in: Option<std::collections::BTreeMap<String, String>>,
    /// The accounts `text` names, in the order of its placeholders (U+E000, the number, U+E001), filled in when shown
    /// with each one's name then, so a sentence never holds a name. None in entries without names and in older ones
    /// (which hold the names themselves).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub names: Option<Vec<ActivityName>>,
    /// "ok", "info", "warn" or "error", as the notification was shown; None for the kinds above.
    pub tone: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for ActivityEntry {
    fn default() -> ActivityEntry {
        ActivityEntry {
            at: stamp_min(),
            kind: String::new(),
            account_id: None,
            account_label: None,
            place_id: None,
            game_name: None,
            code: None,
            text: None,
            text_vi: None,
            text_in: None,
            names: None,
            tone: None,
            extra: Extra::new(),
        }
    }
}

/// An account an activity entry's sentence names (see `ActivityEntry::names`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct ActivityName {
    pub id: Guid,
    /// The account's name when the entry was noted, shown once the account is deleted.
    pub label: String,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for ActivityName {
    fn default() -> ActivityName {
        ActivityName { id: Guid::nil(), label: String::new(), extra: Extra::new() }
    }
}

/// A game's own ban (Roblox's ban API), seen when the game put the account out or Roblox refused a join (Join
/// Error 600). Dropped once the account gets into that game again.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct GameBan {
    /// The place it was banned from (the game's start place when known).
    pub place_id: i64,
    pub game_name: Option<String>,
    pub at: Stamp,
    /// The game's words, when it gave any ("Exploiting").
    pub message: Option<String>,
    /// When a timed ban ends (the ban's endTimestamp, UTC); None for a permanent ban or one whose end isn't known. Past
    /// it, the ban no longer holds.
    pub until: Option<Stamp>,
}

impl Default for GameBan {
    fn default() -> GameBan {
        GameBan { place_id: 0, game_name: None, at: stamp_min(), message: None, until: None }
    }
}

/// A client that ended here lately, with its server and game: Roblox keeps reporting the account there for
/// minutes, which must not read as "playing on another device", even after Bloxkeep restarts.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct EndedSession {
    pub account_id: Guid,
    pub job_id: Option<String>,
    pub place_id: Option<i64>,
    pub at: Stamp,
}

impl Default for EndedSession {
    fn default() -> EndedSession {
        EndedSession { account_id: Guid::nil(), job_id: None, place_id: None, at: stamp_min() }
    }
}

/// A rejoin that was waiting its turn when Bloxkeep closed or locked (auto-rejoin's wait, a preset's restart).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct WaitingRejoin {
    pub account_id: Guid,
    pub target: Option<GameTarget>,
    pub due: Stamp,
    pub preset_id: Option<Guid>,
    pub restart: bool,
}

impl Default for WaitingRejoin {
    fn default() -> WaitingRejoin {
        WaitingRejoin { account_id: Guid::nil(), target: None, due: stamp_min(), preset_id: None, restart: false }
    }
}

/// A Roblox client that was still open when Bloxkeep closed; the next unlock picks it up again.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct OpenSession {
    pub account_id: Guid,
    pub pid: i32,
    pub started_at: Stamp,
    pub target: Option<GameTarget>,
    /// The preset that opened it, whose options still apply to it; None for one opened otherwise.
    pub preset_id: Option<Guid>,
    /// The frame rate limit Bloxkeep gave it as it opened; 0: none.
    pub fps_limit: i32,
    /// Opened by Bloxkeep on Roblox's home screen (PendingLaunch::home_screen).
    pub home_screen: bool,
}

impl Default for OpenSession {
    fn default() -> OpenSession {
        OpenSession { account_id: Guid::nil(), pid: 0, started_at: stamp_min(), target: None, preset_id: None, fps_limit: 0, home_screen: false }
    }
}

// ── JSON as .NET writes it ──────────────────────────────────────────────────────────────────────────────────────────

/// `JsonSerializer.SerializeToUtf8Bytes(value)` with .NET's default options: no spaces, every non-ASCII character and
/// `"<>&'+\`` written as \uXXXX (upper-case hex, UTF-16 units), a double in its shortest round-trip form ("2", "1.5",
/// "1E+20").
pub fn to_net_json<T: Serialize + ?Sized>(value: &T) -> Res<Vec<u8>> {
    let mut out = Vec::with_capacity(256);
    let mut ser = serde_json::Serializer::with_formatter(&mut out, NetFormatter);
    value.serialize(&mut ser)?;
    Ok(out)
}

/// `JsonSerializer.Serialize(value)` (and JsonNode.ToJsonString()): as `to_net_json`, as text.
pub fn to_net_json_string<T: Serialize + ?Sized>(value: &T) -> Res<String> {
    // Only ASCII comes out of NetFormatter.
    Ok(String::from_utf8(to_net_json(value)?).map_err(crate::core::base::Fail::other)?)
}

/// serde_json's compact output with System.Text.Json's escaping (JavaScriptEncoder.Default) and number format.
struct NetFormatter;

fn write_u16_escape<W: ?Sized + std::io::Write>(w: &mut W, unit: u16) -> std::io::Result<()> {
    write!(w, "\\u{unit:04X}")
}

impl serde_json::ser::Formatter for NetFormatter {
    fn write_string_fragment<W: ?Sized + std::io::Write>(&mut self, w: &mut W, fragment: &str) -> std::io::Result<()> {
        let mut start = 0;
        for (i, ch) in fragment.char_indices() {
            let escape = !ch.is_ascii() || matches!(ch, '<' | '>' | '&' | '\'' | '+' | '`' | '\u{7f}');
            if escape {
                w.write_all(fragment[start..i].as_bytes())?;
                let mut units = [0u16; 2];
                for unit in ch.encode_utf16(&mut units) {
                    write_u16_escape(w, *unit)?;
                }
                start = i + ch.len_utf8();
            }
        }
        w.write_all(fragment[start..].as_bytes())
    }

    fn write_char_escape<W: ?Sized + std::io::Write>(&mut self, w: &mut W, escape: serde_json::ser::CharEscape) -> std::io::Result<()> {
        use serde_json::ser::CharEscape;
        match escape {
            CharEscape::Quote => w.write_all(b"\\u0022"),
            CharEscape::ReverseSolidus => w.write_all(b"\\\\"),
            CharEscape::Solidus => w.write_all(b"/"),
            CharEscape::Backspace => w.write_all(b"\\b"),
            CharEscape::FormFeed => w.write_all(b"\\f"),
            CharEscape::LineFeed => w.write_all(b"\\n"),
            CharEscape::CarriageReturn => w.write_all(b"\\r"),
            CharEscape::Tab => w.write_all(b"\\t"),
            CharEscape::AsciiControl(byte) => write_u16_escape(w, byte as u16),
        }
    }

    fn write_f64<W: ?Sized + std::io::Write>(&mut self, w: &mut W, value: f64) -> std::io::Result<()> {
        w.write_all(net_double(value).as_bytes())
    }

    fn write_f32<W: ?Sized + std::io::Write>(&mut self, w: &mut W, value: f32) -> std::io::Result<()> {
        w.write_all(net_double(value as f64).as_bytes())
    }
}

/// A double as .NET's round-trip format writes it: the shortest digits that read back the same, in plain notation for
/// exponents -4 to 14 and as "1.5E+20" / "1E-05" outside them.
pub fn net_double(value: f64) -> String {
    if value == 0.0 {
        return if value.is_sign_negative() { "-0".into() } else { "0".into() };
    }
    let sci = format!("{value:e}"); // "1.5e20", "-1e-7": Rust's shortest round-trip digits
    let (mantissa, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    if (-4..15).contains(&exp) {
        format!("{value}")
    } else {
        format!("{mantissa}E{}{:02}", if exp < 0 { '-' } else { '+' }, exp.abs())
    }
}
