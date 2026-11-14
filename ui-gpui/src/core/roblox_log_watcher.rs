//! Follows Roblox's client logs (every client writes one, whichever launcher started it) to know within a second or two
//! when a window joins, changes server, leaves or is kicked. Each log is tied to its window by Windows' Restart Manager
//! (who has the file open). The format is not an official API, so Roblox's presence stays as the fallback.
use std::{
    collections::{HashMap, HashSet, VecDeque},
    io::{Read as _, Seek, SeekFrom},
    os::windows::fs::OpenOptionsExt,
    sync::{
        LazyLock, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    thread::JoinHandle,
};

use regex::Regex;
use windows_sys::Win32::{
    Foundation::ERROR_MORE_DATA,
    System::RestartManager::{CCH_RM_SESSION_KEY, RM_PROCESS_INFO, RmEndSession, RmGetList, RmRegisterResources, RmStartSession},
};

use super::{
    base::{Kind, Stamp},
    log,
    win_kit::{local_app_data, wide},
};

/// `status`: joining, ingame, left (back in the app, by choice), disconnected, or signedout (signed out inside the window).
/// `reason`: 278 = idle for 20 minutes.
/// `chosen_place`: the game the person last started in this window from Roblox's own screens; None while still in the
/// game it was opened into. Teleports inside that game keep it.
/// `banned_by_game`: Roblox's ban API, rather than a kick: Roblox gives both code 267.
/// `teleport_started`: Roblox's doTeleport, until the next server is reached.
/// `never_joined`: left before any server took it (2026-10-21: ten such launches, none logging why; Roblox's join error
/// lines are off).
#[derive(Clone, Debug, PartialEq)]
pub struct ClientState {
    pub status: String,
    pub place_id: Option<i64>,
    pub job_id: Option<String>,
    pub reason: Option<i32>,
    pub at: Stamp,
    pub chosen_place: Option<i64>,
    pub person_started: bool,
    pub kick_message: Option<String>,
    pub banned_by_game: bool,
    pub teleport_started: Option<Stamp>,
    pub never_joined: bool,
    /// When a timed game ban in its kick line ends (`ban_end_of`); None for a permanent one.
    pub ban_ends: Option<Stamp>,
}

impl ClientState {
    /// new ClientState(Status, PlaceId, JobId, Reason, At) with the other fields at their defaults.
    pub fn new(status: &str, place_id: Option<i64>, job_id: Option<String>, reason: Option<i32>, at: Stamp) -> ClientState {
        ClientState {
            status: status.to_string(),
            place_id,
            job_id,
            reason,
            at,
            chosen_place: None,
            person_started: false,
            kick_message: None,
            banned_by_game: false,
            teleport_started: None,
            never_joined: false,
            ban_ends: None,
        }
    }
}

static SERIAL: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
pub(crate) struct Tail {
    /// Which Tail this is (C# compared the objects themselves: ReferenceEquals).
    serial: u64,
    path: String,
    pid: i32,
    offset: i64,
    partial: String,
    pub(crate) state: Option<ClientState>,
    pub(crate) leaving: bool,
    /// When the last teleport line was logged; only a disconnect soon after it belongs to that teleport.
    pub(crate) teleport_at: Option<Stamp>,
    pending_reason: Option<i32>,
    pending_reason_rank: i32,
    shown_reason_rank: i32,
    /// The game's kick line, logged before the disconnect it explains.
    pending_kick_message: Option<String>,
    pending_ban: bool,
    pending_ban_ends: Option<Stamp>,
    /// A game start that isn't the window's first: the game its "Joining game" names is one the person chose.
    pub(crate) own_start: bool,
    /// Servers connected to ("serverId:") and not yet left (a disconnect line ends one).
    pub(crate) connections: i32,
    closed_by_new_window: Option<Stamp>,
    out_of_memory: Option<Stamp>,
    closed_in_order: Option<Stamp>,
    pub(crate) join_challenged: Option<Stamp>,
    /// Roblox's updater was told to install in the background: this client goes on (see UPDATE_THREAD_MARK).
    background_update: bool,
    update_hand_off: Option<Stamp>,
    /// The signed-in account, from the join report (see JOIN_REPORT_MARK).
    user_id: Option<i64>,
    /// Shown as a drop by a reason line (see drop_on_reason): the disconnect line after it adds nothing.
    pub(crate) dropped_by_reason: bool,
    /// Roblox's own words for error.log (see keep_line): lines that tell why a window ended, and the last few lines.
    said: VecDeque<Kept>,
    other: VecDeque<Kept>,
    last_lines: VecDeque<Kept>,
    lines_read: u64,
}

impl Tail {
    pub(crate) fn new(path: String, pid: i32) -> Tail {
        Tail {
            serial: SERIAL.fetch_add(1, Ordering::Relaxed),
            path,
            pid,
            offset: 0,
            partial: String::new(),
            state: None,
            leaving: false,
            teleport_at: None,
            pending_reason: None,
            pending_reason_rank: 0,
            shown_reason_rank: 0,
            pending_kick_message: None,
            pending_ban: false,
            pending_ban_ends: None,
            own_start: false,
            connections: 0,
            closed_by_new_window: None,
            out_of_memory: None,
            closed_in_order: None,
            join_challenged: None,
            background_update: false,
            update_hand_off: None,
            user_id: None,
            dropped_by_reason: false,
            said: VecDeque::new(),
            other: VecDeque::new(),
            last_lines: VecDeque::new(),
            lines_read: 0,
        }
    }

    /// The log came back shorter than what was read (cut short, or replaced under the same name): read again from the
    /// start as a log just found is, so its lines don't apply on top of what the old content left (a half line, an
    /// open server count, a state, a kick waiting for its disconnect).
    fn restart(&mut self) {
        let fresh = Tail::new(std::mem::take(&mut self.path), self.pid);
        *self = Tail { serial: self.serial, ..fresh };
    }
}

/// The usual client's, the Microsoft Store edition's and Roblox VNG's. BLOXKEEP_ROBLOX_LOGS (tests only) points at
/// made-up logs instead.
static LOG_DIRS: OnceLock<Vec<String>> = OnceLock::new();

fn log_dirs() -> &'static Vec<String> {
    LOG_DIRS.get_or_init(|| match std::env::var("BLOXKEEP_ROBLOX_LOGS") {
        Ok(test_logs) if !test_logs.is_empty() => vec![test_logs],
        _ => {
            let local = local_app_data();
            vec![
                local.join("Roblox").join("logs").to_string_lossy().into_owned(),
                local.join("RobloxPCGDK").join("logs").to_string_lossy().into_owned(),
                // Roblox VNG's client keeps its own folder (seen 2026-10-19), and opens global accounts too.
                local.join("RobloxPCVNG").join("logs").to_string_lossy().into_owned(),
            ]
        }
    })
}

pub fn folders() -> &'static [String] {
    log_dirs()
}

/// How long Roblox's own logs are kept: a week still holds the ones a bug report needs.
pub const KEEP_LOGS: std::time::Duration = std::time::Duration::from_secs(7 * 24 * 3600);

/// Deletes Roblox's logs older than KEEP_LOGS (its folders' own *.log files only). Roblox keeps every one, and a PC farming
/// with many windows writes a log per window per start (RoALT clears its after a day). One still being written can't be
/// deleted and stays. The number deleted.
pub fn clean_old_logs(now: std::time::SystemTime) -> usize {
    let mut removed = 0;
    let age = |t: std::time::SystemTime| now.duration_since(t).unwrap_or_default();
    for dir in log_dirs() {
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        let files: Vec<(std::path::PathBuf, std::time::SystemTime)> = entries
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().to_ascii_lowercase().ends_with(".log"))
            .filter_map(|e| {
                let meta = e.metadata().ok().filter(|m| m.is_file())?;
                Some((e.path(), meta.modified().ok()?))
            })
            .collect();
        // Nothing written for a week by that clock: Roblox unused since, or the clock set far ahead. Either way the newest
        // logs would go too: left as they are.
        let Some(newest) = files.iter().map(|f| f.1).max() else { continue };
        if age(newest) > KEEP_LOGS {
            continue;
        }
        for (path, at) in files {
            // In use or not Bloxkeep's to delete: left.
            if age(at) > KEEP_LOGS && std::fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
    }
    removed
}

// A game start, logged before "Joining game" (seen 2026-10-13 in 0.742, and in the Store edition): a launch that
// stalls while loading never gets further, so it counts as joining from here.
const LAUNCH_MARK: &str = "[FLog::SingleSurfaceApp] launchUGCGameInternal";
// Lines seen in Roblox 0.742 logs.
static JOINING_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[FLog::Output\] ! Joining game '([0-9a-fA-F-]+)' place (\d+)").expect("joining"));
static REASON_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"setting replicator disconnect reason to (\d+)").expect("reason"));
// Used only when the replicator line gave none (285: the player left). Not yet seen on a real kick, so it may
// come before or after the disconnect line.
static NOTIFICATION_REASON_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"Disconnection Notification\. Reason: (\d+)").expect("notification"));
// The server's own code, from the 0.744 client's strings; not seen in a log yet.
// Roblox couldn't reach the server it was sent to: its "Connection Failed" box (279 "No response from server"). Logged
// after "Joining game", with no serverId and no disconnect line after it (the owner's test 2026-11-14, 0.747, a rejoin into
// a server that had just run out of memory):
//   [DFLog::NetworkClient] Failed to connect to server at 128.116.46.33|50694, no response.
//   [DFLog::NetworkClient] Failed to connect to server after falling back from RbxTransport to RakNet. Disconnect code: 279. ...
// RbxTransport's own failure before it ("[DFLog::RbxTransportDummyClient] Failed to establish connection") is not the end:
// RakNet is tried after it.
const CONNECT_FAILED_MARK: &str = "[DFLog::NetworkClient] Failed to connect to server";
static CONNECT_FAILED_CODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"Disconnect code: (\d+)").expect("connect failed"));
/// Roblox's ConnectionError for "No response from server" (ConnectErrors 279).
const NO_RESPONSE: i32 = 279;
static RECEIVED_REASON_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[FLog::Network\] Disconnect reason received: (\d+)").expect("received"));
/// DisconnectClientRequest: the notification line can repeat it after the server's reason, so it never overrides one.
const CLIENT_REQUEST: i32 = 285;
// The reason in words. Roblox 0.744 often logs only these, no code (2026-10-21: a game's kick and a sign-in on
// another device), so a kick read as a lost connection and a sign-in elsewhere was rejoined over.
const REASON_WORDS_MARK: &str = "[FLog::Network] Client has been disconnected with reason: ";
const LOST_WITH_REASON_MARK: &str = "[FLog::Network] Lost connection with reason : ";

/// The words as RobloxPlayerBeta.exe 0.744 holds them, each matched to the Enum.ConnectionError code its name
/// describes. Words not listed leave the reason unknown.
pub static REASON_WORDS: LazyLock<HashMap<&'static str, i32>> = LazyLock::new(|| {
    HashMap::from([
        ("Protocol mismatch, please reconnect", 259),                                              // DisconnectProtocolMismatch
        ("Error while receiving data, please reconnect", 260),                                     // DisconnectReceivePacketError
        ("Error while streaming data, please reconnect", 261),                                     // DisconnectReceivePacketStreamError
        ("Error while sending data, please reconnect", 262),                                       // DisconnectSendPacketError
        ("Invalid teleport destination", 263),                                                     // DisconnectIllegalTeleport
        ("You are already playing a game. Please shut down the other game and try again", 264),    // DisconnectDuplicatePlayer
        ("Lost connection to server due to timeout", 266),                                         // DisconnectTimeout
        ("Disconnected due to timeout, please reconnect", 266),
        ("You have been kicked from the game", 267),                                               // DisconnectLuaKick: a game's Player:Kick
        ("Disconnected from game, possibly due to game joined from another device", 273),          // DisconnectEvicted
        ("This game has been disconnected because you have joined a game from another device", 273),
        ("Developer has shut down this game server for maintenance", 274),                         // DisconnectDevMaintenance
        ("Roblox has shut down this game server for maintenance", 275),                            // DisconnectRobloxMaintenance
        ("Lost connection to the game server, please reconnect", 277),                             // DisconnectConnectionLost
        ("Disconnected due to unsupported version. Please upgrade to the latest version of Roblox", 280), // DisconnectWrongVersion
        ("Client initiated disconnect", 285),                                                      // DisconnectClientRequest
        ("You have been disconnected because you no longer have access to this private server", 286), // DisconnectPrivateServerKickout
        ("The server has shut down", 288),                                                         // ServerShutdown
        ("Developer has shut down all game servers or game server has shut down for other reasons, please reconnect", 288),
        ("Replication has timed out", 290),                                                        // ReplicatorTimeout
        ("Player has been removed from the DataModel", 291),                                       // PlayerRemoved
        ("Disconnected due to internal network failure", 296),                                     // NetworkInternal
        ("Disconnected due to send failure", 297),                                                 // NetworkSend
        ("Disconnected due to network timeout", 298),                                              // NetworkTimeout
        ("Disconnected due to network misbehavior", 299),                                          // NetworkMisbehavior
        ("Disconnected due to security failure", 300),                                             // NetworkSecurity
    ])
});

const CONNECTED_MARK: &str = "[FLog::Network] serverId:";
// The game's words when it kicks a player, logged right before the disconnect (seen 2026-10-21 in 0.743):
// "[FLog::CreatorError] Error: Server Kick Message: A fatal error occured. This may be from an internet issue.; Metadata: "
// A ban by the game (Roblox's ban API) gives the same code 267, but its metadata says who banned and since when:
// "Server Kick Message: ; Metadata: {"moderatorType":"creator","dynamicReasonTextFilterStatus":"2","beginTimestamp":"2026-10-21T00:07:04Z"}".
// A ban met when joining again (Join Error 600) logs nothing (seen the same day): see AppController.CheckJoin.
const KICK_MARK: &str = "Server Kick Message:";
static KICK_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"Server Kick Message: (.*?); Metadata: ?(.*)$").expect("kick"));
const LEAVING_MARK: &str = "[FLog::SingleSurfaceApp] leaveUGCGameInternal";
// Signing out inside the Roblox window (seen 2026-10-13): the window stays open on the sign-in screen.
const SIGNED_OUT_MARK: &str = "logout sync cookie to native";
const DISCONNECTED_MARK: &str = "[FLog::Network] Time to disconnect replication data";
// A teleport (seen 2026-10-13): doTeleport, the same disconnect line as a kick, then "Teleported." and a new
// "Joining game". Or (2026-10-19, 0.743, same place) "Teleported." and "Joining game" first, the disconnect after.
const TELEPORT_MARK: &str = "UgcExperienceController: doTeleport";
// A teleport that never got that far (it failed) must not turn a kick minutes later into "joining" (30 s).
const TELEPORT_WINDOW_SECS: f64 = 30.0;
// Every client polls for a newer one (0.742): "waiting for mutex result is 0X000102" means nothing yet; 0X000001
// or "EventWaitResultUpdate" means one started, and the client closes to make room: what Multi-Roblox prevents.
const WND_CHECK_MARK: &str = "[FLog::WndProcessCheck]";
const NEW_PROCESS_EVENT_MARK: &str = "WindowsAppReloadTelemetry EventWaitResultUpdate";
const NEW_PROCESS_MUTEX_MARK: &str = "waiting for mutex result is 0X000001";
// Windows refusing a client memory, its commit limit (RAM and page files) used up: "Critical [DFLog::Mimalloc] cannot
// commit OS memory (error: 1455 (0x5AF), …)", "… out-of-memory on OS allocation, try again…", and the crash it ends in,
// "RBXCRASH: OutOfMemoryGraphics" (2026-10-22, two windows of six, Windows' low-virtual-memory event at 99.7 %).
const MIMALLOC_MARK: &str = "[DFLog::Mimalloc]";
const COMMIT_REFUSED_MARK: &str = "cannot commit OS memory";
const ALLOCATION_REFUSED_MARK: &str = "out-of-memory on OS allocation";
const OUT_OF_MEMORY_CRASH_MARK: &str = "RBXCRASH: OutOfMemory";
// A window closed in order (X, a close message, making room) logs its tray copy starting, then shutDown. A crash or
// a kill writes neither (2026-10-22: 7 of 40 logs on the owner's PC had both, the 2 crashes neither).
const SHUT_DOWN_MARK: &str = "[FLog::SingleSurfaceApp] shutDown:";
const TRAY_RELAUNCH_MARK: &str = "Successfully launched new process in systray mode";
// The join held for Roblox's bot check: "[DFLog::HttpTraceError] HttpResponse(…)status:403 … url:{
// "https://gamejoin.roblox.com/v1/join-game" } … challengedByGcs". The window shows Roblox's "Security" page, blank
// where the check can't load (Vietnam, 2026-10-23), and Join Error 529 once closed.
const CHALLENGED_MARK: &str = "challengedByGcs";
const GAME_JOIN_MARK: &str = "gamejoin.";
const JOIN_REQUEST_MARK: &str = "/v1/join-";
// An outdated client starts Roblox's updater with its own launch link: "[FLog::UpdateController] Update thread: start
// ""…\RobloxPlayerInstaller.exe" roblox-player:…". After "Update mode is chosen as BACKGROUND" it goes on into its game
// and the updater opens nothing; without that line it quits a second or two later and the updater opens the updated
// client with the same link about 8-10 s on (the owner's logs of 2026-10-30, 0.744 -> 0.745: four of each).
const UPDATE_THREAD_MARK: &str = "[FLog::UpdateController] Update thread: start";
const BACKGROUND_UPDATE_MARK: &str = "[FLog::UpdateController] Update mode is chosen as BACKGROUND";
// Every game join reports who joined (0.745, 2026-11-03): "[FLog::GameJoinLoadTime] Report game_join_loadtime: placeid:…,
// join_time:…, universeid:…, referral_page:Home, sid:…, clienttime:…, userid:12345,". A window opened outside Bloxkeep
// (from Roblox's own app) is matched to its account by it.
const JOIN_REPORT_MARK: &str = "game_join_loadtime";
static JOIN_REPORT_USER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"userid:(\d{1,19})").expect("userid"));
const MAX_READ: i64 = 4 * 1024 * 1024;

/// Clients without a log yet. Each miss waits longer (3 s up to 30 s); after two minutes one is left alone: the
/// "who has this file open" questions aren't free.
const FIRST_WAIT_SECS: f64 = 3.0;
const MAX_WAIT_SECS: f64 = 30.0;
const GIVE_UP_SECS: f64 = 120.0;

/// RobloxLogWatcher.SearchLimit (two minutes).
pub fn search_limit() -> std::time::Duration {
    std::time::Duration::from_secs_f64(GIVE_UP_SECS)
}

/// `found`: already read up to their end then. `foreign`: old logs no client looked for holds: not asked about again.
struct LogSearch {
    found: Vec<Tail>,
    foreign: Vec<String>,
    listed: Vec<String>,
}

/// Each log read on the worker: its pid and serial, the offset read from, the new offset and the text (None: nothing new).
type Chunk = (i32, u64, i64, (i64, Option<String>));

pub struct RobloxLogWatcher {
    pub(crate) tails: HashMap<i32, Tail>,
    /// Logs not asked about again, compared without regard to case (kept lower case).
    foreign: HashSet<String>,
    /// (FirstSeen, NextTry, Wait in seconds).
    unmatched: HashMap<i32, (Stamp, Stamp, f64)>,
    /// On a worker: asking Windows who holds a file open takes 15-50 ms a file (up to 40 logs right after a start).
    search: Option<JoinHandle<LogSearch>>,
    /// Each log with the offset read from, the new one and the text (None: nothing new).
    reading: Option<JoinHandle<Vec<Chunk>>>,
    /// Clients gone (pid, when, their kept lines): one that quit before Bloxkeep took its window still has its words.
    ended: VecDeque<Ended>,
    /// Clients whose lines already went to error.log (App::note_roblox_lines): not written again for a launch's quit.
    noted: HashSet<i32>,
}

/// A client gone: its pid, when, its log and the lines kept from it.
struct Ended {
    pid: i32,
    at: Stamp,
    path: String,
    lines: Vec<String>,
}

impl Default for RobloxLogWatcher {
    fn default() -> Self {
        RobloxLogWatcher::new()
    }
}

impl RobloxLogWatcher {
    pub fn new() -> RobloxLogWatcher {
        RobloxLogWatcher { tails: HashMap::new(), foreign: HashSet::new(), unmatched: HashMap::new(), search: None, reading: None, ended: VecDeque::new(), noted: HashSet::new() }
    }

    pub fn state_of(&self, pid: i32) -> Option<ClientState> {
        self.tails.get(&pid).and_then(|t| t.state.clone())
    }

    /// A newer Roblox client told it to close. These lookups are None when not logged (or the log isn't followed);
    /// read them after `last_state_of`, which takes in the client's last lines.
    pub fn closed_by_new_window_at(&self, pid: i32) -> Option<Stamp> {
        self.tails.get(&pid).and_then(|t| t.closed_by_new_window)
    }

    /// Windows first refused it memory (its commit limit used up).
    pub fn out_of_memory_at(&self, pid: i32) -> Option<Stamp> {
        self.tails.get(&pid).and_then(|t| t.out_of_memory)
    }

    /// It closed its window in order; None for a crash or a killed process.
    pub fn closed_in_order_at(&self, pid: i32) -> Option<Stamp> {
        self.tails.get(&pid).and_then(|t| t.closed_in_order)
    }

    /// Roblox held its join for a bot check (CHALLENGED_MARK); cleared once a game starts or a server is reached.
    pub fn join_challenged_at(&self, pid: i32) -> Option<Stamp> {
        self.tails.get(&pid).and_then(|t| t.join_challenged)
    }

    /// The client handed its launch to Roblox's updater and quits: the updated client comes from the updater
    /// (UPDATE_THREAD_MARK). None too for one updating in the background, which goes on itself.
    pub fn update_hand_off_at(&self, pid: i32) -> Option<Stamp> {
        self.tails.get(&pid).and_then(|t| t.update_hand_off)
    }

    /// Its log is found and followed, so what it said (or didn't) can be read.
    pub fn follows(&self, pid: i32) -> bool {
        self.tails.contains_key(&pid)
    }

    /// The account signed in to this window, once it has joined a game; None before or when not logged.
    pub fn user_id_of(&self, pid: i32) -> Option<i64> {
        self.tails.get(&pid).and_then(|t| t.user_id)
    }

    /// For a client that has just quit, with its final lines (a leave, a disconnect): the next `poll` drops
    /// its log unread.
    pub fn last_state_of(&mut self, pid: i32) -> Option<ClientState> {
        let tail = self.tails.get_mut(&pid)?;
        read(tail);
        ended(tail)
    }

    /// Roblox's own lines from this client's log (see keep_line) from the last WORDS_SPAN, live or just gone; empty when
    /// its log wasn't found. The client counts as written (`ended_since` leaves it out).
    pub fn lines_of(&mut self, pid: i32) -> Vec<String> {
        self.noted.insert(pid);
        let since = Stamp::utc_now().plus_secs(-WORDS_SPAN_SECS);
        match self.tails.get(&pid) {
            Some(tail) => kept_lines(tail, Some(since)),
            None => self.ended.iter().rev().find(|e| e.pid == pid).map(|e| e.lines.clone()).unwrap_or_default(),
        }
    }

    /// Clients gone since `at`, not in `before` and not written yet, newest first: a launch's client that quit before its
    /// window.
    pub fn ended_since(&self, at: Stamp, before: &HashSet<i32>) -> Vec<(i32, Vec<String>)> {
        self.ended
            .iter()
            .rev()
            .filter(|e| e.at.secs_since(at) >= -1.0 && !before.contains(&e.pid) && !self.noted.contains(&e.pid))
            .map(|e| (e.pid, e.lines.clone()))
            .collect()
    }

    /// Player logs begun since `at` that no client here was followed through and nobody holds open, newest first, with
    /// their lines: a client that quit before its log was found (the search waits FIRST_WAIT_SECS). At most two, each
    /// read up to QUIT_LOG_MOST.
    pub fn unfollowed_logs_since(&self, at: Stamp) -> Vec<(String, Vec<String>)> {
        let known: HashSet<String> = self.tails.values().map(|t| t.path.to_lowercase()).chain(self.ended.iter().map(|e| e.path.to_lowercase())).collect();
        let from = std::time::UNIX_EPOCH + std::time::Duration::from_millis((at.unix_ms() - 2000).max(0) as u64);
        let mut logs: Vec<(String, std::time::SystemTime)> = Vec::new();
        for dir in log_dirs() {
            let Ok(entries) = std::fs::read_dir(dir) else { continue };
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if !is_player_log(&name) || name.to_lowercase().contains("crashhandler") {
                    continue;
                }
                let path = entry.path().to_string_lossy().into_owned();
                let Ok(meta) = entry.metadata() else { continue };
                let created = meta.created().or_else(|_| meta.modified()).unwrap_or(std::time::UNIX_EPOCH);
                if created >= from && !known.contains(&path.to_lowercase()) {
                    logs.push((path, created));
                }
            }
        }
        logs.sort_by(|a, b| b.1.cmp(&a.1));
        let mut found = Vec::new();
        for (path, _) in logs.into_iter().take(4) {
            if !opened_by(&path).is_empty() {
                continue; // a running client's
            }
            let mut tail = Tail::new(path.clone(), 0);
            while tail.offset < QUIT_LOG_MOST && read_more(&mut tail) {}
            let name = path.rsplit(['\\', '/']).next().unwrap_or_default().to_string();
            found.push((name, kept_lines(&tail, None)));
            if found.len() == 2 {
                break;
            }
        }
        found
    }

    /// True when a state changed.
    pub fn poll(&mut self, pids: &[i32]) -> bool {
        let now = Stamp::utc_now();
        let gone: Vec<i32> = self.tails.keys().copied().filter(|pid| !pids.contains(pid)).collect();
        for pid in gone {
            if let Some(mut tail) = self.tails.remove(&pid) {
                // Its last lines: the process is gone, so nothing holds the file busy (claimed windows were read by
                // last_state_of this tick already).
                read(&mut tail);
                let lines = kept_lines(&tail, Some(now.plus_secs(-WORDS_SPAN_SECS)));
                self.ended.push_back(Ended { pid, at: now, path: tail.path, lines });
            }
        }
        while self.ended.len() > 40 || self.ended.front().is_some_and(|e| now.secs_since(e.at) > ENDED_KEPT_SECS) {
            self.ended.pop_front();
        }
        let (tails, ended) = (&self.tails, &self.ended);
        self.noted.retain(|pid| tails.contains_key(pid) || ended.iter().any(|e| e.pid == *pid));
        self.tails.retain(|pid, _| pids.contains(pid));
        self.take_search(pids);
        let tails = &self.tails;
        self.unmatched.retain(|pid, _| pids.contains(pid) && !tails.contains_key(pid));
        if pids.is_empty() {
            return false;
        }
        for &pid in pids {
            if !self.tails.contains_key(&pid) {
                self.unmatched.entry(pid).or_insert((now, now, FIRST_WAIT_SECS));
            }
        }
        if self.search.is_none() && self.unmatched.values().any(|u| now >= u.1 && now.secs_since(u.0) < GIVE_UP_SECS) {
            // Only a due client starts the search, but every client without a log takes part.
            self.start_search(pids);
            for u in self.unmatched.values_mut() {
                *u = (u.0, now.plus_secs(u.2), (u.2 * 2.0).min(MAX_WAIT_SECS));
            }
        }
        // Read on a worker: reading a log Roblox is writing can take a second or more (2026-10-29, the owner's PC:
        // 0.2-1.8 s on the window's thread, 1,315 checks over 100 ms in an hour). Lines are applied here, so states
        // only change on the window's thread, a poll later.
        let changed = self.take_reads();
        if self.reading.is_none() && !self.tails.is_empty() {
            let wanted: Vec<(i32, u64, String, i64)> = self.tails.values().map(|t| (t.pid, t.serial, t.path.clone(), t.offset)).collect();
            self.reading = crate::net::Named::new("Bloxkeep log read".into())
                .spawn(move || wanted.into_iter().map(|(pid, serial, path, offset)| (pid, serial, offset, read_chunk(&path, offset))).collect())
                .ok();
        }
        changed
    }

    /// Skips a log dropped or followed anew meanwhile, or read since on the window's thread (`last_state_of`).
    fn take_reads(&mut self) -> bool {
        if !self.reading.as_ref().is_some_and(|r| r.is_finished()) {
            return false;
        }
        let Some(reading) = self.reading.take() else {
            return false;
        };
        let result = match reading.join() {
            Ok(result) => result,
            Err(_) => {
                log::note("Roblox logs: a log read failed.");
                return false;
            }
        };
        let mut changed = false;
        for (pid, serial, from, (offset, text)) in result {
            let Some(tail) = self.tails.get_mut(&pid) else {
                continue;
            };
            if tail.serial != serial || tail.offset != from {
                continue;
            }
            // read_chunk starts over at 0 only when the file is shorter than `from`: the only way back.
            if offset < from {
                tail.restart();
            }
            tail.offset = offset;
            if let Some(text) = text {
                changed |= apply_text(tail, &text);
            }
        }
        changed
    }

    fn start_search(&mut self, pids: &[i32]) {
        let wanted: HashSet<i32> = pids.iter().copied().filter(|pid| !self.tails.contains_key(pid)).collect();
        let skip: HashSet<String> = self.tails.values().map(|t| t.path.to_lowercase()).chain(self.foreign.iter().cloned()).collect();
        self.search = crate::net::Named::new("Bloxkeep log search".into()).spawn(move || find_logs(&wanted, &skip)).ok();
    }

    fn take_search(&mut self, pids: &[i32]) {
        if !self.search.as_ref().is_some_and(|s| s.is_finished()) {
            return;
        }
        let Some(search) = self.search.take() else {
            return;
        };
        let result = match search.join() {
            Ok(result) => result,
            Err(_) => {
                log::note("Roblox logs: the search for logs failed.");
                return;
            }
        };
        // So the list doesn't grow with every client ever started.
        if !result.listed.is_empty() {
            let listed: HashSet<String> = result.listed.iter().map(|p| p.to_lowercase()).collect();
            self.foreign.retain(|p| listed.contains(p));
        }
        self.foreign.extend(result.foreign.iter().map(|p| p.to_lowercase()));
        for tail in result.found {
            if pids.contains(&tail.pid) && !self.tails.contains_key(&tail.pid) && !self.tails.values().any(|t| t.path.eq_ignore_ascii_case(&tail.path)) {
                self.tails.insert(tail.pid, tail);
            }
        }
    }
}

/// A client's state once it has ended: a leave logged and no game joined since is a leave, even without the
/// disconnect line that normally completes it. The owner's PC, 2026-10-29: back to Roblox's home from a game
/// (leaveUGCGameInternal) and the window closed with X 3 s later, before Roblox logged that line; the state stayed
/// "ingame" and auto-rejoin opened the account again as if it had dropped.
fn ended(tail: &Tail) -> Option<ClientState> {
    match &tail.state {
        Some(state) if tail.leaving && (state.status == "ingame" || state.status == "joining") => {
            Some(ClientState { status: "left".into(), reason: None, teleport_started: None, ..state.clone() })
        }
        other => other.clone(),
    }
}

/// "*_Player_*.log", any case.
fn is_player_log(name: &str) -> bool {
    let lower = name.to_lowercase();
    if !lower.ends_with(".log") {
        return false;
    }
    match lower.find("_player_") {
        Some(i) => i + "_player_".len() <= lower.len() - ".log".len(),
        None => false,
    }
}

fn find_logs(wanted: &HashSet<i32>, skip: &HashSet<String>) -> LogSearch {
    let mut result = LogSearch { found: Vec::new(), foreign: Vec::new(), listed: Vec::new() };
    let two_days_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(2 * 24 * 3600);
    // (full name, last write, length)
    let mut logs: Vec<(String, std::time::SystemTime, u64)> = Vec::new();
    for dir in log_dirs() {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue; // a folder that isn't there (an IOException listing one ends the search in the C#: see report)
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !is_player_log(&name) || name.to_lowercase().contains("crashhandler") {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
            let modified = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
            if modified > two_days_ago {
                logs.push((entry.path().to_string_lossy().into_owned(), modified, meta.len()));
            }
        }
    }
    logs.sort_by(|a, b| b.1.cmp(&a.1)); // OrderByDescending: stable
    logs.truncate(40);
    result.listed.extend(logs.iter().map(|l| l.0.clone()));
    let mut found: HashSet<i32> = HashSet::new();
    let two_minutes_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(120);
    for (full_name, modified, length) in &logs {
        if skip.contains(&full_name.to_lowercase()) {
            continue;
        }
        let owner = opened_by(full_name).into_iter().find(|pid| wanted.contains(pid) && !found.contains(pid));
        if let Some(pid) = owner {
            found.insert(pid);
            // From the start: the last join, leave or disconnect may be hours back (about 0.4 MB of log an hour).
            let mut tail = Tail::new(full_name.clone(), pid);
            while tail.offset < *length as i64 && read_more(&mut tail) {}
            result.found.push(tail);
        } else if *modified < two_minutes_ago {
            result.foreign.push(full_name.clone()); // an old log, or another program's client: don't ask again
        }
        if wanted.iter().all(|w| found.contains(w)) {
            break;
        }
    }
    result
}

/// True while something was read.
fn read_more(tail: &mut Tail) -> bool {
    let before = tail.offset;
    read(tail);
    tail.offset > before
}

fn read(tail: &mut Tail) -> bool {
    let (offset, text) = read_chunk(&tail.path, tail.offset);
    if offset < tail.offset {
        tail.restart(); // see take_reads
    }
    tail.offset = offset;
    match text {
        Some(text) => apply_text(tail, &text),
        None => false,
    }
}

fn read_chunk(path: &str, offset: i64) -> (i64, Option<String>) {
    let attempt = || -> std::io::Result<(i64, Option<String>)> {
        // Its size first, which opens no handle (so no antivirus scan of the file): a log that didn't grow, as most
        // don't between two ticks, isn't opened at all (reads of 0.2–1.8 s were seen on the owner's PC).
        if std::fs::metadata(path)?.len() as i64 == offset {
            return Ok((offset, None));
        }
        // FileShare.ReadWrite | FileShare.Delete.
        const FILE_SHARE_READ: u32 = 0x1;
        const FILE_SHARE_WRITE: u32 = 0x2;
        const FILE_SHARE_DELETE: u32 = 0x4;
        let mut stream = std::fs::OpenOptions::new().read(true).share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE).open(path)?;
        let length = stream.metadata()?.len() as i64;
        let mut offset = offset;
        if length < offset {
            offset = 0;
        }
        if length == offset {
            return Ok((offset, None));
        }
        stream.seek(SeekFrom::Start(offset as u64))?;
        let want = MAX_READ.min(length - offset) as usize;
        let mut buffer = vec![0u8; want];
        // ReadAtLeast(buffer, buffer.Length, throwOnEndOfStream: false).
        let mut read = 0usize;
        while read < want {
            let n = stream.read(&mut buffer[read..])?;
            if n == 0 {
                break;
            }
            read += n;
        }
        // A letter of several bytes cut by the chunk's end (a name with accents) is left for the next read, which starts
        // at it: decoded now it would turn into two U+FFFD. Bytes that are wrong inside the text stay as before (U+FFFD).
        if let Err(e) = std::str::from_utf8(&buffer[..read])
            && e.error_len().is_none()
        {
            // Only the start of a letter so far: nothing read, the offset stays for when the rest is written.
            if e.valid_up_to() == 0 {
                return Ok((offset, None));
            }
            read = e.valid_up_to();
        }
        Ok((offset + read as i64, Some(String::from_utf8_lossy(&buffer[..read]).into_owned())))
    };
    attempt().unwrap_or((offset, None))
}

/// MemoryExtensions.EnumerateLines: split at CR, LF, CRLF, NEL, LS, PS and FF.
fn enumerate_lines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0usize;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        match c {
            '\r' => {
                lines.push(&text[start..i]);
                if let Some(&(_, '\n')) = chars.peek() {
                    chars.next();
                    start = i + 2;
                } else {
                    start = i + 1;
                }
            }
            '\n' | '\u{000C}' | '\u{0085}' | '\u{2028}' | '\u{2029}' => {
                lines.push(&text[start..i]);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    lines.push(&text[start..]);
    lines
}

pub(crate) fn apply_text(tail: &mut Tail, read: &str) -> bool {
    let text = std::mem::take(&mut tail.partial) + read;
    let Some(end) = text.rfind('\n') else {
        tail.partial = text;
        return false;
    };
    tail.partial = text[end + 1..].to_string();
    let before = tail.state.clone();
    for line in enumerate_lines(&text[..end]) {
        keep_line(tail, line);
        apply(tail, line);
    }
    tail.state != before
}

// ───────── Roblox's own words for error.log ─────────
// Bloxkeep's log says what Bloxkeep did; why Roblox showed an error, quit or left a game is only in Roblox's log. Users
// rarely send that one (2026-11-15: two reports, 403 at start and rejoins "blocked", neither provable without it), so a
// window that ends gets its own lines written to error.log (App::note_roblox_lines).

/// Kept lines for this long before a window ends (older ones say little about why it ended).
const WORDS_SPAN_SECS: f64 = 600.0;
/// Gone clients' lines kept this long (a launch held for Roblox's updater still finds its client's).
const ENDED_KEPT_SECS: f64 = 1800.0;
/// A quit client's log read this far at most (one that ran seconds writes far less).
const QUIT_LOG_MOST: i64 = 512 * 1024;
const KEEP_SAID: usize = 12;
const KEEP_OTHER: usize = 6;
const KEEP_LAST: usize = 6;
const LINE_MOST: usize = 240;

/// One kept line: its place in the log, its time, the line shortened, and its words with numbers taken out (a retry
/// loop's lines differ only in those).
#[derive(Clone)]
struct Kept {
    n: u64,
    at: Stamp,
    text: String,
    shape: String,
}

/// Never kept: lines with a sign-in ticket itself or a join link (they carry the account's id and names, URL-encoded past any
/// scrub), and the game's own output (other players' names, the game's words).
static NEVER_KEPT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)joinscripturl|ticket=|ticket"|gameinfo:|username|displayname|user_?id|\[FLog::(?:Output|CreatorError)\]"#).expect("never-kept pattern")
});
/// Roblox's own account of a session: its network, joins, leaves, updater and crash lines, and words that say why.
static SAYS_WHY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\[D?FLog::(?:Network|NetworkClient|SessionTransitionFSM|UpdateController|CrashReportLog|Teleport\w*|GameJoin\w*|PlaceLauncher\w*)\]|disconnect|kick|\bban(?:ned)?\b|authenticat|unauthori|forbidden|denied|lost connection|shut ?down|leaveUGCGame|not.?approved|moderat|WebLogin|authentication-ticket|NoReload|singleton|\b40[13]\b|\b429\b|\[D?FLog::HttpTraceError\].*roblox\.com",
    )
    .expect("says-why pattern")
});
/// Other errors, kept apart so a game's missing sounds and meshes can't push out the lines above.
static OTHER_ERROR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)error|fail|exception|crash|timed? ?out|refus|invalid|reject|abort").expect("other-error pattern")
});
static ASSET_NOISE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)rbxasset|rbxcdn|sound|wrap|deform|asset|texture|mesh|animation|memoryPrioritization").expect("asset-noise pattern")
});
/// A join's routine steps (owner's real logs, 2026-11-15: they filled the kept lines after every rejoin and say nothing of why).
static ROUTINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)Replicator (?:created|destroyed)|Time taken to initialize|Assigned peer id|Join snapshot timer|MegaReplicator|UDMUX Address|serverId:|NetworkClient:Remove").expect("routine pattern")
});
/// Numbers, pointers (0x22b8d281cf0) and ids turned to "#", so a retry loop's lines read as one.
static NUMBERS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)0x[0-9a-f]+|\b[0-9a-f]{6,}\b|\d+").expect("numbers pattern"));
static PLAYER_PATH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"Players\.[^\s.:]+").expect("player-path pattern"));

/// The line's own words, after "time,uptime,thread,level": the uptime column (403.97…) must not read as an HTTP 403.
fn words_of(line: &str) -> &str {
    let mut parts = line.splitn(4, ',');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(_), Some(_), Some(_), Some(rest)) => rest.trim_start_matches(|c: char| c.is_ascii_digit()).trim_start_matches([',', ' ']),
        _ => line.trim(),
    }
}

/// "2026-11-14T07:57:02.368Z,1.36,be64,6,Warning [FLog::Warning] text" as "07:57:02 Warning [FLog::Warning] text".
fn short_line(line: &str) -> String {
    let words = PLAYER_PATH.replace_all(words_of(line), "Players.[player]");
    let time = line.split(',').next().unwrap_or_default();
    match time.get(11..19).filter(|_| time.as_bytes().get(10) == Some(&b'T')) {
        Some(clock) if words.len() < line.len() => cut(&format!("{clock} {words}")),
        _ => cut(&words),
    }
}

fn cut(text: &str) -> String {
    match text.char_indices().nth(LINE_MOST) {
        Some((i, _)) => format!("{}…", &text[..i]),
        None => text.to_string(),
    }
}

/// Into a ring of `most`: a line of the same shape as one there replaces it (counted once, at its newest place).
fn push_kept(ring: &mut VecDeque<Kept>, kept: Kept, most: usize) {
    if let Some(i) = ring.iter().position(|k| k.shape == kept.shape) {
        ring.remove(i);
    }
    ring.push_back(kept);
    if ring.len() > most {
        ring.pop_front();
    }
}

fn keep_line(tail: &mut Tail, line: &str) {
    if line.trim().is_empty() || NEVER_KEPT.is_match(line) {
        return;
    }
    tail.lines_read += 1;
    let words = words_of(line);
    let text = short_line(line);
    let kept = Kept { n: tail.lines_read, at: time_of(line), shape: NUMBERS.replace_all(words, "#").into_owned(), text };
    // A game's missing sounds and meshes fail by the hundred, some with "HTTP 403": never the reason a window ended.
    if ASSET_NOISE.is_match(words) || ROUTINE.is_match(words) {
    } else if SAYS_WHY.is_match(words) {
        push_kept(&mut tail.said, kept.clone(), KEEP_SAID);
    } else if OTHER_ERROR.is_match(words) {
        push_kept(&mut tail.other, kept.clone(), KEEP_OTHER);
    }
    // The same line over and over at the end (a heartbeat, a memory callback) counts once.
    push_kept(&mut tail.last_lines, kept, KEEP_LAST);
}

/// Tests: the lines kept from a whole log's text (roblox-words suite).
pub(crate) fn kept_from(text: &str) -> Vec<String> {
    let mut tail = Tail::new(String::new(), 0);
    apply_text(&mut tail, text);
    kept_lines(&tail, None)
}

/// The lines kept, in the log's order; with `since`, the ones that say why only from then on (the last lines always).
fn kept_lines(tail: &Tail, since: Option<Stamp>) -> Vec<String> {
    let recent = |k: &&Kept| since.is_none_or(|s| k.at >= s);
    let mut all: Vec<&Kept> = tail.said.iter().filter(recent).chain(tail.other.iter().filter(recent)).chain(tail.last_lines.iter()).collect();
    all.sort_by_key(|k| k.n);
    all.dedup_by_key(|k| k.n);
    all.into_iter().map(|k| k.text.clone()).collect()
}

/// long.TryParse(…, NumberStyles.None, InvariantCulture): ASCII digits only.
fn parse_i64(s: &str) -> Option<i64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

fn parse_i32(s: &str) -> Option<i32> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// A reason line is a drop by itself: a game that fails while loading (or a server that drops it) shows Roblox's
/// "Disconnected" box, and its "Time to disconnect replication data" line may never come, so the window read as in its
/// game for good (a RoALT user's report 2026-11-14: kicked on joining, still "In game"). Not while leaving, nor in a
/// teleport (its old server's lines), nor a join still on its old server, nor the client's own request (285). None of
/// these lines is logged on a plain leave or teleport (the owner's 0.747 logs, 2026-11-12/13).
fn drop_on_reason(tail: &mut Tail, code: Option<i32>, line: &str) {
    // Roblox's own network line only, never a game's output quoting one.
    let Some(tag) = line.find('[') else { return };
    if !(line[tag..].starts_with("[FLog::Network]") || line[tag..].starts_with("[DFLog::Network")) {
        return;
    }
    let Some(state) = tail.state.clone() else { return };
    if tail.leaving
        || code == Some(CLIENT_REQUEST)
        || !(state.status == "ingame" || state.status == "joining")
        || state.teleport_started.is_some()
        || (state.status == "joining" && tail.connections > 0)
    {
        return;
    }
    let at = time_of(line);
    if tail.teleport_at.is_some_and(|teleport| at.secs_since(teleport) < TELEPORT_WINDOW_SECS) {
        return;
    }
    tail.shown_reason_rank = tail.pending_reason_rank;
    tail.state = Some(ClientState {
        status: "disconnected".into(),
        reason: tail.pending_reason,
        kick_message: tail.pending_kick_message.clone(),
        banned_by_game: tail.pending_ban,
                ban_ends: tail.pending_ban_ends,
        at,
        teleport_started: None,
        ..state
    });
    tail.pending_reason = None;
    (tail.pending_kick_message, tail.pending_ban, tail.pending_ban_ends) = (None, false, None);
    tail.dropped_by_reason = true;
}

/// One line of the log: what it changes in the client's state.
pub(crate) fn apply(tail: &mut Tail, line: &str) {
    if line.contains(JOIN_REPORT_MARK) {
        if let Some(who) = JOIN_REPORT_USER.captures(line) {
            if let Some(user) = parse_i64(&who[1]) {
                if user > 0 {
                    tail.user_id = Some(user);
                }
            }
        }
    }
    // A number too big for its type reads as no place, never as an error that would stop the process check.
    let join = if line.contains("! Joining game") { JOINING_LINE.captures(line) } else { None };
    if let Some(join) = join {
        // A teleport stays open: its old server's disconnect line may still come (see TELEPORT_MARK).
        tail.leaving = false;
        tail.pending_reason = None;
        (tail.pending_kick_message, tail.pending_ban, tail.pending_ban_ends) = (None, false, None);
        let place = parse_i64(&join[2]);
        let previous = tail.state.clone();
        tail.state = Some(ClientState {
            chosen_place: if tail.own_start { place } else { previous.as_ref().and_then(|s| s.chosen_place) },
            person_started: previous.as_ref().is_some_and(|s| s.person_started),
            teleport_started: previous.as_ref().and_then(|s| s.teleport_started),
            ..ClientState::new("joining", place, Some(join[1].to_string()), None, time_of(line))
        });
        tail.own_start = false;
        tail.join_challenged = None;
        tail.dropped_by_reason = false;
    }
    // The line itself: Roblox's client also has "launchUGCGameInternal: deferred download complete" and the like, no game start.
    else if line.contains(LAUNCH_MARK) && line.trim_end().ends_with(LAUNCH_MARK) {
        tail.leaving = false;
        tail.teleport_at = None;
        tail.pending_reason = None;
        (tail.pending_kick_message, tail.pending_ban, tail.pending_ban_ends) = (None, false, None);
        // Bloxkeep's start is a window's first; a later one is the person picking a game on Roblox's screens after
        // leaving or a kick (seen 2026-10-19).
        tail.own_start = tail.state.is_some();
        let chosen = tail.state.as_ref().and_then(|s| s.chosen_place);
        tail.state = Some(ClientState { chosen_place: chosen, person_started: tail.own_start, ..ClientState::new("joining", None, None, None, time_of(line)) });
        tail.join_challenged = None;
        tail.dropped_by_reason = false;
    } else if line.contains(CONNECTED_MARK) {
        tail.connections += 1;
        tail.dropped_by_reason = false;
        if let Some(joining) = tail.state.as_ref().filter(|s| s.status == "joining") {
            tail.state = Some(ClientState { status: "ingame".into(), at: time_of(line), person_started: false, teleport_started: None, ..joining.clone() });
        }
    } else if line.contains(LEAVING_MARK) {
        tail.leaving = true;
        tail.teleport_at = None; // leaving, not moving: the disconnect line that follows is the leave's
        // Left while the game was still loading (or before anything was logged): no disconnect line follows.
        if tail.state.as_ref().is_none_or(|s| s.status == "joining") {
            let never_joined = tail.state.as_ref().is_some_and(|s| s.status == "joining" && s.job_id.is_none());
            let base = tail.state.clone().unwrap_or_else(|| ClientState::new("left", None, None, None, time_of(line)));
            tail.state = Some(ClientState { status: "left".into(), reason: None, at: time_of(line), teleport_started: None, never_joined, ..base });
        }
    } else if line.contains(SIGNED_OUT_MARK) {
        let base = tail.state.clone().unwrap_or_else(|| ClientState::new("signedout", None, None, None, time_of(line)));
        tail.state = Some(ClientState { status: "signedout".into(), reason: None, at: time_of(line), teleport_started: None, ..base });
        // Whoever signs in next in this window is named by their own join report: kept, the window could be
        // adopted as the account that left (audit B15).
        tail.user_id = None;
    } else if line.contains(TELEPORT_MARK) {
        tail.teleport_at = Some(time_of(line));
        // Shown as teleporting until the next server is reached (its three doTeleport lines keep the first time).
        if let Some(moving) = tail.state.as_ref().filter(|s| (s.status == "ingame" || s.status == "joining") && s.teleport_started.is_none()) {
            tail.state = Some(ClientState { teleport_started: tail.teleport_at, ..moving.clone() });
        }
    } else if let Some(kick) = (if line.contains(KICK_MARK) { KICK_LINE.captures(line) } else { None }) {
        let words = kick[1].trim();
        let message = if words.is_empty() { None } else { Some(words.to_string()) };
        let ban = is_ban(&kick[2]);
        let ends = if ban { ban_end_of(&kick[2]) } else { None };
        // Normally before the disconnect line; one logged after it completes the state shown.
        match tail.state.as_ref() {
            Some(shown) if shown.status == "disconnected" && shown.kick_message.is_none() && !shown.banned_by_game => {
                tail.state = Some(ClientState { kick_message: message, banned_by_game: ban, ban_ends: ends, ..shown.clone() });
            }
            _ => (tail.pending_kick_message, tail.pending_ban, tail.pending_ban_ends) = (message, ban, ends),
        }
    } else if line.contains(REASON_WORDS_MARK) || line.contains(LOST_WITH_REASON_MARK) {
        let worded = reason_of_words(line);
        drop_on_reason(tail, worded, line);
        if let Some(known) = worded {
            remember_reason(tail, known, 1);
        }
    } else if let Some(why) = if line.contains("disconnect reason to") { REASON_LINE.captures(line).and_then(|c| parse_i32(&c[1])) } else { None } {
        drop_on_reason(tail, Some(why), line);
        remember_reason(tail, why, 3);
    } else if let Some(sent) =
        if line.contains("Disconnect reason received") { RECEIVED_REASON_LINE.captures(line).and_then(|c| parse_i32(&c[1])) } else { None }
    {
        drop_on_reason(tail, Some(sent), line);
        remember_reason(tail, sent, 3);
    } else if let Some(code) =
        if line.contains("Disconnection Notification") { NOTIFICATION_REASON_LINE.captures(line).and_then(|c| parse_i32(&c[1])) } else { None }
    {
        drop_on_reason(tail, Some(code), line);
        remember_reason(tail, code, if code == CLIENT_REQUEST { 0 } else { 2 });
    }
    // A join that never reached its server ends here: the window sat on Roblox's "Connection Failed" box, read as still
    // joining a named server, which no timeout covered (see CONNECT_FAILED_MARK).
    else if line.contains(CONNECT_FAILED_MARK) {
        let said = CONNECT_FAILED_CODE.captures(line).and_then(|c| parse_i32(&c[1]));
        match tail.state.clone() {
            Some(failing) if !tail.leaving && failing.status == "joining" => {
                tail.shown_reason_rank = 3;
                tail.state = Some(ClientState {
                    status: "disconnected".into(),
                    reason: Some(said.unwrap_or(NO_RESPONSE)),
                    kick_message: None,
                    banned_by_game: false,
                    at: time_of(line),
                    teleport_started: None,
                    ..failing
                });
                tail.teleport_at = None;
                tail.pending_reason = None;
                (tail.pending_kick_message, tail.pending_ban, tail.pending_ban_ends) = (None, false, None);
            }
            // The fallback line after it only sharpens the code.
            Some(shown) if shown.status == "disconnected" && shown.reason == Some(NO_RESPONSE) && said.is_some() => {
                tail.state = Some(ClientState { reason: said, ..shown });
            }
            _ => {}
        }
    } else if line.contains(DISCONNECTED_MARK) && tail.dropped_by_reason && tail.state.as_ref().is_some_and(|s| s.status == "disconnected") {
        // Already shown as a drop by its reason line: this line only ends that server.
        tail.connections = 0.max(tail.connections - 1);
        tail.teleport_at = None;
        tail.leaving = false;
        tail.dropped_by_reason = false;
    } else if line.contains(DISCONNECTED_MARK) && tail.state.is_some() {
        let state = tail.state.clone().expect("checked");
        let at = time_of(line);
        // A join under way while a server is still connected: this line ends the old server, in whatever order
        // Roblox logged them, and the join goes on.
        let old_server = state.status == "joining" && tail.connections > 0;
        tail.connections = 0.max(tail.connections - 1);
        let teleporting = tail.teleport_at.is_some_and(|teleport| at.secs_since(teleport) < TELEPORT_WINDOW_SECS);
        // The new server already named since the teleport: this line is the old one's, and the new state stands.
        let joined_since = tail.teleport_at.is_some_and(|since| state.at >= since);
        tail.teleport_at = None; // once: a later disconnect without a new teleport is a real one
        if teleporting || old_server {
            if state.status != "joining" && !joined_since {
                tail.state = Some(ClientState { status: "joining".into(), at, ..state });
            }
            // The old server's kick words go with it, not with the next server's disconnect.
            tail.pending_reason = None;
            (tail.pending_kick_message, tail.pending_ban, tail.pending_ban_ends) = (None, false, None);
            return;
        }
        tail.shown_reason_rank = if tail.leaving { 0 } else { tail.pending_reason_rank };
        tail.state = Some(if tail.leaving {
            ClientState { status: "left".into(), reason: None, at, teleport_started: None, ..state }
        } else {
            ClientState {
                status: "disconnected".into(),
                reason: tail.pending_reason,
                kick_message: tail.pending_kick_message.clone(),
                banned_by_game: tail.pending_ban,
                ban_ends: tail.pending_ban_ends,
                at,
                teleport_started: None,
                ..state
            }
        });
        tail.leaving = false;
        tail.pending_reason = None;
        (tail.pending_kick_message, tail.pending_ban, tail.pending_ban_ends) = (None, false, None);
    } else if line.contains(WND_CHECK_MARK) && (line.contains(NEW_PROCESS_EVENT_MARK) || line.contains(NEW_PROCESS_MUTEX_MARK)) {
        if tail.closed_by_new_window.is_none() {
            tail.closed_by_new_window = Some(time_of(line));
        }
    } else if line.contains(MIMALLOC_MARK) && (line.contains(COMMIT_REFUSED_MARK) || line.contains(ALLOCATION_REFUSED_MARK)) || line.contains(OUT_OF_MEMORY_CRASH_MARK) {
        if tail.out_of_memory.is_none() {
            tail.out_of_memory = Some(time_of(line));
        }
    } else if line.contains(SHUT_DOWN_MARK) || line.contains(TRAY_RELAUNCH_MARK) {
        if tail.closed_in_order.is_none() {
            tail.closed_in_order = Some(time_of(line));
        }
    } else if line.contains(CHALLENGED_MARK)
        && line.contains(GAME_JOIN_MARK)
        && line.contains(JOIN_REQUEST_MARK)
        && tail.state.as_ref().is_some_and(|s| s.status == "joining" && s.job_id.is_none())
    {
        if tail.join_challenged.is_none() {
            tail.join_challenged = Some(time_of(line));
        }
    } else if line.contains(BACKGROUND_UPDATE_MARK) {
        tail.background_update = true;
    } else if line.contains(UPDATE_THREAD_MARK) && !tail.background_update {
        if tail.update_hand_off.is_none() {
            tail.update_hand_off = Some(time_of(line));
        }
    }
}

// A numeric code can arrive after the words. Replicator (3) > numeric notification (2) > mapped words (1); equal
// ranks keep the first.
fn remember_reason(tail: &mut Tail, code: i32, rank: i32) {
    if let Some(shown) = tail.state.as_ref().filter(|s| s.status == "disconnected") {
        if shown.reason.is_none() || rank > tail.shown_reason_rank {
            tail.state = Some(ClientState { reason: Some(code), ..shown.clone() });
            tail.shown_reason_rank = rank;
        }
    } else if tail.pending_reason.is_none() || rank > tail.pending_reason_rank {
        tail.pending_reason = Some(code);
        tail.pending_reason_rank = rank;
    }
}

/// Roblox's ban API fills moderatorType and beginTimestamp; a plain kick leaves the metadata empty.
pub fn is_ban(metadata: &str) -> bool {
    if !metadata.trim_start().starts_with('{') {
        return false;
    }
    match serde_json::from_str::<serde_json::Value>(metadata) {
        Ok(doc) => doc.as_object().is_some_and(|o| o.contains_key("moderatorType") || o.contains_key("beginTimestamp")),
        Err(_) => metadata.contains("\"moderatorType\""),
    }
}

/// When a timed ban ends, UTC: its endTimestamp, else beginTimestamp + durationSeconds. None for a permanent ban
/// (durationSeconds -1, or no end given). The owner's test 2026-11-14 (BanAsync, 120 s):
/// {"moderatorType":"creator","durationSeconds":"120","endTimestamp":"2026-11-13T23:09:32Z","dynamicReasonTextFilterStatus":"2","beginTimestamp":"2026-11-13T23:07:32Z"};
/// without it the account was still treated as banned ten minutes after the ban ended.
pub fn ban_end_of(metadata: &str) -> Option<Stamp> {
    if !metadata.trim_start().starts_with('{') {
        return None;
    }
    let root = serde_json::from_str::<serde_json::Value>(metadata).ok()?;
    let root = root.as_object()?;
    let text = |name: &str| -> Option<String> {
        match root.get(name)? {
            serde_json::Value::String(s) => Some(s.clone()),
            serde_json::Value::Number(n) => Some(n.to_string()),
            _ => None,
        }
    };
    // Read as UTC when it names no zone (AssumeUniversal).
    let time = |t: Option<String>| -> Option<Stamp> {
        let t = t?.trim().to_string();
        let zoned = t.ends_with(['Z', 'z']) || t.get(10..).is_some_and(|clock| clock.contains(['+', '-']));
        Stamp::parse(&if zoned { t } else { format!("{t}Z") })
    };
    if let Some(end) = time(text("endTimestamp")) {
        return Some(end);
    }
    let seconds = text("durationSeconds")?.trim().parse::<i64>().ok().filter(|s| *s > 0)?;
    time(text("beginTimestamp")).map(|begin| begin.plus_secs(seconds as f64))
}

pub fn reason_of_words(line: &str) -> Option<i32> {
    let tag = line.find('[')?;
    let entry = &line[tag..];
    let words = if let Some(rest) = entry.strip_prefix(REASON_WORDS_MARK) {
        rest
    } else if let Some(rest) = entry.strip_prefix(LOST_WITH_REASON_MARK) {
        rest
    } else {
        return None;
    };
    let words = words.trim();
    match REASON_WORDS.get(words) {
        Some(&code) => Some(code),
        None if words.starts_with(IDLE_WORDS) => Some(278),
        None => None,
    }
}

/// The client's prompt for DisconnectIdle (278), before its minutes, so idle kicks are told on Roblox 0.744 too.
const IDLE_WORDS: &str = "You were disconnected for being idle ";

/// Lines start with an ISO UTC time ("2026-10-12T05:14:35.082Z,…"); one without a zone is taken as UTC.
fn time_of(line: &str) -> Stamp {
    if let Some(comma) = line.find(',') {
        if comma > 0 {
            let text = &line[..comma];
            if let Some(at) = Stamp::parse(text) {
                if at.kind() != Kind::Unspecified {
                    return at.to_utc();
                }
                if let Some(utc) = Stamp::parse(&format!("{}Z", text.trim())) {
                    return utc;
                }
            }
        }
    }
    Stamp::utc_now()
}

// ───────── Restart Manager: which processes hold a file open (what Windows uses for "file in use") ─────────

fn opened_by(path: &str) -> Vec<i32> {
    let mut session = 0u32;
    let mut key = [0u16; CCH_RM_SESSION_KEY as usize + 1];
    // SAFETY: the session is ended below; the file name and the list live on this stack frame for each call.
    unsafe {
        if RmStartSession(&mut session, 0, key.as_mut_ptr()) != 0 {
            return Vec::new();
        }
        let result = (|| {
            let file = wide(path);
            let names = [file.as_ptr()];
            if RmRegisterResources(session, 1, names.as_ptr(), 0, std::ptr::null(), 0, std::ptr::null()) != 0 {
                return Vec::new();
            }
            let (mut needed, mut count, mut reasons) = (0u32, 0u32, 0u32);
            let result = RmGetList(session, &mut needed, &mut count, std::ptr::null_mut(), &mut reasons);
            if result != ERROR_MORE_DATA || needed == 0 {
                return Vec::new();
            }
            let mut info: Vec<RM_PROCESS_INFO> = vec![std::mem::zeroed(); needed as usize];
            count = needed;
            if RmGetList(session, &mut needed, &mut count, info.as_mut_ptr(), &mut reasons) == 0 {
                info.iter().take(count as usize).map(|i| i.Process.dwProcessId as i32).collect()
            } else {
                Vec::new()
            }
        })();
        RmEndSession(session);
        result
    }
}

/// A whole log read as the watcher reads it, for the port's check against the C# reading of the same file
/// (core/selftest.rs, --core-log-replay): one line for each change of state, then what else the log showed. Times are
/// left out (they are the clock's, not the log's).
pub fn replay(text: &str) -> Vec<String> {
    let mut tail = Tail::new(String::new(), 0);
    let mut out = Vec::new();
    let opt = |v: Option<i64>| v.map(|x| x.to_string()).unwrap_or_default();
    for line in text.split_inclusive('\n') {
        if apply_text(&mut tail, line) {
            if let Some(s) = &tail.state {
                out.push(format!(
                    "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
                    s.status,
                    opt(s.place_id),
                    s.job_id.clone().unwrap_or_default(),
                    opt(s.reason.map(i64::from)),
                    opt(s.chosen_place),
                    s.person_started,
                    s.kick_message.clone().unwrap_or_default(),
                    s.banned_by_game,
                    s.teleport_started.is_some(),
                    s.never_joined,
                    opt(s.ban_ends.map(|t| t.to_utc().unix_secs()))
                ));
            }
        }
    }
    out.push(format!(
        "end|newWindow={}|oom={}|inOrder={}|challenged={}|bgUpdate={}|handOff={}|user={}|connections={}|dropped={}|leaving={}",
        tail.closed_by_new_window.is_some(),
        tail.out_of_memory.is_some(),
        tail.closed_in_order.is_some(),
        tail.join_challenged.is_some(),
        tail.background_update,
        tail.update_hand_off.is_some(),
        opt(tail.user_id),
        tail.connections,
        tail.dropped_by_reason,
        tail.leaving
    ));
    out
}
