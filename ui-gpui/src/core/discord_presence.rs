//! DiscordPresence.cs: "Playing Bloxkeep" on the Discord profile (Rich Presence) through the local Discord app's pipe
//! (discord-ipc-0 to 9), as games do. No server or token: an application id is public. Retried later while Discord
//! isn't running.
use std::fs::File;
use std::io::{Read, Write};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use crate::core::base::{Fail, Res, Stamp, new_guid};
use crate::core::log;
use crate::core::models::to_net_json_string;

/// What the profile shows; the application's icon and name come with it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Activity {
    pub details: String,
    pub state: Option<String>,
    pub start_unix: i64,
    pub button_label: String,
    pub button_url: String,
}

/// The tests' stand-in for Discord's pipe; Discord's own otherwise.
static TEST_PIPE: LazyLock<Option<String>> = LazyLock::new(|| std::env::var("BLOXKEEP_DISCORD_PIPE").ok().filter(|p| !p.is_empty()));
const RETRY_AFTER_SECS: f64 = 30.0;
const ANSWER_WITHIN: Duration = Duration::from_secs(3);

struct State {
    pipe: Option<File>,
    client_id: Option<String>,
    sent: Option<String>,
    next_try: Option<Stamp>,
    /// The last try to reach Discord failed: written once, then once more when it connects.
    failing: bool,
}

pub struct DiscordPresence {
    /// C#'s `_gate` (a SemaphoreSlim taken without waiting) and the fields it guards.
    gate: Mutex<State>,
    disposed: std::sync::atomic::AtomicBool,
}

impl Default for DiscordPresence {
    fn default() -> DiscordPresence {
        DiscordPresence::new()
    }
}

impl DiscordPresence {
    pub fn new() -> DiscordPresence {
        DiscordPresence {
            gate: Mutex::new(State { pipe: None, client_id: None, sent: None, next_try: None, failing: false }),
            disposed: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Shows `activity` (None clears it) for application `client_id`; the same one twice is sent once.
    pub fn set(&self, activity: Option<&Activity>, client_id: &str) {
        let json = match activity {
            None => "null".to_string(),
            Some(a) => activity_json(a),
        };
        if self.disposed.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        // Busy with another call: this one is skipped (WaitAsync(0)).
        let Ok(mut s) = self.gate.try_lock() else { return };
        if s.sent.as_deref() == Some(json.as_str()) && s.client_id.as_deref() == Some(client_id) {
            return;
        }
        let result = (|| -> Res<()> {
            if s.pipe.is_none() || s.client_id.as_deref() != Some(client_id) {
                if activity.is_none() || s.next_try.is_some_and(|t| Stamp::utc_now() < t) {
                    return Ok(());
                }
                close(&mut s);
                if !connect(&mut s, client_id) {
                    if !s.failing {
                        log::note("Discord presence: the Discord app isn't reachable on this PC (not running?); tried again every 30 s.");
                    }
                    s.failing = true;
                    s.next_try = Some(Stamp::utc_now().plus_secs(RETRY_AFTER_SECS));
                    return Ok(());
                }
                if s.failing {
                    log::note("Discord presence: connected to the Discord app.");
                }
                s.failing = false;
            }
            let nonce = new_guid().to_string();
            let pipe = s.pipe.as_mut().ok_or_else(|| Fail::other("no pipe"))?;
            write_frame(pipe, 1, &format!("{{\"cmd\":\"SET_ACTIVITY\",\"args\":{{\"pid\":{},\"activity\":{json}}},\"nonce\":\"{nonce}\"}}", std::process::id()))?;
            read_frame(pipe)?; // its answer, so the pipe never fills up
            s.sent = Some(json.clone());
            Ok(())
        })();
        if let Err(e) = result {
            // Discord closed or answered something unexpected: connected again on a later try.
            if !s.failing {
                log::note(&format!("Discord presence: the link to the Discord app dropped ({e}); connected again later."));
            }
            s.failing = true;
            close(&mut s);
            s.next_try = Some(Stamp::utc_now().plus_secs(RETRY_AFTER_SECS));
        }
    }

    /// Quitting: the profile stops showing Bloxkeep (closing the pipe clears it on Discord's side too).
    pub fn dispose(&self) {
        self.disposed.store(true, std::sync::atomic::Ordering::SeqCst);
        let mut s = self.gate.lock().unwrap_or_else(|e| e.into_inner());
        close(&mut s);
    }
}

impl Drop for DiscordPresence {
    fn drop(&mut self) {
        self.dispose();
    }
}

/// `s` cut to Discord's lines of 2 to 128 UTF-16 units.
fn fit(s: &str) -> String {
    let units: Vec<u16> = s.encode_utf16().collect();
    if units.len() > 128 {
        // PORT: C#'s s[..127] can split a surrogate pair; a Rust String can't hold half of one (it becomes U+FFFD).
        String::from_utf16_lossy(&units[..127]) + "…"
    } else if units.len() < 2 {
        format!("{s:<2}")
    } else {
        s.to_string()
    }
}

/// The activity as Discord's SET_ACTIVITY takes it: lines of 2 to 128 characters, one link button.
pub(crate) fn activity_json(a: &Activity) -> String {
    let label: Vec<u16> = a.button_label.encode_utf16().collect();
    let label = if label.len() > 32 { String::from_utf16_lossy(&label[..32]) } else { a.button_label.clone() };
    let mut activity = Map::new();
    activity.insert("details".into(), json!(fit(&a.details)));
    activity.insert("timestamps".into(), json!({ "start": a.start_unix }));
    activity.insert("assets".into(), json!({ "large_text": "Bloxkeep" }));
    activity.insert("buttons".into(), json!([{ "label": label, "url": a.button_url }]));
    if let Some(state) = a.state.as_deref().filter(|s| !s.is_empty()) {
        activity.insert("state".into(), json!(fit(state)));
    }
    to_net_json_string(&Value::Object(activity)).unwrap_or_default()
}

fn connect(s: &mut State, client_id: &str) -> bool {
    let names: Vec<String> = match TEST_PIPE.as_ref() {
        Some(test) => vec![test.clone()],
        None => (0..10).map(|i| format!("discord-ipc-{i}")).collect(),
    };
    for name in names {
        let Some(pipe) = open_pipe(&name, Duration::from_millis(500)) else { continue };
        s.pipe = Some(pipe);
        let hello = to_net_json_string(&json!({ "v": 1, "client_id": client_id })).unwrap_or_default();
        let answer = (|| -> Res<(i32, String)> {
            let pipe = s.pipe.as_mut().ok_or_else(|| Fail::other("no pipe"))?;
            write_frame(pipe, 0, &hello)?;
            read_frame(pipe)
        })();
        match answer {
            Ok((op, body)) => {
                // READY, or a close with the reason (an application id Discord doesn't know).
                if op != 1 || !body.contains("\"READY\"") {
                    log::note(&format!("Discord presence: Discord didn't accept the application (op {op})."));
                    close(s);
                    return false;
                }
                s.client_id = Some(client_id.to_string());
                return true;
            }
            Err(_) => {
                s.pipe = None;
            }
        }
    }
    false
}

/// NamedPipeClientStream(".", name).ConnectAsync with a time limit: a busy pipe is waited for until then.
fn open_pipe(name: &str, within: Duration) -> Option<File> {
    let path = format!(r"\\.\pipe\{name}");
    let until = Instant::now() + within;
    loop {
        match std::fs::OpenOptions::new().read(true).write(true).open(&path) {
            Ok(f) => return Some(f),
            // ERROR_PIPE_BUSY: every instance taken; wait for one.
            Err(e) if e.raw_os_error() == Some(231) => {
                let left = until.saturating_duration_since(Instant::now());
                if left.is_zero() || !wait_named_pipe(&path, left) {
                    return None;
                }
            }
            Err(_) => return None,
        }
    }
}

fn wait_named_pipe(path: &str, within: Duration) -> bool {
    use windows_sys::Win32::System::Pipes::WaitNamedPipeW;
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: a NUL-terminated UTF-16 pipe name that lives across the call.
    unsafe { WaitNamedPipeW(wide.as_ptr(), within.as_millis().min(u32::MAX as u128) as u32) != 0 }
}

/// Bytes waiting in the pipe, without reading them (None: the pipe is broken).
fn available(pipe: &File) -> Option<u32> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::Pipes::PeekNamedPipe;
    let mut avail: u32 = 0;
    // SAFETY: the handle is the open pipe's; no buffer is read into, only the count of waiting bytes is written.
    let ok = unsafe { PeekNamedPipe(pipe.as_raw_handle() as _, std::ptr::null_mut(), 0, std::ptr::null_mut(), &mut avail, std::ptr::null_mut()) };
    if ok == 0 { None } else { Some(avail) }
}

/// Reads exactly `buf.len()` bytes before `until`, polling the pipe so a silent Discord can't hang the call.
fn read_exactly(pipe: &mut File, buf: &mut [u8], until: Instant) -> Res<()> {
    let mut got = 0;
    while got < buf.len() {
        match available(pipe) {
            None => return Err(Fail::other("IOException: the pipe closed")),
            Some(0) => {
                if Instant::now() >= until {
                    return Err(Fail::other("OperationCanceledException: Discord didn't answer in time"));
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Some(n) => {
                let want = (buf.len() - got).min(n as usize);
                let read = pipe.read(&mut buf[got..got + want])?;
                if read == 0 {
                    return Err(Fail::other("IOException: the pipe closed"));
                }
                got += read;
            }
        }
    }
    Ok(())
}

fn write_frame(pipe: &mut File, op: i32, json: &str) -> Res<()> {
    let body = json.as_bytes();
    let mut frame = Vec::with_capacity(8 + body.len());
    frame.extend_from_slice(&op.to_le_bytes());
    frame.extend_from_slice(&(body.len() as i32).to_le_bytes());
    frame.extend_from_slice(body);
    // PORT: the C# gives the write 3 s too; a blocking pipe write has no time limit (Discord reads at once).
    pipe.write_all(&frame)?;
    pipe.flush()?;
    Ok(())
}

fn read_frame(pipe: &mut File) -> Res<(i32, String)> {
    let until = Instant::now() + ANSWER_WITHIN;
    let mut head = [0u8; 8];
    read_exactly(pipe, &mut head, until)?;
    let op = i32::from_le_bytes([head[0], head[1], head[2], head[3]]);
    let length = i32::from_le_bytes([head[4], head[5], head[6], head[7]]);
    if !(0..=1 << 20).contains(&length) {
        return Err(Fail::other("Discord sent a frame of an unexpected size."));
    }
    let mut body = vec![0u8; length as usize];
    read_exactly(pipe, &mut body, until)?;
    Ok((op, String::from_utf8_lossy(&body).into_owned()))
}

fn close(s: &mut State) {
    s.pipe = None;
    s.client_id = None;
    s.sent = None;
}
