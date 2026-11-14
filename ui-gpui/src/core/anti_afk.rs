//! Keeps Roblox from kicking idle accounts (after 20 minutes without input). While the PC is idle, each window is made
//! all but invisible and click-through, given the keyboard and one F15 press, then your window is put back in front
//! (topmost for an instant, in case Windows won't hand back the keyboard), the Roblox window goes back to its place,
//! and only then is it visible again. The cursor doesn't move; real input stops it; impossible while locked.
use std::{
    collections::HashMap,
    ffi::c_void,
    sync::atomic::{AtomicU32, Ordering},
    time::Duration,
};

use windows_sys::Win32::{
    Foundation::HWND,
    Graphics::{
        Dwm::DwmSetWindowAttribute,
        Gdi::{RDW_ALLCHILDREN, RDW_ERASE, RDW_FRAME, RDW_INVALIDATE, RedrawWindow},
    },
    System::{
        RemoteDesktop::{WTSFreeMemory, WTSQuerySessionInformationW},
        SystemInformation::GetTickCount,
        Threading::GetCurrentThreadId,
    },
    UI::{
        Input::KeyboardAndMouse::{GetLastInputInfo, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, LASTINPUTINFO, MAPVK_VK_TO_VSC, MapVirtualKeyW, SendInput},
        Shell::SHQueryUserNotificationState,
        WindowsAndMessaging::{
            BringWindowToTop, GetClassNameW, GetForegroundWindow, GetLayeredWindowAttributes, GetShellWindow, GetWindow, GetWindowLongW, GetWindowThreadProcessId,
            IsHungAppWindow, IsIconic, IsWindow, IsWindowVisible, SetForegroundWindow, SetLayeredWindowAttributes, SetWindowLongW, SetWindowPos, ShowWindowAsync,
        },
    },
};

use super::{log, win_kit::hwnd, window_report};

/// Well inside Roblox's 20 minutes, so a person at the PC most of the time still leaves an idle minute for it.
pub const INTERVAL: Duration = Duration::from_secs(12 * 60);

/// Two minutes before the kick a window doesn't wait for the PC to be free: it is nudged in the first pause of
/// PAUSE_NEEDED (unless a full-screen app is in front).
pub const LAST_CHANCE: Duration = Duration::from_secs(18 * 60);

pub const USER_IDLE_NEEDED: Duration = Duration::from_secs(60);

pub const PAUSE_NEEDED: Duration = Duration::from_secs(2);

/// F15: on no keyboard and used by no game or common app, but input all the same.
const NUDGE_KEY: u16 = 0x7E; // VK_F15

/// 1 of 255: 0 could make Windows treat the window as not shown at all.
pub const HIDDEN_ALPHA: u8 = 1;

/// The size of Windows' INPUT (40 bytes in a 64-bit process, 28 in a 32-bit one): SendInput refuses any other.
pub fn input_size() -> i32 {
    std::mem::size_of::<INPUT>() as i32
}

const SW_RESTORE: i32 = 9;
const SW_SHOWMINNOACTIVE: i32 = 7;
const SW_SHOWNOACTIVATE: i32 = 4;
const SW_HIDE: i32 = 0;
const GWL_EXSTYLE: i32 = -20;
const WS_EX_TOPMOST: i32 = 0x8;
const WS_EX_TRANSPARENT: i32 = 0x20;
const WS_EX_LAYERED: i32 = 0x80000;
const LWA_ALPHA: u32 = 0x2;
const GW_HWNDPREV: u32 = 3;
const SWP_NOSIZE: u32 = 0x1;
const SWP_NOMOVE: u32 = 0x2;
const SWP_NOACTIVATE: u32 = 0x10;
const SWP_NOOWNERZORDER: u32 = 0x200;
// Not async: a window put back under yours while yours is still always-on-top would become always-on-top itself.
// So a window that has stopped answering isn't moved (the move waits for its answer).
const RESTACK: u32 = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
const HWND_TOPMOST: isize = -1;
const HWND_NOTOPMOST: isize = -2;
const DWMWA_TRANSITIONS_FORCEDISABLED: u32 = 3;

const WTS_CURRENT_SESSION: u32 = u32::MAX; // -1
const WTS_SESSION_INFO_EX: i32 = 25;
const WTS_SESSIONSTATE_LOCK: i32 = 0;

const QUNS_BUSY: i32 = 2;
const QUNS_RUNNING_D3D_FULL_SCREEN: i32 = 3;
const QUNS_PRESENTATION_MODE: i32 = 4;

#[link(name = "user32")]
unsafe extern "system" {
    fn AttachThreadInput(attach: u32, attach_to: u32, on: i32) -> i32;
}

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
fn is_visible(h: isize) -> bool {
    // SAFETY: plain query of a handle.
    unsafe { IsWindowVisible(w(h)) != 0 }
}
/// Windows' own "not responding" (no answer for 5 seconds).
fn is_hung(h: isize) -> bool {
    // SAFETY: plain query of a handle.
    unsafe { IsHungAppWindow(w(h)) != 0 }
}
fn foreground() -> isize {
    // SAFETY: plain call.
    unsafe { GetForegroundWindow() as isize }
}
// Async: ShowWindow waits for the window to answer, and a hung Roblox would hold Bloxkeep's timer (rejoin, saves).
fn show_async(h: isize, command: i32) -> bool {
    // SAFETY: plain call; posted to the window's thread.
    unsafe { ShowWindowAsync(w(h), command) != 0 }
}
fn get_ex_style(h: isize) -> i32 {
    // SAFETY: plain query of a handle.
    unsafe { GetWindowLongW(w(h), GWL_EXSTYLE) }
}
fn set_ex_style(h: isize, value: i32) {
    // SAFETY: plain call; only the extended style is written.
    unsafe {
        SetWindowLongW(w(h), GWL_EXSTYLE, value);
    }
}
fn set_window_pos(h: isize, after: isize, flags: u32) -> bool {
    // SAFETY: plain call; no size or place is given (RESTACK keeps them).
    unsafe { SetWindowPos(w(h), w(after), 0, 0, 0, 0, flags) != 0 }
}
fn redraw(h: isize) {
    // SAFETY: plain call over the whole window.
    unsafe {
        RedrawWindow(w(h), std::ptr::null(), std::ptr::null_mut(), RDW_INVALIDATE | RDW_ERASE | RDW_FRAME | RDW_ALLCHILDREN);
    }
}
fn layered_attributes(h: isize) -> Option<(u32, u8, u32)> {
    let (mut key, mut alpha, mut flags) = (0u32, 0u8, 0u32);
    // SAFETY: three out values on the stack.
    if unsafe { GetLayeredWindowAttributes(w(h), &mut key, &mut alpha, &mut flags) } != 0 { Some((key, alpha, flags)) } else { None }
}
fn set_layered_attributes(h: isize, key: u32, alpha: u8, flags: u32) {
    // SAFETY: plain call on a layered window.
    unsafe {
        SetLayeredWindowAttributes(w(h), key, alpha, flags);
    }
}

/// As Windows judges it for its notifications (a game, a video, a presentation).
pub fn full_screen_in_front() -> bool {
    let mut state = 0i32;
    // SAFETY: one out value on the stack.
    let hr = unsafe { SHQueryUserNotificationState(&mut state) };
    hr == 0 && matches!(state, QUNS_BUSY | QUNS_RUNNING_D3D_FULL_SCREEN | QUNS_PRESENTATION_MODE)
}

/// No window can come to the front then. Read from WTSINFOEX level 1: the level (padded to 8 bytes), the session
/// id, then its state.
pub fn locked() -> bool {
    let mut buffer: *mut u16 = std::ptr::null_mut();
    let mut bytes = 0u32;
    // SAFETY: Windows allocates the buffer, read within `bytes` and freed with WTSFreeMemory.
    unsafe {
        if WTSQuerySessionInformationW(std::ptr::null_mut(), WTS_CURRENT_SESSION, WTS_SESSION_INFO_EX, &mut buffer, &mut bytes) == 0 || buffer.is_null() {
            return false;
        }
        let p = buffer as *const u8;
        let result = bytes >= 20
            && std::ptr::read_unaligned(p as *const i32) == 1
            && std::ptr::read_unaligned(p.add(16) as *const i32) == WTS_SESSIONSTATE_LOCK;
        WTSFreeMemory(buffer as *mut c_void);
        result
    }
}

pub fn last_input_tick() -> u32 {
    let mut info = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    // SAFETY: cbSize is set as Windows requires.
    if unsafe { GetLastInputInfo(&mut info) } != 0 { info.dwTime } else { 0 }
}

/// (uint)Environment.TickCount.
fn tick_now() -> u32 {
    // SAFETY: plain call.
    unsafe { GetTickCount() }
}

/// How long ago a tick was (wrapping every 49 days). A last-input tick a few ms ahead of the clock read here is "just
/// now", not 49 days ago: taken plainly it read as the PC long idle just as someone typed.
fn since_tick(last: u32) -> Duration {
    Duration::from_millis((tick_now().wrapping_sub(last) as i32).max(0) as u64)
}

/// The PC's last-input time right after our own nudges, which must not be mistaken for a person.
static OWN_INPUT: AtomicU32 = AtomicU32::new(0);
/// The person's last input before those nudges.
static PERSON_INPUT: AtomicU32 = AtomicU32::new(0);

/// Our own nudges don't count.
pub fn idle_time() -> Duration {
    let mut last = last_input_tick();
    if last == OWN_INPUT.load(Ordering::SeqCst) {
        last = PERSON_INPUT.load(Ordering::SeqCst);
    }
    since_tick(last)
}

/// The person's last input (GetLastInputInfo's tick), our own sends left out.
pub fn person_tick() -> u32 {
    let last = last_input_tick();
    if last == OWN_INPUT.load(Ordering::SeqCst) { PERSON_INPUT.load(Ordering::SeqCst) } else { last }
}

/// Our own input went out (a macro's turn, AntiAfk's own nudges do this themselves): the PC's last input now is ours,
/// and the person's stays `person` (`person_tick` read before sending).
pub fn note_own_input(person: u32) {
    OWN_INPUT.store(last_input_tick(), Ordering::SeqCst);
    PERSON_INPUT.store(person, Ordering::SeqCst);
}

/// Input within USER_IDLE_NEEDED; our own nudges don't count.
pub fn person_active() -> bool {
    let last = last_input_tick();
    last != OWN_INPUT.load(Ordering::SeqCst) && since_tick(last) < USER_IDLE_NEEDED
}

/// A window left as it is (in view): only its place in the stack is noted, to go back to.
pub fn note(h: isize, place: (isize, i32)) -> Before {
    Before { window: h, ex_style: get_ex_style(h), had_attributes: false, key: 0, alpha: 255, flags: 0, above: place.0, depth: place.1, minimized: false }
}

/// A Roblox window as it was before a nudge; `above` is the window it goes back under.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Before {
    pub window: isize,
    pub ex_style: i32,
    pub had_attributes: bool,
    pub key: u32,
    pub alpha: u8,
    pub flags: u32,
    pub above: isize,
    pub depth: i32,
    pub minimized: bool,
}

/// Returns the windows reached; gives up on the rest as soon as someone touches the PC. `press`
/// (tests only) stands in for the key press, so a test never types on the desktop. Blocking: run off the window's thread.
pub fn nudge(windows: &[isize], keep_going: &dyn Fn() -> bool, press: Option<&dyn Fn(isize)>) -> Vec<isize> {
    let press: &dyn Fn(isize) = match press {
        Some(p) => p,
        None => &press_key,
    };
    let mut reached: Vec<isize> = Vec::new();
    let previous = foreground();
    let mut baseline = last_input_tick();
    // Still our last round's input: the person's own is older.
    let person = if baseline == OWN_INPUT.load(Ordering::SeqCst) { PERSON_INPUT.load(Ordering::SeqCst) } else { baseline };
    let mut sent = false;
    // Read before any of them moves.
    let mut places: HashMap<isize, (isize, i32)> = HashMap::new();
    for &h in windows {
        if !places.contains_key(&h) && is_window(h) {
            places.insert(h, place(h));
        }
    }
    let mut hidden: Vec<Before> = Vec::new();
    // Windows Bloxkeep hid, shown for the nudge.
    let mut unseen: Vec<isize> = Vec::new();
    // Each window's outcome for error.log, by process id.
    let mut outcomes: Vec<String> = Vec::new();
    for &h in windows {
        if !keep_going() || last_input_tick() != baseline {
            outcomes.push(if keep_going() { "stopped: a person is back".into() } else { "stopped: turned off".into() });
            break; // turned off, or a person is back: hand the PC over right away
        }
        // Touching a hung window could only make Bloxkeep wait.
        if !is_window(h) || is_hung(h) {
            outcomes.push(format!("{} skipped ({})", pid_of(h), if is_window(h) { "not responding" } else { "gone" }));
            continue;
        }
        // Unseen and click-through first, so neither its coming to the front nor a click at that moment shows.
        // A minimized window still gets kicked: restored for the nudge, then minimized again.
        let minimized = is_iconic(h);
        // A window Bloxkeep hid (roblox_processes::hide_windows) is shown for the nudge, still unseen, then hidden.
        let unshown = !is_visible(h);
        hidden.push(hide(h, places.get(&h).copied().unwrap_or((0, 0)), minimized));
        if minimized {
            show_async(h, SW_RESTORE);
        } else if unshown {
            show_async(h, SW_SHOWNOACTIVATE);
        }
        // try … finally: the window goes back to minimized or hidden whatever happened.
        (|| {
            if !focus(h) {
                outcomes.push(format!("{} skipped (Windows refused to bring it to the front)", pid_of(h)));
                return;
            }
            std::thread::sleep(Duration::from_millis(if minimized { 400 } else { 150 })); // a restored Roblox window needs a moment to draw again
            // The key goes to whichever window has the keyboard, and a person's keys would mix with it.
            if foreground() != h || last_input_tick() != baseline || !keep_going() {
                let why = if foreground() != h {
                    "lost the front before the key"
                } else if last_input_tick() != baseline {
                    "a person is back"
                } else {
                    "turned off"
                };
                outcomes.push(format!("{} skipped ({why})", pid_of(h)));
                return;
            }
            press(h);
            sent = true;
            std::thread::sleep(Duration::from_millis(60));
            // Our own key counts as input; anything newer than this is a person.
            baseline = last_input_tick();
            reached.push(h);
            outcomes.push(format!("{} done{}", pid_of(h), if minimized { " (minimized)" } else if unshown { " (hidden)" } else { "" }));
        })();
        if minimized {
            show_async(h, SW_SHOWMINNOACTIVE);
        }
        if unshown {
            show_async(h, SW_HIDE);
            unseen.push(h);
        }
    }
    // finally:
    // Else the last input is the person's, and must stay theirs.
    if sent {
        OWN_INPUT.store(baseline, Ordering::SeqCst);
        PERSON_INPUT.store(person, Ordering::SeqCst);
    }
    // Nothing touched (turned off, a person back, no window answering): the window you're in is left alone.
    if !hidden.is_empty() {
        give_back(previous);
    }
    // Highest in the stack first, so one that goes under another finds it there; seen again only once in place
    // (a minimized one once minimized again).
    let mut by_depth: Vec<&Before> = hidden.iter().collect();
    by_depth.sort_by_key(|b| b.depth); // OrderBy: stable
    for before in by_depth {
        put_back(before);
    }
    let mut i = 0;
    while i < 10
        && (hidden.iter().any(|b| b.minimized && is_window(b.window) && !is_iconic(b.window)) || unseen.iter().any(|&w| is_window(w) && is_visible(w)))
    {
        std::thread::sleep(Duration::from_millis(50));
        i += 1;
    }
    for before in &hidden {
        show(before);
    }
    if !outcomes.is_empty() {
        log::note(&format!("Anti-AFK: nudged {} of {} Roblox window(s): {}.", reached.len(), windows.len(), outcomes.join("; ")));
    }
    reached
}

/// The nearest shown, not-topmost window above, and the depth. Hidden ones don't count: a program's hidden
/// input-method window rides along with its front window, so going back under one would leave the window in front.
pub fn place(h: isize) -> (isize, i32) {
    let mut above: isize = 0;
    let mut depth = 0;
    // SAFETY: plain queries of handles.
    let prev = |x: isize| unsafe { GetWindow(w(x), GW_HWNDPREV) as isize };
    let mut win = prev(h);
    while win != 0 && depth < 5000 {
        if above == 0 && is_visible(win) && (get_ex_style(win) & WS_EX_TOPMOST) == 0 {
            above = win;
        }
        win = prev(win);
        depth += 1;
    }
    (above, depth)
}

/// A minimized one also loses Windows' restore animation, which the taskbar would show.
pub fn hide(h: isize, place: (isize, i32), minimized: bool) -> Before {
    hidden(h, place, minimized, true)
}

/// `hide` for a macro's turn: its clicks must land on it (alpha 1 still takes them), so it isn't made click-through.
pub fn hide_for_input(h: isize, place: (isize, i32), minimized: bool) -> Before {
    hidden(h, place, minimized, false)
}

fn hidden(h: isize, place: (isize, i32), minimized: bool, click_through: bool) -> Before {
    let ex = get_ex_style(h);
    let (mut key, mut alpha, mut flags) = (0u32, 255u8, 0u32);
    let mut had = false;
    if (ex & WS_EX_LAYERED) != 0 {
        (key, alpha, flags, had) = match layered_attributes(h) {
            Some((k, a, f)) => (k, a, f, true),
            // Layered another way (UpdateLayeredWindow): nothing to read, and `hide`'s alpha would stay. Shown fully again.
            None => (0, 255, LWA_ALPHA, true),
        };
    }
    let before = Before { window: h, ex_style: ex, had_attributes: had, key, alpha, flags, above: place.0, depth: place.1, minimized };
    set_ex_style(h, ex | WS_EX_LAYERED | if click_through { WS_EX_TRANSPARENT } else { 0 });
    set_layered_attributes(h, 0, HIDDEN_ALPHA, LWA_ALPHA);
    if minimized {
        animations(h, false);
    }
    before
}

/// Only the bits `hide` set are touched. A hung window is left to `recover`: a change of
/// style waits for the window's answer.
pub fn show(b: &Before) {
    if !is_window(b.window) || is_hung(b.window) {
        if is_window(b.window) {
            log::note(&format!(
                "Unseen window: Roblox window {} stopped responding while shown unseen (nudge, picture or macro turn); left unseen until it answers.",
                pid_of(b.window)
            ));
        }
        return;
    }
    const OURS: i32 = WS_EX_LAYERED | WS_EX_TRANSPARENT;
    set_ex_style(b.window, (get_ex_style(b.window) & !OURS) | (b.ex_style & OURS));
    if b.had_attributes {
        set_layered_attributes(b.window, b.key, b.alpha, b.flags);
    }
    if b.minimized {
        animations(b.window, true);
    }
    redraw(b.window);
}

fn animations(h: isize, on: bool) {
    let disabled: i32 = if on { 0 } else { 1 };
    // SAFETY: one int, its size given.
    unsafe {
        DwmSetWindowAttribute(w(h), DWMWA_TRANSITIONS_FORCEDISABLED as _, &disabled as *const i32 as *const c_void, std::mem::size_of::<i32>() as u32);
    }
}

pub fn put_back(b: &Before) {
    if b.minimized || b.above == 0 || !is_window(b.window) || !is_window(b.above) || is_hung(b.window) {
        return;
    }
    set_window_pos(b.window, b.above, RESTACK | SWP_NOOWNERZORDER);
}

/// Windows may refuse to hand the keyboard back (a click landed meanwhile): made topmost for an instant, your
/// window comes back on top all the same.
pub fn give_back(previous: isize) {
    if previous == 0 || !is_window(previous) {
        return;
    }
    let lift = !is_desktop(previous) && is_visible(previous) && !is_iconic(previous) && (get_ex_style(previous) & WS_EX_TOPMOST) == 0 && !is_hung(previous);
    if lift {
        set_window_pos(previous, HWND_TOPMOST, RESTACK);
    }
    if !focus(previous) {
        log::note(&format!(
            "Keyboard: couldn't give the keyboard back to the window that had it (pid {}, {}); Windows refused.",
            pid_of(previous),
            window_report::describe(previous)
        ));
    }
    if lift {
        set_window_pos(previous, HWND_NOTOPMOST, RESTACK); // first of the usual windows
    }
}

/// Its icons or the wallpaper behind them: never lifted over the windows.
fn is_desktop(h: isize) -> bool {
    // SAFETY: plain calls; the class name buffer holds 32 chars.
    unsafe {
        if w(h) == GetShellWindow() {
            return true;
        }
        let mut name = [0u16; 32];
        let n = GetClassNameW(w(h), name.as_mut_ptr(), name.len() as i32);
        if n <= 0 {
            return false;
        }
        let name = String::from_utf16_lossy(&name[..n as usize]);
        name == "Progman" || name == "WorkerW"
    }
}

fn key_input(scan: u16, flags: u32) -> INPUT {
    INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: NUDGE_KEY, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: 0 } } }
}

fn press_key(_: isize) {
    // SAFETY: SendInput gets one INPUT of Windows' own layout and its size.
    unsafe {
        let scan = MapVirtualKeyW(NUDGE_KEY as u32, MAPVK_VK_TO_VSC) as u16;
        let down = [key_input(scan, 0)];
        SendInput(1, down.as_ptr(), input_size());
        std::thread::sleep(Duration::from_millis(30)); // held for a moment, like a real key
        let up = [key_input(scan, KEYEVENTF_KEYUP)];
        SendInput(1, up.as_ptr(), input_size());
    }
}

/// Undoes a nudge cut short by Bloxkeep closing; told apart by HIDDEN_ALPHA, which nothing else uses.
/// A macro's turn leaves its unseen window layered but not click-through: undone the same way.
pub fn recover(windows: &[isize]) {
    const OURS: i32 = WS_EX_LAYERED | WS_EX_TRANSPARENT;
    let mut recovered: Vec<u32> = Vec::new();
    for &win in windows {
        let ex = get_ex_style(win);
        if (ex & WS_EX_LAYERED) == 0 {
            continue;
        }
        let Some((_, alpha, flags)) = layered_attributes(win) else {
            continue;
        };
        if (flags & LWA_ALPHA) == 0 || alpha != HIDDEN_ALPHA || is_hung(win) {
            continue;
        }
        set_ex_style(win, ex & !OURS);
        animations(win, true);
        redraw(win);
        recovered.push(pid_of(win));
    }
    if !recovered.is_empty() {
        log::note(&format!(
            "Unseen window: made {} unseen Roblox window(s) visible again after a cut-short nudge or turn (pids {}).",
            recovered.len(),
            recovered.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", ")
        ));
    }
}

/// The window's process id, for error.log (0 when gone).
fn pid_of(h: isize) -> u32 {
    let mut pid = 0u32;
    // SAFETY: one out value on the stack.
    unsafe {
        GetWindowThreadProcessId(w(h), &mut pid);
    }
    pid
}

/// Windows only lets the app that last received input do it, so the foreground thread's input is shared meanwhile.
pub fn focus(h: isize) -> bool {
    let fg = foreground();
    if fg == h {
        return true;
    }
    // SAFETY: plain calls; the input sharing is undone before returning.
    unsafe {
        let me = GetCurrentThreadId();
        let other = if fg == 0 { 0 } else { GetWindowThreadProcessId(w(fg), std::ptr::null_mut()) };
        // Not to a program that has stopped answering: sharing its input could leave Bloxkeep waiting on it too.
        let attached = other != 0 && other != me && !is_hung(fg) && AttachThreadInput(me, other, 1) != 0;
        BringWindowToTop(w(h));
        let ok = SetForegroundWindow(w(h)) != 0;
        if attached {
            AttachThreadInput(me, other, 0);
        }
        ok
    }
}
