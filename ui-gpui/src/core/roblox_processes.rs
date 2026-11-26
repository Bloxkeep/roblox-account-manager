//! Finds Roblox clients and their windows, closes them, hides and arranges their windows.
use std::{
    collections::{HashMap, HashSet},
    ffi::c_void,
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicI32, Ordering},
    },
    time::Duration,
};

use regex::Regex;
use windows_sys::Win32::{
    Foundation::{CloseHandle, FILETIME, GetLastError, HWND, LPARAM, RECT},
    Graphics::Dwm::DwmGetWindowAttribute,
    System::Threading::{GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, SetProcessInformation, TerminateProcess, WaitForSingleObject},
    UI::{
        HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi},
        WindowsAndMessaging::{
            EnumWindows, GW_OWNER, GetForegroundWindow, GetWindow, GetWindowLongW, GetWindowPlacement, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
            GetWindowThreadProcessId, IsHungAppWindow, IsIconic, IsWindow, IsWindowVisible, IsZoomed, PostMessageW, SMTO_ABORTIFHUNG, SendMessageTimeoutW,
            SetForegroundWindow, SetWindowPlacement, SetWindowPos, ShowWindowAsync, WINDOWPLACEMENT,
        },
    },
};

use super::{
    base::Stamp,
    input_turn, log,
    win_kit::{Point, Rect, Size, file_stem, filetime_to_stamp, hwnd, pids_by_name, process_list, process_name, screen_from_rect},
};

const PLAYER_PROCESS_NAME: &str = "RobloxPlayerBeta";
const WM_SETTEXT: u32 = 0x000C;
const WM_CLOSE: u32 = 0x0010;
const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOMOVE: u32 = 0x0002;
const SWP_NOZORDER: u32 = 0x0004;
const SWP_NOACTIVATE: u32 = 0x0010;
const SWP_NOSENDCHANGING: u32 = 0x0400;
const SWP_ASYNCWINDOWPOS: u32 = 0x4000;
const SW_RESTORE: i32 = 9;
const SW_SHOWMINNOACTIVE: i32 = 7;
const SW_SHOWMAXIMIZED: i32 = 3;
const GWL_STYLE: i32 = -16;
const WS_CAPTION: i32 = 0x00C00000;
const SYNCHRONIZE: u32 = 0x00100000;
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
const HWND_TOPMOST: isize = -1;
const HWND_NOTOPMOST: isize = -2;
const HWND_BOTTOM: isize = 1;

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtQuerySystemInformation(info_class: i32, buffer: *mut c_void, length: i32, returned: *mut i32) -> i32;
    fn NtQueryInformationProcess(process: *mut c_void, info_class: i32, buffer: *mut c_void, length: i32, returned: *mut i32) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn SetProcessWorkingSetSizeEx(process: *mut c_void, minimum: usize, maximum: usize, flags: u32) -> i32;
    fn GetProcessWorkingSetSizeEx(process: *mut c_void, minimum: *mut usize, maximum: *mut usize, flags: *mut u32) -> i32;
}

/// PROCESS_MEMORY_COUNTERS_EX.
#[repr(C)]
#[derive(Default)]
struct ProcessMemoryCountersEx {
    cb: u32,
    page_fault_count: u32,
    peak_working_set_size: usize,
    working_set_size: usize,
    quota_peak_paged_pool_usage: usize,
    quota_paged_pool_usage: usize,
    quota_peak_non_paged_pool_usage: usize,
    quota_non_paged_pool_usage: usize,
    pagefile_usage: usize,
    peak_pagefile_usage: usize,
    private_usage: usize,
}

#[link(name = "psapi")]
unsafe extern "system" {
    fn EmptyWorkingSet(process: *mut c_void) -> i32;
    fn GetProcessMemoryInfo(process: *mut c_void, counters: *mut ProcessMemoryCountersEx, cb: u32) -> i32;
}

// ───────── small Win32 wrappers ─────────

fn w(h: isize) -> HWND {
    hwnd(h)
}
fn is_window(h: isize) -> bool {
    // SAFETY: plain query of a handle.
    unsafe { IsWindow(w(h)) != 0 }
}
fn is_iconic(h: isize) -> bool {
    // SAFETY: plain query of a handle.
    unsafe { IsIconic(w(h)) != 0 }
}
fn is_zoomed(h: isize) -> bool {
    // SAFETY: plain query of a handle.
    unsafe { IsZoomed(w(h)) != 0 }
}
fn is_visible(h: isize) -> bool {
    // SAFETY: plain query of a handle.
    unsafe { IsWindowVisible(w(h)) != 0 }
}
fn is_hung(h: isize) -> bool {
    // SAFETY: plain query of a handle.
    unsafe { IsHungAppWindow(w(h)) != 0 }
}
fn show_async(h: isize, cmd: i32) -> bool {
    // SAFETY: plain call; posted to the window's thread.
    unsafe { ShowWindowAsync(w(h), cmd) != 0 }
}
fn set_window_pos(h: isize, after: isize, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> bool {
    // SAFETY: plain call.
    unsafe { SetWindowPos(w(h), w(after), x, y, cx, cy, flags) != 0 }
}
fn set_foreground(h: isize) -> bool {
    // SAFETY: plain call.
    unsafe { SetForegroundWindow(w(h)) != 0 }
}
fn style(h: isize) -> i32 {
    // SAFETY: plain query of a handle.
    unsafe { GetWindowLongW(w(h), GWL_STYLE) }
}
fn owner_pid(h: isize) -> u32 {
    let mut pid = 0u32;
    // SAFETY: one out value on the stack.
    unsafe {
        GetWindowThreadProcessId(w(h), &mut pid);
    }
    pid
}
fn sleep_ms(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}
fn filetime(ft: &FILETIME) -> i64 {
    ((ft.dwHighDateTime as i64) << 32) | ft.dwLowDateTime as i64
}

/// A process handle, closed when dropped.
struct ProcHandle(*mut c_void);

impl ProcHandle {
    fn open(access: u32, pid: i32) -> Option<ProcHandle> {
        // SAFETY: plain call; the handle is closed by Drop.
        let h = unsafe { OpenProcess(access, 0, pid as u32) };
        if h.is_null() { None } else { Some(ProcHandle(h)) }
    }
}

impl Drop for ProcHandle {
    fn drop(&mut self) {
        // SAFETY: the handle was opened by `open` and is closed once.
        unsafe {
            CloseHandle(self.0);
        }
    }
}

/// GetProcessTimes' creation time; None when refused.
fn creation_time(handle: *mut c_void) -> Option<i64> {
    let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let (mut created, mut exit, mut kernel, mut user) = (zero, zero, zero, zero);
    // SAFETY: four out values on the stack.
    if unsafe { GetProcessTimes(handle, &mut created, &mut exit, &mut kernel, &mut user) } != 0 { Some(filetime(&created)) } else { None }
}

// ───────── the clients ─────────

/// `parent`: may have exited since; 0 when not known.
/// `created`: a FILETIME, UTC: a pid Windows gave to a newer process has another one. 0 when not known.
/// `private`: private bytes, its share of the commit limit; 0 when not known.
/// `working_set`: its RAM in use, with what it shares with every other program (Roblox's own code, Windows'
/// libraries, the graphics driver): Windows holds that once for all of them.
/// `private_working_set`: its RAM in use that is its own (Task Manager's "Memory"); 0 when not read.
/// `page_faults`: its page faults so far, and `hard_faults` those read from disk (0 when not read): a window
/// short of RAM it needs keeps faulting (RAM limit, Auto).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerProcess {
    pub pid: i32,
    pub working_set: i64,
    pub parent: i32,
    pub created: i64,
    pub private: i64,
    pub private_working_set: i64,
    pub page_faults: i64,
    pub hard_faults: i64,
}

impl PlayerProcess {
    /// The RAM it takes, as shown (cards, sidebar total, Mini, Game monitor, Discord): its own, so windows added up
    /// count what they share once, not once each (5 windows were shown about 1 GB more than they took, 2026-10-29).
    pub fn ram(&self) -> i64 {
        if self.private_working_set > 0 { self.private_working_set } else { self.working_set }
    }
}

/// A pid acted on later must still be that client (see `close`).
static CREATED: LazyLock<Mutex<HashMap<i32, i64>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// From one system-wide snapshot: no handle to Roblox is opened, and one snapshot answers a whole process check.
pub fn get_players() -> Vec<PlayerProcess> {
    let players = snapshot().unwrap_or_else(snapshot_by_name);
    let mut created = CREATED.lock().unwrap_or_else(|e| e.into_inner());
    created.clear();
    for p in &players {
        if p.created != 0 {
            created.insert(p.pid, p.created);
        }
    }
    players
}

/// Where a running client's file is, for roblox_installs::note_running: asked only while no client seen before is
/// still on disk (a PC where Roblox's usual places have none), once a client.
pub fn note_client_files(players: &[PlayerProcess]) {
    static ASKED: LazyLock<Mutex<HashSet<(i32, i64)>>> = LazyLock::new(|| Mutex::new(HashSet::new()));
    if players.is_empty() || super::roblox_installs::seen_client().is_some() {
        return;
    }
    let mut asked = ASKED.lock().unwrap_or_else(|e| e.into_inner());
    if asked.len() > 500 {
        asked.clear();
    }
    for p in players {
        if p.created == 0 || !asked.insert((p.pid, p.created)) {
            continue;
        }
        let Some(h) = ProcHandle::open(PROCESS_QUERY_LIMITED_INFORMATION, p.pid) else { continue };
        let mut path = vec![0u16; 1024];
        let mut size = path.len() as u32;
        // SAFETY: the buffer holds `size` chars.
        if creation_time(h.0) == Some(p.created) && unsafe { QueryFullProcessImageNameW(h.0, 0, path.as_mut_ptr(), &mut size) } != 0 {
            super::roblox_installs::note_running(&String::from_utf16_lossy(&path[..size as usize]));
        }
    }
}

/// Without parents or start times: where `snapshot` can't read one.
/// PORT: .NET's Process read its working set and private bytes from the system's list; here they are asked of each
/// process (GetProcessMemoryInfo), and are 0 where Roblox refuses the handle.
fn snapshot_by_name() -> Vec<PlayerProcess> {
    let Some(pids) = pids_by_name(PLAYER_PROCESS_NAME) else {
        return Vec::new();
    };
    pids.into_iter()
        .map(|pid| {
            let (mut working_set, mut private) = (0i64, 0i64);
            if let Some(h) = ProcHandle::open(PROCESS_QUERY_LIMITED_INFORMATION, pid) {
                let mut c = ProcessMemoryCountersEx { cb: std::mem::size_of::<ProcessMemoryCountersEx>() as u32, ..Default::default() };
                // SAFETY: cb is set to the struct's size.
                if unsafe { GetProcessMemoryInfo(h.0, &mut c, c.cb) } != 0 {
                    working_set = c.working_set_size as i64;
                    private = c.private_usage as i64;
                }
            }
            PlayerProcess { pid, working_set, private, ..Default::default() }
        })
        .collect()
}

const SYSTEM_PROCESS_INFORMATION: i32 = 5;
const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004_u32 as i32;
/// So the next snapshot usually takes one call.
static SNAPSHOT_SIZE: AtomicI32 = AtomicI32::new(512 * 1024);
/// The last snapshot's buffer, kept for the next: half a MB or more isn't asked of Windows and given back every tick.
/// Taken by one caller at a time; another meanwhile makes its own. (u64s: aligned as the entries need.)
static SNAPSHOT_BUFFER: Mutex<Option<Vec<u64>>> = Mutex::new(None);

/// The kept buffer when it is free and big enough, else a new one.
fn take_snapshot_buffer(size: usize) -> Vec<u64> {
    let mut kept = SNAPSHOT_BUFFER.lock().unwrap_or_else(|e| e.into_inner());
    if kept.as_ref().is_some_and(|b| b.len() * 8 >= size) {
        return kept.take().unwrap_or_default();
    }
    drop(kept);
    vec![0u64; size.div_ceil(8)]
}

/// Kept for the next snapshot unless one is kept already (then freed); a smaller one kept before is freed.
fn keep_snapshot_buffer(buffer: Vec<u64>) {
    let mut kept = SNAPSHOT_BUFFER.lock().unwrap_or_else(|e| e.into_inner());
    if kept.as_ref().is_none_or(|k| k.len() < buffer.len()) {
        *kept = Some(buffer);
    }
}

/// NtQuerySystemInformation, as Process.GetProcessesByName reads it, but with parents and start times and without
/// an object per process. 64-bit SYSTEM_PROCESS_INFORMATION layout; None in a 32-bit process or when refused.
fn snapshot() -> Option<Vec<PlayerProcess>> {
    if std::mem::size_of::<usize>() != 8 {
        return None;
    }
    const WORKING_SET_PRIVATE: usize = 8;
    const HARD_FAULT_COUNT: usize = 16;
    const CREATE_TIME: usize = 32;
    const NAME_LENGTH: usize = 56;
    const NAME_BUFFER: usize = 64;
    const PROCESS_ID: usize = 80;
    const PARENT_ID: usize = 88;
    const PAGE_FAULT_COUNT: usize = 128;
    const WORKING_SET_SIZE: usize = 144;
    const PRIVATE_PAGE_COUNT: usize = 200;
    const ENTRY_END: usize = 208;
    let image: Vec<u16> = format!("{PLAYER_PROCESS_NAME}.exe").encode_utf16().collect();
    for _attempt in 0..4 {
        let size = SNAPSHOT_SIZE.load(Ordering::SeqCst);
        let mut buffer = take_snapshot_buffer(size as usize);
        let mut needed = 0i32;
        // SAFETY: the buffer holds at least `size` bytes; entries are read only inside the `needed` bytes returned.
        let status = unsafe { NtQuerySystemInformation(SYSTEM_PROCESS_INFORMATION, buffer.as_mut_ptr() as *mut c_void, size, &mut needed) };
        if status == STATUS_INFO_LENGTH_MISMATCH {
            SNAPSHOT_SIZE.store(needed.max(size) + 64 * 1024, Ordering::SeqCst);
            keep_snapshot_buffer(buffer);
            continue;
        }
        if status < 0 || needed > size {
            keep_snapshot_buffer(buffer);
            return None;
        }
        let base = buffer.as_ptr() as *const u8;
        let end = base as usize + needed as usize;
        let mut players = Vec::new();
        let mut offset = 0usize;
        // SAFETY: as above; the name pointer is checked to lie inside the buffer before it is read.
        unsafe {
            let rd64 = |e: *const u8, at: usize| std::ptr::read_unaligned(e.add(at) as *const i64);
            let rd32 = |e: *const u8, at: usize| std::ptr::read_unaligned(e.add(at) as *const i32);
            while offset + ENTRY_END <= needed as usize {
                let entry = base.add(offset);
                let length = std::ptr::read_unaligned(entry.add(NAME_LENGTH) as *const u16) as usize;
                let name = std::ptr::read_unaligned(entry.add(NAME_BUFFER) as *const usize);
                if length == image.len() * 2 && name >= base as usize && name + length <= end && is_image(name as *const u16, &image) {
                    players.push(PlayerProcess {
                        pid: rd64(entry, PROCESS_ID) as i32,
                        working_set: rd64(entry, WORKING_SET_SIZE),
                        parent: rd64(entry, PARENT_ID) as i32,
                        created: rd64(entry, CREATE_TIME),
                        private: rd64(entry, PRIVATE_PAGE_COUNT),
                        private_working_set: rd64(entry, WORKING_SET_PRIVATE),
                        page_faults: rd32(entry, PAGE_FAULT_COUNT) as u32 as i64,
                        hard_faults: rd32(entry, HARD_FAULT_COUNT) as u32 as i64,
                    });
                }
                let next = rd32(entry, 0);
                if next <= 0 {
                    break;
                }
                offset += next as usize;
            }
        }
        keep_snapshot_buffer(buffer);
        return Some(players);
    }
    None
}

/// The name's characters equal the image's, compared as char.ToUpperInvariant does.
///
/// # Safety
/// `chars` points at `image.len()` readable UTF-16 units.
unsafe fn is_image(chars: *const u16, image: &[u16]) -> bool {
    let upper = |u: u16| char::from_u32(u as u32).and_then(|c| c.to_uppercase().next()).map(|c| c as u32).unwrap_or(u as u32);
    for (i, &want) in image.iter().enumerate() {
        // SAFETY: inside the range the caller vouched for.
        let got = unsafe { std::ptr::read_unaligned(chars.add(i)) };
        if upper(got) != upper(want) {
            return false;
        }
    }
    true
}

pub fn get_player_pids() -> Vec<i32> {
    get_players().iter().map(|p| p.pid).collect()
}

unsafe extern "system" fn collect_window(h: HWND, data: LPARAM) -> i32 {
    // SAFETY: `data` is the &mut HashMap passed by visible_windows, alive for the whole EnumWindows call.
    let result = unsafe { &mut *(data as *mut HashMap<i32, Vec<isize>>) };
    let mut owner = 0u32;
    // SAFETY: plain queries of the window being listed.
    unsafe {
        GetWindowThreadProcessId(h, &mut owner);
    }
    if let Some(list) = result.get_mut(&(owner as i32)) {
        // SAFETY: plain queries of the window being listed.
        let unowned = unsafe { GetWindow(h, GW_OWNER) }.is_null();
        if (is_visible(h as isize) || is_hidden(h as isize)) && unowned {
            list.push(h as isize);
        }
    }
    1
}

/// One pass over the desktop's windows. A window Bloxkeep hid (`hide_windows`) counts as visible, so
/// every check that a client has its window (not a tray copy, matched to its launch) still holds.
/// The clients with a window Windows calls "not responding" (no answer for 5 s: IsHungAppWindow), in one pass over the
/// desktop's windows; no process is opened.
pub fn hung_pids(pids: &[i32]) -> HashSet<i32> {
    visible_windows(pids).into_iter().filter(|(_, list)| list.iter().any(|h| is_hung(*h))).map(|(pid, _)| pid).collect()
}

pub fn visible_windows(pids: &[i32]) -> HashMap<i32, Vec<isize>> {
    let mut result: HashMap<i32, Vec<isize>> = pids.iter().map(|&pid| (pid, Vec::new())).collect();
    if result.is_empty() {
        return result;
    }
    // SAFETY: the callback only touches `result`, which outlives the call.
    unsafe {
        EnumWindows(Some(collect_window), &mut result as *mut HashMap<i32, Vec<isize>> as LPARAM);
    }
    result
}

// ───────── windows hidden while their game runs ─────────
// Shown again when Bloxkeep closes, or at its next start after a crash: the windows it hid are written down (pid
// and window) in the data folder.

const SW_HIDE: i32 = 0;
const SW_SHOWNA: i32 = 8;
/// Each with its client: a handle reused by another window is never shown or counted for it.
static HIDDEN: LazyLock<Mutex<HashMap<isize, i32>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn hidden_file() -> std::path::PathBuf {
    crate::core::vault::data_dir().join("hidden-windows.txt")
}

fn hidden_map() -> std::sync::MutexGuard<'static, HashMap<isize, i32>> {
    HIDDEN.lock().unwrap_or_else(|e| e.into_inner())
}

/// The window is still there and still that process's (a handle can be taken by another window).
pub fn still_of(h: isize, pid: i32) -> bool {
    let mut owner = 0u32;
    // SAFETY: plain queries of a handle.
    is_window(h) && unsafe { GetWindowThreadProcessId(w(h), &mut owner) } != 0 && owner as i32 == pid
}

pub fn is_hidden(h: isize) -> bool {
    let pid = hidden_map().get(&h).copied();
    pid.is_some_and(|pid| still_of(h, pid))
}

pub fn has_hidden(pid: i32) -> bool {
    let list: Vec<isize> = hidden_map().iter().filter(|(_, p)| **p == pid).map(|(h, _)| *h).collect();
    list.into_iter().any(|h| still_of(h, pid))
}

/// True when the ended client's window was hidden, so the window auto-rejoin opens is hidden too.
pub fn forget_hidden(pid: i32) -> bool {
    let was = {
        let mut map = hidden_map();
        let before = map.len();
        map.retain(|_, p| *p != pid);
        map.len() != before
    };
    if was {
        save_hidden();
    }
    was
}

/// Returns how many were hidden now.
pub fn hide_windows(pids: &[i32]) -> i32 {
    let mut count = 0;
    for &pid in pids {
        let h = main_window(pid);
        if h == 0 || is_hidden(h) {
            continue;
        }
        hidden_map().insert(h, pid);
        show_async(h, SW_HIDE);
        count += 1;
    }
    save_hidden();
    count
}

/// All when None; a minimized one stays so.
pub fn show_windows(pids: Option<&[i32]>) -> i32 {
    let mut shown: Vec<(isize, i32)> = {
        let mut map = hidden_map();
        let shown: Vec<(isize, i32)> = map.iter().filter(|(_, p)| pids.is_none_or(|only| only.contains(p))).map(|(h, p)| (*h, *p)).collect();
        for (h, _) in &shown {
            map.remove(h);
        }
        shown
    };
    shown.retain(|&(h, pid)| still_of(h, pid));
    for &(h, _) in &shown {
        show_async(h, SW_SHOWNA);
    }
    save_hidden();
    shown.len() as i32
}

/// At start: shows windows a crashed run left hidden (only those still there, of the same process).
pub fn show_left_hidden() -> i32 {
    let mut shown = 0;
    let path = hidden_file();
    if !path.is_file() {
        return 0;
    }
    let done = (|| -> std::io::Result<()> {
        for line in std::fs::read_to_string(&path)?.lines() {
            let parts: Vec<&str> = line.split(' ').collect();
            if parts.len() != 2 {
                continue;
            }
            let (Ok(pid), Ok(handle)) = (parts[0].trim().parse::<i32>(), parts[1].trim().parse::<i64>()) else {
                continue;
            };
            let h = handle as isize;
            if still_of(h, pid) && !is_visible(h) {
                show_async(h, SW_SHOWNA);
                shown += 1;
            }
        }
        std::fs::remove_file(&path)
    })();
    if let Err(ex) = done {
        log::write(&ex);
    }
    shown
}

fn save_hidden() {
    let entries: Vec<(isize, i32)> = hidden_map().iter().map(|(h, p)| (*h, *p)).collect();
    let lines: Vec<String> = entries.into_iter().filter(|&(h, p)| still_of(h, p)).map(|(h, p)| format!("{p} {}", h as i64)).collect();
    let done = if lines.is_empty() {
        match std::fs::remove_file(hidden_file()) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    } else {
        // File.WriteAllLines: each line ended by a newline.
        std::fs::write(hidden_file(), lines.iter().map(|l| format!("{l}\r\n")).collect::<String>())
    };
    if let Err(ex) = done {
        log::write(&ex);
    }
}

fn top_level_windows(pid: i32) -> Vec<isize> {
    visible_windows(&[pid]).remove(&pid).unwrap_or_default()
}

fn get_title(h: isize) -> String {
    // SAFETY: the buffer holds length + 1 chars, as told to Windows.
    unsafe {
        let length = GetWindowTextLengthW(w(h));
        if length <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; length as usize + 1];
        let n = GetWindowTextW(w(h), buf.as_mut_ptr(), buf.len() as i32);
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }
}

pub fn set_windows_title(windows: &[isize], title: &str) {
    let text: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    for &h in windows {
        let current = get_title(h);
        if current == title || !current.starts_with("Roblox") {
            continue;
        }
        let mut result = 0usize;
        // SAFETY: the text lives on this frame for the call, which waits at most 500 ms.
        unsafe {
            SendMessageTimeoutW(w(h), WM_SETTEXT, 0, text.as_ptr() as LPARAM, SMTO_ABORTIFHUNG, 500, &mut result);
        }
    }
}

/// When to end it if its windows didn't close, how long to end its children for.
/// `created`: 0 when not known: a process that has its id later is another one.
#[derive(Clone, Copy)]
struct Closing {
    kill_at: Option<Stamp>,
    until: Stamp,
    handle: isize,
    created: i64,
    /// Its windows were asked to close (not ended at once): Roblox then starts its tray copy as it goes.
    soft: bool,
}

/// Path.IsPathFullyQualified.
fn is_path_fully_qualified(path: &str) -> bool {
    let b = path.as_bytes();
    (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/'))
        || (b.len() >= 2 && (b[0] == b'\\' || b[0] == b'/') && (b[1] == b'\\' || b[1] == b'/'))
}

/// The executable path as supplied at launch. QueryFullProcessImageName resolves junctions, hiding which alias
/// was used; the command line retains it. Only the first argument is returned, never tickets in later arguments.
pub fn launch_path(pid: i32, expected_creation: i64) -> Option<String> {
    let line = command_line_of(pid, expected_creation)?;
    let line = line.trim_start();
    if line.is_empty() {
        return None;
    }
    let path = if let Some(rest) = line.strip_prefix('"') {
        // line.IndexOf('"', 1) < 1: no closing quote.
        let end = rest.find('"')?;
        &rest[..end]
    } else {
        match line.find([' ', '\t']) {
            Some(end) => &line[..end],
            None => line,
        }
    };
    if is_path_fully_qualified(path) { Some(path.to_string()) } else { None }
}

/// The id of a client that ended between two process checks may already belong to another process. Asked of the
/// handle when Windows gives one (then the id can't change hands), else of a fresh snapshot.
/// Returns (still the client, when the process with this id now started (0 when not known), when the client the last
/// snapshot saw started (0 when not known)).
fn still_the_client(pid: i32, handle: Option<&ProcHandle>) -> (bool, i64, i64) {
    let expected = CREATED.lock().unwrap_or_else(|e| e.into_inner()).get(&pid).copied().unwrap_or(0);
    if let Some(h) = handle {
        if let Some(created) = creation_time(h.0) {
            let mut path = vec![0u16; 1024];
            let mut size = path.len() as u32;
            // SAFETY: the buffer holds `size` chars.
            if unsafe { QueryFullProcessImageNameW(h.0, 0, path.as_mut_ptr(), &mut size) } != 0 {
                let name = String::from_utf16_lossy(&path[..size as usize]);
                return (file_stem(&name).eq_ignore_ascii_case(PLAYER_PROCESS_NAME) && (expected == 0 || created == expected), created, expected);
            }
        }
    }
    // Roblox's anti-tamper may refuse the handle: Windows' process list says the same, without one.
    if let Some(now) = snapshot() {
        let found = now.iter().find(|p| p.pid == pid).copied().unwrap_or_default();
        return (found.pid == pid && (expected == 0 || found.created == expected), found.created, expected);
    }
    let same = process_name(pid).is_some_and(|n| n.eq_ignore_ascii_case(PLAYER_PROCESS_NAME));
    (same, expected, expected)
}

struct ClosingState {
    map: HashMap<i32, Closing>,
    sweeping: bool,
}

static CLOSING: LazyLock<Mutex<ClosingState>> = LazyLock::new(|| Mutex::new(ClosingState { map: HashMap::new(), sweeping: false }));

fn closing() -> std::sync::MutexGuard<'static, ClosingState> {
    CLOSING.lock().unwrap_or_else(|e| e.into_inner())
}

/// Being closed by `close` (or its sweep for the tray copy it leaves), until the sweep is done with it.
pub fn is_closing(pid: i32) -> bool {
    closing().map.contains_key(&pid)
}

/// A window asked to close (not ended at once) whose sweep is still on. Roblox starts its tray copy seconds after such
/// a window goes (five, measured), and the sweep ends that copy; a client started meanwhile hands its launch to the
/// copy and quits with no window (a launch three seconds after a close did). Over within CHILD_SWEEP_SECS of the close.
pub fn soft_close_under_way() -> bool {
    closing().map.values().any(|c| c.soft)
}

const CLOSE_GRACE_SECS: f64 = 2.0;
const CHILD_SWEEP_SECS: f64 = 10.0;
const SWEEP_TICK: Duration = Duration::from_millis(500);

fn start_sweeper(s: &mut ClosingState) {
    if !s.sweeping {
        s.sweeping = true;
        crate::threads::run(sweep_closing);
    }
}

/// Asks its windows to close like their X, and terminates it 2 s later if still there; `force`
/// terminates at once. The tray copy Roblox starts as a child of the closing client is ended too. All closes share
/// one sweep, so closing ten takes no more snapshots than one.
pub fn close(pid: i32, force: bool) {
    // Every window Bloxkeep closes is said here, the one place they all pass (five callers wrote no line of their
    // own, audit 2026-11-15): with the caller's line before it, error.log shows who closed a window and why.
    log::note_file(&format!("Roblox window {pid}: closed by Bloxkeep{}.", if force { ", at once" } else { "" }));
    // Held open until the sweep ends, so Windows can't give this pid to a new process meanwhile. None when
    // Roblox's anti-tamper refuses it.
    let keep = ProcHandle::open(SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, pid);
    // It ended since the last process check and its id went to another process: nothing is closed.
    let (same, created, expected) = still_the_client(pid, keep.as_ref());
    if !same {
        drop(keep);
        // The tray copy it may have started as it ended is still swept.
        let mut s = closing();
        if !s.map.contains_key(&pid) {
            s.map.insert(pid, Closing { kill_at: None, until: Stamp::utc_now().plus_secs(CHILD_SWEEP_SECS), handle: 0, created: expected, soft: false });
        }
        start_sweeper(&mut s);
        return;
    }
    let windows = if force { Vec::new() } else { top_level_windows(pid) };
    for &h in &windows {
        // SAFETY: plain posted message.
        unsafe {
            PostMessageW(w(h), WM_CLOSE, 0, 0);
        }
    }
    if windows.is_empty() {
        kill(pid);
    }
    let now = Stamp::utc_now();
    let mut s = closing();
    if let Some(again) = s.map.get_mut(&pid) {
        // Closed again: the sweep may hold the first entry and closes its handle, so that entry is stretched
        // and the handle just opened is the spare.
        if !windows.is_empty() && !again.kill_at.is_some_and(|at| at < now.plus_secs(CLOSE_GRACE_SECS)) {
            again.kill_at = Some(now.plus_secs(CLOSE_GRACE_SECS));
        }
        again.until = now.plus_secs(CHILD_SWEEP_SECS);
        again.soft |= !windows.is_empty();
        drop(keep);
    } else {
        let handle = keep.map(|k| {
            let h = k.0 as isize;
            std::mem::forget(k); // closed by the sweep
            h
        });
        s.map.insert(
            pid,
            Closing {
                kill_at: if windows.is_empty() { None } else { Some(now.plus_secs(CLOSE_GRACE_SECS)) },
                until: now.plus_secs(CHILD_SWEEP_SECS),
                handle: handle.unwrap_or(0),
                created,
                soft: !windows.is_empty(),
            },
        );
    }
    start_sweeper(&mut s);
}

/// One snapshot per tick for every client being closed.
fn sweep_closing() {
    loop {
        std::thread::sleep(SWEEP_TICK);
        // One bad tick must not end the loop for good.
        if crate::core::program::catch_unwind(sweep_once).is_err() {
            log::note("Closing Roblox: a sweep failed.");
        }
        let mut s = closing();
        if s.map.is_empty() {
            s.sweeping = false;
            return;
        }
    }
}

fn sweep_once() {
    let items: Vec<(i32, Closing)> = closing().map.iter().map(|(p, c)| (*p, *c)).collect();
    let players = snapshot();
    let (tree, alive) = match &players {
        None => player_children_live(),
        Some(players) => (player_children(players), players.iter().map(|p| p.pid).collect::<HashSet<i32>>()),
    };
    let created: HashMap<i32, i64> = players.as_ref().map(|ps| ps.iter().map(|p| (p.pid, p.created)).collect()).unwrap_or_default();
    let same = |pid: i32, since: i64| since == 0 || created.get(&pid).is_none_or(|&at| at == since);
    let newer = |child: i32, since: i64| since == 0 || created.get(&child).is_none_or(|&at| at >= since);
    let now = Stamp::utc_now();
    for (pid, item) in items {
        // Read under the lock: a second close of the same client may be stretching the entry.
        let (kill_at, until) = closing().map.get(&pid).map(|c| (c.kill_at, c.until)).unwrap_or((item.kill_at, item.until));
        if let Some(at) = kill_at {
            if now >= at {
                if alive.contains(&pid) && same(pid, item.created) {
                    kill(pid);
                }
                if let Some(c) = closing().map.get_mut(&pid) {
                    if c.kill_at == Some(at) {
                        c.kill_at = None;
                    }
                }
            }
        }
        for &child in tree.get(&pid).map(|v| v.as_slice()).unwrap_or(&[]) {
            if newer(child, item.created) {
                kill(child);
            }
        }
        if now < until {
            continue;
        }
        let removed = {
            let mut s = closing();
            // Stretched meanwhile: not done with it yet.
            if s.map.get(&pid).is_some_and(|c| c.until != until) {
                continue;
            }
            s.map.remove(&pid)
        };
        if let Some(c) = removed {
            if c.handle != 0 {
                // SAFETY: the handle `close` kept for this entry, closed once as the entry goes.
                unsafe {
                    CloseHandle(c.handle as *mut c_void);
                }
            }
        }
    }
}

/// Clients by parent (which may have exited). A client started before the process its parent's id now belongs to
/// isn't that process's child (the id was handed on).
pub fn player_children(players: &[PlayerProcess]) -> HashMap<i32, Vec<i32>> {
    if !players.is_empty() && players.iter().all(|p| p.parent == 0) {
        return player_children_live().0;
    }
    let created: HashMap<i32, i64> = players.iter().map(|p| (p.pid, p.created)).collect();
    let mut tree: HashMap<i32, Vec<i32>> = HashMap::new();
    for p in players {
        if p.parent == 0 || p.parent == p.pid || created.get(&p.parent).is_some_and(|&pc| pc != 0 && p.created != 0 && pc > p.created) {
            continue;
        }
        tree.entry(p.parent).or_default().push(p.pid);
    }
    tree
}

/// Clients by parent from Toolhelp's list, and every client alive.
pub fn player_children_live() -> (HashMap<i32, Vec<i32>>, HashSet<i32>) {
    let mut tree: HashMap<i32, Vec<i32>> = HashMap::new();
    let mut alive = HashSet::new();
    let Some(list) = process_list() else {
        return (tree, alive);
    };
    let image = format!("{PLAYER_PROCESS_NAME}.exe");
    for entry in list {
        if !entry.exe.eq_ignore_ascii_case(&image) {
            continue;
        }
        let (pid, parent) = (entry.pid as i32, entry.parent as i32);
        alive.insert(pid);
        if parent != pid {
            tree.entry(parent).or_default().push(pid);
        }
    }
    (tree, alive)
}

/// `focus` once a nudge, a picture or a macro's turn under way is over (input_turn).
pub fn focus_in_turn(pid: i32) {
    input_turn::run_when_free("focus", move || focus(pid));
}

pub fn focus(pid: i32) {
    let h = main_window(pid);
    if h == 0 {
        return;
    }
    // Show (Mini window, tray, Monitor) on a window Bloxkeep hid: shown again first, as Ctrl+Alt+H shows it, so it
    // is known as shown; brought to the front hidden, nothing came up (review 2026-10-30).
    if is_hidden(h) {
        show_windows(Some(&[pid]));
    }
    if is_iconic(h) {
        show_async(h, SW_RESTORE);
    }
    let place = if is_plain_window(h) { window_rect(h) } else { None };
    set_foreground(h);
    if let Some(before) = place {
        crate::threads::run(move || keep_place(h, before));
    }
}

/// Coming to the front sends Roblox the "changing" message, and it answers by growing back to its least size: a tile
/// Arrange made smaller came up bigger, over its neighbours, clicked in Game monitor. Grown where it stands (its
/// corner unmoved), it gets its tile back; moved or resized by hand, it is left as it is.
fn keep_place(h: isize, before: Rect) {
    for _ in 0..10 {
        sleep_ms(50);
        let Some(now) = window_rect(h) else {
            continue;
        };
        if now == before {
            continue;
        }
        if now.location() == before.location() && now.width >= before.width && now.height >= before.height && is_plain_window(h) {
            place_window(h, before);
        }
        return;
    }
}

pub fn is_minimised(h: isize) -> bool {
    is_iconic(h)
}

/// Only a window with a title bar (Roblox's fullscreen has none). Windows brings the window to the front with it.
/// Returns false when it left the window as it was.
/// Not a window Bloxkeep hid: maximising shows it (review 2026-10-30), and it stays known as hidden.
pub fn maximise(h: isize) -> bool {
    if h == 0 || is_hidden(h) || is_iconic(h) || is_zoomed(h) || (style(h) & WS_CAPTION) != WS_CAPTION {
        return false;
    }
    show_async(h, SW_SHOWMAXIMIZED)
}

/// Without bringing another one to the front. Not a window Bloxkeep hid: minimising shows it on the taskbar
/// (a hidden window rejoined into a light-AFK preset, review 2026-10-30).
/// False when nothing was asked (already minimised, hidden) or Windows refused the ask.
pub fn minimise(h: isize) -> bool {
    h != 0 && !is_hidden(h) && !is_iconic(h) && show_async(h, SW_SHOWMINNOACTIVE)
}

/// A minimized window opened again where it was, without taking the keyboard from the app in front.
pub fn restore_quietly(h: isize) {
    const SW_SHOWNOACTIVATE: i32 = 4;
    if h != 0 && is_iconic(h) {
        show_async(h, SW_SHOWNOACTIVATE);
    }
}

/// A maximised window back to an ordinary one, without taking the keyboard from the app in front.
/// False when it wasn't maximised or Windows refused the ask.
pub fn unmaximise_quietly(h: isize) -> bool {
    const SW_SHOWNOACTIVATE: i32 = 4;
    h != 0 && is_zoomed(h) && show_async(h, SW_SHOWNOACTIVATE)
}

/// The window is still there (any process's).
pub fn exists(h: isize) -> bool {
    h != 0 && is_window(h)
}

pub fn is_maximised(h: isize) -> bool {
    is_zoomed(h)
}

/// It has a title bar (Roblox's full screen has none).
pub fn has_caption(h: isize) -> bool {
    (style(h) & WS_CAPTION) == WS_CAPTION
}

/// Windows calls it "not responding" (no answer for 5 s).
pub fn is_hung_window(h: isize) -> bool {
    is_hung(h)
}

/// The process whose window is in front; zero for none. A minimised window keeps the foreground when it was put
/// away without handing it on (minimised without activating another, as on joining), yet nobody sees or plays it:
/// counted in front, it was left out of the RAM limit and held back from anti-AFK as if in use.
pub fn foreground_pid() -> i32 {
    // SAFETY: plain call.
    let h = unsafe { GetForegroundWindow() } as isize;
    if h == 0 || is_iconic(h) {
        return 0;
    }
    owner_pid(h) as i32
}

pub fn main_window(pid: i32) -> isize {
    main_window_of(&top_level_windows(pid))
}

/// The one titled "Roblox…", else the first; zero for none.
pub fn main_window_of(windows: &[isize]) -> isize {
    match windows.iter().find(|&&h| get_title(h).starts_with("Roblox")) {
        Some(&main) if main != 0 => main,
        _ => windows.first().copied().unwrap_or(0),
    }
}

pub const LEAST_WINDOW: Size = Size { width: 160, height: 120 };
pub const MOST_WINDOW: Size = Size { width: 7680, height: 4320 };

/// Where Bloxkeep's window comes back to from maximised. Its normal size stays, unless it can't fit its screen and runs
/// past it (2560 × 1398 on a 1380-high work area, 2026-11-01: back under the taskbar, and "restore" hardly changed
/// anything): then `preferred`, centred. A window snapped to an edge is only bigger by Windows'
/// invisible resize border (`slack`), and stays.
pub fn restored_place(r: Rect, area: Rect, slack: i32, preferred: Size) -> Rect {
    if (r.width <= area.width && r.height <= area.height) || area.inflated(slack, slack).contains_rect(r) {
        return r;
    }
    let size = Size::new(preferred.width.min(area.width), preferred.height.min(area.height));
    Rect::new(area.left() + (area.width - size.width) / 2, area.top() + (area.height - size.height) / 2, size.width, size.height)
}

/// See Settings.WindowLayout: grids up to 8 × 8.
pub fn is_layout(layout: &str) -> bool {
    matches!(layout, "fill" | "small" | "medium" | "large" | "custom") || grid_of(layout).is_some()
}

fn grid_of(layout: &str) -> Option<(i32, i32)> {
    let b = layout.as_bytes();
    if b.len() == 3 && b[1] == b'x' && (b'1'..=b'8').contains(&b[0]) && (b'1'..=b'8').contains(&b[2]) {
        Some(((b[0] - b'0') as i32, (b[2] - b'0') as i32))
    } else {
        None
    }
}

/// An Arrange's grid: each tile in order, whether they stand side by side (else at overlapping spots, more windows than
/// the area holds), and one tile's size.
#[derive(Clone, Debug, PartialEq)]
pub struct Tiles {
    pub every: Vec<Rect>,
    pub side_by_side: bool,
    pub cell: Size,
    pub cols: i32,
}

/// The tiles of `count` windows on `area` (`layout`: see `grid_cells`, "fill" by default).
pub fn tiles(count: i32, area: Rect, layout: &str, custom: Size) -> Tiles {
    let (cols, width, height) = grid_cells(count, area.size(), layout, custom);
    let cell = Size::new(width, height);
    let side_by_side = (count + cols - 1) / cols * height <= area.height;
    let every: Vec<Rect> = if side_by_side {
        (0..count.max(0)).map(|i| Rect::new(area.x + i % cols * width, area.y + i / cols * height, width, height)).collect()
    } else {
        overlap_spots(count, area, cell).iter().map(|&p| Rect::at(p, cell)).collect()
    };
    Tiles { every, side_by_side, cell, cols }
}

/// A minimised window opened again (clicked or by the hotkey: it comes up as the person asked). False when Windows
/// refused the ask.
pub(crate) fn open_window(h: isize) -> bool {
    h != 0 && show_async(h, SW_RESTORE)
}

/// Windows on another screen are moved onto the one asked for first, their size kept, and given time to take its scale.
/// Roblox sizes itself for a new scale late (150 % to 125 % on a real PC), over the tile just given: back at its least
/// size (800 × 600 in its own units), bigger than the tiles, so they were laid over each other and the screen showed one
/// window. Sized once on the screen, it takes the tile as on its own screen. Blocking.
pub(crate) fn onto_screen_first(windows: &[isize], area: Rect, note: &mut String) {
    let target = screen_from_rect(area).device_name;
    let elsewhere = |h: isize| window_rect(h).is_some_and(|r| screen_from_rect(r).device_name != target);
    let moving: Vec<isize> = windows.iter().copied().filter(|&h| elsewhere(h)).collect();
    if moving.is_empty() {
        return;
    }
    for (i, &h) in moving.iter().enumerate() {
        let i = i as i32;
        set_window_pos(h, 0, area.x + i * 24, area.y + i * 24, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOSENDCHANGING | SWP_ASYNCWINDOWPOS);
    }
    let until = std::time::Instant::now() + Duration::from_secs(2);
    while std::time::Instant::now() < until && moving.iter().any(|&h| is_window(h) && elsewhere(h)) {
        sleep_ms(100);
    }
    sleep_ms(500); // Roblox's own resize for the new scale
    note.push_str(&format!("{} moved from another screen first; ", moving.len()));
}

/// Until none of `windows` is maximised or minimised any more (a restore asked), 3 s at most. Blocking.
pub(crate) fn until_restored(windows: &[isize]) {
    let until = std::time::Instant::now() + Duration::from_secs(3);
    sleep_ms(100);
    while std::time::Instant::now() < until && windows.iter().any(|&h| is_window(h) && (is_zoomed(h) || is_iconic(h))) {
        sleep_ms(100);
    }
    sleep_ms(150); // the restore's own size settles
}

/// Open windows into their tiles (`tiles[i]` is `windows[i]`'s, from `grid`), as Arrange places them. Roblox keeps its
/// window at least 800 × 600 in its own units (1224 × 957 px at 150 %) by answering the "changing" message a move
/// sends: smaller tiles left windows overlapping past the screen's edge (2026-10-21). Sent without it
/// (SWP_NOSENDCHANGING), Roblox takes the size (tried on a real PC, 853 × 764). Windows that grow back anyway, or more
/// windows than fit, are spread with `overlap_spots` so every title bar shows. Returns where each was put: its tile, or
/// its spot when they were laid over each other. Blocking.
pub(crate) fn place_tiles(windows: &[isize], tiles: &[Rect], grid: &Tiles, area: Rect, note: &mut String) -> Vec<Rect> {
    if windows.is_empty() {
        return Vec::new();
    }
    let cell = grid.cell;
    if grid.side_by_side {
        for (i, &h) in windows.iter().enumerate() {
            let _ = move_then_size(h, 0, tiles[i], SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOSENDCHANGING | SWP_ASYNCWINDOWPOS);
        }
    } else {
        let spots: Vec<Point> = tiles.iter().map(|t| Point::new(t.x, t.y)).collect();
        stack(windows, &spots, cell, SWP_NOSENDCHANGING);
    }
    let mut kept = kept_size(windows, cell);
    // A window just moved from a screen at another scale can answer that scale after its tile came (one of four,
    // 1224 × 957 kept over a 640 × 460 tile, the next arrange took it): the tiles are given once more first.
    if kept.is_some() && grid.side_by_side {
        sleep_ms(500);
        for (i, &h) in windows.iter().enumerate() {
            let _ = move_then_size(h, 0, tiles[i], SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOSENDCHANGING | SWP_ASYNCWINDOWPOS);
        }
        kept = kept_size(windows, cell);
        if kept.is_none() {
            note.push_str("tiles given twice (a window took its new screen's scale late); ");
        }
    }
    if let Some(size) = kept {
        note.push_str(&format!("laid over each other: windows kept {}×{}, bigger than the {}×{} tiles; ", size.width, size.height, cell.width, cell.height));
        let spots = overlap_spots(windows.len() as i32, area, size);
        stack(windows, &spots, size, 0);
        return spots.iter().map(|&p| Rect::at(p, size)).collect();
    }
    note.push_str(&if grid.side_by_side {
        format!("tiled {} across, {}×{}; ", grid.cols, cell.width, cell.height)
    } else {
        format!("overlapping spots, {}×{}; ", cell.width, cell.height)
    });
    grow_shrunk(windows, tiles);
    if grid.side_by_side {
        close_gaps(windows, tiles);
    }
    tiles.to_vec()
}

/// One window into its tile as Arrange places it (a window that missed its tile in an Arrange, or was lent while it
/// ran): onto the tile's screen first, then the place, then the size, then over the tile by its own unseen borders.
/// False when Windows refused the ask. Blocking.
pub(crate) fn place_in_tile(h: isize, tile: Rect) -> bool {
    let mut note = String::new();
    onto_screen_first(&[h], tile, &mut note);
    if !move_then_size(h, 0, tile, SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOSENDCHANGING | SWP_ASYNCWINDOWPOS) {
        return false;
    }
    let until = std::time::Instant::now() + Duration::from_secs(1);
    while std::time::Instant::now() < until && window_rect(h).is_some_and(|r| r.size() != tile.size()) {
        sleep_ms(20);
    }
    close_gaps(&[h], &[tile]);
    true
}

/// Each window goes just under the next, in turn (each moves on its own thread), so a later one lies over an
/// earlier one's body, not its title bar.
fn stack(windows: &[isize], spots: &[Point], size: Size, flags: u32) {
    for i in (0..windows.len()).rev() {
        let last = i == windows.len() - 1;
        let _ = move_then_size(
            windows[i],
            if last { 0 } else { windows[i + 1] },
            Rect::at(spots[i], size),
            flags | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS | if last { SWP_NOZORDER } else { 0 },
        );
        sleep_ms(40);
    }
}

/// The place first, then the size. Moved onto a screen at another scale, Roblox (aware of each screen's scale) takes
/// the size Windows gives it for that scale as it gets there: given with the move, its tile's size was scaled again,
/// so windows from a 150 % screen tiled on a 100 % one came out a third smaller (review 2026-10-30).
/// False when Windows refused either ask.
fn move_then_size(h: isize, after: isize, r: Rect, flags: u32) -> bool {
    let moved = set_window_pos(h, 0, r.x, r.y, 0, 0, flags | SWP_NOSIZE | SWP_NOZORDER);
    set_window_pos(h, after, r.x, r.y, r.width, r.height, flags) && moved
}

/// Windows that ended smaller than their tile (their scale change answered after the size) are given it again, a few
/// times at most. `kept_size` only sees windows that kept a bigger size.
fn grow_shrunk(windows: &[isize], tiles: &[Rect]) {
    const SLACK: i32 = 2;
    for _ in 0..3 {
        let shrunk: Vec<usize> = (0..windows.len())
            .filter(|&i| window_rect(windows[i]).is_some_and(|r| r.width < tiles[i].width - SLACK || r.height < tiles[i].height - SLACK))
            .collect();
        if shrunk.is_empty() {
            return;
        }
        for i in shrunk {
            let t = tiles[i];
            set_window_pos(windows[i], 0, t.x, t.y, t.width, t.height, SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOSENDCHANGING | SWP_ASYNCWINDOWPOS);
        }
        sleep_ms(150);
    }
}

/// A window's rectangle holds Windows' unseen resize borders (9 px left, right and below at 150 %, a real PC), which
/// showed as gaps of 18 and 9 between tiles. Each window goes over its tile by its own borders, read now that it is on
/// its screen, so what shows fills the tile, as Windows' own Snap does.
fn close_gaps(windows: &[isize], tiles: &[Rect]) {
    for (i, &h) in windows.iter().enumerate() {
        let (Some(all), Some(seen)) = (window_rect(h), seen_frame(h)) else {
            continue;
        };
        *UNSEEN_BORDERS.lock().unwrap_or_else(|e| e.into_inner()) = Some([seen.left() - all.left(), seen.top() - all.top(), all.right() - seen.right(), all.bottom() - seen.bottom()]);
        let t = tiles[i];
        place_window(
            h,
            Rect::from_ltrb(t.left() - (seen.left() - all.left()), t.top() - (seen.top() - all.top()), t.right() + (all.right() - seen.right()), t.bottom() + (all.bottom() - seen.bottom())),
        );
    }
}

/// A Roblox window's unseen borders (left, top, right, bottom) as last measured on one in its tile (`close_gaps`).
static UNSEEN_BORDERS: Mutex<Option<[i32; 4]>> = Mutex::new(None);

/// A tile grown by those borders, for a minimised window (Windows can't say its borders until it shows): opened, it
/// fills its tile as the others do. The tile itself while none was measured yet.
pub(crate) fn over_borders(t: Rect) -> Rect {
    match *UNSEEN_BORDERS.lock().unwrap_or_else(|e| e.into_inner()) {
        Some([l, top, r, b]) => Rect::from_ltrb(t.left() - l, t.top() - top, t.right() + r, t.bottom() + b),
        None => t,
    }
}

/// The window as it shows, without the unseen borders; None when Windows can't say.
pub fn seen_frame(h: isize) -> Option<Rect> {
    const DWMWA_EXTENDED_FRAME_BOUNDS: i32 = 9;
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    // SAFETY: one RECT on the stack, its size given.
    let hr = unsafe { DwmGetWindowAttribute(w(h), DWMWA_EXTENDED_FRAME_BOUNDS as _, &mut r as *mut RECT as *mut c_void, std::mem::size_of::<RECT>() as u32) };
    if hr == 0 && r.right > r.left { Some(Rect::from_win(&r)) } else { None }
}

/// Arrange, clicked or Ctrl+Alt+A: Windows lets an app stack windows under the one in front, never over it, so each is made always-on-top for
/// an instant (as anti-AFK gives your window back), which leaves it first of the usual windows; the last ends on top and gets the keyboard
/// (the hotkey lets Bloxkeep hand it). Only stacked under the app in front, the tiled windows stayed behind it: after the first press
/// (minimized windows come up as they restore) nothing showed.
pub(crate) fn lift(windows: &[isize]) {
    lift_quietly(windows);
    if let Some(&last) = windows.last() {
        set_foreground(last);
    }
}

/// Over every other app, as `lift`, but none taken to the front for the keyboard.
pub(crate) fn lift_quietly(windows: &[isize]) {
    const RESTACK: u32 = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
    for &h in windows {
        if !is_window(h) || is_hung(h) {
            continue;
        }
        set_window_pos(h, HWND_TOPMOST, 0, 0, 0, 0, RESTACK);
        set_window_pos(h, HWND_NOTOPMOST, 0, 0, 0, 0, RESTACK);
        sleep_ms(20);
    }
}

/// Game monitor's Bring all to front: minimized ones opened again, then all over every app, where they are. Blocking.
pub fn bring_all_to_front(windows: &[isize]) {
    let windows: Vec<isize> = windows.iter().copied().filter(|&h| is_window(h) && !is_hidden(h)).collect();
    if windows.is_empty() {
        return;
    }
    if windows.iter().any(|&h| is_iconic(h)) {
        for &h in windows.iter().filter(|&&h| is_iconic(h)) {
            show_async(h, SW_RESTORE);
        }
        until_restored(&windows);
    }
    lift(&windows);
}

/// Game monitor's Send all to back: behind every other app, as they are (not minimized: the games keep drawing).
pub fn send_all_to_back(windows: &[isize]) {
    for &h in windows.iter().filter(|&&h| is_window(h)) {
        set_window_pos(h, HWND_BOTTOM, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS);
    }
}

/// The whole frame; None once it is gone.
pub fn window_rect(h: isize) -> Option<Rect> {
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    // SAFETY: one RECT on the stack.
    if unsafe { GetWindowRect(w(h), &mut r) } != 0 { Some(Rect::from_win(&r)) } else { None }
}

pub fn is_player_window(h: isize) -> bool {
    process_name(owner_pid(h) as i32).is_some_and(|n| n.eq_ignore_ascii_case(PLAYER_PROCESS_NAME))
}

/// Windows' resize border, at the window's DPI.
pub fn resize_border(h: isize) -> i32 {
    const SM_CXSIZEFRAME: i32 = 32;
    const SM_CXPADDEDBORDER: i32 = 92;
    // SAFETY: plain queries.
    unsafe {
        let d = GetDpiForWindow(w(h));
        let dpi = if d > 0 { d } else { 96 };
        GetSystemMetricsForDpi(SM_CXSIZEFRAME as _, dpi) + GetSystemMetricsForDpi(SM_CXPADDEDBORDER as _, dpi) + 2
    }
}

/// Covers its whole screen, without a title bar: how Roblox goes full screen.
pub fn is_fullscreen(h: isize, r: Rect) -> bool {
    (style(h) & WS_CAPTION) != WS_CAPTION && r.contains_rect(super::win_kit::screen_from_handle(h).bounds)
}

/// Shown as an ordinary window, title bar and all.
pub fn is_plain_window(h: isize) -> bool {
    !is_hidden(h) && !is_iconic(h) && !is_zoomed(h) && (style(h) & WS_CAPTION) == WS_CAPTION
}

/// As Arrange gives its tiles (without Roblox's least size).
/// False when Windows refused the ask.
pub fn place_window(h: isize, r: Rect) -> bool {
    set_window_pos(h, 0, r.x, r.y, r.width, r.height, SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOSENDCHANGING | SWP_ASYNCWINDOWPOS)
}

/// A minimised window's place for when it is opened again: `r`, as an ordinary window (not maximised, as a window
/// that was maximised when put away would come back). It stays minimised. False when that was its place already.
pub fn restore_to(h: isize, r: Rect) -> bool {
    const WPF_RESTORETOMAXIMIZED: u32 = 2;
    const SW_SHOWMINNOACTIVE: u32 = 7;
    const WS_EX_TOOLWINDOW: u32 = 0x80;
    const GWL_EXSTYLE: i32 = -20;
    // (Not one that doesn't answer: its placement is set by a message to it.)
    if h == 0 || !is_iconic(h) || is_hung(h) {
        return false;
    }
    // rcNormalPosition is in work-area coordinates (but a tool window's): moved by the work area's offset from its screen.
    let screen = screen_from_rect(r);
    // SAFETY: plain call.
    let ex = unsafe { GetWindowLongW(hwnd(h), GWL_EXSTYLE) } as u32;
    let (dx, dy) = if ex & WS_EX_TOOLWINDOW == 0 { (screen.working_area.x - screen.bounds.x, screen.working_area.y - screen.bounds.y) } else { (0, 0) };
    let normal = RECT { left: r.x - dx, top: r.y - dy, right: r.x - dx + r.width, bottom: r.y - dy + r.height };
    // SAFETY: the structure's length is set as Windows requires.
    unsafe {
        let mut wp: WINDOWPLACEMENT = std::mem::zeroed();
        wp.length = std::mem::size_of::<WINDOWPLACEMENT>() as u32;
        if GetWindowPlacement(hwnd(h), &mut wp) == 0 {
            return false;
        }
        let n = wp.rcNormalPosition;
        if wp.flags & WPF_RESTORETOMAXIMIZED == 0 && (n.left, n.top, n.right, n.bottom) == (normal.left, normal.top, normal.right, normal.bottom) {
            return false;
        }
        wp.flags &= !WPF_RESTORETOMAXIMIZED;
        wp.showCmd = SW_SHOWMINNOACTIVE;
        wp.rcNormalPosition = normal;
        SetWindowPlacement(hwnd(h), &wp) != 0
    }
}

/// A minimised window opens maximised the next time (Settings' Maximise for a window that stays minimised). It stays
/// minimised. False when it would already.
pub fn restore_maximised(h: isize) -> bool {
    const WPF_RESTORETOMAXIMIZED: u32 = 2;
    const SW_SHOWMINNOACTIVE: u32 = 7;
    if h == 0 || !is_iconic(h) || is_hung(h) {
        return false;
    }
    // SAFETY: the structure's length is set as Windows requires.
    unsafe {
        let mut wp: WINDOWPLACEMENT = std::mem::zeroed();
        wp.length = std::mem::size_of::<WINDOWPLACEMENT>() as u32;
        if GetWindowPlacement(hwnd(h), &mut wp) == 0 || wp.flags & WPF_RESTORETOMAXIMIZED != 0 {
            return false;
        }
        wp.flags |= WPF_RESTORETOMAXIMIZED;
        wp.showCmd = SW_SHOWMINNOACTIVE;
        SetWindowPlacement(hwnd(h), &wp) != 0
    }
}

/// A minimised window opens maximised the next time it is opened (`restore_maximised` was taken, or it was maximised
/// when put away).
pub fn opens_maximised(h: isize) -> bool {
    const WPF_RESTORETOMAXIMIZED: u32 = 2;
    if h == 0 || !is_iconic(h) {
        return false;
    }
    // SAFETY: the structure's length is set as Windows requires.
    unsafe {
        let mut wp: WINDOWPLACEMENT = std::mem::zeroed();
        wp.length = std::mem::size_of::<WINDOWPLACEMENT>() as u32;
        GetWindowPlacement(hwnd(h), &mut wp) != 0 && wp.flags & WPF_RESTORETOMAXIMIZED != 0
    }
}

/// Where a minimised window opens again, as an ordinary window (`restore_to`'s other way round); None when it isn't
/// minimised, opens maximised, or has no title bar (put away from Roblox's full screen: its place is the whole
/// screen, no place for a window).
pub fn resting_place(h: isize) -> Option<Rect> {
    const WPF_RESTORETOMAXIMIZED: u32 = 2;
    const WS_EX_TOOLWINDOW: u32 = 0x80;
    const GWL_EXSTYLE: i32 = -20;
    if h == 0 || !is_iconic(h) || (style(h) & WS_CAPTION) != WS_CAPTION {
        return None;
    }
    // SAFETY: the structure's length is set as Windows requires; plain calls.
    let (wp, ex) = unsafe {
        let mut wp: WINDOWPLACEMENT = std::mem::zeroed();
        wp.length = std::mem::size_of::<WINDOWPLACEMENT>() as u32;
        if GetWindowPlacement(hwnd(h), &mut wp) == 0 {
            return None;
        }
        (wp, GetWindowLongW(hwnd(h), GWL_EXSTYLE) as u32)
    };
    let n = wp.rcNormalPosition;
    if wp.flags & WPF_RESTORETOMAXIMIZED != 0 || n.right <= n.left || n.bottom <= n.top {
        return None;
    }
    // (In work-area coordinates, but a tool window's: back by the work area's offset from its screen.)
    let r = Rect::new(n.left, n.top, n.right - n.left, n.bottom - n.top);
    let screen = screen_from_rect(r);
    let (dx, dy) = if ex & WS_EX_TOOLWINDOW == 0 { (screen.working_area.x - screen.bounds.x, screen.working_area.y - screen.bounds.y) } else { (0, 0) };
    Some(Rect::new(r.x + dx, r.y + dy, r.width, r.height))
}

/// The size a window's edge was dragged to, when it kept a bigger one (Roblox's least size); None when it took it or
/// the drag moved it. Each grabbed edge moves as far as the mouse did, no smaller than LEAST_WINDOW.
pub fn after_drag(start: Rect, end: Rect, grab: Point, release: Point, border: i32) -> Option<Rect> {
    let left = grab.x < start.left() + border;
    let right = grab.x >= start.right() - border;
    let top = grab.y < start.top() + border;
    let bottom = grab.y >= start.bottom() - border;
    if !(left || right || top || bottom) {
        return None;
    }
    let (dx, dy) = (release.x - grab.x, release.y - grab.y);
    let mut l = start.left() + if left { dx } else { 0 };
    let mut r = start.right() + if right { dx } else { 0 };
    let mut t = start.top() + if top { dy } else { 0 };
    let mut b = start.bottom() + if bottom { dy } else { 0 };
    if r - l < LEAST_WINDOW.width {
        if left {
            l = r - LEAST_WINDOW.width;
        } else {
            r = l + LEAST_WINDOW.width;
        }
    }
    if b - t < LEAST_WINDOW.height {
        if top {
            t = b - LEAST_WINDOW.height;
        } else {
            b = t + LEAST_WINDOW.height;
        }
    }
    let meant = Rect::from_ltrb(l, t, r, b);
    if meant.width < end.width || meant.height < end.height { Some(meant) } else { None }
}

/// The size kept when they refused `cell`; None when they took it. Read until it holds: the
/// smallest, as one still moving is larger, never smaller.
fn kept_size(windows: &[isize], cell: Size) -> Option<Size> {
    const SLACK: i32 = 2;
    let mut last: Option<Size> = None;
    for _ in 0..10 {
        sleep_ms(100);
        let sizes: Vec<Size> = windows.iter().filter_map(|&h| window_rect(h)).map(|r| r.size()).filter(|s| !s.is_empty()).collect();
        if sizes.is_empty() {
            return None;
        }
        let least = Size::new(sizes.iter().map(|s| s.width).min().unwrap_or(0), sizes.iter().map(|s| s.height).min().unwrap_or(0));
        if least.width <= cell.width + SLACK && least.height <= cell.height + SLACK {
            return None;
        }
        let kept = Size::new(least.width.max(cell.width), least.height.max(cell.height));
        if Some(kept) == last {
            return Some(kept);
        }
        last = Some(kept);
    }
    last
}

/// When they don't fit side by side: a near-square grid spread evenly over `area`, so every window
/// stays on the screen and overlaps the next evenly.
pub fn overlap_spots(count: i32, area: Rect, size: Size) -> Vec<Point> {
    let cols = 1.max((count as f64).sqrt().ceil() as i32);
    let rows = 1.max((count as f64 / cols as f64).ceil() as i32);
    let (w, h) = (size.width.min(area.width), size.height.min(area.height));
    let step_x = if cols > 1 { w.min((area.width - w) / (cols - 1)) } else { 0 };
    let step_y = if rows > 1 { h.min((area.height - h) / (rows - 1)) } else { 0 };
    (0..count.max(0)).map(|i| Point::new(area.x + i % cols * step_x, area.y + i / cols * step_y)).collect()
}

/// "fill": a near-square grid over the whole area. A grid ("4x4"): its cells, whatever the count. "custom":
/// `custom`, held to the screen. small / medium / large: 8 / 6 / 4 across at 8:7 from the top-left,
/// more and smaller only when the rows wouldn't fit. Returns (cols, width, height).
pub fn grid_cells(count: i32, area: Size, layout: &str, custom: Size) -> (i32, i32, i32) {
    if let Some((gc, gr)) = grid_of(layout) {
        return (gc, area.width / gc, area.height / gr);
    }
    if layout == "custom" && !custom.is_empty() {
        let w = custom.width.clamp(LEAST_WINDOW.width, LEAST_WINDOW.width.max(area.width));
        let h = custom.height.clamp(LEAST_WINDOW.height, LEAST_WINDOW.height.max(area.height));
        return (1.max(area.width / w), w, h);
    }
    let across = match layout {
        "small" => 8,
        "medium" => 6,
        "large" => 4,
        _ => 0,
    };
    if across == 0 || count <= 0 {
        let c = 1.max((count as f64).sqrt().ceil() as i32);
        let r = 1.max((count as f64 / c as f64).ceil() as i32);
        return (c, area.width / c, area.height / r);
    }
    let mut cols = across;
    loop {
        let width = area.width / cols;
        let height = width * 7 / 8;
        let rows = (count as f64 / cols as f64).ceil() as i32;
        if rows * height <= area.height || cols >= 64 {
            return (cols, width, height);
        }
        cols += 1;
    }
}

/// Low: Windows takes RAM from it first, only when it runs short (as it does for background apps).
pub fn set_memory_priority(pid: i32, background: bool) -> bool {
    const PROCESS_SET_INFORMATION: u32 = 0x0200;
    const PROCESS_MEMORY_PRIORITY: i32 = 0;
    const MEMORY_PRIORITY_LOW: u32 = 2;
    const MEMORY_PRIORITY_NORMAL: u32 = 5;
    let Some(h) = ProcHandle::open(PROCESS_SET_INFORMATION, pid) else {
        return false;
    };
    let info: u32 = if background { MEMORY_PRIORITY_LOW } else { MEMORY_PRIORITY_NORMAL }; // MEMORY_PRIORITY_INFORMATION
    // SAFETY: one u32, its size given.
    unsafe { SetProcessInformation(h.0, PROCESS_MEMORY_PRIORITY, &info as *const u32 as *const c_void, std::mem::size_of::<u32>() as u32) != 0 }
}

const PROCESS_SET_QUOTA: u32 = 0x0100;
const PROCESS_QUERY_INFORMATION: u32 = 0x0400;
const QUOTA_LIMITS_HARDWS_MIN_DISABLE: u32 = 0x2;
const QUOTA_LIMITS_HARDWS_MAX_ENABLE: u32 = 0x4;
const QUOTA_LIMITS_HARDWS_MAX_DISABLE: u32 = 0x8;

/// At most `mb` of the client in RAM, or no such limit when None: Windows keeps the rest in virtual memory (in RAM
/// still while it has room to spare), and the game keeps all of its memory, so nothing it holds is lost. Lifted,
/// the client takes back what it uses again, a little at a time. Tried on a real PC (2026-10-29, Steal An Egg, 1
/// GB): the game ran on (events, money) and, minimized, used no more CPU; one drawing in view used about twice the
/// CPU, so only windows that aren't drawing are limited (see AppController.Processes.cs).
pub fn limit_ram(pid: i32, mb: Option<i64>) -> bool {
    let Some(h) = ProcHandle::open(PROCESS_SET_QUOTA | PROCESS_QUERY_INFORMATION, pid) else {
        return false;
    };
    // SAFETY: plain calls on a handle opened with the rights they need.
    unsafe {
        if let Some(limit) = mb {
            return SetProcessWorkingSetSizeEx(h.0, 50 * 1024 * 1024, (limit * 1024 * 1024) as usize, QUOTA_LIMITS_HARDWS_MAX_ENABLE | QUOTA_LIMITS_HARDWS_MIN_DISABLE) != 0;
        }
        // A high soft maximum with the limit off: nothing is taken from it now (-1 would empty its RAM at once).
        // PORT: GC.GetGCMemoryInfo().TotalAvailableMemoryBytes is the PC's RAM (GlobalMemoryStatusEx) here.
        let all = (super::system_memory::read().total_mb * 1024 * 1024).max(4 * 1024 * 1024 * 1024);
        SetProcessWorkingSetSizeEx(h.0, 50 * 1024 * 1024, all as usize, QUOTA_LIMITS_HARDWS_MAX_DISABLE | QUOTA_LIMITS_HARDWS_MIN_DISABLE) != 0
    }
}

/// Takes the client's pages out of RAM now: what it still uses comes back as it touches it, nothing is lost.
pub fn empty_ram(pid: i32) -> bool {
    let Some(h) = ProcHandle::open(PROCESS_SET_QUOTA | PROCESS_QUERY_INFORMATION, pid) else {
        return false;
    };
    // SAFETY: plain call on a handle with the rights it needs.
    unsafe { EmptyWorkingSet(h.0) != 0 }
}

/// Whether the client has a hard RAM limit (one a Bloxkeep that closed before lifting it may have left).
pub fn has_ram_limit(pid: i32) -> bool {
    let Some(h) = ProcHandle::open(PROCESS_QUERY_INFORMATION, pid) else {
        return false;
    };
    let (mut min, mut max, mut flags) = (0usize, 0usize, 0u32);
    // SAFETY: three out values on the stack.
    unsafe { GetProcessWorkingSetSizeEx(h.0, &mut min, &mut max, &mut flags) != 0 && (flags & QUOTA_LIMITS_HARDWS_MAX_ENABLE) != 0 }
}

/// Also when it exited while a handle still holds it (Bloxkeep's close keeps one). A change that failed on an ended
/// client is no refusal: pid lists come from the last scan.
pub fn ended(pid: i32) -> bool {
    const ERROR_INVALID_PARAMETER: u32 = 87;
    let Some(h) = ProcHandle::open(SYNCHRONIZE, pid) else {
        // SAFETY: plain call, right after the failed open.
        return unsafe { GetLastError() } == ERROR_INVALID_PARAMETER;
    };
    // SAFETY: plain wait of zero ms on a live handle.
    unsafe { WaitForSingleObject(h.0, 0) == 0 } // WAIT_OBJECT_0: exited
}

const PROCESS_COMMAND_LINE_INFORMATION: i32 = 60;
/// By pid and start time, so a reused pid is asked again.
static TRAY_COPIES: LazyLock<Mutex<HashMap<(i32, i64), bool>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// The copy Roblox leaves in the notification area when its window closes (0.743, 2026-10-16, website, Bloxstrap
/// and Store): "RobloxPlayerBeta.exe --launch-to-tray". It never gets a window; a Roblox started while it runs hands
/// its launch to it and quits ("NoReload Detector: WebPlay NoReload with Tray only!"), so no window opens.
pub fn is_tray_copy(pid: i32, created: i64) -> bool {
    // No start time (the by-name fallback): a reused pid would get the old verdict, so asked afresh each time.
    if created == 0 {
        return command_line_of(pid, 0).is_some_and(|l| l.to_lowercase().contains("--launch-to-tray"));
    }
    {
        let mut known = TRAY_COPIES.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(&k) = known.get(&(pid, created)) {
            return k;
        }
        if known.len() > 200 {
            known.clear();
        }
    }
    // Kept only when the command line was read: one refused once (the process still starting) is asked again, or a
    // tray copy taken for a window would never be ended and swallow the next launch.
    let Some(line) = command_line_of(pid, 0) else { return false };
    let tray = line.to_lowercase().contains("--launch-to-tray");
    TRAY_COPIES.lock().unwrap_or_else(|e| e.into_inner()).insert((pid, created), tray);
    tray
}

const UPDATER_PROCESS_NAME: &str = "RobloxPlayerInstaller";

/// An outdated client starts the updater and quits, and the updated client comes from it ("[FLog::UpdateController]
/// WindowsUpdateController: updaterFullPath: …", 0.744). Under Bloxstrap the client finds no updater ("Updater not
/// found. Skipping update"): Bloxstrap updates before starting it.
pub fn updater_running_since(since_utc: Stamp) -> bool {
    let Some(pids) = pids_by_name(UPDATER_PROCESS_NAME) else {
        return false;
    };
    pids.into_iter().any(|pid| {
        // Gone meanwhile, or not ours to read: not counted.
        ProcHandle::open(PROCESS_QUERY_LIMITED_INFORMATION, pid).and_then(|h| creation_time(h.0)).is_some_and(|ft| filetime_to_stamp(ft) >= since_utc)
    })
}

static TRACKERS: LazyLock<Mutex<HashMap<(i32, i64), String>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
// .NET's "(?<=\s)-b" is "\s-b" here (the regex crate has no look-behind); only the groups are read, so it matches the same.
static TRACKER_TEXT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)browsertrackerid[:=](\d+)|\s-b\s+(\d+)").expect("tracker"));

/// Each launch link Bloxkeep makes has a new one ("+browsertrackerid:N"), and every launcher (Bloxstrap included)
/// passes the link to RobloxPlayerBeta.exe untouched; the old launcher passed "-b N". "" when none, None when
/// unreadable. Nothing else of the command line is kept: the sign-in ticket is in it.
pub fn launch_tracker_of(pid: i32, created: i64) -> Option<String> {
    if created != 0
        && let Some(known) = TRACKERS.lock().unwrap_or_else(|e| e.into_inner()).get(&(pid, created))
    {
        return Some(known.clone());
    }
    let line = command_line_of(pid, created)?;
    let tracker = match TRACKER_TEXT.captures(&line) {
        None => String::new(),
        Some(c) => c.get(1).or_else(|| c.get(2)).map(|m| m.as_str().to_string()).unwrap_or_default(),
    };
    // No start time (the by-name fallback): a reused pid would get the old launch's id, so not kept.
    if created == 0 {
        return Some(tracker);
    }
    let mut known = TRACKERS.lock().unwrap_or_else(|e| e.into_inner());
    if known.len() > 200 {
        known.clear();
    }
    known.insert((pid, created), tracker.clone());
    Some(tracker)
}

// As TRACKER_TEXT: "(?<=\s)-j" is "\s-j".
static LAUNCHER_URL_TEXT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)placelauncherurl:([^+\s"]+)|\s-j\s+"?([^\s"]+)"#).expect("launcher url"));

/// The Roblox app Roblox's installer opened after an update (RobloxInstalls.OwnInstaller): started with
/// "-isInstallerLaunch" and no roblox-player link (a game the installer was asked to open has one).
pub fn opened_by_installer(pid: i32, created: i64) -> bool {
    command_line_of(pid, created).is_some_and(|line| {
        let lower = line.to_lowercase();
        lower.contains("-isinstallerlaunch") && !lower.contains("roblox-player:")
    })
}

/// The PlaceLauncher link a client was started with (which game and what kind of server it asked for), unescaped; ""
/// when it had none (Roblox's home screen), None when unreadable. Only the link: the sign-in ticket elsewhere on the
/// command line is never returned.
pub fn place_launcher_url_of(pid: i32, created: i64) -> Option<String> {
    command_line_of(pid, created).map(|line| place_launcher_url(&line))
}

pub fn place_launcher_url(command_line: &str) -> String {
    let Some(found) = LAUNCHER_URL_TEXT.captures(command_line) else {
        return String::new();
    };
    let url = found.get(1).or_else(|| found.get(2)).map(|m| m.as_str()).unwrap_or("");
    // Uri.UnescapeDataString: escapes that make UTF-8 decoded, any other left as it is (not the whole link left escaped).
    crate::core::roblox_api::unescape_data_string(url)
}

/// Windows 8.1 and later; None when it can't be read (gone, not ours).
fn command_line_of(pid: i32, expected_creation: i64) -> Option<String> {
    let h = ProcHandle::open(PROCESS_QUERY_LIMITED_INFORMATION, pid)?;
    if expected_creation != 0 && creation_time(h.0) != Some(expected_creation) {
        return None;
    }
    let mut size = 0i32;
    // SAFETY: the first call only asks the size; the buffer then holds `size` bytes, aligned for the pointer in it.
    unsafe {
        NtQueryInformationProcess(h.0, PROCESS_COMMAND_LINE_INFORMATION, std::ptr::null_mut(), 0, &mut size);
        if size <= 0 || size > 1 << 16 {
            return None;
        }
        let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
        if NtQueryInformationProcess(h.0, PROCESS_COMMAND_LINE_INFORMATION, buffer.as_mut_ptr() as *mut c_void, size, &mut size) != 0 {
            return None;
        }
        // A UNICODE_STRING (length in bytes, then a pointer to the text) heads the buffer.
        let base = buffer.as_ptr() as *const u8;
        let length = std::ptr::read_unaligned(base as *const u16) as usize;
        let text = std::ptr::read_unaligned(base.add(std::mem::size_of::<usize>()) as *const *const u16);
        if length == 0 || text.is_null() {
            return Some(String::new());
        }
        Some(String::from_utf16_lossy(std::slice::from_raw_parts(text, length / 2)))
    }
}

/// Before a launch, which they would take over (`is_tray_copy`). Returns how many were ended.
pub fn close_tray_copies() -> i32 {
    let players = get_players();
    let windows = visible_windows(&players.iter().map(|p| p.pid).collect::<Vec<_>>());
    let mut ended = 0;
    for p in &players {
        if windows.get(&p.pid).is_some_and(|l| !l.is_empty()) || !is_tray_copy(p.pid, p.created) {
            continue;
        }
        kill(p.pid);
        ended += 1;
    }
    ended
}

/// Process.Kill: TerminateProcess with exit code -1; nothing when it is gone or refused.
pub fn kill(pid: i32) {
    const PROCESS_TERMINATE: u32 = 0x0001;
    if let Some(h) = ProcHandle::open(PROCESS_TERMINATE, pid) {
        // SAFETY: plain call on a handle with the right to end it.
        unsafe {
            TerminateProcess(h.0, u32::MAX);
        }
    }
}
