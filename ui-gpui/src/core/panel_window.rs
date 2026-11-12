// PanelWindow.cs: a macro's panel beside the game: panel.html (ui/src/panel/PanelView.tsx) in a small always-on-top tool
// window, in the app's theme (until 2026-11 hand-drawn WinForms, whose choice list spilled out of the window). The page
// draws Run/Stop, the status, the tabs and the settings the maker let the player change; what is clicked comes back as an
// "act" and goes to the App (core/app/panels.rs). Placed right of the game (else left of it), never over it; a place the
// person dragged it to is kept per macro in prefs' panel_windows. Folded, it is a small pill.
//
// PORT: in C# the panel and AppController lived on the one UI thread. Here the window (a `WebTool` over
// `WebToolWindow`, like the overlay and the mini window) lives on the windows thread (win_form), and the App on the core
// thread (rt). So the panel is two halves:
// - `PanelWindow`, on the windows thread: the window, its page, its placement, Push's "send only when changed".
// - `PanelHandle`, the App's (Send, cloned freely): what the App reads and sets (Visible, Bounds, Handle, AutoShown,
//   Suspended, Before, Pill) and the commands it gives, sent to the window with `win_form::invoke`.
// What the C# panel asked AppController for synchronously (PanelMessage, PanelGameRect, PanelsShownBesides, PanelShown,
// PanelAct, PanelActivated, Dismissed) is asked through `rt::post` + `App::with`, and the answer comes back by
// `win_form::invoke`. The steps keep the C# order; only the hops between them are new.
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::{Rc, Weak},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicIsize, Ordering},
    },
    time::Duration,
};

use serde_json::{Value, json};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetWindowLongW, GetWindowThreadProcessId, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos, WS_EX_TOPMOST,
};

use super::{
    app::App,
    base::Guid,
    log, prefs, rt,
    themes::{self, Theme},
    ui,
    web_tool_window::{WebTool, WebToolWindow, scale_at, scaled},
    win_form::{self, CloseReason},
    win_kit::{Point, Rect, Size, all_screens, hwnd, screen_from_rect},
    window_report,
};

const GAP: i32 = 8;

/// What both halves read and set: the App's handle and the window.
struct Shared {
    macro_id: Guid,
    /// The window's handle once made (0 before, and after it went).
    hwnd: AtomicIsize,
    /// Opening: shown once the page says its size.
    showing: AtomicBool,
    pill: AtomicBool,
    auto_shown: AtomicBool,
    suspended: AtomicBool,
    before: AtomicIsize,
    disposed: AtomicBool,
}

thread_local! {
    /// The panels' windows, on the windows thread, by macro.
    static WINDOWS: RefCell<HashMap<Guid, Rc<PanelWindow>>> = RefCell::new(HashMap::new());
}

// ───────────────────────── the App's half ─────────────────────────

/// The App's hold on a macro's panel (C#'s PanelWindow reference in AppController._panels).
#[derive(Clone)]
pub struct PanelHandle {
    shared: Arc<Shared>,
}

impl PartialEq for PanelHandle {
    fn eq(&self, other: &PanelHandle) -> bool {
        Arc::ptr_eq(&self.shared, &other.shared)
    }
}

impl PanelHandle {
    /// new PanelWindow(app, macroId): its window is made on the windows thread.
    pub fn new(macro_id: Guid) -> PanelHandle {
        let shared = Arc::new(Shared {
            macro_id,
            hwnd: AtomicIsize::new(0),
            showing: AtomicBool::new(false),
            pill: AtomicBool::new(false),
            auto_shown: AtomicBool::new(false),
            suspended: AtomicBool::new(false),
            before: AtomicIsize::new(0),
            disposed: AtomicBool::new(false),
        });
        let s = shared.clone();
        win_form::invoke(move || {
            let panel = PanelWindow::new(s);
            WINDOWS.with(|w| w.borrow_mut().insert(macro_id, panel));
        });
        PanelHandle { shared }
    }

    /// Work for the window, on the windows thread (dropped when this panel's window is gone).
    fn on_window(&self, f: impl FnOnce(&Rc<PanelWindow>) + Send + 'static) {
        let s = self.shared.clone();
        win_form::invoke(move || {
            let panel = WINDOWS.with(|w| w.borrow().get(&s.macro_id).cloned());
            if let Some(p) = panel
                && Arc::ptr_eq(&p.shared, &s)
            {
                f(&p);
            }
        });
    }

    pub fn macro_id(&self) -> Guid {
        self.shared.macro_id
    }

    pub fn is_disposed(&self) -> bool {
        self.shared.disposed.load(Ordering::SeqCst)
    }

    pub fn handle(&self) -> isize {
        self.shared.hwnd.load(Ordering::SeqCst)
    }

    pub fn is_handle_created(&self) -> bool {
        self.handle() != 0
    }

    pub fn visible(&self) -> bool {
        !self.is_disposed() && win_form::is_visible(self.handle())
    }

    pub fn bounds(&self) -> Rect {
        win_form::bounds(self.handle())
    }

    pub fn pill(&self) -> bool {
        self.shared.pill.load(Ordering::SeqCst)
    }

    /// Shown by itself (its game started), not by the person: an update may go on while it shows.
    pub fn auto_shown(&self) -> bool {
        self.shared.auto_shown.load(Ordering::SeqCst)
    }

    pub fn set_auto_shown(&self, value: bool) {
        self.shared.auto_shown.store(value, Ordering::SeqCst);
    }

    /// Stepped aside for a moment (the screen colour picker): not shown again by the panels' tick meanwhile, and its
    /// page kept. Back and still hidden, the page goes after the keep time as usual.
    pub fn suspended(&self) -> bool {
        self.shared.suspended.load(Ordering::SeqCst)
    }

    pub fn set_suspended(&self, value: bool) {
        self.shared.suspended.store(value, Ordering::SeqCst);
        if !value {
            self.on_window(|p| p.tool.release_later());
        }
    }

    /// The window in front before this one took the keyboard (a Roblox window the focus may go back to).
    pub fn before(&self) -> isize {
        self.shared.before.load(Ordering::SeqCst)
    }

    pub fn set_before(&self, h: isize) {
        self.shared.before.store(h, Ordering::SeqCst);
    }

    /// Shown (or brought forward); `activate` false: by itself, the game keeps the keyboard.
    pub fn open(&self, activate: bool) {
        self.on_window(move |p| win_form::spawn(p.clone().open(activate)));
    }

    /// Hidden by the app (its game ended, the macro deleted): not counted as the person closing it.
    pub fn hide_quietly(&self) {
        self.shared.showing.store(false, Ordering::SeqCst);
        self.on_window(|p| p.hide_quietly());
    }

    /// The person closed it (the page's ✕): hidden, and the App hears it (Dismissed).
    pub fn dismiss(&self) {
        self.on_window(|p| p.dismiss());
    }

    /// What the page draws now, from the app: every tick, but only sent when it changed (or while it opens: the page
    /// answers each one with its size, and the window shows then). The message is made here, the "changed" is decided
    /// by the window.
    pub fn push(&self, app: &App) {
        if self.is_disposed() || (!self.visible() && !self.shared.showing.load(Ordering::SeqCst)) {
            return;
        }
        let message = app.panel_message(self.shared.macro_id, self.pill());
        self.on_window(move |p| p.send(message));
    }

    /// A change refused or a Run that couldn't start: said under its row (name) or in the status line (None).
    pub fn error(&self, name: Option<&str>, text: &str) {
        let data = json!({ "name": name, "text": text });
        self.on_window(move |p| p.tool.send_event("panelError", data));
    }

    /// "Snap to game": the kept place forgotten, placed by the game again.
    pub fn snap(&self, app: &App) {
        let removed = prefs::lock().panel_windows.shift_remove(&self.shared.macro_id.to_string()).is_some();
        if removed {
            prefs::save();
        }
        if !self.visible() {
            return;
        }
        let r = by_game(app.panel_game_rect(self.shared.macro_id), self.bounds().size(), &app.panels_shown_besides(self));
        self.on_window(move |p| p.tool.set_location(r.location()));
    }

    /// On top of everything, or just below `front`.
    pub fn layer(&self, on_top: bool, front: isize) {
        self.on_window(move |p| p.layer(on_top, front));
    }

    /// The colour "Pick" with no game open: the panel steps aside (Suspended) while the screen is pictured, and comes back.
    /// `picked` runs on the core thread with the App.
    pub fn pick_screen_colour(&self, custom_theme: Option<Value>, picked: impl Fn(&Rc<App>, String) + Send + Sync + 'static) {
        let picked = Arc::new(picked);
        self.on_window(move |p| {
            let weak = Rc::downgrade(p);
            let suspend: Rc<dyn Fn(bool)> = Rc::new(move |on: bool| {
                if let Some(p) = weak.upgrade() {
                    p.shared.suspended.store(on, Ordering::SeqCst);
                    if !on {
                        p.tool.release_later();
                    }
                }
            });
            let picked = picked.clone();
            super::overlay_window::screen_picker::pick(
                p.clone() as Rc<dyn WebTool>,
                Some(suspend),
                move |colour: &str| {
                    let colour = colour.to_string();
                    let picked = picked.clone();
                    rt::post(move || App::with(|app| picked(app, colour)));
                },
                custom_theme,
            );
        });
    }

    /// Dispose(): the window and its browser go (never Close, which would only hide it).
    pub fn dispose(&self) {
        if self.shared.disposed.swap(true, Ordering::SeqCst) {
            return;
        }
        let s = self.shared.clone();
        win_form::invoke(move || {
            let panel = WINDOWS.with(|w| {
                let mut w = w.borrow_mut();
                match w.get(&s.macro_id) {
                    Some(p) if Arc::ptr_eq(&p.shared, &s) => w.remove(&s.macro_id),
                    _ => None,
                }
            });
            if let Some(p) = panel {
                p.tool.dispose();
            }
            s.hwnd.store(0, Ordering::SeqCst);
        });
    }
}

/// To the App, on the core thread, with this panel's handle (not a newer one for the macro).
fn to_app(shared: &Arc<Shared>, f: impl FnOnce(&Rc<App>, &PanelHandle) + Send + 'static) {
    let s = shared.clone();
    rt::post(move || {
        App::with(|app| {
            let handle = app.panels.panels.borrow().get(&s.macro_id).cloned();
            if let Some(h) = handle
                && Arc::ptr_eq(&h.shared, &s)
            {
                f(app, &h);
            }
        })
    });
}

// ───────────────────────── the window's half ─────────────────────────

pub struct PanelWindow {
    tool: WebToolWindow,
    shared: Arc<Shared>,
    placed: Cell<bool>,
    /// What the page was last sent, and the theme its frame was last drawn in: pushed again only when changed.
    sent: RefCell<Option<String>>,
    framed: RefCell<Option<Theme>>,
    /// The layering error.log was last told (on top, move done).
    layer_logged: Cell<Option<(bool, bool)>>,
}

impl PanelWindow {
    fn new(shared: Arc<Shared>) -> Rc<PanelWindow> {
        let me = Rc::new(PanelWindow {
            tool: WebToolWindow::new(),
            shared,
            placed: Cell::new(false),
            sent: RefCell::new(None),
            framed: RefCell::new(None),
            layer_logged: Cell::new(None),
        });
        me.tool.bind(Rc::downgrade(&me) as Weak<dyn WebTool>);
        // Lowered while a message box shows, so the box opens in front of it (as the mini window and the overlay).
        ui::note_on_top(me.tool.form_ref());
        // PORT: WinForms made the handle when first needed; it is made now so the App's half can read it from its thread.
        let h = me.tool.handle();
        me.shared.hwnd.store(h, Ordering::SeqCst);
        me
    }

    fn short(&self) -> String {
        self.shared.macro_id.to_string()[..8].to_string()
    }

    fn showing(&self) -> bool {
        self.shared.showing.load(Ordering::SeqCst)
    }

    fn set_showing(&self, v: bool) {
        self.shared.showing.store(v, Ordering::SeqCst);
    }

    /// Shown (or brought forward); `activate` false: by itself, the game keeps the keyboard. (C#'s async void Open.)
    async fn open(self: Rc<Self>, activate: bool) {
        if self.tool.visible() {
            self.push();
            if activate {
                log::note(&format!("Macro panel {}: brought to the front (already open).", self.short()));
                self.tool.activate();
            }
            return;
        }
        self.tool.set_activate_on_show(activate);
        self.set_showing(true);
        let me: Rc<dyn WebTool> = self.clone();
        if !WebToolWindow::ensure_browser(me).await || self.tool.is_disposed() {
            if !self.tool.is_disposed() {
                log::note(&format!("Macro panel {}: not shown, its page couldn't start.", self.short()));
            }
            self.set_showing(false);
            return;
        }
        self.push(); // the page answers with its size, and the window shows then (on_page_size)
    }

    /// Push from the window's side: the App makes the message.
    fn push(&self) {
        to_app(&self.shared, |app, h| h.push(app));
    }

    /// Push's second half: the message the App made, sent only when it changed.
    fn send(&self, message: Value) {
        if self.tool.is_disposed() || !self.tool.browser_ready() || (!self.tool.visible() && !self.showing()) {
            return;
        }
        let theme = themes::current();
        if self.framed.borrow().as_ref() != Some(&theme) {
            *self.framed.borrow_mut() = Some(theme);
            self.tool.frame();
        }
        let json = message.to_string();
        if self.sent.borrow().as_deref() == Some(json.as_str()) && self.tool.visible() && !self.showing() {
            return;
        }
        *self.sent.borrow_mut() = Some(json);
        self.tool.send_event("panel", message);
    }

    fn hide_quietly(&self) {
        self.set_showing(false);
        if self.tool.visible() {
            self.tool.hide();
        }
    }

    fn dismiss(&self) {
        log::note(&format!("Macro panel {}: closed by the person.", self.short()));
        self.hide_quietly();
        // Dismissed?.Invoke(): the App's handler (AppController.ShowPanelWindow's lambda, App::panel_dismissed).
        let id = self.shared.macro_id;
        to_app(&self.shared, move |app, _| app.panel_dismissed(id));
    }

    /// The page said its size: placed (with what the App knows of the game and the other panels), then sized and shown.
    fn size_to(&self, css: Size, game: Option<Rect>, others: Vec<Rect>) {
        if !self.tool.visible() && !self.showing() {
            return;
        }
        let visible = self.tool.visible();
        let scale = if visible { self.tool.scale_now() } else { scale_at(self.place_for(scaled(css, 1.0), game, &others).location()) };
        let mut size = scaled(css, scale);
        // Never taller than the screen it is on (the rows scroll inside).
        let from = if visible { self.tool.bounds().location() } else { self.place_for(size, game, &others).location() };
        let work = win_form::screen_from_point(from).working_area;
        size.height = size.height.min(work.height - (24.0 * scale) as i32);
        if !visible {
            let at = self.place_for(size, game, &others);
            // The place first, then the size: set together on a screen at another scale, Windows' scale change resized it
            // again by the ratio (TrayWindow, 2026-10-30).
            self.tool.set_location(at.location());
            self.tool.set_bounds(Rect::at(at.location(), size));
            self.placed.set(true);
            self.set_showing(false);
            self.tool.show();
            let activated = self.tool.activate_on_show();
            if activated {
                self.tool.activate();
            }
            log::note(&format!(
                "Macro panel {}: shown always on top {}, {}.",
                self.short(),
                if activated { "by the person, activated" } else { "by itself, without the keyboard" },
                window_report::describe(self.tool.handle())
            ));
            self.layer_logged.set(Some((true, true)));
            to_app(&self.shared, |app, h| app.panel_shown(h));
        } else if self.tool.bounds().size() != size {
            self.tool.set_size(size); // folded, unfolded, a tab with more rows: the top-left stays
        }
    }

    /// Where it shows: its saved place (on a screen), else by the game.
    fn place_for(&self, size: Size, game: Option<Rect>, others: &[Rect]) -> Rect {
        let saved = prefs::lock().panel_windows.get(&self.shared.macro_id.to_string()).cloned();
        if let Some(at) = saved.as_deref().and_then(parse)
            && all_screens()
                .iter()
                .any(|s| win_form::intersects(s.working_area, Rect::new(at.x + size.width / 4, at.y + 2, 1.max(size.width / 2), 24)))
        {
            return Rect::at(ui::inside(Rect::at(at, size), win_form::screen_from_point(at).working_area, 8), size);
        }
        by_game(game, size, others)
    }

    /// On top of everything (a Roblox window, Bloxkeep or a panel is in front), or just below `front`
    /// (another program came forward: it covers the panel, the panel still covers what it covered before).
    fn layer(&self, on_top: bool, front: isize) {
        if !self.tool.visible() || !self.tool.is_handle_created() {
            return;
        }
        let h = self.tool.handle();
        let after = if on_top { HWND_TOPMOST } else { hwnd(front) };
        // SAFETY: plain call on our own window.
        let placed = unsafe { SetWindowPos(hwnd(h), after, 0, 0, 0, 0, SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE) } != 0;
        // Runs on every foreground change: logged only when this panel's on-top state or the move's success changes.
        if self.layer_logged.get() != Some((on_top, placed)) {
            let mut front_pid = 0u32;
            // SAFETY: plain query; the pid is written to a local.
            unsafe { GetWindowThreadProcessId(hwnd(front), &mut front_pid) };
            // SAFETY: plain query of our own window's style.
            let topmost = (unsafe { GetWindowLongW(hwnd(h), GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST) != 0;
            log::note(&format!(
                "Macro panel {}: {} (in front: pid {front_pid}{}){}; Windows says {}.",
                self.short(),
                if on_top { "on top of everything" } else { "put just below the window in front" },
                if front_pid == std::process::id() { ", Bloxkeep" } else { "" },
                if placed { "" } else { ", but Windows refused the move" },
                if topmost { "always on top" } else { "not on top" }
            ));
            self.layer_logged.set(Some((on_top, placed)));
        }
    }
}

impl WebTool for PanelWindow {
    fn tool(&self) -> &WebToolWindow {
        &self.tool
    }
    fn type_name(&self) -> &'static str {
        "PanelWindow"
    }
    fn page_file(&self) -> &'static str {
        "panel.html"
    }
    fn starts_top_most(&self) -> bool {
        true
    }
    fn keep_browser(&self) -> Duration {
        Duration::from_secs(60)
    }
    /// A panel stepped aside keeps its page. A folded one no longer does once hidden (its game ended): it held its page
    /// for the whole session (audit B13). Shown, folded or not, the page stays anyway (release_browser).
    fn hold_browser(&self) -> bool {
        self.shared.suspended.load(Ordering::SeqCst)
    }

    fn on_page_ready(&self) {
        *self.sent.borrow_mut() = None; // a new page: it has nothing yet
        if self.tool.visible() || self.showing() {
            self.push();
        }
    }

    fn on_page_size(&self, css: Size, args: &Value) {
        if !self.tool.visible() && !self.showing() {
            return;
        }
        self.shared.pill.store(args.get("pill") == Some(&Value::Bool(true)), Ordering::SeqCst);
        // PanelGameRect and PanelsShownBesides are the App's: asked on its thread, then placed here.
        to_app(&self.shared, move |app, h| {
            let game = app.panel_game_rect(h.macro_id());
            let others = app.panels_shown_besides(h);
            h.on_window(move |p| p.size_to(css, game, others));
        });
    }

    /// Dragged by its title: that place is kept for this macro.
    fn on_dragged(&self) {
        if !self.tool.visible() || !self.placed.get() {
            return;
        }
        let at = self.tool.bounds().location();
        prefs::lock().panel_windows.insert(self.shared.macro_id.to_string(), format!("{},{}", at.x, at.y));
        prefs::save();
    }

    fn on_act(&self, what: &str, args: &Value) {
        let (what, args) = (what.to_string(), args.clone());
        to_app(&self.shared, move |app, h| app.panel_act(h, &what, &args));
    }

    fn on_browser_failed(&self) {
        let shown = self.tool.visible();
        self.tool.browser_failed_base(self.type_name());
        if shown {
            log::note(&format!("Macro panel {}: opened again after its page crashed.", self.short()));
            let id = self.shared.macro_id;
            let s = self.shared.clone();
            let panel = WINDOWS.with(|w| w.borrow().get(&id).cloned()).filter(|p| Arc::ptr_eq(&p.shared, &s));
            if let Some(p) = panel {
                win_form::spawn(p.open(false));
            }
        }
    }

    /// WM_ACTIVATE (not WA_INACTIVE): the App hears which window had the keyboard before.
    /// PORT: WebToolWindow's on_activated doesn't pass the message's lParam (the window deactivated); 0 is passed, which
    /// is what Windows gives for another program's window anyway, and the App then takes the last window in front.
    fn on_activated(&self) {
        // The page takes the keyboard, as WinForms gave it back to the browser control on activation.
        self.tool.focus_page();
        to_app(&self.shared, |app, h| app.panel_activated(h, 0));
    }

    /// Alt+F4 (and Windows asking it to close) only hides it; the app disposes it itself (Dispose, never Close).
    fn on_form_closing(&self, reason: CloseReason) -> bool {
        if reason == CloseReason::UserClosing {
            self.dismiss();
            return true;
        }
        false
    }

    fn on_disposing(&self) {
        self.shared.hwnd.store(0, Ordering::SeqCst);
    }
}

// ───────────────────────── placement ─────────────────────────

/// Right of the game + 8 px, top-aligned, when it fits that monitor's work area; else left of it; else the old place
/// (the work area's right edge, 80 px down). Never over the game: Roblox's player list is at its top right, and a
/// macro's click under a Bloxkeep window makes that window click-through for a moment, which can blank its page.
/// Other panels already there push it down, 8 px apart.
pub fn by_game(game: Option<Rect>, size: Size, others: &[Rect]) -> Rect {
    let work;
    let at;
    match game {
        Some(g) if g.width > 0 && g.height > 0 => {
            work = screen_from_rect(g).working_area;
            if g.right() + GAP + size.width <= work.right() {
                at = Point::new(g.right() + GAP, work.top().max(g.top()));
            } else if g.left() - GAP - size.width >= work.left() {
                at = Point::new(g.left() - GAP - size.width, work.top().max(g.top()));
            } else {
                at = Point::new(work.right() - size.width - 16, work.top() + 80);
            }
        }
        _ => {
            work = win_form::primary_screen().working_area;
            at = Point::new(work.right() - size.width - 16, work.top() + 80);
        }
    }
    stack(Rect::at(at, size), others, work)
}

/// Moved down below every panel it would cover, 8 px apart, kept on the work area.
pub fn stack(mut r: Rect, others: &[Rect], work: Rect) -> Rect {
    for _ in 0..20 {
        let Some(hit) = others.iter().filter(|o| win_form::intersects(**o, r)).map(|o| o.bottom()).max() else {
            break;
        };
        r.y = hit + GAP;
    }
    if r.bottom() > work.bottom() {
        r.y = work.top().max(work.bottom() - r.height);
    }
    r
}

/// "x,y" as saved (int.TryParse with the invariant culture: white space around and a sign are taken).
fn parse(text: &str) -> Option<Point> {
    let p: Vec<&str> = text.split(',').collect();
    if p.len() < 2 {
        return None;
    }
    let x = p[0].trim().parse::<i32>().ok()?;
    let y = p[1].trim().parse::<i32>().ok()?;
    Some(Point::new(x, y))
}
