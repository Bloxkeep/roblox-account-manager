// WebsiteForm.cs: Bloxkeep's browser for www.roblox.com.
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{Arc, Mutex, mpsc as std_mpsc},
};

use futures::{channel::oneshot, future::join_all};
use serde_json::{Value, json};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, RECT, WPARAM},
    UI::{
        Input::KeyboardAndMouse::{GetFocus, VK_NEXT, VK_PRIOR, VK_TAB},
        WindowsAndMessaging::{
            DestroyWindow, IsChild, MINMAXINFO, SW_HIDE, SW_RESTORE, SW_SHOW, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SetForegroundWindow, SetWindowPos,
            WA_INACTIVE, WM_ACTIVATE, WM_CLOSE, WM_DESTROY, WM_DPICHANGED, WM_ERASEBKGND, WM_GETMINMAXINFO, WM_KEYDOWN, WM_SIZE, WS_CHILD, WS_CLIPCHILDREN,
            WS_CLIPSIBLINGS, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
        },
    },
};

use super::{
    chrome_view::{
        ChromeView, look,
        host::{self, Invoker, KEYS_CONTROL, KEYS_SHIFT},
    },
    website_tab::{TabCore, WebsiteTab},
};
use crate::core::{
    base::Guid,
    l, themes, ui,
    win_kit::{Rect, screen_from_handle},
};

/// A saved account as the website window's account picker shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct WebsiteAccount {
    pub id: Guid,
    pub name: String,
    pub avatar: Option<String>,
}

type NewBlankTabFn = Box<dyn Fn()>;
type AccountsFn = Box<dyn Fn() -> Vec<WebsiteAccount>>;
type SwitchAccountFn = Box<dyn Fn(WebsiteTab, Guid)>;
type FormClosedFn = Box<dyn Fn()>;

// Keys values (System.Windows.Forms.Keys).
const KEY_T: u32 = 0x54;
const KEY_W: u32 = 0x57;

thread_local! {
    /// The website window of this thread (each has a thread of its own, as WinForms' UI thread).
    static FORM: RefCell<Option<Rc<FormCore>>> = const { RefCell::new(None) };
}

/// Bloxkeep's browser for www.roblox.com: one window, one tab per account (`WebsiteTab`, each its own private sign-in),
/// plus blank tabs (+) for signing in on the site. Its top (the tabs, back/forward/reload, the site, the status line,
/// Save to Bloxkeep) is the app's own page (`ChromeView`, apart from the roblox.com ones).
///
/// This is the window as the controller holds it, on the core's thread; the window runs on a thread of its own.
#[derive(Clone)]
pub struct WebsiteForm(Arc<FormShared>);

impl PartialEq for WebsiteForm {
    fn eq(&self, other: &WebsiteForm) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

struct FormShared {
    invoker: Option<Invoker>,
    hwnd: isize,
    mirror: Mutex<FormMirror>,
}

#[derive(Default)]
struct FormMirror {
    tabs: Vec<WebsiteTab>,
    selected: Option<WebsiteTab>,
    disposed: bool,
    /// The callbacks the app set, kept on the core's thread (their numbers there).
    new_blank_tab: Option<u64>,
    accounts: Option<u64>,
    switch_account: Option<u64>,
    form_closed: Vec<u64>,
}

impl WebsiteForm {
    /// new WebsiteForm(): the window is made (hidden) on its own thread before this returns.
    pub fn new() -> WebsiteForm {
        let (tx, rx) = std_mpsc::channel::<(Invoker, isize)>();
        // PORT: WinForms made the form on the one UI thread; here it gets a thread of its own, and the core waits the moment
        // it takes to make the window, so every call after this reaches it in order.
        let started = host::start_thread("website window", move || {
            let made = FormCore::open();
            if made.is_none() {
                host::quit(); // no window: the thread ends at once
            }
            let _ = tx.send(made.map(|me| (Invoker::current(), me.hwnd)).unwrap_or((Invoker(0), 0)));
        });
        let (invoker, hwnd) = if started { rx.recv().unwrap_or((Invoker(0), 0)) } else { (Invoker(0), 0) };
        let ok = hwnd != 0;
        let shared = Arc::new(FormShared {
            invoker: if ok { Some(invoker) } else { None },
            hwnd,
            mirror: Mutex::new(FormMirror { disposed: !ok, ..FormMirror::default() }),
        });
        if ok {
            // The window learns its handle on the core's side, to keep the mirror (tabs, selected, closed) up to date.
            let s = shared.clone();
            invoker.run(move || {
                FORM.with(|f| {
                    if let Some(me) = f.borrow().as_ref() {
                        *me.shared.borrow_mut() = Some(s);
                    }
                });
            });
        }
        WebsiteForm(shared)
    }

    fn mirror(&self) -> std::sync::MutexGuard<'_, FormMirror> {
        self.0.mirror.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Runs `f` with the window on its thread; false when it's gone.
    fn on_form(&self, f: impl FnOnce(&Rc<FormCore>) + Send + 'static) -> bool {
        let Some(invoker) = self.0.invoker else { return false };
        invoker.run(move || {
            let me = FORM.with(|f| f.borrow().clone());
            if let Some(me) = me {
                f(&me);
            }
        })
    }

    /// NewBlankTab: opens a blank tab in this window (the + button, Ctrl+T); set by the app, which keeps the list of tabs.
    pub fn set_new_blank_tab(&self, f: impl Fn() + 'static) {
        let id = host::keep_on_core::<NewBlankTabFn>(Box::new(f));
        if let Some(old) = self.mirror().new_blank_tab.replace(id) {
            host::drop_on_core(old);
        }
    }

    /// Accounts: the saved accounts the top's account picker lists (as the app shows them); set by the app.
    pub fn set_accounts(&self, f: impl Fn() -> Vec<WebsiteAccount> + 'static) {
        let id = host::keep_on_core::<AccountsFn>(Box::new(f));
        if let Some(old) = self.mirror().accounts.replace(id) {
            host::drop_on_core(old);
        }
    }

    /// SwitchAccount: an account picked on the top for this tab; set by the app, which keeps one tab per account.
    pub fn set_switch_account(&self, f: impl Fn(WebsiteTab, Guid) + 'static) {
        let id = host::keep_on_core::<SwitchAccountFn>(Box::new(f));
        if let Some(old) = self.mirror().switch_account.replace(id) {
            host::drop_on_core(old);
        }
    }

    /// FormClosed += f (on the core's thread).
    pub fn on_form_closed(&self, f: impl Fn() + 'static) {
        let id = host::keep_on_core::<FormClosedFn>(Box::new(f));
        self.mirror().form_closed.push(id);
    }

    pub fn is_disposed(&self) -> bool {
        self.mirror().disposed || !host::is_window(self.0.hwnd)
    }

    pub fn visible(&self) -> bool {
        !self.is_disposed() && host::is_visible(self.0.hwnd)
    }

    /// WindowState == Minimized.
    pub fn is_minimized(&self) -> bool {
        !self.is_disposed() && host::is_minimized(self.0.hwnd)
    }

    /// Opens beside the main window, centred on its screen.
    pub fn place_near(&self, owner: isize) {
        self.on_form(move |me| me.place_near(owner));
    }

    /// Show(): the first time, placed first (OnLoad).
    pub fn show(&self) {
        self.on_form(|me| me.show());
    }

    /// WindowState = Normal, from minimized.
    pub fn restore(&self) {
        let hwnd = self.0.hwnd;
        self.on_form(move |_| host::show(hwnd, SW_RESTORE as i32));
    }

    /// Activate().
    pub fn activate(&self) {
        let hwnd = self.0.hwnd;
        self.on_form(move |_| {
            // SAFETY: plain call.
            unsafe { SetForegroundWindow(hwnd as HWND) };
        });
    }

    pub fn add_tab(&self, tab: &WebsiteTab) {
        let tab = tab.clone();
        self.on_form(move |me| me.add_tab(tab));
    }

    pub fn select_tab(&self, tab: &WebsiteTab) {
        let tab = tab.clone();
        self.on_form(move |me| me.select_tab_by_handle(&tab));
    }

    /// The top again: the saved accounts or their names changed.
    pub fn refresh_top(&self) {
        self.on_form(|me| me.push());
    }

    /// Closes one tab (after a last read of a saved account's session); the window goes with its last tab.
    pub async fn close_tab(&self, tab: &WebsiteTab) {
        let (tx, rx) = oneshot::channel::<()>();
        let tab = tab.clone();
        let posted = self.on_form(move |me| {
            let me = me.clone();
            host::spawn(async move {
                me.close_tab(&tab).await;
                let _ = tx.send(());
            });
        });
        if posted {
            let _ = rx.await;
        }
    }

    /// Closes every tab at once, without the last read of the sessions (the vault is locking, or the app quitting).
    pub fn close_now(&self) {
        self.on_form(|me| me.close_now());
    }

}

/// The window itself, on its thread.
struct FormCore {
    hwnd: isize,
    theme: themes::Theme,
    pages: isize,
    chrome: Rc<ChromeView>,
    /// The handle the controller holds: told what the tabs are now.
    shared: RefCell<Option<Arc<FormShared>>>,
    tabs: RefCell<Vec<(Rc<TabCore>, String)>>,
    selected: RefCell<Option<Rc<TabCore>>>,
    closing_all: Cell<bool>,
    done: Cell<bool>,
    push_soon: Cell<bool>,
    keys: Cell<u32>,
    loaded: Cell<bool>,
    disposed: Cell<bool>,
    place_near: Cell<isize>,
}

impl FormCore {
    fn open() -> Option<Rc<FormCore>> {
        let theme = themes::current();
        let hwnd = host::create_window(host::CLASS_FORM, "Roblox", WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN, 0, 0, Rect::default());
        if hwnd == 0 {
            return None;
        }
        host::set_icon(hwnd);
        let pages = host::create_window(host::CLASS_CHILD, "", WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN | WS_CLIPSIBLINGS, 0, hwnd, Rect::default());
        let chrome = ChromeView::new(hwnd, 84);
        let me = Rc::new(FormCore {
            hwnd,
            theme: theme.clone(),
            pages,
            chrome,
            shared: RefCell::new(None),
            tabs: RefCell::new(Vec::new()),
            selected: RefCell::new(None),
            closing_all: Cell::new(false),
            done: Cell::new(false),
            push_soon: Cell::new(false),
            keys: Cell::new(0),
            loaded: Cell::new(false),
            disposed: Cell::new(false),
            place_near: Cell::new(0),
        });
        let weak = Rc::downgrade(&me);
        // Placed by hand (lay_out), not docked: the top's account menu reaches over the page below while it's open.
        me.chrome.set_relayout(Rc::new({
            let weak = weak.clone();
            move || {
                if let Some(me) = weak.upgrade() {
                    me.lay_out();
                }
            }
        }));
        me.chrome.on_act(Rc::new({
            let weak = weak.clone();
            move |what: &str, args: &Value| {
                if let Some(me) = weak.upgrade() {
                    me.on_act(what, args);
                }
            }
        }));
        // KeyPreview: the window's keys, whichever of its browsers has the focus.
        me.chrome.control().on_keys(me.key_handler());
        host::set_handler(hwnd, {
            let weak = weak.clone();
            move |h: HWND, msg: u32, w: WPARAM, l: LPARAM| weak.upgrade()?.on_message(h, msg, w, l)
        });
        host::set_handler(pages, {
            let weak = weak.clone();
            move |h: HWND, msg: u32, w: WPARAM, l: LPARAM| {
                let me = weak.upgrade()?;
                match msg {
                    WM_ERASEBKGND => Some(host::fill_background(h, w, me.theme.background)),
                    WM_SIZE => {
                        me.fit_tabs();
                        Some(0)
                    }
                    WM_KEYDOWN => me.on_key_down(w, l),
                    _ => None,
                }
            }
        });
        FORM.with(|f| *f.borrow_mut() = Some(me.clone()));

        // OnHandleCreated: the themed title bar, the top's page.
        ui::use_themed_title_bar(hwnd, &me.theme);
        // In the loop's turn: the core, waiting for this window to exist, is answered first.
        {
            let me = me.clone();
            host::defer(move || {
                host::spawn(async move {
                    if me.chrome.start().await {
                        me.push();
                    }
                });
            });
        }
        // After the top and the page exist: a minimum above the form's size resizes it, which lays it out (WM_GETMINMAXINFO).
        me.lay_out();
        Some(me)
    }

    fn key_handler(self: &Rc<Self>) -> Rc<dyn Fn(u32) -> bool> {
        let weak = Rc::downgrade(self);
        Rc::new(move |keys: u32| weak.upgrade().map(|me| me.process_cmd_key(keys)).unwrap_or(false))
    }

    fn on_key_down(self: &Rc<Self>, w: WPARAM, _l: LPARAM) -> Option<isize> {
        if self.process_cmd_key(host::keys_now(w as u32)) { Some(0) } else { None }
    }

    fn on_message(self: &Rc<Self>, h: HWND, msg: u32, w: WPARAM, l: LPARAM) -> Option<isize> {
        match msg {
            WM_SIZE => {
                self.lay_out();
                self.push(); // the room the account menu has
                Some(0)
            }
            WM_ERASEBKGND => Some(host::fill_background(h, w, self.theme.background)),
            WM_GETMINMAXINFO => {
                let dpi = host::dpi(self.hwnd);
                // SAFETY: lparam is the MINMAXINFO Windows asks about, alive for this message.
                let info = unsafe { &mut *(l as *mut MINMAXINFO) };
                info.ptMinTrackSize.x = host::logical(480, dpi);
                info.ptMinTrackSize.y = host::logical(360, dpi);
                Some(0)
            }
            WM_DPICHANGED => {
                // SAFETY: lparam is the RECT Windows suggests for the new scale.
                let r = unsafe { &*(l as *const RECT) };
                // SAFETY: plain call.
                unsafe { SetWindowPos(h, std::ptr::null_mut(), r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOZORDER | SWP_NOACTIVATE) };
                self.chrome.dpi_changed();
                Some(0)
            }
            // Activated: back from the main window (where the theme may have changed): drawn again in it.
            WM_ACTIVATE => {
                if (w & 0xFFFF) as u32 != WA_INACTIVE {
                    self.push();
                }
                None
            }
            WM_KEYDOWN => self.on_key_down(w, l),
            WM_CLOSE => {
                self.form_closing();
                Some(0)
            }
            WM_DESTROY => {
                self.form_closed();
                host::quit();
                Some(0)
            }
            _ => None,
        }
    }

    fn place_near(&self, owner: isize) {
        self.place_near.set(owner);
        if self.loaded.get() {
            self.place();
        }
    }

    fn show(self: &Rc<Self>) {
        if self.disposed.get() {
            return;
        }
        if !self.loaded.get() {
            // OnLoad.
            self.loaded.set(true);
            self.place();
        }
        host::show(self.hwnd, SW_SHOW as i32);
    }

    /// Only with the window real, and its place before its size: set together, or before the handle, on a screen at
    /// another scale than the main one, the window was sized at the wrong scale and Windows' scale change resized it by
    /// the ratio again (review 2026-10-30; as ShellForm.RestorePlace).
    fn place(&self) {
        let owner = self.place_near.get();
        if owner == 0 || !host::is_window(owner) {
            return;
        }
        let area = screen_from_handle(owner).working_area;
        let now = host::window_rect(self.hwnd);
        let x = area.x + 0.max((area.width - now.width) / 2);
        let y = area.y + 0.max((area.height - now.height) / 2);
        // SAFETY: plain call; the place only, at the window's present size.
        unsafe { SetWindowPos(self.hwnd as HWND, std::ptr::null_mut(), x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE) };
        let dpi = host::dpi(self.hwnd);
        let (w, h) = (host::logical(1280, dpi).min(area.width), host::logical(860, dpi).min(area.height));
        host::set_bounds(self.hwnd, Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h));
    }

    /// The top above the page, at its own height; with its account menu open, taller by the menu and over the page, but
    /// cut to the bar and the menu, so the page shows beside it. Sibling windows: the top, in front, draws over the page.
    fn lay_out(&self) {
        // Laid out after they're gone.
        if self.disposed.get() || self.chrome.control().is_disposed() {
            return;
        }
        let area = host::client_size(self.hwnd);
        let bar = self.chrome.bar_pixels().min(area.height);
        let menu = self.chrome.menu_pixels();
        let height = match menu {
            Some(m) => bar.max(m.bottom()).min(area.height),
            None => bar,
        };
        let top = self.chrome.control();
        top.set_bounds(Rect::new(0, 0, area.width, height));
        match menu {
            Some(open) => top.set_region(Some(&[Rect::new(0, 0, area.width, bar), open])),
            None => top.set_region(None),
        }
        host::set_bounds(self.pages, Rect::new(0, bar, area.width, (area.height - bar).max(0)));
        top.bring_to_front();
    }

    /// The tabs fill the pages panel (Dock = Fill).
    fn fit_tabs(&self) {
        let size = host::client_size(self.pages);
        for (tab, _) in self.tabs.borrow().iter() {
            tab.control().set_bounds(Rect::new(0, 0, size.width, size.height));
        }
    }

    /// The handle's view of the tabs, after each change.
    fn mirror(&self) {
        let shared = self.shared.borrow().clone();
        if let Some(shared) = shared {
            let mut m = shared.mirror.lock().unwrap_or_else(|e| e.into_inner());
            m.tabs = self.tabs.borrow().iter().map(|(t, _)| t.handle()).collect();
            m.selected = self.selected.borrow().as_ref().map(|t| t.handle());
        }
    }

    fn find(&self, tab: &WebsiteTab) -> Option<Rc<TabCore>> {
        self.tabs.borrow().iter().find(|(t, _)| Arc::ptr_eq(&t.shared, &tab.0)).map(|(t, _)| t.clone())
    }

    fn is_selected(&self, tab: &Rc<TabCore>) -> bool {
        self.selected.borrow().as_ref().is_some_and(|s| Rc::ptr_eq(s, tab))
    }

    fn add_tab(self: &Rc<Self>, handle: WebsiteTab) {
        if self.disposed.get() || handle.is_closed() || self.find(&handle).is_some() {
            return;
        }
        let tab = TabCore::attach(&handle, self.pages);
        let weak_form = Rc::downgrade(self);
        let weak_tab = Rc::downgrade(&tab);
        *tab.title_changed.borrow_mut() = Some(Rc::new({
            let (f, t) = (weak_form.clone(), weak_tab.clone());
            move || {
                let (Some(me), Some(tab)) = (f.upgrade(), t.upgrade()) else { return };
                if me.is_selected(&tab) {
                    me.sync_title();
                }
                me.push();
            }
        }));
        *tab.state_changed.borrow_mut() = Some(Rc::new({
            let (f, t) = (weak_form.clone(), weak_tab.clone());
            move || {
                let (Some(me), Some(tab)) = (f.upgrade(), t.upgrade()) else { return };
                if me.is_selected(&tab) {
                    me.push();
                }
            }
        }));
        *tab.close_requested.borrow_mut() = Some(Rc::new({
            let (f, t) = (weak_form.clone(), weak_tab.clone());
            move || {
                let (Some(me), Some(tab)) = (f.upgrade(), t.upgrade()) else { return };
                let handle = tab.handle();
                host::spawn(async move { me.close_tab(&handle).await });
            }
        }));
        tab.control().on_keys(self.key_handler());
        tab.control().set_visible(false);
        let key = self.keys.get() + 1;
        self.keys.set(key);
        self.tabs.borrow_mut().push((tab.clone(), format!("t{key}")));
        self.fit_tabs();
        self.mirror();
        self.select_tab(&tab);
        tab.start();
    }

    fn select_tab_by_handle(self: &Rc<Self>, handle: &WebsiteTab) {
        if let Some(tab) = self.find(handle) {
            self.select_tab(&tab);
        }
    }

    fn select_tab(self: &Rc<Self>, tab: &Rc<TabCore>) {
        if !self.tabs.borrow().iter().any(|(t, _)| Rc::ptr_eq(t, tab)) {
            return;
        }
        *self.selected.borrow_mut() = Some(tab.clone());
        // The chosen page shown over the one before, which is hidden only a moment later: hidden first, the window's
        // empty ground showed between the two (the owner, 2026-11-18: "nhấp nháy" on every switch).
        tab.control().set_visible(true);
        tab.control().bring_to_front();
        let me = self.clone();
        host::spawn(async move {
            host::delay(std::time::Duration::from_millis(150)).await;
            let Some(now) = me.selected.borrow().clone() else { return };
            for (other, _) in me.tabs.borrow().iter() {
                if !Rc::ptr_eq(other, &now) {
                    other.control().set_visible(false);
                }
            }
        });
        self.mirror();
        self.sync_title();
        self.push();
        if self.contains_focus() {
            tab.focus_page();
        }
    }

    /// ContainsFocus: the keyboard is in this window or one of its children.
    fn contains_focus(&self) -> bool {
        // SAFETY: plain calls.
        unsafe {
            let focus = GetFocus();
            !focus.is_null() && (focus as isize == self.hwnd || IsChild(self.hwnd as HWND, focus) != 0)
        }
    }

    fn sync_title(&self) {
        let title = match self.selected.borrow().as_ref() {
            Some(tab) if tab.title() != "Roblox" => l::f("{0} — Roblox", &[&tab.title()]),
            _ => "Roblox".to_string(),
        };
        host::set_text(self.hwnd, &title);
    }

    /// What the top shows now, once per round of changes (a tab added and chosen is one).
    fn push(self: &Rc<Self>) {
        if self.push_soon.get() || self.disposed.get() {
            return;
        }
        self.push_soon.set(true);
        let me = self.clone();
        host::defer(move || {
            me.push_soon.set(false);
            if me.disposed.get() {
                return;
            }
            // PORT: Accounts is the app's, on the core's thread: asked there, and the top drawn when it answers.
            let accounts_fn = me.shared.borrow().as_ref().and_then(|s| s.mirror.lock().unwrap_or_else(|e| e.into_inner()).accounts);
            let me = me.clone();
            host::spawn(async move {
                let accounts = match accounts_fn {
                    Some(id) => host::ask_core::<AccountsFn, Vec<WebsiteAccount>>(id, |f, tx| {
                        let _ = tx.send(f());
                    })
                    .await
                    .unwrap_or_default(),
                    None => Vec::new(),
                };
                if me.disposed.get() {
                    return;
                }
                me.show_top(&accounts);
            });
        });
    }

    fn show_top(&self, accounts: &[WebsiteAccount]) {
        let tab = self.selected.borrow().clone();
        let dpi = host::dpi(self.hwnd);
        let height = host::client_size(self.hwnd).height;
        let (theme, custom_theme, language) = look();
        self.chrome.show(json!({
            "mode": "tabs",
            "accounts": accounts.iter().map(|a| json!({ "id": a.id.to_string(), "name": a.name, "avatar": a.avatar })).collect::<Vec<_>>(),
            "account": tab.as_ref().and_then(|t| t.account_id()).map(|id| id.to_string()),
            // How tall the account menu may be (CSS pixels): the window below the top's own height.
            "room": 0.max(((height - self.chrome.bar_pixels()) as f32 * 96f32 / dpi as f32) as i32),
            "tabs": self.tabs.borrow().iter().map(|(t, key)| json!({
                "id": key,
                "title": t.title(),
                "avatar": t.avatar(),
                "saved": t.account_id().is_some(),
                "selected": tab.as_ref().is_some_and(|s| Rc::ptr_eq(s, t)),
            })).collect::<Vec<_>>(),
            "back": tab.as_ref().is_some_and(|t| t.can_go_back.get()),
            "forward": tab.as_ref().is_some_and(|t| t.can_go_forward.get()),
            "host": tab.as_ref().and_then(|t| t.host.borrow().clone()),
            "message": tab.as_ref().map(|t| t.message.borrow().clone()).unwrap_or_default(),
            "tone": tab.as_ref().map(|t| t.tone.borrow().clone()).unwrap_or_default(),
            "retry": tab.as_ref().is_some_and(|t| t.show_retry.get()),
            "save": tab.as_ref().is_some_and(|t| t.can_save.get()),
            "theme": theme,
            "customTheme": custom_theme,
            "language": language,
        }));
    }

    fn new_blank_tab(&self) {
        let id = self.shared.borrow().as_ref().and_then(|s| s.mirror.lock().unwrap_or_else(|e| e.into_inner()).new_blank_tab);
        if let Some(id) = id {
            host::call_on_core::<NewBlankTabFn>(id, |f| f());
        }
    }

    /// Clicked on the top. Only this window's own buttons: the page can't send a tab anywhere.
    fn on_act(self: &Rc<Self>, what: &str, args: &Value) {
        let key = args.get("id").and_then(Value::as_str).unwrap_or("");
        let by_key = self.tabs.borrow().iter().find(|(_, k)| k == key).map(|(t, _)| t.clone());
        let selected = self.selected.borrow().clone();
        match what {
            "select" => {
                if let Some(tab) = by_key {
                    self.select_tab(&tab);
                    tab.focus_page();
                }
            }
            "close" => {
                if let Some(tab) = by_key {
                    let me = self.clone();
                    let handle = tab.handle();
                    host::spawn(async move { me.close_tab(&handle).await });
                }
            }
            // A tab dragged to another place in the strip (as a browser's): `to` its place among the tabs now.
            "move" => {
                let to = args.get("to").and_then(Value::as_u64).map(|t| t as usize);
                if let (Some(tab), Some(to)) = (by_key, to) {
                    let moved = {
                        let mut tabs = self.tabs.borrow_mut();
                        match tabs.iter().position(|(t, _)| Rc::ptr_eq(t, &tab)) {
                            Some(from) if from != to.min(tabs.len() - 1) => {
                                let item = tabs.remove(from);
                                let to = to.min(tabs.len());
                                tabs.insert(to, item);
                                true
                            }
                            _ => false,
                        }
                    };
                    if moved {
                        self.mirror();
                        self.push();
                    }
                }
            }
            "newTab" => self.new_blank_tab(),
            "back" => {
                if let Some(tab) = selected {
                    tab.go_back();
                    tab.focus_page();
                }
            }
            "forward" => {
                if let Some(tab) = selected {
                    tab.go_forward();
                    tab.focus_page();
                }
            }
            "reload" => {
                if let Some(tab) = selected {
                    tab.reload();
                    tab.focus_page();
                }
            }
            "retry" => {
                if let Some(tab) = selected {
                    tab.retry();
                }
            }
            "save" => {
                if let Some(tab) = selected {
                    tab.save();
                }
            }
            "account" => {
                let account = args.get("account").and_then(Value::as_str).and_then(|a| a.parse::<Guid>().ok());
                if let (Some(current), Some(account)) = (selected, account) {
                    let id = self.shared.borrow().as_ref().and_then(|s| s.mirror.lock().unwrap_or_else(|e| e.into_inner()).switch_account);
                    if let Some(id) = id {
                        let handle = current.handle();
                        host::call_on_core::<SwitchAccountFn>(id, move |f| f(handle, account));
                    }
                }
            }
            _ => {}
        }
    }

    /// Closes one tab (after a last read of a saved account's session); the window goes with its last tab.
    async fn close_tab(self: Rc<Self>, handle: &WebsiteTab) {
        let index = self.tabs.borrow().iter().position(|(t, _)| Arc::ptr_eq(&t.shared, &handle.0));
        let Some(index) = index else { return };
        let (tab, _) = self.tabs.borrow_mut().remove(index);
        self.mirror();
        if self.is_selected(&tab) {
            *self.selected.borrow_mut() = None;
            self.mirror();
            let next = {
                let tabs = self.tabs.borrow();
                if tabs.is_empty() { None } else { Some(tabs[index.min(tabs.len() - 1)].0.clone()) }
            };
            if let Some(next) = next {
                self.select_tab(&next);
            }
        }
        self.push();
        tab.control().set_visible(false);
        tab.clone().close().await;
        tab.dispose();
        if self.tabs.borrow().is_empty() && !self.closing_all.get() && !self.disposed.get() {
            self.done.set(true);
            host::close(self.hwnd);
        }
    }

    /// Closes every tab at once, without the last read of the sessions (the vault is locking, or the app quitting).
    fn close_now(&self) {
        self.closing_all.set(true);
        self.done.set(true);
        let tabs: Vec<_> = self.tabs.borrow_mut().drain(..).collect();
        for (tab, _) in tabs {
            tab.close_now();
        }
        self.mirror();
        host::close(self.hwnd);
    }

    /// OnFormClosing: each saved account's session is read once more before its tab goes, a moment at most.
    fn form_closing(self: &Rc<Self>) {
        if self.done.get() || self.tabs.borrow().is_empty() {
            self.destroy();
            return;
        }
        self.done.set(true);
        self.closing_all.set(true);
        host::show(self.hwnd, SW_HIDE as i32);
        let me = self.clone();
        host::spawn(async move {
            let closing: Vec<_> = me.tabs.borrow().iter().map(|(t, _)| t.clone().close()).collect();
            join_all(closing).await;
            me.tabs.borrow_mut().clear();
            me.mirror();
            if !me.disposed.get() {
                me.destroy();
            }
        });
    }

    fn destroy(&self) {
        if self.disposed.get() {
            return;
        }
        // SAFETY: plain call on this window, on its own thread.
        unsafe { DestroyWindow(self.hwnd as HWND) };
    }

    /// OnFormClosed: whatever tab is left goes at once; the app hears it on its thread.
    fn form_closed(&self) {
        let tabs: Vec<_> = self.tabs.borrow_mut().drain(..).collect();
        for (tab, _) in tabs {
            tab.close_now();
        }
        *self.selected.borrow_mut() = None;
        self.disposed.set(true);
        self.chrome.control().dispose();
        let shared = self.shared.borrow_mut().take();
        if let Some(shared) = shared {
            let (closed, kept) = {
                let mut m = shared.mirror.lock().unwrap_or_else(|e| e.into_inner());
                m.tabs.clear();
                m.selected = None;
                m.disposed = true;
                let closed: Vec<u64> = m.form_closed.drain(..).collect();
                let kept: Vec<u64> = [m.new_blank_tab.take(), m.accounts.take(), m.switch_account.take()].into_iter().flatten().collect();
                (closed, kept)
            };
            for id in &closed {
                host::call_on_core::<FormClosedFn>(*id, |f| f());
            }
            for id in closed.into_iter().chain(kept) {
                host::drop_on_core(id);
            }
        }
        let _ = FORM.try_with(|f| *f.borrow_mut() = None);
    }

    /// Ctrl+T a blank tab, Ctrl+W closes this one, Ctrl+Tab / Ctrl+Shift+Tab (or Ctrl+PgDn / PgUp) the next or previous.
    fn process_cmd_key(self: &Rc<Self>, keys: u32) -> bool {
        let (tab, next, prior) = (VK_TAB as u32, VK_NEXT as u32, VK_PRIOR as u32);
        if keys == KEYS_CONTROL | KEY_T {
            self.new_blank_tab();
            return true;
        }
        if keys == KEYS_CONTROL | KEY_W {
            let selected = self.selected.borrow().clone();
            if let Some(selected) = selected {
                let me = self.clone();
                let handle = selected.handle();
                host::spawn(async move { me.close_tab(&handle).await });
                return true;
            }
            return false;
        }
        if keys == KEYS_CONTROL | tab || keys == KEYS_CONTROL | next {
            self.step(1);
            return true;
        }
        if keys == KEYS_CONTROL | KEYS_SHIFT | tab || keys == KEYS_CONTROL | prior {
            self.step(-1);
            return true;
        }
        false
    }

    fn step(self: &Rc<Self>, by: i32) {
        let target = {
            let tabs = self.tabs.borrow();
            let selected = self.selected.borrow();
            let (true, Some(selected)) = (tabs.len() >= 2, selected.as_ref()) else { return };
            // FindIndex: -1 when the chosen tab isn't in the list.
            let i = tabs.iter().position(|(t, _)| Rc::ptr_eq(t, selected)).map(|i| i as i32).unwrap_or(-1);
            let n = tabs.len() as i32;
            tabs[((i + by + n) % n) as usize].0.clone()
        };
        self.select_tab(&target);
        let selected = self.selected.borrow().clone();
        if let Some(selected) = selected {
            selected.focus_page();
        }
    }
}
