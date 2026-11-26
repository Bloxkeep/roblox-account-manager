// MacroDesk.cs.
//! The real desktop for macros: Roblox windows found through Bloxkeep's own bookkeeping, a window's turn the way
//! anti-AFK's nudge takes one (a loan, window_loan.rs: shown unseen when minimised or hidden, brought forward with the
//! foreground thread's input shared, your window and the Roblox window's place and state given back after), and input sent with SendInput as scan codes (games read
//! those). Nothing here reads or writes Roblox's memory, hooks it, or posts messages to it.
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, ClientToScreen, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, ReleaseDC, SRCCOPY,
    SelectObject,
};
use windows_sys::Win32::Security::{GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, TOKEN_MANDATORY_LABEL, TOKEN_QUERY, TokenIntegrityLevel};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, KEYEVENTF_UNICODE, MOUSEEVENTF_ABSOLUTE,
    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP,
    MOUSEEVENTF_VIRTUALDESK, MOUSEEVENTF_WHEEL, MOUSEINPUT, SendInput,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GA_ROOT, GWL_EXSTYLE, GetAncestor, GetClientRect, GetCursorPos, GetForegroundWindow, GetSystemMetrics, GetWindowLongW, GetWindowThreadProcessId, IsHungAppWindow,
    IsWindow, LWA_ALPHA, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SetCursorPos, SetLayeredWindowAttributes, SetWindowLongW,
    WS_EX_LAYERED, WS_EX_TRANSPARENT, WindowFromPoint,
};

use crate::core::anti_afk;
use crate::core::base::{Cancel, Fail, Guid, Res, Stamp};
use crate::core::log;
use crate::core::macro_model::MacroKey;
use crate::core::macro_ocr;
use crate::core::macro_player::{IMacroDesk, IMacroTurn, MacroWindow, macro_geometry};
use crate::core::roblox_processes;
use crate::core::win_kit::{self, Picture, Point, Rect, hwnd};
use crate::core::window_loan::{self, How, Loan};
use crate::core::window_shot;

/// The account's Roblox as Bloxkeep knows it: its process, the frame rate limit it was given, whether its log says it is in a game.
pub type ClientFn = Arc<dyn Fn(Guid) -> Option<(i32, i32, bool)> + Send + Sync>;
/// When raw input last saw a person's device.
pub type PersonAtFn = Arc<dyn Fn() -> Option<Stamp> + Send + Sync>;
/// Raw input works (registered, a device seen).
pub type RawWorksFn = Arc<dyn Fn() -> bool + Send + Sync>;

/// A person's input this recent holds every run back.
pub const PERSON_QUIET: Duration = Duration::from_secs(5);
/// Windows takes a moment to make the window the one in front.
const FRONT_TIME: Duration = Duration::from_millis(250);
/// Bloxkeep closing: how long a window put back down is waited for (each of the two asks).
const ABORT_WAIT: Duration = Duration::from_millis(150);

/// error.log: SendInput is being refused (UIPI, a secure desktop); written when that starts and ends.
static SEND_FAILING: AtomicBool = AtomicBool::new(false);

/// The turns going on now, by their address, with whether their window was minimised or hidden (AreaAsync's
/// `turn is Turn { Minimized: false, Unshown: false }`: a turn is known here without a cast).
type Turns = Arc<Mutex<HashMap<usize, (bool, bool)>>>;

fn turn_key(turn: &dyn IMacroTurn) -> usize {
    (turn as *const dyn IMacroTurn).cast::<()>() as usize
}

pub struct MacroDesk {
    client: ClientFn,
    person_at: Option<PersonAtFn>,
    raw_works: Option<RawWorksFn>,
    elevated: Mutex<HashMap<i32, bool>>,
    /// When UniKey/EVKey were last looked for (None: never), and whether one ran.
    ime: Mutex<(Option<Instant>, bool)>,
    /// error.log: the last screen copy failure, and the last window a turn was refused for (and why).
    area_failed: Mutex<Option<String>>,
    refused: Mutex<Option<String>>,
    turns: Turns,
}

impl MacroDesk {
    pub fn new(client: ClientFn, person_at: Option<PersonAtFn>, raw_works: Option<RawWorksFn>) -> MacroDesk {
        MacroDesk {
            client,
            person_at,
            raw_works,
            elevated: Mutex::new(HashMap::new()),
            ime: Mutex::new((None, false)),
            area_failed: Mutex::new(None),
            refused: Mutex::new(None),
            turns: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn is_roblox(h: isize) -> bool {
        if h == 0 {
            return false;
        }
        let pid = pid_of(h);
        roblox_processes::get_player_pids().contains(&(pid as i32))
    }

    /// The window's client area on the screen.
    pub fn client_on_screen(h: isize) -> Rect {
        let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        // SAFETY: out struct on the stack.
        if unsafe { GetClientRect(hwnd(h), &mut r) } == 0 {
            return Rect::default();
        }
        let mut origin = POINT { x: 0, y: 0 };
        // SAFETY: in/out struct on the stack.
        unsafe { ClientToScreen(hwnd(h), &mut origin) };
        Rect::new(origin.x, origin.y, r.right - r.left, r.bottom - r.top)
    }

    /// The process runs at a higher integrity level than Bloxkeep (as administrator): Windows drops our input to it.
    fn elevated_pid(pid: i32) -> bool {
        // PORT: C# caught InvalidOperationException/Win32Exception from Process.GetCurrentProcess and logged "couldn't
        // tell"; the Win32 calls here fail by returning null, which counts as not elevated as there.
        let theirs = integrity(pid as u32);
        let ours = integrity(std::process::id());
        matches!((theirs, ours), (Some(t), Some(o)) if t > o)
    }

    fn note_refused(&self, pid: i32, why: &str) {
        let key = format!("{pid}:{why}");
        let mut refused = self.refused.lock().unwrap_or_else(|e| e.into_inner());
        if refused.as_deref() == Some(key.as_str()) {
            return;
        }
        *refused = Some(key);
        drop(refused);
        log::note(&format!("Macro: no turn for window {pid}: {why}."));
    }

    /// A person's input came since `started`. Raw input knows a device's input from a program's; without it, input
    /// Windows processed long after our last send.
    pub fn person_since(started: Stamp, person_at: Option<&PersonAtFn>, raw_works: Option<&RawWorksFn>, last_input_tick: u32, last_sent: u32) -> bool {
        // Raw input not working (registration refused, no device seen) never names the person: Windows' last input does.
        if let Some(person_at) = person_at
            && raw_works.is_none_or(|w| w())
        {
            return person_at().is_some_and(|at| at > started);
        }
        (last_input_tick.wrapping_sub(last_sent) as i32) > 150
    }
}

fn pid_of(h: isize) -> u32 {
    let mut pid = 0u32;
    // SAFETY: out value on the stack.
    unsafe { GetWindowThreadProcessId(hwnd(h), &mut pid) };
    pid
}

fn foreground() -> isize {
    // SAFETY: plain query.
    unsafe { GetForegroundWindow() as isize }
}

fn is_window(h: isize) -> bool {
    // SAFETY: plain query of a handle.
    unsafe { IsWindow(hwnd(h)) != 0 }
}

fn cursor_pos() -> Option<Point> {
    let mut p = POINT { x: 0, y: 0 };
    // SAFETY: out struct on the stack.
    if unsafe { GetCursorPos(&mut p) } != 0 { Some(Point::new(p.x, p.y)) } else { None }
}

fn tick_count() -> u32 {
    // SAFETY: plain call.
    unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount() }
}

/// The process's integrity level (the last sub-authority of its token's mandatory label); None when it can't be read.
fn integrity(pid: u32) -> Option<i32> {
    // SAFETY: every handle opened here is closed here; the buffer is sized by Windows' first answer and read within it.
    unsafe {
        let process: HANDLE = if pid == std::process::id() { GetCurrentProcess() } else { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if process.is_null() {
            return None;
        }
        let own = pid == std::process::id();
        let mut token: HANDLE = std::ptr::null_mut();
        let result = if OpenProcessToken(process, TOKEN_QUERY, &mut token) == 0 {
            None
        } else {
            let mut size = 0u32;
            GetTokenInformation(token, TokenIntegrityLevel, std::ptr::null_mut(), 0, &mut size);
            let level = if size == 0 {
                None
            } else {
                let mut buffer = vec![0u8; size as usize];
                if GetTokenInformation(token, TokenIntegrityLevel, buffer.as_mut_ptr() as *mut c_void, size, &mut size) == 0 {
                    None
                } else {
                    // TOKEN_MANDATORY_LABEL: its SID's last sub-authority is the level.
                    let label = std::ptr::read_unaligned(buffer.as_ptr() as *const TOKEN_MANDATORY_LABEL);
                    let sid = label.Label.Sid;
                    let count = *GetSidSubAuthorityCount(sid);
                    Some(*GetSidSubAuthority(sid, (count as u32).wrapping_sub(1)) as i32)
                }
            };
            CloseHandle(token);
            level
        };
        if !own {
            CloseHandle(process);
        }
        result
    }
}

/// The screen's pixels in `r` (Graphics.CopyFromScreen); Err with the failure's kind (C#'s exception type name).
fn copy_from_screen(r: Rect) -> Result<Picture, String> {
    // SAFETY: GDI objects made here are deleted here; the DIB's bits are read within its size before it goes.
    unsafe {
        let screen = GetDC(std::ptr::null_mut());
        if screen.is_null() {
            return Err("Win32Exception".into());
        }
        let dc = CreateCompatibleDC(screen);
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: r.width,
            biHeight: -r.height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..std::mem::zeroed()
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let dib = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
        if dib.is_null() || bits.is_null() {
            DeleteDC(dc);
            ReleaseDC(std::ptr::null_mut(), screen);
            return Err("Win32Exception".into());
        }
        let old = SelectObject(dc, dib);
        let ok = BitBlt(dc, 0, 0, r.width, r.height, screen, r.x, r.y, SRCCOPY) != 0;
        let mut picture = Picture::new(r.width, r.height);
        if ok {
            let src = std::slice::from_raw_parts(bits as *const u32, (r.width * r.height) as usize);
            for (i, p) in src.iter().enumerate() {
                picture.pixels[i] = p | 0xFF00_0000;
            }
        }
        SelectObject(dc, old);
        DeleteObject(dib);
        DeleteDC(dc);
        ReleaseDC(std::ptr::null_mut(), screen);
        if ok { Ok(picture) } else { Err("Win32Exception".into()) }
    }
}

/// `whole.Clone(r, …)`: the part of a picture in `r`.
fn crop(whole: &Picture, r: Rect) -> Picture {
    let mut out = Picture::new(r.width, r.height);
    for y in 0..r.height {
        for x in 0..r.width {
            out.set(x, y, whole.get(r.x + x, r.y + y));
        }
    }
    out
}

impl IMacroDesk for MacroDesk {
    fn now(&self) -> Stamp {
        Stamp::utc_now()
    }

    fn delay(&self, ms: i32, ct: &Cancel) -> Res<()> {
        if ms <= 0 {
            return Ok(());
        }
        if ct.sleep(Duration::from_millis(ms as u64)) { Ok(()) } else { Err(Fail::other("cancelled")) }
    }

    fn hold(&self) -> Option<String> {
        if anti_afk::locked() {
            return Some("locked".into());
        }
        // Raw input names the device (a program's input has none); GetLastInputInfo with our own sends left out.
        if anti_afk::idle_time() < PERSON_QUIET || self.person_at.as_ref().and_then(|f| f()).is_some_and(|at| at.age_secs() < PERSON_QUIET.as_secs_f64()) {
            return Some("personActive".into());
        }
        // A Roblox window in full screen in front is no reason to wait.
        if anti_afk::full_screen_in_front() && !MacroDesk::is_roblox(foreground()) {
            return Some("fullscreen".into());
        }
        None
    }

    fn window(&self, account_id: Guid) -> Option<MacroWindow> {
        let (pid, fps, in_game) = (self.client)(account_id)?;
        let h = roblox_processes::main_window_of(&roblox_processes::visible_windows(&[pid]).remove(&pid).unwrap_or_default());
        if h == 0 || !is_window(h) {
            return None;
        }
        let elevated = {
            let mut cache = self.elevated.lock().unwrap_or_else(|e| e.into_inner());
            match cache.get(&pid) {
                Some(&e) => e,
                None => {
                    if cache.len() > 64 {
                        cache.clear();
                    }
                    let e = MacroDesk::elevated_pid(pid);
                    cache.insert(pid, e);
                    if e {
                        log::note(&format!("Macro: window {pid} runs as administrator; Windows drops Bloxkeep's input to it."));
                    }
                    e
                }
            }
        };
        Some(MacroWindow { account_id, pid, handle: h, client: MacroDesk::client_on_screen(h), fps, in_game, elevated })
    }

    /// UniKey or EVKey running (looked for every 30 s): with Telex on they rewrite W, A, O, E, D, S, F, R, X, J, Z.
    fn ime_on(&self) -> bool {
        let mut ime = self.ime.lock().unwrap_or_else(|e| e.into_inner());
        if ime.0.is_some_and(|at| at.elapsed() < Duration::from_secs(30)) {
            return ime.1;
        }
        ime.0 = Some(Instant::now());
        ime.1 = false;
        for name in ["UniKeyNT", "UniKey", "EVKey64", "EVKey32", "EVKey"] {
            if win_kit::pids_by_name(name).is_some_and(|found| !found.is_empty()) {
                ime.1 = true;
            }
        }
        ime.1
    }

    fn picture(&self, window: &MacroWindow, turn: Option<&dyn IMacroTurn>) -> Option<Picture> {
        window_shot::client(window.handle, turn.is_some())
    }

    /// A window in view: copied from the screen (a fraction of PrintWindow's time, for many looks a second;
    /// Bloxkeep's own see-through windows aren't in the copy). A window shown unseen isn't on the screen: pictured whole.
    fn area(&self, window: &MacroWindow, turn: &dyn IMacroTurn, area: Rect) -> Option<Picture> {
        if area.width <= 0 || area.height <= 0 {
            return None;
        }
        let ours = self.turns.lock().unwrap_or_else(|e| e.into_inner()).get(&turn_key(turn)).copied();
        if ours == Some((false, false)) {
            let client = turn.client();
            match copy_from_screen(Rect::new(client.x + area.x, client.y + area.y, area.width, area.height)) {
                Ok(shot) => return Some(shot),
                // the screen locked, a secure desktop
                Err(kind) => {
                    // Looked at many times a second: written when it starts failing, not each time.
                    let mut failed = self.area_failed.lock().unwrap_or_else(|e| e.into_inner());
                    if failed.as_deref() != Some(kind.as_str()) {
                        *failed = Some(kind.clone());
                        drop(failed);
                        log::note(&format!("Macro: copying window {}'s area from the screen failed ({kind}).", window.pid));
                    }
                    return None;
                }
            }
        }
        let whole = window_shot::client(window.handle, true)?;
        let r = Rect::intersect(area, Rect::new(0, 0, whole.width, whole.height));
        if r.width > 0 && r.height > 0 { Some(crop(&whole, r)) } else { None }
    }

    fn read(&self, picture: &Picture, area: Rect) -> Res<Option<Vec<Vec<macro_ocr::Line>>>> {
        // The picture is the caller's: read from a copy (C# read it on a worker; this is already off the app's thread).
        let copy = picture.clone();
        macro_ocr::read(&copy, area)
    }

    fn begin_turn(&self, window: &MacroWindow, ct: &Cancel) -> Res<Option<Arc<dyn IMacroTurn>>> {
        let h = window.handle;
        // SAFETY: plain query of a handle.
        if !is_window(h) || unsafe { IsHungAppWindow(hwnd(h)) } != 0 {
            self.note_refused(window.pid, if is_window(h) { "it isn't responding" } else { "its window is gone" });
            return Ok(None);
        }
        let person = anti_afk::person_tick();
        // Lent: its place in the stack and state noted; minimised or hidden, shown unseen (but not click-through: the
        // macro's clicks must land on it). Left alone by the app's other window rules until the turn gives it back.
        let Some(loan) = window_loan::borrow(h, How::TURN) else {
            self.note_refused(window.pid, "its window is gone or isn't responding");
            return Ok(None);
        };
        let (minimized, unshown) = (loan.was().minimised, loan.was().unshown);
        let turn = Turn::new(h, person, self.person_at.clone(), self.raw_works.clone(), loan, self.turns.clone());
        // A zero move first: Windows lets the app that last sent input change the window in front.
        turn.send(&[mouse(0, 0, MOUSEEVENTF_MOVE, 0)]);
        anti_afk::focus(h);
        let waited = Instant::now();
        while foreground() != h && waited.elapsed() < FRONT_TIME {
            if !ct.sleep(Duration::from_millis(10)) {
                turn.dispose();
                return Err(Fail::other("cancelled"));
            }
            anti_afk::focus(h);
        }
        if foreground() != h {
            self.note_refused(
                window.pid,
                &format!(
                    "Windows didn't bring it forward in {} ms{}",
                    FRONT_TIME.as_millis(),
                    if minimized {
                        ", it was minimized"
                    } else if unshown {
                        ", it was hidden"
                    } else {
                        ""
                    }
                ),
            );
            turn.dispose();
            return Ok(None);
        }
        {
            let mut refused = self.refused.lock().unwrap_or_else(|e| e.into_inner());
            if refused.is_some() {
                *refused = None;
                drop(refused);
                log::note(&format!("Macro: window {} came forward again.", window.pid));
            }
        }
        // A restored window needs a moment to draw again (as anti-AFK gives it); one in view, a frame.
        let wait = if minimized || unshown { 400 } else { 17.max(if window.fps > 0 { 1000 / window.fps } else { 17 }) };
        if !ct.sleep(Duration::from_millis(wait as u64)) {
            turn.dispose();
            return Err(Fail::other("cancelled"));
        }
        *turn.client.lock().unwrap_or_else(|e| e.into_inner()) = MacroDesk::client_on_screen(h);
        let shared: Arc<dyn IMacroTurn> = Arc::new(turn);
        // Known by its place in the Arc from now on (AreaAsync).
        self.turns.lock().unwrap_or_else(|e| e.into_inner()).insert(turn_key(shared.as_ref()), (minimized, unshown));
        Ok(Some(shared))
    }
}

/// What a turn changes as it goes.
struct TurnState {
    /// Bloxkeep's own windows made click-through for this turn, with the extended style each had.
    through: Vec<(isize, i32)>,
    /// What this turn pressed and has not let go yet, with the input that lets it go.
    down: HeldInputs<INPUT>,
    sent: bool,
    done: bool,
    /// GetLastInputInfo's tick when we last sent; input processed well after it is a person's.
    last_sent: u32,
}

/// A window in front for a macro's input.
pub struct Turn {
    h: isize,
    person: u32,
    person_at: Option<PersonAtFn>,
    raw_works: Option<RawWorksFn>,
    cursor: Option<Point>,
    /// The window as lent (taken when the turn gives it back).
    loan: Mutex<Option<Loan>>,
    started: Stamp,
    client: Mutex<Rect>,
    state: Mutex<TurnState>,
    turns: Turns,
}

// SAFETY: INPUT holds only plain numbers (its dwExtraInfo is a usize), so the held inputs may move between threads.
unsafe impl Send for TurnState {}

impl Turn {
    fn new(h: isize, person: u32, person_at: Option<PersonAtFn>, raw_works: Option<RawWorksFn>, loan: Loan, turns: Turns) -> Turn {
        Turn {
            h,
            person,
            person_at,
            raw_works,
            cursor: cursor_pos(),
            loan: Mutex::new(Some(loan)),
            started: Stamp::utc_now(),
            client: Mutex::new(Rect::default()),
            state: Mutex::new(TurnState { through: Vec::new(), down: HeldInputs::new(), sent: false, done: false, last_sent: tick_count() }),
            turns,
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, TurnState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn send(&self, inputs: &[INPUT]) {
        Turn::send_in(&mut self.state(), inputs);
    }

    /// Sends under the state lock the caller holds, so a press and its note go together: abort, between the two, would
    /// let go of everything noted and leave this press down. False when the turn is over (nothing sent).
    fn send_in(st: &mut TurnState, inputs: &[INPUT]) -> bool {
        if st.done {
            return false; // given back (or Bloxkeep closing): nothing more goes out
        }
        // SAFETY: the slice's length and element size are passed as Windows requires.
        let taken = unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32) };
        // Written when it starts and stops failing, not for every key.
        if (taken == 0) != SEND_FAILING.load(Ordering::SeqCst) {
            SEND_FAILING.store(taken == 0, Ordering::SeqCst);
            if taken == 0 {
                // SAFETY: plain call.
                let error = unsafe { GetLastError() };
                log::note(&format!("Macro: Windows refused Bloxkeep's input (SendInput error {error})."));
            } else {
                log::note("Macro: Windows takes Bloxkeep's input again.");
            }
        }
        st.sent = true;
        st.last_sent = tick_count();
        true
    }

    /// A press and what lets it go noted, in one go under the lock.
    fn press(&self, inputs: &[INPUT], what: &str, up: bool, release: INPUT) {
        let mut st = self.state();
        if Turn::send_in(&mut st, inputs) {
            st.down.note(what, up, release);
        }
    }

    /// Ends the turn's input: what is still down (a press halfway, a step that panicked) let go, nothing sent after.
    /// False when it had ended already.
    fn let_go_all(&self) -> bool {
        let mut st = self.state();
        if st.done {
            return false;
        }
        let ups = st.down.ups();
        if !ups.is_empty() {
            // SAFETY: the slice's length and element size are passed as Windows requires.
            unsafe { SendInput(ups.len() as u32, ups.as_ptr(), std::mem::size_of::<INPUT>() as i32) };
            st.sent = true;
            st.last_sent = tick_count();
        }
        st.done = true;
        true
    }

    /// Our windows, the pointer, your window and the Roblox window's place and state back (a re-minimise or re-hide
    /// waited for up to `wait`, twice); nothing is sent after it. `person`: a person's input came during the turn, so a
    /// window they now have in front is left open (window_loan::back).
    fn put_back(&self, person: bool, wait: Duration) {
        let through = {
            let mut st = self.state();
            st.done = true;
            std::mem::take(&mut st.through)
        };
        self.turns.lock().unwrap_or_else(|e| e.into_inner()).remove(&((self as *const Turn).cast::<()>() as usize));
        for (window, style) in through {
            if is_window(window) {
                // SAFETY: plain call on a window of ours.
                unsafe { SetWindowLongW(hwnd(window), GWL_EXSTYLE, style) };
            }
        }
        if let Some(c) = self.cursor {
            // SAFETY: plain call.
            unsafe { SetCursorPos(c.x, c.y) };
        }
        let loan = self.loan.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(loan) = loan {
            loan.give_back_with(Some(person), wait);
        }
    }
}

impl IMacroTurn for Turn {
    /// Read again each time (the window may have moved or changed size since the turn began); the last known when
    /// Windows can't say (the window going).
    fn client(&self) -> Rect {
        let now = MacroDesk::client_on_screen(self.h);
        let mut known = self.client.lock().unwrap_or_else(|e| e.into_inner());
        if now.width > 0 && now.height > 0 {
            *known = now;
        }
        *known
    }

    fn still_front(&self) -> bool {
        foreground() == self.h && is_window(self.h)
    }

    fn person_input(&self) -> bool {
        let last_sent = self.state().last_sent;
        MacroDesk::person_since(self.started, self.person_at.as_ref(), self.raw_works.as_ref(), anti_afk::last_input_tick(), last_sent)
    }

    fn key(&self, key: &MacroKey, up: bool) {
        let input = keyboard(key.vk, key.scan, KEYEVENTF_SCANCODE | if key.extended { KEYEVENTF_EXTENDEDKEY } else { 0 } | if up { KEYEVENTF_KEYUP } else { 0 });
        let mut release = input;
        // SAFETY: a keyboard INPUT: its union holds a KEYBDINPUT.
        unsafe { release.Anonymous.ki.dwFlags |= KEYEVENTF_KEYUP };
        self.press(&[input], &format!("key {}:{}", key.scan, if key.extended { "True" } else { "False" }), up, release);
    }

    /// PORT: C# got one UTF-16 unit per call; the player hands a whole character once (`up` false), and each of its
    /// units goes down and up together in one SendInput (hi down, hi up, lo down, lo up for a pair past U+FFFF), so the
    /// halves never interleave. Nothing stays held; `up` true sends nothing.
    fn char(&self, ch: char, up: bool) {
        if up {
            return;
        }
        let mut units = [0u16; 2];
        let mut inputs = Vec::with_capacity(4);
        for &c in ch.encode_utf16(&mut units).iter() {
            let input = keyboard(0, c, KEYEVENTF_UNICODE);
            let mut release = input;
            // SAFETY: a keyboard INPUT: its union holds a KEYBDINPUT.
            unsafe { release.Anonymous.ki.dwFlags |= KEYEVENTF_KEYUP };
            inputs.push(input);
            inputs.push(release);
        }
        self.send(&inputs);
    }

    fn move_to(&self, screen: Point) {
        let (x, y) = macro_geometry::absolute(screen, virtual_desktop());
        self.send(&[mouse(x, y, MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK, 0)]);
    }

    fn move_by(&self, dx: i32, dy: i32) {
        self.send(&[mouse(dx, dy, MOUSEEVENTF_MOVE, 0)]);
    }

    fn button(&self, button: &str, up: bool) {
        let flag = |released: bool| match button {
            "right" => {
                if released {
                    MOUSEEVENTF_RIGHTUP
                } else {
                    MOUSEEVENTF_RIGHTDOWN
                }
            }
            "middle" => {
                if released {
                    MOUSEEVENTF_MIDDLEUP
                } else {
                    MOUSEEVENTF_MIDDLEDOWN
                }
            }
            _ => {
                if released {
                    MOUSEEVENTF_LEFTUP
                } else {
                    MOUSEEVENTF_LEFTDOWN
                }
            }
        };
        self.press(&[mouse(0, 0, flag(up), 0)], &format!("button {}", flag(true)), up, mouse(0, 0, flag(true), 0));
    }

    fn wheel(&self, notches: i32) {
        self.send(&[mouse(0, 0, MOUSEEVENTF_WHEEL, notches.wrapping_mul(120))]);
    }

    fn clear(&self, screen: Point) -> bool {
        for _ in 0..8 {
            // SAFETY: plain queries.
            let root = unsafe { GetAncestor(WindowFromPoint(POINT { x: screen.x, y: screen.y }), GA_ROOT) } as isize;
            if root == 0 || root == self.h {
                return true;
            }
            if pid_of(root) != std::process::id() {
                return false; // another app's window
            }
            // Noted under the lock, changed after it: SetWindowLongW waits on the window's (the app's) thread, which may
            // itself be waiting on this lock in abort.
            let style = {
                let mut st = self.state();
                if st.done || st.through.iter().any(|w| w.0 == root) {
                    return false; // the turn is over, or ours that won't let clicks through
                }
                // SAFETY: plain query of a window of ours (no message sent).
                let style = unsafe { GetWindowLongW(hwnd(root), GWL_EXSTYLE) };
                st.through.push((root, style));
                style
            };
            // SAFETY: plain calls on a window of ours.
            unsafe {
                SetWindowLongW(hwnd(root), GWL_EXSTYLE, style | WS_EX_LAYERED as i32 | WS_EX_TRANSPARENT as i32);
                // Newly layered, a window shows nothing until it is given its opacity.
                if style & WS_EX_LAYERED as i32 == 0 {
                    SetLayeredWindowAttributes(hwnd(root), 0, 255, LWA_ALPHA);
                }
            }
            // Given back meanwhile (abort, from the app's thread): its style was put back before this change, so again.
            let missed = {
                let st = self.state();
                st.done && !st.through.iter().any(|w| w.0 == root)
            };
            if missed && is_window(root) {
                // SAFETY: plain call on a window of ours.
                unsafe { SetWindowLongW(hwnd(root), GWL_EXSTYLE, style) };
                return false;
            }
        }
        false
    }

    fn cursor_at(&self) -> Point {
        cursor_pos().unwrap_or_default()
    }

    /// The pointer back where it was, your window back in front, the Roblox window back in its place and
    /// state, and our input noted as ours (anti-AFK and the next turn don't take it for a person's).
    fn dispose(&self) {
        if !self.let_go_all() {
            return;
        }
        let person = self.person_input();
        self.put_back(person, window_loan::WAIT);
        if self.state().sent {
            std::thread::sleep(Duration::from_millis(30)); // our last input reaches GetLastInputInfo
            anti_afk::note_own_input(self.person);
        }
    }

    /// Bloxkeep closing: what is still down let go and the PC given back before this returns.
    fn abort(&self) {
        if !self.let_go_all() {
            return;
        }
        let person = self.person_input();
        self.put_back(person, ABORT_WAIT);
    }
}

/// A turn dropped without being given back (the player's thread panicked mid-step): given back here, keys let go.
impl Drop for Turn {
    fn drop(&mut self) {
        self.dispose();
    }
}

fn virtual_desktop() -> Rect {
    // SAFETY: plain queries.
    unsafe {
        Rect::new(
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    }
}

fn keyboard(vk: u16, scan: u16, flags: u32) -> INPUT {
    INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: 0 } } }
}

fn mouse(dx: i32, dy: i32, flags: u32, data: i32) -> INPUT {
    INPUT { r#type: INPUT_MOUSE, Anonymous: INPUT_0 { mi: MOUSEINPUT { dx, dy, mouseData: data as _, dwFlags: flags, time: 0, dwExtraInfo: 0 } } }
}

/// What a turn pressed and has not let go yet, each with the input that lets it go, oldest first.
pub struct HeldInputs<T> {
    held: Vec<(String, T)>,
}

impl<T: Clone> HeldInputs<T> {
    pub fn new() -> HeldInputs<T> {
        HeldInputs { held: Vec::new() }
    }

    /// A press notes what it pressed; letting it go forgets it.
    pub fn note(&mut self, what: &str, up: bool, release: T) {
        self.held.retain(|h| h.0 != what);
        if !up {
            self.held.push((what.to_string(), release));
        }
    }

    /// Every let-go still owed, newest first; forgotten once taken.
    pub fn ups(&mut self) -> Vec<T> {
        let ups = self.held.iter().rev().map(|h| h.1.clone()).collect();
        self.held.clear();
        ups
    }
}

impl<T: Clone> Default for HeldInputs<T> {
    fn default() -> Self {
        HeldInputs::new()
    }
}
