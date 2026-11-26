// What the suites that work on real windows share: each starts itself again on a desktop of its own that is never shown
// (Windows moves, minimises, hides and stacks windows there as on the one in view), makes windows of its own there that
// stand in for Roblox's, and tells the run that started it what it found. Nothing of it shows on the desktop in view, and
// no key or click is sent: the keyboard can't be given on a desktop that isn't in view (SetForegroundWindow says no there,
// and the window in front stays none), so a check that needs it is left to the code's own decision.
use std::{
    collections::HashMap,
    ffi::c_void,
    ptr::{null, null_mut},
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

use windows_sys::Win32::{
    Foundation::{CloseHandle, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    Graphics::Gdi::{BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, PAINTSTRUCT},
    System::Threading::{CreateProcessW, GetCurrentThreadId, GetExitCodeProcess, PROCESS_INFORMATION, STARTUPINFOW, Sleep, TerminateProcess, WaitForSingleObject},
    UI::WindowsAndMessaging::{
        CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DispatchMessageW, EnumWindows, GetClientRect, GetMessageW, GetWindowRect, IsIconic, IsWindowVisible, IsZoomed,
        MINMAXINFO, MSG, PostMessageW, PostThreadMessageW, RegisterClassW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_RESTORE, SW_SHOWNOACTIVATE, SetWindowPos,
        SetWindowTextW, ShowWindow, WM_APP, WM_GETMINMAXINFO, WM_MOVE, WM_PAINT, WM_QUIT, WM_WINDOWPOSCHANGED, WNDCLASSW, WS_VISIBLE,
    },
};

use super::Checks;
use crate::core::win_kit::{Rect, Size};

#[link(name = "user32")]
unsafe extern "system" {
    fn CreateDesktopW(name: *const u16, device: *const u16, devmode: *const c_void, flags: u32, access: u32, security: *const c_void) -> isize;
    fn CloseDesktop(desktop: isize) -> i32;
    fn GetThreadDesktop(thread: u32) -> isize;
    fn GetUserObjectInformationW(object: isize, index: i32, info: *mut c_void, length: u32, needed: *mut u32) -> i32;
}

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// The name of the desktop this thread is on ("Default": the one in view).
fn desktop_name() -> String {
    const UOI_NAME: i32 = 2;
    let mut name = [0u16; 128];
    let mut needed = 0u32;
    // SAFETY: a buffer of the length given; plain queries.
    let ok = unsafe { GetUserObjectInformationW(GetThreadDesktop(GetCurrentThreadId()), UOI_NAME, name.as_mut_ptr().cast(), (name.len() * 2) as u32, &mut needed) };
    if ok == 0 {
        return String::new();
    }
    String::from_utf16_lossy(&name).trim_end_matches('\0').to_string()
}

/// This run is on the desktop in view: its window checks go to a desktop of their own (`run_there`).
pub fn in_view() -> bool {
    desktop_name() == "Default"
}

/// Checks said here and kept for the run that started this one.
pub struct Rec<'a> {
    c: &'a mut Checks,
    lines: Vec<String>,
}

impl<'a> Rec<'a> {
    pub fn new(c: &'a mut Checks) -> Rec<'a> {
        Rec { c, lines: Vec::new() }
    }
    pub fn check(&mut self, what: &str, ok: bool, detail: impl std::fmt::Debug) {
        let detail = format!("{detail:?}");
        self.lines.push(if ok { format!("P\t{what}") } else { format!("F\t{what}\t{detail}") });
        self.c.check(what, ok, detail);
    }
    pub fn skip(&mut self, what: &str, why: &str) {
        self.lines.push(format!("S\t{what}\t{why}"));
        self.c.skip(what, why);
    }
    /// Written for the run that started this one, in this run's folder.
    pub fn save(self, dir: &str) {
        let _ = std::fs::write(std::path::Path::new(dir).join("results.txt"), self.lines.join("\n"));
    }
}

/// Starts `suite` again on a desktop made for it (never switched to: nothing of it shows), waits `wait_ms` at most for it,
/// and takes its checks as this run's.
pub fn run_there(c: &mut Checks, dir: &str, suite: &str, wait_ms: u32) {
    const GENERIC_ALL: u32 = 0x1000_0000;
    const WAIT_TIMEOUT: u32 = 258;
    let name = format!("bloxkeep-{suite}-test-{}", std::process::id());
    let child_dir = format!("{dir}-desk");
    let results = std::path::Path::new(&child_dir).join("results.txt");
    let _ = std::fs::remove_file(&results);
    let ran = "the window checks ran on a desktop of their own";
    // SAFETY: plain calls with strings that outlive them; the handles made are closed below.
    let (code, late) = unsafe {
        let desktop = CreateDesktopW(wide(&name).as_ptr(), null(), null(), 0, GENERIC_ALL, null());
        if desktop == 0 {
            c.skip(ran, "Windows made no desktop for them");
            return;
        }
        let exe = std::env::current_exe().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        let mut line = wide(&format!("\"{exe}\" --core-suite {suite} \"{child_dir}\""));
        let mut on = wide(&format!("WinSta0\\{name}"));
        let mut si: STARTUPINFOW = std::mem::zeroed();
        si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
        si.lpDesktop = on.as_mut_ptr();
        let mut pi: PROCESS_INFORMATION = std::mem::zeroed();
        let started = CreateProcessW(null(), line.as_mut_ptr(), null(), null(), 0, 0, null(), null(), &si, &mut pi);
        let (mut code, mut late) = (u32::MAX, false);
        if started != 0 {
            // One that runs past its time is ended: its windows go with it, and its desktop can be closed.
            if WaitForSingleObject(pi.hProcess, wait_ms) == WAIT_TIMEOUT {
                late = true;
                TerminateProcess(pi.hProcess, 1);
                WaitForSingleObject(pi.hProcess, 5000);
            }
            GetExitCodeProcess(pi.hProcess, &mut code);
            CloseHandle(pi.hThread);
            CloseHandle(pi.hProcess);
        }
        CloseDesktop(desktop);
        (code, late)
    };
    let Ok(text) = std::fs::read_to_string(&results) else {
        c.check(ran, false, format!("no results; its exit code was {code}{}", if late { ", ended after its time" } else { "" }));
        return;
    };
    c.check(ran, true, ());
    for line in text.lines() {
        let mut parts = line.splitn(3, '\t');
        match (parts.next(), parts.next(), parts.next()) {
            (Some("P"), Some(what), _) => c.check(what, true, ()),
            (Some("F"), Some(what), detail) => c.check(what, false, detail.unwrap_or("")),
            (Some("S"), Some(what), why) => c.skip(what, why.unwrap_or("")),
            _ => {}
        }
    }
}

// ───────── windows that stand in for Roblox's ─────────

/// Asks a stand-in's thread to be busy for this many ms (wParam), as a Roblox window busy loading takes an ask late.
pub const BUSY: u32 = WM_APP + 1;
const GROW: u32 = WM_APP + 2;
const SHRINK: u32 = WM_APP + 3;

/// How a stand-in answers what is asked of it, besides as any window does.
#[derive(Clone, Copy, Debug, Default)]
pub struct Kind {
    /// The least size it can be sized to by a drag or an ordinary move (WM_GETMINMAXINFO), as Roblox keeps 800 × 600 in
    /// its own units. A move sent without the "changing" message isn't held to it.
    pub least: Option<Size>,
    /// Sized smaller than `least` however that was done, it grows back to it a moment after.
    pub grows_back: bool,
    /// On its next move it takes two thirds of its size: at once (false) or a moment after (true), as Roblox answers a
    /// move onto a screen at another scale.
    pub shrinks_on_move: Option<bool>,
    /// Drawn steel blue with an orange edge 40 px wide at its right and bottom.
    pub painted: bool,
}

static KINDS: LazyLock<Mutex<HashMap<isize, Kind>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn kinds() -> std::sync::MutexGuard<'static, HashMap<isize, Kind>> {
    KINDS.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn set_kind(w: isize, kind: Kind) {
    kinds().insert(w, kind);
}

fn kind_of(w: isize) -> Kind {
    kinds().get(&w).copied().unwrap_or_default()
}

/// Steel blue and orange, as COLORREF (0x00BBGGRR).
pub const STEEL_BLUE: u32 = 0x00B4_8246;
pub const ORANGE: u32 = 0x0000_A5FF;
pub const EDGE: i32 = 40;

unsafe extern "system" fn stand_in_proc(hw: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    let me = hw as isize;
    // SAFETY: the window's own messages, handed on as they came; plain calls on the window itself. The kind is copied
    // out of its lock before anything that sends the window another message.
    unsafe {
        match m {
            BUSY => {
                Sleep(w as u32);
                0
            }
            WM_GETMINMAXINFO => {
                let done = DefWindowProcW(hw, m, w, l);
                if let Some(least) = kind_of(me).least {
                    let mm = &mut *(l as *mut MINMAXINFO);
                    mm.ptMinTrackSize = POINT { x: least.width, y: least.height };
                }
                done
            }
            WM_WINDOWPOSCHANGED => {
                let done = DefWindowProcW(hw, m, w, l);
                let k = kind_of(me);
                if k.grows_back && k.least.is_some() {
                    PostMessageW(hw, GROW, 0, 0);
                }
                done
            }
            GROW => {
                grow_back(me);
                0
            }
            WM_MOVE => {
                let done = DefWindowProcW(hw, m, w, l);
                let later = {
                    let mut all = kinds();
                    all.get_mut(&me).and_then(|k| k.shrinks_on_move.take())
                };
                match later {
                    Some(true) => {
                        PostMessageW(hw, SHRINK, 0, 0);
                    }
                    Some(false) => shrink(me),
                    None => {}
                }
                done
            }
            SHRINK => {
                shrink(me);
                0
            }
            WM_PAINT if kind_of(me).painted => {
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let dc = BeginPaint(hw, &mut ps);
                let mut all = RECT { left: 0, top: 0, right: 0, bottom: 0 };
                GetClientRect(hw, &mut all);
                let (blue, orange) = (CreateSolidBrush(STEEL_BLUE), CreateSolidBrush(ORANGE));
                FillRect(dc, &all, blue);
                FillRect(dc, &RECT { left: all.right - EDGE, ..all }, orange);
                FillRect(dc, &RECT { top: all.bottom - EDGE, ..all }, orange);
                DeleteObject(blue);
                DeleteObject(orange);
                EndPaint(hw, &ps);
                0
            }
            _ => DefWindowProcW(hw, m, w, l),
        }
    }
}

fn grow_back(w: isize) {
    let Some(least) = kind_of(w).least else { return };
    if iconic(w) || zoomed(w) {
        return;
    }
    let r = rect_of(w);
    if r.width < least.width || r.height < least.height {
        // SAFETY: plain call on the window's own thread.
        unsafe { SetWindowPos(h(w), null_mut(), 0, 0, r.width.max(least.width), r.height.max(least.height), SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE) };
    }
}

fn shrink(w: isize) {
    let r = rect_of(w);
    // SAFETY: plain call on the window's own thread.
    unsafe { SetWindowPos(h(w), null_mut(), 0, 0, r.width * 2 / 3, r.height * 2 / 3, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE) };
}

/// A thread of `count` windows of `style` that answer as any app's do (their `Kind` aside), laid one below and right of
/// the other. They are gone with the thread, when this is dropped.
pub struct Windows {
    pub thread: u32,
    pub all: Vec<isize>,
}

impl Drop for Windows {
    fn drop(&mut self) {
        // SAFETY: plain call; the windows end with their thread.
        unsafe { PostThreadMessageW(self.thread, WM_QUIT, 0, 0) };
    }
}

pub fn make_windows(count: usize, style: u32) -> Windows {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let class = wide("BloxkeepDeskTestWindow");
        // SAFETY: a class of this process with the stand-ins' procedure (registered once; a second time is refused and
        // the first stands); windows made and pumped on this thread only.
        unsafe {
            let mut wc: WNDCLASSW = std::mem::zeroed();
            wc.style = CS_HREDRAW | CS_VREDRAW;
            wc.lpfnWndProc = Some(stand_in_proc);
            wc.lpszClassName = class.as_ptr();
            RegisterClassW(&wc);
            let all: Vec<isize> = (0..count as i32)
                .map(|i| CreateWindowExW(0, class.as_ptr(), class.as_ptr(), style | WS_VISIBLE, 40 + (i % 8) * 30, 40 + (i % 8) * 30, 500, 400, null_mut(), null_mut(), null_mut(), null()) as isize)
                .collect();
            let _ = tx.send((GetCurrentThreadId(), all));
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
                DispatchMessageW(&msg);
            }
        }
    });
    let (thread, all) = rx.recv().expect("the windows' thread");
    Windows { thread, all }
}

pub fn h(w: isize) -> HWND {
    w as _
}

pub fn iconic(w: isize) -> bool {
    // SAFETY: plain query.
    unsafe { IsIconic(h(w)) != 0 }
}

pub fn zoomed(w: isize) -> bool {
    // SAFETY: plain query.
    unsafe { IsZoomed(h(w)) != 0 }
}

pub fn visible(w: isize) -> bool {
    // SAFETY: plain query.
    unsafe { IsWindowVisible(h(w)) != 0 }
}

pub fn rect_of(w: isize) -> Rect {
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    // SAFETY: one RECT on the stack.
    unsafe { GetWindowRect(h(w), &mut r) };
    Rect::new(r.left, r.top, r.right - r.left, r.bottom - r.top)
}

pub fn client_size(w: isize) -> Size {
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    // SAFETY: one RECT on the stack.
    unsafe { GetClientRect(h(w), &mut r) };
    Size::new(r.right - r.left, r.bottom - r.top)
}

pub fn show(w: isize, how: i32) {
    // SAFETY: plain call; the window's thread answers it.
    unsafe { ShowWindow(h(w), how) };
}

pub fn set_title(w: isize, title: &str) {
    // SAFETY: the text outlives the call.
    unsafe { SetWindowTextW(h(w), wide(title).as_ptr()) };
}

/// An ordinary open window at `r` (sized as a person's move would: held to its least size).
pub fn put(w: isize, r: Rect) {
    show(w, SW_RESTORE);
    show(w, SW_SHOWNOACTIVATE);
    // SAFETY: plain call.
    unsafe { SetWindowPos(h(w), null_mut(), r.x, r.y, r.width, r.height, SWP_NOZORDER | SWP_NOACTIVATE) };
}

/// To the top of the stack, without the keyboard.
pub fn top(w: isize) {
    // SAFETY: plain call.
    unsafe { SetWindowPos(h(w), null_mut(), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
}

/// What is asked of a window comes about a moment later, on the window's own thread.
pub fn settle() {
    std::thread::sleep(Duration::from_millis(350));
}

pub fn near(a: Rect, b: Rect, slack: i32) -> bool {
    (a.x - b.x).abs() <= slack && (a.y - b.y).abs() <= slack && (a.right() - b.right()).abs() <= slack && (a.bottom() - b.bottom()).abs() <= slack
}

/// Until `done`, `ms` at most.
pub fn wait_for(ms: u64, done: impl Fn() -> bool) -> bool {
    let until = Instant::now() + Duration::from_millis(ms);
    loop {
        if done() {
            return true;
        }
        if Instant::now() >= until {
            return false;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

unsafe extern "system" fn list_window(hw: HWND, data: LPARAM) -> i32 {
    // SAFETY: `data` is the Vec passed by `stacking`, alive for the whole EnumWindows call.
    let list = unsafe { &mut *(data as *mut Vec<isize>) };
    list.push(hw as isize);
    1
}

/// This desktop's top-level windows, the topmost first.
pub fn stacking() -> Vec<isize> {
    let mut list: Vec<isize> = Vec::new();
    // SAFETY: the callback only touches `list`, which outlives the call.
    unsafe { EnumWindows(Some(list_window), &mut list as *mut Vec<isize> as LPARAM) };
    list
}

/// Where `w` stands in `order` (0: on top); None when it isn't there.
pub fn rank(order: &[isize], w: isize) -> Option<usize> {
    order.iter().position(|&x| x == w)
}
