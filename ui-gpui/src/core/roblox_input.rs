//! Which window the person's own input reached, for anti-AFK. GetLastInputInfo only says the PC was used: a mouse moved
//! on another monitor counted for the Roblox window in front (2026-10-19: a window was kicked for idling 8 minutes after
//! Bloxkeep last counted input for it). Raw input (RIDEV_INPUTSINK) names each event's device; input with none, sent by a
//! program (anti-AFK's F15, a Vietnamese input method), is left out (seen 2026-10-22). Keys count for the window in
//! front, the mouse only when over it; which key or where is never read. Runs on its own thread: a mouse sends up to
//! thousands of moves a second.
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::Duration,
};

use windows_sys::Win32::{
    Foundation::{GetLastError, HWND, LPARAM, LRESULT, POINT, WPARAM},
    System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
    UI::{
        Input::{GetRawInputData, HRAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER, RegisterRawInputDevices},
        WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GWLP_USERDATA, GetCursorPos, GetMessageW, GetWindowLongPtrW, GetWindowThreadProcessId,
            HWND_MESSAGE, MSG, PostThreadMessageW, RegisterClassW, SetWindowLongPtrW, WNDCLASSW, WindowFromPoint,
        },
    },
};

use super::{base::Stamp, log, roblox_processes, win_kit::wide};

const WM_INPUT: u32 = 0x00FF;
const WM_QUIT: u32 = 0x0012;
const WM_LISTEN: u32 = 0x8000 + 1; // WM_APP + 1: wParam 1 listens, 0 stops
const RID_HEADER: u32 = 0x10000005;
const RID_INPUT: u32 = 0x10000003;
const RIM_TYPEMOUSE: u32 = 0;
const RIM_TYPEKEYBOARD: u32 = 1;
const RIDEV_INPUTSINK: u32 = 0x100;
const RIDEV_REMOVE: u32 = 0x1;
const GENERIC_DESKTOP: u16 = 1;
const MOUSE: u16 = 2;
const KEYBOARD: u16 = 6;

/// A mouse sends hundreds of moves a second: where the pointer is, is looked at once in this time (250 ms).
const MOUSE_LOOK_SECS: f64 = 0.25;

type MovedFn = Box<dyn Fn(i32, i32) + Send + Sync>;

/// What the input thread and the callers share.
struct Shared {
    reached: Mutex<HashMap<i32, Stamp>>,
    mouse_looked_at: Mutex<Option<Stamp>>,
    person_at: AtomicI64,
    mouse_moved: Mutex<Option<MovedFn>>,
}

pub struct RobloxInput {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
    thread_id: Arc<AtomicU32>,
    working: Arc<AtomicBool>,
    listening: bool,
    listen_fail_noted: bool,
}

impl RobloxInput {
    pub fn new() -> RobloxInput {
        let shared = Arc::new(Shared {
            reached: Mutex::new(HashMap::new()),
            mouse_looked_at: Mutex::new(None),
            person_at: AtomicI64::new(0),
            mouse_moved: Mutex::new(None),
        });
        let thread_id = Arc::new(AtomicU32::new(0));
        let working = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = mpsc::channel::<()>();
        let (s, t, wk) = (shared.clone(), thread_id.clone(), working.clone());
        // PORT: the C# thread was STA for its WinForms NativeWindow; a plain Win32 message-only window needs no COM apartment.
        let thread = crate::threads::own("Bloxkeep input", move || run(s, t, wk, ready_tx)).ok();
        let _ = ready_rx.recv_timeout(Duration::from_secs(5));
        RobloxInput { shared, thread, thread_id, working, listening: true, listen_fail_noted: false }
    }

    /// Windows takes the registration: input is seen. When it doesn't, anti-AFK counts as before. False too once the
    /// input thread is gone (nothing is seen any more: a person would never count).
    pub fn working(&self) -> bool {
        self.working.load(Ordering::SeqCst) && self.thread.as_ref().is_some_and(|t| !t.is_finished())
    }

    /// Input is taken only while it is needed (a Roblox window runs, AppController's process check says): with none, every
    /// move of a 1000–8000 Hz mouse would wake this thread for nothing. Done on this class's own thread; the same twice is nothing.
    pub fn listen(&mut self, on: bool) {
        let id = self.thread_id.load(Ordering::SeqCst);
        if !self.working() || on == self.listening || id == 0 {
            return;
        }
        // SAFETY: plain call posting to the input thread's queue.
        if unsafe { PostThreadMessageW(id, WM_LISTEN, if on { 1 } else { 0 }, 0) } != 0 {
            self.listening = on;
        } else if !self.listen_fail_noted {
            self.listen_fail_noted = true; // tried again every process check: written once
            log::note(&format!("Raw input: couldn't ask its thread to turn listening {}.", if on { "on" } else { "off" }));
        }
    }

    /// When the person's own input last reached this process's window (UTC), or None.
    pub fn reached_at(&self, pid: i32) -> Option<Stamp> {
        self.shared.reached.lock().unwrap_or_else(|e| e.into_inner()).get(&pid).copied()
    }

    /// A handle a macro recording keeps (C# handed the recorder the RobloxInput itself): whether input is seen, and the
    /// mouse-move callback. It doesn't own the input thread.
    pub fn handle(&self) -> RawInput {
        RawInput { shared: self.shared.clone(), working: self.working.clone() }
    }

    /// Dispose: asks the input thread to end.
    pub fn dispose(&mut self) {
        let id = self.thread_id.load(Ordering::SeqCst);
        if id != 0 && self.thread.as_ref().is_some_and(|t| !t.is_finished()) {
            // SAFETY: plain call posting to the input thread's queue.
            unsafe {
                PostThreadMessageW(id, WM_QUIT, 0, 0);
            }
        }
    }
}

/// What a macro recording uses of RobloxInput (`RobloxInput::handle`).
#[derive(Clone)]
pub struct RawInput {
    shared: Arc<Shared>,
    working: Arc<AtomicBool>,
}

impl RawInput {
    pub fn working(&self) -> bool {
        self.working.load(Ordering::SeqCst)
    }
    /// Same as `RobloxInput::person_at`, read from any thread.
    pub fn person_at(&self) -> Option<Stamp> {
        let ms = self.shared.person_at.load(Ordering::SeqCst);
        if ms > 0 { Some(Stamp::from_unix_ms(ms)) } else { None }
    }
    pub fn set_mouse_moved(&self, moved: Option<MovedFn>) {
        *self.shared.mouse_moved.lock().unwrap_or_else(|e| e.into_inner()) = moved;
    }
}

impl Default for RobloxInput {
    fn default() -> Self {
        RobloxInput::new()
    }
}

impl Drop for RobloxInput {
    fn drop(&mut self) {
        self.dispose();
    }
}

/// The process an input reached, or 0: a key goes to the window in front, the mouse only when the pointer is
/// over it.
pub fn reached(keyboard: bool, front: i32, under_pointer: i32) -> i32 {
    if front != 0 && (keyboard || under_pointer == front) { front } else { 0 }
}

/// Mouse and keyboard to this window, also in the background; with no window, that undone.
fn register(target: HWND) -> bool {
    let flags = if target.is_null() { RIDEV_REMOVE } else { RIDEV_INPUTSINK };
    let devices = [
        RAWINPUTDEVICE { usUsagePage: GENERIC_DESKTOP, usUsage: MOUSE, dwFlags: flags, hwndTarget: target },
        RAWINPUTDEVICE { usUsagePage: GENERIC_DESKTOP, usUsage: KEYBOARD, dwFlags: flags, hwndTarget: target },
    ];
    // SAFETY: two RAWINPUTDEVICEs and their size.
    unsafe { RegisterRawInputDevices(devices.as_ptr(), 2, std::mem::size_of::<RAWINPUTDEVICE>() as u32) != 0 }
}

/// The message-only window Windows sends the input to: its procedure finds the shared state through GWLP_USERDATA.
unsafe extern "system" fn sink_proc(h: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_INPUT {
        // SAFETY: GWLP_USERDATA holds the Arc<Shared> pointer set by `run`, alive until the window is destroyed.
        let p = unsafe { GetWindowLongPtrW(h, GWLP_USERDATA) } as *const Shared;
        if !p.is_null() {
            // SAFETY: as above.
            note(unsafe { &*p }, lparam as HRAWINPUT);
        }
    }
    // SAFETY: the default handling of every message (base.WndProc).
    unsafe { DefWindowProcW(h, msg, wparam, lparam) }
}

/// Not working any more once the input thread ends, however it ends (RawInput handles have no thread to look at).
struct NotWorking(Arc<AtomicBool>);

impl Drop for NotWorking {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

fn run(shared: Arc<Shared>, thread_id: Arc<AtomicU32>, working: Arc<AtomicBool>, ready: mpsc::Sender<()>) {
    let _ended = NotWorking(working.clone());
    let class = wide("BloxkeepInputSink");
    let window: HWND;
    // SAFETY: the class and window are this thread's; the Arc stays alive in this function until the window is destroyed.
    unsafe {
        thread_id.store(GetCurrentThreadId(), Ordering::SeqCst);
        let instance = GetModuleHandleW(std::ptr::null());
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(sink_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class.as_ptr(),
        };
        RegisterClassW(&wc); // a second RobloxInput finds it registered already
        window = CreateWindowExW(0, class.as_ptr(), std::ptr::null(), 0, 0, 0, 0, 0, HWND_MESSAGE, std::ptr::null_mut(), instance, std::ptr::null());
        if window.is_null() {
            log::note(&format!("Raw input: no window to receive input (error {}).", GetLastError()));
        } else {
            SetWindowLongPtrW(window, GWLP_USERDATA, Arc::as_ptr(&shared) as isize);
            let ok = register(window);
            working.store(ok, Ordering::SeqCst);
            if !ok {
                log::note(&format!(
                    "Raw input: Windows refused the registration (error {}); anti-AFK and macros go by Windows' last input instead.",
                    GetLastError()
                ));
            }
        }
    }
    let _ = ready.send(());
    if working.load(Ordering::SeqCst) {
        // SAFETY: an ordinary message loop of this thread.
        unsafe {
            let mut message: MSG = std::mem::zeroed();
            while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
                if message.hwnd.is_null() && message.message == WM_LISTEN {
                    let on = message.wParam != 0;
                    if !register(if on { window } else { std::ptr::null_mut() }) {
                        log::note(&format!("Raw input: turning listening {} failed (error {}).", if on { "on" } else { "off" }, GetLastError()));
                    }
                } else {
                    DispatchMessageW(&message);
                }
            }
        }
        register(std::ptr::null_mut());
    }
    if !window.is_null() {
        // SAFETY: the window is this thread's; its pointer to `shared` goes with it.
        unsafe {
            SetWindowLongPtrW(window, GWLP_USERDATA, 0);
            DestroyWindow(window);
        }
    }
    drop(shared);
}

fn note(shared: &Shared, input: HRAWINPUT) {
    let header_size = std::mem::size_of::<RAWINPUTHEADER>() as u32;
    let mut size = header_size;
    // SAFETY: the header is plain data of the size given.
    let header = unsafe {
        let mut header: RAWINPUTHEADER = std::mem::zeroed();
        if GetRawInputData(input, RID_HEADER, &mut header as *mut RAWINPUTHEADER as *mut _, &mut size, header_size) == u32::MAX {
            return;
        }
        header
    };
    // No device: sent by a program (SendInput), not the person.
    if header.hDevice.is_null() || !(header.dwType == RIM_TYPEMOUSE || header.dwType == RIM_TYPEKEYBOARD) {
        return;
    }
    let keyboard = header.dwType == RIM_TYPEKEYBOARD;
    let now = Stamp::utc_now();
    shared.person_at.store(now.unix_ms().max(1), Ordering::SeqCst);
    if !keyboard {
        if let Some(moved) = shared.mouse_moved.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            moved_by(input, size, moved.as_ref());
        }
    }
    if !keyboard {
        let mut looked = shared.mouse_looked_at.lock().unwrap_or_else(|e| e.into_inner());
        if looked.is_some_and(|at| now.secs_since(at) < MOUSE_LOOK_SECS) {
            return;
        }
        *looked = Some(now);
    }
    let front = roblox_processes::foreground_pid();
    let pid = reached(keyboard, front, if keyboard { front } else { under_pointer() });
    if pid == 0 {
        return;
    }
    let mut map = shared.reached.lock().unwrap_or_else(|e| e.into_inner());
    if map.len() > 64 && !map.contains_key(&pid) {
        map.clear(); // only the last few minutes matter; this keeps the list to the windows in use
    }
    map.insert(pid, now);
}

/// RAWMOUSE after the header: usFlags (2 bytes, then 2 of padding), the button data (4), ulRawButtons (4),
/// lLastX, lLastY.
fn moved_by(input: HRAWINPUT, header_size: u32, moved: &(dyn Fn(i32, i32) + Send + Sync)) {
    const MOUSE_MOVE_ABSOLUTE: u16 = 1;
    let mut size = 0u32;
    // SAFETY: the first call asks only the size; the second fills a buffer of that size.
    unsafe {
        GetRawInputData(input, RID_INPUT, std::ptr::null_mut(), &mut size, header_size);
        if size < header_size + 20 || size > 256 {
            return;
        }
        let mut data = vec![0u8; size as usize];
        if GetRawInputData(input, RID_INPUT, data.as_mut_ptr() as *mut _, &mut size, header_size) == u32::MAX {
            return;
        }
        let at = header_size as usize;
        if (u16::from_le_bytes([data[at], data[at + 1]]) & MOUSE_MOVE_ABSOLUTE) != 0 {
            return; // a tablet or remote desktop: positions, not moves
        }
        let x = i32::from_le_bytes([data[at + 12], data[at + 13], data[at + 14], data[at + 15]]);
        let y = i32::from_le_bytes([data[at + 16], data[at + 17], data[at + 18], data[at + 19]]);
        if x != 0 || y != 0 {
            moved(x, y);
        }
    }
}

fn under_pointer() -> i32 {
    // SAFETY: plain queries with values on the stack.
    unsafe {
        let mut point = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut point) == 0 {
            return 0;
        }
        let window = WindowFromPoint(point);
        if window.is_null() {
            return 0;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(window, &mut pid);
        pid as i32
    }
}
