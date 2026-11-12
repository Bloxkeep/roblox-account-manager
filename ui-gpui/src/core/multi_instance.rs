//! Roblox allows one client per session through a named event, "ROBLOX_singletonEvent": a client opening it tells the
//! running one to close. Taking the name first as a mutex makes Roblox's create fail (one object per name, whatever its
//! kind), so clients run side by side. Holding the event itself stopped working with October 2026 builds. Nothing is
//! injected into or read from Roblox. Names the Roblox watch finds (RobloxWatch) are taken the same way.
use std::sync::Mutex;

use windows_sys::Win32::{
    Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError},
    System::Threading::{CreateMutexW, ReleaseMutex},
};

use super::{
    base::{Res, Stamp},
    win_kit::{Hive, RegKey, stamp_to_filetime, wide},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayAutostart {
    pub value_name: String,
    pub enabled: bool,
}

pub const EVENT_NAME: &str = "ROBLOX_singletonEvent";
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const STARTUP_APPROVED_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

struct Held {
    /// Each mutex (its handle) and whether this run owns it.
    held: Vec<(isize, bool)>,
    held_names: Vec<String>,
    extra: Vec<String>,
    armed: bool,
}

static STATE: Mutex<Held> = Mutex::new(Held { held: Vec::new(), held_names: Vec::new(), extra: Vec::new(), armed: false });

fn state() -> std::sync::MutexGuard<'static, Held> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// True while every name is held as a mutex — by this app, or by another multi-instance tool (then it works too).
/// False when a Roblox client got there first: the name is its event, until every client has closed.
pub fn armed() -> bool {
    state().armed
}

/// The checks set it as the C# tests set the Armed property (nothing is taken or let go).
pub(crate) fn set_armed(value: bool) {
    state().armed = value;
}

/// The names held besides EVENT_NAME (the Roblox watch's), for the page.
pub fn extra_held() -> Vec<String> {
    let s = state();
    if s.armed { s.held_names.iter().skip(1).cloned().collect() } else { Vec::new() }
}

/// Held, but the names to hold changed since: taken again once no Roblox client is running.
pub fn stale() -> bool {
    let s = state();
    s.armed && s.held_names != wanted(&s)
}

/// Names to hold next to EVENT_NAME from the next `enable` on.
pub fn set_extra_names(names: &[String]) {
    let mut out: Vec<String> = Vec::new();
    for n in names {
        if n != EVENT_NAME && !out.contains(n) {
            out.push(n.clone());
        }
    }
    state().extra = out;
}

fn wanted(s: &Held) -> Vec<String> {
    std::iter::once(EVENT_NAME.to_string()).chain(s.extra.iter().cloned()).collect()
}

/// Must be called on a thread that stays (the UI thread): a mutex is owned by the thread that creates it.
pub fn enable() {
    let mut s = state();
    disable_locked(&mut s);
    let names = wanted(&s);
    for name in &names {
        let n = wide(name);
        // SAFETY: plain call; the handle is kept in `held` and closed by `disable`.
        let (handle, owned) = unsafe {
            let h = CreateMutexW(std::ptr::null(), 1, n.as_ptr());
            (h, GetLastError() != ERROR_ALREADY_EXISTS)
        };
        // The name is Roblox's event (a client is running, perhaps the one Windows starts in the tray): the
        // process check tries again once no client is running.
        if handle.is_null() {
            disable_locked(&mut s);
            return;
        }
        s.held.push((handle as isize, owned));
    }
    s.held_names = names;
    s.armed = true;
}

pub fn disable() {
    disable_locked(&mut state());
}

fn disable_locked(s: &mut Held) {
    for &(handle, owned) in &s.held {
        // SAFETY: the handles were made by `enable` and are released and closed once.
        unsafe {
            if owned {
                ReleaseMutex(handle as _); // ApplicationException (not owned by this thread) is let go
            }
            CloseHandle(handle as _);
        }
    }
    s.held.clear();
    s.held_names.clear();
    s.armed = false;
}

/// Roblox's start-with-Windows entry ("--launch-to-tray"), whose hidden client grabs the singleton first.
pub fn find_tray_autostart() -> Res<Option<TrayAutostart>> {
    let Some(run) = RegKey::open(Hive::CurrentUser, RUN_KEY, false)? else {
        return Ok(None);
    };
    for name in run.value_names() {
        let Some(command) = run.get_string(&name) else {
            continue;
        };
        let lower = command.to_lowercase();
        if !lower.contains("robloxplayerbeta") || !lower.contains("--launch-to-tray") {
            continue;
        }
        let approved = RegKey::open(Hive::CurrentUser, STARTUP_APPROVED_KEY, false)?;
        // Task Manager marks a disabled startup entry with an odd first byte (0x03); missing means enabled.
        let enabled = match approved.and_then(|k| k.get_binary(&name)) {
            Some(flags) if !flags.is_empty() => flags[0] & 1 == 0,
            _ => true,
        };
        return Ok(Some(TrayAutostart { value_name: name, enabled }));
    }
    Ok(None)
}

/// Same effect as switching the entry off in Task Manager › Startup apps (and reversible there).
pub fn disable_tray_autostart(entry: &TrayAutostart) -> Res<()> {
    let key = RegKey::create(Hive::CurrentUser, STARTUP_APPROVED_KEY)?;
    let mut data = [0u8; 12];
    data[0] = 0x03;
    data[4..12].copy_from_slice(&stamp_to_filetime(Stamp::utc_now()).to_le_bytes());
    key.set_binary(&entry.value_name, &data)
}
