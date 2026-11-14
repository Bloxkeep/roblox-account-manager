// TrayWindow.cs: the tray icon's quick panel (left click) and menu (right click): tray.html (ui/src/Tray.tsx) in a small
// frameless window above the taskbar by the pointer (until 2026-10-26 hand-drawn WinForms, which the owner found ugly).
// It uses the main window's browser profile, started on first open and let go a minute after the last close (reopening
// then takes about a second). While open, the state is pushed every two seconds.
//
// On the windows thread. What it draws comes from the App (TrayState and the rest, on the core's thread through
// rt::post), and comes back here with win_form::invoke to the one tray window (`current`).
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    sync::atomic::{AtomicIsize, Ordering},
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
        Shell::{ABM_GETTASKBARPOS, APPBARDATA, SHAppBarMessage},
        WindowsAndMessaging::{
            AppendMenuW, CS_DROPSHADOW, CreatePopupMenu, DestroyMenu, GetClientRect, MF_STRING, PostMessageW, TPM_BOTTOMALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON,
            TrackPopupMenuEx, WA_INACTIVE, WM_ACTIVATE, WM_ERASEBKGND, WM_SIZE, WS_CLIPCHILDREN, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
        },
    },
};

use super::{
    app::App,
    app_pages,
    base::{Cancel, Fail, Guid, Res},
    clock, l, log, rt, shell_form, themes,
    ui,
    web_tool_window::{browser::Browser, dpi_at, num},
    win_form::{self, Timer, colorref},
    win_kit::{Point, Rect, Size, hwnd, wide},
    window_report,
};

/// How long the browser is kept after the tray last closed: opened again meanwhile, it is there at once.
const KEEP_BROWSER: Duration = Duration::from_secs(60);

const ABE_LEFT: i32 = 0;
const ABE_TOP: i32 = 1;
const ABE_RIGHT: i32 = 2;
const ABE_BOTTOM: i32 = 3;

thread_local! {
    static CURRENT: RefCell<Weak<TrayWindow>> = RefCell::new(Weak::new());
}

type Starting = Shared<LocalBoxFuture<'static, Result<(), Fail>>>;

/// What a click on the page asks of the app (the C# events).
#[derive(Default)]
pub struct TrayEvents {
    pub open_clicked: Option<Box<dyn Fn()>>,
    pub mini_clicked: Option<Box<dyn Fn()>>,
    /// Multi-Roblox's switch, with whether it is on now (the page offers it only where it can switch from here).
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
    handle: AtomicIsize,
    disposed: Cell<bool>,
    refresh: Timer,
    release: Timer,
    web: RefCell<Option<Rc<Browser>>>,
    web_gen: Cell<u64>,
    next_gen: Cell<u64>,
    starting: RefCell<Option<(u64, Starting)>>,
    start_cancel: RefCell<Option<Cancel>>,
    ready: RefCell<Option<Rc<(Cell<bool>, RefCell<Option<oneshot::Sender<()>>>)>>>,
    /// "panel" or "menu"; a new count each time it opens, so the page starts on its first page.
    mode: RefCell<String>,
    opened: Cell<i32>,
    anchor: Cell<Point>,
    /// The page's size in its own pixels, as it last said.
    content: Cell<Size>,
    showing: Cell<bool>,
    hidden_at: Cell<i64>,
    /// What the page was last sent (and to which browser), and the theme the window was last drawn in.
    sent: RefCell<Option<String>>,
    sent_to: Cell<u64>,
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
            release: Timer::new(KEEP_BROWSER.as_millis() as u32),
            web: RefCell::new(None),
            web_gen: Cell::new(0),
            next_gen: Cell::new(1),
            starting: RefCell::new(None),
            start_cancel: RefCell::new(None),
            ready: RefCell::new(None),
            mode: RefCell::new("panel".into()),
            opened: Cell::new(0),
            anchor: Cell::new(Point::default()),
            content: Cell::new(Size::new(344, 300)),
            showing: Cell::new(false),
            hidden_at: Cell::new(i64::MIN / 2),
            sent: RefCell::new(None),
            sent_to: Cell::new(0),
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
        let w = Rc::downgrade(&me);
        me.release.on_tick(move || {
            if let Some(t) = w.upgrade() {
                t.release_browser();
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

    /// The window's handle, made now when it has none (borderless, on top, not in Alt+Tab, Windows' shadow).
    fn ensure_handle(self: &Rc<Self>) -> isize {
        let h = self.hwnd();
        if h != 0 || self.disposed.get() {
            return h;
        }
        let weak = Rc::downgrade(self);
        let handler: win_form::Handler = Rc::new(move |h, msg, wparam, _lparam| {
            let me = weak.upgrade()?;
            match msg {
                WM_ERASEBKGND => {
                    // SAFETY: the DC is Windows' for this message; the brush is made and freed here.
                    unsafe {
                        let mut r: RECT = std::mem::zeroed();
                        GetClientRect(hwnd(h), &mut r);
                        let brush = CreateSolidBrush(colorref(themes::current().surface));
                        FillRect(wparam as HDC, &r, brush);
                        DeleteObject(brush);
                    }
                    Some(1)
                }
                WM_SIZE => {
                    let web = me.web.borrow().clone();
                    if let Some(web) = web {
                        web.set_bounds(win_form::client_size(h));
                    }
                    None
                }
                WM_ACTIVATE if (wparam & 0xFFFF) as u32 == WA_INACTIVE => {
                    me.on_deactivate();
                    None
                }
                _ => None,
            }
        });
        let Ok(h) = win_form::create_window("Bloxkeep.Tray", CS_DROPSHADOW, WS_POPUP | WS_CLIPCHILDREN, WS_EX_TOOLWINDOW | WS_EX_TOPMOST, Rect::new(0, 0, 344, 300), 0, ui::TITLE, handler)
        else {
            return 0;
        };
        self.handle.store(h, Ordering::SeqCst);
        // OnHandleCreated.
        self.frame();
        h
    }

    pub fn visible(&self) -> bool {
        win_form::is_visible(self.hwnd())
    }

    /// The tray icon was clicked just as the tray went away (the click itself took it away): don't bring it back.
    pub fn recently_hidden(&self) -> bool {
        clock::tick() - self.hidden_at.get() < 400
    }

    /// Opens the panel or the menu by the tray icon (the pointer at `anchor`); the same again closes it.
    pub fn toggle(self: &Rc<Self>, anchor: Point, mode: &str) {
        let me = self.clone();
        let mode = mode.to_string();
        win_form::spawn(async move {
            if me.visible() {
                let same = *me.mode.borrow() == mode;
                me.hide_tray();
                if same {
                    return;
                }
            } else if me.recently_hidden() && *me.mode.borrow() == mode {
                return;
            }
            *me.mode.borrow_mut() = mode.clone();
            me.anchor.set(anchor);
            me.opened.set(me.opened.get() + 1);
            me.showing.set(true);
            me.release.stop();
            match TrayWindow::ensure_browser(me.clone()).await {
                Ok(true) => {}
                Ok(false) => {
                    log::note(&format!("Tray: {mode} not shown, its page was let go while starting."));
                    return; // let go meanwhile (closed again, its page failed, the app quitting)
                }
                Err(e) => {
                    log::write(&e);
                    me.showing.set(false);
                    me.release_browser();
                    // The page can't start (WebView2 missing or broken): Windows' own small menu, so the app can still be
                    // opened and quit from the tray.
                    me.fallback_menu(anchor);
                    return;
                }
            }
            me.push(); // the page answers with its size, and the window shows then (on_size)
            // A page that never answers: not left "showing", or its browser was kept for good.
            let opened = me.opened.get();
            win_form::delay(15_000).await;
            if me.showing.get() && me.opened.get() == opened && !me.visible() {
                log::note(&format!("Tray: {mode} not shown, its page didn't draw in 15 s."));
                me.showing.set(false);
                me.release.start();
            }
        });
    }

    /// Open and Quit in a menu Windows draws itself, at the pointer (only when the tray's page can't start).
    fn fallback_menu(self: &Rc<Self>, at: Point) {
        // SAFETY: a popup menu made, tracked and destroyed here; the strings live for each call.
        unsafe {
            let menu = CreatePopupMenu();
            if menu.is_null() {
                log::note("Tray: its page can't start and Windows' own menu couldn't be made either; nothing shown.");
                return;
            }
            log::note("Tray: its page can't start; Windows' own small menu shown instead.");
            let open = wide(&l::t("Open Bloxkeep"));
            let quit = wide(&l::t("Quit Bloxkeep"));
            AppendMenuW(menu, MF_STRING, 1, open.as_ptr());
            AppendMenuW(menu, MF_STRING, 2, quit.as_ptr());
            // Windows' rule for tray menus: its owner in front, so a click elsewhere closes it, and a message after it.
            let h = self.ensure_handle();
            win_form::set_foreground(h);
            let chosen = TrackPopupMenuEx(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN, at.x, at.y, hwnd(h), std::ptr::null());
            PostMessageW(hwnd(h), 0, 0, 0);
            DestroyMenu(menu);
            if chosen == 1 {
                if let Some(f) = self.events.borrow().open_clicked.as_ref() {
                    f();
                }
            } else if chosen == 2 {
                if let Some(f) = self.events.borrow().quit_clicked.as_ref() {
                    f();
                }
            }
        }
    }

    pub fn hide_tray(&self) {
        self.showing.set(false);
        if !self.visible() {
            return;
        }
        self.hidden_at.set(clock::tick());
        self.refresh.stop();
        let web = self.web.borrow().clone();
        if let Some(web) = web {
            web.set_visible(false);
        }
        win_form::hide(self.hwnd());
        log::note(&format!("Tray: {} hidden.", self.mode.borrow()));
        self.release.start();
    }

    fn on_deactivate(&self) {
        // Only once on screen: the browser starting up in the hidden window can take the activation away too.
        if self.visible() {
            self.hide_tray();
        }
    }

    fn browser_ready(&self) -> bool {
        self.web.borrow().is_some() && self.ready.borrow().as_ref().is_some_and(|r| r.0.get())
    }

    /// Starts the page (once). Ok(false) when the start was let go meanwhile (the tray closed as its page failed, the
    /// app quitting); a start that failed is an Err.
    async fn ensure_browser(me: Rc<TrayWindow>) -> Res<bool> {
        while !me.browser_ready() {
            if me.disposed.get() {
                return Ok(false);
            }
            if me.starting.borrow().is_none() {
                let cancel = Cancel::new();
                *me.start_cancel.borrow_mut() = Some(cancel.clone());
                let id = me.next_gen.get();
                me.next_gen.set(id + 1);
                let fut: LocalBoxFuture<'static, Res<()>> = TrayWindow::start_browser(me.clone(), cancel).boxed_local();
                *me.starting.borrow_mut() = Some((id, fut.shared()));
            }
            let Some((id, starting)) = me.starting.borrow().clone() else { return Ok(false) };
            let result = starting.await;
            let current = me.starting.borrow().as_ref().map(|(i, _)| *i);
            if let Err(e) = result {
                // Let go meanwhile: its failure is nobody's (WebToolWindow.EnsureBrowserAsync).
                if current == Some(id) {
                    *me.starting.borrow_mut() = None;
                    return Err(e);
                }
            }
            if current != Some(id) {
                if current.is_none() {
                    return Ok(false);
                }
                continue; // a newer start is the one that counts
            }
            if !me.browser_ready() {
                *me.starting.borrow_mut() = None;
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// `cancel`: called off when the tray lets this start go (`release_browser`, Dispose).
    async fn start_browser(me: Rc<TrayWindow>, cancel: Cancel) -> Res<()> {
        let env = win_form::wait_for(app_pages::create_environment(), u32::MAX, &cancel).await??;
        if me.disposed.get() {
            return Err(Fail::other("Cannot access a disposed object. Object name: 'TrayWindow'."));
        }
        let generation = me.next_gen.get();
        me.next_gen.set(generation + 1);
        me.web_gen.set(generation);
        // The browser starts once its control has a window, and a hidden form's controls get none by themselves.
        let h = me.ensure_handle();
        let web = Rc::new(win_form::wait_for(Browser::create(h, &env, generation), u32::MAX, &cancel).await??);
        // Disposed or let go while the browser started: nothing of this start is used.
        if me.disposed.get() || me.web_gen.get() != generation {
            web.close();
            return Err(Fail::other("Cannot access a disposed object. Object name: 'TrayWindow'."));
        }
        web.set_background(themes::current().surface);
        web.set_bounds(win_form::client_size(h));
        // Its page draws only while it is told it is seen: made in the hidden window, it is told so when the window
        // shows (`on_size`). Never told, it stayed an empty box (the owner, 2026-11-13; the mini window's same fault).
        web.set_visible(win_form::is_visible(h));
        *me.web.borrow_mut() = Some(web.clone());
        app_pages::configure(web.core(), &env)?;
        let weak = Rc::downgrade(&me);
        // Loaded: its first "ready" can come before the page is listened to, so the load itself counts as ready too.
        web.on_navigation_completed({
            let weak = weak.clone();
            move |ok| {
                if let (true, Some(t)) = (ok, weak.upgrade()) {
                    t.set_ready();
                }
            }
        });
        // Its page gone (the renderer crashed or hung, the browser gone): the next open starts it again. Not for a GPU
        // process restart, which the page lives through (shell_form::failure_of).
        web.on_process_failed({
            let weak = weak.clone();
            move |kind| {
                if !matches!(shell_form::failure_of(kind), shell_form::BrowserFailure::ReloadPage | shell_form::BrowserFailure::BrowserGone) {
                    return;
                }
                let Some(t) = weak.upgrade() else { return };
                if t.hwnd() == 0 || t.disposed.get() {
                    return;
                }
                let weak = weak.clone();
                win_form::spawn(async move {
                    let Some(t) = weak.upgrade() else { return };
                    if t.web_gen.get() != generation {
                        return;
                    }
                    log::note("Tray: its page crashed; hidden, started again on the next open.");
                    t.hide_tray();
                    t.release_browser();
                });
            }
        });
        web.on_web_message({
            let weak = weak.clone();
            move |json| {
                let weak = weak.clone();
                win_form::spawn(async move {
                    if let Some(t) = weak.upgrade() {
                        t.on_message(&json);
                    }
                });
            }
        });
        let (tx, rx) = oneshot::channel();
        *me.ready.borrow_mut() = Some(Rc::new((Cell::new(false), RefCell::new(Some(tx)))));
        web.navigate(&format!("{}tray.html", app_pages::ORIGIN))?;
        win_form::wait_for(rx, 15_000, &cancel).await?.map_err(|_| Fail::other("cancelled"))?;
        Ok(())
    }

    fn set_ready(&self) {
        let ready = self.ready.borrow().clone();
        if let Some(r) = ready {
            r.0.set(true);
            if let Some(tx) = r.1.borrow_mut().take() {
                let _ = tx.send(());
            }
        }
    }

    /// The browser goes a minute after the tray last closed (its memory with it); the next open starts it again.
    /// A start under way is let go too.
    fn release_browser(&self) {
        self.release.stop();
        if self.visible() || self.showing.get() {
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

    /// The page's messages, as the app's page sends them: { id, cmd, args }.
    pub fn on_message(&self, json: &str) {
        let Ok(root) = serde_json::from_str::<Value>(json) else {
            log::note("Tray: a message from its page couldn't be read.");
            return;
        };
        let cmd = root.get("cmd").and_then(Value::as_str).unwrap_or("");
        let args = root.get("args").filter(|a| a.is_object()).cloned().unwrap_or(Value::Null);
        match cmd {
            "ready" => self.set_ready(),
            "size" if args.is_object() => self.on_size(num(&args, "w"), num(&args, "h"), num(&args, "opened")),
            "act" if args.is_object() => self.act(&args),
            _ => {}
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

    /// What the page draws: which part, the state now, the theme and the language. On the core's thread (the App's
    /// TrayState, TrayPictureUrl and TrayCustomTheme).
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

    /// What the page draws now: on open, after a click and on the refresh; sent only when it changed, except while
    /// it opens (the page answers with its size, and the window shows then). The frame is drawn again on a theme change.
    fn push(&self) {
        if self.web.borrow().is_none() || (!self.visible() && !self.showing.get()) {
            return;
        }
        let theme = themes::current();
        if self.framed.borrow().as_ref() != Some(&theme) {
            *self.framed.borrow_mut() = Some(theme);
            win_form::invalidate(self.hwnd());
            self.frame();
        }
        let mode = self.mode.borrow().clone();
        let opened = self.opened.get();
        let mini_open = self.mini_open.borrow().as_ref().is_some_and(|f| f());
        rt::post(move || {
            App::with(|app| {
                let message = TrayWindow::message(app, &mode, opened, mini_open);
                win_form::invoke(move || {
                    if let Some(t) = TrayWindow::current() {
                        t.send(message);
                    }
                });
            })
        });
    }

    fn send(&self, message: Value) {
        let web = self.web.borrow().clone();
        let Some(web) = web else { return };
        if !self.visible() && !self.showing.get() {
            return;
        }
        let json = message.to_string();
        if self.sent.borrow().as_deref() == Some(json.as_str()) && web.id == self.sent_to.get() && self.visible() && !self.showing.get() {
            return;
        }
        match web.post_json(&json) {
            Ok(()) => {
                *self.sent.borrow_mut() = Some(json);
                self.sent_to.set(web.id);
            }
            Err(e) => log::write(&e),
        }
    }

    /// The page's size; the window shows once the page has drawn this opening (`opened`), not what it showed before.
    fn on_size(&self, width: i32, height: i32, opened: i32) {
        if width <= 0 || height <= 0 || (self.showing.get() && opened != self.opened.get()) {
            return;
        }
        self.content.set(Size::new(width, height));
        if !self.showing.get() && !self.visible() {
            return;
        }
        self.place();
        if self.showing.get() && !self.visible() {
            self.showing.set(false);
            let h = self.hwnd();
            win_form::show(h, true);
            win_form::activate(h);
            let front = win_form::set_foreground(h);
            log::note(&format!(
                "Tray: {} shown always on top, {}{}.",
                self.mode.borrow(),
                window_report::describe(h),
                if front { "" } else { "; Windows didn't bring it to the front" }
            ));
            let web = self.web.borrow().clone();
            if let Some(web) = web {
                web.set_visible(true);
                web.focus();
            }
            self.refresh.start();
        }
        self.showing.set(false);
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

    pub fn dispose(&self) {
        if self.disposed.replace(true) {
            return;
        }
        self.refresh.dispose();
        self.release.dispose();
        let web = self.web.borrow_mut().take();
        if let Some(web) = web {
            web.close();
        }
        *self.starting.borrow_mut() = None;
        if let Some(c) = self.start_cancel.borrow_mut().take() {
            c.cancel();
        }
        win_form::destroy(self.handle.swap(0, Ordering::SeqCst));
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
