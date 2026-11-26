// AppController.cs: everything behind the interface. The interface sends {id, cmd, args}; each command gets a
// {type:"reply"} and state changes are pushed as events. Cookies and passwords reach the interface only when the user
// reveals one.
//
// `UserError` (an error whose message is shown to the user as-is) is `base::Fail::User` here.
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use indexmap::IndexMap;
use serde::Serialize;
use serde_json::{Value, json};

use super::App;
use super::accounts::BootProgress;
use crate::core::base::{Cancel, Fail, Guid, Res, Stamp};
use crate::core::game_target::GameTarget;
use crate::core::models::{Account, stamp_min};
use crate::core::roblox_api::{GameInfo, HTTP_REQUEST_EXCEPTION, JSON_EXCEPTION, Presence, TASK_CANCELED_EXCEPTION, fail_type_name, is_fail_kind, realm};
use crate::core::roblox_log_watcher::RobloxLogWatcher;
use crate::core::roblox_usage::RobloxUsage;
use crate::core::vault::{self, Vault};
use crate::core::{l, log, multi_instance, prefs, rt};

/// A future the interface hands the controller (the window's own work, awaited on the core's thread).
pub type LocalFuture = Pin<Box<dyn Future<Output = ()>>>;

/// `rejoin`: started by auto-rejoin rather than by the person. `preset_id`: the preset that opened the client; kept
/// through rejoins and restarts. Compared by reference where the C# does (ReferenceEqualityComparer): hold it in an
/// `Rc` and key sets with `LaunchRef`.
#[derive(Debug)]
pub struct PendingLaunch {
    pub account_id: Guid,
    pub before: HashSet<i32>,
    /// Settable: moved back with the clock (see TrackProcesses).
    pub started_at: Cell<Stamp>,
    pub target: Option<GameTarget>,
    pub rejoin: bool,
    pub preset_id: Option<Guid>,
    /// The tracker id in its launch link, also on its client's command line (see ClaimWindows).
    pub tracker: Option<String>,
    /// The rejoin's earlier tries that opened no window (see `RejoinRequest::open_retries`).
    pub open_retries: i32,
    /// The frame rate limit Bloxkeep gave it as it opened; 0: none. Its card says so.
    pub fps_limit: i32,
    /// Opened by Bloxkeep on Roblox's home screen: every game in it is the person's pick, and a drop goes back to the one
    /// it was in. Not a window adopted from outside Bloxkeep, whose server may be a private one (audit R9).
    pub home_screen: bool,
}

impl PendingLaunch {
    pub fn new(account_id: Guid, before: HashSet<i32>, started_at: Stamp, target: Option<GameTarget>, rejoin: bool, preset_id: Option<Guid>) -> PendingLaunch {
        PendingLaunch { account_id, before, started_at: Cell::new(started_at), target, rejoin, preset_id, tracker: None, open_retries: 0, fps_limit: 0, home_screen: false }
    }
}

/// A launch compared by reference (C#'s ReferenceEqualityComparer.Instance), for the sets and maps keyed by one.
#[derive(Clone, Debug)]
pub struct LaunchRef(pub Rc<PendingLaunch>);

impl PartialEq for LaunchRef {
    fn eq(&self, other: &LaunchRef) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for LaunchRef {}
impl Hash for LaunchRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (Rc::as_ptr(&self.0) as usize).hash(state);
    }
}

/// `network_retries`: attempts that failed only because there was no connection. `restart`: its preset's planned
/// restart, not a drop: it goes ahead whatever auto-rejoin says.
#[derive(Clone, Debug, PartialEq)]
pub struct RejoinRequest {
    pub account_id: Guid,
    pub target: GameTarget,
    pub due: Stamp,
    pub network_retries: i32,
    pub preset_id: Option<Guid>,
    pub restart: bool,
    /// Tries that opened no window: Roblox wouldn't start (its updater swapping the build folder) or quit before
    /// its window came. One that wouldn't start is tried again up to `MAX_OPEN_RETRIES` times; one that quit, for as
    /// long as auto-rejoin is on (`retry_open`), said once after `MAX_OPEN_RETRIES`.
    pub open_retries: i32,
}

impl RejoinRequest {
    pub fn new(account_id: Guid, target: GameTarget, due: Stamp) -> RejoinRequest {
        RejoinRequest { account_id, target, due, network_retries: 0, preset_id: None, restart: false, open_retries: 0 }
    }
}

pub const HAND_OVER_WINDOW: Duration = Duration::from_secs(60);
pub const PENDING_TIMEOUT: Duration = Duration::from_secs(120);
pub const PRESENCE_INTERVAL: Duration = Duration::from_secs(30);
pub const BACKGROUND_PRESENCE_INTERVAL: Duration = Duration::from_secs(4 * 60);
pub const REJOIN_DELAY: Duration = Duration::from_secs(10);
pub const DISCONNECT_GRACE: Duration = Duration::from_secs(2 * 60);
pub const RESUME_GRACE: Duration = Duration::from_secs(60);
pub const NETWORK_RETRY_DELAY: Duration = Duration::from_secs(30);
pub const MAX_RETRY_DELAY: Duration = Duration::from_secs(5 * 60);
pub const MAX_NETWORK_RETRIES: i32 = 20;
/// A refused launch ticket the sign-in check didn't confirm as a sign-out: tried again this many times, then said.
pub const MAX_TICKET_RETRIES: i32 = 3;
/// A rejoin that opened no window: tried again this many times (30 s, 1 min, 1.5 min on), then said.
pub const MAX_OPEN_RETRIES: i32 = 3;

/// Said for a refused launch ticket when the account is still signed in.
pub fn refused_text() -> String {
    l::t("Roblox refused to open the game for this account. Try again in a moment.")
}

/// One small GitHub API request an hour, well within its 60 an hour for anonymous callers (several PCs per IP too).
pub const UPDATE_CHECK_INTERVAL: Duration = Duration::from_secs(60 * 60);
pub const ROBLOX_VERSION_INTERVAL: Duration = Duration::from_secs(30 * 60);
pub const MEMORY_WARNING_INTERVAL: Duration = Duration::from_secs(5 * 60);
// Scanning launcher folders and the client's file version is slow; also re-read on a launch option change or vault open.
pub const INSTALL_CACHE_LIFETIME: Duration = Duration::from_secs(3 * 60);
pub const REGISTRY_CACHE_LIFETIME: Duration = Duration::from_secs(30);
pub const SAVE_DELAY: Duration = Duration::from_secs(3);
pub const BACKGROUND_SWEEP: Duration = Duration::from_secs(15);
pub const MEMORY_WARNING_LOAD: i32 = 92;
pub const DEFAULT_CLIENT_MB: i64 = 900;
/// A Roblox client's in-game share of Windows' commit limit when none runs to measure: 2.1–2.4 GB on 0.743,
/// 3.05–3.16 GB on 0.744 (the three biggest in Windows' low-virtual-memory event of 2026-10-22).
pub const DEFAULT_CLIENT_COMMIT_MB: i64 = 3072;
/// Below this much of the commit limit left, programs start failing; Windows itself warns at about this point.
pub const COMMIT_MARGIN_MB: i64 = 1024;
pub const MB: i64 = 1024 * 1024;
// PORT: `Json` (JsonSerializerDefaults.Web: camelCase names) is the shape every reply and event is built in here with
// json!; the names are written camelCase by hand.

/// A launch whose clients have all quit (failed sign-in, closed by the person) stops waiting after this.
pub const PENDING_GONE_GRACE: Duration = Duration::from_secs(5);
pub const TRAY_SWEEP: Duration = Duration::from_secs(15);
/// A log stuck in "joining" (a join or teleport that never finished) stops being trusted after this.
pub const JOINING_TIMEOUT: Duration = Duration::from_secs(2 * 60);
/// A client that started its game and got no server in this time isn't getting one (server ended, full, or
/// join failed). 2026-10-17 (0.743): five clients sat on "This experience has ended, or the server became unavailable…"
/// for three minutes with no "Joining game" line.
pub const GET_IN_TIMEOUT: Duration = Duration::from_secs(60);
/// A teleport that hasn't reached its next server this long after it started is no longer taken as one under way:
/// Roblox's status for the account is asked again (rejoin.rs's stuck_in_game). A teleport takes seconds; one that ends
/// in Roblox's "Disconnected" box (Error Code 256, "Lost connection due to an unknown error") logs no arrival and often
/// no disconnect either, and the window read as teleporting for good: nothing looked at it again. A real teleport
/// reaches its next server in about a second (a user's 0.748 logs: doTeleport to the new serverId in 0.5 s and 2 s),
/// so 45 s leaves a slow one plenty.
pub const TELEPORT_GIVE_UP: Duration = Duration::from_secs(45);
/// …and past this long it is a failed one whatever Roblox's status says (an account whose status is hidden): rejoined.
pub const TELEPORT_DEAD: Duration = Duration::from_secs(3 * 60);
/// How long past its time a rejoin kept with the vault is still done after an unlock (see OnUnlocked).
pub const WAITING_REJOIN_KEPT: Duration = Duration::from_secs(10 * 60);
pub const MAX_ASLEEP_EVENTS: usize = 30;

/// The only commands allowed while the screen is locked; the rest would show or change what is behind it.
pub const SCREEN_LOCK_COMMANDS: &[&str] = &["getState", "keepView", "pageBusy", "pageShown", "monitorArea", "setLanguage", "unlock", "lock"];

/// The instance fields AppController.cs declares.
pub struct ControllerState {
    // PORT: `_owner` (the WinForms Form) is the main window's handle here: dialogs are owned by it.
    pub owner: Cell<isize>,
    /// Where replies and events go (the interface): `_post`.
    pub post: RefCell<Option<Rc<dyn Fn(&str)>>>,
    // PORT: `_procTimer` (a WinForms timer, 1500 ms, 5000 ms in the background) is the loop `start` spawns; this is
    // its interval.
    pub proc_interval_ms: Cell<u64>,
    pub running: RefCell<IndexMap<Guid, (i32, Rc<PendingLaunch>)>>,
    pub pending: RefCell<Vec<Rc<PendingLaunch>>>,
    pub pending_seen_alive: RefCell<HashMap<LaunchRef, Stamp>>,
    /// The client last seen starting for a launch (a Roblox begun since it, no window yet): should it quit before its
    /// window, what it left behind (a crash report, Windows' error record) is looked up by this id.
    pub pending_seen_pid: RefCell<HashMap<LaunchRef, i32>>,
    /// Launches the person closed while still opening: their client is closed as soon as it is claimed.
    pub close_when_claimed: RefCell<HashSet<LaunchRef>>,
    /// Clients that left a game by ending (X button, crash): the windowless tray copy Roblox then starts as
    /// their child is ended if it appears within `TRAY_SWEEP`.
    pub tray_sweep: RefCell<HashMap<i32, Stamp>>,
    pub avatar_urls: RefCell<HashMap<i64, String>>,
    pub games: RefCell<HashMap<i64, Option<GameInfo>>>,
    pub game_fetched_at: RefCell<HashMap<i64, Stamp>>,
    pub presence: RefCell<HashMap<i64, Presence>>,
    /// Accounts whose last presence came from their own cookie (or showed them in a game): trustworthy.
    pub presence_reliable: RefCell<HashSet<i64>>,
    pub logs: RefCell<RobloxLogWatcher>,
    /// The Roblox settings file holds values Bloxkeep wrote; the person's own come back when Roblox is closed.
    pub roblox_settings_pending: Cell<bool>,
    /// Light AFK's Fast Flags are in a client's file (or were, when Bloxkeep closed before putting it back).
    pub roblox_flags_pending: Cell<bool>,
    pub roblox_settings_written_at: Cell<Stamp>,
    pub nudging: Cell<bool>,
    pub rejoin_queue: RefCell<Vec<RejoinRequest>>,
    /// Rejoins of each account since its client last stayed in a game a while (see KeepsDropping).
    pub rejoins_in_a_row: RefCell<HashMap<Guid, i32>>,
    pub offline_strikes: RefCell<HashMap<Guid, i32>>,
    pub vault: RefCell<Option<Arc<Vault>>>,
    /// The vault `vault_token` last saw and its number: the Weak keeps its memory from being reused by the next vault
    /// (an address alone came back the same after a lock and unlock), while its contents go with the last strong one.
    pub last_vault: RefCell<(std::sync::Weak<Vault>, usize)>,
    pub launch_cts: RefCell<Option<Cancel>>,
    pub rejoin_cts: RefCell<Option<Cancel>>,
    /// While a multi-account launch waits between two: the accounts still to open (rejoins of the others go
    /// meanwhile, see RunDueRejoin). None while it opens one or when none is going.
    pub launch_waiting: RefCell<Option<HashSet<Guid>>>,
    /// A launch whose windows are tiled as each joins its game: its start and the accounts it opens, each holding its
    /// tile from the first (see `tile_when_open`).
    pub tile_plan: RefCell<Option<(Stamp, Vec<Guid>)>>,
    pub boot_status_done: Cell<bool>,
    pub boot_avatars_done: Cell<bool>,
    pub boot: RefCell<Option<BootProgress>>,
    pub checking: Cell<bool>,
    pub last_armed: Cell<bool>,
    pub presence_busy: Cell<bool>,
    pub presence_loaded: Cell<bool>,
    pub rejoining: Cell<bool>,
    pub trim_refused_shown: Cell<bool>,
    pub disposed: Cell<bool>,
    pub next_presence: Cell<Stamp>,
    pub last_full_presence: Cell<Stamp>,
    pub last_memory_warning: Cell<Stamp>,
    /// Auto clean RAM's last run, either kind (memory.rs).
    pub last_auto_clean: Cell<Stamp>,
    /// When the timed auto clean started counting (the first look with it on).
    pub auto_clean_since: Cell<Stamp>,
    pub installs_read_at: Cell<Stamp>,
    pub sweep_background_until: Cell<Stamp>,
    pub resumed_at: Cell<Stamp>,
    pub last_session_start: Cell<Stamp>,
    pub last_tick: Cell<Stamp>,
    pub last_metrics: RefCell<Option<String>>,
    pub last_state: RefCell<Option<String>>,
    pub page_view: RefCell<Option<String>>,
    pub roblox_usage: RefCell<RobloxUsage>,
    pub roblox_updating: Cell<bool>,
    pub registry: Cell<Option<(bool, bool)>>,
    pub registry_read_at: Cell<Stamp>,
    pub background: Cell<bool>,
    /// Set when the browser was recreated while the app kept running; the next page restores its view.
    pub resuming_page: Cell<bool>,
    /// The page drew its first screen after loading (the window drops the picture it showed meanwhile).
    pub page_shown: RefCell<Option<Rc<dyn Fn() -> LocalFuture>>>,
    /// Recreates the interface drawing on the CPU (true) or the graphics card (false).
    pub set_safe_rendering: RefCell<Option<Rc<dyn Fn(bool) -> LocalFuture>>>,
    pub appearance_changed: RefCell<Option<Rc<dyn Fn()>>>,
    /// A page is there to receive state. False after the window released its browser in the background (building
    /// state for nobody is the costly part of a day in the tray); the next page asks with getState.
    pub page_attached: Cell<bool>,
    /// Set while the window suspends the page in the background: no state is built or sent (that would wake it), and
    /// events wait, the latest few, with every reply. Woken, it gets those, one fresh state and the memory figures.
    pub page_asleep: Cell<bool>,
    pub asleep_events: RefCell<Vec<String>>,
}

impl Default for ControllerState {
    fn default() -> ControllerState {
        let min = stamp_min();
        ControllerState {
            owner: Cell::new(0),
            post: RefCell::new(None),
            proc_interval_ms: Cell::new(1500),
            running: RefCell::new(IndexMap::new()),
            pending: RefCell::new(Vec::new()),
            pending_seen_alive: RefCell::new(HashMap::new()),
            pending_seen_pid: RefCell::new(HashMap::new()),
            close_when_claimed: RefCell::new(HashSet::new()),
            tray_sweep: RefCell::new(HashMap::new()),
            avatar_urls: RefCell::new(HashMap::new()),
            games: RefCell::new(HashMap::new()),
            game_fetched_at: RefCell::new(HashMap::new()),
            presence: RefCell::new(HashMap::new()),
            presence_reliable: RefCell::new(HashSet::new()),
            logs: RefCell::new(RobloxLogWatcher::new()),
            roblox_settings_pending: Cell::new(crate::core::roblox_game_settings::has_baseline()),
            roblox_flags_pending: Cell::new(crate::core::roblox_flags::pending()),
            roblox_settings_written_at: Cell::new(min),
            nudging: Cell::new(false),
            rejoin_queue: RefCell::new(Vec::new()),
            rejoins_in_a_row: RefCell::new(HashMap::new()),
            offline_strikes: RefCell::new(HashMap::new()),
            vault: RefCell::new(None),
            last_vault: RefCell::new((std::sync::Weak::new(), 0)),
            launch_cts: RefCell::new(None),
            rejoin_cts: RefCell::new(None),
            launch_waiting: RefCell::new(None),
            tile_plan: RefCell::new(None),
            boot_status_done: Cell::new(false),
            boot_avatars_done: Cell::new(false),
            boot: RefCell::new(None),
            checking: Cell::new(false),
            last_armed: Cell::new(false),
            presence_busy: Cell::new(false),
            presence_loaded: Cell::new(false),
            rejoining: Cell::new(false),
            trim_refused_shown: Cell::new(false),
            disposed: Cell::new(false),
            next_presence: Cell::new(min),
            last_full_presence: Cell::new(min),
            last_memory_warning: Cell::new(min),
            last_auto_clean: Cell::new(min),
            auto_clean_since: Cell::new(min),
            installs_read_at: Cell::new(min),
            sweep_background_until: Cell::new(min),
            resumed_at: Cell::new(min),
            last_session_start: Cell::new(min),
            last_tick: Cell::new(min),
            last_metrics: RefCell::new(None),
            last_state: RefCell::new(None),
            page_view: RefCell::new(None),
            roblox_usage: RefCell::new(RobloxUsage::new()),
            roblox_updating: Cell::new(false),
            registry: Cell::new(None),
            registry_read_at: Cell::new(min),
            background: Cell::new(false),
            resuming_page: Cell::new(false),
            page_shown: RefCell::new(None),
            set_safe_rendering: RefCell::new(None),
            appearance_changed: RefCell::new(None),
            page_attached: Cell::new(false),
            page_asleep: Cell::new(false),
            asleep_events: RefCell::new(Vec::new()),
        }
    }
}

// ───────────────────────────── replies ─────────────────────────────

/// What a command's method hands back, as the interface's `result` (C#'s `object?`): every method of the command table
/// returns one of these, sync or awaited.
pub trait Reply {
    fn into_reply(self) -> Res<Value>;
}

impl Reply for Value {
    fn into_reply(self) -> Res<Value> {
        Ok(self)
    }
}
impl Reply for () {
    fn into_reply(self) -> Res<Value> {
        Ok(Value::Null)
    }
}
impl<T: Reply> Reply for Option<T> {
    fn into_reply(self) -> Res<Value> {
        match self {
            Some(v) => v.into_reply(),
            None => Ok(Value::Null),
        }
    }
}
impl<T: Reply> Reply for Res<T> {
    fn into_reply(self) -> Res<Value> {
        self.and_then(Reply::into_reply)
    }
}
impl<T: Serialize> Reply for Vec<T> {
    fn into_reply(self) -> Res<Value> {
        Ok(serde_json::to_value(self)?)
    }
}

/// `impl Reply` for types serialized as they are (their serde shape must be the page's: camelCase).
#[macro_export]
macro_rules! reply_as_json {
    ($($t:ty),* $(,)?) => {
        $(impl $crate::core::app::controller::Reply for $t {
            fn into_reply(self) -> $crate::core::base::Res<serde_json::Value> {
                Ok(serde_json::to_value(self)?)
            }
        })*
    };
}
reply_as_json!(bool, String, &str, i32, i64, u32, u64, usize, f64, Guid);

/// Task.FromResult(value): the command's answer, whatever it was.
pub fn done(value: impl Reply) -> Res<Value> {
    value.into_reply()
}

// ───────────────────────────── arguments ─────────────────────────────

/// JsonValueKind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JsonKind {
    Object,
    Array,
    String,
    Number,
    True,
    False,
    Null,
}

pub fn kind_of(v: &Value) -> JsonKind {
    match v {
        Value::Object(_) => JsonKind::Object,
        Value::Array(_) => JsonKind::Array,
        Value::String(_) => JsonKind::String,
        Value::Number(_) => JsonKind::Number,
        Value::Bool(true) => JsonKind::True,
        Value::Bool(false) => JsonKind::False,
        Value::Null => JsonKind::Null,
    }
}

pub fn has(a: &Value, name: &str, kind: JsonKind) -> bool {
    a.as_object().and_then(|o| o.get(name)).is_some_and(|v| kind_of(v) == kind)
}

/// Str: the string, or "" when it isn't one.
pub fn str_of(a: &Value, name: &str) -> String {
    if has(a, name, JsonKind::String) { a[name].as_str().unwrap_or("").to_string() } else { String::new() }
}

pub fn opt_str(a: &Value, name: &str) -> Option<String> {
    if has(a, name, JsonKind::String) { a[name].as_str().map(str::to_string) } else { None }
}

/// Bool: true only for a JSON true.
pub fn bool_of(a: &Value, name: &str) -> bool {
    has(a, name, JsonKind::True)
}

pub fn opt_bool(a: &Value, name: &str) -> Option<bool> {
    if has(a, name, JsonKind::True) {
        Some(true)
    } else if has(a, name, JsonKind::False) {
        Some(false)
    } else {
        None
    }
}

/// Long: a whole number that fits an Int64, else 0 (TryGetInt64: 1.5 is not one).
pub fn long_of(a: &Value, name: &str) -> i64 {
    if has(a, name, JsonKind::Number) { a[name].as_i64().unwrap_or(0) } else { 0 }
}
pub fn long(a: &Value, name: &str) -> i64 {
    long_of(a, name)
}

/// TryGetInt32 of a property: None when missing, not a number or out of Int32's range.
pub fn int32_of(a: &Value, name: &str) -> Option<i32> {
    a.as_object().and_then(|o| o.get(name)).and_then(Value::as_i64).and_then(|v| i32::try_from(v).ok())
}

/// Guid.TryParse.
pub fn parse_guid(text: &str) -> Option<Guid> {
    let t = text.trim();
    if t.is_empty() { None } else { Guid::parse_str(t).ok() }
}

/// Id: the "id" argument as a Guid, or Guid.Empty.
pub fn id_of(a: &Value) -> Guid {
    parse_guid(&str_of(a, "id")).unwrap_or(Guid::nil())
}

pub fn ids_of(a: &Value) -> Vec<Guid> {
    guid_list(a, "ids")
}

/// The array's Guids, each once in its order; an entry that isn't one is left out.
pub fn guid_list(a: &Value, name: &str) -> Vec<Guid> {
    if !has(a, name, JsonKind::Array) {
        return Vec::new();
    }
    let mut out: Vec<Guid> = Vec::new();
    for e in a[name].as_array().into_iter().flatten() {
        let g = e.as_str().and_then(parse_guid).unwrap_or(Guid::nil());
        if g != Guid::nil() && !out.contains(&g) {
            out.push(g);
        }
    }
    out
}

/// What the person is told: Bloxkeep's and Roblox's own sentences as they are, else a short fixed sentence. Windows'
/// texts are in Windows' language and name internals, so they go to the log.
pub fn error_text(fail: &Fail) -> String {
    match fail {
        Fail::User(m) => m.clone(),
        Fail::Roblox { message, .. } => message.clone(),
        Fail::Other(m) if vault::is_invalid_password(fail) => m.clone(),
        // HttpRequestException or TaskCanceledException (a call off: "cancelled", base::Cancel::check).
        Fail::Other(m) if is_fail_kind(fail, HTTP_REQUEST_EXCEPTION) || is_fail_kind(fail, TASK_CANCELED_EXCEPTION) => {
            log::note(&format!("{}: {m}", fail_type_name(fail))); // offline, most likely: the reason without a stack
            l::t("Couldn't reach Roblox. Check your internet connection and try again.")
        }
        // The C#'s sentence by the exception's type: access denied, a file in use, unexpected JSON (an io::Error's text
        // ends with "(os error N)"; 5 is access denied).
        Fail::Other(m) => {
            log::write(fail);
            match os_error_code(m) {
                Some(5) => l::t("Windows denied access to the file."),
                // Only the file errors (not found, path not found, in use, locked): out of memory (8), a small page file
                // (1455), a socket (10000 and up) aren't about a file.
                // Disk full (39, 112) and a file mapped by another program (1224) are IOExceptions in the C# too.
                Some(2 | 3 | 32 | 33 | 39 | 112 | 1224) => l::t("Windows couldn't read or write the file (another program may be using it)."),
                Some(_) => l::t("Something went wrong. Try again in a moment."),
                None if is_fail_kind(fail, JSON_EXCEPTION) => l::t("Roblox sent an answer Bloxkeep didn't expect. Try again in a moment."),
                None => l::t("Something went wrong. Try again in a moment."),
            }
        }
    }
}

/// Windows' error number in an io::Error's text ("… (os error 2)"), as Win32Exception.NativeErrorCode.
fn os_error_code(text: &str) -> Option<i32> {
    let at = text.rfind("(os error ")?;
    text[at + 10..].trim_end_matches(')').trim().parse().ok()
}

/// Why Roblox wouldn't start, for the person (a notification, Discord, /pc, the activity log). Windows' own message
/// names the program it was asked to start, and with the roblox-player: link handed to Windows that is the link, sign-in
/// ticket and all (.NET's "An error occurred trying to start process '…'", checked 2026-10-30): never shown. Bloxkeep's
/// own sentences (a launcher not installed) are, the ticket-like parts hidden all the same.
pub fn start_error_text(fail: &Fail) -> String {
    log::note(&format!("Roblox didn't start: {}", start_error_log(fail)));
    match os_error_code(fail.message()) {
        Some(code) => l::f("Windows couldn't start Roblox (error {0}).", &[&code]),
        None => crate::core::roblox_launcher::hide_ticket(fail.message()),
    }
}

pub fn start_error_log(fail: &Fail) -> String {
    let kind = match fail {
        Fail::User(_) => "UserError",
        Fail::Roblox { .. } => "RobloxApiException",
        Fail::Other(_) => if os_error_code(fail.message()).is_some() { "Win32Exception" } else { "Exception" },
    };
    let code = os_error_code(fail.message()).unwrap_or(0);
    format!("{kind} {code}: {}", crate::core::roblox_launcher::hide_ticket(fail.message()))
}

fn quoted(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| "\"\"".into())
}

impl App {
    /// `Self::ids_of` and the rest, for partials that call AppController's static helpers through the type.
    pub fn ids_of(a: &Value) -> Vec<Guid> {
        ids_of(a)
    }

    pub fn bool_of(a: &Value, name: &str) -> bool {
        bool_of(a, name)
    }

    pub fn str_of(a: &Value, name: &str) -> String {
        str_of(a, name)
    }

    pub fn error_text(fail: &Fail) -> String {
        error_text(fail)
    }

    /// The constructor's work (AppController(owner, postJson)); the App itself is made by mod.rs's `App::start`,
    /// so this one is `init`.
    pub fn init(self: &Rc<Self>, owner: isize, post: Rc<dyn Fn(&str)>) {
        self.controller.owner.set(owner);
        *self.controller.post.borrow_mut() = Some(post);
        // _procTimer.Tick += OnProcTimer; _procTimer.Start();
        {
            let app = self.clone();
            rt::spawn(async move {
                loop {
                    // Waited in short steps: the time between checks may be shortened meanwhile (a launch starts
                    // while the app idles at 4 s, and its window must not wait those out).
                    let mut waited = 0u64;
                    loop {
                        let want = app.controller.proc_interval_ms.get();
                        if waited >= want {
                            break;
                        }
                        let step = (want - waited).min(250);
                        rt::sleep(Duration::from_millis(step)).await;
                        waited += step;
                    }
                    if app.controller.disposed.get() {
                        break;
                    }
                    app.on_proc_timer();
                }
            });
        }
        // _updater.Changed += OnUpdaterChanged: the updater reports from its own threads.
        self.hook_updater();
        // Checked here, not at the first tick: an update found before unlock replaces the lock screen, best before anyone types.
        let updater = self.updates.updater.borrow().clone();
        if prefs::lock().check_updates && updater.supported() {
            self.updates.next_update_check.set(Stamp::utc_now().plus_secs(UPDATE_CHECK_INTERVAL.as_secs_f64()));
            rt::spawn(async move {
                let _ = rt::blocking(move || updater.check()).await;
            });
        }
        self.watch_foreground();
        self.watch_drags();
        // Multi-Roblox before the master password (see Prefs.MultiRobloxAtStart).
        let at_start = prefs::lock().multi_roblox_at_start;
        if at_start && !multi_instance::armed() {
            multi_instance::enable();
        }
        // PORT: SystemEvents.PowerModeChanged and TimeChanged come as WM_POWERBROADCAST and WM_TIMECHANGE to the shell
        // window (shell_form's wnd_proc); NetworkChange.NetworkAvailabilityChanged is this watcher (AppController.Power).
        super::power::watch_network();
        self.note_version();
    }

    /// While the window is minimised, poll less often; Roblox status comes back up to date when it shows again.
    pub fn set_background(&self, value: bool) {
        if self.controller.background.get() && !value {
            self.refresh_presence_soon(Duration::ZERO);
        }
        self.controller.background.set(value);
        self.controller.proc_interval_ms.set(if value { 5000 } else { 1500 });
    }

    /// The vault is open and no lock screen is up: what the mini window and the window's cover picture may show.
    /// The idle lock screen counts as locked (2026-10-30 review: the mini window offered "Close all" behind it).
    pub fn unlocked(&self) -> bool {
        self.open().is_some()
    }

    /// The sleeping page went (the browser closed, or a new page): what waited for it goes too.
    pub fn forget_sleep(&self) {
        self.controller.page_asleep.set(false);
        self.controller.asleep_events.borrow_mut().clear();
    }

    /// When a join not in a server yet is asked about (CheckJoinAsync). A join waiting for room logs nothing, like a
    /// refused one: on 2026-10-16 four accounts waited 17-38 s silently, then got in. Asking Roblox in that wait might
    /// upset it, so wait past the longest seen; an account the game is known to have banned is asked at once.
    pub fn join_check_after(&self, account: &Account, target: &GameTarget) -> Duration {
        // A chosen server is asked about as a party's is (SERVER_CHECK): one that had ended sat on Roblox's "This experience
        // has ended" for a minute before another was tried.
        if self.game_ban_of(account, Some(target.place_id)).is_some() {
            Duration::from_secs(5)
        } else if target.job_id.as_deref().is_some_and(|j| !j.is_empty()) {
            super::launching::SERVER_CHECK
        } else {
            Duration::from_secs(40)
        }
    }

    // ───────────────────────────── bridge ─────────────────────────────

    pub fn post(&self, json: &str) {
        if self.controller.disposed.get() {
            return;
        }
        if self.controller.page_asleep.get() {
            // A reply always waits, or the page's command would never finish; only the newest events are kept.
            let mut events = self.controller.asleep_events.borrow_mut();
            events.push(json.to_string());
            if events.len() > MAX_ASLEEP_EVENTS
                && let Some(oldest) = events.iter().position(|j| j.starts_with("{\"type\":\"event\""))
            {
                events.remove(oldest);
            }
            return;
        }
        let post = self.controller.post.borrow().clone();
        if let Some(post) = post {
            post(json);
        }
    }

    /// An event for the page; none while the screen is locked (a toast could name an account on the lock screen).
    pub fn emit(&self, name: &str, data: Value) {
        if !self.auto_lock.screen_locked.get() || name == "saveView" {
            self.post(&format!("{{\"type\":\"event\",\"name\":{},\"data\":{}}}", quoted(name), data));
        }
    }

    pub fn toast(&self, text: &str, kind: &str) {
        // A warning or error the person saw is in error.log too (names and ids scrubbed): reports carry it.
        if kind == "warn" || kind == "error" {
            log::note_file(&format!("Shown ({kind}): {}", self.scrub(text)));
        }
        self.emit("toast", json!({ "text": text, "kind": kind }));
    }

    pub fn progress(&self, text: Option<&str>) {
        // The clear names nobody, so it goes through the lock screen too: a check that ends behind it would otherwise
        // leave its banner up after unlocking.
        if text.is_none() {
            self.post(&format!("{{\"type\":\"event\",\"name\":\"progress\",\"data\":{}}}", json!({ "text": Value::Null })));
            return;
        }
        self.emit("progress", json!({ "text": text }));
    }

    pub fn push_state(&self) {
        self.rename_websites();
        if self.controller.page_attached.get() && !self.controller.page_asleep.get() {
            let state = self.build_state();
            self.post_state(&state, true);
        }
    }

    /// For timers: an identical state isn't sent again (re-rendering is costly with hundreds of accounts). Commands
    /// always push, so a control the page changed on its own is put back when the app refused the change.
    pub fn push_state_if_changed(&self) {
        if self.controller.page_attached.get() && !self.controller.page_asleep.get() {
            let state = super::state::with_kept_probes(|| self.build_state());
            self.post_state(&state, false);
        }
    }

    fn post_state(&self, state: &Value, force: bool) {
        let json = state.to_string();
        if !force && self.controller.last_state.borrow().as_deref() == Some(json.as_str()) {
            return;
        }
        *self.controller.last_state.borrow_mut() = Some(json.clone());
        self.post(&format!("{{\"type\":\"event\",\"name\":\"state\",\"data\":{json}}}"));
    }

    /// A message from the interface, {id, cmd, args}: its reply goes out through `post`. This is the boundary to the
    /// page: every failure becomes an error reply instead of a crash.
    pub async fn handle_message(self: &Rc<Self>, json: &str) {
        let mut id: i64 = 0;
        let mut asked = String::new();
        let outcome: Res<Value> = async {
            let root: Value = match serde_json::from_str(json) {
                Ok(v) => v,
                Err(e) => {
                    log::write(&e);
                    return Err(Fail::user(l::t("Roblox sent an answer Bloxkeep didn't expect. Try again in a moment.")));
                }
            };
            id = root.get("id").and_then(Value::as_i64).filter(|v| i32::try_from(*v).is_ok()).ok_or_else(|| Fail::other("The given key 'id' was not present."))?;
            let cmd = match root.get("cmd") {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Null) => String::new(),
                _ => return Err(Fail::other("The requested operation requires an element of type 'String'.")),
            };
            let args = root.get("args").cloned().unwrap_or(Value::Null);
            asked = cmd.clone();
            self.handle(&cmd, &args).await
        }
        .await;
        let reply = match outcome {
            Ok(result) => format!("{{\"type\":\"reply\",\"id\":{id},\"ok\":true,\"result\":{result}}}"),
            Err(fail) => {
                // An unexpected failure is logged by error_text; which command it came from goes just before it.
                if matches!(fail, Fail::Other(_)) && !vault::is_invalid_password(&fail) {
                    log::note(&format!("Command \"{asked}\" failed:"));
                } else if !self.withheld(&asked) {
                    // What the person was told goes in error.log too, so a report never needs a screenshot to say why
                    // (error.log must cover every case). Names and ids scrubbed.
                    let why = if vault::is_invalid_password(&fail) { "wrong password".to_string() } else { self.scrub(&error_text(&fail)) };
                    log::note_file(&format!("Command \"{asked}\" refused: {why}"));
                }
                format!("{{\"type\":\"reply\",\"id\":{id},\"ok\":false,\"error\":{}}}", quoted(&error_text(&fail)))
            }
        };
        self.post(&reply);
    }

    /// A command the lock screen doesn't send, while it is up: refused, and its answer withheld.
    fn withheld(&self, cmd: &str) -> bool {
        self.auto_lock.screen_locked.get() && !SCREEN_LOCK_COMMANDS.contains(&cmd)
    }

    /// One command, its answer or why not (DispatchAsync).
    pub async fn handle(self: &Rc<Self>, cmd: &str, a: &Value) -> Res<Value> {
        // AppController.Gpui.cs: the GPUI window's own command, answered before the page's commands (and their lock
        // screen rule) were looked at, as HandleGpuiMessageAsync did (see gpui.rs).
        if cmd == "robloxSignIn" {
            return done(self.roblox_sign_in(a));
        }
        if self.withheld(cmd) {
            return Err(Fail::user(l::t("Bloxkeep is locked.")));
        }
        let result = self.dispatch_command(cmd, a).await?;
        // The lock screen came up while it ran: what it did stands, but its answer doesn't reach the lock screen.
        if self.withheld(cmd) {
            return Err(Fail::user(l::t("Bloxkeep is locked.")));
        }
        Ok(result)
    }

    async fn dispatch_command(self: &Rc<Self>, cmd: &str, a: &Value) -> Res<Value> {
        match cmd {
            "getState" => done(self.initial_state()),
            "console" => {
                let since = a.get("version").and_then(Value::as_u64).unwrap_or(u64::MAX);
                Ok(match log::console_lines(since) {
                    Some((version, lines)) => json!({ "version": version, "lines": lines }),
                    None => Value::Null,
                })
            }
            "consoleClear" => {
                log::console_clear();
                Ok(Value::Null)
            }
            "keepView" => done(self.keep_view(&str_of(a, "view"), bool_of(a, "restart"))),
            "pageBusy" => done(self.set_page_busy(bool_of(a, "busy"))),
            "pageShown" => done(self.on_page_shown().await),
            "checkUpdates" => done(self.check_updates().await),
            "applyUpdate" => done(self.apply_update().await),
            "updateAtLock" => done(self.set_update_at_lock(bool_of(a, "go"))),
            "setAutoUpdate" => done(self.set_auto_update(bool_of(a, "enabled"))),
            "setBetaUpdates" => done(self.set_beta_updates(bool_of(a, "enabled"))),
            "setCountOnline" => done(self.set_count_online(bool_of(a, "enabled"))),
            "setBoardName" => done(self.set_board_name(&str_of(a, "name"))),
            "communityNow" => done(self.community_now()),
            "whatsNew" => {
                let current = self.updates.updater.borrow().current_version().to_string();
                done(super::updates::releases_json(&crate::core::whats_new::between(None, &current)))
            }
            "seenWhatsNew" => done(self.seen_whats_new()),
            "setLanguage" => done(self.set_language(&str_of(a, "language"))),
            "tryLanguage" => done(self.try_language(&str_of(a, "language"))),
            "setTheme" => done(self.set_theme(&str_of(a, "theme"))),
            "setSystemThemes" => done(self.set_system_themes(&str_of(a, "light"), &str_of(a, "dark"))),
            "setCustomTheme" => done(self.set_custom_theme(
                &str_of(a, "background"),
                &str_of(a, "accent"),
                if has(a, "fade", JsonKind::Number) { int32_of(a, "fade") } else { None },
                if has(a, "blur", JsonKind::Number) { int32_of(a, "blur") } else { None },
            )),
            "pickThemePicture" => done(self.pick_theme_picture().await),
            "clearThemePicture" => done(self.clear_theme_picture()),
            "useWallpaperEngine" => done(self.use_wallpaper_engine(&str_of(a, "path"), &str_of(a, "accent"))),
            "saveTheme" => done(self.save_theme(&str_of(a, "name"))),
            "useSavedTheme" => done(self.use_saved_theme(&str_of(a, "id"))),
            "deleteSavedTheme" => done(self.delete_saved_theme(&str_of(a, "id"))),
            "exportTheme" => done(self.export_theme(&str_of(a, "id")).await),
            "importTheme" => done(self.import_theme().await),
            "setCloseToTray" => done(self.set_close_to_tray(bool_of(a, "enabled"))),
            "setStartWithWindows" => done(self.set_start_with_windows(bool_of(a, "enabled"))),
            "setHotkeys" => done(self.set_hotkeys(bool_of(a, "enabled"))),
            "setHideNames" => done(self.set_hide_names(bool_of(a, "enabled"))),
            "createVault" => done(self.create_vault(&str_of(a, "password"), &str_of(a, "name")).await),
            "unlock" => done(self.unlock(&str_of(a, "password")).await),
            "lock" => done(self.lock()),
            "changePassword" => done(self.change_password(&str_of(a, "current"), &str_of(a, "next")).await),
            "addViaBrowser" => done(self.add_via_browser(bool_of(a, "create")).await),
            "quickLoginStart" => done(self.quick_login_start(realm::normalize(Some(&str_of(a, "realm")))).await),
            "quickLoginPoll" => done(self.quick_login_poll().await),
            "quickLoginCancel" => done(self.quick_login_cancel()),
            "loginPassword" => {
                let (username, password, realm) = (str_of(a, "username"), str_of(a, "password"), realm::normalize(Some(&str_of(a, "realm"))));
                let app = self.clone();
                done(self.login_step(move || async move { app.login_password(&username, &password, realm).await }, true).await)
            }
            "loginFinish" => {
                let app = self.clone();
                done(self.login_step(move || async move { app.login_finish().await }, false).await)
            }
            "loginTwoStep" => {
                let (method, code) = (str_of(a, "method"), str_of(a, "code"));
                let app = self.clone();
                done(self.login_step(move || async move { app.login_two_step(&method, &code).await }, false).await)
            }
            "loginTwoStepSend" => {
                let method = str_of(a, "method");
                let app = self.clone();
                done(self.login_step(move || async move { app.login_two_step_send(&method).await }, false).await)
            }
            "loginTwoStepCode" => done(self.login_two_step_code()),
            "loginCancel" => done(self.login_cancel()),
            "loginCaptcha" => {
                let app = self.clone();
                done(self.login_step(move || async move { app.login_captcha().await }, false).await)
            }
            "addViaCookie" => done(self.add_via_cookie(a).await),
            "vaultImportPick" => done(
                self.vault_import_pick(if str_of(a, "from") == "ram" { super::vault_import::ImportFrom::Ram } else { super::vault_import::ImportFrom::Vault }).await,
            ),
            "listImportOpen" => done(self.list_import_open(&str_of(a, "text")).await),
            "vaultImportOpen" => done(self.vault_import_open(&str_of(a, "password")).await),
            "vaultImportApply" => done(self.vault_import_apply(a)),
            "vaultImportCancel" => done(self.drop_vault_import(false)),
            "updateAccount" => done(self.update_account_command(a).await),
            "setAutoRejoin" => done(self.set_auto_rejoin(&ids_of(a), bool_of(a, "enabled"))),
            "cancelRejoin" => done(self.cancel_rejoin(id_of(a))),
            "setUseRobloxConfig" => done(self.set_use_roblox_config(&ids_of(a), bool_of(a, "enabled"))),
            "setFpsCap" => done(self.set_fps_cap(&ids_of(a), long_of(a, "fps").clamp(0, i32::MAX as i64) as i32)),
            "setGroup" => done(self.set_group(&ids_of(a), &str_of(a, "group"))),
            "setMain" => done(self.set_main(&ids_of(a), bool_of(a, "on"))),
            "makeFriends" => done(self.make_friends(&ids_of(a)).await),
            "friendAct" => done(self.friend_act(a).await),
            "reveal" => done(self.reveal(id_of(a), &str_of(a, "field"))),
            "totpCode" => done(self.authenticator_code(id_of(a))),
            "totpCheck" => done(App::check_authenticator(&str_of(a, "key"))),
            "deleteAccounts" => done(self.delete_accounts(&ids_of(a))),
            "checkAccounts" => done(self.check_accounts(&ids_of(a)).await),
            "solveBotCheck" => done(self.solve_bot_check(id_of(a)).await),
            "switchServer" => done(self.switch_server(&ids_of(a)).await),
            "hideWindows" => done(self.hide_windows(&ids_of(a))),
            "showWindows" => done(self.show_windows(&ids_of(a))),
            "refreshPresence" => done(self.refresh_presence().await),
            "copy" => done(self.copy(id_of(a), &str_of(a, "field"))),
            "resolveGame" => done(self.resolve_game(&str_of(a, "input")).await),
            "searchGames" => done(self.search_games(&str_of(a, "query")).await),
            "gameIcons" => done(self.game_icons(a).await),
            "gameLists" => done(self.game_lists().await),
            "servers" => done(self.servers(long_of(a, "placeId"), bool_of(a, "fewest"), opt_str(a, "cursor")).await),
            "gameDetails" => done(self.game_details(long_of(a, "placeId")).await),
            "gameSocialLinks" => done(self.game_social_links(long_of(a, "universeId"), id_of(a)).await),
            "openGameWiki" => done(App::open_game_wiki(&str_of(a, "name"))),
            "openGamePage" => done(App::open_game_page(long_of(a, "placeId"))),
            "serverRegions" => done(self.server_regions(long_of(a, "placeId"), &ids_of(a), a.get("ask").and_then(Value::as_u64).unwrap_or(60) as usize).await),
            "serverFacts" => {
                let jobs: Vec<String> = a.get("jobs").and_then(Value::as_array).into_iter().flatten().filter_map(|j| j.as_str().map(str::to_string)).collect();
                done(self.server_facts(long_of(a, "placeId"), jobs, &ids_of(a)).await)
            }
            "gameCodes" => {
                let (name, universe, place) = (str_of(a, "name"), long_of(a, "universe"), long_of(a, "placeId"));
                done(crate::core::rt::blocking(move || crate::core::game_codes::codes(&name, universe, place, &crate::core::base::Cancel::new())).await)
            }
            "friendServers" => done(self.friend_servers(long_of(a, "placeId"), &ids_of(a)).await),
            "privateServers" => done(self.private_servers(long_of(a, "placeId"), &ids_of(a), bool_of(a, "fresh")).await),
            "openGameServers" => done(self.open_game_servers(id_of(a), long_of(a, "placeId"))),
            "gameCommunity" => done(self.game_community(long_of(a, "groupId"), id_of(a), &str_of(a, "action")).await),
            "ownedServers" => done(self.owned_servers(bool_of(a, "fresh")).await),
            "pinGame" => done(self.toggle_pin(long_of(a, "placeId"), long_of(a, "universeId"), &str_of(a, "name"))),
            "saveServer" => done(self.save_server(a)),
            "deleteServer" => done(self.delete_server(id_of(a))),
            "copyServerCode" => done(self.copy_server_code(id_of(a))),
            "launch" => done(self.launch(a).await),
            "cancelLaunch" => done(self.cancel_launch()),
            "savePreset" => done(self.save_preset(a).await),
            "deletePreset" => done(self.delete_preset(id_of(a))),
            "runPreset" => done(self.run_preset(id_of(a), bool_of(a, "force")).await),
            "closePreset" => done(self.close_preset_windows(id_of(a))),
            "presetDiscordTest" => done(self.send_preset_discord_test(id_of(a), opt_str(a, "url")).await),
            "duplicatePreset" => done(self.duplicate_preset(id_of(a))),
            "setPresetOrder" => done(self.set_preset_order(&ids_of(a))),
            "setPresetSchedule" => done(self.set_preset_schedule(id_of(a), bool_of(a, "enabled"))),
            "presetIcons" => done(self.preset_icons().await),
            "saveMacro" => done(self.save_macro(a)),
            "deleteMacro" => done(self.delete_macro(id_of(a))),
            "duplicateMacro" => done(self.duplicate_macro(id_of(a))),
            "moveMacro" => done(self.move_macro(id_of(a), long_of(a, "to") as i32)),
            "runMacro" => {
                let accounts = if has(a, "accountIds", JsonKind::Array) { Some(guid_list(a, "accountIds")) } else { None };
                done(self.run_macro(id_of(a), accounts.as_deref()))
            }
            "stopMacros" => {
                let stop = id_of(a);
                done(self.stop_macros(if stop != Guid::nil() { Some(stop) } else { None }, parse_guid(&str_of(a, "accountId")), bool_of(a, "trial")))
            }
            "macroPicture" => done(self.macro_picture(parse_guid(&str_of(a, "accountId")).unwrap_or(Guid::nil())).await),
            "macroTest" => done(self.macro_test(a).await),
            "recordMacro" => done(self.record_macro(parse_guid(&str_of(a, "accountId")).unwrap_or(Guid::nil()))),
            "stopRecording" => done(self.stop_recording().await),
            "importMacro" => done(self.import_macro().await),
            "pickModFile" => done(self.pick_mod_file().await),
            "exportMacro" => done(self.export_macro(id_of(a)).await),
            "showPanel" => done(self.show_panel(id_of(a), true)),
            "hidePanel" => done(self.show_panel(id_of(a), false)),
            "setOption" => {
                // PORT: `default(JsonElement)` (no "value" given) is JSON null here.
                let value = a.as_object().and_then(|o| o.get("value")).cloned().unwrap_or(Value::Null);
                done(self.set_option(id_of(a), &str_of(a, "name"), &value, false))
            }
            "editOnGame" => done(self.edit_on_game_command(a)),
            "pickOnGame" => done(self.pick_on_game_command(a).await),
            "macroTryRun" => done(self.macro_try_run_command(a)),
            "macroTemplates" => done(self.macro_templates_dto()),
            "macroWatch" => done(self.macro_watch_command(a)),
            "setLaunchOrder" => done(self.set_launch_order(&ids_of(a))),
            "closeAccounts" => done(self.close_accounts(&ids_of(a), bool_of(a, "force"))),
            "closeAll" => done(self.close_all(bool_of(a, "force"))),
            "arrangeWindows" => done(self.arrange_windows(long_of(a, "monitor") as i32, false, false).await),
            "setSettings" => done(self.set_settings(a)),
            "setRendering" => done(self.set_rendering(bool_of(a, "gpu")).await),
            "disableTrayAutostart" => done(self.disable_tray_autostart()),
            "removeRecent" => done(self.remove_recent(long_of(a, "placeId"))),
            "openProfile" => done(App::open_url(&format!("https://www.roblox.com/users/{}/profile", long_of(a, "userId")))),
            "openRobloxPage" => {
                let kind = str_of(a, "kind");
                let url = if kind == "trades" {
                    "https://www.roblox.com/trades".to_string()
                } else if kind == "groupCreate" {
                    "https://www.roblox.com/communities/create".to_string()
                } else if kind == "event" {
                    // (A game's event: its id is a long number Roblox writes as text.)
                    let id: String = str_of(a, "id").chars().filter(char::is_ascii_digit).collect();
                    format!("https://www.roblox.com/events/{id}")
                } else {
                    let path = match kind.as_str() {
                        "bundle" => "bundles",
                        "group" => "communities",
                        "badge" => "badges",
                        "pass" => "game-pass",
                        "library" => "library",
                        _ => "catalog",
                    };
                    format!("https://www.roblox.com/{path}/{}", long_of(a, "id"))
                };
                done(App::open_url(&url))
            }
            // (page "settings": on Roblox's own Settings page, for what it keeps there only: privacy, security…)
            "openWebsite" => done(self.open_website(id_of(a), (str_of(a, "page") == "settings").then_some(super::website::SETTINGS_URL))),
            "openAppeal" => done(self.open_website(id_of(a), Some(super::website::APPEAL_URL))),
            // Marketplace › Watching: a change to a watched thing, said as Bloxkeep's other news is.
            "notifyItem" => {
                self.notify(&str_of(a, "text"), "info", "item", Guid::nil(), true, Some(&str_of(a, "key")));
                done(Value::Null)
            }
            "openModPage" => done(App::open_url(&format!("https://gamebanana.com/mods/{}", long_of(a, "id")))),
            // Roblox › Mods: every file a mod gave back to Roblox's own, now.
            "restoreMods" => {
                let (back, left) = crate::core::rt::blocking(crate::core::roblox_mods::restore_all).await;
                done(serde_json::json!({ "back": back, "left": left }))
            }
            "openWebsiteBlank" => done(self.open_blank_website()),
            "openDataFolder" => done(App::open_data_folder()),
            "showPath" => done(self.show_path(&str_of(a, "which"))),
            "monitorArea" => done(self.monitor_area_command(a)),
            "monitorAct" => done(self.monitor_act_command(a)),
            "monitorLane" => done(self.monitor_lane_command(a)),
            "setNotifications" => done(self.set_notifications(a)),
            "setDailySignInCheck" => done(self.set_daily_sign_in_check(bool_of(a, "enabled"))),
            "skipConfirm" => done(self.skip_confirm(&str_of(a, "key"))),
            "resetSkipConfirm" => done(self.reset_skip_confirm()),
            "toggleMini" => done(self.toggle_mini()),
            "window" => done(self.control_window(&str_of(a, "action"))),
            "setBackup" => done(self.set_backup(a)),
            "pickBackupFolder" => done(self.pick_backup_folder().await),
            "openBackupFolder" => done(App::open_backup_folder()),
            "backupSuggestions" => done(self.refresh_backup_suggestions()),
            "backupNow" => done(self.backup_now().await),
            "restoreBackup" => done(self.restore_backup().await),
            "pickVault" => done(self.show_vault_picker()),
            "closeVaultPicker" => done(self.close_vault_picker()),
            "selectVault" => done(self.select_vault(&str_of(a, "id"))),
            "addVault" => done(self.add_vault()),
            "cancelAddVault" => done(self.cancel_add_vault()),
            "renameVault" => done(self.rename_vault(&str_of(a, "id"), &str_of(a, "name"))),
            "pickVaultPicture" => done(self.pick_vault_picture(&str_of(a, "id")).await),
            "clearVaultPicture" => done(self.clear_vault_picture(&str_of(a, "id"))),
            "removeVault" => done(self.remove_vault(&str_of(a, "id"))),
            "setAskVault" => done(self.set_ask_vault(bool_of(a, "enabled"))),
            "pickLauncher" => done(self.pick_launcher().await),
            "playStats" => {
                let game = opt_str(a, "game").filter(|g| !g.is_empty());
                done(self.play_stats_async(long_of(a, "days") as i32, game).await)
            }
            "todayStrips" => done(self.today_strips()),
            "activity" => done(self.activity_list(a)),
            "exportActivity" => done(self.export_activity(a).await),
            "findPlayer" => done(self.find_player(a).await),
            "joinServer" => done(self.join_server(a).await),
            "forgetPlayer" => done(self.forget_player(&str_of(a, "name"))),
            "setDiscord" => done(self.set_discord(a)),
            "setDiscordPresence" => done(self.set_discord_presence(bool_of(a, "enabled"))),
            "setDiscordWebhook" => done(self.set_discord_webhook(&str_of(a, "url"))),
            "setRemote" => done(self.set_remote(bool_of(a, "enabled"))),
            "remoteCode" => done(self.remote_new_code()),
            "remoteAllow" => done(self.remote_allow(bool_of(a, "allow"), opt_str(a, "id").as_deref())),
            "revealDiscordWebhook" => done(self.reveal_discord_webhook()),
            "discordTest" => done(self.send_discord_now(false).await),
            "setPhone" => done(self.set_phone(a)),
            "feedCards" => done(self.feed_cards_dto()),
            "wikiGames" => done(self.wiki_games(bool_of(a, "again")).await),
            "gameLooks" => done(self.game_looks(a).await),
            "playedGames" => done(self.played_games()),
            "playSessions" => done(self.play_sessions_raw()),
            "screenTime" => done(self.screen_time_of(long_of(a, "userId"))),
            "wikiGame" => done(self.wiki_game(&str_of(a, "id")).await),
            "wikiOpen" => done(self.open_wiki_link(&str_of(a, "url"))),
            "wikiApi" => done(App::wiki_api(a).await),
            "wikiFind" => done(App::wiki_find(a).await),
            "wikiAbout" => done(App::wiki_about(a).await),
            "wikiPick" => done(self.set_wiki_pick(a)),
            "wikiMark" => done(self.mark_wiki_page(a)),
            "feedFollow" => done(self.set_feed_follow(&str_of(a, "game"), bool_of(a, "on"))),
            "feedRead" => done(self.mark_feed_read(&str_of(a, "game"))),
            "feedSeen" => done(self.mark_feeds_seen()),
            "feedAlert" => done(self.set_feed_alert(a)),
            "feedOpen" => done(self.open_feed_link(&str_of(a, "url"))),
            "phoneTest" => done(self.send_phone_test().await),
            "discordSummaryNow" => done(self.send_discord_now(true).await),
            "openReleaseNotes" => done(App::open_release_notes(&str_of(a, "version"))),
            "hideRobloxClosed" => done(self.hide_roblox_closed()),
            "multiOnly" => done(self.multi_only(bool_of(a, "on"))),
            "updateRoblox" => done(self.update_roblox(false)),
            "updateAndReopen" => done(self.update_and_reopen()),
            "openRobloxWatch" => done(App::open_url(crate::core::roblox_watch::DOCS_URL)),
            // The user guide on GitHub, in the app's language (docs/guide.md, docs/guide.vi.md).
            "openGuide" => done(App::open_url(&format!(
                "https://github.com/Bloxkeep/roblox-account-manager/blob/main/docs/guide{}.md",
                if l::language() == "vi" { ".vi" } else { "" }
            ))),
            "openGitHub" => done(App::open_url("https://github.com/Bloxkeep/roblox-account-manager")),
            // The bot redirects to a working community invite (services/DiscordBot/worker invite()), so this address never changes.
            "openDiscord" => done(App::open_url("https://bloxkeep-bot.bloxkeep-app.workers.dev/discord")),
            // Bloxkeep's app added to the person's own Discord account, so /pc works in any DM (Control from Discord).
            "openDiscordApp" => done(App::open_url("https://discord.com/oauth2/authorize?client_id=1561788269933498408&integration_type=1&scope=applications.commands")),
            "feedbackPreview" => done(self.feedback_preview(a)),
            "sendFeedback" => done(self.send_feedback(a).await),
            _ => Err(Fail::user(l::f("Unknown command: {0}", &[&cmd]))),
        }
    }
}
