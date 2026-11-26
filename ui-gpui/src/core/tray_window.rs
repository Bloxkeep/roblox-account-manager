// TrayWindow.cs: the tray icon's quick panel (left click) and menu (right click), by the pointer above the taskbar.
// Drawn by the interface itself (ui-gpui/src/tray.rs) in a window of its own, in the same process. Until 2026-11-19 it
// was a page (tray.html, ui/src/Tray.tsx) in a WebView2 browser: a browser of its own, so in Task Manager a "WebView2
// Manager", a GPU process and helpers came and went beside Bloxkeep while the pointer was near the icon, and it took
// warming up ahead of the click, keeping the browser a minute after, waiting for its page and a stand-in menu for when
// WebView2 failed. All of that went with the browser.
//
// Here stays what decides the tray's behaviour: which part opens and closing it again on the same click, where it goes
// (by the pointer, next to the taskbar), hiding it once something else is clicked, what it shows (the App's TrayState,
// sent again every two seconds while it is open) and what its clicks do. The interface draws what it is sent, says the
// size it drew (`on_size`), and the window is placed and shown then.
//
// On the windows thread, which is the interface's thread too. What it draws comes from the App (on the core's thread,
// through rt::post) and goes straight to the interface (gpui_host::post).
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    sync::atomic::{AtomicIsize, Ordering},
};

use serde_json::{Value, json};
use windows_sys::Win32::{
    Graphics::Dwm::{DWMWA_CLOAK, DwmSetWindowAttribute},
    UI::{
        HiDpi::GetDpiForWindow,
        Shell::{ABM_GETTASKBARPOS, APPBARDATA, SHAppBarMessage},
    },
};

use super::{
    app::App,
    base::Guid,
    clock, gpui_host, l, log, rt, themes, ui,
    web_tool_window::dpi_at,
    win_form::{self, Timer},
    win_kit::{Point, Rect, Size},
    window_report,
};

/// How long the interface has to draw an opening before it is given up on (not left "showing").
const DRAW_WITHIN_MS: u32 = 5_000;
/// How long an opening waits for its drawing at the size its window was just given, before it shows anyway.
const FIT_WITHIN_MS: u32 = 300;

const ABE_LEFT: i32 = 0;
const ABE_TOP: i32 = 1;
const ABE_RIGHT: i32 = 2;
const ABE_BOTTOM: i32 = 3;

thread_local! {
    static CURRENT: RefCell<Weak<TrayWindow>> = RefCell::new(Weak::new());
}

/// What a click in the tray asks of the app (the C# events).
#[derive(Default)]
pub struct TrayEvents {
    pub open_clicked: Option<Box<dyn Fn()>>,
    pub mini_clicked: Option<Box<dyn Fn()>>,
    /// Multi-Roblox's switch, with whether it is on now (the tray offers it only where it can switch from here).
    pub multi_clicked: Option<Box<dyn Fn(bool)>>,
    pub account_clicked: Option<Box<dyn Fn(Guid)>>,
    pub preset_clicked: Option<Box<dyn Fn(Guid)>>,
    /// "Macros ›": a macro's panel shown or hidden.
    pub panel_clicked: Option<Box<dyn Fn(Guid)>>,
    pub new_roblox_clicked: Option<Box<dyn Fn()>>,
    pub close_all_clicked: Option<Box<dyn Fn()>>,
    pub show_client_clicked: Option<Box<dyn Fn(i32)>>,
    pub close_client_clicked: Option<Box<dyn Fn(Guid)>>,
    pub quit_clicked: Option<Box<dyn Fn()>>,
}

pub struct TrayWindow {
    /// The interface's tray window, once it made one (`attach`).
    handle: AtomicIsize,
    disposed: Cell<bool>,
    refresh: Timer,
    /// "panel" or "menu"; a new count each time it opens, so the interface starts it on its first page.
    mode: RefCell<String>,
    opened: Cell<i32>,
    anchor: Cell<Point>,
    /// The size the interface drew, in its own (unscaled) pixels.
    content: Cell<Size>,
    /// Opening: sent to the interface, not drawn yet.
    showing: Cell<bool>,
    /// Shown to Windows but kept off the screen (DWM's cloak) while the interface draws an opening: GPUI draws only
    /// windows Windows calls visible, so a hidden one would never be drawn, measured or shown.
    cloaked: Cell<bool>,
    /// The interface drew this opening (said its size) at least once.
    drawn: Cell<bool>,
    /// Drawings of this opening that didn't fit its window yet.
    waits: Cell<u8>,
    hidden_at: Cell<i64>,
    /// The theme the window's corners were last set in.
    framed: RefCell<Option<themes::Theme>>,
    pub events: RefCell<TrayEvents>,
    /// Whether the mini window is open (MiniOpen).
    pub mini_open: RefCell<Option<Box<dyn Fn() -> bool>>>,
}

impl TrayWindow {
    pub fn new() -> Rc<TrayWindow> {
        let me = Rc::new(TrayWindow {
            handle: AtomicIsize::new(0),
            disposed: Cell::new(false),
            refresh: Timer::new(2000),
            mode: RefCell::new("panel".into()),
            opened: Cell::new(0),
            anchor: Cell::new(Point::default()),
            content: Cell::new(Size::new(344, 300)),
            showing: Cell::new(false),
            cloaked: Cell::new(false),
            drawn: Cell::new(false),
            waits: Cell::new(0),
            hidden_at: Cell::new(i64::MIN / 2),
            framed: RefCell::new(None),
            events: RefCell::new(TrayEvents::default()),
            mini_open: RefCell::new(None),
        });
        let w = Rc::downgrade(&me);
        me.refresh.on_tick(move || {
            if let Some(t) = w.upgrade() {
                t.push();
            }
        });
        CURRENT.with(|c| *c.borrow_mut() = Rc::downgrade(&me));
        me
    }

    /// The tray window, on the windows thread.
    pub fn current() -> Option<Rc<TrayWindow>> {
        CURRENT.with(|c| c.borrow().upgrade())
    }

    fn hwnd(&self) -> isize {
        self.handle.load(Ordering::SeqCst)
    }

    /// The interface made its tray window: this one is placed, shown and hidden from here. Windows 11's rounded
    /// corners in the theme's line colour.
    pub fn attach(&self, h: isize) {
        if self.disposed.get() {
            return;
        }
        self.handle.store(h, Ordering::SeqCst);
        log::note(&format!("Tray: the interface made its window, {}.", window_report::describe(h)));
        *self.framed.borrow_mut() = Some(themes::current());
        self.frame();
        // The first opening made the window: drawn now, off the screen until it fits.
        if self.showing.get() {
            self.park();
        }
    }

    /// On the screen (not just drawing an opening under the cloak).
    pub fn visible(&self) -> bool {
        let h = self.hwnd();
        h != 0 && win_form::is_visible(h) && !self.cloaked.get()
    }

    fn cloak(&self, on: bool) {
        let h = self.hwnd();
        if h == 0 || self.cloaked.get() == on {
            return;
        }
        self.cloaked.set(on);
        let value: i32 = on.into();
        // SAFETY: plain call with an int on the stack.
        unsafe { DwmSetWindowAttribute(h as _, DWMWA_CLOAK as _, &value as *const i32 as *const _, 4) };
    }

    /// An opening is drawn: the window shown to Windows (so GPUI draws it), cloaked, where it goes at its last size,
    /// without the keyboard. It comes on screen once the drawing fits it (`on_size`).
    fn park(&self) {
        let h = self.hwnd();
        if h == 0 || self.disposed.get() {
            return;
        }
        if !win_form::is_visible(h) {
            self.cloak(true);
            self.place();
            win_form::show(h, false);
        }
    }

    /// The tray icon was clicked just as the tray went away (the click itself took it away): don't bring it back.
    pub fn recently_hidden(&self) -> bool {
        clock::tick() - self.hidden_at.get() < 400
    }

    /// Opens the panel or the menu by the tray icon (the pointer at `anchor`); the same again closes it.
    pub fn toggle(self: &Rc<Self>, anchor: Point, mode: &str) {
        if self.disposed.get() {
            return;
        }
        if self.visible() {
            let same = *self.mode.borrow() == mode;
            self.hide_tray();
            if same {
                return;
            }
        } else if self.recently_hidden() && *self.mode.borrow() == mode {
            return;
        }
        *self.mode.borrow_mut() = mode.to_string();
        self.anchor.set(anchor);
        self.opened.set(self.opened.get() + 1);
        self.showing.set(true);
        self.drawn.set(false);
        self.waits.set(0);
        // The interface draws it and says its size (`on_size`); the window comes on screen then.
        self.push();
        self.park();
        // Never drawn (the interface stuck or gone): not left "showing", so the next click tries again.
        let me = self.clone();
        let opened = self.opened.get();
        win_form::spawn(async move {
            win_form::delay(DRAW_WITHIN_MS).await;
            if !me.showing.get() || me.opened.get() != opened {
                return;
            }
            // Drawn but never at its window's size: shown as it is (a tray that never opens is the worse fault).
            if me.drawn.get() {
                me.reveal(Some(format!("its drawing never fitted the window in {} s", DRAW_WITHIN_MS / 1000)));
                return;
            }
            let h = me.hwnd();
            let stage = if h == 0 {
                "the interface never made its window"
            } else if !win_form::is_visible(h) {
                "its window was made but not shown to Windows"
            } else {
                "its window was shown under the cloak but never drawn"
            };
            log::note(&format!("Tray: {} not shown in {} s: {stage}.", me.mode.borrow(), DRAW_WITHIN_MS / 1000));
            me.hide_tray();
        });
    }

    pub fn hide_tray(&self) {
        self.showing.set(false);
        let h = self.hwnd();
        if !win_form::is_visible(h) {
            return;
        }
        let was_on_screen = self.visible();
        self.refresh.stop();
        win_form::hide(h);
        self.cloak(false);
        if was_on_screen {
            self.hidden_at.set(clock::tick());
            log::note(&format!("Tray: {} hidden.", self.mode.borrow()));
        }
    }

    /// Something else was clicked: it goes. (Only once on screen.)
    pub fn on_deactivate(&self) {
        if self.visible() {
            self.hide_tray();
        }
    }

    fn id_of(o: &Value) -> Guid {
        o.get("id").and_then(Value::as_str).and_then(|s| Guid::parse_str(s).ok()).unwrap_or(Guid::nil())
    }

    pub fn act(&self, a: &Value) {
        let what = a.get("what").and_then(Value::as_str).unwrap_or("");
        let ev = |f: &dyn Fn(&TrayEvents)| f(&self.events.borrow());
        match what {
            "open" => {
                self.hide_tray();
                ev(&|e| if let Some(f) = &e.open_clicked { f() });
            }
            "mini" => {
                self.hide_tray();
                ev(&|e| if let Some(f) = &e.mini_clicked { f() });
            }
            // Turning Multi-Roblox on with a vault: Bloxkeep comes up and asks its question there (App::tray_multi).
            "multiAsk" => {
                self.hide_tray();
                ev(&|e| if let Some(f) = &e.open_clicked { f() });
                ev(&|e| if let Some(f) = &e.multi_clicked { f(false) });
            }
            "multi" => {
                let on = a.get("on") == Some(&Value::Bool(true));
                ev(&|e| if let Some(f) = &e.multi_clicked { f(on) });
                self.push();
            }
            "account" | "preset" | "panel" | "closeClient" => {
                let id = TrayWindow::id_of(a);
                if id.is_nil() {
                    return;
                }
                match what {
                    "account" => {
                        self.hide_tray();
                        ev(&|e| if let Some(f) = &e.account_clicked { f(id) });
                    }
                    "preset" => {
                        self.hide_tray();
                        ev(&|e| if let Some(f) = &e.preset_clicked { f(id) });
                    }
                    "panel" => {
                        self.hide_tray();
                        ev(&|e| if let Some(f) = &e.panel_clicked { f(id) });
                    }
                    _ => {
                        ev(&|e| if let Some(f) = &e.close_client_clicked { f(id) });
                        self.push();
                    }
                }
            }
            "newRoblox" => {
                self.hide_tray();
                ev(&|e| if let Some(f) = &e.new_roblox_clicked { f() });
            }
            "closeAll" => {
                // From the panel, after its second click: it stays, showing the windows go. From the menu: done.
                ev(&|e| if let Some(f) = &e.close_all_clicked { f() });
                if *self.mode.borrow() == "menu" {
                    self.hide_tray();
                } else {
                    self.push();
                }
            }
            "show" => {
                let Some(pid) = a.get("pid").and_then(Value::as_f64) else { return };
                self.hide_tray();
                let pid = pid as i32;
                ev(&|e| if let Some(f) = &e.show_client_clicked { f(pid) });
            }
            "quit" => {
                self.hide_tray();
                ev(&|e| if let Some(f) = &e.quit_clicked { f() });
            }
            "hide" => self.hide_tray(),
            _ => {}
        }
    }

    /// What the interface draws: which part, the state now, the theme and the language. On the core's thread (the
    /// App's TrayState, TrayPictureUrl and TrayCustomTheme).
    pub fn message(app: &App, mode: &str, opened: i32, mini_open: bool) -> Value {
        let s = app.tray_state();
        let state = json!({
            "open": s.open, "multi": s.multi, "multiOnly": s.multi_only, "robloxCount": s.roblox_count, "ramMb": s.ram_mb, "miniOpen": mini_open,
            "presets": s.presets.iter().map(|(id, name)| json!({ "id": id, "name": name })).collect::<Vec<_>>(),
            "accounts": s.accounts.as_deref().unwrap_or(&[]).iter().map(|(id, name, running)| json!({ "id": id, "name": name, "running": running, "avatar": app.tray_picture_url(*id) })).collect::<Vec<_>>(),
            "macros": s.macros.as_deref().unwrap_or(&[]).iter().map(|(id, name, shown, state)| json!({ "id": id, "name": name, "shown": shown, "state": state })).collect::<Vec<_>>(),
            "clients": s.clients.as_deref().unwrap_or(&[]).iter().map(|x| json!({ "id": x.id, "name": x.name, "game": x.game, "pid": x.pid, "ramMb": x.ram_mb, "avatar": app.tray_picture_url(x.id) })).collect::<Vec<_>>(),
        });
        json!({
            "type": "event", "name": "tray",
            "data": { "mode": mode, "opened": opened, "state": state, "theme": themes::current().id, "customTheme": app.tray_custom_theme(), "language": l::language() },
        })
    }

    /// What the interface draws now: on open, after a click and on the refresh. The corners follow a theme change.
    fn push(&self) {
        if self.disposed.get() || (!self.visible() && !self.showing.get()) {
            return;
        }
        let theme = themes::current();
        if self.framed.borrow().as_ref() != Some(&theme) {
            *self.framed.borrow_mut() = Some(theme);
            self.frame();
        }
        let mode = self.mode.borrow().clone();
        let opened = self.opened.get();
        let mini_open = self.mini_open.borrow().as_ref().is_some_and(|f| f());
        rt::post(move || App::with(|app| gpui_host::post(TrayWindow::message(app, &mode, opened, mini_open).to_string())));
    }

    /// The size the interface drew (its own pixels) and the size of the window it drew in. An opening (`opened`)
    /// comes on screen once it was drawn at its window's size, not with a frame of what it showed before; if that
    /// drawing doesn't come (FIT_WITHIN_MS, two drawings), it comes on screen anyway, and the log says why.
    pub fn on_size(self: &Rc<Self>, width: i32, height: i32, drawn_w: i32, drawn_h: i32, opened: i32) {
        if width <= 0 || height <= 0 || self.hwnd() == 0 || (self.showing.get() && opened != self.opened.get()) {
            return;
        }
        let changed = self.content.get() != Size::new(width, height);
        self.content.set(Size::new(width, height));
        if !self.showing.get() {
            if self.visible() && changed {
                self.place();
            }
            return;
        }
        self.drawn.set(true);
        if changed || !self.fits(drawn_w, drawn_h) {
            self.place();
        }
        if self.fits(drawn_w, drawn_h) {
            self.reveal(None);
            return;
        }
        let waits = self.waits.get() + 1;
        self.waits.set(waits);
        let client = self.client();
        if waits > 2 {
            self.reveal(Some(format!("drawn at {drawn_w}×{drawn_h} in a window of {}×{}", client.width, client.height)));
            return;
        }
        let me = self.clone();
        win_form::spawn(async move {
            win_form::delay(FIT_WITHIN_MS).await;
            if me.showing.get() && me.opened.get() == opened {
                let client = me.client();
                me.reveal(Some(format!("no drawing at the window's {}×{} within {} ms", client.width, client.height, FIT_WITHIN_MS)));
            }
        });
    }

    /// The window's inside in the interface's pixels (its screen's scale taken off).
    fn client(&self) -> Size {
        let h = self.hwnd();
        // SAFETY: plain query on our own window.
        let dpi = unsafe { GetDpiForWindow(h as _) }.max(96) as f32;
        let c = win_form::client_size(h);
        Size::new((c.width as f32 * 96.0 / dpi).round() as i32, (c.height as f32 * 96.0 / dpi).round() as i32)
    }

    /// What the interface drew was drawn at the window's size (to a pixel: its scale rounds).
    fn fits(&self, drawn_w: i32, drawn_h: i32) -> bool {
        let c = self.client();
        (c.width - drawn_w).abs() <= 1 && (c.height - drawn_h).abs() <= 1
    }

    /// The opening on screen: out of the cloak, to the front with the keyboard.
    fn reveal(&self, why: Option<String>) {
        if !self.showing.replace(false) {
            return;
        }
        let h = self.hwnd();
        if h == 0 {
            return;
        }
        self.cloak(false);
        if !win_form::is_visible(h) {
            win_form::show(h, false);
        }
        win_form::activate(h);
        let front = win_form::set_foreground(h);
        log::note(&format!(
            "Tray: {} shown always on top, {}{}{}.",
            self.mode.borrow(),
            window_report::describe(h),
            if front { "" } else { "; Windows didn't bring it to the front" },
            why.map(|w| format!("; shown though {w}")).unwrap_or_default()
        ));
        self.refresh.start();
    }

    /// By the pointer next to the taskbar, inside the screen at its scale; a height change keeps the taskbar edge
    /// in place.
    fn place(&self) {
        let anchor = self.anchor.get();
        let screen = win_form::screen_from_point(anchor);
        let scale = dpi_at(anchor) as f32 / 96.0;
        let content = self.content.get();
        let size = Size::new((content.width as f32 * scale).ceil() as i32, (content.height as f32 * scale).ceil() as i32);
        let r = place_by(anchor, size, screen.working_area, screen.bounds, taskbar(), (12.0 * scale) as i32);
        // The place first, then the size: set together while the window is on a screen at another scale, Windows' scale
        // change resized it again by the ratio (review 2026-10-30).
        let h = self.hwnd();
        win_form::set_location(h, r.location());
        win_form::set_bounds(h, r);
    }

    /// Windows 11's rounded corners and a border in the theme's line colour.
    fn frame(&self) {
        let h = self.hwnd();
        if h != 0 {
            ui::round_corners(h, &themes::current());
        }
    }

    /// The app is closing: nothing more is shown or sent (the window is the interface's, and goes with it).
    pub fn dispose(&self) {
        if self.disposed.replace(true) {
            return;
        }
        self.refresh.dispose();
        let h = self.handle.swap(0, Ordering::SeqCst);
        if h != 0 {
            win_form::hide(h);
        }
    }
}

/// Where the panel goes, in the screen's pixels: by the pointer, next to the taskbar. The taskbar's edge is the one
/// Windows says (`taskbar`, when on this screen): guessed from the work area alone, an auto-hidden taskbar (work area =
/// whole screen) put the panel over it, and a bottom one with another app's bar at a side was taken for a side one
/// (review 2026-10-30). Guessed only when Windows doesn't say.
pub fn place_by(anchor: Point, size: Size, work: Rect, all: Rect, taskbar: Option<(Rect, i32)>, gap: i32) -> Rect {
    let edge;
    let mut free = work;
    match taskbar {
        Some((bar, e)) if win_form::intersects(all, bar) && (ABE_LEFT..=ABE_BOTTOM).contains(&e) => {
            edge = e;
            free = match edge {
                ABE_BOTTOM => Rect::from_ltrb(work.left(), work.top(), work.right(), work.bottom().min(bar.top())),
                ABE_TOP => Rect::from_ltrb(work.left(), work.top().max(bar.bottom()), work.right(), work.bottom()),
                ABE_LEFT => Rect::from_ltrb(work.left().max(bar.right()), work.top(), work.right(), work.bottom()),
                _ => Rect::from_ltrb(work.left(), work.top(), work.right().min(bar.left()), work.bottom()),
            };
        }
        _ => {
            edge = if work.top() > all.top() {
                ABE_TOP
            } else if work.left() > all.left() {
                ABE_LEFT
            } else if work.right() < all.right() {
                ABE_RIGHT
            } else {
                ABE_BOTTOM
            };
        }
    }
    let clamp = |v: i32, min: i32, max: i32| min.max(v.min(max));
    let w = size.width;
    let h = size.height.min(free.height - gap * 2);
    if edge == ABE_LEFT || edge == ABE_RIGHT {
        return Rect::new(
            if edge == ABE_LEFT { free.left() + gap } else { free.right() - w - gap },
            clamp(anchor.y - h + gap, free.top() + gap, free.bottom() - h - gap),
            w,
            h,
        );
    }
    Rect::new(clamp(anchor.x - w / 2, free.left() + gap, free.right() - w - gap), if edge == ABE_TOP { free.top() + gap } else { free.bottom() - h - gap }, w, h)
}

/// The taskbar's place and edge (also while auto-hidden), None when Windows doesn't say.
fn taskbar() -> Option<(Rect, i32)> {
    // SAFETY: the structure's size is set as Windows requires.
    unsafe {
        let mut data: APPBARDATA = std::mem::zeroed();
        data.cbSize = std::mem::size_of::<APPBARDATA>() as u32;
        if SHAppBarMessage(ABM_GETTASKBARPOS, &mut data) == 0 {
            return None;
        }
        let r = Rect::from_win(&data.rc);
        if r.width > 0 && r.height > 0 { Some((r, data.uEdge as i32)) } else { None }
    }
}
