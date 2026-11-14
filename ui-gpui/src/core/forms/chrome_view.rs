// ChromeView.cs: the top of Bloxkeep's browser windows (WebsiteForm, BrowserLoginForm), and PageLook (the theme and
// language the app's own pages draw in). Also here, as `host`: what WinForms and its WebView2 control gave the C# windows
// of this folder for free (a window thread with its message loop, BeginInvoke, WinForms timers, Task.Delay on the UI
// thread, a WebView2 control in a child window), so the six windows of RobloxManager/Forms share one of each.
#![allow(clippy::missing_safety_doc)]

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use serde_json::{Value, json};

use crate::core::{
    app_pages, l, log,
    shell_form::{self, BrowserFailure},
    themes,
    win_kit::Rect,
};

use self::host::{WebHost, defer};

/// The theme and language a page of the app's draws in, as the app's own page gets them.
pub mod page_look {
    use serde_json::{Value, json};

    use crate::core::{prefs, themes};

    pub fn theme() -> String {
        themes::current().id.to_string()
    }

    /// A custom theme's colours, without its picture (these pages are plain).
    pub fn custom_theme() -> Value {
        let custom = prefs::lock().custom.clone();
        json!({
            "background": custom.background,
            "accent": custom.accent,
            "fade": custom.fade,
            "blur": custom.blur,
            "picture": Value::Null,
            "video": false,
        })
    }
}

/// What WinForms and the WebView2 WinForms control did for the C# windows: each window (or set of windows, as the
/// website window and its tabs) has a thread of its own with its message loop, as WinForms' UI thread; work for it
/// from elsewhere arrives through its `Invoker` (BeginInvoke); its `await`s run on it (`spawn`); `Timer` is
/// System.Windows.Forms.Timer; `WebHost` is the WebView2 control (a child window the browser draws into).
pub mod host {
    use std::{
        any::Any,
        cell::{Cell, RefCell},
        collections::{HashMap, HashSet},
        ffi::c_void,
        future::Future,
        pin::Pin,
        rc::{Rc, Weak},
        sync::{
            Arc, Once,
            atomic::{AtomicU64, Ordering},
        },
        task::{Context, Poll},
        time::Duration,
    };

    use futures::{
        channel::oneshot,
        future::LocalBoxFuture,
        task::{ArcWake, waker},
    };
    use webview2_com::{
        AcceleratorKeyPressedEventHandler, CreateCoreWebView2ControllerCompletedHandler, CreateCoreWebView2EnvironmentCompletedHandler,
        GetCookiesCompletedHandler,
        Microsoft::Web::WebView2::Win32::{
            COREWEBVIEW2_COLOR, COREWEBVIEW2_KEY_EVENT_KIND, COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN, COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN,
            COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC, CreateCoreWebView2EnvironmentWithOptions, ICoreWebView2, ICoreWebView2_2, ICoreWebView2Controller,
            ICoreWebView2Controller2, ICoreWebView2ControllerOptions, ICoreWebView2CookieList, ICoreWebView2Environment, ICoreWebView2Environment10,
            ICoreWebView2EnvironmentOptions, ICoreWebView2WebResourceResponse,
        },
        take_pwstr,
    };
    use windows::{
        Win32::{
            Foundation::{E_ABORT, E_POINTER},
            System::Com::IStream,
            UI::Shell::SHCreateMemStream,
        },
        core::{BOOL, HSTRING, Interface, PCWSTR, PWSTR},
    };
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{CombineRgn, CreateRectRgn, CreateSolidBrush, DeleteObject, FillRect, HDC, RGN_OR, SetWindowRgn},
        System::{
            Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize},
            LibraryLoader::GetModuleHandleW,
        },
        UI::{
            HiDpi::{GetDpiForSystem, GetDpiForWindow},
            Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_MENU, VK_SHIFT},
            WindowsAndMessaging::{
                CS_DBLCLKS, CS_DROPSHADOW, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GA_ROOT, GetAncestor, GetClientRect, GetMessageW,
                GetWindowRect, HWND_MESSAGE, HWND_TOP, ICON_BIG, ICON_SMALL, IDC_ARROW, IsIconic, IsWindow, IsWindowVisible, KillTimer, LoadCursorW, MSG,
                PM_REMOVE, PeekMessageW, PostMessageW, PostQuitMessage, RegisterClassExW, SW_HIDE, SW_SHOWNA, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
                SWP_NOZORDER, SendMessageW, SetTimer, SetWindowPos, SetWindowTextW, ShowWindow, TranslateMessage, WM_APP, WM_CLOSE, WM_ERASEBKGND,
                WM_NCDESTROY, WM_NULL, WM_SETICON, WM_SIZE, WM_TIMER, WNDCLASSEXW, WNDPROC, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_VISIBLE,
            },
        },
    };

    use crate::core::{
        rt,
        themes::Color,
        win_kit::{Rect, Size, wide},
    };

    pub type WinResult<T> = windows::core::Result<T>;

    /// A closure from another thread, to run on the window thread (lparam: a `Box<Box<dyn FnOnce() + Send>>`).
    const WM_RUN: u32 = WM_APP + 0x41;
    /// A task of the window thread was woken (wparam: its number).
    const WM_POLL: u32 = WM_APP + 0x42;

    /// A top-level window of these forms.
    pub const CLASS_FORM: &str = "BloxkeepForm";
    /// The same with Windows' shadow under it (CS_DROPSHADOW: the dialog, as WebToolWindow had).
    pub const CLASS_SHADOW: &str = "BloxkeepFormShadow";
    /// A child window: the panel of the website window's pages, a WebView2's host.
    pub const CLASS_CHILD: &str = "BloxkeepChild";
    const CLASS_INVOKER: &str = "BloxkeepInvoker";

    /// What ShowDialog returned.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum DialogResult {
        None,
        Ok,
        Cancel,
    }

    fn instance() -> windows_sys::Win32::Foundation::HINSTANCE {
        // SAFETY: plain call; null asks for this program's own module.
        unsafe { GetModuleHandleW(std::ptr::null()) }
    }

    fn register_classes() {
        static ONCE: Once = Once::new();
        ONCE.call_once(|| {
            let register = |name: &str, style: u32, proc_: WNDPROC| {
                let class = wide(name);
                // SAFETY: WNDCLASSEXW is plain data, filled before the call; Windows copies the class name.
                unsafe {
                    let mut wc: WNDCLASSEXW = std::mem::zeroed();
                    wc.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
                    wc.style = style;
                    wc.lpfnWndProc = proc_;
                    wc.hInstance = instance();
                    wc.hCursor = LoadCursorW(std::ptr::null_mut(), IDC_ARROW);
                    wc.lpszClassName = class.as_ptr();
                    RegisterClassExW(&wc);
                }
            };
            register(CLASS_FORM, CS_DBLCLKS, Some(form_proc));
            register(CLASS_SHADOW, CS_DBLCLKS | CS_DROPSHADOW, Some(form_proc));
            register(CLASS_CHILD, 0, Some(form_proc));
            register(CLASS_INVOKER, 0, Some(invoker_proc));
        });
    }

    /// A window's messages: Some(result) when handled, None for Windows' own handling.
    pub type Handler = Rc<dyn Fn(HWND, u32, WPARAM, LPARAM) -> Option<LRESULT>>;

    struct ThreadHost {
        invoker: isize,
        tasks: RefCell<HashMap<u64, Option<LocalBoxFuture<'static, ()>>>>,
        rewake: RefCell<HashSet<u64>>,
        next: Cell<u64>,
        timers: RefCell<HashMap<usize, Rc<dyn Fn()>>>,
    }

    thread_local! {
        static HANDLERS: RefCell<HashMap<isize, Handler>> = RefCell::new(HashMap::new());
        static HOST: RefCell<Option<Rc<ThreadHost>>> = const { RefCell::new(None) };
        /// The core's thread: callbacks the App gave a window (they hold its Rc, so they never leave this thread).
        static CORE_FNS: RefCell<HashMap<u64, Rc<dyn Any>>> = RefCell::new(HashMap::new());
    }

    static NEXT_CORE_FN: AtomicU64 = AtomicU64::new(1);

    fn try_here() -> Option<Rc<ThreadHost>> {
        HOST.try_with(|h| h.borrow().clone()).ok().flatten()
    }

    /// This thread's host, made the first time (its invoker: a message-only window).
    fn here() -> Rc<ThreadHost> {
        if let Some(h) = try_here() {
            return h;
        }
        register_classes();
        let class = wide(CLASS_INVOKER);
        // SAFETY: plain call; a message-only window of the class registered above.
        let invoker = unsafe {
            CreateWindowExW(0, class.as_ptr(), std::ptr::null(), 0, 0, 0, 0, 0, HWND_MESSAGE, std::ptr::null_mut(), instance(), std::ptr::null())
        } as isize;
        let host = Rc::new(ThreadHost { invoker, tasks: RefCell::default(), rewake: RefCell::default(), next: Cell::new(1000), timers: RefCell::default() });
        HOST.with(|h| *h.borrow_mut() = Some(host.clone()));
        host
    }

    /// This thread runs windows of these forms (a message loop of `host`).
    pub fn is_window_thread() -> bool {
        try_here().is_some()
    }

    unsafe extern "system" fn form_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        let handler = HANDLERS.try_with(|m| m.borrow().get(&(hwnd as isize)).cloned()).ok().flatten();
        let result = handler.and_then(|h| crate::core::program::shielded(|| None, || h(hwnd, msg, w, l)));
        if msg == WM_NCDESTROY {
            let _ = HANDLERS.try_with(|m| m.borrow_mut().remove(&(hwnd as isize)));
        }
        match result {
            Some(r) => r,
            // SAFETY: the window's own message, passed on as Windows gave it.
            None => unsafe { DefWindowProcW(hwnd, msg, w, l) },
        }
    }

    unsafe extern "system" fn invoker_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        match msg {
            WM_RUN => {
                // SAFETY: lparam is the box `Invoker::run` leaked for this one message.
                let f: Box<dyn FnOnce() + Send> = *unsafe { Box::from_raw(l as *mut Box<dyn FnOnce() + Send>) };
                crate::core::program::shielded(|| (), f);
                0
            }
            WM_POLL => {
                if let Some(h) = try_here() {
                    poll_task(&h, w as u64);
                }
                0
            }
            WM_TIMER => {
                let tick = try_here().and_then(|h| h.timers.borrow().get(&w).cloned());
                if let Some(tick) = tick {
                    crate::core::program::shielded(|| (), || tick());
                }
                0
            }
            // SAFETY: the window's own message, passed on as Windows gave it.
            _ => unsafe { DefWindowProcW(hwnd, msg, w, l) },
        }
    }

    pub fn set_handler(hwnd: isize, handler: impl Fn(HWND, u32, WPARAM, LPARAM) -> Option<LRESULT> + 'static) {
        HANDLERS.with(|m| m.borrow_mut().insert(hwnd, Rc::new(handler)));
    }

    // ───────── BeginInvoke ─────────

    /// A window thread, as other threads reach it (Control.BeginInvoke): a number, so it can cross threads.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Invoker(pub isize);

    impl Invoker {
        /// The calling window thread's.
        pub fn current() -> Invoker {
            Invoker(here().invoker)
        }

        /// Runs `f` on that thread, in its turn; false (and `f` dropped) when the thread is gone.
        pub fn run(self, f: impl FnOnce() + Send + 'static) -> bool {
            let boxed: Box<Box<dyn FnOnce() + Send>> = Box::new(Box::new(f));
            let raw = Box::into_raw(boxed);
            // SAFETY: the box is taken back by invoker_proc, or here when the message couldn't be posted.
            let ok = unsafe { PostMessageW(self.0 as HWND, WM_RUN, 0, raw as LPARAM) } != 0;
            if !ok {
                // SAFETY: not posted, so still ours.
                drop(unsafe { Box::from_raw(raw) });
            }
            ok
        }
    }

    // ───────── the thread and its loop ─────────

    /// A window thread (WinForms' UI thread): a single-threaded apartment, as WebView2 needs; `body` makes its windows,
    /// then its message loop runs until `quit`.
    pub fn start_thread(name: &str, body: impl FnOnce() + Send + 'static) -> bool {
        crate::net::Named::new(name.to_string())
            .spawn(move || {
                // SAFETY: plain call, paired with CoUninitialize below on this same thread.
                let hr = unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as _) };
                let _ = here();
                body();
                run_loop(None);
                shut_down();
                if hr >= 0 {
                    // SAFETY: CoInitializeEx succeeded on this thread.
                    unsafe { CoUninitialize() };
                }
            })
            .is_ok()
    }

    /// The message loop; with `until`, a nested one (ShowDialog's) that ends when it says so. False when WM_QUIT came
    /// (posted again for the loop outside).
    pub fn run_loop(until: Option<&dyn Fn() -> bool>) -> bool {
        // SAFETY: MSG is plain data filled by GetMessageW.
        let mut msg: MSG = unsafe { std::mem::zeroed() };
        loop {
            if let Some(done) = until {
                if done() {
                    return true;
                }
            }
            // SAFETY: plain calls on this thread's queue.
            unsafe {
                let r = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
                if r == 0 {
                    if until.is_some() {
                        PostQuitMessage(msg.wParam as i32);
                    }
                    return false;
                }
                if r == -1 {
                    return false;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    /// A nested loop waits on something: looked at again now.
    pub fn wake() {
        if let Some(h) = try_here() {
            // SAFETY: plain call; WM_NULL does nothing but return GetMessageW.
            unsafe { PostMessageW(h.invoker as HWND, WM_NULL, 0, 0) };
        }
    }

    /// Ends this thread's message loop (Application.ExitThread).
    pub fn quit() {
        // SAFETY: plain call on this thread.
        unsafe { PostQuitMessage(0) };
    }

    fn shut_down() {
        let Some(h) = try_here() else { return };
        // SAFETY: a WM_RUN still queued owns its box: taken back and dropped; then the invoker goes.
        unsafe {
            let mut msg: MSG = std::mem::zeroed();
            while PeekMessageW(&mut msg, h.invoker as HWND, WM_RUN, WM_RUN, PM_REMOVE) != 0 {
                drop(Box::from_raw(msg.lParam as *mut Box<dyn FnOnce() + Send>));
            }
            DestroyWindow(h.invoker as HWND);
        }
        let tasks: Vec<_> = h.tasks.borrow_mut().drain().collect();
        drop(tasks);
        h.timers.borrow_mut().clear();
        let _ = HOST.try_with(|x| *x.borrow_mut() = None);
    }

    // ───────── async on the window thread ─────────

    struct TaskWaker {
        invoker: isize,
        id: u64,
    }

    impl ArcWake for TaskWaker {
        fn wake_by_ref(me: &Arc<Self>) {
            // SAFETY: plain call; a window gone takes the message with it.
            unsafe { PostMessageW(me.invoker as HWND, WM_POLL, me.id as WPARAM, 0) };
        }
    }

    /// `_ = FooAsync()` on the window thread: runs at once up to its first wait, then goes on in the loop's turns.
    pub fn spawn(f: impl Future<Output = ()> + 'static) {
        let h = here();
        let id = h.next.get();
        h.next.set(id + 1);
        h.tasks.borrow_mut().insert(id, Some(Box::pin(f)));
        poll_task(&h, id);
    }

    fn poll_task(h: &Rc<ThreadHost>, id: u64) {
        let taken = {
            let mut tasks = h.tasks.borrow_mut();
            match tasks.get_mut(&id) {
                None => return,
                Some(slot) => slot.take(),
            }
        };
        let Some(mut fut) = taken else {
            // Being polled already (a nested loop under it): polled again once it has given back.
            h.rewake.borrow_mut().insert(id);
            return;
        };
        let w = waker(Arc::new(TaskWaker { invoker: h.invoker, id }));
        let mut cx = Context::from_waker(&w);
        // A task that panics ends there (shown as an unexpected error); the thread goes on.
        match crate::core::program::shielded(|| Poll::Ready(()), || fut.as_mut().poll(&mut cx)) {
            Poll::Ready(()) => {
                h.tasks.borrow_mut().remove(&id);
                h.rewake.borrow_mut().remove(&id);
            }
            Poll::Pending => {
                if let Some(slot) = h.tasks.borrow_mut().get_mut(&id) {
                    *slot = Some(fut);
                }
                if h.rewake.borrow_mut().remove(&id) {
                    w.wake_by_ref();
                }
            }
        }
    }

    struct YieldOnce(bool);

    impl Future for YieldOnce {
        type Output = ();
        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            if self.0 {
                return Poll::Ready(());
            }
            self.0 = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }

    /// BeginInvoke from the window thread itself: `f` runs on a later turn of the loop.
    pub fn defer(f: impl FnOnce() + 'static) {
        spawn(async move {
            YieldOnce(false).await;
            f();
        });
    }

    /// Slow work on a thread of its own; the window thread goes on meanwhile and takes its answer (None: that thread died).
    pub async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
        let (tx, rx) = oneshot::channel();
        let _ = crate::net::Named::new("form-work".into()).spawn(move || {
            let _ = tx.send(f());
        });
        rx.await.ok()
    }

    // ───────── System.Windows.Forms.Timer ─────────

    /// System.Windows.Forms.Timer: ticks on the window thread while enabled.
    pub struct Timer {
        id: usize,
        interval: Cell<u32>,
        enabled: Cell<bool>,
        disposed: Cell<bool>,
        host: Rc<ThreadHost>,
    }

    impl Timer {
        pub fn new(interval_ms: u32) -> Rc<Timer> {
            let host = here();
            let id = host.next.get() as usize;
            host.next.set(host.next.get() + 1);
            Rc::new(Timer { id, interval: Cell::new(interval_ms.max(1)), enabled: Cell::new(false), disposed: Cell::new(false), host })
        }

        /// Tick += f.
        pub fn on_tick(&self, f: impl Fn() + 'static) {
            self.host.timers.borrow_mut().insert(self.id, Rc::new(f));
        }

        pub fn interval(&self) -> u32 {
            self.interval.get()
        }

        /// Interval = ms: a running timer starts its wait again.
        pub fn set_interval(&self, ms: u32) {
            self.interval.set(ms.max(1));
            if self.enabled.get() {
                // SAFETY: plain call; the same id replaces the running timer.
                unsafe { SetTimer(self.host.invoker as HWND, self.id, self.interval.get(), None) };
            }
        }

        /// Start(): nothing when it runs already.
        pub fn start(&self) {
            if self.disposed.get() || self.enabled.get() {
                return;
            }
            self.enabled.set(true);
            // SAFETY: plain call on this thread's invoker.
            unsafe { SetTimer(self.host.invoker as HWND, self.id, self.interval.get(), None) };
        }

        pub fn stop(&self) {
            if self.enabled.replace(false) {
                // SAFETY: plain call.
                unsafe { KillTimer(self.host.invoker as HWND, self.id) };
            }
        }

        pub fn dispose(&self) {
            self.stop();
            if !self.disposed.replace(true) {
                self.host.timers.borrow_mut().remove(&self.id);
            }
        }
    }

    impl Drop for Timer {
        fn drop(&mut self) {
            self.dispose();
        }
    }

    /// `f` once, after `ms`, on this window thread.
    pub fn once(ms: u32, f: impl FnOnce() + 'static) {
        let host = here();
        let id = host.next.get() as usize;
        host.next.set(host.next.get() + 1);
        let slot = RefCell::new(Some(f));
        let invoker = host.invoker;
        let weak: Weak<ThreadHost> = Rc::downgrade(&host);
        host.timers.borrow_mut().insert(
            id,
            Rc::new(move || {
                // SAFETY: plain call; this timer fires once.
                unsafe { KillTimer(invoker as HWND, id) };
                if let Some(h) = weak.upgrade() {
                    h.timers.borrow_mut().remove(&id);
                }
                if let Some(f) = slot.borrow_mut().take() {
                    f();
                }
            }),
        );
        // SAFETY: plain call on this thread's invoker.
        unsafe { SetTimer(invoker as HWND, id, ms.max(1), None) };
    }

    /// Task.Delay(d) on the window thread.
    pub async fn delay(d: Duration) {
        let (tx, rx) = oneshot::channel::<()>();
        once(d.as_millis().min(u32::MAX as u128) as u32, move || {
            let _ = tx.send(());
        });
        let _ = rx.await;
    }

    // ───────── the App's callbacks, kept on the core's thread ─────────

    /// Keeps `f` (an App closure: not Send) on the core's thread; only from there. Its number reaches it from any thread.
    pub fn keep_on_core<T: 'static>(f: T) -> u64 {
        let id = NEXT_CORE_FN.fetch_add(1, Ordering::Relaxed);
        CORE_FNS.with(|m| m.borrow_mut().insert(id, Rc::new(f)));
        id
    }

    /// Runs `run` with that callback on the core's thread (rt::post), in its turn.
    pub fn call_on_core<T: 'static>(id: u64, run: impl FnOnce(&T) + Send + 'static) {
        rt::post(move || {
            let f = CORE_FNS.with(|m| m.borrow().get(&id).cloned());
            if let Some(f) = f {
                if let Some(t) = f.downcast_ref::<T>() {
                    run(t);
                }
            }
        });
    }

    /// The same, with an answer back to the calling (window) thread; None when the core gave none.
    pub async fn ask_core<T: 'static, R: Send + 'static>(id: u64, run: impl FnOnce(&T, oneshot::Sender<R>) + Send + 'static) -> Option<R> {
        let (tx, rx) = oneshot::channel();
        call_on_core::<T>(id, move |t| run(t, tx));
        rx.await.ok()
    }

    /// The callback is no longer needed.
    pub fn drop_on_core(id: u64) {
        rt::post(move || {
            CORE_FNS.with(|m| m.borrow_mut().remove(&id));
        });
    }

    // ───────── windows ─────────

    pub fn create_window(class: &str, title: &str, style: u32, ex_style: u32, parent: isize, bounds: Rect) -> isize {
        register_classes();
        let _ = here();
        let class = wide(class);
        let title = wide(title);
        // SAFETY: plain call; class registered above, strings zero-ended and alive for the call.
        unsafe {
            CreateWindowExW(
                ex_style,
                class.as_ptr(),
                title.as_ptr(),
                style,
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
                parent as HWND,
                std::ptr::null_mut(),
                instance(),
                std::ptr::null(),
            ) as isize
        }
    }

    /// The window's DPI (Control.DeviceDpi).
    pub fn dpi(hwnd: isize) -> u32 {
        // SAFETY: plain calls.
        let d = if hwnd != 0 { unsafe { GetDpiForWindow(hwnd as HWND) } } else { 0 };
        if d > 0 { d } else { unsafe { GetDpiForSystem() }.max(96) }
    }

    /// LogicalToDeviceUnits: a 96-DPI length at this DPI.
    pub fn logical(px: i32, dpi: u32) -> i32 {
        (px as f64 * dpi as f64 / 96.0).round() as i32
    }

    pub fn client_size(hwnd: isize) -> Size {
        let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        // SAFETY: plain call into a RECT on the stack.
        unsafe { GetClientRect(hwnd as HWND, &mut r) };
        Size::new(r.right - r.left, r.bottom - r.top)
    }

    pub fn window_rect(hwnd: isize) -> Rect {
        let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        // SAFETY: plain call into a RECT on the stack.
        unsafe { GetWindowRect(hwnd as HWND, &mut r) };
        Rect::from_win(&r)
    }

    pub fn set_bounds(hwnd: isize, r: Rect) {
        // SAFETY: plain call.
        unsafe { SetWindowPos(hwnd as HWND, std::ptr::null_mut(), r.x, r.y, r.width, r.height, SWP_NOZORDER | SWP_NOACTIVATE) };
    }

    pub fn set_text(hwnd: isize, text: &str) {
        let t = wide(text);
        // SAFETY: plain call with a zero-ended string.
        unsafe { SetWindowTextW(hwnd as HWND, t.as_ptr()) };
    }

    pub fn is_window(hwnd: isize) -> bool {
        // SAFETY: plain call.
        hwnd != 0 && unsafe { IsWindow(hwnd as HWND) } != 0
    }

    pub fn is_visible(hwnd: isize) -> bool {
        // SAFETY: plain call.
        unsafe { IsWindowVisible(hwnd as HWND) != 0 }
    }

    pub fn is_minimized(hwnd: isize) -> bool {
        // SAFETY: plain call.
        unsafe { IsIconic(hwnd as HWND) != 0 }
    }

    pub fn show(hwnd: isize, cmd: i32) {
        // SAFETY: plain call.
        unsafe { ShowWindow(hwnd as HWND, cmd) };
    }

    /// Control.FindForm: the top-level window a child is in.
    pub fn root(hwnd: isize) -> isize {
        // SAFETY: plain call.
        unsafe { GetAncestor(hwnd as HWND, GA_ROOT) as isize }
    }

    /// Form.Close(): WM_CLOSE in the loop's turn.
    pub fn close(hwnd: isize) {
        // SAFETY: plain call.
        unsafe { PostMessageW(hwnd as HWND, WM_CLOSE, 0, 0) };
    }

    /// Icon = Ui.AppIcon.
    pub fn set_icon(hwnd: isize) {
        if let Some(icon) = crate::core::ui::app_icon() {
            // SAFETY: plain calls; the icon is the app's, kept for its whole run.
            unsafe {
                SendMessageW(hwnd as HWND, WM_SETICON, ICON_BIG as WPARAM, icon);
                SendMessageW(hwnd as HWND, WM_SETICON, ICON_SMALL as WPARAM, crate::core::ui::app_icon_small().unwrap_or(icon));
            }
        }
    }

    pub fn colorref(c: Color) -> u32 {
        c.r as u32 | (c.g as u32) << 8 | (c.b as u32) << 16
    }

    /// BackColor: the window's face filled with it (WM_ERASEBKGND).
    pub fn fill_background(hwnd: HWND, hdc: WPARAM, color: Color) -> LRESULT {
        let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        // SAFETY: the DC is the one Windows passed for this erase; the brush is made and deleted here.
        unsafe {
            GetClientRect(hwnd, &mut r);
            let brush = CreateSolidBrush(colorref(color));
            FillRect(hdc as HDC, &r, brush);
            DeleteObject(brush as _);
        }
        1
    }

    /// The Keys value WinForms gave ProcessCmdKey: the key and the modifiers held.
    pub fn keys_now(vk: u32) -> u32 {
        // SAFETY: plain calls; the high bit says the key is down.
        let down = |k: u16| unsafe { GetKeyState(k as i32) } < 0;
        let mut keys = vk;
        if down(VK_SHIFT) {
            keys |= KEYS_SHIFT;
        }
        if down(VK_CONTROL) {
            keys |= KEYS_CONTROL;
        }
        if down(VK_MENU) {
            keys |= KEYS_ALT;
        }
        keys
    }

    pub const KEYS_SHIFT: u32 = 0x10000;
    pub const KEYS_CONTROL: u32 = 0x20000;
    pub const KEYS_ALT: u32 = 0x40000;

    // ───────── WebView2 ─────────

    /// WebView2RuntimeNotFoundException: no runtime (HRESULT_FROM_WIN32(ERROR_FILE_NOT_FOUND) from the loader).
    pub fn runtime_missing(e: &windows::core::Error) -> bool {
        e.code().0 as u32 == 0x8007_0002
    }

    /// CoreWebView2Environment.CreateAsync(null, folder, options).
    pub async fn environment(user_data: &str, options: Option<ICoreWebView2EnvironmentOptions>) -> WinResult<ICoreWebView2Environment> {
        let (tx, rx) = oneshot::channel();
        let handler = CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(move |result, env| {
            let _ = tx.send(result.and_then(|()| env.ok_or_else(|| windows::core::Error::from(E_POINTER))));
            Ok(())
        }));
        let folder = HSTRING::from(user_data);
        // SAFETY: plain call; the handler is called back on this (STA) thread through its message loop.
        unsafe { CreateCoreWebView2EnvironmentWithOptions(PCWSTR::null(), &folder, options.as_ref(), &handler)? };
        rx.await.unwrap_or_else(|_| Err(E_ABORT.into()))
    }

    /// A string property of WebView2 (an LPWSTR it allocated): "" when it couldn't be read.
    pub fn text(get: impl FnOnce(&mut PWSTR) -> WinResult<()>) -> String {
        let mut p = PWSTR::null();
        match get(&mut p) {
            Ok(()) => take_pwstr(p),
            Err(_) => String::new(),
        }
    }

    /// A BOOL property of WebView2: false when it couldn't be read.
    pub fn flag(get: impl FnOnce(&mut BOOL) -> WinResult<()>) -> bool {
        let mut b = BOOL::default();
        get(&mut b).is_ok() && b.as_bool()
    }

    pub fn color(c: Color) -> COREWEBVIEW2_COLOR {
        COREWEBVIEW2_COLOR { A: 255, R: c.r, G: c.g, B: c.b }
    }

    /// CookieManager.GetCookiesAsync(uri): each cookie's name and value.
    pub async fn cookies(core: &ICoreWebView2, uri: &str) -> WinResult<Vec<(String, String)>> {
        // SAFETY: COM calls on the thread that made the WebView.
        let manager = unsafe { core.cast::<ICoreWebView2_2>()?.CookieManager()? };
        let (tx, rx) = oneshot::channel();
        let handler = GetCookiesCompletedHandler::create(Box::new(move |result, list: Option<ICoreWebView2CookieList>| {
            let out = result.and_then(|()| {
                let mut found = Vec::new();
                if let Some(list) = list {
                    let mut count = 0u32;
                    // SAFETY: COM calls on the list WebView2 handed over.
                    unsafe {
                        list.Count(&mut count)?;
                        for i in 0..count {
                            let cookie = list.GetValueAtIndex(i)?;
                            found.push((text(|p| cookie.Name(p)), text(|p| cookie.Value(p))));
                        }
                    }
                }
                Ok(found)
            });
            let _ = tx.send(out);
            Ok(())
        }));
        let uri = HSTRING::from(uri);
        // SAFETY: plain COM call; the answer comes through the handler.
        unsafe { manager.GetCookies(&uri, &handler)? };
        rx.await.unwrap_or_else(|_| Err(E_ABORT.into()))
    }

    /// env.CreateWebResourceResponse(stream, status, reason, headers); no body when `body` is None.
    pub fn response(env: &ICoreWebView2Environment, body: Option<&[u8]>, status: i32, reason: &str, headers: &str) -> WinResult<ICoreWebView2WebResourceResponse> {
        // SAFETY: SHCreateMemStream copies the bytes; the COM call takes its own reference to the stream.
        unsafe {
            let stream: Option<IStream> = body.and_then(|b| SHCreateMemStream(Some(b)));
            env.CreateWebResourceResponse(stream.as_ref(), status, &HSTRING::from(reason), &HSTRING::from(headers))
        }
    }

    /// A request's body, read whole.
    pub fn read_stream(stream: &IStream) -> Vec<u8> {
        let mut out = Vec::new();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let mut read = 0u32;
            // SAFETY: `buf` holds `buf.len()` bytes; `read` says how many were written.
            let hr = unsafe { stream.Read(buf.as_mut_ptr() as *mut c_void, buf.len() as u32, Some(&mut read)) };
            if hr.is_err() || read == 0 {
                break;
            }
            out.extend_from_slice(&buf[..read as usize]);
        }
        out
    }

    /// The WebView2 WinForms control: a child window the browser draws into, the controller kept to its size.
    pub struct WebHost {
        pub hwnd: isize,
        controller: RefCell<Option<ICoreWebView2Controller>>,
        core: RefCell<Option<ICoreWebView2>>,
        background: Cell<Color>,
        disposed: Cell<bool>,
        keys: RefCell<Option<Rc<dyn Fn(u32) -> bool>>>,
    }

    impl WebHost {
        /// DefaultBackgroundColor = background; shown, at no size until placed.
        pub fn new(parent: isize, background: Color) -> Rc<WebHost> {
            let hwnd = create_window(CLASS_CHILD, "", WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN | WS_CLIPSIBLINGS, 0, parent, Rect::default());
            let me = Rc::new(WebHost {
                hwnd,
                controller: RefCell::new(None),
                core: RefCell::new(None),
                background: Cell::new(background),
                disposed: Cell::new(false),
                keys: RefCell::new(None),
            });
            let weak = Rc::downgrade(&me);
            set_handler(hwnd, move |h, msg, w, _l| {
                let me = weak.upgrade()?;
                match msg {
                    WM_SIZE => {
                        me.fit();
                        Some(0)
                    }
                    WM_ERASEBKGND => Some(fill_background(h, w, me.background.get())),
                    _ => None,
                }
            });
            me
        }

        pub fn core(&self) -> Option<ICoreWebView2> {
            self.core.borrow().clone()
        }

        pub fn controller(&self) -> Option<ICoreWebView2Controller> {
            self.controller.borrow().clone()
        }

        pub fn is_disposed(&self) -> bool {
            self.disposed.get()
        }

        /// The window's keys (ProcessCmdKey): handed what the browser was pressed with; true takes the key.
        pub fn on_keys(&self, f: Rc<dyn Fn(u32) -> bool>) {
            *self.keys.borrow_mut() = Some(f);
        }

        /// EnsureCoreWebView2Async(env[, options]).
        pub async fn ensure(self: &Rc<Self>, env: &ICoreWebView2Environment, options: Option<&ICoreWebView2ControllerOptions>) -> WinResult<ICoreWebView2> {
            if let Some(core) = self.core() {
                return Ok(core);
            }
            let (tx, rx) = oneshot::channel();
            let handler = CreateCoreWebView2ControllerCompletedHandler::create(Box::new(move |result, controller| {
                let _ = tx.send(result.and_then(|()| controller.ok_or_else(|| windows::core::Error::from(E_POINTER))));
                Ok(())
            }));
            let parent = windows::Win32::Foundation::HWND(self.hwnd as *mut c_void);
            // SAFETY: COM calls on this window thread; the answer comes through the handler.
            unsafe {
                match options {
                    Some(o) => env.cast::<ICoreWebView2Environment10>()?.CreateCoreWebView2ControllerWithOptions(parent, o, &handler)?,
                    None => env.CreateCoreWebView2Controller(parent, &handler)?,
                }
            }
            let controller = rx.await.unwrap_or_else(|_| Err(E_ABORT.into()))?;
            if self.disposed.get() {
                // SAFETY: plain COM call; the control went while the browser started (ObjectDisposedException).
                unsafe {
                    let _ = controller.Close();
                }
                return Err(E_ABORT.into());
            }
            // SAFETY: COM calls on this window thread.
            let core = unsafe { controller.CoreWebView2()? };
            if let Ok(c2) = controller.cast::<ICoreWebView2Controller2>() {
                // SAFETY: plain COM call.
                unsafe {
                    let _ = c2.SetDefaultBackgroundColor(color(self.background.get()));
                }
            }
            let weak = Rc::downgrade(self);
            let mut token = 0i64;
            // SAFETY: plain COM call; the handler runs on this thread.
            unsafe {
                let _ = controller.add_AcceleratorKeyPressed(
                    &AcceleratorKeyPressedEventHandler::create(Box::new(move |_, args| {
                        let (Some(me), Some(args)) = (weak.upgrade(), args) else { return Ok(()) };
                        let Some(keys) = me.keys.borrow().clone() else { return Ok(()) };
                        let mut kind = COREWEBVIEW2_KEY_EVENT_KIND::default();
                        args.KeyEventKind(&mut kind)?;
                        if kind != COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN && kind != COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN {
                            return Ok(());
                        }
                        let mut vk = 0u32;
                        args.VirtualKey(&mut vk)?;
                        if keys(keys_now(vk)) {
                            args.SetHandled(true)?;
                        }
                        Ok(())
                    })),
                    &mut token,
                );
            }
            *self.controller.borrow_mut() = Some(controller);
            *self.core.borrow_mut() = Some(core.clone());
            self.fit();
            self.sync_visible();
            Ok(core)
        }

        fn fit(&self) {
            let Some(c) = self.controller() else { return };
            let s = client_size(self.hwnd);
            let r = windows::Win32::Foundation::RECT { left: 0, top: 0, right: s.width, bottom: s.height };
            // SAFETY: plain COM calls.
            unsafe {
                let _ = c.SetBounds(r);
                let _ = c.NotifyParentWindowPositionChanged();
            }
        }

        fn sync_visible(&self) {
            if let Some(c) = self.controller() {
                // SAFETY: plain COM call.
                unsafe {
                    let _ = c.SetIsVisible(is_visible(self.hwnd));
                }
            }
        }

        pub fn bounds(&self) -> Rect {
            let parent_origin = {
                let mut p = windows_sys::Win32::Foundation::POINT { x: 0, y: 0 };
                // SAFETY: plain call mapping the child's top left into its parent.
                unsafe {
                    windows_sys::Win32::Graphics::Gdi::MapWindowPoints(
                        self.hwnd as HWND,
                        windows_sys::Win32::UI::WindowsAndMessaging::GetParent(self.hwnd as HWND),
                        &mut p,
                        1,
                    )
                };
                p
            };
            let s = client_size(self.hwnd);
            Rect::new(parent_origin.x, parent_origin.y, s.width, s.height)
        }

        pub fn set_bounds(&self, r: Rect) {
            if self.disposed.get() {
                return;
            }
            set_bounds(self.hwnd, r);
            self.fit();
        }

        pub fn visible(&self) -> bool {
            is_visible(self.hwnd)
        }

        pub fn set_visible(&self, on: bool) {
            if self.disposed.get() {
                return;
            }
            show(self.hwnd, if on { SW_SHOWNA } else { SW_HIDE });
            self.sync_visible();
        }

        /// Focus(): the page takes the keyboard.
        pub fn focus(&self) {
            if let Some(c) = self.controller() {
                // SAFETY: plain COM call.
                unsafe {
                    let _ = c.MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC);
                }
            }
        }

        /// BringToFront: above its siblings.
        pub fn bring_to_front(&self) {
            // SAFETY: plain call.
            unsafe { SetWindowPos(self.hwnd as HWND, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
        }

        /// Region: only these rectangles of it show (None: all of it).
        pub fn set_region(&self, parts: Option<&[Rect]>) {
            // SAFETY: the regions are made here; SetWindowRgn takes the final one over (Windows deletes it).
            unsafe {
                match parts {
                    None => {
                        SetWindowRgn(self.hwnd as HWND, std::ptr::null_mut(), 1);
                    }
                    Some(parts) => {
                        let all = CreateRectRgn(0, 0, 0, 0);
                        for p in parts {
                            let one = CreateRectRgn(p.x, p.y, p.right(), p.bottom());
                            CombineRgn(all, all, one, RGN_OR);
                            DeleteObject(one as _);
                        }
                        SetWindowRgn(self.hwnd as HWND, all, 1);
                    }
                }
            }
        }

        /// Dispose(): the browser closed, the window gone.
        pub fn dispose(&self) {
            if self.disposed.replace(true) {
                return;
            }
            *self.keys.borrow_mut() = None;
            self.core.borrow_mut().take();
            if let Some(c) = self.controller.borrow_mut().take() {
                // SAFETY: plain COM call.
                unsafe {
                    let _ = c.Close();
                }
            }
            // SAFETY: plain call; the window is this control's own.
            unsafe { DestroyWindow(self.hwnd as HWND) };
        }
    }
}

/// The top of Bloxkeep's browser windows (WebsiteForm, BrowserLoginForm): browser.html (ui/src/browser/BrowserChrome.tsx)
/// in a WebView2 of the app's own profile docked above the roblox.com one, in the app's theme (until 2026-11 WinForms
/// buttons and labels). It is another browser than the roblox.com pages' (their own throw-away profiles): it gets only
/// what it draws (names as the app shows them, the site's host name, the status words), never a cookie, a page of the
/// site or what is typed there, and it can only ask for the window's own buttons (select, close, back…), never send a
/// page anywhere.
pub struct ChromeView {
    form: isize,
    web: Rc<WebHost>,
    ready: Cell<bool>,
    css_height: Cell<i32>,
    last: RefCell<Option<Value>>,
    /// The open menu under the bar (its place in CSS pixels: x, y, w, h), or none.
    menu: Cell<Option<[f32; 4]>>,
    /// Set by a window that places the bar itself (WebsiteForm: its menu over the page): called when the bar's height
    /// or its menu changes, instead of the bar taking its height docked.
    relayout: RefCell<Option<Rc<dyn Fn()>>>,
    // PORT: WinForms laid a docked control out by itself when its Height changed; here the window that docks the bar
    // (BrowserLoginForm) gives its layout, called where the C# set the height.
    docked: RefCell<Option<Rc<dyn Fn()>>>,
    /// Something clicked on the page: its name and the rest of what it said.
    act: RefCell<Option<Rc<dyn Fn(&str, &Value)>>>,
}

impl ChromeView {
    /// `css_height`: its height until the page has said (CSS pixels), so the page below doesn't jump.
    pub fn new(form: isize, css_height: i32) -> Rc<ChromeView> {
        let web = WebHost::new(form, themes::current().background);
        let me = Rc::new(ChromeView {
            form,
            web,
            ready: Cell::new(false),
            css_height: Cell::new(css_height),
            last: RefCell::new(None),
            menu: Cell::new(None),
            relayout: RefCell::new(None),
            docked: RefCell::new(None),
            act: RefCell::new(None),
        });
        let width = host::client_size(form).width;
        me.web.set_bounds(Rect::new(0, 0, width, me.scaled(css_height)));
        me
    }

    pub fn set_relayout(&self, f: Rc<dyn Fn()>) {
        *self.relayout.borrow_mut() = Some(f);
    }

    pub fn set_docked(&self, f: Rc<dyn Fn()>) {
        *self.docked.borrow_mut() = Some(f);
    }

    /// Act += f.
    pub fn on_act(&self, f: Rc<dyn Fn(&str, &Value)>) {
        *self.act.borrow_mut() = Some(f);
    }

    /// The bar's own height, in device pixels.
    pub fn bar_pixels(&self) -> i32 {
        self.scaled(self.css_height.get())
    }

    /// The open menu's place on the bar's page, in device pixels; None when none is open.
    pub fn menu_pixels(&self) -> Option<Rect> {
        let [x, y, w, h] = self.menu.get()?;
        let s = host::dpi(self.form) as f32 / 96f32;
        Some(Rect::from_ltrb((x * s).floor() as i32, (y * s).floor() as i32, ((x + w) * s).ceil() as i32, ((y + h) * s).ceil() as i32))
    }

    pub fn control(&self) -> &Rc<WebHost> {
        &self.web
    }

    fn scaled(&self, css: i32) -> i32 {
        (css as f32 * host::dpi(self.form) as f32 / 96f32).ceil() as i32
    }

    fn fit(&self) {
        let relayout = self.relayout.borrow().clone();
        if let Some(relayout) = relayout {
            relayout();
        } else {
            let docked = self.docked.borrow().clone();
            if let Some(docked) = docked {
                docked();
            }
        }
    }

    /// Dragged onto a monitor of another scale: its height again from the page's, once Windows has rescaled the form.
    pub fn dpi_changed(self: &Rc<Self>) {
        let me = Rc::downgrade(self);
        defer(move || {
            if let Some(me) = me.upgrade() {
                if !me.web.is_disposed() {
                    me.fit();
                }
            }
        });
    }

    /// Starts the page (the window's handle must exist); false, logged, when it couldn't.
    pub async fn start(self: &Rc<Self>) -> bool {
        use webview2_com::{Microsoft::Web::WebView2::Win32::COREWEBVIEW2_PROCESS_FAILED_KIND, ProcessFailedEventHandler, WebMessageReceivedEventHandler};
        use windows::core::HSTRING;

        let env = match app_pages::create_environment().await {
            Ok(env) => env,
            Err(e) => {
                log::write(&e);
                return false;
            }
        };
        let core = match self.web.ensure(&env, None).await {
            Ok(core) => core,
            Err(e) => {
                log::write(&e);
                return false;
            }
        };
        if self.web.is_disposed() || !host::is_window(self.form) {
            return false;
        }
        if let Err(e) = app_pages::configure(&core, &env) {
            log::write(&e);
            return false;
        }
        let me = Rc::downgrade(self);
        let mut token = 0i64;
        // SAFETY: COM calls on this window thread; the handlers run here too.
        let added: host::WinResult<()> = unsafe {
            (|| {
                let m = me.clone();
                core.add_WebMessageReceived(
                    &WebMessageReceivedEventHandler::create(Box::new(move |_, args| {
                        let Some(args) = args else { return Ok(()) };
                        let json = host::text(|p| args.WebMessageAsJson(p));
                        let m = m.clone();
                        defer(move || {
                            if let Some(me) = m.upgrade() {
                                me.on_message(&json);
                            }
                        });
                        Ok(())
                    })),
                    &mut token,
                )?;
                // Its page gone (the renderer crashed or hung): loaded again. The browser gone: nothing is left to load it
                // in, and the window's buttons are this page, so the window closes. A GPU process restart: the page goes
                // on (ShellForm.FailureOf).
                let m = me.clone();
                core.add_ProcessFailed(
                    &ProcessFailedEventHandler::create(Box::new(move |_, args| {
                        let Some(args) = args else { return Ok(()) };
                        let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND::default();
                        args.ProcessFailedKind(&mut kind)?;
                        let failure = shell_form::failure_of(kind);
                        let Some(me) = m.upgrade() else { return Ok(()) };
                        if !matches!(failure, BrowserFailure::ReloadPage | BrowserFailure::BrowserGone) || !host::is_window(me.form) {
                            return Ok(());
                        }
                        let m = m.clone();
                        defer(move || {
                            let Some(me) = m.upgrade() else { return };
                            me.ready.set(false);
                            if !host::is_window(me.form) || me.web.is_disposed() {
                                return;
                            }
                            if failure == BrowserFailure::BrowserGone {
                                host::close(me.form);
                            } else if let Some(c) = me.web.core() {
                                let _ = c.Navigate(&HSTRING::from(format!("{}browser.html", app_pages::ORIGIN)));
                            }
                        });
                        Ok(())
                    })),
                    &mut token,
                )?;
                core.Navigate(&HSTRING::from(format!("{}browser.html", app_pages::ORIGIN)))?;
                Ok(())
            })()
        };
        match added {
            Ok(()) => true,
            Err(e) => {
                log::write(&e);
                false
            }
        }
    }

    /// What the page draws now (a "browser" event); kept for a page that loads (again) later.
    pub fn show(&self, data: Value) {
        *self.last.borrow_mut() = Some(data.clone());
        if self.ready.get() {
            self.post(&data);
        }
    }

    fn post(&self, data: &Value) {
        if self.web.is_disposed() {
            return;
        }
        let Some(core) = self.web.core() else { return };
        let message = json!({ "type": "event", "name": "browser", "data": data }).to_string();
        // SAFETY: plain COM call on this window thread.
        if let Err(e) = unsafe { core.PostWebMessageAsJson(&windows::core::HSTRING::from(message)) } {
            log::write(&e);
        }
    }

    fn on_message(&self, json: &str) {
        if self.web.is_disposed() {
            return;
        }
        let Ok(root) = serde_json::from_str::<Value>(json) else { return };
        let cmd = root.get("cmd").and_then(Value::as_str).unwrap_or("");
        let args = root.get("args").filter(|a| a.is_object());
        match (cmd, args) {
            ("ready", _) => {
                self.ready.set(true);
                let last = self.last.borrow().clone();
                if let Some(last) = last {
                    self.post(&last);
                }
            }
            ("size", Some(args)) => {
                if let Some(h) = args.get("h").and_then(Value::as_f64) {
                    let css = h.ceil() as i32;
                    if css > 0 && css < 400 {
                        self.css_height.set(css);
                        self.fit();
                    }
                }
            }
            // The page's menu opened (its place) or closed (none): the window lets it over the page meanwhile.
            ("menu", Some(args)) => {
                let menu = match (number(args, "x"), number(args, "y"), number(args, "w"), number(args, "h")) {
                    (Some(x), Some(y), Some(mw), Some(mh)) if mw > 0.0 && mh > 0.0 => Some([x as f32, y as f32, mw.min(2000.0) as f32, mh.min(2000.0) as f32]),
                    _ => None,
                };
                self.menu.set(menu);
                self.fit();
            }
            ("act", Some(args)) => {
                let what = args.get("what").and_then(Value::as_str).unwrap_or("").to_string();
                let act = self.act.borrow().clone();
                if let Some(act) = act {
                    act(&what, args);
                }
            }
            _ => {}
        }
    }
}

fn number(args: &Value, name: &str) -> Option<f64> {
    let d = args.get(name)?.as_f64()?;
    if d.is_finite() && d >= 0.0 { Some(d) } else { None }
}

/// The page's language and look, as every "browser" event carries them.
pub fn look() -> (String, Value, String) {
    (page_look::theme(), page_look::custom_theme(), l::language())
}
