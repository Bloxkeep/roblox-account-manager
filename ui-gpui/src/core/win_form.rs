// What the app's own windows (tray, mini window, overlay, page windows, start cover, the shell's hooks) stood on and
// WinForms gave the C# app for free: the one UI thread every Form lived on (Application.Run, BeginInvoke, async/await
// continuing on that thread, System.Windows.Forms.Timer), a Form's handle made with its CreateParams, Show/Hide/Close/
// Dispose and their events. No C# file of its own: these were WinForms'.
//
// The windows thread: `attach` makes the calling thread it (the interface's thread, which pumps messages for every
// window on it, as Application.Run did); `start` makes one of its own when nothing attached (before the interface
// exists: a dialog at start). Everything here that touches a window runs on it; `invoke` gets there from any thread.
#![allow(clippy::missing_safety_doc)]

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    future::Future,
    pin::Pin,
    rc::{Rc, Weak},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering},
    },
    task::Context,
};

use futures::{
    channel::oneshot,
    task::{ArcWake, waker},
};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    Graphics::Gdi::{ClientToScreen, InvalidateRect, MONITOR_DEFAULTTONEAREST, MonitorFromPoint, RDW_ALLCHILDREN, RDW_ERASE, RDW_FRAME, RDW_INVALIDATE, RedrawWindow},
    System::{
        Com::{COINIT_APARTMENTTHREADED, CoInitializeEx},
        LibraryLoader::GetModuleHandleW,
        Threading::GetCurrentThreadId,
    },
    UI::{
        HiDpi::{GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI},
        Input::KeyboardAndMouse::{GetAsyncKeyState, SetActiveWindow, VK_LBUTTON},
        WindowsAndMessaging::*,
    },
};

use super::{
    base::{Fail, Res},
    themes::Color,
    win_kit::{Point, Rect, Screen, Size, all_screens, hwnd, screen_from_rect, wide},
};

// ───────── the windows thread ─────────

const WM_APP_INVOKE: u32 = WM_APP + 0x31;
const WM_APP_WAKE: u32 = WM_APP + 0x32;

/// The hidden window the windows thread gets `invoke`s, task wake-ups and timers through (0: no windows thread yet).
static INVOKE_HWND: AtomicIsize = AtomicIsize::new(0);
static WINDOWS_THREAD: AtomicU64 = AtomicU64::new(0);
static STARTING: AtomicBool = AtomicBool::new(false);

type Task = Pin<Box<dyn Future<Output = ()>>>;
type Posted = Box<dyn FnOnce() + Send + 'static>;

thread_local! {
    static TASKS: RefCell<HashMap<u64, Task>> = RefCell::new(HashMap::new());
    /// Tasks being polled now, and those woken meanwhile (from a nested loop under the poll: a modal, a menu, a drag):
    /// polled again once the poll gives back, or the wake would be lost.
    static POLLING: RefCell<std::collections::HashSet<u64>> = RefCell::new(std::collections::HashSet::new());
    static REWAKE: RefCell<std::collections::HashSet<u64>> = RefCell::new(std::collections::HashSet::new());
    static NEXT_TASK: Cell<u64> = const { Cell::new(1) };
    static TIMERS: RefCell<HashMap<usize, Weak<TimerInner>>> = RefCell::new(HashMap::new());
    static DELAYS: RefCell<HashMap<usize, oneshot::Sender<()>>> = RefCell::new(HashMap::new());
    // The handler box create_window is making a window with, until WM_NCCREATE takes it.
    static UNTAKEN: Cell<usize> = const { Cell::new(0) };
    static NEXT_TIMER: Cell<usize> = const { Cell::new(1) };
}

/// Makes the calling thread the windows thread (it must pump messages: the interface's thread does). Once; later
/// calls do nothing.
pub fn attach() -> bool {
    if INVOKE_HWND.load(Ordering::SeqCst) != 0 {
        return is_windows_thread();
    }
    // SAFETY: COM for this thread as WinForms' [STAThread] had it (WebView2 needs it); S_FALSE when it already was.
    unsafe {
        CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32);
    }
    let class = wide("Bloxkeep.Invoke");
    // SAFETY: a plain window class with our procedure; registering it twice only fails, which is fine.
    let h = unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(invoke_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class.as_ptr(),
        };
        RegisterClassW(&wc);
        CreateWindowExW(0, class.as_ptr(), class.as_ptr(), 0, 0, 0, 0, 0, HWND_MESSAGE, std::ptr::null_mut(), instance, std::ptr::null())
    };
    if h.is_null() {
        return false;
    }
    // SAFETY: plain query.
    WINDOWS_THREAD.store(unsafe { GetCurrentThreadId() } as u64, Ordering::SeqCst);
    INVOKE_HWND.store(h as isize, Ordering::SeqCst);
    true
}

/// A windows thread of its own, pumping messages, when none is attached yet (a dialog before the interface).
pub fn start() {
    if INVOKE_HWND.load(Ordering::SeqCst) != 0 || STARTING.swap(true, Ordering::SeqCst) {
        return;
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = crate::threads::own("windows", move || {
        let ok = attach();
        let _ = tx.send(());
        if ok {
            pump();
        }
    });
    let _ = rx.recv();
}

/// Application.Run's loop: until WM_QUIT.
pub fn pump() {
    // SAFETY: the standard message loop over a zeroed MSG.
    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// Waits for `rx`'s answer while this thread's windows go on getting their messages (a modal wait, as ShowDialog's loop
/// was): for the windows thread waiting on another thread that may send to its windows (an owner disabled by a dialog
/// on a thread of its own). None when the sender went away.
pub fn wait_pumping<T>(rx: &std::sync::mpsc::Receiver<T>) -> Option<T> {
    wait_pumping_for(rx, None)
}

/// wait_pumping with a time limit: None once `limit` has passed without an answer.
pub fn wait_pumping_for<T>(rx: &std::sync::mpsc::Receiver<T>, limit: Option<std::time::Duration>) -> Option<T> {
    use std::sync::mpsc::TryRecvError;
    let deadline = limit.map(|l| std::time::Instant::now() + l);
    loop {
        match rx.try_recv() {
            Ok(v) => return Some(v),
            Err(TryRecvError::Disconnected) => return None,
            Err(TryRecvError::Empty) => {}
        }
        if deadline.is_some_and(|d| std::time::Instant::now() >= d) {
            return None;
        }
        // SAFETY: a wait on no handles that wakes for any message (or after 50 ms, for the answer), then the standard
        // dispatch of what came.
        unsafe {
            MsgWaitForMultipleObjects(0, std::ptr::null(), 0, 50, QS_ALLINPUT);
            let mut msg: MSG = std::mem::zeroed();
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_QUIT {
                    // The app is closing: no more waiting without the pump, since the other thread may be calling
                    // into this one (a dialog's EnableWindow on its owner) and would hang the quit.
                    PostQuitMessage(msg.wParam as i32);
                    return rx.try_recv().ok();
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}

/// This is the windows thread (InvokeRequired is false).
pub fn is_windows_thread() -> bool {
    // SAFETY: plain query.
    WINDOWS_THREAD.load(Ordering::SeqCst) == unsafe { GetCurrentThreadId() } as u64
}

/// The windows thread is there.
pub fn ready() -> bool {
    INVOKE_HWND.load(Ordering::SeqCst) != 0
}

/// BeginInvoke: `f` runs on the windows thread, in its turn. Dropped when there is no windows thread.
pub fn invoke(f: impl FnOnce() + Send + 'static) {
    let h = INVOKE_HWND.load(Ordering::SeqCst);
    if h == 0 {
        return;
    }
    let boxed: Box<Posted> = Box::new(Box::new(f));
    let raw = Box::into_raw(boxed);
    // SAFETY: the box is taken back by invoke_proc, or here when the message couldn't be posted.
    unsafe {
        if PostMessageW(hwnd(h), WM_APP_INVOKE, 0, raw as LPARAM) == 0 {
            drop(Box::from_raw(raw));
        }
    }
}

struct Wake {
    id: u64,
}

impl ArcWake for Wake {
    fn wake_by_ref(me: &Arc<Self>) {
        let h = INVOKE_HWND.load(Ordering::SeqCst);
        // SAFETY: a plain post; the id is looked up on the windows thread.
        unsafe {
            PostMessageW(hwnd(h), WM_APP_WAKE, me.id as WPARAM, 0);
        }
    }
}

/// An async method of a window, not waited for (`_ = FooAsync()`, `async void`): runs on the windows thread from its
/// next turn. Only from the windows thread.
pub fn spawn(f: impl Future<Output = ()> + 'static) {
    let id = NEXT_TASK.with(|n| {
        let id = n.get();
        n.set(id + 1);
        id
    });
    TASKS.with(|t| t.borrow_mut().insert(id, Box::pin(f)));
    Wake::wake_by_ref(&Arc::new(Wake { id }));
}

fn poll_task(id: u64) {
    let Some(mut task) = TASKS.with(|t| t.borrow_mut().remove(&id)) else {
        if POLLING.with(|p| p.borrow().contains(&id)) {
            REWAKE.with(|r| r.borrow_mut().insert(id));
        }
        return;
    };
    let w = waker(Arc::new(Wake { id }));
    let mut cx = Context::from_waker(&w);
    POLLING.with(|p| p.borrow_mut().insert(id));
    // A task that panics ends there (shown as an unexpected error); the windows thread goes on.
    let pending = super::program::shielded(|| false, || task.as_mut().poll(&mut cx).is_pending());
    POLLING.with(|p| p.borrow_mut().remove(&id));
    let again = REWAKE.with(|r| r.borrow_mut().remove(&id));
    if pending {
        TASKS.with(|t| t.borrow_mut().insert(id, task));
        if again {
            w.wake_by_ref();
        }
    }
}

/// Task.Delay on the windows thread. A delay dropped before its time (it lost a race: `wait_for`) kills its timer.
pub async fn delay(ms: u32) {
    struct Pending(usize);
    impl Drop for Pending {
        fn drop(&mut self) {
            let id = self.0;
            if DELAYS.try_with(|d| d.borrow_mut().remove(&id)).ok().flatten().is_some() {
                // SAFETY: the timer set below on our own window; not fired yet, so still there.
                unsafe {
                    KillTimer(hwnd(INVOKE_HWND.load(Ordering::SeqCst)), id);
                }
            }
        }
    }
    let (tx, rx) = oneshot::channel();
    let id = next_timer_id();
    DELAYS.with(|d| d.borrow_mut().insert(id, tx));
    let _pending = Pending(id);
    let h = INVOKE_HWND.load(Ordering::SeqCst);
    // SAFETY: a timer on our own hidden window; killed when it fires or when this is dropped.
    unsafe {
        SetTimer(hwnd(h), id, ms.max(1), None);
    }
    let _ = rx.await;
}

/// Slow work (a download, a wait for a process) on a pool thread while the window's task waits for it.
pub async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    crate::threads::call(f).await.ok().and_then(|answer| answer.ok())
}

/// Done when `cancel` is called off (looked at every 50 ms), for racing against a wait.
pub async fn cancelled(cancel: super::base::Cancel) {
    while !cancel.is_cancelled() {
        delay(50).await;
    }
}

/// A wait for `f` with a time limit and a token (WaitAsync(timeout, token)): Err "timed out" or "cancelled".
pub async fn wait_for<T>(f: impl Future<Output = T>, timeout_ms: u32, cancel: &super::base::Cancel) -> Res<T> {
    use futures::FutureExt;
    let mut f = Box::pin(f.fuse());
    let mut limit = Box::pin(delay(timeout_ms).fuse());
    let mut stop = Box::pin(cancelled(cancel.clone()).fuse());
    futures::select! {
        v = f => Ok(v),
        _ = limit => Err(Fail::other("The operation has timed out.")),
        _ = stop => Err(Fail::other("cancelled")),
    }
}

fn next_timer_id() -> usize {
    NEXT_TIMER.with(|n| {
        let id = n.get();
        n.set(id + 1);
        id
    })
}

unsafe extern "system" fn invoke_proc(h: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_APP_INVOKE => {
            // SAFETY: lparam is the Box::into_raw of `invoke`, taken back once here.
            let f = unsafe { Box::from_raw(lparam as *mut Posted) };
            super::program::shielded(|| (), f);
            0
        }
        WM_APP_WAKE => {
            poll_task(wparam as u64);
            0
        }
        WM_TIMER => {
            let id = wparam;
            if let Some(tx) = DELAYS.with(|d| d.borrow_mut().remove(&id)) {
                // SAFETY: the timer was set on this window with this id.
                unsafe {
                    KillTimer(h, id);
                }
                let _ = tx.send(());
            } else if let Some(t) = TIMERS.with(|t| t.borrow().get(&id).and_then(|w| w.upgrade())) {
                let tick = t.tick.borrow().clone();
                if let Some(tick) = tick {
                    super::program::shielded(|| (), || tick());
                }
            } else {
                // SAFETY: a timer nobody holds any more.
                unsafe {
                    KillTimer(h, id);
                }
            }
            0
        }
        // SAFETY: the default handling of every other message.
        _ => unsafe { DefWindowProcW(h, msg, wparam, lparam) },
    }
}

// ───────── System.Windows.Forms.Timer ─────────

struct TimerInner {
    id: usize,
    interval: Cell<u32>,
    enabled: Cell<bool>,
    tick: RefCell<Option<Rc<dyn Fn()>>>,
}

/// A WinForms Timer: ticks on the windows thread every `interval` ms while started.
#[derive(Clone)]
pub struct Timer(Rc<TimerInner>);

impl Timer {
    pub fn new(interval: u32) -> Timer {
        let inner = Rc::new(TimerInner { id: next_timer_id(), interval: Cell::new(interval.max(1)), enabled: Cell::new(false), tick: RefCell::new(None) });
        TIMERS.with(|t| t.borrow_mut().insert(inner.id, Rc::downgrade(&inner)));
        Timer(inner)
    }
    /// Tick += …
    pub fn on_tick(&self, f: impl Fn() + 'static) {
        *self.0.tick.borrow_mut() = Some(Rc::new(f));
    }
    pub fn start(&self) {
        let h = INVOKE_HWND.load(Ordering::SeqCst);
        self.0.enabled.set(true);
        // SAFETY: (re)sets our own timer on our hidden window.
        unsafe {
            SetTimer(hwnd(h), self.0.id, self.0.interval.get(), None);
        }
    }
    pub fn stop(&self) {
        self.0.enabled.set(false);
        let h = INVOKE_HWND.load(Ordering::SeqCst);
        // SAFETY: kills our own timer; nothing happens when it isn't set.
        unsafe {
            KillTimer(hwnd(h), self.0.id);
        }
    }
    pub fn enabled(&self) -> bool {
        self.0.enabled.get()
    }
    pub fn set_enabled(&self, on: bool) {
        if on { self.start() } else { self.stop() }
    }
    pub fn interval(&self) -> u32 {
        self.0.interval.get()
    }
    /// Interval = ms: a running timer starts counting again, as WinForms' does.
    pub fn set_interval(&self, ms: u32) {
        self.0.interval.set(ms.max(1));
        if self.enabled() {
            self.start();
        }
    }
    pub fn dispose(&self) {
        self.stop();
        *self.0.tick.borrow_mut() = None;
        TIMERS.with(|t| t.borrow_mut().remove(&self.0.id));
    }
}

// ───────── a Form's handle ─────────

/// Why a form is closing (FormClosingEventArgs.CloseReason).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseReason {
    None,
    /// The person (the X, Alt+F4) or the app's own Close().
    UserClosing,
    WindowsShutDown,
    TaskManagerClosing,
}

/// A window's message handler: Some(result) when handled, None for the default handling.
pub type Handler = Rc<dyn Fn(isize, u32, usize, isize) -> Option<isize>>;

/// What a window registered elsewhere (Ui.NoteOnTop's list) knows of a form: its handle (0: none yet) and whether it
/// was disposed. Can cross threads.
#[derive(Clone, Default)]
pub struct FormRef {
    pub hwnd: Arc<AtomicIsize>,
    pub disposed: Arc<AtomicBool>,
}

impl FormRef {
    pub fn handle(&self) -> isize {
        self.hwnd.load(Ordering::SeqCst)
    }
    pub fn is_disposed(&self) -> bool {
        self.disposed.load(Ordering::SeqCst)
    }
}

unsafe extern "system" fn form_proc(h: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: GWLP_USERDATA holds the Box<Handler> put there at WM_NCCREATE, freed at WM_NCDESTROY.
    unsafe {
        if msg == WM_NCCREATE {
            let cs = &*(lparam as *const CREATESTRUCTW);
            SetWindowLongPtrW(h, GWLP_USERDATA, cs.lpCreateParams as isize);
            UNTAKEN.with(|u| {
                if u.get() == cs.lpCreateParams as usize {
                    u.set(0);
                }
            });
        }
        let data = GetWindowLongPtrW(h, GWLP_USERDATA) as *const Handler;
        let mut result = None;
        if !data.is_null() {
            let handler = (*data).clone();
            result = super::program::shielded(|| None, || handler(h as isize, msg, wparam, lparam));
        }
        if msg == WM_NCDESTROY && !data.is_null() {
            SetWindowLongPtrW(h, GWLP_USERDATA, 0);
            drop(Box::from_raw(data as *mut Handler));
        }
        match result {
            Some(r) => r,
            None => DefWindowProcW(h, msg, wparam, lparam),
        }
    }
}

/// Registers the class once (a CreateParams.ClassStyle is a class of its own).
fn ensure_class(name: &str, class_style: u32) {
    let class = wide(name);
    // SAFETY: registering an existing class only fails; the arrow cursor as WinForms' default.
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let mut existing: WNDCLASSW = std::mem::zeroed();
        if GetClassInfoW(instance, class.as_ptr(), &mut existing) != 0 {
            return;
        }
        let wc = WNDCLASSW {
            style: class_style | CS_DBLCLKS,
            lpfnWndProc: Some(form_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: std::ptr::null_mut(),
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class.as_ptr(),
        };
        RegisterClassW(&wc);
    }
}

/// CreateHandle: a window of class `class` (made with `class_style`), its styles, place and title; `handler` gets every
/// message. On the windows thread.
pub fn create_window(class: &str, class_style: u32, style: u32, ex_style: u32, bounds: Rect, parent: isize, title: &str, handler: Handler) -> Res<isize> {
    ensure_class(class, class_style);
    let class_w = wide(class);
    let title_w = wide(title);
    let data = Box::into_raw(Box::new(handler));
    UNTAKEN.with(|u| u.set(data as usize));
    // SAFETY: the handler's box goes in through lpCreateParams and is freed at WM_NCDESTROY (or below when nothing was made).
    let h = unsafe {
        CreateWindowExW(
            ex_style,
            class_w.as_ptr(),
            title_w.as_ptr(),
            style,
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
            hwnd(parent),
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            data as *const std::ffi::c_void,
        )
    };
    let untaken = UNTAKEN.with(|u| u.replace(0)) == data as usize;
    if h.is_null() {
        // Once WM_NCCREATE took the box, WM_NCDESTROY freed it with the failed window.
        if untaken {
            // SAFETY: no window took the box.
            unsafe { drop(Box::from_raw(data)) };
        }
        return Err(Fail::other("CreateWindowEx failed"));
    }
    Ok(h as isize)
}

// ───────── a Form's members ─────────

pub fn is_window(h: isize) -> bool {
    // SAFETY: plain query.
    h != 0 && unsafe { IsWindow(hwnd(h)) } != 0
}

/// Visible.
pub fn is_visible(h: isize) -> bool {
    // SAFETY: plain query.
    h != 0 && unsafe { IsWindowVisible(hwnd(h)) } != 0
}

pub fn is_iconic(h: isize) -> bool {
    // SAFETY: plain query.
    h != 0 && unsafe { IsIconic(hwnd(h)) } != 0
}

pub fn is_zoomed(h: isize) -> bool {
    // SAFETY: plain query.
    h != 0 && unsafe { IsZoomed(hwnd(h)) } != 0
}

/// Bounds: the window's place on screen.
pub fn bounds(h: isize) -> Rect {
    let mut r: RECT = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    // SAFETY: plain query into a RECT on the stack.
    unsafe {
        GetWindowRect(hwnd(h), &mut r);
    }
    Rect::from_win(&r)
}

/// ClientSize.
pub fn client_size(h: isize) -> Size {
    let mut r: RECT = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    // SAFETY: plain query into a RECT on the stack.
    unsafe {
        GetClientRect(hwnd(h), &mut r);
    }
    Size::new(r.right - r.left, r.bottom - r.top)
}

/// The client area's top-left on screen (PointToScreen(Point.Empty)).
pub fn client_origin(h: isize) -> Point {
    let mut p = POINT { x: 0, y: 0 };
    // SAFETY: plain conversion of a POINT on the stack.
    unsafe {
        ClientToScreen(hwnd(h), &mut p);
    }
    Point::new(p.x, p.y)
}

/// Bounds = r, without changing the z-order or taking the keyboard.
pub fn set_bounds(h: isize, r: Rect) {
    // SAFETY: plain call on our own window.
    unsafe {
        SetWindowPos(hwnd(h), std::ptr::null_mut(), r.x, r.y, r.width, r.height, SWP_NOZORDER | SWP_NOACTIVATE);
    }
}

/// Location = p.
pub fn set_location(h: isize, p: Point) {
    // SAFETY: plain call on our own window.
    unsafe {
        SetWindowPos(hwnd(h), std::ptr::null_mut(), p.x, p.y, 0, 0, SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOSIZE);
    }
}

/// Size = s.
pub fn set_size(h: isize, s: Size) {
    // SAFETY: plain call on our own window.
    unsafe {
        SetWindowPos(hwnd(h), std::ptr::null_mut(), 0, 0, s.width, s.height, SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOMOVE);
    }
}

/// Show(): with or without taking the keyboard (ShowWithoutActivation).
pub fn show(h: isize, activate: bool) {
    // SAFETY: plain call.
    unsafe {
        ShowWindow(hwnd(h), if activate { SW_SHOW } else { SW_SHOWNOACTIVATE });
    }
}

/// Hide().
pub fn hide(h: isize) {
    // SAFETY: plain call.
    unsafe {
        ShowWindow(hwnd(h), SW_HIDE);
    }
}

/// Activate(): to the front with the keyboard, when Windows lets it.
pub fn activate(h: isize) {
    // SAFETY: plain calls on our own window.
    unsafe {
        SetForegroundWindow(hwnd(h));
        SetActiveWindow(hwnd(h));
    }
}

pub fn set_foreground(h: isize) -> bool {
    // SAFETY: plain call.
    unsafe { SetForegroundWindow(hwnd(h)) != 0 }
}

pub fn foreground_window() -> isize {
    // SAFETY: plain query.
    unsafe { GetForegroundWindow() as isize }
}

/// DeviceDpi: the window's monitor's DPI (96 without a handle).
pub fn device_dpi(h: isize) -> u32 {
    if h == 0 {
        return 96;
    }
    // SAFETY: plain query.
    let dpi = unsafe { GetDpiForWindow(hwnd(h)) };
    if dpi == 0 { 96 } else { dpi }
}

/// The DPI of the monitor nearest a point (96 when Windows can't say).
pub fn dpi_at(p: Point) -> u32 {
    // SAFETY: plain queries.
    unsafe {
        let monitor = MonitorFromPoint(POINT { x: p.x, y: p.y }, MONITOR_DEFAULTTONEAREST);
        let (mut x, mut y) = (0u32, 0u32);
        if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x, &mut y) == 0 && x > 0 { x } else { 96 }
    }
}

/// LogicalToDeviceUnits: logical pixels at the window's scale.
pub fn logical_to_device(h: isize, v: i32) -> i32 {
    (v as f64 * device_dpi(h) as f64 / 96.0).round() as i32
}

/// Cursor.Position.
pub fn cursor_position() -> Point {
    let mut p = POINT { x: 0, y: 0 };
    // SAFETY: plain query.
    unsafe {
        GetCursorPos(&mut p);
    }
    Point::new(p.x, p.y)
}

/// (MouseButtons & MouseButtons.Left) != 0.
pub fn left_button_down() -> bool {
    // SAFETY: plain query.
    (unsafe { GetAsyncKeyState(VK_LBUTTON as i32) } as u16 & 0x8000) != 0
}

/// Screen.FromPoint.
pub fn screen_from_point(p: Point) -> Screen {
    screen_from_rect(Rect::new(p.x, p.y, 1, 1))
}

/// Screen.PrimaryScreen ?? Screen.AllScreens[0].
pub fn primary_screen() -> Screen {
    let all = all_screens();
    all.iter().find(|s| s.primary).cloned().or_else(|| all.first().cloned()).unwrap_or_else(|| screen_from_point(Point::default()))
}

/// Rectangle.IntersectsWith.
pub fn intersects(a: Rect, b: Rect) -> bool {
    b.x < a.x + a.width && a.x < b.x + b.width && b.y < a.y + a.height && a.y < b.y + b.height
}

/// ColorTranslator.ToWin32: 0x00BBGGRR.
pub fn colorref(c: Color) -> u32 {
    c.r as u32 | (c.g as u32) << 8 | (c.b as u32) << 16
}

/// Opacity: below 1 the window is layered (WinForms made it WS_EX_LAYERED the same way); 1 solid again.
pub fn set_opacity(h: isize, opacity: f64) {
    // SAFETY: plain style and layering calls on our own window.
    unsafe {
        let ex = GetWindowLongPtrW(hwnd(h), GWL_EXSTYLE) as u32;
        if opacity >= 1.0 {
            // Solid again: WinForms took the layered style off at Opacity = 1, and so do we.
            if ex & WS_EX_LAYERED != 0 {
                SetWindowLongPtrW(hwnd(h), GWL_EXSTYLE, (ex & !WS_EX_LAYERED) as isize);
                RedrawWindow(hwnd(h), std::ptr::null(), std::ptr::null_mut(), RDW_ERASE | RDW_INVALIDATE | RDW_FRAME | RDW_ALLCHILDREN);
            }
            return;
        }
        if ex & WS_EX_LAYERED == 0 {
            SetWindowLongPtrW(hwnd(h), GWL_EXSTYLE, (ex | WS_EX_LAYERED) as isize);
        }
        SetLayeredWindowAttributes(hwnd(h), 0, (opacity.clamp(0.0, 1.0) * 255.0).round() as u8, LWA_ALPHA);
    }
}

/// The ex-style has WS_EX_TOPMOST.
pub fn is_top_most(h: isize) -> bool {
    // SAFETY: plain query.
    (unsafe { GetWindowLongPtrW(hwnd(h), GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST) != 0
}

/// Place the main window behind the foreground without moving it into the topmost group.
pub(crate) fn stack_normal_behind(h: isize, front: isize) {
    const FLAGS: u32 = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER;
    // A topmost foreground window is already above any normal window. Inserting after its handle could promote ours.
    if !is_top_most(front) {
        // SAFETY: the caller verified both window handles on the UI thread.
        unsafe { SetWindowPos(hwnd(h), hwnd(front), 0, 0, 0, 0, FLAGS) };
    }
    if is_top_most(h) {
        // The foreground could also have changed its group during the first call.
        unsafe { SetWindowPos(hwnd(h), HWND_NOTOPMOST, 0, 0, 0, 0, FLAGS) };
    }
}

/// The window's thread is this one (`!InvokeRequired`).
pub fn on_own_thread(h: isize) -> bool {
    // SAFETY: plain queries.
    unsafe { GetWindowThreadProcessId(hwnd(h), std::ptr::null_mut()) == GetCurrentThreadId() }
}

/// DestroyWindow.
pub fn destroy(h: isize) {
    if is_window(h) {
        // SAFETY: our own window, on its thread.
        unsafe {
            DestroyWindow(hwnd(h));
        }
    }
}

/// Invalidate().
pub fn invalidate(h: isize) {
    // SAFETY: plain call.
    unsafe {
        InvalidateRect(hwnd(h), std::ptr::null(), 0);
    }
}

/// Update(): paints what is invalid now.
pub fn update(h: isize) {
    // SAFETY: plain call.
    unsafe {
        windows_sys::Win32::Graphics::Gdi::UpdateWindow(hwnd(h));
    }
}

/// The window's process id (GetWindowThreadProcessId).
pub fn window_pid(h: isize) -> u32 {
    let mut pid = 0u32;
    // SAFETY: plain query.
    unsafe {
        GetWindowThreadProcessId(hwnd(h), &mut pid);
    }
    pid
}

/// Text = …
pub fn set_text(h: isize, text: &str) {
    let w = wide(text);
    // SAFETY: plain call with a zero-ended string.
    unsafe {
        SetWindowTextW(hwnd(h), w.as_ptr());
    }
}

/// Icon = …: the big and small icon of the window.
pub fn set_icon(h: isize, icon: isize) {
    if icon == 0 {
        return;
    }
    // SAFETY: plain messages to our own window; the icon is the app's, kept for the whole run.
    unsafe {
        SendMessageW(hwnd(h), WM_SETICON, ICON_BIG as WPARAM, icon);
        // (The title bar's: the .ico's own small picture when this is the app's icon, not the big one shrunk.)
        let small = if super::ui::app_icon() == Some(icon) { super::ui::app_icon_small().unwrap_or(icon) } else { icon };
        SendMessageW(hwnd(h), WM_SETICON, ICON_SMALL as WPARAM, small);
    }
}

/// A WM_DPICHANGED's suggested rectangle.
pub fn suggested_rect(lparam: isize) -> Rect {
    // SAFETY: WM_DPICHANGED's lParam points to a RECT for the message's time.
    Rect::from_win(unsafe { &*(lparam as *const RECT) })
}

/// MinimumSize and MaximumSize as WM_GETMINMAXINFO answers them (Size.Empty: no limit).
pub fn apply_min_max(lparam: isize, min: Size, max: Size) {
    // SAFETY: WM_GETMINMAXINFO's lParam points to a MINMAXINFO for the message's time.
    let info = unsafe { &mut *(lparam as *mut MINMAXINFO) };
    if !min.is_empty() {
        info.ptMinTrackSize.x = min.width;
        info.ptMinTrackSize.y = min.height;
    }
    if !max.is_empty() {
        info.ptMaxTrackSize.x = max.width;
        info.ptMaxTrackSize.y = max.height;
    }
}
