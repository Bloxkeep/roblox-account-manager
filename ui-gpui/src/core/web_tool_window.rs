// WebToolWindow.cs: a small borderless window whose whole face is one of the app's pages (wwwroot/<page>.html, drawing
// window.BloxkeepUI.<page>(...) from its own ui-<page>.js bundle, ui/build.mjs): the macro panel beside the game today,
// the mini window (mini_window), the game overlay and colour picker (overlay_window), the dialogs (forms::dialog);
// tray_window still has its own copy of this host (the owner's rule of 2026-11-03: nothing the user sees drawn in
// WinForms). The window is only the host: a tool window (not in Alt+Tab), Windows 11's rounded corners in the theme's
// line colour, the browser of the main window's profile (one shared environment, so one browser and GPU process while
// the main window's is alive) started on first show and let go a while after the window hides.
//
// The page and the host talk in the app page's own messages: the page posts { cmd, args } ("ready", "size" with its
// size in CSS pixels, "act" with what was clicked, "drag" to move the window, "resize" to make it taller or shorter by
// its bottom edge); the host posts { type: "event", name, data }. The window takes the page's size times its monitor's
// scale.
//
// A C# subclass (MiniWindow, OverlayWindow, Dialog, PanelWindow) is a struct holding a `WebToolWindow` and implementing
// `WebTool` (its overrides); the base calls the subclass through that trait, as the C# virtual methods did.
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    sync::atomic::Ordering,
    time::Duration,
};

use futures::{
    FutureExt,
    channel::oneshot,
    future::{LocalBoxFuture, Shared},
};
use serde_json::{Value, json};
use windows_sys::Win32::{
    Foundation::RECT,
    Graphics::Gdi::{CreateSolidBrush, DeleteObject, FillRect, HDC},
    UI::{
        Input::KeyboardAndMouse::ReleaseCapture,
        WindowsAndMessaging::{
            CS_DROPSHADOW, DefWindowProcW, GetClientRect, HTBOTTOM, HTCAPTION, MA_ACTIVATE, SendMessageW, WA_INACTIVE, WM_ACTIVATE, WM_CLOSE, WM_DESTROY, WM_DPICHANGED,
            WM_ENDSESSION, WM_ERASEBKGND, WM_GETMINMAXINFO, WM_MOUSEACTIVATE, WM_NCLBUTTONDOWN, WM_QUERYENDSESSION, WM_SIZE, WS_CLIPCHILDREN, WS_CLIPSIBLINGS,
            WS_EX_CONTROLPARENT, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
        },
    },
};

use super::{
    app_pages,
    base::{Cancel, Fail, Res},
    log, shell_form, themes,
    themes::Color,
    ui,
    win_form::{self, CloseReason, FormRef, Timer, colorref},
    win_kit::{Point, Rect, Size, hwnd},
};

/// The overrides of a page's window (the C# virtual and abstract members).
pub trait WebTool: 'static {
    /// The window's base part.
    fn tool(&self) -> &WebToolWindow;
    /// GetType().Name, for error.log.
    fn type_name(&self) -> &'static str;
    /// The page's file in wwwroot ("panel.html").
    fn page_file(&self) -> &'static str;
    /// Always on top, set in the window's style from its first moment (setting TopMost before Show activates it).
    fn starts_top_most(&self) -> bool {
        false
    }
    /// How long the browser is kept after the window hid; shown again meanwhile, it is there at once.
    fn keep_browser(&self) -> Duration {
        Duration::from_secs(60)
    }
    /// While true the browser is kept even though the window is hidden.
    fn hold_browser(&self) -> bool {
        false
    }
    /// Windows 11's rounded corners and the hairline in the theme's line colour (not over a game: the overlay is square).
    fn rounded(&self) -> bool {
        true
    }
    /// Windows' shadow under the window.
    fn shadow(&self) -> bool {
        true
    }
    /// The colour shown before the page draws, and under a page that is see-through.
    fn backdrop(&self) -> Color {
        themes::current().surface
    }
    /// A click on the page takes the keyboard (it needs it for typing, keys and Esc).
    fn activate_on_click(&self) -> bool {
        true
    }
    /// The page has loaded (also again after a crash): push what it draws.
    fn on_page_ready(&self) {}
    /// The page's size changed (CSS pixels); `args` has the rest of what it said.
    fn on_page_size(&self, _css: Size, _args: &Value) {}
    /// Something clicked on the page.
    fn on_act(&self, what: &str, args: &Value);
    /// The page crashed: hidden, and started again on the next show.
    fn on_browser_failed(&self) {
        self.tool().browser_failed_base(self.type_name());
    }
    /// The person moved the window by its page (the move has ended).
    fn on_dragged(&self) {}
    /// The person sized the window by its page (the sizing has ended).
    fn on_resized(&self) {}
    /// Dragged onto a monitor of another scale.
    fn on_dpi_changed(&self, new_dpi: u32, suggested: Rect) {
        self.tool().dpi_changed_base(new_dpi, suggested);
    }
    /// Shown or hidden.
    fn on_visible_changed(&self) {
        self.tool().visible_changed_base();
    }
    /// The window got the keyboard.
    fn on_activated(&self) {}
    /// The window lost the keyboard.
    fn on_deactivate(&self) {}
    /// FormClosing: true cancels the close.
    fn on_form_closing(&self, _reason: CloseReason) -> bool {
        false
    }
    /// FormClosed.
    fn on_form_closed(&self) {}
    /// Dispose(true)'s own part, before the base's.
    fn on_disposing(&self) {}
}

/// TaskCompletionSource for "the page answered".
struct Ready {
    done: Cell<bool>,
    tx: RefCell<Option<oneshot::Sender<()>>>,
}

impl Ready {
    fn new() -> (Rc<Ready>, oneshot::Receiver<()>) {
        let (tx, rx) = oneshot::channel();
        (Rc::new(Ready { done: Cell::new(false), tx: RefCell::new(Some(tx)) }), rx)
    }
    fn try_set_result(&self) {
        self.done.set(true);
        if let Some(tx) = self.tx.borrow_mut().take() {
            let _ = tx.send(());
        }
    }
}

type Starting = Shared<LocalBoxFuture<'static, Res<()>>>;

/// The base part of a page's window.
pub struct WebToolWindow {
    me: RefCell<Option<Weak<dyn WebTool>>>,
    form: FormRef,
    release: Timer,
    web: RefCell<Option<Rc<browser::Browser>>>,
    /// Which browser start is the one that counts (0: none, let go).
    web_gen: Cell<u64>,
    next_gen: Cell<u64>,
    starting: RefCell<Option<(u64, Starting)>>,
    start_cancel: RefCell<Option<Cancel>>,
    ready: RefCell<Option<Rc<Ready>>>,
    activate: Cell<bool>,
    /// The page's size in its own (CSS) pixels, as it last said.
    page: Cell<Size>,
    text: RefCell<String>,
    back: Cell<Color>,
    /// Bounds set before the window had a handle.
    pending: Cell<Rect>,
    opacity: Cell<f64>,
    minimum_size: Cell<Size>,
    maximum_size: Cell<Size>,
}

impl Default for WebToolWindow {
    fn default() -> Self {
        WebToolWindow::new()
    }
}

impl WebToolWindow {
    pub fn new() -> WebToolWindow {
        WebToolWindow {
            me: RefCell::new(None),
            form: FormRef::default(),
            release: Timer::new(60_000),
            web: RefCell::new(None),
            web_gen: Cell::new(0),
            next_gen: Cell::new(1),
            starting: RefCell::new(None),
            start_cancel: RefCell::new(None),
            ready: RefCell::new(None),
            activate: Cell::new(true),
            page: Cell::new(Size::default()),
            // Text = Ui.Title.
            text: RefCell::new(ui::TITLE.to_string()),
            back: Cell::new(themes::current().surface),
            // WinForms' default place and size.
            pending: Cell::new(Rect::new(0, 0, 300, 300)),
            opacity: Cell::new(1.0),
            minimum_size: Cell::new(Size::default()),
            maximum_size: Cell::new(Size::default()),
        }
    }

    /// Ties the base to the window that holds it (the constructor's `this`); once, right after Rc::new.
    pub fn bind(&self, me: Weak<dyn WebTool>) {
        if let Some(owner) = me.upgrade() {
            self.back.set(owner.backdrop());
            let weak = me.clone();
            self.release.on_tick(move || {
                if let Some(o) = weak.upgrade() {
                    o.tool().release_browser(false);
                }
            });
        }
        *self.me.borrow_mut() = Some(me);
    }

    fn owner(&self) -> Option<Rc<dyn WebTool>> {
        self.me.borrow().as_ref().and_then(|w| w.upgrade())
    }

    /// What Ui.NoteOnTop keeps of this window.
    pub fn form_ref(&self) -> FormRef {
        self.form.clone()
    }

    pub fn is_disposed(&self) -> bool {
        self.form.is_disposed()
    }

    pub fn is_handle_created(&self) -> bool {
        self.form.handle() != 0
    }

    /// Text = …
    pub fn set_text(&self, text: &str) {
        *self.text.borrow_mut() = text.to_string();
        let h = self.form.handle();
        if h != 0 {
            win_form::set_text(h, text);
        }
    }

    /// Opacity = …
    pub fn set_opacity(&self, opacity: f64) {
        self.opacity.set(opacity);
        let h = self.form.handle();
        if h != 0 {
            win_form::set_opacity(h, opacity);
        }
    }

    /// The window's handle, made now when it has none (`Handle`).
    pub fn handle(&self) -> isize {
        let h = self.form.handle();
        if h != 0 || self.is_disposed() {
            return h;
        }
        let Some(owner) = self.owner() else { return 0 };
        // CreateParams: not in Alt+Tab, on top from the start when asked, Windows' shadow as the class's.
        let mut ex = WS_EX_TOOLWINDOW | WS_EX_CONTROLPARENT;
        if owner.starts_top_most() {
            ex |= WS_EX_TOPMOST;
        }
        let (class, class_style) = if owner.shadow() { ("Bloxkeep.ToolShadow", CS_DROPSHADOW) } else { ("Bloxkeep.Tool", 0) };
        let weak = self.me.borrow().clone().expect("bound");
        let handler: win_form::Handler = Rc::new(move |h, msg, wparam, lparam| {
            let owner = weak.upgrade()?;
            owner.tool().message(&owner, h, msg, wparam, lparam)
        });
        let text = self.text.borrow().clone();
        let Ok(h) = win_form::create_window(class, class_style, WS_POPUP | WS_CLIPCHILDREN | WS_CLIPSIBLINGS, ex, self.pending.get(), 0, &text, handler) else {
            log::note(&format!("{}: its window couldn't be made.", owner.type_name()));
            return 0;
        };
        self.form.hwnd.store(h, Ordering::SeqCst);
        // Icon = Ui.AppIcon.
        if let Some(icon) = ui::app_icon() {
            win_form::set_icon(h, icon);
        }
        if self.opacity.get() < 1.0 {
            win_form::set_opacity(h, self.opacity.get());
        }
        // OnHandleCreated.
        self.frame_with(&*owner);
        h
    }

    fn message(&self, owner: &Rc<dyn WebTool>, h: isize, msg: u32, wparam: usize, lparam: isize) -> Option<isize> {
        match msg {
            WM_ERASEBKGND => {
                // BackColor painted under the page.
                // SAFETY: the DC is Windows' for this message; the brush is made and freed here.
                unsafe {
                    let mut r: RECT = std::mem::zeroed();
                    GetClientRect(hwnd(h), &mut r);
                    let brush = CreateSolidBrush(colorref(self.back.get()));
                    FillRect(wparam as HDC, &r, brush);
                    DeleteObject(brush);
                }
                Some(1)
            }
            WM_SIZE => {
                // Dock = Fill.
                let web = self.web.borrow().clone();
                if let Some(web) = web {
                    web.set_bounds(win_form::client_size(h));
                }
                None
            }
            WM_MOUSEACTIVATE => {
                // SAFETY: the default handling first, as base.WndProc.
                let r = unsafe { DefWindowProcW(hwnd(h), msg, wparam, lparam) };
                Some(if owner.activate_on_click() { MA_ACTIVATE as isize } else { r })
            }
            WM_ACTIVATE => {
                if (wparam & 0xFFFF) as u32 != WA_INACTIVE {
                    owner.on_activated();
                } else {
                    owner.on_deactivate();
                }
                None
            }
            WM_DPICHANGED => {
                owner.on_dpi_changed(((wparam >> 16) & 0xFFFF) as u32, win_form::suggested_rect(lparam));
                Some(0)
            }
            WM_GETMINMAXINFO => {
                win_form::apply_min_max(lparam, self.minimum_size.get(), self.maximum_size.get());
                Some(0)
            }
            WM_CLOSE => {
                self.close_with(CloseReason::UserClosing);
                Some(0)
            }
            WM_QUERYENDSESSION => Some(1),
            WM_ENDSESSION if wparam != 0 => {
                self.close_with(CloseReason::WindowsShutDown);
                Some(0)
            }
            WM_DESTROY => {
                self.form.hwnd.store(0, Ordering::SeqCst);
                None
            }
            _ => None,
        }
    }

    // ───────────────────────── a Form's members ─────────────────────────

    pub fn visible(&self) -> bool {
        win_form::is_visible(self.form.handle())
    }

    /// Show(): without the keyboard when ActivateOnShow says so (a game keeps it).
    pub fn show(&self) {
        let h = self.handle();
        if h == 0 || self.visible() {
            return;
        }
        win_form::show(h, self.activate.get());
        if let Some(o) = self.owner() {
            o.on_visible_changed();
        }
    }

    pub fn hide(&self) {
        let h = self.form.handle();
        if h == 0 || !self.visible() {
            return;
        }
        win_form::hide(h);
        if let Some(o) = self.owner() {
            o.on_visible_changed();
        }
    }

    /// Activate().
    pub fn activate(&self) {
        let h = self.form.handle();
        if h != 0 {
            win_form::activate(h);
        }
    }

    /// Close(): FormClosing (which may cancel), FormClosed, then the window is disposed. A window never made is only disposed.
    pub fn close(&self) {
        if self.form.handle() == 0 {
            self.dispose();
            return;
        }
        self.close_with(CloseReason::UserClosing);
    }

    fn close_with(&self, reason: CloseReason) {
        if self.is_disposed() {
            return;
        }
        let Some(owner) = self.owner() else { return };
        if owner.on_form_closing(reason) {
            return;
        }
        let h = self.form.handle();
        if h != 0 {
            win_form::hide(h);
        }
        owner.on_form_closed();
        self.dispose();
    }

    /// Dispose(): the subclass's part, then the base's; the window goes without FormClosing or FormClosed.
    pub fn dispose(&self) {
        if self.form.disposed.swap(true, Ordering::SeqCst) {
            return;
        }
        if let Some(owner) = self.owner() {
            owner.on_disposing();
        }
        self.release.dispose();
        let web = self.web.borrow_mut().take();
        if let Some(web) = web {
            web.close();
        }
        self.web_gen.set(0);
        *self.starting.borrow_mut() = None;
        if let Some(c) = self.start_cancel.borrow_mut().take() {
            c.cancel();
        }
        let h = self.form.hwnd.swap(0, Ordering::SeqCst);
        win_form::destroy(h);
    }

    /// Bounds.
    pub fn bounds(&self) -> Rect {
        let h = self.form.handle();
        if h != 0 { win_form::bounds(h) } else { self.pending.get() }
    }

    /// Bounds = r.
    pub fn set_bounds(&self, r: Rect) {
        let h = self.form.handle();
        if h != 0 {
            win_form::set_bounds(h, r);
        } else {
            self.pending.set(r);
        }
    }

    /// Location = p.
    pub fn set_location(&self, p: Point) {
        let h = self.form.handle();
        if h != 0 {
            win_form::set_location(h, p);
        } else {
            let r = self.pending.get();
            self.pending.set(Rect::new(p.x, p.y, r.width, r.height));
        }
    }

    /// Size = s.
    pub fn set_size(&self, s: Size) {
        let h = self.form.handle();
        if h != 0 {
            win_form::set_size(h, s);
        } else {
            let r = self.pending.get();
            self.pending.set(Rect::new(r.x, r.y, s.width, s.height));
        }
    }

    pub fn set_minimum_size(&self, s: Size) {
        self.minimum_size.set(s);
    }

    pub fn set_maximum_size(&self, s: Size) {
        self.maximum_size.set(s);
    }

    /// The page's last size in CSS pixels (empty until it has said).
    pub fn page_size(&self) -> Size {
        self.page.get()
    }

    /// The page has started and answered.
    pub fn browser_ready(&self) -> bool {
        self.web.borrow().is_some() && self.ready.borrow().as_ref().is_some_and(|r| r.done.get())
    }

    /// Whether the next show takes the keyboard.
    pub fn activate_on_show(&self) -> bool {
        self.activate.get()
    }

    pub fn set_activate_on_show(&self, value: bool) {
        self.activate.set(value);
    }

    // ───────────────────────── the browser ─────────────────────────

    /// Starts the page (once); false when it couldn't (logged; the window stays hidden) or the window went meanwhile.
    pub async fn ensure_browser(me: Rc<dyn WebTool>) -> bool {
        let t = me.tool();
        t.release.stop();
        while !t.browser_ready() {
            if t.is_disposed() {
                return false;
            }
            if t.starting.borrow().is_none() {
                let cancel = Cancel::new();
                *t.start_cancel.borrow_mut() = Some(cancel.clone());
                let id = t.next_gen.get();
                t.next_gen.set(id + 1);
                let fut: LocalBoxFuture<'static, Res<()>> = WebToolWindow::start_browser(me.clone(), cancel).boxed_local();
                *t.starting.borrow_mut() = Some((id, fut.shared()));
            }
            let Some((id, starting)) = t.starting.borrow().clone() else { return false };
            let result = starting.await;
            let current = t.starting.borrow().as_ref().map(|(i, _)| *i);
            if let Err(e) = result {
                if current == Some(id) {
                    log::write(&e);
                    t.release_browser(true);
                    return false;
                }
            }
            // Let go while it started (the window disposed, its page failed): the start that came after, if any, is the
            // one that counts. Never this one's failure taking that one's browser down with it.
            if current != Some(id) {
                if current.is_none() {
                    return false;
                }
                continue;
            }
            if !t.browser_ready() {
                log::note(&format!("{}: its page started but never said it was ready; the window stays hidden.", me.type_name()));
                *t.starting.borrow_mut() = None;
                return false;
            }
        }
        true
    }

    /// `cancel`: called off when the window lets this start go (`release_browser`, Dispose).
    async fn start_browser(me: Rc<dyn WebTool>, cancel: Cancel) -> Res<()> {
        let env = win_form::wait_for(app_pages::create_environment(), u32::MAX, &cancel).await??;
        let t = me.tool();
        if t.is_disposed() {
            return Err(Fail::other(format!("Cannot access a disposed object. Object name: '{}'.", me.type_name())));
        }
        let generation = t.next_gen.get();
        t.next_gen.set(generation + 1);
        t.web_gen.set(generation);
        // The browser starts once its control has a window, and a hidden form's controls get none by themselves.
        let h = t.handle();
        let web = win_form::wait_for(browser::Browser::create(h, &env, generation), u32::MAX, &cancel).await??;
        let web = Rc::new(web);
        // Disposed or let go while the browser started: nothing of this start is used.
        if t.is_disposed() || t.web_gen.get() != generation {
            web.close();
            return Err(Fail::other(format!("Cannot access a disposed object. Object name: '{}'.", me.type_name())));
        }
        web.set_background(me.backdrop());
        web.set_bounds(win_form::client_size(h));
        *t.web.borrow_mut() = Some(web.clone());
        app_pages::configure(web.core(), &env)?;
        let weak = Rc::downgrade(&me);
        // Loaded: its first "ready" can come before the page is listened to, so the load itself counts as ready too.
        web.on_navigation_completed({
            let weak = weak.clone();
            move |ok| {
                if !ok {
                    return;
                }
                if let Some(o) = weak.upgrade() {
                    let ready = o.tool().ready.borrow().clone();
                    if let Some(r) = ready {
                        r.try_set_result();
                    }
                }
            }
        });
        // Its page gone (the renderer crashed or hung, the browser gone): the next show starts it again. Not for a GPU
        // process restart, which the page lives through (shell_form::failure_of).
        web.on_process_failed({
            let weak = weak.clone();
            move |kind| {
                let failure = shell_form::failure_of(kind);
                if !matches!(failure, shell_form::BrowserFailure::ReloadPage | shell_form::BrowserFailure::BrowserGone) {
                    return;
                }
                let Some(o) = weak.upgrade() else { return };
                if !o.tool().is_handle_created() || o.tool().is_disposed() {
                    return;
                }
                let weak = weak.clone();
                win_form::spawn(async move {
                    if let Some(o) = weak.upgrade() {
                        if o.tool().web_gen.get() == generation {
                            o.on_browser_failed();
                        }
                    }
                });
            }
        });
        web.on_web_message({
            let weak = weak.clone();
            move |json| {
                let weak = weak.clone();
                win_form::spawn(async move {
                    if let Some(o) = weak.upgrade() {
                        o.tool().on_message(&o, &json);
                    }
                });
            }
        });
        let (ready, rx) = Ready::new();
        *t.ready.borrow_mut() = Some(ready);
        web.navigate(&format!("{}{}", app_pages::ORIGIN, me.page_file()))?;
        win_form::wait_for(rx, 15_000, &cancel).await?.map_err(|_| Fail::other("cancelled"))?;
        Ok(())
    }

    /// The page crashed: hidden, and started again on the next show.
    pub fn browser_failed_base(&self, type_name: &str) {
        log::note(&format!(
            "{type_name}: its page crashed; hidden ({}), started again on the next show.",
            if self.visible() { "was shown" } else { "was hidden" }
        ));
        self.hide();
        self.release_browser(true);
    }

    /// The browser goes (its memory with it) unless the window is shown or holds it; the next show starts it again.
    /// A start under way is let go too.
    pub fn release_browser(&self, force: bool) {
        self.release.stop();
        let hold = self.owner().is_some_and(|o| o.hold_browser());
        if !force && (self.visible() || hold) {
            return;
        }
        *self.ready.borrow_mut() = None;
        *self.starting.borrow_mut() = None;
        let cancel = self.start_cancel.borrow_mut().take();
        self.web_gen.set(0);
        let web = self.web.borrow_mut().take();
        if let Some(web) = web {
            web.close();
        }
        if let Some(c) = cancel {
            c.cancel();
        }
    }

    /// OnVisibleChanged's base part.
    pub fn visible_changed_base(&self) {
        // The browser follows its window, as the WinForms WebView2 control did (it set IsVisible on VisibleChanged):
        // shown, it is visible and awake; hidden, it is told so (the sleep below needs that). Without it the mini
        // window could come back as an empty dark box (owner, 2026-11-12): put to sleep while it was hidden, nothing
        // woke the page when the window showed again.
        let web = self.web.borrow().clone();
        if let Some(web) = &web {
            web.set_visible(self.visible());
            if self.visible() && web.is_suspended() {
                if let Err(e) = web.resume() {
                    log::write(&e);
                }
            }
        }
        if self.visible() {
            self.release.stop();
        } else if web.is_some() && !self.is_disposed() {
            let Some(owner) = self.owner() else { return };
            self.release.set_interval(owner.keep_browser().as_millis().max(1000) as u32);
            self.release.start();
            win_form::spawn(WebToolWindow::sleep(owner));
        }
    }

    /// A hidden page that is kept a while sleeps meanwhile (TrySuspend: its timers and drawing stop), as the main
    /// page does. Showing it wakes it; so does an event sent to it (`send_event`), and it then sleeps again.
    async fn sleep(me: Rc<dyn WebTool>) {
        if me.keep_browser() < Duration::from_secs(2) {
            return; // let go almost at once anyway
        }
        // The browser hears it is hidden on a later turn of the message loop; asked to sleep before that, it refuses.
        win_form::delay(300).await;
        let t = me.tool();
        let web = t.web.borrow().clone();
        let Some(web) = web else { return };
        if t.visible() || t.is_disposed() || web.is_suspended() {
            return;
        }
        // Shown again meanwhile, or the browser went: nothing to put to sleep.
        let _ = web.try_suspend().await;
        // Shown while it went to sleep: woken at once (the show's own wake came before the sleep took).
        if t.visible() && web.is_suspended() {
            web.set_visible(true);
            if let Err(e) = web.resume() {
                log::write(&e);
            }
        }
    }

    /// The browser is let go after keep_browser again (after hold_browser turned false).
    pub fn release_later(&self) {
        if !self.visible() && self.web.borrow().is_some() && !self.is_disposed() {
            let Some(owner) = self.owner() else { return };
            self.release.set_interval(owner.keep_browser().as_millis().max(1000) as u32);
            self.release.start();
        }
    }

    /// The page takes the keyboard (the window was activated: its keys go to the page, not the window).
    pub fn focus_page(&self) {
        let web = self.web.borrow().clone();
        if let Some(web) = web {
            web.focus();
        }
    }

    // ───────────────────────── messages ─────────────────────────

    /// An event to the page, as the app's page gets them: { type: "event", name, data }.
    pub fn send_event(&self, name: &str, data: Value) {
        let web = self.web.borrow().clone();
        let Some(web) = web else { return };
        let result = (|| -> Res<()> {
            if web.is_suspended() {
                web.resume()?;
                if !self.visible() {
                    if let Some(o) = self.owner() {
                        win_form::spawn(WebToolWindow::sleep(o));
                    }
                }
            }
            web.post_json(&json!({ "type": "event", "name": name, "data": data }).to_string())
        })();
        if let Err(e) = result {
            log::write(&e);
        }
    }

    /// The page's messages: { id, cmd, args }.
    fn on_message(&self, owner: &Rc<dyn WebTool>, json: &str) {
        if self.is_disposed() {
            return;
        }
        let Ok(root) = serde_json::from_str::<Value>(json) else {
            log::note(&format!("{}: a message from its page couldn't be read.", owner.type_name()));
            return;
        };
        let cmd = root.get("cmd").and_then(Value::as_str).unwrap_or("");
        let args = root.get("args").filter(|a| a.is_object()).cloned().unwrap_or(Value::Null);
        match cmd {
            "ready" => {
                let ready = self.ready.borrow().clone();
                if let Some(r) = ready {
                    r.try_set_result();
                }
                owner.on_page_ready();
            }
            "size" if args.is_object() => {
                let (w, h) = (num(&args, "w"), num(&args, "h"));
                if w > 0 && h > 0 {
                    self.page.set(Size::new(w, h));
                    owner.on_page_size(self.page.get(), &args);
                }
            }
            "drag" => self.begin_drag(owner),
            "resize" => self.begin_resize(owner),
            "act" if args.is_object() => owner.on_act(&str_of(&args, "what"), &args),
            _ => {}
        }
    }

    // ───────────────────────── moving and sizing ─────────────────────────

    /// The page's title row pressed and moved: Windows moves the window with the pointer, as by a title bar. (Not CSS
    /// app-region: a drag region swallows the row's right-click, which opens the page's own menu, and a double-click on
    /// it would maximise the borderless window.)
    fn begin_drag(&self, owner: &Rc<dyn WebTool>) {
        if !self.visible() || !win_form::left_button_down() {
            return;
        }
        // SAFETY: plain calls; the move loop runs inside SendMessage until the button is let go.
        unsafe {
            ReleaseCapture();
            SendMessageW(hwnd(self.form.handle()), WM_NCLBUTTONDOWN, HTCAPTION as usize, 0);
        }
        owner.on_dragged();
    }

    /// The page's bottom grip pressed: Windows sizes the window by its bottom edge with the pointer, as by a sizing border
    /// (the window has none: its page fills it). The window's minimum and maximum size hold meanwhile.
    fn begin_resize(&self, owner: &Rc<dyn WebTool>) {
        if !self.visible() || !win_form::left_button_down() {
            return;
        }
        // SAFETY: as begin_drag.
        unsafe {
            ReleaseCapture();
            SendMessageW(hwnd(self.form.handle()), WM_NCLBUTTONDOWN, HTBOTTOM as usize, 0);
        }
        owner.on_resized();
    }

    /// The scale of the monitor the window is on now.
    pub fn scale_now(&self) -> f32 {
        if self.is_handle_created() { win_form::device_dpi(self.form.handle()) as f32 / 96.0 } else { scale_at(self.bounds().location()) }
    }

    /// Dragged onto a monitor of another scale: Windows' suggested place is taken (the base does that), then only the size
    /// is set again from the page's size at the new scale. Never the current size times the ratio: with the page's own
    /// size kept apart, a size set twice can't grow or shrink by the ratio twice (TrayWindow's 2026-10-30 lesson).
    pub fn dpi_changed_base(&self, new_dpi: u32, suggested: Rect) {
        self.set_bounds(suggested);
        let page = self.page.get();
        if page.width > 0 {
            self.set_size(scaled(page, new_dpi as f32 / 96.0));
        }
    }

    // ───────────────────────── look ─────────────────────────

    /// Windows 11's rounded corners and a border in the theme's line colour; the colour shown before the page draws.
    pub fn frame(&self) {
        if let Some(owner) = self.owner() {
            self.frame_with(&*owner);
        }
    }

    fn frame_with(&self, owner: &dyn WebTool) {
        let back = owner.backdrop();
        self.back.set(back);
        let web = self.web.borrow().clone();
        if let Some(web) = web {
            web.set_background(back);
        }
        let h = self.form.handle();
        if h != 0 {
            win_form::invalidate(h);
            if owner.rounded() {
                ui::round_corners(h, &themes::current());
            }
        }
    }
}

pub fn num(o: &Value, name: &str) -> i32 {
    o.get(name).and_then(Value::as_f64).map(|v| v.ceil() as i32).unwrap_or(0)
}

pub fn str_of(o: &Value, name: &str) -> String {
    o.get(name).and_then(Value::as_str).unwrap_or("").to_string()
}

/// A size in CSS pixels as screen pixels on a monitor of this scale.
pub fn scaled(css: Size, scale: f32) -> Size {
    Size::new((css.width as f32 * scale).ceil() as i32, (css.height as f32 * scale).ceil() as i32)
}

/// The scale (1 = 96 DPI) of the monitor nearest a point.
pub fn scale_at(p: Point) -> f32 {
    dpi_at(p) as f32 / 96.0
}

/// The DPI of the monitor nearest a point (96 when Windows can't say).
pub fn dpi_at(p: Point) -> u32 {
    win_form::dpi_at(p)
}

/// One browser on a window of ours (the WebView2 WinForms control's part): webview2-com's controller and its page.
pub mod browser {
    use std::cell::RefCell;

    use futures::channel::oneshot;
    use webview2_com::{
        CreateCoreWebView2ControllerCompletedHandler, Microsoft::Web::WebView2::Win32::*, NavigationCompletedEventHandler, ProcessFailedEventHandler,
        TrySuspendCompletedHandler, WebMessageReceivedEventHandler, take_pwstr,
    };
    use windows::{
        Win32::Foundation::{HWND, RECT},
        core::{BOOL, HSTRING, Interface, PCWSTR, PWSTR},
    };

    use crate::core::{
        base::{Fail, Res},
        themes::Color,
        win_kit::Size,
    };

    fn fail(e: windows::core::Error) -> Fail {
        Fail::other(e.message())
    }

    pub struct Browser {
        controller: ICoreWebView2Controller,
        core: ICoreWebView2,
        /// Which start made it (to tell two browsers of one window apart).
        pub id: u64,
        tokens: RefCell<Vec<i64>>,
    }

    impl Browser {
        /// EnsureCoreWebView2Async on `parent` with the shared environment.
        pub async fn create(parent: isize, env: &ICoreWebView2Environment, id: u64) -> Res<Browser> {
            let (tx, rx) = oneshot::channel::<Res<ICoreWebView2Controller>>();
            let tx = RefCell::new(Some(tx));
            let handler = CreateCoreWebView2ControllerCompletedHandler::create(Box::new(move |result: windows::core::Result<()>, controller: Option<ICoreWebView2Controller>| {
                let out = match (result, controller) {
                    (Ok(()), Some(c)) => Ok(c),
                    (Err(e), _) => Err(fail(e)),
                    (Ok(()), None) => Err(Fail::other("WebView2 gave no controller")),
                };
                if let Some(tx) = tx.borrow_mut().take() {
                    let _ = tx.send(out);
                }
                Ok(())
            }));
            // SAFETY: a COM call on the windows thread (STA); the handler is kept alive by COM until it is called.
            unsafe { env.CreateCoreWebView2Controller(HWND(parent as *mut _), &handler) }.map_err(fail)?;
            let controller = rx.await.map_err(|_| Fail::other("WebView2 never answered"))??;
            // SAFETY: a plain COM getter.
            let core = unsafe { controller.CoreWebView2() }.map_err(fail)?;
            Ok(Browser { controller, core, id, tokens: RefCell::new(Vec::new()) })
        }

        pub fn core(&self) -> &ICoreWebView2 {
            &self.core
        }

        /// Dock = Fill: the browser over the whole client area.
        pub fn set_bounds(&self, size: Size) {
            // SAFETY: plain COM call.
            let _ = unsafe { self.controller.SetBounds(RECT { left: 0, top: 0, right: size.width, bottom: size.height }) };
        }

        pub fn set_visible(&self, visible: bool) {
            // SAFETY: plain COM call.
            let _ = unsafe { self.controller.SetIsVisible(visible.into()) };
        }

        /// DefaultBackgroundColor.
        pub fn set_background(&self, c: Color) {
            if let Ok(c2) = self.controller.cast::<ICoreWebView2Controller2>() {
                // SAFETY: plain COM call.
                let _ = unsafe { c2.SetDefaultBackgroundColor(COREWEBVIEW2_COLOR { A: 255, R: c.r, G: c.g, B: c.b }) };
            }
        }

        pub fn navigate(&self, url: &str) -> Res<()> {
            let url = HSTRING::from(url);
            // SAFETY: the string lives for the call.
            unsafe { self.core.Navigate(PCWSTR(url.as_ptr())) }.map_err(fail)
        }

        /// PostWebMessageAsJson.
        pub fn post_json(&self, json: &str) -> Res<()> {
            let json = HSTRING::from(json);
            // SAFETY: the string lives for the call.
            unsafe { self.core.PostWebMessageAsJson(PCWSTR(json.as_ptr())) }.map_err(fail)
        }

        /// Focus(): the keyboard to the page.
        pub fn focus(&self) {
            // SAFETY: plain COM call.
            let _ = unsafe { self.controller.MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC) };
        }

        /// IsSuspended (false where the runtime can't say).
        pub fn is_suspended(&self) -> bool {
            let Ok(c3) = self.core.cast::<ICoreWebView2_3>() else { return false };
            let mut b = BOOL::default();
            // SAFETY: plain COM getter into a BOOL on the stack.
            unsafe { c3.IsSuspended(&mut b) }.is_ok() && b.as_bool()
        }

        /// TrySuspendAsync: whether it went to sleep.
        pub async fn try_suspend(&self) -> Res<bool> {
            let c3 = self.core.cast::<ICoreWebView2_3>().map_err(fail)?;
            let (tx, rx) = oneshot::channel::<Res<bool>>();
            let tx = RefCell::new(Some(tx));
            let handler = TrySuspendCompletedHandler::create(Box::new(move |result: windows::core::Result<()>, ok: bool| {
                if let Some(tx) = tx.borrow_mut().take() {
                    let _ = tx.send(result.map(|_| ok).map_err(fail));
                }
                Ok(())
            }));
            // SAFETY: COM call on the windows thread; the handler is kept by COM until called.
            unsafe { c3.TrySuspend(&handler) }.map_err(fail)?;
            rx.await.map_err(|_| Fail::other("cancelled"))?
        }

        pub fn resume(&self) -> Res<()> {
            let c3 = self.core.cast::<ICoreWebView2_3>().map_err(fail)?;
            // SAFETY: plain COM call.
            unsafe { c3.Resume() }.map_err(fail)
        }

        /// NavigationCompleted: whether it loaded.
        pub fn on_navigation_completed(&self, f: impl Fn(bool) + 'static) {
            let handler = NavigationCompletedEventHandler::create(Box::new(move |_sender, args: Option<ICoreWebView2NavigationCompletedEventArgs>| {
                let mut ok = BOOL::default();
                if let Some(args) = args {
                    // SAFETY: plain COM getter.
                    let _ = unsafe { args.IsSuccess(&mut ok) };
                }
                f(ok.as_bool());
                Ok(())
            }));
            let mut token = 0i64;
            // SAFETY: COM registration; the token is kept so nothing else needs it.
            if unsafe { self.core.add_NavigationCompleted(&handler, &mut token) }.is_ok() {
                self.tokens.borrow_mut().push(token);
            }
        }

        /// ProcessFailed: which process.
        pub fn on_process_failed(&self, f: impl Fn(COREWEBVIEW2_PROCESS_FAILED_KIND) + 'static) {
            let handler = ProcessFailedEventHandler::create(Box::new(move |_sender, args: Option<ICoreWebView2ProcessFailedEventArgs>| {
                if let Some(args) = args {
                    let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND::default();
                    // SAFETY: plain COM getter.
                    if unsafe { args.ProcessFailedKind(&mut kind) }.is_ok() {
                        f(kind);
                    }
                }
                Ok(())
            }));
            let mut token = 0i64;
            // SAFETY: COM registration.
            if unsafe { self.core.add_ProcessFailed(&handler, &mut token) }.is_ok() {
                self.tokens.borrow_mut().push(token);
            }
        }

        /// WebMessageReceived: the message as JSON (WebMessageAsJson).
        pub fn on_web_message(&self, f: impl Fn(String) + 'static) {
            let handler = WebMessageReceivedEventHandler::create(Box::new(move |_sender, args: Option<ICoreWebView2WebMessageReceivedEventArgs>| {
                if let Some(args) = args {
                    let mut json = PWSTR::null();
                    // SAFETY: COM getter; the string is freed by take_pwstr.
                    if unsafe { args.WebMessageAsJson(&mut json) }.is_ok() {
                        f(take_pwstr(json));
                    }
                }
                Ok(())
            }));
            let mut token = 0i64;
            // SAFETY: COM registration.
            if unsafe { self.core.add_WebMessageReceived(&handler, &mut token) }.is_ok() {
                self.tokens.borrow_mut().push(token);
            }
        }

        /// web.Dispose(): the browser closes.
        pub fn close(&self) {
            // SAFETY: plain COM call; nothing uses the controller afterwards.
            let _ = unsafe { self.controller.Close() };
        }
    }
}
