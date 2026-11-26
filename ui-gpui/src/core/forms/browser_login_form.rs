// BrowserLoginForm.cs: sign-in on roblox.com in a throw-away WebView2 profile.
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::{Rc, Weak},
    sync::{Arc, Mutex},
    time::Duration,
};

use futures::{
    StreamExt,
    channel::{mpsc, oneshot},
    future::LocalBoxFuture,
};
use serde_json::{Value, json};
use webview2_com::{
    ContentLoadingEventHandler, Microsoft::Web::WebView2::Win32::*, NavigationCompletedEventHandler, NavigationStartingEventHandler,
    NewWindowRequestedEventHandler, SourceChangedEventHandler,
};
use windows::core::{HSTRING, Interface};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    UI::Input::KeyboardAndMouse::EnableWindow,
    UI::WindowsAndMessaging::{
        SW_SHOW, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos, WM_CLOSE, WM_DESTROY, WM_DPICHANGED, WM_ERASEBKGND, WM_SIZE, WS_CLIPCHILDREN,
        WS_OVERLAPPEDWINDOW,
    },
};

use super::{
    chrome_view::{
        ChromeView, look,
        host::{self, DialogResult, Invoker, Timer, WebHost},
    },
    website_tab,
};
use crate::core::{
    base::{Fail, Res, new_guid},
    l, log, roblox_api, themes, ui, web_view2_setup,
    win_kit::Rect,
};

const PROFILE_PREFIX: &str = "Bloxkeep-login-";
const LOGIN_URL: &str = "https://www.roblox.com/login";
/// roblox.com's home, signed out: its sign-up form.
const SIGN_UP_URL: &str = "https://www.roblox.com/";
/// A blocked connection hangs instead of failing, so don't wait for the browser to give up.
const SLOW_AFTER: Duration = Duration::from_secs(12);

/// The account's cookie on its way to the controller, and where its answer goes.
type SignedIn = (String, oneshot::Sender<Res<String>>);

/// Sign-in on roblox.com in a throw-away WebView2 profile: each new session cookie goes to `on_signed_in`, then a fresh
/// profile opens for the next account. The password typed here is never read by the app. Its top (the site, what to
/// do, Retry, Done) is the app's own page (ChromeView), apart from the roblox.com one.
///
/// Made and shown from the core's thread; the window itself runs on a thread of its own (as WinForms' UI thread).
pub struct BrowserLoginForm {
    /// Making new accounts on Roblox's own sign-up form (the person fills it in and solves Roblox's checks), not signing in.
    create: bool,
    on_signed_in: Rc<dyn Fn(String) -> LocalBoxFuture<'static, Res<String>>>,
    window: Arc<Mutex<WindowSlot>>,
    /// Never shown (the checks: the window and its browser are made as always, off the desktop).
    unseen: bool,
}

#[derive(Default)]
struct WindowSlot {
    invoker: Option<Invoker>,
    hwnd: isize,
    close_asked: bool,
}

impl BrowserLoginForm {
    /// `on_signed_in`: saves the account for a new session cookie and returns a short confirmation.
    /// `create`: opens Roblox's sign-up form: each account made there is saved the same way.
    pub fn new(on_signed_in: impl Fn(String) -> LocalBoxFuture<'static, Res<String>> + 'static, create: bool) -> BrowserLoginForm {
        BrowserLoginForm { create, on_signed_in: Rc::new(on_signed_in), window: Arc::new(Mutex::new(WindowSlot::default())), unseen: false }
    }

    /// The same window, never shown: for the checks.
    #[cfg(any(test, feature = "core"))]
    pub fn unseen(mut self) -> BrowserLoginForm {
        self.unseen = true;
        self
    }

    /// ShowDialog(owner): over the owner (disabled meanwhile) until closed. Each sign-in is handed to `on_signed_in`
    /// here, on the core's thread, while the window waits for its answer.
    pub async fn show_dialog(&self, owner: isize) -> DialogResult {
        let (tx, mut rx) = mpsc::unbounded::<SignedIn>();
        let create = self.create;
        let slot = self.window.clone();
        log::note(if create { "Sign-in window: opening Roblox's sign-up form." } else { "Sign-in window: opening." });
        let unseen = self.unseen;
        let started = host::start_thread("sign-in window", move || LoginWindow::open(owner, create, tx, slot, unseen));
        if !started {
            log::note("Sign-in window: its thread couldn't start.");
            return DialogResult::Cancel;
        }
        // Ends when the window is gone (its sender with it), also when its thread ended early (a failure there).
        let mut saved = 0;
        while let Some((cookie, reply)) = rx.next().await {
            let answer = (self.on_signed_in)(cookie).await;
            saved += answer.is_ok() as u32;
            let _ = reply.send(answer);
        }
        // The app is usable again however the window went: it was disabled while the window was up, and a window
        // whose thread failed never gave it back.
        if owner != 0 {
            // SAFETY: plain call; enabling an enabled window does nothing.
            unsafe { EnableWindow(owner as HWND, 1) };
        }
        // "Couldn't sign in" from an uninstaller is read against this: closed with none saved is the case to look into.
        log::note(&format!("Sign-in window: closed, {saved} account{} saved.", if saved == 1 { "" } else { "s" }));
        DialogResult::Cancel
    }

    /// Close(): from the controller (the screen locked, the vault closed).
    pub fn close(&self) {
        let mut slot = self.window.lock().unwrap_or_else(|e| e.into_inner());
        slot.close_asked = true;
        if let Some(invoker) = slot.invoker {
            let hwnd = slot.hwnd;
            invoker.run(move || host::close(hwnd));
        }
    }

    /// Removes profiles left behind if the app was closed before deletion finished.
    pub fn cleanup_leftover_profiles() {
        let temp = std::env::temp_dir();
        let Ok(entries) = std::fs::read_dir(&temp) else { return };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_dir && (name.starts_with(PROFILE_PREFIX) || name.starts_with("RobloxManager-login-")) {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }
}

/// The window, on its own thread.
struct LoginWindow {
    hwnd: isize,
    owner: isize,
    create: bool,
    theme: themes::Theme,
    to_core: RefCell<Option<mpsc::UnboundedSender<SignedIn>>>,
    poll: Rc<Timer>,
    slow: Rc<Timer>,
    chrome: Rc<ChromeView>,
    web: RefCell<Option<Rc<WebHost>>>,
    profile_dir: RefCell<Option<PathBuf>>,
    polling: Cell<bool>,
    closing: Cell<bool>,
    disposed: Cell<bool>,
    // What the bar says once the page is up: the instructions, or how the last sign-in went.
    ready_text: RefCell<String>,
    ready_tone: RefCell<String>,
    // What the top shows now.
    message: RefCell<String>,
    tone: RefCell<String>,
    host_name: RefCell<Option<String>>,
    retry: Cell<bool>,
}

impl LoginWindow {
    fn open(owner: isize, create: bool, to_core: mpsc::UnboundedSender<SignedIn>, slot: Arc<Mutex<WindowSlot>>, unseen: bool) {
        let theme = themes::current();
        let title = if create { l::t("Create a Roblox account") } else { l::t("Sign in to Roblox") };
        let hwnd = host::create_window(host::CLASS_FORM, &title, WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN, 0, owner, Rect::default());
        if hwnd == 0 {
            log::note("Sign-in window: Windows couldn't make it.");
            return;
        }
        host::set_icon(hwnd);
        // StartPosition = CenterParent, Size = 1000 × 800 at the window's scale.
        let dpi = host::dpi(if owner != 0 { owner } else { hwnd });
        let (w, h) = (host::logical(1000, dpi), host::logical(800, dpi));
        let around = if owner != 0 { host::window_rect(owner) } else { crate::core::win_kit::screen_from_handle(hwnd).working_area };
        host::set_bounds(hwnd, Rect::new(around.x + (around.width - w) / 2, around.y + (around.height - h) / 2, w, h));

        let ready_text = if create {
            l::t("Fill in Roblox's sign-up form. Each account you make is saved to Bloxkeep. Press Done when finished.")
        } else {
            l::t("Sign in to each account in turn. Press Done when finished.")
        };
        let me = Rc::new(LoginWindow {
            hwnd,
            owner,
            create,
            theme: theme.clone(),
            to_core: RefCell::new(Some(to_core)),
            poll: Timer::new(1000),
            slow: Timer::new(SLOW_AFTER.as_millis() as u32),
            chrome: ChromeView::new(hwnd, 52),
            web: RefCell::new(None),
            profile_dir: RefCell::new(None),
            polling: Cell::new(false),
            closing: Cell::new(false),
            disposed: Cell::new(false),
            ready_text: RefCell::new(ready_text),
            ready_tone: RefCell::new(String::new()),
            message: RefCell::new(String::new()),
            tone: RefCell::new(String::new()),
            host_name: RefCell::new(None),
            retry: Cell::new(false),
        });
        {
            let mut s = slot.lock().unwrap_or_else(|e| e.into_inner());
            s.invoker = Some(Invoker::current());
            s.hwnd = hwnd;
            if s.close_asked {
                host::close(hwnd);
            }
        }

        let weak = Rc::downgrade(&me);
        me.chrome.on_act(Rc::new({
            let weak = weak.clone();
            move |what: &str, _args: &Value| {
                let Some(me) = weak.upgrade() else { return };
                if what == "done" {
                    host::close(me.hwnd);
                } else if what == "retry" {
                    me.retry_open();
                }
            }
        }));
        me.chrome.set_docked(Rc::new({
            let weak = weak.clone();
            move || {
                if let Some(me) = weak.upgrade() {
                    me.lay_out();
                }
            }
        }));
        me.poll.on_tick({
            let weak = weak.clone();
            move || {
                if let Some(me) = weak.upgrade() {
                    host::spawn(async move { me.poll_cookie().await });
                }
            }
        });
        me.slow.on_tick({
            let weak = weak.clone();
            move || {
                if let Some(me) = weak.upgrade() {
                    me.slow.stop();
                    me.show_unreachable(&format!("no answer in {} s", SLOW_AFTER.as_secs()));
                }
            }
        });
        // The window keeps its own state for as long as Windows keeps the window (the handler goes at WM_NCDESTROY).
        // Held only by the tasks that start it, the state went as soon as the browser was up: the controller took the
        // window for closed, and an empty frame stayed on screen that nothing answered (error.log said "closed" in the
        // second it opened).
        host::set_handler(hwnd, {
            let me = me.clone();
            move |h: HWND, msg: u32, w: WPARAM, l: LPARAM| me.on_message(h, msg, w, l)
        });

        // OnHandleCreated: the themed title bar, the top's page.
        ui::use_themed_title_bar(hwnd, &me.theme);
        {
            let me = me.clone();
            host::spawn(async move { me.start_chrome().await });
        }
        // Load: the sign-in page.
        {
            let me = me.clone();
            host::spawn(async move { me.open_fresh_browser().await });
        }
        // ShowDialog: the owner can't be used meanwhile.
        if owner != 0 {
            // SAFETY: plain call; the owner is enabled again when this window closes.
            unsafe { EnableWindow(owner as HWND, 0) };
        }
        me.lay_out();
        if unseen {
            return;
        }
        host::show(hwnd, SW_SHOW);
        log::note(&format!("Sign-in window: shown, {}.", crate::core::window_report::describe(hwnd)));
    }

    fn on_message(self: &Rc<Self>, h: HWND, msg: u32, w: WPARAM, l: LPARAM) -> Option<isize> {
        match msg {
            WM_SIZE => {
                self.lay_out();
                Some(0)
            }
            WM_ERASEBKGND => Some(host::fill_background(h, w, self.theme.background)),
            WM_DPICHANGED => {
                // SAFETY: lparam is the RECT Windows suggests for the new scale.
                let r = unsafe { &*(l as *const windows_sys::Win32::Foundation::RECT) };
                // SAFETY: plain call.
                unsafe { SetWindowPos(h, std::ptr::null_mut(), r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOZORDER | SWP_NOACTIVATE) };
                self.chrome.dpi_changed();
                Some(0)
            }
            WM_CLOSE => {
                if self.owner != 0 {
                    // SAFETY: plain call; enabled before this window goes so the owner gets the activation back.
                    unsafe { EnableWindow(self.owner as HWND, 1) };
                }
                // SAFETY: plain call on this window.
                unsafe { windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(h) };
                Some(0)
            }
            WM_DESTROY => {
                self.on_form_closed();
                host::quit();
                Some(0)
            }
            _ => None,
        }
    }

    /// The bar docked on top at its height; the browser fills the rest.
    fn lay_out(&self) {
        if self.disposed.get() {
            return;
        }
        let area = host::client_size(self.hwnd);
        let bar = self.chrome.bar_pixels().min(area.height);
        self.chrome.control().set_bounds(Rect::new(0, 0, area.width, bar));
        if let Some(web) = self.web.borrow().as_ref() {
            web.set_bounds(Rect::new(0, bar, area.width, (area.height - bar).max(0)));
        }
    }

    async fn start_chrome(self: Rc<Self>) {
        if self.chrome.start().await {
            self.push();
        }
    }

    /// What the top shows now.
    fn push(&self) {
        if self.disposed.get() {
            return;
        }
        let (theme, custom_theme, language) = look();
        self.chrome.show(json!({
            "mode": "login",
            "tabs": [],
            "back": false,
            "forward": false,
            "host": *self.host_name.borrow(),
            "message": *self.message.borrow(),
            "tone": *self.tone.borrow(),
            "retry": self.retry.get(),
            "save": false,
            "theme": theme,
            "customTheme": custom_theme,
            "language": language,
        }));
    }

    /// A new WebView2 with its own empty profile, so the previous account's session can't carry over.
    async fn open_fresh_browser(self: Rc<Self>) {
        let dir = std::env::temp_dir().join(format!("{PROFILE_PREFIX}{}", new_guid().simple()));
        *self.profile_dir.borrow_mut() = Some(dir.clone());
        let web = WebHost::new(self.hwnd, self.theme.background);
        // fill the space left by the bar
        *self.web.borrow_mut() = Some(web.clone());
        self.lay_out();
        let result: host::WinResult<()> = async {
            let env = host::environment(&dir.to_string_lossy(), web_view2_setup::environment_options().map(Into::into)).await?;
            let core = web.ensure(&env, None).await?;
            if self.closing.get() || web.is_disposed() {
                return Ok(());
            }
            self.configure(&core)?;
            self.open_login_page(&core);
            self.poll.start();
            log::note("Sign-in window: its browser is up, Roblox's page asked for.");
            Ok(())
        }
        .await;
        if let Err(e) = result {
            log::note(&format!("Sign-in window: its browser couldn't start ({}).", e.message()));
            if host::runtime_missing(&e) {
                ui::error(Some(self.hwnd), &l::t("Microsoft Edge WebView2 isn't installed. Install it, or add the account with a cookie."));
                host::close(self.hwnd);
            } else if !self.closing.get() {
                ui::error(Some(self.hwnd), &l::f("Couldn't open the sign-in browser: {0}", &[&e.message()]));
                host::close(self.hwnd);
            }
        }
    }

    fn configure(self: &Rc<Self>, core: &ICoreWebView2) -> host::WinResult<()> {
        let weak: Weak<LoginWindow> = Rc::downgrade(self);
        let mut token = 0i64;
        // SAFETY: COM calls on this window thread; every handler runs here too.
        unsafe {
            let settings = core.Settings()?;
            let settings4 = settings.cast::<ICoreWebView2Settings4>()?;
            settings4.SetIsPasswordAutosaveEnabled(false)?;
            settings4.SetIsGeneralAutofillEnabled(false)?;
            // DevTools would show the session cookie, which is how cookie thieves talk people into sharing it.
            settings.SetAreDevToolsEnabled(false)?;
            // Only Roblox's own pages here: this window shows no address, so another site would pass for the sign-in
            // page. A link elsewhere opens in the person's own browser, and only when they clicked it.
            core.add_NavigationStarting(
                &NavigationStartingEventHandler::create(Box::new(move |_, args| {
                    let Some(args) = args else { return Ok(()) };
                    let uri = host::text(|p| args.Uri(p));
                    if website_tab::is_roblox(&uri) {
                        return Ok(());
                    }
                    args.SetCancel(true)?;
                    if host::flag(|b| args.IsUserInitiated(b)) {
                        website_tab::open_outside(&uri);
                    }
                    Ok(())
                })),
                &mut token,
            )?;
            let c = core.clone();
            core.add_NewWindowRequested(
                &NewWindowRequestedEventHandler::create(Box::new(move |_, args| {
                    let Some(args) = args else { return Ok(()) };
                    args.SetHandled(true)?;
                    let uri = host::text(|p| args.Uri(p));
                    if website_tab::is_roblox(&uri) {
                        c.Navigate(&HSTRING::from(uri))?;
                    } else if host::flag(|b| args.IsUserInitiated(b)) {
                        website_tab::open_outside(&uri);
                    }
                    Ok(())
                })),
                &mut token,
            )?;
            // The site's name only (never the address), so it's plain this is roblox.com.
            let w = weak.clone();
            let c = core.clone();
            core.add_SourceChanged(
                &SourceChangedEventHandler::create(Box::new(move |_, _| {
                    let Some(me) = w.upgrade() else { return Ok(()) };
                    let source = host::text(|p| c.Source(p));
                    let host_name = if website_tab::is_roblox(&source) { website_tab::uri_host(&source) } else { None };
                    if *me.host_name.borrow() == host_name || me.closing.get() {
                        return Ok(());
                    }
                    *me.host_name.borrow_mut() = host_name;
                    me.push();
                    Ok(())
                })),
                &mut token,
            )?;
            let w = weak.clone();
            core.add_ContentLoading(
                &ContentLoadingEventHandler::create(Box::new(move |_, args| {
                    let (Some(me), Some(args)) = (w.upgrade(), args) else { return Ok(()) };
                    if host::flag(|b| args.IsErrorPage(b)) || me.closing.get() {
                        return Ok(());
                    }
                    me.slow.stop();
                    me.retry.set(false);
                    let (text, tone) = (me.ready_text.borrow().clone(), me.ready_tone.borrow().clone());
                    me.say(&text, &tone);
                    Ok(())
                })),
                &mut token,
            )?;
            let w = weak.clone();
            core.add_NavigationCompleted(
                &NavigationCompletedEventHandler::create(Box::new(move |_, args| {
                    let (Some(me), Some(args)) = (w.upgrade(), args) else { return Ok(()) };
                    let mut status = COREWEBVIEW2_WEB_ERROR_STATUS::default();
                    let _ = args.WebErrorStatus(&mut status);
                    // Canceled = superseded by another navigation (redirects), not a network problem.
                    if host::flag(|b| args.IsSuccess(b)) || status == COREWEBVIEW2_WEB_ERROR_STATUS_OPERATION_CANCELED || me.closing.get() {
                        return Ok(());
                    }
                    me.slow.stop();
                    me.show_unreachable(&format!("WebView2 error {}", status.0));
                    Ok(())
                })),
                &mut token,
            )?;
        }
        Ok(())
    }

    fn open_login_page(&self, core: &ICoreWebView2) {
        self.retry.set(false);
        let text = if self.create { l::t("Opening Roblox's sign-up page…") } else { l::t("Opening the Roblox sign-in page…") };
        self.say(&text, "");
        self.slow.stop();
        self.slow.start();
        // SAFETY: plain COM call on this window thread.
        unsafe {
            let _ = core.Navigate(&HSTRING::from(if self.create { SIGN_UP_URL } else { LOGIN_URL }));
        }
    }

    fn retry_open(&self) {
        let core = self.web.borrow().as_ref().and_then(|w| w.core());
        if let Some(core) = core {
            self.open_login_page(&core);
        }
    }

    fn show_unreachable(&self, why: &str) {
        if self.closing.get() {
            return;
        }
        log::note(&format!("Sign-in window: Roblox's page didn't load ({why}); Retry shown."));
        self.retry.set(true);
        self.say(&l::t("Can't reach roblox.com. Check your internet connection, then press Retry."), "bad");
    }

    fn say(&self, text: &str, tone: &str) {
        if tone == "bad" {
            crate::core::app::App::note_shown("Sign-in window", text);
        }
        *self.message.borrow_mut() = text.to_string();
        *self.tone.borrow_mut() = tone.to_string();
        self.push();
    }

    async fn poll_cookie(self: Rc<Self>) {
        if self.polling.get() {
            return;
        }
        let core = self.web.borrow().as_ref().and_then(|w| w.core());
        let Some(core) = core else { return };
        self.polling.set(true);
        match host::cookies(&core, "https://www.roblox.com").await {
            Ok(cookies) => {
                let session = cookies.into_iter().find(|(name, value)| name == ".ROBLOSECURITY" && !value.is_empty());
                if let Some((_, value)) = session {
                    self.poll.stop();
                    self.save_and_continue(value).await;
                }
            }
            Err(_) => {
                // Page is navigating or the window is closing; the next tick tries again.
            }
        }
        self.polling.set(false);
    }

    async fn save_and_continue(self: &Rc<Self>, cookie: String) {
        let (reply, answer) = oneshot::channel();
        let sent = self.to_core.borrow().as_ref().map(|tx| tx.unbounded_send((cookie, reply)).is_ok()).unwrap_or(false);
        let result = if sent { answer.await.ok() } else { None };
        let (message, ok) = match result {
            Some(Ok(message)) => (message, true),
            Some(Err(ex)) if matches!(ex, Fail::User(_)) || roblox_api::is_expected_error(&ex) => {
                log::note(&format!("Sign-in window: the account wasn't saved: {}", ex.message()));
                (ex.message().to_string(), false)
            }
            Some(Err(ex)) => {
                // PORT: the C# let any other exception escape the timer's async handler (the app's crash handler took
                // it); here it is logged and shown as this sign-in's failure.
                log::write(&ex);
                (ex.message().to_string(), false)
            }
            // The controller is gone (the app is quitting): nothing was saved.
            None => return,
        };
        if self.closing.get() || self.disposed.get() {
            return;
        }
        *self.ready_tone.borrow_mut() = if ok { "ok" } else { "bad" }.to_string();
        *self.ready_text.borrow_mut() = if ok {
            format!(
                "{message}  {}",
                if self.create { l::t("Make the next account, or press Done.") } else { l::t("Sign in to the next account, or press Done.") }
            )
        } else {
            message
        };
        let (text, tone) = (self.ready_text.borrow().clone(), self.ready_tone.borrow().clone());
        self.say(&text, &tone);
        self.release_browser();
        self.clone().open_fresh_browser().await;
    }

    fn release_browser(&self) {
        self.poll.stop();
        self.slow.stop();
        if let Some(web) = self.web.borrow_mut().take() {
            web.dispose();
        }
        if let Some(dir) = self.profile_dir.borrow_mut().take() {
            crate::threads::run(move || delete_profile(&dir));
        }
    }

    fn on_form_closed(&self) {
        self.closing.set(true);
        self.release_browser();
        self.poll.dispose();
        self.slow.dispose();
        self.chrome.control().dispose();
        self.disposed.set(true);
        // The controller's wait ends with this.
        self.to_core.borrow_mut().take();
    }
}

/// The browser process exits a moment after the control is disposed, so deletion is retried.
fn delete_profile(dir: &std::path::Path) {
    for _ in 0..20 {
        if !dir.exists() {
            return;
        }
        match std::fs::remove_dir_all(dir) {
            Ok(()) => return,
            Err(_) => std::thread::sleep(Duration::from_millis(500)),
        }
    }
}
