// MiniWindow.cs: a small always-on-top window listing the running accounts (status, name, game, RAM, Show and Close):
// mini.html (ui/src/mini/MiniView.tsx) in the app's theme, the same chrome as a macro's panel (until 2026-11 hand-drawn
// WinForms). It keeps working while the main window is in the tray (its own page, the shared browser profile). Esc or ✕
// hides it; its place and height are kept in prefs' mini_window; its bottom edge sizes it. The page is let go 60 s
// after it hides.
//
// It lives on the windows thread; what it asks of the App goes through rt::post, and the App's answer comes back with
// win_form::invoke to the one mini window (`current`).
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    time::Duration,
};

use serde_json::Value;

use super::{
    app::App,
    base::Guid,
    log, prefs, rt,
    web_tool_window::{WebTool, WebToolWindow, scale_at, scaled, str_of},
    win_form::{self, Timer},
    win_kit::{Point, Rect, Size, all_screens},
    window_report,
};

/// Its width and its heights in CSS pixels (the page's); the window is these times its monitor's scale.
const PAGE_WIDTH: i32 = 300;
const MIN_HEIGHT: i32 = 200;
const DEFAULT_HEIGHT: i32 = 360;
const GAP: i32 = 16;

thread_local! {
    static CURRENT: RefCell<Weak<MiniWindow>> = RefCell::new(Weak::new());
}

pub struct MiniWindow {
    tool: WebToolWindow,
    open_app: Box<dyn Fn()>,
    timer: Timer,
    showing: Cell<bool>,
    failed: Cell<bool>,
    placed: Cell<bool>,
    /// What the page was last sent: the two-second refresh sends only a change.
    sent: RefCell<Option<String>>,
    /// Its height in CSS pixels, as the person left it.
    height: Cell<i32>,
    /// Dismissed.
    pub on_dismissed: RefCell<Option<Box<dyn Fn()>>>,
}

impl MiniWindow {
    pub fn new(open_app: impl Fn() + 'static) -> Rc<MiniWindow> {
        let me = Rc::new(MiniWindow {
            tool: WebToolWindow::new(),
            open_app: Box::new(open_app),
            timer: Timer::new(2000),
            showing: Cell::new(false),
            failed: Cell::new(false),
            placed: Cell::new(false),
            sent: RefCell::new(None),
            height: Cell::new(DEFAULT_HEIGHT),
            on_dismissed: RefCell::new(None),
        });
        let weak: Weak<dyn WebTool> = Rc::downgrade(&me) as Weak<dyn WebTool>;
        me.tool.bind(weak);
        // A message box lowers it meanwhile, so it opens in front (ui::modal).
        super::ui::note_on_top(me.tool.form_ref());
        let w = Rc::downgrade(&me);
        me.timer.on_tick(move || {
            if let Some(m) = w.upgrade() {
                m.tick();
            }
        });
        CURRENT.with(|c| *c.borrow_mut() = Rc::downgrade(&me));
        me
    }

    /// The mini window, on the windows thread.
    pub fn current() -> Option<Rc<MiniWindow>> {
        CURRENT.with(|c| c.borrow().upgrade())
    }

    pub fn visible(&self) -> bool {
        self.tool.visible()
    }

    /// `activate`: false when reopened at start, so the lock screen keeps the keyboard.
    pub fn open(self: &Rc<Self>, activate: bool) {
        let me = self.clone();
        win_form::spawn(async move {
            prefs::lock().mini_open = true;
            prefs::save();
            if me.tool.visible() {
                me.push();
                if activate {
                    log::note("Mini window: brought to the front (already open).");
                    me.tool.activate();
                }
                return;
            }
            if me.showing.get() {
                return;
            }
            me.tool.set_activate_on_show(activate);
            me.showing.set(true);
            let dynme: Rc<dyn WebTool> = me.clone();
            if !WebToolWindow::ensure_browser(dynme).await || me.tool.is_disposed() {
                if !me.tool.is_disposed() {
                    log::note("Mini window: not shown, its page couldn't start.");
                }
                me.showing.set(false);
                return;
            }
            me.push(); // the page answers with its size once it has drawn, and the window shows then (on_page_size)
        });
    }

    /// Hidden at the person's request (✕, Esc, Alt+F4, the page's button); forgotten as open, unlike the app's exit.
    pub fn dismiss(&self) {
        self.showing.set(false);
        if !self.tool.visible() {
            return;
        }
        self.save_bounds();
        prefs::lock().mini_open = false;
        prefs::save();
        self.tool.hide();
        log::note("Mini window: hidden by the person.");
        if let Some(f) = self.on_dismissed.borrow().as_ref() {
            f();
        }
    }

    /// A new theme or language: drawn again at once.
    pub fn apply_theme(&self) {
        if !self.tool.visible() {
            return;
        }
        self.tool.frame();
        self.push();
    }

    /// A refresh that fails is logged (the first of a run of failures), not shown: the next tick tries again.
    fn tick(&self) {
        self.push();
    }

    /// What the page draws now: every two seconds while shown, and after each click; sent only when it changed,
    /// except while it opens (the page answers with its size, and the window shows then).
    pub fn push(&self) {
        if self.tool.is_disposed() || !self.tool.browser_ready() || (!self.tool.visible() && !self.showing.get()) {
            return;
        }
        rt::post(|| {
            App::with(|app| {
                // A failure is caught here as the C#'s catch (Exception) in Tick was.
                let message = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| app.mini_message()));
                win_form::invoke(move || {
                    if let Some(me) = MiniWindow::current() {
                        match message {
                            Ok(m) => {
                                me.send(m);
                                me.failed.set(false);
                            }
                            Err(_) => {
                                if !me.failed.get() {
                                    log::note("Mini window: its refresh failed.");
                                }
                                me.failed.set(true);
                            }
                        }
                    }
                });
            });
        });
    }

    fn send(&self, message: Value) {
        if self.tool.is_disposed() || !self.tool.browser_ready() || (!self.tool.visible() && !self.showing.get()) {
            return;
        }
        let json = message.to_string();
        if self.sent.borrow().as_deref() == Some(json.as_str()) && self.tool.visible() && !self.showing.get() {
            return;
        }
        *self.sent.borrow_mut() = Some(json);
        self.tool.send_event("mini", message);
    }

    /// Only the height changes, between its least and the screen's.
    fn limits(&self, scale: f32) {
        let width = scaled(Size::new(PAGE_WIDTH, 1), scale).width;
        self.tool.set_minimum_size(Size::default());
        self.tool.set_maximum_size(Size::default());
        self.tool.set_minimum_size(Size::new(width, scaled(Size::new(1, MIN_HEIGHT), scale).height));
        // A maximum height of 0 isn't "any" to Windows' size limits: a huge one stands in.
        self.tool.set_maximum_size(Size::new(width, 20000));
    }

    fn default_place(&self) -> Rect {
        let area = win_form::primary_screen().working_area;
        let size = scaled(Size::new(PAGE_WIDTH, self.height.get()), scale_at(area.location()));
        let margin = scaled(Size::new(GAP, 1), scale_at(area.location())).width;
        Rect::new(area.right() - size.width - margin, area.bottom() - size.height - margin, size.width, size.height)
    }

    /// The saved place (its height in CSS pixels), unless no screen shows its top any more (a monitor unplugged).
    fn saved_place(&self) -> Option<Rect> {
        let saved = prefs::lock().mini_window.clone()?;
        let parts: Vec<&str> = saved.split(',').collect();
        if parts.len() != 4 {
            return None;
        }
        let x: i32 = parts[0].parse().ok()?;
        let y: i32 = parts[1].parse().ok()?;
        let h: i32 = parts[3].parse().ok()?;
        if h <= 0 {
            return None;
        }
        self.height.set(MIN_HEIGHT.max(h));
        let size = scaled(Size::new(PAGE_WIDTH, self.height.get()), scale_at(Point::new(x, y)));
        // The middle half of its title row must be somewhere it can be grabbed.
        let grip = Rect::new(x + size.width / 4, y + 2, size.width / 2, 24);
        if all_screens().iter().any(|s| win_form::intersects(s.working_area, grip)) { Some(Rect::new(x, y, size.width, size.height)) } else { None }
    }

    /// Noted, not written: the callers save prefs once they are done.
    fn save_bounds(&self) {
        if !self.tool.visible() || !self.placed.get() {
            return;
        }
        let b = self.tool.bounds();
        self.height.set(MIN_HEIGHT.max((b.height as f32 / self.tool.scale_now()).round() as i32));
        prefs::lock().mini_window = Some(format!("{},{},{},{}", b.x, b.y, PAGE_WIDTH, self.height.get()));
    }
}

impl WebTool for MiniWindow {
    fn tool(&self) -> &WebToolWindow {
        &self.tool
    }
    fn type_name(&self) -> &'static str {
        "MiniWindow"
    }
    fn page_file(&self) -> &'static str {
        "mini.html"
    }
    fn starts_top_most(&self) -> bool {
        true
    }
    fn keep_browser(&self) -> Duration {
        Duration::from_secs(60)
    }

    fn on_visible_changed(&self) {
        self.tool.visible_changed_base();
        if self.tool.visible() {
            self.timer.start();
        } else {
            self.timer.stop();
        }
    }

    fn on_page_ready(&self) {
        *self.sent.borrow_mut() = None; // a new page: it has nothing yet
        if self.tool.visible() || self.showing.get() {
            self.push();
        }
    }

    /// The page has drawn: shown now, where it was left (or at the bottom right of the main screen).
    fn on_page_size(&self, _css: Size, _args: &Value) {
        if !self.showing.get() || self.tool.visible() {
            return;
        }
        self.showing.set(false);
        let at = self.saved_place().unwrap_or_else(|| self.default_place());
        let scale = scale_at(at.location());
        self.limits(scale);
        let area = win_form::screen_from_point(at.location()).working_area;
        let size = Size::new(scaled(Size::new(PAGE_WIDTH, 1), scale).width, scaled(Size::new(1, self.height.get()), scale).height.min(area.height));
        // The place first, then the size: set together on a screen at another scale, Windows' scale change resized it
        // again by the ratio (TrayWindow, 2026-10-30).
        let location = super::ui::inside(Rect::at(at.location(), size), area, 0);
        self.tool.set_location(location);
        self.tool.set_bounds(Rect::at(location, size));
        self.placed.set(true);
        self.tool.show();
        let activated = self.tool.activate_on_show();
        if activated {
            self.tool.activate();
        }
        log::note(&format!(
            "Mini window: shown always on top{}, {}.",
            if activated { " and activated" } else { " without the keyboard (reopened at start)" },
            window_report::describe(self.tool.handle())
        ));
    }

    /// Moved to a screen with another scale: the same page size there (the base takes Windows' place).
    fn on_dpi_changed(&self, new_dpi: u32, suggested: Rect) {
        self.tool.dpi_changed_base(new_dpi, suggested);
        let scale = new_dpi as f32 / 96.0;
        self.limits(scale);
        self.tool.set_size(scaled(Size::new(PAGE_WIDTH, self.height.get()), scale));
    }

    fn on_dragged(&self) {
        self.save_bounds();
        prefs::save();
    }

    fn on_resized(&self) {
        self.save_bounds();
        prefs::save();
    }

    fn on_act(&self, what: &str, args: &Value) {
        match what {
            "hide" => {
                self.dismiss();
                return;
            }
            "open" => {
                (self.open_app)();
                return;
            }
            "show" => {
                if let Ok(id) = Guid::parse_str(&str_of(args, "id")) {
                    rt::post(move || App::with(|app| app.show_client(id)));
                }
            }
            "close" => {
                if let Ok(id) = Guid::parse_str(&str_of(args, "id")) {
                    rt::post(move || App::with(|app| app.close_client(id)));
                }
            }
            "closeAll" => {
                let remember = args.get("remember") == Some(&Value::Bool(true));
                rt::post(move || {
                    App::with(|app| {
                        if remember {
                            app.skip_confirm("closeAll");
                        }
                        app.close_all_clients();
                    })
                });
            }
            _ => {}
        }
        // PORT: the C# pushed right after the (synchronous) call; here the App's answer arrives in its turn after the
        // posted call above, as the push's own request is queued behind it on the core's thread.
        self.push();
    }

    fn on_browser_failed(&self) {
        let shown = self.tool.visible();
        self.tool.browser_failed_base(self.type_name());
        if shown {
            log::note("Mini window: opened again after its page crashed.");
            if let Some(me) = MiniWindow::current() {
                me.open(false);
            }
        }
    }

    fn on_form_closing(&self, reason: win_form::CloseReason) -> bool {
        // Alt+F4 hides, like Esc; the app's own exit disposes it.
        if reason == win_form::CloseReason::UserClosing {
            self.dismiss();
            return true;
        }
        self.save_bounds();
        prefs::save();
        false
    }

    fn on_disposing(&self) {
        // Closed for the exit (not dismissed): its place is kept, and it comes back next start if it was open.
        self.save_bounds();
        self.timer.dispose();
    }
}
