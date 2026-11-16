// ShellForm.cs: the app window. In C# it hosted the HTML interface in WebView2 (and, behind a switch, the GPUI one as a
// child); here the interface is the GPUI window itself, and the shell is what the C# form did around the page: where the
// window goes and is remembered, its entrance (stand-in, fade, start logo), minimising and closing to the tray (the tray
// icon, its panel and notifications), the global hotkeys, Windows' theme changes, the mini window and the monitor tab's
// pictures, and what the controller asks of the window. With "keep running when closed" on, closing hides it to the tray
// so auto-rejoin and Multi-Roblox keep working.
//
// The shell takes over the interface's window when it exists (`attach`, from gpui_host::window_made): its messages are
// seen first (a subclass), on the interface's thread, which is the windows thread. The App asks things of it through the
// functions `wire` gives it (they hop to the windows thread); what the shell asks of the App goes through rt::post.
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicIsize, Ordering},
    },
    time::Duration,
};

use futures::{FutureExt, channel::oneshot, future::LocalBoxFuture};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_PROCESS_FAILED_KIND, COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED, COREWEBVIEW2_PROCESS_FAILED_KIND_GPU_PROCESS_EXITED,
    COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED, COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_UNRESPONSIVE,
};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::{
        Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey},
        Shell::{
            DefSubclassProc, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_ERROR, NIIF_NONE, NIIF_WARNING, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
            RemoveWindowSubclass, SetWindowSubclass, Shell_NotifyIconW,
        },
        WindowsAndMessaging::*,
    },
};

use super::{
    app::{App, hotkeys, macros, monitor::MonitorArea},
    base::Guid,
    clock, l, log, monitor_tab::MonitorTab,
    mini_window::MiniWindow,
    prefs, relaunch, roblox_processes, rt,
    start_cover::StartCover,
    themes,
    tray_window::TrayWindow,
    ui, vault,
    win_form::{self, Timer},
    win_kit::{Point, Rect, Size, all_screens, hwnd, screen_from_handle, screen_from_rect, wide},
    window_opening::WindowOpening,
};

/// What a page's window does when one of its browser's processes fails (`failure_of`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowserFailure {
    /// Nothing: the page goes on (a helper process the pages don't need).
    None,
    /// The page's renderer is gone or hung: the page is loaded again.
    ReloadPage,
    /// The GPU process exited; Chromium starts it again by itself and the pages go on.
    GpuRestarted,
    /// The whole browser is gone: the page needs a new one.
    BrowserGone,
}

/// What a browser process failing means for a page, the one rule for every window of the app's pages (the page windows,
/// the tray, the browser windows' top). Every page of the browser hears every failure, a GPU process exit too: Chromium
/// starts that one again by itself and the pages go on, so it is no reason to load a page again.
pub fn failure_of(kind: COREWEBVIEW2_PROCESS_FAILED_KIND) -> BrowserFailure {
    if kind == COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED || kind == COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_UNRESPONSIVE {
        BrowserFailure::ReloadPage
    } else if kind == COREWEBVIEW2_PROCESS_FAILED_KIND_GPU_PROCESS_EXITED {
        BrowserFailure::GpuRestarted
    } else if kind == COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED {
        BrowserFailure::BrowserGone
    } else {
        BrowserFailure::None
    }
}

const BALLOON_MS: u32 = 8000;
const WM_TRAY: u32 = WM_APP + 0x41;
const TRAY_ID: u32 = 1;
const VK_H: u32 = 0x48;
const VK_A: u32 = 0x41;
const VK_M: u32 = 0x4D;
const VK_N: u32 = 0x4E;
const VK_B: u32 = 0x42;
const VK_D0: u32 = 0x30;
const VK_F8: u32 = 0x77;
const MOD_ALT: u32 = 0x1;
const MOD_CONTROL: u32 = 0x2;
const MOD_NOREPEAT: u32 = 0x4000;
const SUBCLASS_ID: usize = 0xB10C;

/// The hotkeys (see app::hotkeys), each with its key after Ctrl+Alt.
const HOTKEYS: [(i32, u32); 5] =
    [(hotkeys::HOTKEY_HIDE, VK_H), (hotkeys::HOTKEY_ARRANGE, VK_A), (hotkeys::HOTKEY_MINIMIZE, VK_M), (hotkeys::HOTKEY_NEXT, VK_N), (hotkeys::HOTKEY_BLOXKEEP, VK_B)];

fn start_marker() -> std::path::PathBuf {
    vault::data_dir().join("starting.flag")
}

// ───────── reaching the shell ─────────

thread_local! {
    static SHELL: RefCell<Option<Rc<ShellForm>>> = const { RefCell::new(None) };
}

/// The shell's window, for questions from any thread (0: not attached yet, or closed).
static SHELL_HWND: AtomicIsize = AtomicIsize::new(0);
/// The mini window shows (for WindowNow and SetMiniOpen from the core's thread).
static MINI_SHOWN: AtomicBool = AtomicBool::new(false);
static QUITTING: AtomicBool = AtomicBool::new(false);
/// How the next run starts (the C# constructor's arguments), given by program::main_core before the interface runs.
static PREPARED: Mutex<Option<(Option<relaunch::State>, bool)>> = Mutex::new(None);
/// What the last run handed over, for App::resume once App::init has run (the C# constructed the controller first).
static RESUME_AFTER_INIT: Mutex<Option<relaunch::State>> = Mutex::new(None);
/// With the C#'s GPUI host the window had no title bar: its top bar moved the window. Set when the interface's window
/// keeps Windows' frame and wants only its title bar gone (`WM_NCCALCSIZE`); off by default, as GPUI draws its own.
pub static CUSTOM_FRAME: AtomicBool = AtomicBool::new(false);

/// What the shell starts with: what the last run handed over when it quit for an update in the background (relaunch),
/// and whether Windows started it at sign-in (start_with_windows: in the tray, locked).
pub fn prepare(resume: Option<relaunch::State>, startup: bool) {
    *PREPARED.lock().unwrap_or_else(|e| e.into_inner()) = Some((resume, startup));
}

/// Runs `f` on the windows thread with the shell (BeginInvoke); dropped when there is no shell.
pub fn with(f: impl FnOnce(&Rc<ShellForm>) + Send + 'static) {
    win_form::invoke(move || {
        let shell = SHELL.with(|s| s.borrow().clone());
        if let Some(shell) = shell {
            f(&shell);
        }
    });
}

/// The shell took over the interface's window.
pub fn is_attached() -> bool {
    SHELL_HWND.load(Ordering::SeqCst) != 0
}

/// Asks the App something and waits for its answer (at most `wait`): the C# called the controller on the same thread.
/// Err(true) when the wait ran out (the core is busy, not failed); Err(false) when no answer will come.
fn ask_app<T: Send + 'static>(wait: Duration, f: impl FnOnce(&Rc<App>) -> T + Send + 'static) -> Result<T, bool> {
    let (tx, rx) = std::sync::mpsc::channel();
    rt::post(move || {
        App::with(|app| {
            let _ = tx.send(f(app));
        })
    });
    rx.recv_timeout(wait).map_err(|e| matches!(e, std::sync::mpsc::RecvTimeoutError::Timeout))
}

// ───────── the tray icon (WinForms' NotifyIcon) ─────────

struct TrayIcon {
    window: isize,
    visible: Cell<bool>,
    created_msg: u32,
}

impl TrayIcon {
    fn data(&self) -> NOTIFYICONDATAW {
        // SAFETY: plain data; the size is set as Windows requires.
        let mut d: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        d.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        d.hWnd = hwnd(self.window);
        d.uID = TRAY_ID;
        d
    }

    fn add(&self) {
        let mut d = self.data();
        d.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        d.uCallbackMessage = WM_TRAY;
        d.hIcon = ui::app_icon_small().unwrap_or(0) as _;
        let tip = wide(ui::TITLE);
        d.szTip[..tip.len()].copy_from_slice(&tip);
        // SAFETY: the structure lives for the call.
        unsafe { Shell_NotifyIconW(NIM_ADD, &d) };
    }

    fn set_visible(&self, on: bool) {
        if on == self.visible.get() {
            return;
        }
        self.visible.set(on);
        if on {
            self.add();
        } else {
            let d = self.data();
            // SAFETY: as above.
            unsafe { Shell_NotifyIconW(NIM_DELETE, &d) };
        }
    }

    /// ShowBalloonTip: a Windows notification (shown as a toast).
    fn show_balloon(&self, title: &str, text: &str, icon: u32) {
        let mut d = self.data();
        d.uFlags = NIF_INFO;
        d.dwInfoFlags = icon;
        let t = wide(title);
        let x = wide(text);
        let tn = t.len().min(d.szInfoTitle.len());
        d.szInfoTitle[..tn].copy_from_slice(&t[..tn]);
        let xn = x.len().min(d.szInfo.len());
        d.szInfo[..xn].copy_from_slice(&x[..xn]);
        if let Some(last) = d.szInfo.last_mut() {
            *last = 0;
        }
        // SAFETY: as above.
        unsafe { Shell_NotifyIconW(NIM_MODIFY, &d) };
    }
}

// ───────── the shell ─────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WindowState {
    Normal,
    Minimized,
    Maximized,
}

pub struct ShellForm {
    handle: isize,
    tray: TrayIcon,
    tray_window: Rc<TrayWindow>,
    /// Hides the tray icon again once a notification shown with close-to-tray off is surely gone.
    balloon_timer: Timer,
    /// Until when the newest notification balloon may be on screen (clock::tick); the icon isn't hidden before that.
    balloon_until: Cell<i64>,
    cover_timer: Timer,
    /// The small always-on-top window, made the first time it is opened and kept (hidden) from then on.
    mini: RefCell<Option<Rc<MiniWindow>>>,
    monitor: Rc<MonitorTab>,
    theme_id: RefCell<&'static str>,
    quitting: Cell<bool>,
    hiding: Cell<bool>,
    /// The interface got on screen at least once this run (so the start marker may go).
    page_loaded: Cell<bool>,
    cover: RefCell<Option<Rc<StartCover>>>,
    /// The program in front when this run started after an update in the background.
    front_at_start: isize,
    /// How the window comes back after an update in the background: hidden, minimized or shown (None on a normal start).
    resume_window: Option<String>,
    /// Stays in the tray at start (it was there when the update in the background began).
    start_hidden: Cell<bool>,
    show_quietly: Cell<bool>,
    told_locked_after_update: Cell<bool>,
    /// How often the window was asked to the front, so a hide under way can tell it was overtaken.
    brought_to_front: Cell<i32>,
    fading: Cell<bool>,
    opening: Cell<bool>,
    opacity: Cell<f64>,
    shown_once: Cell<bool>,
    /// Inside on_form_closing: a second close (a posted WM_CLOSE while its "couldn't save" box pumps) is cancelled.
    in_closing: Cell<bool>,
    last_state: Cell<WindowState>,
    minimum_size: Cell<Size>,
    hotkeys_taken: RefCell<Vec<String>>,
    /// The hotkeys left to the keyboard's AltGr (its hook refused), as error.log was last told of them.
    hotkeys_typing: RefCell<Vec<String>>,
    /// The hotkeys on keys the keyboard types with AltGr, on a keyboard hook; None when there are none.
    alt_gr_hotkeys: RefCell<Option<super::altgr_hotkeys::AltGrHotkeys>>,
    /// The AltGr hotkeys error.log was last told of.
    alt_gr_noted: RefCell<String>,
    closed: Cell<bool>,
}

/// The window's first shows and hides, written down (the trial build's window stayed hidden on the owner's PC,
/// 2026-11-12: which step hid it). The first few only.
static SHOW_NOTES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn note_show(what: &str) {
    if SHOW_NOTES.fetch_add(1, Ordering::SeqCst) < 40 {
        log::note(&format!("Main window: {what}"));
    }
}

unsafe extern "system" fn shell_proc(h: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM, _id: usize, _data: usize) -> LRESULT {
    const WM_SHOWWINDOW: u32 = 0x0018;
    const WM_WINDOWPOSCHANGED: u32 = 0x0047;
    if msg == WM_SHOWWINDOW {
        note_show(&format!("WM_SHOWWINDOW show={wparam} status={lparam}"));
    } else if msg == WM_WINDOWPOSCHANGED && lparam != 0 {
        // SAFETY: WM_WINDOWPOSCHANGED's lparam is a WINDOWPOS.
        let flags = unsafe { (*(lparam as *const windows_sys::Win32::UI::WindowsAndMessaging::WINDOWPOS)).flags };
        if flags & (0x0040 | 0x0080) != 0 {
            note_show(&format!("position changed, {}", if flags & 0x0080 != 0 { "hidden" } else { "shown" }));
        }
    }
    let shell = SHELL.with(|s| s.borrow().clone());
    if let Some(shell) = shell {
        // A panic while closing keeps the window up (the default close would quit the app instead of hiding it).
        let fallback = move || (msg == WM_CLOSE).then_some(0);
        if let Some(r) = super::program::shielded(fallback, || shell.wnd_proc(msg, wparam, lparam)) {
            return r;
        }
    }
    // SAFETY: the next handler of the chain (the interface's own).
    unsafe { DefSubclassProc(h, msg, wparam, lparam) }
}

/// Takes over the interface's window `h` (the C# constructor, OnHandleCreated, OnLoad and Shown, in that order). On the
/// interface's thread, which becomes the windows thread.
/// PORT: the C# window was made invisible (Opacity 0) before it first showed; the interface's window should be made
/// hidden, so the place and the fade below come before anything is seen.
pub fn attach(h: isize) {
    if is_attached() || h == 0 {
        return;
    }
    win_form::attach();
    let (resume, startup) = PREPARED.lock().unwrap_or_else(|e| e.into_inner()).take().unwrap_or((None, false));
    let shell = ShellForm::new(h, resume, startup);
    SHELL.with(|s| *s.borrow_mut() = Some(shell.clone()));
    SHELL_HWND.store(h, Ordering::SeqCst);
    // The C# constructor's `new AppController(this, postJson)`: the App owns its dialogs by this window and answers the
    // interface through gpui_host::post. Queued on the core's thread before the interface's first getState (which is
    // sent after this returns), so that one already has somewhere to go. (Never called before 2026-11-12: every reply
    // was dropped, and the real window showed an empty app with CPU and RAM at 0 %.)
    rt::post(move || App::with(|app| app.init(h, Rc::new(|line: &str| super::gpui_host::post(line.to_string())))));
    // Resume after init: its unlock pushes state and toasts and opens dialogs, which need the owner and the post.
    if let Some(state) = RESUME_AFTER_INIT.lock().unwrap_or_else(|e| e.into_inner()).take() {
        rt::post(move || App::with(|app| app.resume(state)));
    }
    // SAFETY: our procedure sees the window's messages first; removed at WM_NCDESTROY.
    unsafe { SetWindowSubclass(hwnd(h), Some(shell_proc), SUBCLASS_ID, 0) };
    shell.on_handle_created();
    // The window's place, its show and its Shown on a turn of their own, once the interface's start is over: put back
    // on a screen with another scale, Windows tells the interface of the new scale there and then, and the interface,
    // still busy starting, crashed (every start with the window saved on the owner's second screen, 2026-11-15:
    // "RefCell already borrowed" at async_context.rs:178). Nothing is seen before: the window is still hidden.
    win_form::invoke(|| {
        let Some(shell) = SHELL.with(|s| s.borrow().clone()) else { return };
        shell.on_load();
        win_form::spawn({
            let shell = shell.clone();
            async move { shell.init().await }
        });
        note_show(&format!("attached, start hidden={}", shell.start_hidden.get()));
        shell.set_visible(true);
        note_show(&format!("after show: visible={}", shell.visible()));
        // Started in the tray (Windows' sign-in, an update while hidden): the Shown event, and its entrance, wait for
        // the first real show (set_visible), as WinForms' did.
        if shell.visible() && !shell.shown_once.get() {
            shell.on_shown();
        }
    });
}

/// The App's side of the window (the delegates the C# constructor set on AppController): each hops to the windows
/// thread, or answers from what any thread may ask Windows. On the core's thread, once the App exists.
pub fn wire(app: &Rc<App>) {
    // Safe rendering (CPU drawing) is a setting of every page window's browser; the main window has no page any more.
    *app.controller.set_safe_rendering.borrow_mut() = Some(Rc::new(|safe: bool| -> LocalBoxFuture<'static, ()> {
        async move {
            let changed = {
                let mut p = prefs::lock();
                if safe != p.safe_rendering {
                    p.safe_rendering = safe;
                    true
                } else {
                    false
                }
            };
            if changed {
                prefs::save();
            }
        }
        .boxed_local()
    }));
    *app.controller.appearance_changed.borrow_mut() = Some(Rc::new(|| with(|s| s.apply_appearance())));
    *app.hotkeys.hotkeys_changed.borrow_mut() = Some(Rc::new(|| with(|s| s.register_hotkeys())));
    *app.notify.show_notification.borrow_mut() = Some(Rc::new(|text: &str, kind: &str| show_notification(text, kind)));
    *app.controller.page_shown.borrow_mut() = Some(Rc::new(|| -> LocalBoxFuture<'static, ()> {
        let (tx, rx) = oneshot::channel();
        with(move |s| {
            let s = s.clone();
            win_form::spawn(async move {
                s.cover_off().await;
                let _ = tx.send(());
            });
        });
        async move {
            let _ = rx.await;
        }
        .boxed_local()
    }));
    *app.mini.set_mini_open.borrow_mut() = Some(Rc::new(|open: Option<bool>| -> bool {
        let show = open.unwrap_or(!MINI_SHOWN.load(Ordering::SeqCst));
        with(move |s| {
            s.set_mini_open(Some(show), true);
        });
        show
    }));
    *app.monitor.set_monitor_area.borrow_mut() = Some(Rc::new(|area: Option<MonitorArea>| with(move |s| s.monitor.set_area(area))));
    *app.monitor.monitor_refresh.borrow_mut() = Some(Rc::new(|| with(|s| s.monitor.refresh())));
    *app.relaunch.quit_for_update.borrow_mut() = Some(Rc::new(|| with(|s| s.quit())));
    // After the command's answer has gone back to the page: closing to the tray takes its own turn.
    *app.relaunch.window_control.borrow_mut() = Some(Rc::new(|action: &str| {
        let action = action.to_string();
        with(move |s| s.window_control(&action));
    }));
    *app.relaunch.window_now.borrow_mut() = Some(Rc::new(window_now));
}

/// WindowNow: hidden, minimized or shown; whether it is in front; whether the mini window shows. From any thread.
pub fn window_now() -> (String, bool, bool) {
    let h = SHELL_HWND.load(Ordering::SeqCst);
    let visible = win_form::is_visible(h);
    let minimized = win_form::is_iconic(h);
    let window = if !visible {
        "hidden"
    } else if minimized {
        "minimized"
    } else {
        "shown"
    };
    (window.to_string(), visible && !minimized && in_front(h), MINI_SHOWN.load(Ordering::SeqCst))
}

/// This window or a dialog it owns is active. Asked of Windows: the active window can be another of the app's (a
/// dialog) while the keyboard sits there.
fn in_front(h: isize) -> bool {
    let front = win_form::foreground_window();
    if front == 0 {
        return false;
    }
    // Form.ActiveForm: one of the app's windows is the active one.
    if win_form::window_pid(front) == std::process::id() {
        return true;
    }
    // SAFETY: plain query.
    h != 0 && unsafe { GetAncestor(hwnd(front), GA_ROOTOWNER) } as isize == h
}

fn in_background(h: isize) -> bool {
    !win_form::is_visible(h) || win_form::is_iconic(h)
}

/// A Windows notification (tray balloon, shown as a toast) while the window is hidden, minimised or behind another
/// program; false when it is in front, so the interface's own toast is enough. From any thread.
pub fn show_notification(text: &str, kind: &str) -> bool {
    let h = SHELL_HWND.load(Ordering::SeqCst);
    if h == 0 || QUITTING.load(Ordering::SeqCst) || (!in_background(h) && in_front(h)) {
        return false;
    }
    let (text, kind) = (text.to_string(), kind.to_string());
    with(move |s| s.balloon(&text, &kind));
    true
}

fn fit(size: Size, area: Size) -> Size {
    Size::new(size.width.min(area.width), size.height.min(area.height))
}

impl ShellForm {
    fn new(h: isize, resume: Option<relaunch::State>, startup: bool) -> Rc<ShellForm> {
        let mut resume_window = None;
        let (mut start_hidden, mut show_quietly, mut front_at_start) = (false, false, 0);
        if let Some(state) = resume {
            resume_window = Some(state.window.clone());
            start_hidden = state.window == "hidden";
            show_quietly = !state.foreground;
            front_at_start = win_form::foreground_window();
            *RESUME_AFTER_INIT.lock().unwrap_or_else(|e| e.into_inner()) = Some(state);
        } else if startup {
            start_hidden = true;
        }
        let tray_window = TrayWindow::new();
        let monitor = MonitorTab::new(h, move || Some(h)).clone();
        // The tray icon's own hidden window gets its clicks and notifications.
        let tray_handler: win_form::Handler = Rc::new(|_h, msg, _wparam, lparam| {
            let shell = SHELL.with(|s| s.borrow().clone())?;
            if msg == WM_TRAY {
                shell.on_tray_message(lparam as u32 & 0xFFFF);
                return Some(0);
            }
            if msg == shell.tray.created_msg && shell.tray.visible.get() {
                // Explorer started again: the icon is added again, as WinForms does.
                shell.tray.add();
                return Some(0);
            }
            None
        });
        let tray_hwnd = win_form::create_window("Bloxkeep.TrayIcon", 0, 0, WS_EX_TOOLWINDOW, Rect::default(), 0, ui::TITLE, tray_handler).unwrap_or(0);
        let created = wide("TaskbarCreated");
        // SAFETY: a zero-ended name.
        let created_msg = unsafe { RegisterWindowMessageW(created.as_ptr()) };
        let shell = Rc::new(ShellForm {
            handle: h,
            tray: TrayIcon { window: tray_hwnd, visible: Cell::new(false), created_msg },
            tray_window,
            balloon_timer: Timer::new(BALLOON_MS + 1000),
            balloon_until: Cell::new(i64::MIN / 2),
            cover_timer: Timer::new(4000),
            mini: RefCell::new(None),
            monitor,
            theme_id: RefCell::new(themes::current().id),
            quitting: Cell::new(false),
            hiding: Cell::new(false),
            page_loaded: Cell::new(false),
            cover: RefCell::new(None),
            front_at_start,
            resume_window,
            start_hidden: Cell::new(start_hidden),
            show_quietly: Cell::new(show_quietly),
            told_locked_after_update: Cell::new(false),
            brought_to_front: Cell::new(0),
            fading: Cell::new(false),
            opening: Cell::new(false),
            opacity: Cell::new(1.0),
            shown_once: Cell::new(false),
            in_closing: Cell::new(false),
            last_state: Cell::new(WindowState::Normal),
            minimum_size: Cell::new(Size::default()),
            hotkeys_taken: RefCell::new(Vec::new()),
            hotkeys_typing: RefCell::new(Vec::new()),
            alt_gr_hotkeys: RefCell::new(None),
            alt_gr_noted: RefCell::new(String::new()),
            closed: Cell::new(false),
        });
        win_form::set_text(h, ui::TITLE);
        if let Some(icon) = ui::app_icon() {
            win_form::set_icon(h, icon);
        }
        // Made on the main screen, at its scale; put on the screen with the mouse in on_load (fit_first_place), or where it
        // was saved (restore_place).
        let main = win_form::primary_screen().working_area;
        shell.minimum_size.set(fit(Size::new(shell.ldu(980), shell.ldu(620)), main.size()));
        let size = fit(Size::new(shell.ldu(1320), shell.ldu(840)), main.size());
        win_form::set_bounds(h, Rect::new(main.left() + (main.width - size.width) / 2, main.top() + (main.height - size.height) / 2, size.width, size.height));
        let w = Rc::downgrade(&shell);
        shell.cover_timer.on_tick(move || {
            if let Some(s) = w.upgrade() {
                s.remove_cover(); // the interface never said it was drawn
            }
        });
        let w = Rc::downgrade(&shell);
        shell.balloon_timer.on_tick(move || {
            if let Some(s) = w.upgrade() {
                s.balloon_timer.stop();
                s.hide_tray_if_idle();
            }
        });
        shell.create_tray_icon();
        shell.tray.set_visible(prefs::lock().close_to_tray);
        shell
    }

    /// LogicalToDeviceUnits.
    fn ldu(&self, v: i32) -> i32 {
        win_form::logical_to_device(self.handle, v)
    }

    fn visible(&self) -> bool {
        win_form::is_visible(self.handle)
    }

    fn window_state(&self) -> WindowState {
        if win_form::is_iconic(self.handle) {
            WindowState::Minimized
        } else if win_form::is_zoomed(self.handle) {
            WindowState::Maximized
        } else {
            WindowState::Normal
        }
    }

    fn set_window_state(&self, state: WindowState) {
        let cmd = match state {
            WindowState::Normal => SW_RESTORE,
            WindowState::Minimized => SW_MINIMIZE,
            WindowState::Maximized => SW_MAXIMIZE,
        };
        // SAFETY: plain call on our window.
        unsafe { ShowWindow(hwnd(self.handle), cmd) };
    }

    fn set_opacity(&self, value: f64) {
        self.opacity.set(value);
        // PORT: Form.Opacity made the window layered; the interface's window is drawn by GPUI (DirectComposition), and a
        // layered style's alpha is applied by Windows over it.
        win_form::set_opacity(self.handle, value);
    }

    /// Back after an update but locked: said once, since the window may be in the tray and a scheduled preset waits for the unlock.
    fn tell_if_locked_after_update(self: &Rc<Self>) {
        if self.told_locked_after_update.get() {
            return;
        }
        let Ok(Some(text)) = ask_app(Duration::from_secs(10), |app| app.locked_after_update()) else { return };
        self.told_locked_after_update.set(true);
        let me = self.clone();
        win_form::spawn(async move {
            me.balloon(&text, "warn");
        });
    }

    /// Shows or hides the window (SetVisibleCore and OnVisibleChanged).
    fn set_visible(self: &Rc<Self>, mut value: bool) {
        note_show(&format!("set_visible({value})"));
        // Updated while in the tray: back in the tray; the window is made when first opened.
        if value && self.start_hidden.get() {
            self.start_hidden.set(false);
            // Quiet only for the restart itself: opened from the tray later, the window comes to the front as usual.
            self.show_quietly.set(false);
            self.tray.set_visible(true);
            rt::post(|| App::with(|app| app.set_background(true)));
            if prefs::lock().mini_open {
                let me = self.clone();
                win_form::spawn(async move {
                    me.set_mini_open(Some(true), false);
                });
            }
            self.tell_if_locked_after_update();
            value = false;
        }
        if value == self.visible() {
            return;
        }
        // SAFETY: plain call on our window.
        unsafe { ShowWindow(hwnd(self.handle), if value { if self.show_quietly.get() { SW_SHOWNOACTIVATE } else { SW_SHOW } } else { SW_HIDE }) };
        self.on_visible_changed();
        if value && !self.shown_once.get() {
            self.on_shown();
        }
    }

    /// Shown for the first time (the Shown event).
    fn on_shown(self: &Rc<Self>) {
        self.shown_once.set(true);
        win_form::update(self.handle); // the start logo painted first, then the window comes in with it
        let me = self.clone();
        win_form::spawn(async move { me.open_window().await });
        // The mini window comes back where it was, without taking the keyboard from the lock screen.
        if prefs::lock().mini_open {
            self.set_mini_open(Some(true), false);
        }
        if self.resume_window.is_some() {
            self.tell_if_locked_after_update();
        }
        // Back after an update in the background while another program was in front: that one keeps the keyboard
        // (a minimised or maximised window is activated as it shows, so the keyboard goes back) and stays on top.
        if !self.show_quietly.get() {
            // Meant to open in front: a run started by Velopack's Update.exe (after an update) isn't the one the person
            // started, and Windows leaves its window behind the others, so it is brought forward (the owner,
            // 2026-11-15: "nó ko tự pop up lên nó ở đằng sau ấy lúc mới cập nhật xong"). Said in the log either way.
            // (On a turn of its own: this runs inside the interface's start, and bringing the window forward makes
            // Windows tell the interface at once that it is active, while it is still busy; 1.0.25-alpha.12 crashed so
            // at every start on the owner's PC, 2026-11-15.)
            let h = self.handle;
            win_form::invoke(move || {
                if win_form::foreground_window() != h && win_form::is_window(h) {
                    let ok = super::anti_afk::focus(h);
                    log::note(&format!("Main window: opened behind another program, brought to the front: {}.", if ok { "done" } else { "Windows refused" }));
                }
            });
        } else {
            self.show_quietly.set(false);
            // (On a turn of its own too: giving the keyboard back tells the interface at once that it isn't active.)
            let (front, h) = (self.front_at_start, self.handle);
            win_form::invoke(move || {
                if front != 0 && front != h && win_form::is_window(front) && win_form::is_window(h) {
                    if win_form::foreground_window() == h {
                        win_form::set_foreground(front);
                    }
                    win_form::stack_normal_behind(h, front);
                }
            });
        }
    }

    /// Shows or hides the mini window (None toggles); the interface is told, since its button shows the state.
    pub fn set_mini_open(self: &Rc<Self>, open: Option<bool>, activate: bool) -> bool {
        let show = open.unwrap_or_else(|| !self.mini.borrow().as_ref().is_some_and(|m| m.visible()));
        if show {
            if self.mini.borrow().is_none() {
                let me = Rc::downgrade(self);
                let mini = MiniWindow::new(move || {
                    if let Some(s) = me.upgrade() {
                        s.show_in_front("mini window");
                    }
                });
                *mini.on_dismissed.borrow_mut() = Some(Box::new(|| {
                    MINI_SHOWN.store(false, Ordering::SeqCst);
                    rt::post(|| App::with(|app| app.push_state()));
                }));
                // Open only once it is really on screen: a page that never drew left the button stuck on "open".
                *mini.on_shown.borrow_mut() = Some(Box::new(|| {
                    if !MINI_SHOWN.swap(true, Ordering::SeqCst) {
                        rt::post(|| App::with(|app| app.push_state()));
                    }
                }));
                *self.mini.borrow_mut() = Some(mini);
            }
            let mini = self.mini.borrow().clone();
            if let Some(mini) = mini {
                mini.open(activate);
            }
        } else {
            let mini = self.mini.borrow().clone();
            let was = mini.as_ref().is_some_and(|m| m.visible());
            if let Some(mini) = mini {
                mini.dismiss(); // which pushes the state itself
            }
            // Never got on screen (its page didn't start, or asked again while opening): dismiss had nothing to do, so
            // the flag the page's button toggles from is put right here, or the button stayed stuck on "open".
            if !was && MINI_SHOWN.swap(false, Ordering::SeqCst) {
                rt::post(|| App::with(|app| app.push_state()));
            }
        }
        show
    }

    fn create_tray_icon(self: &Rc<Self>) {
        // The quick panel (a left click) and the menu (a right click), drawn by the tray's page (tray_window).
        let tw = &self.tray_window;
        let me = Rc::downgrade(self);
        *tw.mini_open.borrow_mut() = Some(Box::new({
            let me = me.clone();
            move || me.upgrade().is_some_and(|s| s.mini.borrow().as_ref().is_some_and(|m| m.visible()))
        }));
        let mut ev = tw.events.borrow_mut();
        ev.open_clicked = Some(Box::new({
            let me = me.clone();
            move || {
                if let Some(s) = me.upgrade() {
                    s.show_in_front("tray");
                }
            }
        }));
        ev.mini_clicked = Some(Box::new({
            let me = me.clone();
            move || {
                if let Some(s) = me.upgrade() {
                    s.set_mini_open(None, true);
                }
            }
        }));
        ev.quit_clicked = Some(Box::new({
            let me = me.clone();
            move || {
                if let Some(s) = me.upgrade() {
                    s.quit();
                }
            }
        }));
        // Only offered where it can switch from here.
        ev.multi_clicked = Some(Box::new(|on: bool| rt::post(move || App::with(|app| app.tray_multi(!on)))));
        ev.account_clicked = Some(Box::new(|id: Guid| rt::post(move || App::with(|app| {
            let app = app.clone();
            rt::spawn(async move { app.tray_open_account(id).await });
        }))));
        ev.preset_clicked = Some(Box::new(|id: Guid| rt::post(move || App::with(|app| {
            let app = app.clone();
            rt::spawn(async move { app.tray_run_preset(id).await });
        }))));
        ev.panel_clicked = Some(Box::new(|id: Guid| rt::post(move || App::with(|app| app.tray_toggle_panel(id)))));
        ev.new_roblox_clicked = Some(Box::new(|| rt::post(|| App::with(|app| {
            let app = app.clone();
            rt::spawn(async move { app.tray_new_roblox().await });
        }))));
        ev.close_all_clicked = Some(Box::new(|| rt::post(|| App::with(|app| {
            app.close_all_clients();
        }))));
        ev.show_client_clicked = Some(Box::new(|pid: i32| rt::post(move || App::with(|app| app.tray_show_client(pid)))));
        ev.close_client_clicked = Some(Box::new(|id: Guid| rt::post(move || App::with(|app| app.close_client(id)))));
    }

    /// The tray icon's own messages: clicks, and a notification clicked or gone.
    fn on_tray_message(self: &Rc<Self>, event: u32) {
        const NIN_BALLOONHIDE: u32 = WM_USER + 3;
        const NIN_BALLOONTIMEOUT: u32 = WM_USER + 4;
        const NIN_BALLOONUSERCLICK: u32 = WM_USER + 5;
        match event {
            // The pointer on the icon: the tray's page starts ahead of the click (tray_window::warm).
            WM_MOUSEMOVE => self.tray_window.warm(),
            WM_LBUTTONUP | WM_RBUTTONUP => {
                self.tray_window.toggle(win_form::cursor_position(), if event == WM_LBUTTONUP { "panel" } else { "menu" });
            }
            // A notification shown while the icon is normally hidden (close-to-tray off) takes the icon with it — but
            // not while a newer balloon is still showing (Windows closes the one it replaced right after).
            NIN_BALLOONUSERCLICK => {
                self.show_in_front("notification");
                self.balloon_until.set(i64::MIN / 2);
                self.hide_tray_if_idle();
            }
            NIN_BALLOONHIDE | NIN_BALLOONTIMEOUT => self.hide_tray_if_idle(),
            _ => {}
        }
    }

    /// The balloon itself (show_notification's part on the windows thread).
    fn balloon(&self, text: &str, kind: &str) {
        if self.closed.get() || self.quitting.get() {
            return;
        }
        self.balloon_until.set(clock::tick() + BALLOON_MS as i64);
        self.tray.set_visible(true);
        self.tray.show_balloon(ui::TITLE, text, match kind {
            "error" => NIIF_ERROR,
            "warn" => NIIF_WARNING,
            _ => NIIF_NONE,
        });
        // The closed event may come early (dismissed) or not at all: the timer hides the icon in the end.
        self.balloon_timer.stop();
        self.balloon_timer.set_interval(BALLOON_MS + 1000);
        self.balloon_timer.start();
    }

    fn balloon_showing(&self) -> bool {
        clock::tick() < self.balloon_until.get()
    }

    /// Back to the icon's normal state, unless a notification is still on screen.
    fn hide_tray_if_idle(&self) {
        if !self.closed.get() && !self.balloon_showing() {
            self.tray.set_visible(prefs::lock().close_to_tray);
        }
    }

    /// Closes for real, even with "keep running when closed" on.
    pub fn quit(&self) {
        self.quitting.set(true);
        QUITTING.store(true, Ordering::SeqCst);
        // SAFETY: a plain post to our window: closing goes through on_form_closing.
        unsafe { PostMessageW(hwnd(self.handle), WM_CLOSE, 0, 0) };
    }

    /// The window's buttons from the interface (WindowControl).
    fn window_control(&self, action: &str) {
        if self.closed.get() {
            return;
        }
        if action == "close" {
            // SAFETY: as quit.
            unsafe { PostMessageW(hwnd(self.handle), WM_CLOSE, 0, 0) };
        } else if action == "minimize" {
            self.set_window_state(WindowState::Minimized);
        } else {
            self.set_window_state(if self.window_state() == WindowState::Maximized { WindowState::Normal } else { WindowState::Maximized });
        }
    }

    /// FormClosing: true cancels the close.
    fn on_form_closing(self: &Rc<Self>, shutting_down: bool) -> bool {
        if self.in_closing.replace(true) {
            return !shutting_down;
        }
        struct Out<'a>(&'a Cell<bool>);
        impl Drop for Out<'_> {
            fn drop(&mut self) {
                self.0.set(false);
            }
        }
        let _out = Out(&self.in_closing);
        self.save_place();
        // Windows shutting down and Task Manager still close the app; the uninstaller and the tray menu use quit.
        if !self.quitting.get() && prefs::lock().close_to_tray && !shutting_down {
            let me = self.clone();
            win_form::spawn(async move { me.hide_to_tray() });
            return true;
        }
        // Changes that can't be written yet (file held by an antivirus) would be lost: ask, unless Windows is shutting down.
        // A core still busy after the wait (a file dialog open on it) is not a failed save: no "Couldn't save" then.
        let saved = match ask_app(Duration::from_secs(10), |app| app.save_before_closing()) {
            Ok(saved) => saved,
            Err(timed_out) => {
                if timed_out {
                    log::note("Closing: the save before closing didn't answer in 10 s; quitting without asking.");
                }
                timed_out
            }
        };
        if !saved && !shutting_down {
            let owner = Some(self.handle);
            let quit = ui::modal(|| {
                crate::core::forms::dialog::confirm(owner, &l::t("Couldn't save your changes. Quit anyway and lose them?"), &l::t("Quit anyway"), &l::t("Keep Bloxkeep open"), true)
            });
            if !quit {
                self.quitting.set(false);
                QUITTING.store(false, Ordering::SeqCst);
                return true;
            }
        }
        // No fade on the way out (1.0.19 to 1.0.21-alpha.3 had one): turning the window see-through for it made Windows
        // show it empty, in the theme's colour, for a frame or two first (OBS, 2026-11-07 11:51, maximised, both closes).
        false
    }

    /// PORT: the C# took a picture of the page first (shown over the window on its way back); the interface draws
    /// itself, so it hides at once.
    fn hide_to_tray(self: &Rc<Self>) {
        if self.hiding.get() {
            return;
        }
        self.hiding.set(true);
        let brought = self.brought_to_front.get();
        self.hiding.set(false);
        // Opened from the tray meanwhile: the person changed their mind.
        if self.quitting.get() || self.closed.get() || self.brought_to_front.get() != brought {
            return;
        }
        self.tray.set_visible(true);
        self.set_visible(false);
        if prefs::lock().tray_hint_shown {
            return;
        }
        prefs::lock().tray_hint_shown = true;
        prefs::save();
        self.tray.show_balloon(ui::TITLE, &l::t("Bloxkeep is still running. Right-click this icon to quit."), NIIF_NONE);
    }

    /// A new theme or close-to-tray choice from Settings, or Windows switching between light and dark.
    fn apply_appearance(&self) {
        let theme = themes::current();
        *self.theme_id.borrow_mut() = theme.id;
        ui::use_themed_title_bar(self.handle, &theme);
        let mini = self.mini.borrow().clone();
        if let Some(mini) = mini {
            mini.apply_theme();
        }
        rt::post(|| App::with(|app| app.apply_theme_to_panels()));
        self.tray.set_visible(prefs::lock().close_to_tray || self.balloon_showing());
    }

    /// Windows' settings changed (SystemEvents.UserPreferenceChanged, General): a theme that follows Windows follows it.
    fn on_user_preference_changed(self: &Rc<Self>) {
        if prefs::lock().theme != themes::FOLLOW_WINDOWS || self.closed.get() {
            return;
        }
        let me = self.clone();
        win_form::spawn(async move {
            if themes::current().id == *me.theme_id.borrow() {
                return;
            }
            me.apply_appearance();
            rt::post(|| App::with(|app| app.push_state()));
        });
    }

    /// Shows the window and puts it in front, saying from where. Not a BringToFront overload: passed as an Action,
    /// that name bound to Control.BringToFront, which only restacks, so Open in the tray and the mini window did nothing
    /// (owner, 2026-11-08).
    pub fn show_in_front(self: &Rc<Self>, why: &str) {
        // Each time Bloxkeep puts itself over what the person is doing, and from where (a player found it "stuck on top").
        log::note(&format!("Bloxkeep's window brought to the front ({why})."));
        self.brought_to_front.set(self.brought_to_front.get() + 1);
        if !self.visible() {
            self.set_visible(true);
        }
        // Back to the size it had, maximised or not (setting WindowState = Normal would un-maximise it).
        if self.window_state() == WindowState::Minimized {
            // SAFETY: plain call on our window.
            unsafe { ShowWindow(hwnd(self.handle), SW_RESTORE) };
        }
        win_form::activate(self.handle);
    }

    fn on_handle_created(&self) {
        ui::use_themed_title_bar(self.handle, &themes::current());
        self.register_hotkeys();
    }

    /// The hotkeys from any app while the switch is on; one another app holds is reported in Settings.
    fn register_hotkeys(&self) {
        // Asked of the App without waiting on it: a busy core used to time the ask out and drop the macro hotkeys.
        rt::post(|| {
            App::with(|app| {
                let (digits, recording) = (app.macro_hotkeys(), app.recording_macro());
                with(move |s| s.apply_hotkeys(digits, recording));
            })
        });
    }

    fn apply_hotkeys(&self, digits: Vec<i32>, recording: bool) {
        let h = hwnd(self.handle);
        // SAFETY: unregistering ids that may not be registered only fails.
        unsafe {
            for (id, _) in HOTKEYS {
                UnregisterHotKey(h, id);
            }
            for id in macros::HOTKEY_STOP_MACROS..=macros::HOTKEY_END_RECORDING {
                UnregisterHotKey(h, id);
            }
        }
        // The old hook goes before a new one is set.
        self.alt_gr_hotkeys.borrow_mut().take();
        let mut taken: Vec<String> = Vec::new();
        let mut typing: Vec<String> = Vec::new();
        let global = prefs::lock().global_hotkeys;
        if global {
            // AltGr is Ctrl+Alt to Windows: a hotkey on a key the keyboard types with AltGr took the character from every
            // app (a French keyboard's @ is AltGr+0, and Ctrl+Alt+0 stops macros: a user's report, 1.0.21, 2026-11-11).
            // Those keys go to a keyboard hook that tells the left Alt (the hotkey) from AltGr (typing).
            let alt_gr = super::altgr_hotkeys::typed();
            let mut hooked: std::collections::HashMap<u32, i32> = std::collections::HashMap::new();
            let mut hooked_names: Vec<String> = Vec::new();
            let mut add = |id: i32, key: u32, name: String| {
                if alt_gr.contains(&key) {
                    hooked.insert(key, id);
                    hooked_names.push(name);
                // SAFETY: a plain registration on our window.
                } else if unsafe { RegisterHotKey(h, id, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, key) } == 0 {
                    taken.push(name);
                }
            };
            for (id, key) in HOTKEYS {
                add(id, key, format!("Ctrl+Alt+{}", char::from_u32(key).unwrap_or('?')));
            }
            // Macros (Beta): Ctrl+Alt+0 stops them all; Ctrl+Alt+1…9 only for the digits a macro holds.
            add(macros::HOTKEY_STOP_MACROS, VK_D0, "Ctrl+Alt+0".into());
            for digit in digits {
                add(macros::HOTKEY_MACRO_FIRST + digit - 1, VK_D0 + digit as u32, format!("Ctrl+Alt+{digit}"));
            }
            if !hooked.is_empty() {
                // Posted: the hook's thread must answer Windows at once.
                let started = super::altgr_hotkeys::AltGrHotkeys::start(hooked, |id| with(move |s| s.on_hotkey(id)));
                let names = hooked_names.join(",");
                match started {
                    // Left to the keyboard, as said in Settings.
                    None => typing.extend(hooked_names),
                    Some(hook) => {
                        if names != *self.alt_gr_noted.borrow() {
                            log::note(&format!("Hotkeys on AltGr keys, with the left Alt only: {}.", hooked_names.join(", ")));
                        }
                        *self.alt_gr_hotkeys.borrow_mut() = Some(hook);
                    }
                }
                *self.alt_gr_noted.borrow_mut() = names;
            }
        }
        if !typing.is_empty() && typing.join(",") != self.hotkeys_typing.borrow().join(",") {
            log::note(&format!("Hotkeys left to the keyboard's AltGr: {}.", typing.join(", ")));
        }
        *self.hotkeys_typing.borrow_mut() = typing.clone();
        // SAFETY: a plain registration on our window.
        unsafe {
            // F8 ends a recording, whatever the switch says (the recorder also watches for it itself).
            if recording {
                RegisterHotKey(h, macros::HOTKEY_END_RECORDING, MOD_NOREPEAT, VK_F8);
            }
        }
        if !taken.is_empty() && taken.join(",") != self.hotkeys_taken.borrow().join(",") {
            log::note(&format!("Hotkeys another app holds: {}.", taken.join(", ")));
        }
        *self.hotkeys_taken.borrow_mut() = taken.clone();
        rt::post(move || {
            App::with(|app| {
                *app.hotkeys.hotkeys_taken.borrow_mut() = taken;
                *app.hotkeys.hotkeys_typing.borrow_mut() = typing;
            })
        });
    }

    /// A hotkey, from Windows (RegisterHotKey) or the AltGr hook.
    fn on_hotkey(self: &Rc<Self>, id: i32) {
        // Ctrl+Alt+B: Bloxkeep itself, from the tray too (as its icon's Open does).
        if id == hotkeys::HOTKEY_BLOXKEEP {
            log::note("Hotkey Ctrl+Alt+B received.");
            self.show_in_front("Ctrl+Alt+B");
        } else {
            rt::post(move || App::with(|app| app.on_hotkey(id)));
        }
    }

    /// The window's messages, before the interface's own (WndProc).
    fn wnd_proc(self: &Rc<Self>, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        match msg {
            // The interface draws its own top bar and leaves Windows' frame on the other sides: only the top edge is the
            // client's. Maximised, Windows puts the frame beyond the screen's edge: the client starts that far down.
            // (Asked of Windows: the state still says Normal while the window is being maximised, so the interface's
            // top 11 px sat above the screen; seen on the owner's maximised window, 2026-11-11.)
            WM_NCCALCSIZE if wparam != 0 && CUSTOM_FRAME.load(Ordering::SeqCst) => {
                // SAFETY: lParam is NCCALCSIZE_PARAMS for this message; rgrc[0].top is its second i32.
                unsafe {
                    let top_ptr = (lparam as *mut i32).add(1);
                    let mut top = *top_ptr;
                    DefSubclassProc(hwnd(self.handle), msg, wparam, lparam);
                    if win_form::is_zoomed(self.handle) {
                        top += super::gpui_host::frame_thickness(self.handle);
                    }
                    *top_ptr = top;
                }
                Some(0)
            }
            WM_HOTKEY => {
                self.on_hotkey(wparam as i32);
                Some(0)
            }
            // Coming back from the taskbar: Windows asks first, while the window is still minimised. Invisible from here,
            // so it never shows before it is drawn; it fades in once back (on_resize).
            WM_QUERYOPEN => {
                if self.window_state() == WindowState::Minimized && self.shown_once.get() && !self.fading.get() && self.opacity.get() >= 1.0 {
                    self.set_opacity(0.0);
                }
                None
            }
            WM_GETMINMAXINFO => {
                // SAFETY: the interface's own limits first, then MinimumSize.
                let r = unsafe { DefSubclassProc(hwnd(self.handle), msg, wparam, lparam) };
                win_form::apply_min_max(lparam, self.minimum_size.get(), Size::default());
                Some(r)
            }
            WM_SIZE => {
                // SAFETY: the interface resizes itself first.
                let r = unsafe { DefSubclassProc(hwnd(self.handle), msg, wparam, lparam) };
                self.on_resize();
                Some(r)
            }
            WM_EXITSIZEMOVE => {
                self.save_place(); // OnResizeEnd
                None
            }
            WM_SETTINGCHANGE => {
                self.on_user_preference_changed();
                None
            }
            // SystemEvents.PowerModeChanged and TimeChanged.
            WM_POWERBROADCAST => {
                use super::app::power::{PowerMode, on_power_mode_changed};
                match wparam as u32 {
                    PBT_APMSUSPEND => on_power_mode_changed(PowerMode::Suspend),
                    PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND => on_power_mode_changed(PowerMode::Resume),
                    PBT_APMPOWERSTATUSCHANGE => on_power_mode_changed(PowerMode::StatusChange),
                    _ => {}
                }
                None
            }
            WM_TIMECHANGE => {
                super::app::power::on_time_changed();
                None
            }
            // Moved to a screen of another scale: the smallest size follows (WinForms rescaled MinimumSize), or a 100 %
            // size held on a 150 % screen let the window shrink to two thirds of what it needs.
            WM_DPICHANGED => {
                // SAFETY: lParam is the suggested RECT for this message.
                let suggested = unsafe { *(lparam as *const windows_sys::Win32::Foundation::RECT) };
                let dpi = (wparam & 0xFFFF) as i32;
                let area = screen_from_rect(Rect::new(suggested.left, suggested.top, suggested.right - suggested.left, suggested.bottom - suggested.top)).working_area;
                self.minimum_size.set(fit(Size::new(980 * dpi / 96, 620 * dpi / 96), area.size()));
                None
            }
            // Every WM_CLOSE (the ×, Alt+F4, the taskbar's Close) follows "keep running when closed"; only quit(), the tray's
            // Exit and Windows shutting down really quit. Telling the taskbar's Close from another program's isn't possible.
            WM_CLOSE => {
                if self.on_form_closing(false) {
                    return Some(0);
                }
                None
            }
            WM_QUERYENDSESSION => None,
            WM_ENDSESSION if wparam != 0 => {
                self.on_form_closing(true);
                self.on_form_closed(true);
                None
            }
            WM_DESTROY => {
                self.on_form_closed(false);
                None
            }
            WM_NCDESTROY => {
                // SAFETY: our subclass goes with the window.
                unsafe { RemoveWindowSubclass(hwnd(self.handle), Some(shell_proc), SUBCLASS_ID) };
                SHELL_HWND.store(0, Ordering::SeqCst);
                None
            }
            _ => None,
        }
    }

    /// At start, the logo in the window until the interface's loading screen is drawn. The interface can be slow to
    /// start on a slow PC or after an update, so the fallback is long.
    fn show_start_cover(&self) {
        let size = self.ldu(72);
        let Some(logo) = ui::app_icon_picture(size) else { return };
        let cover = StartCover::new(logo, size as f32 / 72.0, themes::current().background);
        // PORT: a child window over the interface's; GPUI's DirectComposition content may draw above child windows.
        if cover.attach(self.handle).is_err() {
            cover.dispose();
            return;
        }
        self.put_cover(cover, 15_000);
    }

    fn put_cover(&self, cover: Rc<StartCover>, fallback_ms: u32) {
        self.remove_cover();
        *self.cover.borrow_mut() = Some(cover);
        self.cover_timer.set_interval(fallback_ms);
        self.cover_timer.start();
    }

    /// The interface is drawn: its cover comes off, the start logo's between two hops (StartCover::at_rest) and
    /// once the window is solid. The interface's answer waits for it, so its own logo starts where this one stood.
    async fn cover_off(self: Rc<Self>) {
        let cover = self.cover.borrow().clone();
        if let Some(start) = cover {
            start.at_rest().await;
        }
        while (self.fading.get() || self.opening.get()) && !self.closed.get() {
            win_form::delay(15).await;
        }
        self.remove_cover();
    }

    fn remove_cover(&self) {
        self.cover_timer.stop();
        let cover = self.cover.borrow_mut().take();
        if let Some(cover) = cover {
            cover.dispose();
        }
    }

    fn on_resize(self: &Rc<Self>) {
        let cover = self.cover.borrow().clone();
        if let Some(c) = cover {
            c.fit(self.handle);
        }
        self.update_background();
        // Maximised or restored with the title bar's button (no ResizeEnd for that): remembered for the next start.
        let state = self.window_state();
        let last = self.last_state.get();
        if state != last {
            // Back from the taskbar, still invisible (WM_QUERYOPEN): drawn now, then the fade.
            if last == WindowState::Minimized && state != WindowState::Minimized && self.opacity.get() < 1.0 && !self.fading.get() {
                let me = self.clone();
                win_form::spawn(async move { me.fade_in(260.0, true) });
            }
            let restored = last == WindowState::Maximized && state == WindowState::Normal;
            self.last_state.set(state);
            if restored {
                let me = self.clone();
                win_form::spawn(async move { me.fit_restored() }); // not inside the resize that's under way
            }
            if state != WindowState::Minimized {
                self.save_place();
            }
        }
    }

    fn fit_restored(&self) {
        if self.closed.get() || self.window_state() != WindowState::Normal {
            return;
        }
        let bounds = win_form::bounds(self.handle);
        let place = roblox_processes::restored_place(
            bounds,
            screen_from_handle(self.handle).working_area,
            roblox_processes::resize_border(self.handle),
            Size::new(self.ldu(1320), self.ldu(840)),
        );
        if place == bounds {
            return;
        }
        win_form::set_bounds(self.handle, place);
        self.save_place();
    }

    fn on_load(&self) {
        // Opened: see fade_in. Invisible until then, so the white Windows paints a new window with for a frame or two
        // never shows either. First of all: making the window maximised below (a saved maximised window, or one too big
        // for the screen) shows it there and then, and it flashed white, full screen, for a frame (OBS, 2026-11-07).
        self.set_opacity(0.0);
        if saved_place().is_some() {
            self.restore_place();
        } else {
            self.fit_first_place();
        }
        // An update in the background found the window minimised: it comes back that way, solid (nothing to fade).
        if self.resume_window.as_deref() == Some("minimized") {
            self.set_opacity(1.0);
            self.set_window_state(WindowState::Minimized);
        }
    }

    /// The window's entrance on start: a stand-in drawn as the window will look grows in from the middle
    /// (window_opening), then the window, waiting invisible above it, fades in over it in a moment and the stand-in goes;
    /// then the start logo begins to hop. A plain fade when there is nothing to stand in for (no start logo, minimised)
    /// or the stand-in can't be made.
    async fn open_window(self: Rc<Self>) {
        let start = self.cover.borrow().clone();
        let frame = roblox_processes::seen_frame(self.handle).filter(|f| f.width > 0 && f.height > 0);
        let (Some(start), Some(frame), false) = (start.clone(), frame, self.window_state() == WindowState::Minimized) else {
            self.fade_in(260.0, true);
            if let Some(s) = start {
                s.start();
            }
            return;
        };
        self.opening.set(true);
        let origin = win_form::client_origin(self.handle);
        let csize = win_form::client_size(self.handle);
        let client = Rect::new(origin.x - frame.x, origin.y - frame.y, csize.width, csize.height);
        // Windows 11 rounds a window that isn't maximised and draws its hairline.
        let round = self.window_state() == WindowState::Normal && ui::windows_build() >= 22000;
        let theme = themes::current();
        let argb = |c: themes::Color| 0xFF00_0000 | (c.r as u32) << 16 | (c.g as u32) << 8 | c.b as u32;
        let draw = {
            let start = start.clone();
            move |g: &mut super::win_kit::Picture, area: Rect| start.draw_logo(g, area)
        };
        let plate = WindowOpening::plate(frame.size(), client, argb(theme.background), argb(theme.line), if round { self.ldu(8) } else { 0 }, &draw);
        match WindowOpening::new(frame, plate) {
            Ok(mut stand_in) => {
                stand_in.grow(self.handle);
                self.fade_in(140.0, false);
                while self.fading.get() && !self.closed.get() {
                    win_form::delay(15).await;
                }
                drop(stand_in);
            }
            Err(e) => {
                log::note(&format!("Window opening skipped: {}", e.message()));
                self.fade_in(260.0, true);
            }
        }
        self.opening.set(false);
        start.start();
    }

    /// The window opening: it fades in while rising the last few pixels into its place (unless maximised), a quarter
    /// of a second, slowing at the end. Bloxkeep's own, as Windows' animation effects may be off (they are on many PCs,
    /// the owner's too), which made the window just appear, white for a frame first. Meanwhile the window holds only its
    /// start logo (show_start_cover).
    fn fade_in(self: &Rc<Self>, ms: f64, rise: bool) {
        if self.opacity.get() >= 1.0 || self.fading.get() || self.closed.get() {
            return;
        }
        if self.window_state() == WindowState::Minimized {
            self.set_opacity(1.0);
            return;
        }
        let rise = rise && self.window_state() == WindowState::Normal;
        let home = win_form::bounds(self.handle).location();
        let drop = if rise { self.ldu(14) } else { 0 };
        let clock = std::time::Instant::now();
        let timer = Timer::new(1);
        self.fading.set(true);
        let me = Rc::downgrade(self);
        let t = timer.clone();
        let step = Rc::new(move || {
            let Some(s) = me.upgrade() else {
                t.dispose();
                return;
            };
            let p = (clock.elapsed().as_secs_f64() * 1000.0 / ms).min(1.0);
            let e = 1.0 - (1.0 - p).powi(3);
            if s.closed.get() {
                t.dispose();
                s.fading.set(false);
                return;
            }
            if drop > 0 && s.window_state() == WindowState::Normal {
                // SAFETY: plain call on our window.
                unsafe {
                    SetWindowPos(hwnd(s.handle), std::ptr::null_mut(), home.x, home.y + (drop as f64 * (1.0 - e)).round() as i32, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE)
                };
            }
            s.set_opacity(if p >= 1.0 { 1.0 } else { e });
            if p >= 1.0 {
                t.dispose();
                s.fading.set(false);
            }
        });
        let s2 = step.clone();
        timer.on_tick(move || s2());
        step();
        if self.fading.get() {
            timer.start();
        }
    }

    /// No saved place (a first start, or a lost screen): the screen with the mouse, fitted. On a 1366×768 or 1080p laptop
    /// at 125–150 % the preferred size would put the launch dock under the taskbar. Only now, with the window real and
    /// moved there first: sized at the main screen's scale, a window opening on a 150 % screen beside a 100 % main one was
    /// fitted at 100 % and then scaled up past the taskbar (review 2026-10-30).
    fn fit_first_place(&self) {
        let area = win_form::screen_from_point(win_form::cursor_position()).working_area;
        let b = win_form::bounds(self.handle);
        // There first, so a screen with another scale gives a real dpi change; then sized at that scale.
        win_form::set_location(self.handle, Point::new(area.left() + 0.max((area.width - b.width) / 2), area.top() + 0.max((area.height - b.height) / 2)));
        let preferred = Size::new(self.ldu(1320), self.ldu(840));
        self.minimum_size.set(fit(Size::new(self.ldu(980), self.ldu(620)), area.size()));
        let size = fit(preferred, area.size());
        win_form::set_bounds(self.handle, Rect::new(area.left() + (area.width - size.width) / 2, area.top() + (area.height - size.height) / 2, size.width, size.height));
        if preferred.width > area.width || preferred.height > area.height {
            self.set_window_state(WindowState::Maximized);
        }
    }

    fn restore_place(&self) {
        // Only now, with the window real: moved to a screen with another scale, it then gets a proper dpi change.
        let Some(saved) = saved_place() else { return };
        let area = screen_from_rect(saved).working_area;
        win_form::set_location(self.handle, saved.location());
        let min = fit(Size::new(self.ldu(980), self.ldu(620)), area.size()); // at that screen's scale
        self.minimum_size.set(min);
        let size = Size::new(saved.width.clamp(min.width, min.width.max(area.width)), saved.height.clamp(min.height, min.height.max(area.height)));
        // Inside the screen, not only as big as it: the size alone left a window saved wider than the screen cut off.
        let at = ui::inside(Rect::at(saved.location(), size), area, roblox_processes::resize_border(self.handle));
        win_form::set_bounds(self.handle, Rect::at(at, size));
        if prefs::lock().main_maximized {
            self.set_window_state(WindowState::Maximized);
        }
        self.last_state.set(self.window_state());
    }

    /// Remembers the size it has when not maximised (also while maximised or minimised) and whether it is maximised.
    fn save_place(&self) {
        if !self.visible() || self.closed.get() {
            return;
        }
        let state = self.window_state();
        let normal = if state == WindowState::Normal { win_form::bounds(self.handle) } else { restore_bounds(self.handle) };
        if normal.width <= 0 || normal.height <= 0 {
            return;
        }
        let place = format!("{},{},{},{}", normal.x, normal.y, normal.width, normal.height);
        {
            let mut p = prefs::lock();
            // Minimised says nothing about maximised: the last known state is kept.
            let maximized = if state == WindowState::Minimized { p.main_maximized } else { state == WindowState::Maximized };
            if p.main_window.as_deref() == Some(place.as_str()) && maximized == p.main_maximized {
                return;
            }
            p.main_window = Some(place);
            p.main_maximized = maximized;
        }
        prefs::save();
    }

    fn on_visible_changed(self: &Rc<Self>) {
        self.update_background();
        // Back from the tray (it went away into it): fades in again.
        if self.visible() && self.shown_once.get() && self.opacity.get() < 1.0 {
            let me = self.clone();
            win_form::spawn(async move { me.fade_in(260.0, true) });
        }
    }

    /// In the background the App polls less (its Background). PORT: the C# also put the page to sleep or let its browser
    /// go here; the interface has no browser.
    fn update_background(&self) {
        let background = in_background(self.handle);
        rt::post(move || App::with(|app| app.set_background(background)));
        self.monitor.place(); // gone at once while minimised or hidden; back with the interface's next report
    }

    /// The start: CPU drawing for the page windows when the last start never got on screen, the start logo, and the
    /// marker taken away once the interface is there.
    async fn init(self: Rc<Self>) {
        // The last start never got the interface on screen (for example a broken graphics driver): don't use the GPU.
        let safe = prefs::lock().safe_rendering;
        if start_marker().exists() && !safe {
            prefs::lock().safe_rendering = true;
            prefs::save();
        }
        if !prefs::lock().safe_rendering {
            delete_safe_profile();
        }
        self.show_start_cover();
        // The interface is in this process and already has its window (StartGpuiAsync's success).
        self.page_loaded.set(true);
        try_mark_starting(false);
    }

    fn on_form_closed(&self, shutting_down: bool) {
        if self.closed.replace(true) {
            return;
        }
        self.alt_gr_hotkeys.borrow_mut().take();
        // A run whose interface got on screen was good; so was one quit by the person or Windows.
        if self.page_loaded.get() || self.quitting.get() || shutting_down {
            try_mark_starting(false);
        }
        self.tray.set_visible(false);
        win_form::destroy(self.tray.window);
        self.tray_window.dispose();
        // Closed for the exit (not dismissed), so it comes back next start if it was open.
        let mini = self.mini.borrow_mut().take();
        if let Some(mini) = mini {
            use super::web_tool_window::WebTool;
            mini.tool().dispose();
        }
        self.balloon_timer.dispose();
        self.remove_cover();
        self.cover_timer.dispose();
        self.monitor.dispose();
        if shutting_down {
            // Windows may end the process once WM_ENDSESSION returns: the clean-up (Roblox's flags and settings put
            // back, Discord told, the clipboard cleared) is waited for, as the C# did it in OnFormClosed itself.
            let (tx, rx) = std::sync::mpsc::channel::<()>();
            rt::post(move || {
                App::with(|app| app.dispose());
                let _ = tx.send(());
            });
            // Pumped: a step of dispose that needs this thread (a window it owns) doesn't stall the rest to the time limit.
            let _ = win_form::wait_pumping_for(&rx, Some(Duration::from_secs(5)));
        } else {
            rt::post(|| App::with(|app| app.dispose()));
        }
    }
}

/// The saved place, unless no screen shows the middle of its title bar any more (a monitor unplugged).
fn saved_place() -> Option<Rect> {
    let saved = prefs::lock().main_window.clone()?;
    let parts: Vec<&str> = saved.split(',').collect();
    if parts.len() != 4 {
        return None;
    }
    let (x, y, w, h): (i32, i32, i32, i32) = (parts[0].parse().ok()?, parts[1].parse().ok()?, parts[2].parse().ok()?, parts[3].parse().ok()?);
    if w <= 0 || h <= 0 {
        return None;
    }
    let grip = Rect::new(x + w / 4, y + 2, w / 2, 24);
    if all_screens().iter().any(|s| win_form::intersects(s.working_area, grip)) { Some(Rect::new(x, y, w, h)) } else { None }
}

/// RestoreBounds: the place it has when not maximised or minimised.
fn restore_bounds(h: isize) -> Rect {
    // SAFETY: the structure's length is set as Windows requires.
    unsafe {
        let mut wp: WINDOWPLACEMENT = std::mem::zeroed();
        wp.length = std::mem::size_of::<WINDOWPLACEMENT>() as u32;
        if GetWindowPlacement(hwnd(h), &mut wp) == 0 {
            return Rect::default();
        }
        // rcNormalPosition is in work-area coordinates: moved by the work area's offset from its screen.
        let r = Rect::from_win(&wp.rcNormalPosition);
        let screen = screen_from_rect(r);
        let ex = GetWindowLongPtrW(hwnd(h), GWL_EXSTYLE) as u32;
        if ex & WS_EX_TOOLWINDOW == 0 {
            let dx = screen.working_area.x - screen.bounds.x;
            let dy = screen.working_area.y - screen.bounds.y;
            return Rect::new(r.x + dx, r.y + dy, r.width, r.height);
        }
        r
    }
}

/// CPU drawing's browser profile, left from a switch back to the GPU (about 11 MB): gone at the next start, made
/// again if CPU drawing comes back. Best effort; what is held is tried next start.
fn delete_safe_profile() {
    let dir = vault::cache_dir().join("webview-safe");
    if dir.is_dir() {
        let _ = std::fs::remove_dir_all(dir);
    }
}

fn try_mark_starting(starting: bool) {
    // An IOException or UnauthorizedAccessException: left be.
    if starting {
        let _ = std::fs::create_dir_all(vault::data_dir());
        let _ = std::fs::write(start_marker(), "");
    } else {
        let _ = std::fs::remove_file(start_marker());
    }
}
