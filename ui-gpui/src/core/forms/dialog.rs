// Dialog.cs: the app's own message box.
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM},
    Graphics::Dwm::DwmSetWindowAttribute,
    UI::{
        Controls::{
            TASKDIALOG_BUTTON, TASKDIALOGCONFIG, TDF_ALLOW_DIALOG_CANCELLATION, TDF_CALLBACK_TIMER, TDF_POSITION_RELATIVE_TO_WINDOW, TDF_SIZE_TO_CONTENT,
            TDF_USE_COMMAND_LINKS, TDM_CLICK_BUTTON, TDN_TIMER, TaskDialogIndirect,
        },
        Input::KeyboardAndMouse::{EnableWindow, GetAsyncKeyState, ReleaseCapture, SetActiveWindow, SetFocus, VK_LBUTTON},
        WindowsAndMessaging::{
            DestroyWindow, GetCursorPos, IDCANCEL, MA_ACTIVATE, PostMessageW, SW_HIDE, SW_SHOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SendMessageW,
            SetForegroundWindow, SetWindowPos, ShowWindowAsync, WM_ACTIVATE, WM_APP, WM_CLOSE, WM_DESTROY, WM_DPICHANGED, WM_ERASEBKGND, WM_MOUSEACTIVATE,
            WM_NCLBUTTONDOWN, WM_SIZE, WS_CLIPCHILDREN, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW, WS_POPUP,
        },
    },
};

use super::chrome_view::host::{self, Timer};
use crate::core::{
    l, log, themes,
    tool_page::{self, FromPage},
    ui,
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

pub fn error(owner: Option<isize>, text: &str) {
    show(owner, text, None, Kind::Error, &[Choice::new(l::t("OK"))], 0, None, None, None);
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
    // An error or a warning the person is shown is in error.log too (names scrubbed), whichever thread asks.
    if matches!(kind, Kind::Error | Kind::Warning) {
        let what = if matches!(kind, Kind::Error) { "Message box (error)" } else { "Message box (warning)" };
        crate::core::app::App::note_shown(what, &format!("{}{text}", heading.map(|h| format!("{h}: ")).unwrap_or_default()));
    }
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

/// On a window thread: the app's dialog, modal (a nested loop), or Windows' own when it can't be drawn.
fn ask_here(q: Question) -> (i32, bool) {
    // (No interface to draw it: the uninstaller's question, a start that failed before the window.)
    if !tool_page::interface_there() {
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
    dialog.dispose();
    drop(dialog);
    if failed {
        log::note(&format!(
            "Message box: {} ({} ms); Windows' dialog asked instead.",
            why.as_deref().unwrap_or("its face didn't start"),
            asked.elapsed().as_millis()
        ));
        let mut rest = q.clone();
        rest.timeout = remaining(q.timeout, asked.elapsed());
        return plain(&rest);
    }
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

/// The app's own message box: its window here, its face the interface's (ui-gpui/src/toolpage/dialog.rs, drawn in a
/// window of the interface's own put inside this one: core/tool_page.rs), in the app's theme. Buttons side by side, or
/// stacked when they are choices (`Choice::detail`). Enter presses the default; Esc, ✕, Alt+F4 and the timeout pick the
/// cancel one (the last by default). `ui::modal` shows it above the always-on-top windows.
///
/// It waits unseen (cloaked) until its face has drawn at the window's size, so it never flashes empty. Where it can't be
/// drawn (no interface: the uninstaller's question, a failed start; the interface not answering in time), Windows' own
/// task dialog asks the same question instead.
///
/// It runs on a thread of its own ("message box"), not the interface's: what the face says comes to it through a small
/// queue and a posted message (WM_FACE), and the face's window, a child of this one, is moved only with
/// SWP_ASYNCWINDOWPOS. Until 2026-11-19 its face was dialog.html in a WebView2 browser of its own.
struct DialogWindow {
    hwnd: isize,
    /// Shown with an owner: a tool window (not on the taskbar).
    #[allow(dead_code)]
    owned: bool,
    center: Point,
    data: Value,
    limit: Rc<Timer>,
    /// Drawn but not yet at the window's size after this long: shown anyway (logged).
    reveal_late: Rc<Timer>,
    picked: Cell<i32>,
    remembered: Cell<bool>,
    /// Sized and placed for its face (still cloaked until the face has drawn at that size).
    placed: Cell<bool>,
    shown: Cell<bool>,
    failed: Cell<bool>,
    /// Why the face gave up, for error.log.
    fail_why: RefCell<Option<String>>,
    /// The face: its id with the interface (0: none) and its window (0: not yet in this one).
    page_id: Cell<u64>,
    child: Cell<isize>,
    /// What the face said, waiting for this thread's turn.
    inbox: Arc<Mutex<Vec<FromPage>>>,
    /// The face's size in its own (CSS) pixels, as it last said.
    page: Cell<Size>,
    closed: Cell<bool>,
    /// The owner `show_dialog` disabled (0: none): enabled again before the window goes, so Windows gives it the focus
    /// back rather than another program.
    owner_off: Cell<isize>,
    disposed: Cell<bool>,
    /// The timeout's timer, started when the window shows (dialog.Shown += timer.Start).
    on_shown_extra: RefCell<Option<std::rc::Weak<Timer>>>,
}

/// Posted to the dialog's window when the face said something (the inbox has it).
const WM_FACE: u32 = WM_APP + 0x2B;

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
            "language": l::language(),
        });
        // Without an owner (the uninstaller's question) it is a window of its own on the taskbar, so it can't get lost.
        let ex_style = if owned { WS_EX_TOOLWINDOW } else { WS_EX_APPWINDOW };
        // Unseen and tiny until the face has said its size; there already, so it starts at that monitor's scale.
        let hwnd = host::create_window(host::CLASS_SHADOW, ui::TITLE, WS_POPUP | WS_CLIPCHILDREN, ex_style, owner.unwrap_or(0), Rect::new(center.x, center.y, 1, 1));
        let me = Rc::new(DialogWindow {
            hwnd,
            owned,
            center,
            data,
            limit: Timer::new(start_limit().as_millis().max(1) as u32),
            reveal_late: Timer::new(300),
            picked: Cell::new(q.cancel_index),
            remembered: Cell::new(false),
            placed: Cell::new(false),
            shown: Cell::new(false),
            failed: Cell::new(false),
            fail_why: RefCell::new(None),
            page_id: Cell::new(0),
            child: Cell::new(0),
            inbox: Arc::new(Mutex::new(Vec::new())),
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
                    me.fail(&format!("its face hadn't drawn after {:.0} s", start_limit().as_secs_f64()));
                }
            }
        });
        me.reveal_late.on_tick({
            let weak = weak.clone();
            move || {
                let Some(me) = weak.upgrade() else { return };
                me.reveal_late.stop();
                if me.placed.get() && !me.shown.get() {
                    log::note("Message box: its face hadn't drawn at the window's size after 300 ms; shown anyway.");
                    me.reveal();
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
                if self.child.get() != 0 {
                    tool_page::fit(self.hwnd, self.child.get(), true);
                }
                Some(0)
            }
            WM_FACE => {
                let said = std::mem::take(&mut *self.inbox.lock().unwrap_or_else(|e| e.into_inner()));
                for m in said {
                    self.from_face(m);
                }
                Some(0)
            }
            // The face's window (another thread's) is told it has the keyboard: its keys (Enter, Esc, Tab) are its own.
            WM_ACTIVATE => {
                let child = self.child.get();
                if child != 0 {
                    // SAFETY: plain calls on the face's window; posted, never waited for across threads.
                    unsafe {
                        PostMessageW(child as HWND, WM_ACTIVATE, w & 0xFFFF, 0);
                        if (w & 0xFFFF) != 0 {
                            SetFocus(child as HWND);
                        }
                    }
                }
                None
            }
            // A click on the face takes the keyboard.
            WM_MOUSEACTIVATE => Some(MA_ACTIVATE as LRESULT),
            // Dragged onto a monitor of another scale: Windows' suggested place is taken, then only the size is set
            // again from the face's size at the new scale (never the current size times the ratio).
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
                self.let_face_go();
                // SAFETY: plain call on this window.
                unsafe { DestroyWindow(h) };
                Some(0)
            }
            WM_DESTROY => {
                self.owner_back();
                self.closed.set(true);
                self.limit.stop();
                self.reveal_late.stop();
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

    /// Shown (cloaked): the interface is asked for its face, inside this window.
    fn on_shown(self: &Rc<Self>) {
        self.limit.start();
        let extra = self.on_shown_extra.borrow().as_ref().and_then(|w| w.upgrade());
        if let Some(timer) = extra {
            timer.start();
        }
        let (inbox, hwnd) = (self.inbox.clone(), self.hwnd);
        let id = tool_page::register(move |_, m| {
            inbox.lock().unwrap_or_else(|e| e.into_inner()).push(m);
            // SAFETY: plain call; a window gone meanwhile just doesn't get it (the inbox goes with the dialog).
            unsafe { PostMessageW(hwnd as HWND, WM_FACE, 0, 0) };
        });
        self.page_id.set(id);
        if !tool_page::open(id, "dialog", self.hwnd) {
            self.fail("the interface wasn't there");
        }
    }

    fn from_face(self: &Rc<Self>, m: FromPage) {
        if self.closed.get() {
            return;
        }
        match m {
            FromPage::Attached(child) => {
                self.child.set(child);
                tool_page::fit(self.hwnd, child, true);
                tool_page::event(self.page_id.get(), "dialog", self.data.clone());
            }
            FromPage::Message(root) => self.on_message(&root),
            FromPage::Gone => self.fail("its face went"),
        }
    }

    /// The face's messages: { cmd, args }.
    fn on_message(self: &Rc<Self>, root: &Value) {
        let cmd = root.get("cmd").and_then(Value::as_str).unwrap_or("");
        let args = root.get("args").filter(|a| a.is_object());
        match (cmd, args) {
            ("size", Some(args)) => {
                let (w, h, vw, vh, _) = tool_page::drawn(args);
                if w > 0 && h > 0 {
                    self.page.set(Size::new(w, h));
                    self.on_page_size(Size::new(w, h), vw, vh);
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

    /// The face has drawn: sized to it (never taller than the screen; the text then scrolls) and centred, still unseen;
    /// shown once it has drawn at that size.
    fn on_page_size(&self, css: Size, vw: i32, vh: i32) {
        if self.failed.get() || self.closed.get() {
            return;
        }
        let scale = host::dpi(self.hwnd) as f32 / 96.0;
        let work = screen_from_rect(Rect::new(self.center.x, self.center.y, 1, 1)).working_area;
        let mut size = scaled(css, scale);
        size.width = size.width.min(work.width - (16.0 * scale) as i32);
        size.height = size.height.min(work.height - (40.0 * scale) as i32);
        if !self.placed.get() {
            let at = ui::inside(Rect::new(self.center.x - size.width / 2, self.center.y - size.height / 2, size.width, size.height), work, 0);
            // The place first, then the size (TrayWindow, 2026-10-30: set together on a screen at another scale, Windows
            // resized it again by the ratio).
            // SAFETY: plain calls.
            unsafe {
                SetWindowPos(self.hwnd as HWND, std::ptr::null_mut(), at.x, at.y, 0, 0, SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOSIZE);
            }
            host::set_bounds(self.hwnd, Rect::new(at.x, at.y, size.width, size.height));
            self.placed.set(true);
            self.reveal_late.start();
        } else if host::window_rect(self.hwnd).size() != size {
            // SAFETY: plain call.
            unsafe { SetWindowPos(self.hwnd as HWND, std::ptr::null_mut(), 0, 0, size.width, size.height, SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOMOVE) };
        }
        // Drawn at the window's own size (in the face's pixels, a pixel either way): on screen.
        let client = host::client_size(self.hwnd);
        let (cw, ch) = ((client.width as f32 / scale).round() as i32, (client.height as f32 / scale).round() as i32);
        if !self.shown.get() && (cw - vw).abs() <= 1 && (ch - vh).abs() <= 1 {
            self.reveal();
        }
    }

    fn reveal(&self) {
        if self.shown.replace(true) {
            return;
        }
        self.limit.stop();
        self.reveal_late.stop();
        self.cloak(false);
        // SAFETY: plain calls (Activate()); the face's window gets the keyboard through WM_ACTIVATE.
        unsafe {
            SetForegroundWindow(self.hwnd as HWND);
            SetActiveWindow(self.hwnd as HWND);
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

    fn fail(&self, why: &str) {
        if self.failed.get() || self.closed.get() {
            return;
        }
        self.failed.set(true);
        *self.fail_why.borrow_mut() = Some(why.to_string());
        host::close(self.hwnd);
    }

    /// The face's title row pressed and moved (or its bottom grip): Windows moves (sizes) the window with the pointer,
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

    /// The face out of sight at once and closed by the interface on its own turn (before this window goes, so Windows
    /// doesn't destroy another thread's window with it).
    fn let_face_go(&self) {
        let id = self.page_id.replace(0);
        let child = self.child.replace(0);
        if child != 0 {
            // SAFETY: plain call on the face's window; not waited for across threads.
            unsafe { ShowWindowAsync(child as HWND, SW_HIDE) };
        }
        if id != 0 {
            tool_page::unregister(id);
            tool_page::close(id);
        }
    }

    fn dispose(&self) {
        if self.disposed.replace(true) {
            return;
        }
        self.limit.dispose();
        self.reveal_late.dispose();
        self.let_face_go();
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
