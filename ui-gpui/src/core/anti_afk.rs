//! Keeps Roblox from kicking idle accounts (after 20 minutes without input). While the PC is idle, each window is made
//! all but invisible and click-through, given the keyboard and one F15 press, then your window is put back in front
//! (topmost for an instant, in case Windows won't hand back the keyboard), the Roblox window goes back to its place,
//! and only then is it visible again. The cursor doesn't move; real input stops it; impossible while locked.
use std::{
    ffi::c_void,
    sync::atomic::{AtomicU32, Ordering},
    time::Duration,
};

use windows_sys::Win32::{
    Foundation::HWND,
    System::{
        RemoteDesktop::{WTSFreeMemory, WTSQuerySessionInformationW},
        SystemInformation::GetTickCount,
        Threading::GetCurrentThreadId,
    },
    UI::{
        Input::KeyboardAndMouse::{GetLastInputInfo, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, LASTINPUTINFO, MAPVK_VK_TO_VSC, MapVirtualKeyW, SendInput},
        Shell::SHQueryUserNotificationState,
        WindowsAndMessaging::{
            BringWindowToTop, GetClassNameW, GetForegroundWindow, GetShellWindow, GetWindowLongW, GetWindowThreadProcessId, IsHungAppWindow, IsIconic, IsWindow,
            IsWindowVisible, SetForegroundWindow, SetWindowPos,
        },
    },
};

use super::{
    log,
    win_kit::hwnd,
    window_loan::{self, How},
    window_report,
};

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

const GWL_EXSTYLE: i32 = -20;
const WS_EX_TOPMOST: i32 = 0x8;
const SWP_NOSIZE: u32 = 0x1;
const SWP_NOMOVE: u32 = 0x2;
const SWP_NOACTIVATE: u32 = 0x10;
const RESTACK: u32 = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
const HWND_TOPMOST: isize = -1;
const HWND_NOTOPMOST: isize = -2;

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
fn get_ex_style(h: isize) -> i32 {
    // SAFETY: plain query of a handle.
    unsafe { GetWindowLongW(w(h), GWL_EXSTYLE) }
}
fn set_window_pos(h: isize, after: isize, flags: u32) -> bool {
    // SAFETY: plain call; no size or place is given (RESTACK keeps them).
    unsafe { SetWindowPos(w(h), w(after), 0, 0, 0, 0, flags) != 0 }
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

/// Returns the windows reached; gives up on the rest as soon as someone touches the PC. `press`
/// (tests only) stands in for the key press, so a test never types on the desktop. Blocking: run off the window's thread.
pub fn nudge(windows: &[isize], keep_going: &dyn Fn() -> bool, press: Option<&dyn Fn(isize)>) -> Vec<isize> {
    nudge_held(windows, &[], keep_going, press)
}

/// `nudge`, the windows in `longer` held in front longer before and after their key: ones whose key Roblox didn't take
/// last time (a window kept to little memory can take a while to come back).
pub fn nudge_held(windows: &[isize], longer: &[isize], keep_going: &dyn Fn() -> bool, press: Option<&dyn Fn(isize)>) -> Vec<isize> {
    let press: &dyn Fn(isize) = match press {
        Some(p) => p,
        None => &press_key,
    };
    let mut reached: Vec<isize> = Vec::new();
    let mut baseline = last_input_tick();
    // Still our last round's input: the person's own is older.
    let person = if baseline == OWN_INPUT.load(Ordering::SeqCst) { PERSON_INPUT.load(Ordering::SeqCst) } else { baseline };
    let mut sent = false;
    // Every window's place in the stack read before any of them moves; each lent (left alone by the app's other
    // window rules) until the round is given back.
    let mut round = window_loan::borrow_round(windows, How::NUDGE);
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
        // A minimized window still gets kicked: restored for the nudge, then minimized again; a window Bloxkeep hid
        // (roblox_processes::hide_windows) is shown for the nudge, still unseen, then hidden.
        let Some(lent) = round.lend(h) else {
            outcomes.push(format!("{} skipped (gone, or already in this round)", pid_of(h)));
            continue;
        };
        let (minimized, unshown) = (lent.minimised, lent.unshown);
        // try … finally: the window goes back to minimized or hidden whatever happened.
        (|| {
            if !focus(h) {
                outcomes.push(format!("{} skipped (Windows refused to bring it to the front)", pid_of(h)));
                return;
            }
            let long = longer.contains(&h);
            // A restored Roblox window needs a moment to draw again.
            std::thread::sleep(Duration::from_millis(match (long, minimized || unshown) {
                (true, true) => 1500,
                (true, false) => 500,
                (false, true) => 400,
                (false, false) => 150,
            }));
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
            // Roblox reads the key a little after it is sent (about 100 ms after, in its logs): a window minimized or
            // hidden again before then drops it.
            std::thread::sleep(Duration::from_millis(match (long, minimized || unshown) {
                (true, _) => 1000,
                (false, true) => 300,
                (false, false) => 60,
            }));
            // Our own key counts as input; anything newer than this is a person.
            baseline = last_input_tick();
            reached.push(h);
            outcomes.push(format!("{} done{}", pid_of(h), if minimized { " (minimized)" } else if unshown { " (hidden)" } else { "" }));
        })();
        // Asked down again at once; the give-back below waits for it and sees it unseen until then.
        round.put_away(h);
    }
    // finally:
    // Else the last input is the person's, and must stay theirs.
    if sent {
        OWN_INPUT.store(baseline, Ordering::SeqCst);
        PERSON_INPUT.store(person, Ordering::SeqCst);
    }
    // The keyboard back to the window that had it (none when no window was taken: the window you're in is left
    // alone), then each window back in its place, the highest first, and state, and seen again only once there
    // (window_loan.rs). Our own key is noted above first, so it isn't taken for a person's.
    round.give_back();
    if !outcomes.is_empty() {
        log::note(&format!("Anti-AFK: nudged {} of {} Roblox window(s): {}.", reached.len(), windows.len(), outcomes.join("; ")));
    }
    reached
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
pub(crate) fn is_desktop(h: isize) -> bool {
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

fn key_input(vk: u16, scan: u16, flags: u32) -> INPUT {
    INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: 0 } } }
}

fn press_key(_: isize) {
    // SAFETY: plain Windows call.
    let scan = unsafe { MapVirtualKeyW(NUDGE_KEY as u32, MAPVK_VK_TO_VSC) } as u16;
    press(NUDGE_KEY, scan, false);
}

fn press(vk: u16, scan: u16, extended: bool) {
    let ext = if extended { KEYEVENTF_EXTENDEDKEY } else { 0 };
    // SAFETY: SendInput gets one INPUT of Windows' own layout and its size.
    unsafe {
        let down = [key_input(vk, scan, ext)];
        SendInput(1, down.as_ptr(), input_size());
        std::thread::sleep(Duration::from_millis(30)); // held for a moment, like a real key
        let up = [key_input(vk, scan, ext | KEYEVENTF_KEYUP)];
        SendInput(1, up.as_ptr(), input_size());
    }
}

/// A key Settings may give anti-AFK: F15, or a keyboard key macros press (not a mouse button).
pub fn key_ok(name: &str) -> bool {
    name == "F15" || super::macro_model::macro_keys::contains(name)
}

/// Presses Settings' key (`key_ok`; anything else is F15) in the window given the keyboard: Space makes the character
/// jump, for a game that wants it to move (a Pet Simulator 99 player, 2026-11-20).
pub fn press_named(name: &str) {
    match super::macro_model::macro_keys::get(name).filter(|_| name != "F15") {
        Some(k) => press(k.vk, k.scan, k.extended),
        None => press_key(0),
    }
}

/// Undoes a nudge, picture or macro turn cut short by Bloxkeep closing (window_loan::recover).
pub fn recover(windows: &[isize]) {
    window_loan::recover(windows);
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
