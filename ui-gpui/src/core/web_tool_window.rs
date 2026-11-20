// WebToolWindow.cs: a small borderless window whose whole face is one of the app's pages: the macro panel beside the
// game today, the mini window (mini_window), the game overlay and colour picker (overlay_window) (the owner's rule of
// 2026-11-03: nothing the user sees drawn in WinForms). The window is only the host: a tool window (not in Alt+Tab),
// Windows 11's rounded corners in the theme's line colour. Its page was a WebView2 browser (ui/'s <page>.html) until
// 2026-11-19; now the interface draws it, in a window of its own put inside this one (tool_page,
// ui-gpui/src/toolpage.rs), made on the first show and let go a while after the window hides.
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
    collections::HashMap,
    rc::{Rc, Weak},
    sync::atomic::Ordering,
    time::Duration,
};

use futures::{
    channel::oneshot,
    future::{Either, select},
};
use serde_json::Value;
use windows_sys::Win32::{
    Foundation::RECT,
    Graphics::{
        Dwm::{DWMWA_CLOAK, DwmSetWindowAttribute},
        Gdi::{CreateSolidBrush, DeleteObject, FillRect, HDC},
    },
    UI::{
        Input::KeyboardAndMouse::{ReleaseCapture, SetFocus},
        WindowsAndMessaging::{
            CS_DROPSHADOW, DefWindowProcW, GetClientRect, HTBOTTOM, HTCAPTION, MA_ACTIVATE, SW_HIDE, SendMessageW, ShowWindow, WA_INACTIVE, WM_ACTIVATE, WM_CLOSE,
            WM_DESTROY, WM_DPICHANGED, WM_ENDSESSION, WM_ERASEBKGND, WM_GETMINMAXINFO, WM_MOUSEACTIVATE, WM_NCLBUTTONDOWN, WM_QUERYENDSESSION, WM_SIZE,
            WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_EX_CONTROLPARENT, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
        },
    },
};

use super::{
    log, themes,
    themes::Color,
    tool_page::{self, FromPage},
    ui,
    win_form::{self, CloseReason, FormRef, Timer, colorref},
    win_kit::{Point, Rect, Size, hwnd},
};

/// How long a shown window waits, unseen, for its page to draw at its size before it comes on screen anyway (logged).
const DRAWN_WITHIN_MS: u32 = 300;
/// How long the interface may take to make a page.
const MADE_WITHIN_MS: u32 = 15_000;

/// The overrides of a page's window (the C# virtual and abstract members).
pub trait WebTool: 'static {
    /// The window's base part.
    fn tool(&self) -> &WebToolWindow;
    /// GetType().Name, for error.log.
    fn type_name(&self) -> &'static str;
    /// The page's file in wwwroot ("panel.html"); the interface's page of that name ("panel") draws it now.
    fn page_file(&self) -> &'static str;
    /// Always on top, set in the window's style from its first moment (setting TopMost before Show activates it).
    fn starts_top_most(&self) -> bool {
        false
    }
    /// How long the page is kept after the window hid; shown again meanwhile, it is there at once.
    fn keep_browser(&self) -> Duration {
        Duration::from_secs(60)
    }
    /// While true the page is kept even though the window is hidden.
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
    /// Nothing of its own drawn under the page (no redirection surface): where the page draws see-through, what is
    /// behind the window shows (the overlay's dimmed game), while every click still lands on the window.
    fn see_through(&self) -> bool {
        false
    }
    /// The colour shown before the page draws, and under a page that is see-through.
    fn backdrop(&self) -> Color {
        themes::current().surface
    }
    /// A click on the page takes the keyboard (it needs it for typing, keys and Esc).
    fn activate_on_click(&self) -> bool {
        true
    }
    /// The page is there (also again after it went): push what it draws.
    fn on_page_ready(&self) {}
    /// The page's size changed (CSS pixels); `args` has the rest of what it said.
    fn on_page_size(&self, _css: Size, _args: &Value) {}
    /// Something clicked on the page.
    fn on_act(&self, what: &str, args: &Value);
    /// The page went: hidden, and made again on the next show.
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

thread_local! {
    /// The pages of this thread's windows, by id (what the interface says about one is handed to its window).
    static PAGES: RefCell<HashMap<u64, Weak<dyn WebTool>>> = RefCell::new(HashMap::new());
}

/// The base part of a page's window.
pub struct WebToolWindow {
    me: RefCell<Option<Weak<dyn WebTool>>>,
    form: FormRef,
    release: Timer,
    /// The page asked of the interface (0: none).
    page_id: Cell<u64>,
    /// The page's window, inside this one, once made (0: not yet).
    child: Cell<isize>,
    /// Who waits for the page to be made.
    waits: RefCell<Vec<oneshot::Sender<()>>>,
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
    /// Shown but kept unseen (DWM's cloak) until its page drew at its size.
    cloaked: Cell<bool>,
    /// Which showing the page is to draw for (it says it back with its size).
    wake_seq: Cell<u64>,
    /// Shows it anyway when the drawing doesn't come.
    reveal_late: Timer,
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
            page_id: Cell::new(0),
            child: Cell::new(0),
            waits: RefCell::new(Vec::new()),
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
            cloaked: Cell::new(false),
            wake_seq: Cell::new(0),
            reveal_late: Timer::new(DRAWN_WITHIN_MS),
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
            let weak = me.clone();
            self.reveal_late.on_tick(move || {
                if let Some(o) = weak.upgrade() {
                    let t = o.tool();
                    let c = t.client_css();
                    t.reveal(Some(format!("{}: shown though its page hadn't drawn at the window's {}×{} within {DRAWN_WITHIN_MS} ms.", o.type_name(), c.width, c.height)));
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
        if owner.see_through() {
            ex |= WS_EX_NOREDIRECTIONBITMAP;
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
            WM_ERASEBKGND if owner.see_through() => Some(1),
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
                tool_page::fit(h, self.child.get(), false);
                None
            }
            WM_MOUSEACTIVATE => {
                // SAFETY: the default handling first, as base.WndProc.
                let r = unsafe { DefWindowProcW(hwnd(h), msg, wparam, lparam) };
                Some(if owner.activate_on_click() { MA_ACTIVATE as isize } else { r })
            }
            WM_ACTIVATE => {
                // The page's window is told too: a child window gets no WM_ACTIVATE of its own, and the interface takes
                // the keyboard on it then (its keys, Esc, Tab).
                let child = self.child.get();
                if child != 0 {
                    // SAFETY: a plain message to the page's window, on this same thread.
                    unsafe { SendMessageW(hwnd(child), WM_ACTIVATE, wparam & 0xFFFF, 0) };
                }
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

    /// Show(): without the keyboard when ActivateOnShow says so (a game keeps it). With its page made, the window is
    /// shown unseen (DWM's cloak) until the page has drawn at its size: GPUI draws a shown window only, and the first
    /// moment would otherwise show nothing, or the page as it was when the window last hid.
    pub fn show(&self) {
        let h = self.handle();
        if h == 0 || self.visible() {
            return;
        }
        if self.child.get() != 0 {
            self.cloak(true);
            win_form::show(h, false);
            let seq = self.wake_seq.get() + 1;
            self.wake_seq.set(seq);
            self.reveal_late.stop();
            self.reveal_late.start();
            tool_page::wake(self.page_id.get(), seq);
        } else {
            win_form::show(h, self.activate.get());
        }
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
        self.reveal_late.stop();
        if self.cloaked.get() {
            self.cloak(false);
        }
        if let Some(o) = self.owner() {
            o.on_visible_changed();
        }
    }

    fn cloak(&self, on: bool) {
        self.cloaked.set(on);
        let h = self.form.handle();
        if h == 0 {
            return;
        }
        let value: i32 = on as i32;
        // SAFETY: plain call on our own window with an int on the stack.
        unsafe { DwmSetWindowAttribute(hwnd(h), DWMWA_CLOAK as u32, &value as *const i32 as *const _, 4) };
    }

    /// The cloak off: the window is seen (and takes the keyboard when it is to). `late`: why it wasn't waited for.
    fn reveal(&self, late: Option<String>) {
        self.reveal_late.stop();
        if !self.cloaked.get() {
            return;
        }
        self.cloak(false);
        if let Some(why) = late {
            log::note(&why);
        }
        if self.activate.get() && self.visible() {
            self.activate();
        }
    }

    /// The window's inside in the page's pixels (its screen's scale taken off).
    fn client_css(&self) -> Size {
        let h = self.form.handle();
        let c = win_form::client_size(h);
        let dpi = win_form::device_dpi(h).max(96) as f32;
        Size::new((c.width as f32 * 96.0 / dpi).round() as i32, (c.height as f32 * 96.0 / dpi).round() as i32)
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
        self.reveal_late.dispose();
        self.let_page_go();
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

    /// The page is made and in the window.
    pub fn browser_ready(&self) -> bool {
        self.child.get() != 0
    }

    /// Whether the next show takes the keyboard.
    pub fn activate_on_show(&self) -> bool {
        self.activate.get()
    }

    pub fn set_activate_on_show(&self, value: bool) {
        self.activate.set(value);
    }

    // ───────────────────────── the page ─────────────────────────

    /// Has the interface make the page (once) and waits for it; false when it couldn't (logged; the window stays
    /// hidden) or the window went meanwhile.
    pub async fn ensure_browser(me: Rc<dyn WebTool>) -> bool {
        let t = me.tool();
        t.release.stop();
        loop {
            if t.is_disposed() {
                return false;
            }
            if t.child.get() != 0 {
                return true;
            }
            if t.page_id.get() == 0 {
                let h = t.handle();
                if h == 0 {
                    return false;
                }
                let id = tool_page::register(|id, m| {
                    win_form::invoke(move || {
                        let owner = PAGES.with(|p| p.borrow().get(&id).and_then(Weak::upgrade));
                        match owner {
                            Some(o) => o.tool().from_page(&o, id, m),
                            None => {
                                if matches!(m, FromPage::Attached(_)) {
                                    tool_page::close(id);
                                }
                            }
                        }
                    })
                });
                PAGES.with(|p| p.borrow_mut().insert(id, Rc::downgrade(&me)));
                t.page_id.set(id);
                let kind = me.page_file().trim_end_matches(".html");
                if !tool_page::open(id, kind, h) {
                    log::note(&format!("{}: no interface to draw its page; the window stays hidden.", me.type_name()));
                    t.let_page_go();
                    return false;
                }
            }
            let (tx, rx) = oneshot::channel();
            t.waits.borrow_mut().push(tx);
            let id = t.page_id.get();
            match select(rx, Box::pin(win_form::delay(MADE_WITHIN_MS))).await {
                Either::Left(_) => continue,
                Either::Right(_) => {
                    if t.page_id.get() == id && t.child.get() == 0 {
                        log::note(&format!("{}: the interface didn't make its page within {} s; the window stays hidden.", me.type_name(), MADE_WITHIN_MS / 1000));
                        t.let_page_go();
                        return false;
                    }
                }
            }
        }
    }

    /// What the interface says about this window's page.
    fn from_page(&self, owner: &Rc<dyn WebTool>, id: u64, m: FromPage) {
        if id != self.page_id.get() || self.is_disposed() {
            if matches!(m, FromPage::Attached(_)) {
                tool_page::close(id);
            }
            return;
        }
        match m {
            FromPage::Attached(child) => {
                self.child.set(child);
                tool_page::fit(self.form.handle(), child, false);
                for tx in self.waits.borrow_mut().drain(..) {
                    let _ = tx.send(());
                }
                owner.on_page_ready();
            }
            FromPage::Message(v) => self.on_message(owner, &v),
            FromPage::Gone => {
                if self.child.get() != 0 {
                    owner.on_browser_failed();
                }
            }
        }
    }

    /// The page went without being asked: hidden, and made again on the next show.
    pub fn browser_failed_base(&self, type_name: &str) {
        log::note(&format!(
            "{type_name}: its page went; hidden ({}), made again on the next show.",
            if self.visible() { "was shown" } else { "was hidden" }
        ));
        self.hide();
        self.release_browser(true);
    }

    /// The page goes (its memory with it) unless the window is shown or holds it; the next show makes it again.
    pub fn release_browser(&self, force: bool) {
        self.release.stop();
        let hold = self.owner().is_some_and(|o| o.hold_browser());
        if !force && (self.visible() || hold) {
            return;
        }
        self.let_page_go();
    }

    /// The page and its window go; who waited for it wakes and finds it gone.
    fn let_page_go(&self) {
        let id = self.page_id.replace(0);
        let child = self.child.replace(0);
        if child != 0 {
            // Out of sight at once; the interface closes it on its own turn.
            // SAFETY: plain call on the page's window.
            unsafe { ShowWindow(hwnd(child), SW_HIDE) };
        }
        if id != 0 {
            tool_page::unregister(id);
            PAGES.with(|p| p.borrow_mut().remove(&id));
            tool_page::close(id);
        }
        self.waits.borrow_mut().clear();
    }

    /// OnVisibleChanged's base part: hidden, the page is let go after keep_browser (unless shown again meanwhile).
    pub fn visible_changed_base(&self) {
        if self.visible() {
            self.release.stop();
        } else if self.page_id.get() != 0 && !self.is_disposed() {
            let Some(owner) = self.owner() else { return };
            self.release.set_interval(owner.keep_browser().as_millis().max(1000) as u32);
            self.release.start();
        }
    }

    /// The page is let go after keep_browser again (after hold_browser turned false).
    pub fn release_later(&self) {
        if !self.visible() && self.page_id.get() != 0 && !self.is_disposed() {
            let Some(owner) = self.owner() else { return };
            self.release.set_interval(owner.keep_browser().as_millis().max(1000) as u32);
            self.release.start();
        }
    }

    /// The page takes the keyboard (the window was activated: its keys go to the page, not the window).
    pub fn focus_page(&self) {
        let child = self.child.get();
        if child != 0 {
            // SAFETY: plain call; the page's window is on this thread.
            unsafe { SetFocus(hwnd(child)) };
        }
    }

    // ───────────────────────── messages ─────────────────────────

    /// An event to the page, as the app's page got them: by name, with its data.
    pub fn send_event(&self, name: &str, data: Value) {
        let id = self.page_id.get();
        if id == 0 || self.child.get() == 0 {
            return;
        }
        tool_page::event(id, name, data);
    }

    /// The page's messages: { cmd, args }.
    fn on_message(&self, owner: &Rc<dyn WebTool>, root: &Value) {
        if self.is_disposed() {
            return;
        }
        let cmd = root.get("cmd").and_then(Value::as_str).unwrap_or("");
        let args = root.get("args").filter(|a| a.is_object()).cloned().unwrap_or(Value::Null);
        match cmd {
            "size" if args.is_object() => {
                let (w, h, vw, vh, seq) = tool_page::drawn(&args);
                if w > 0 && h > 0 {
                    // (Every report, as the page's were: one comes after each event it was sent, and a window that
                    // waits to show shows on it.)
                    self.page.set(Size::new(w, h));
                    owner.on_page_size(self.page.get(), &args);
                    // Drawn for this showing at the window's size: on screen.
                    let c = self.client_css();
                    if self.cloaked.get() && seq == self.wake_seq.get() && (c.width - vw).abs() <= 1 && (c.height - vh).abs() <= 1 {
                        self.reveal(None);
                    }
                }
            }
            "drag" => self.begin_drag(owner),
            "resize" => self.begin_resize(owner),
            "act" if args.is_object() => owner.on_act(&str_of(&args, "what"), &args),
            _ => {}
        }
    }

    // ───────────────────────── moving and sizing ─────────────────────────

    /// The page's title row pressed and moved: Windows moves the window with the pointer, as by a title bar.
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
        let h = self.form.handle();
        if h != 0 {
            win_form::invalidate(h);
            if owner.rounded() {
                ui::round_corners(h, &themes::current());
            }
        }
    }
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
