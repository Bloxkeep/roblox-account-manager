// Dialog.cs: the app's own message box.
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::{Duration, Instant},
};

use futures::{
    channel::oneshot,
    future::{Either, select},
};
use serde_json::{Value, json};
use webview2_com::{
    BrowserProcessExitedEventHandler,
    Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_PROCESS_FAILED_KIND, GetAvailableCoreWebView2BrowserVersionString, ICoreWebView2_2, ICoreWebView2Environment, ICoreWebView2Environment5,
    },
    NavigationCompletedEventHandler, ProcessFailedEventHandler, WebMessageReceivedEventHandler,
};
use windows::core::{HSTRING, Interface, PCWSTR, PWSTR};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM},
    Graphics::Dwm::DwmSetWindowAttribute,
    UI::{
        Controls::{
            TASKDIALOG_BUTTON, TASKDIALOGCONFIG, TDF_ALLOW_DIALOG_CANCELLATION, TDF_CALLBACK_TIMER, TDF_POSITION_RELATIVE_TO_WINDOW, TDF_SIZE_TO_CONTENT,
            TDF_USE_COMMAND_LINKS, TDM_CLICK_BUTTON, TDN_TIMER, TaskDialogIndirect,
        },
        Input::KeyboardAndMouse::{EnableWindow, GetAsyncKeyState, ReleaseCapture, SetActiveWindow, VK_LBUTTON},
        WindowsAndMessaging::{
            DestroyWindow, DispatchMessageW, EnumWindows, GetCursorPos, GetWindowThreadProcessId, IDCANCEL, IsWindowVisible, MA_ACTIVATE,
            MSG, PM_REMOVE, PeekMessageW, PostMessageW, SW_SHOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SendMessageW, SetForegroundWindow, SetWindowPos,
            TranslateMessage, WM_CLOSE, WM_DESTROY, WM_DPICHANGED, WM_ERASEBKGND, WM_MOUSEACTIVATE, WM_NCLBUTTONDOWN, WM_SIZE, WS_CLIPCHILDREN,
            WS_EX_APPWINDOW, WS_EX_TOOLWINDOW, WS_POPUP,
        },
    },
};

use super::chrome_view::{
    host::{self, Timer, WebHost},
    page_look,
};
use crate::core::{
    app_pages, l, log,
    shell_form::{self, BrowserFailure},
    themes, ui,
    win_kit::{Point, Rect, Size, screen_from_rect},
};

/// The icon of the question.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Info,
    Warning,
    Error,
}

/// A button: its text, an optional line under it (a choice), and whether it destroys something.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub text: String,
    pub detail: Option<String>,
    pub danger: bool,
}

impl Choice {
    pub fn new(text: impl Into<String>) -> Choice {
        Choice { text: text.into(), detail: None, danger: false }
    }
    pub fn with_detail(text: impl Into<String>, detail: impl Into<String>) -> Choice {
        Choice { text: text.into(), detail: Some(detail.into()), danger: false }
    }
    pub fn danger(text: impl Into<String>, danger: bool) -> Choice {
        Choice { text: text.into(), detail: None, danger }
    }
}

/// How long the page may take to draw before Windows' own dialog asks instead. Longer in the tests: GitHub's busy machine
/// took more than 6 s for a first browser (2026-11-07), and the fallback can't be answered there.
static START_LIMIT_MS: AtomicU64 = AtomicU64::new(6000);

pub fn start_limit() -> Duration {
    Duration::from_millis(START_LIMIT_MS.load(Ordering::Relaxed))
}

pub fn set_start_limit(limit: Duration) {
    START_LIMIT_MS.store(limit.as_millis() as u64, Ordering::Relaxed);
}

/// How long the browser is kept after the window hid (WebToolWindow.KeepBrowser): the dialog's goes with it.
pub const KEEP_BROWSER: Duration = Duration::from_secs(1);
/// The page's file in wwwroot.
pub const PAGE_FILE: &str = "dialog.html";
const DWMWA_CLOAK: u32 = 13;
const HTCAPTION: usize = 2;
const HTBOTTOM: usize = 15;

/// One question, as it travels to the thread that asks it.
#[derive(Clone, Debug)]
struct Question {
    owner: Option<isize>,
    text: String,
    heading: Option<String>,
    kind: Kind,
    choices: Vec<Choice>,
    default_index: i32,
    cancel_index: i32,
    footnote: Option<String>,
    timeout: Option<Duration>,
    remember: Option<String>,
}

// ───────────────────────── asking ─────────────────────────

/// The index of the button clicked; `cancel_index` (the last by default) for Esc, the X or the timeout.
#[allow(clippy::too_many_arguments)]
pub fn show(
    owner: Option<isize>,
    text: &str,
    heading: Option<&str>,
    kind: Kind,
    choices: &[Choice],
    default_index: i32,
    cancel_index: Option<i32>,
    footnote: Option<&str>,
    timeout: Option<Duration>,
) -> i32 {
    ask(question(owner, text, heading, kind, choices, default_index, cancel_index.unwrap_or(choices.len() as i32 - 1), footnote, timeout, None)).0
}

/// `show` from the core's thread: the question gets a window thread of its own and the core goes on meanwhile.
#[allow(clippy::too_many_arguments)]
pub async fn show_async(
    owner: Option<isize>,
    text: &str,
    heading: Option<&str>,
    kind: Kind,
    choices: &[Choice],
    default_index: i32,
    cancel_index: Option<i32>,
    footnote: Option<&str>,
    timeout: Option<Duration>,
) -> i32 {
    ask_async(question(owner, text, heading, kind, choices, default_index, cancel_index.unwrap_or(choices.len() as i32 - 1), footnote, timeout, None)).await.0
}

pub fn error(owner: Option<isize>, text: &str) {
    show(owner, text, None, Kind::Error, &[Choice::new(l::t("OK"))], 0, None, None, None);
}

/// `confirm` with a "Don't ask again" box; Remember is true only with a yes.
pub fn confirm_remember(owner: Option<isize>, text: &str, yes: &str, no: &str, danger: bool) -> (bool, bool) {
    let (picked, remember) = ask(question(
        owner,
        text,
        None,
        if danger { Kind::Warning } else { Kind::Info },
        &[Choice::danger(yes, danger), Choice::new(no)],
        if danger { 1 } else { 0 },
        1,
        None,
        None,
        Some(&l::t("Don't ask again")),
    ));
    (picked == 0, picked == 0 && remember)
}

/// Yes or no; with `danger` the "yes" is red and Enter means "no".
pub fn confirm(owner: Option<isize>, text: &str, yes: &str, no: &str, danger: bool) -> bool {
    show(owner, text, None, if danger { Kind::Warning } else { Kind::Info }, &[Choice::danger(yes, danger), Choice::new(no)], if danger { 1 } else { 0 }, None, None, None) == 0
}

#[allow(clippy::too_many_arguments)]
fn question(
    owner: Option<isize>,
    text: &str,
    heading: Option<&str>,
    kind: Kind,
    choices: &[Choice],
    default_index: i32,
    cancel_index: i32,
    footnote: Option<&str>,
    timeout: Option<Duration>,
    remember: Option<&str>,
) -> Question {
    Question {
        owner,
        text: text.to_string(),
        heading: heading.map(str::to_string),
        kind,
        choices: choices.to_vec(),
        default_index,
        cancel_index,
        footnote: footnote.map(str::to_string),
        timeout,
        remember: remember.map(str::to_string),
    }
}

fn ask(q: Question) -> (i32, bool) {
    // A page needs a window thread: asked from another one, the question goes to one of its own.
    // PORT: the C# handed it to the app's UI thread (Invoke); the app has no such thread now, so it gets its own.
    if !host::is_window_thread() {
        let (tx, rx) = std::sync::mpsc::channel();
        let asked = q.clone();
        let started = host::start_thread("message box", move || {
            let _ = tx.send(ask_here(asked));
            host::quit();
        });
        if started {
            // The windows thread keeps its messages going meanwhile: the dialog disables its owner (a message sent to
            // this thread), and a blocked wait there froze the app for good (closing's "Couldn't save your changes").
            let answer = if crate::core::win_form::is_windows_thread() { crate::core::win_form::wait_pumping(&rx) } else { rx.recv().ok() };
            if let Some(answer) = answer {
                return answer;
            }
        }
        return plain(&q);
    }
    ask_here(q)
}

async fn ask_async(q: Question) -> (i32, bool) {
    let (tx, rx) = oneshot::channel();
    let asked = q.clone();
    let started = host::start_thread("message box", move || {
        let _ = tx.send(ask_here(asked));
        host::quit();
    });
    match if started { rx.await.ok() } else { None } {
        Some(answer) => answer,
        None => plain(&q),
    }
}

/// On a window thread: the page's dialog, modal (a nested loop), or Windows' own when the page can't be shown.
fn ask_here(q: Question) -> (i32, bool) {
    if !web_view2_installed() {
        return plain(&q);
    }
    let asked = Instant::now();
    let dialog = DialogWindow::create(&q);
    if dialog.hwnd == 0 {
        return plain(&q);
    }
    let timer = Timer::new(1000);
    if let Some(after) = q.timeout {
        timer.set_interval(after.as_millis().clamp(1000, i32::MAX as u128) as u32);
        let weak = Rc::downgrade(&dialog);
        let timer_weak = Rc::downgrade(&timer);
        let cancel_index = q.cancel_index;
        timer.on_tick(move || {
            if let Some(t) = timer_weak.upgrade() {
                t.stop();
            }
            if let Some(d) = weak.upgrade() {
                d.picked.set(cancel_index);
                host::close(d.hwnd);
            }
        });
        *dialog.on_shown_extra.borrow_mut() = Some(Rc::downgrade(&timer));
    }
    dialog.show_dialog(q.owner);
    timer.dispose();
    let failed = dialog.failed.get();
    let why = dialog.fail_why.borrow().clone();
    let answer = (dialog.picked.get(), dialog.remembered.get());
    let env = dialog.env.borrow_mut().take();
    dialog.dispose();
    drop(dialog);
    if failed {
        log::note(&format!(
            "Message box: {} ({} ms); Windows' dialog asked instead.",
            why.as_deref().unwrap_or("its page didn't start"),
            asked.elapsed().as_millis()
        ));
        let mut rest = q.clone();
        rest.timeout = remaining(q.timeout, asked.elapsed());
        return plain(&rest);
    }
    wait_for_browser_exit(env);
    answer
}

/// The least the fallback dialog is left up, whatever the first wait used.
const FALLBACK_LEAST: Duration = Duration::from_secs(3);

/// What is left of `timeout` after the page's failed attempt took `elapsed`: the fallback dialog gets that, not the whole
/// timeout again (the uninstall question must be answered within the seconds Velopack allows the step; unanswered, it
/// keeps the data). None stays None: no timeout.
pub fn remaining(timeout: Option<Duration>, elapsed: Duration) -> Option<Duration> {
    timeout.map(|total| total.saturating_sub(elapsed).max(FALLBACK_LEAST))
}

static HAS_WEB_VIEW2: AtomicBool = AtomicBool::new(false);

fn web_view2_installed() -> bool {
    if HAS_WEB_VIEW2.load(Ordering::Relaxed) {
        return true;
    }
    let mut version = PWSTR::null();
    // SAFETY: plain call; the string it gives is freed by take_pwstr.
    let found = unsafe { GetAvailableCoreWebView2BrowserVersionString(PCWSTR::null(), &mut version) }.is_ok() && !webview2_com::take_pwstr(version).is_empty();
    HAS_WEB_VIEW2.store(found, Ordering::Relaxed);
    found
}

/// Asked with no other window of the app open (the uninstaller's question, a failed start): its browser is let finish
/// exiting, a moment at most, so what comes next (deleting the app's data) finds no file of it held open.
fn wait_for_browser_exit(env: Option<ICoreWebView2Environment>) {
    let Some(env) = env else { return };
    if app_windows_open() {
        return;
    }
    let exited = Rc::new(Cell::new(false));
    let flag = exited.clone();
    let mut token = 0i64;
    // SAFETY: COM calls on this window thread; the handler runs here, inside the loop below.
    let subscribed = unsafe {
        env.cast::<ICoreWebView2Environment5>().and_then(|env5| {
            env5.add_BrowserProcessExited(
                &BrowserProcessExitedEventHandler::create(Box::new(move |_, _| {
                    flag.set(true);
                    Ok(())
                })),
                &mut token,
            )
        })
    };
    if subscribed.is_err() {
        return;
    }
    let clock = Instant::now();
    while !exited.get() && clock.elapsed() < Duration::from_millis(3000) {
        // Application.DoEvents.
        // SAFETY: plain calls on this thread's queue.
        unsafe {
            let mut msg: MSG = std::mem::zeroed();
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Application.OpenForms.Count > 0.
// PORT: WinForms counted the app's forms; here any visible top-level window of this process counts (the main window
// is GPUI's), so a main window hidden in the tray no longer counts and the wait (3 s at most) runs then too.
fn app_windows_open() -> bool {
    unsafe extern "system" fn each(hwnd: HWND, found: LPARAM) -> i32 {
        let mut pid = 0u32;
        // SAFETY: plain calls; `found` is the bool below, alive for the whole enumeration.
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid == std::process::id() && IsWindowVisible(hwnd) != 0 {
                *(found as *mut bool) = true;
                return 0;
            }
        }
        1
    }
    let mut found = false;
    // SAFETY: the callback only writes `found`, which outlives the call.
    unsafe { EnumWindows(Some(each), &mut found as *mut bool as LPARAM) };
    found
}

/// The same question in Windows' own task dialog, for when the page can't be shown.
fn plain(q: &Question) -> (i32, bool) {
    struct TimerData {
        after_ms: u32,
        fired: Cell<bool>,
    }
    unsafe extern "system" fn callback(hwnd: HWND, msg: u32, w: WPARAM, _l: LPARAM, data: isize) -> windows_sys::core::HRESULT {
        if msg == TDN_TIMER as u32 && data != 0 {
            // SAFETY: `data` is the TimerData below, alive while the dialog is.
            let d = unsafe { &*(data as *const TimerData) };
            if w as u32 >= d.after_ms && !d.fired.replace(true) {
                // SAFETY: plain call; Cancel closes the dialog, as page.BoundDialog.Close() did.
                unsafe { PostMessageW(hwnd, TDM_CLICK_BUTTON as u32, IDCANCEL as WPARAM, 0) };
            }
        }
        0
    }

    let listed = q.choices.iter().any(|c| c.detail.is_some());
    let title = host_wide(ui::TITLE);
    let heading = q.heading.as_deref().map(host_wide);
    let text = host_wide(&q.text);
    let footnote = q.footnote.as_deref().map(host_wide);
    let remember = q.remember.as_deref().map(host_wide);
    let labels: Vec<Vec<u16>> = q
        .choices
        .iter()
        .map(|c| host_wide(&if listed { match &c.detail { Some(d) => format!("{}\n{}", c.text, d), None => c.text.clone() } } else { c.text.clone() }))
        .collect();
    let buttons: Vec<TASKDIALOG_BUTTON> = labels.iter().enumerate().map(|(i, t)| TASKDIALOG_BUTTON { nButtonID: 100 + i as i32, pszButtonText: t.as_ptr() }).collect();
    let n = buttons.len() as i32;
    let data = q.timeout.map(|after| TimerData { after_ms: after.as_millis().clamp(1000, i32::MAX as u128) as u32, fired: Cell::new(false) });
    let owner = q.owner.unwrap_or(0);

    // SAFETY: TASKDIALOGCONFIG is plain data; every string and the buttons outlive the call.
    let (result, picked_id, checked) = unsafe {
        let mut config: TASKDIALOGCONFIG = std::mem::zeroed();
        config.cbSize = std::mem::size_of::<TASKDIALOGCONFIG>() as u32;
        config.hwndParent = owner as HWND;
        let mut flags = TDF_ALLOW_DIALOG_CANCELLATION | TDF_SIZE_TO_CONTENT;
        if listed {
            flags |= TDF_USE_COMMAND_LINKS;
        }
        if owner != 0 {
            flags |= TDF_POSITION_RELATIVE_TO_WINDOW;
        }
        if data.is_some() {
            flags |= TDF_CALLBACK_TIMER;
        }
        config.dwFlags = flags;
        config.pszWindowTitle = title.as_ptr();
        // TD_ERROR_ICON, TD_WARNING_ICON, TD_INFORMATION_ICON (MAKEINTRESOURCE(-2, -1, -3)).
        config.Anonymous1.pszMainIcon = match q.kind {
            Kind::Error => 0xFFFEusize,
            Kind::Warning => 0xFFFFusize,
            Kind::Info => 0xFFFDusize,
        } as *const u16;
        config.pszMainInstruction = heading.as_ref().map(|h| h.as_ptr()).unwrap_or(std::ptr::null());
        config.pszContent = text.as_ptr();
        config.cButtons = buttons.len() as u32;
        config.pButtons = buttons.as_ptr();
        config.nDefaultButton = 100 + if n == 0 { 0 } else { q.default_index.clamp(0, n - 1) };
        config.pszVerificationText = remember.as_ref().map(|r| r.as_ptr()).unwrap_or(std::ptr::null());
        config.pszFooter = footnote.as_ref().map(|f| f.as_ptr()).unwrap_or(std::ptr::null());
        if let Some(d) = &data {
            config.pfCallback = Some(callback);
            config.lpCallbackData = d as *const TimerData as isize;
        }
        let mut picked = 0i32;
        let mut checked = 0i32;
        let hr = TaskDialogIndirect(&config, &mut picked, std::ptr::null_mut(), &mut checked);
        (hr, picked, checked != 0)
    };
    let picked = if result >= 0 && picked_id >= 100 && picked_id < 100 + n { picked_id - 100 } else { -1 };
    (if picked >= 0 { picked } else { q.cancel_index }, picked >= 0 && remember.is_some() && checked)
}

fn host_wide(s: &str) -> Vec<u16> {
    crate::core::win_kit::wide(s)
}

// ───────────────────────── the window ─────────────────────────

/// The app's own message box: dialog.html (ui/src/dialog/DialogView.tsx), the page's .modal in a small window of its own,
/// in the app's theme (until 2026-11 hand-drawn WinForms). Buttons side by side, or stacked when they are choices
/// (`Choice::detail`). Enter presses the default; Esc, ✕, Alt+F4 and the timeout pick the cancel one (the last by
/// default). `ui::modal` shows it above the always-on-top windows.
///
/// It waits unseen (cloaked) until its page has drawn, so it never flashes empty. Where the page can't start (no
/// WebView2 yet: the question that offers to install it; a browser that won't start), Windows' own task dialog asks the
/// same question instead.
// PORT: Dialog was a WebToolWindow (WebToolWindow.cs, not in this package); the parts of that base a dialog uses (the
// borderless tool window with Windows' shadow and rounded corners, the page of the app's profile, its messages, the
// drag and resize by the page, the scale change) are written out here.
struct DialogWindow {
    hwnd: isize,
    /// Shown with an owner: a tool window (not on the taskbar).
    #[allow(dead_code)]
    owned: bool,
    center: Point,
    data: Value,
    limit: Rc<Timer>,
    picked: Cell<i32>,
    remembered: Cell<bool>,
    shown: Cell<bool>,
    failed: Cell<bool>,
    /// Why the page gave up, for error.log.
    fail_why: RefCell<Option<String>>,
    env: RefCell<Option<ICoreWebView2Environment>>,
    web: RefCell<Option<Rc<WebHost>>>,
    ready: RefCell<Option<oneshot::Sender<()>>>,
    /// The page's size in its own (CSS) pixels, as it last said.
    page: Cell<Size>,
    closed: Cell<bool>,
    /// The owner `show_dialog` disabled (0: none): enabled again before the window goes, so Windows gives it the focus
    /// back rather than another program.
    owner_off: Cell<isize>,
    disposed: Cell<bool>,
    /// The timeout's timer, started when the window shows (dialog.Shown += timer.Start).
    on_shown_extra: RefCell<Option<std::rc::Weak<Timer>>>,
}

impl DialogWindow {
    fn create(q: &Question) -> Rc<DialogWindow> {
        let owner = q.owner.filter(|&o| o != 0).map(host::root);
        let owned = owner.is_some();
        let center = center_for(owner);
        let n = q.choices.len() as i32;
        let data = json!({
            "text": q.text,
            "heading": q.heading,
            "kind": match q.kind { Kind::Error => "error", Kind::Warning => "warning", Kind::Info => "info" },
            "choices": q.choices.iter().map(|c| json!({ "text": c.text, "detail": c.detail, "danger": c.danger })).collect::<Vec<_>>(),
            // PORT: Math.Clamp threw for an empty list of choices; 0 here.
            "def": if n == 0 { 0 } else { q.default_index.clamp(0, n - 1) },
            "cancel": q.cancel_index,
            "footnote": q.footnote,
            "remember": q.remember,
            "theme": page_look::theme(),
            "customTheme": page_look::custom_theme(),
            "language": l::language(),
        });
        // Without an owner (the uninstaller's question) it is a window of its own on the taskbar, so it can't get lost.
        let ex_style = if owned { WS_EX_TOOLWINDOW } else { WS_EX_APPWINDOW };
        // Unseen and tiny until the page has said its size; there already, so it starts at that monitor's scale.
        let hwnd = host::create_window(host::CLASS_SHADOW, ui::TITLE, WS_POPUP | WS_CLIPCHILDREN, ex_style, owner.unwrap_or(0), Rect::new(center.x, center.y, 1, 1));
        let me = Rc::new(DialogWindow {
            hwnd,
            owned,
            center,
            data,
            limit: Timer::new(start_limit().as_millis().max(1) as u32),
            picked: Cell::new(q.cancel_index),
            remembered: Cell::new(false),
            shown: Cell::new(false),
            failed: Cell::new(false),
            fail_why: RefCell::new(None),
            env: RefCell::new(None),
            web: RefCell::new(None),
            ready: RefCell::new(None),
            page: Cell::new(Size::default()),
            closed: Cell::new(false),
            owner_off: Cell::new(0),
            disposed: Cell::new(false),
            on_shown_extra: RefCell::new(None),
        });
        if hwnd == 0 {
            return me;
        }
        host::set_icon(hwnd);
        let weak = Rc::downgrade(&me);
        me.limit.on_tick({
            let weak = weak.clone();
            move || {
                let Some(me) = weak.upgrade() else { return };
                me.limit.stop();
                if !me.shown.get() {
                    me.fail(&format!("its page hadn't drawn after {:.0} s", start_limit().as_secs_f64()));
                }
            }
        });
        host::set_handler(hwnd, move |h, msg, w, l| {
            let me = weak.upgrade()?;
            me.on_window_message(h, msg, w, l)
        });
        // OnHandleCreated: cloaked; the frame (rounded corners, the theme's hairline).
        me.cloak(true);
        ui::round_corners(hwnd, &themes::current());
        me
    }

    fn on_window_message(self: &Rc<Self>, h: HWND, msg: u32, w: WPARAM, l: LPARAM) -> Option<LRESULT> {
        match msg {
            WM_ERASEBKGND => Some(host::fill_background(h, w, themes::current().surface)),
            WM_SIZE => {
                if let Some(web) = self.web.borrow().as_ref() {
                    let s = host::client_size(self.hwnd);
                    web.set_bounds(Rect::new(0, 0, s.width, s.height));
                }
                Some(0)
            }
            // A click on the page takes the keyboard (it needs it for typing, keys and Esc).
            WM_MOUSEACTIVATE => Some(MA_ACTIVATE as LRESULT),
            // Dragged onto a monitor of another scale: Windows' suggested place is taken, then only the size is set
            // again from the page's size at the new scale (never the current size times the ratio).
            WM_DPICHANGED => {
                // SAFETY: lparam is the RECT Windows suggests for the new scale.
                let r = unsafe { &*(l as *const windows_sys::Win32::Foundation::RECT) };
                // SAFETY: plain call.
                unsafe { SetWindowPos(h, std::ptr::null_mut(), r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOZORDER | SWP_NOACTIVATE) };
                let page = self.page.get();
                if page.width > 0 {
                    let new_dpi = (w & 0xFFFF) as f32;
                    let size = scaled(page, new_dpi / 96.0);
                    // SAFETY: plain call.
                    unsafe { SetWindowPos(h, std::ptr::null_mut(), 0, 0, size.width, size.height, SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOMOVE) };
                }
                Some(0)
            }
            WM_CLOSE => {
                self.owner_back();
                // SAFETY: plain call on this window.
                unsafe { DestroyWindow(h) };
                Some(0)
            }
            WM_DESTROY => {
                self.owner_back();
                self.closed.set(true);
                self.limit.stop();
                host::wake();
                Some(0)
            }
            _ => None,
        }
    }

    fn cloak(&self, on: bool) {
        let value: i32 = if on { 1 } else { 0 };
        // SAFETY: plain call with an int on the stack.
        unsafe { DwmSetWindowAttribute(self.hwnd as HWND, DWMWA_CLOAK as _, &value as *const i32 as *const _, 4) };
    }

    /// ShowDialog(owner): the owner disabled, shown (cloaked), a loop of its own until it closes.
    fn show_dialog(self: &Rc<Self>, owner: Option<isize>) {
        let owner = owner.filter(|&o| o != 0).map(host::root);
        if let Some(o) = owner {
            // SAFETY: plain call; enabled again as the window goes (owner_back).
            unsafe { EnableWindow(o as HWND, 0) };
            self.owner_off.set(o);
        }
        host::show(self.hwnd, SW_SHOW);
        self.on_shown();
        let me = self.clone();
        host::run_loop(Some(&move || me.closed.get()));
        self.owner_back();
    }

    fn owner_back(&self) {
        let o = self.owner_off.replace(0);
        if o != 0 {
            // SAFETY: plain call.
            unsafe { EnableWindow(o as HWND, 1) };
        }
    }

    fn on_shown(self: &Rc<Self>) {
        self.limit.start();
        let extra = self.on_shown_extra.borrow().as_ref().and_then(|w| w.upgrade());
        if let Some(timer) = extra {
            timer.start();
        }
        let me = self.clone();
        host::spawn(async move {
            if !me.ensure_browser().await || me.closed.get() {
                me.fail("its browser didn't start");
                return;
            }
            let env = me.web.borrow().as_ref().and_then(|w| w.core()).and_then(|core| {
                // SAFETY: plain COM call on this window thread.
                unsafe { core.cast::<ICoreWebView2_2>().and_then(|c| c.Environment()).ok() }
            });
            *me.env.borrow_mut() = env;
            let data = me.data.clone();
            me.send_event("dialog", &data);
        });
    }

    /// Starts the page; false when it couldn't (logged).
    async fn ensure_browser(self: &Rc<Self>) -> bool {
        let env = match app_pages::create_environment().await {
            Ok(env) => env,
            Err(e) => {
                log::write(&e);
                return false;
            }
        };
        if self.closed.get() {
            return false;
        }
        let web = WebHost::new(self.hwnd, themes::current().surface);
        let s = host::client_size(self.hwnd);
        web.set_bounds(Rect::new(0, 0, s.width, s.height));
        *self.web.borrow_mut() = Some(web.clone());
        let core = match web.ensure(&env, None).await {
            Ok(core) => core,
            Err(e) => {
                log::write(&e);
                self.release_browser();
                return false;
            }
        };
        if self.closed.get() || web.is_disposed() {
            return false;
        }
        if let Err(e) = app_pages::configure(&core, &env) {
            log::write(&e);
            self.release_browser();
            return false;
        }
        let (tx, rx) = oneshot::channel::<()>();
        *self.ready.borrow_mut() = Some(tx);
        let weak = Rc::downgrade(self);
        let mut token = 0i64;
        // SAFETY: COM calls on this window thread; the handlers run here too.
        let added: host::WinResult<()> = unsafe {
            (|| {
                // Loaded: its first "ready" can come before the page is listened to, so the load itself counts as ready too.
                let w = weak.clone();
                core.add_NavigationCompleted(
                    &NavigationCompletedEventHandler::create(Box::new(move |_, args| {
                        let (Some(me), Some(args)) = (w.upgrade(), args) else { return Ok(()) };
                        if host::flag(|b| args.IsSuccess(b)) {
                            if let Some(tx) = me.ready.borrow_mut().take() {
                                let _ = tx.send(());
                            }
                        }
                        Ok(())
                    })),
                    &mut token,
                )?;
                // Its page gone (the renderer crashed or hung, the browser gone). Not for a GPU process restart, which the
                // page lives through (ShellForm.FailureOf).
                let w = weak.clone();
                let this_web = Rc::downgrade(&web);
                core.add_ProcessFailed(
                    &ProcessFailedEventHandler::create(Box::new(move |_, args| {
                        let Some(args) = args else { return Ok(()) };
                        let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND::default();
                        args.ProcessFailedKind(&mut kind)?;
                        if matches!(shell_form::failure_of(kind), BrowserFailure::ReloadPage | BrowserFailure::BrowserGone) {
                            let (w, this_web) = (w.clone(), this_web.clone());
                            host::defer(move || {
                                let Some(me) = w.upgrade() else { return };
                                let same = matches!((me.web.borrow().as_ref(), this_web.upgrade()), (Some(a), Some(b)) if Rc::ptr_eq(a, &b));
                                if same && !me.closed.get() {
                                    me.on_browser_failed();
                                }
                            });
                        }
                        Ok(())
                    })),
                    &mut token,
                )?;
                let w = weak.clone();
                core.add_WebMessageReceived(
                    &WebMessageReceivedEventHandler::create(Box::new(move |_, args| {
                        let Some(args) = args else { return Ok(()) };
                        let json = host::text(|p| args.WebMessageAsJson(p));
                        let w = w.clone();
                        host::defer(move || {
                            if let Some(me) = w.upgrade() {
                                me.on_message(&json);
                            }
                        });
                        Ok(())
                    })),
                    &mut token,
                )?;
                core.Navigate(&HSTRING::from(format!("{}{}", app_pages::ORIGIN, PAGE_FILE)))?;
                Ok(())
            })()
        };
        if let Err(e) = added {
            log::write(&e);
            self.release_browser();
            return false;
        }
        match select(rx, Box::pin(host::delay(Duration::from_secs(15)))).await {
            Either::Left((Ok(()), _)) => true,
            _ => {
                log::note("Dialog: its page started but never said it was ready; the window stays hidden.");
                self.release_browser();
                false
            }
        }
    }

    fn release_browser(&self) {
        self.ready.borrow_mut().take();
        if let Some(web) = self.web.borrow_mut().take() {
            web.dispose();
        }
    }

    /// An event to the page, as the app's page gets them: { type: "event", name, data }.
    fn send_event(&self, name: &str, data: &Value) {
        let core = self.web.borrow().as_ref().and_then(|w| w.core());
        let Some(core) = core else { return };
        let message = json!({ "type": "event", "name": name, "data": data }).to_string();
        // SAFETY: plain COM call on this window thread.
        if let Err(e) = unsafe { core.PostWebMessageAsJson(&HSTRING::from(message)) } {
            log::write(&e);
        }
    }

    /// The page's messages: { id, cmd, args }.
    fn on_message(self: &Rc<Self>, json: &str) {
        if self.closed.get() {
            return;
        }
        let Ok(root) = serde_json::from_str::<Value>(json) else {
            log::note("Dialog: a message from its page couldn't be read.");
            return;
        };
        let cmd = root.get("cmd").and_then(Value::as_str).unwrap_or("");
        let args = root.get("args").filter(|a| a.is_object());
        match (cmd, args) {
            ("ready", _) => {
                if let Some(tx) = self.ready.borrow_mut().take() {
                    let _ = tx.send(());
                }
                self.on_page_ready();
            }
            ("size", Some(args)) => {
                let (w, h) = (num(args, "w"), num(args, "h"));
                if w > 0 && h > 0 {
                    self.page.set(Size::new(w, h));
                    self.on_page_size(Size::new(w, h));
                }
            }
            ("drag", _) => self.begin_drag(HTCAPTION),
            ("resize", _) => self.begin_drag(HTBOTTOM),
            ("act", Some(args)) => {
                let what = args.get("what").and_then(Value::as_str).unwrap_or("").to_string();
                self.on_act(&what, args);
            }
            _ => {}
        }
    }

    fn on_page_ready(&self) {
        if !self.closed.get() && !self.failed.get() {
            self.send_event("dialog", &self.data);
        }
    }

    /// The page has drawn: sized to it (never taller than the screen; the text then scrolls), centred, shown.
    fn on_page_size(&self, css: Size) {
        if self.failed.get() || self.closed.get() {
            return;
        }
        let scale = host::dpi(self.hwnd) as f32 / 96.0;
        let work = screen_from_rect(Rect::new(self.center.x, self.center.y, 1, 1)).working_area;
        let mut size = scaled(css, scale);
        size.width = size.width.min(work.width - (16.0 * scale) as i32);
        size.height = size.height.min(work.height - (40.0 * scale) as i32);
        if self.shown.get() {
            if host::window_rect(self.hwnd).size() != size {
                // SAFETY: plain call.
                unsafe { SetWindowPos(self.hwnd as HWND, std::ptr::null_mut(), 0, 0, size.width, size.height, SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOMOVE) };
            }
            return;
        }
        let at = ui::inside(Rect::new(self.center.x - size.width / 2, self.center.y - size.height / 2, size.width, size.height), work, 0);
        // The place first, then the size (TrayWindow, 2026-10-30: set together on a screen at another scale, Windows
        // resized it again by the ratio).
        // SAFETY: plain calls.
        unsafe {
            SetWindowPos(self.hwnd as HWND, std::ptr::null_mut(), at.x, at.y, 0, 0, SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOSIZE);
        }
        host::set_bounds(self.hwnd, Rect::new(at.x, at.y, size.width, size.height));
        self.shown.set(true);
        self.limit.stop();
        self.cloak(false);
        // SAFETY: plain calls (Activate()).
        unsafe {
            SetForegroundWindow(self.hwnd as HWND);
            SetActiveWindow(self.hwnd as HWND);
        }
        if let Some(web) = self.web.borrow().as_ref() {
            web.focus();
        }
    }

    fn on_act(&self, what: &str, args: &Value) {
        match what {
            "pick" => {
                let index = args.get("index").and_then(Value::as_f64).map(|i| i as i32).unwrap_or(-1);
                if index < 0 {
                    return;
                }
                self.picked.set(index);
                self.remembered.set(args.get("remember") == Some(&Value::Bool(true)));
                host::close(self.hwnd);
            }
            "cancel" => host::close(self.hwnd),
            _ => {}
        }
    }

    /// Its page crashed: Windows' own dialog asks the question instead.
    fn on_browser_failed(&self) {
        self.fail("its page crashed");
    }

    fn fail(&self, why: &str) {
        if self.failed.get() || self.closed.get() {
            return;
        }
        self.failed.set(true);
        *self.fail_why.borrow_mut() = Some(why.to_string());
        host::close(self.hwnd);
    }

    /// The page's title row pressed and moved (or its bottom grip): Windows moves (sizes) the window with the pointer,
    /// as by a title bar (a sizing border).
    fn begin_drag(&self, hit: usize) {
        // SAFETY: plain calls; the high bit says the button is down.
        unsafe {
            if !host::is_visible(self.hwnd) || GetAsyncKeyState(VK_LBUTTON as i32) >= 0 {
                return;
            }
            ReleaseCapture();
            SendMessageW(self.hwnd as HWND, WM_NCLBUTTONDOWN, hit, 0);
        }
    }

    fn dispose(&self) {
        if self.disposed.replace(true) {
            return;
        }
        self.limit.dispose();
        self.release_browser();
        if host::is_window(self.hwnd) {
            self.owner_back();
            // SAFETY: plain call on this window.
            unsafe { DestroyWindow(self.hwnd as HWND) };
        }
    }
}

/// Over the owner's window when it is on screen, else in the middle of the screen the pointer is on.
fn center_for(owner: Option<isize>) -> Point {
    if let Some(o) = owner {
        if host::is_window(o) && host::is_visible(o) && !host::is_minimized(o) {
            let r = host::window_rect(o);
            return Point::new(r.x + r.width / 2, r.y + r.height / 2);
        }
    }
    let mut p = POINT { x: 0, y: 0 };
    // SAFETY: plain call into a POINT on the stack.
    unsafe { GetCursorPos(&mut p) };
    let work = screen_from_rect(Rect::new(p.x, p.y, 1, 1)).working_area;
    Point::new(work.x + work.width / 2, work.y + work.height / 2)
}

/// A size in CSS pixels as screen pixels on a monitor of this scale.
fn scaled(css: Size, scale: f32) -> Size {
    Size::new((css.width as f32 * scale).ceil() as i32, (css.height as f32 * scale).ceil() as i32)
}

fn num(o: &Value, name: &str) -> i32 {
    o.get(name).and_then(Value::as_f64).map(|v| v.ceil() as i32).unwrap_or(0)
}
