// WebsiteTab.cs: one tab of Bloxkeep's browser (WebsiteForm).
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

use futures::{
    FutureExt,
    channel::oneshot,
    future::{LocalBoxFuture, Shared, select},
};
use webview2_com::{
    ClearBrowsingDataCompletedHandler, ContentLoadingEventHandler, HistoryChangedEventHandler, Microsoft::Web::WebView2::Win32::*, NavigationCompletedEventHandler,
    NavigationStartingEventHandler, NewWindowRequestedEventHandler, SourceChangedEventHandler, WebResourceRequestedEventHandler,
};
use windows::{
    Win32::Foundation::E_ABORT,
    core::{HSTRING, Interface},
};
use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};

use super::{
    chrome_view::host::{self, Invoker, Timer, WebHost},
    dialog::{self, Choice},
};
use crate::core::{
    base::{Cancel, Fail, Guid, Res, Stamp, new_guid},
    l, log, prefs, rt,
    roblox_api::{self, RobloxUser, realm},
    themes, ui, web_view2_setup,
    win_kit::wide,
};

const FOLDER_PREFIX: &str = "Bloxkeep-web-";
const HOME_URL: &str = "https://www.roblox.com/home";
const LOGIN_URL: &str = "https://www.roblox.com/login";
const COOKIE_NAME: &str = ".ROBLOSECURITY";
/// A blocked connection hangs instead of failing, so don't wait for the browser to give up.
const SLOW_AFTER: Duration = Duration::from_secs(12);
// A saved account's tab reads back a cookie Roblox renewed (e.g. after "Log out of all other sessions"); a blank tab's
// learns who signed in.
const SAVED_SYNC_MS: u32 = 15000;
const BLANK_SYNC_MS: u32 = 2000;
const FIRST_WAIT: Duration = Duration::from_secs(30);
const LONGEST_WAIT: Duration = Duration::from_secs(60);
/// DateTime.MaxValue / default(DateTime) for `ask_again_at` (ms since 1970, UTC).
const AT_MAX: i64 = i64::MAX;
const AT_MIN: i64 = i64::MIN;

/// Gets a renewed cookie of a saved account's site to check and save; false = couldn't tell yet (no internet), ask again
/// at the next read. On the core's thread.
pub type SessionChanged = Box<dyn Fn(Guid, String) -> LocalBoxFuture<'static, bool>>;
/// Save to Bloxkeep: keeps the account signed in on a blank tab and says how that went (its id, name, a message).
pub type SaveAccount = Box<dyn Fn(String, RobloxUser) -> Res<(Guid, String, String)>>;
type ClosedFn = Box<dyn Fn()>;
/// Who a session cookie is (replaced by the checks); off the window thread.
pub type Identify = Arc<dyn Fn(String) -> Res<Option<RobloxUser>> + Send + Sync>;

thread_local! {
    // The browser all tabs share, and its folder; a new one (new folder) once the last tab has closed.
    // PORT: static in the C# (one UI thread); here per window thread: every tab of a website window lives on its thread,
    // and a website window closes with its last tab, so a new window starts a new browser as the C# did then.
    static ENV: RefCell<Option<Shared<LocalBoxFuture<'static, Result<ICoreWebView2Environment, (i32, String)>>>>> = const { RefCell::new(None) };
    static ENV_DIR: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
    static TABS_OPEN: Cell<i32> = const { Cell::new(0) };
    /// The tabs on this window thread, by their number.
    static TABS: RefCell<HashMap<u64, Rc<TabCore>>> = RefCell::new(HashMap::new());
}

static NEXT_TAB: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// One tab of Bloxkeep's browser (WebsiteForm): www.roblox.com signed in as a saved account, or blank for signing in on
/// the site (Save to Bloxkeep keeps that account). Each tab is its own InPrivate profile: the session cookie lives only
/// there, in memory, never on disk or in the person's Chrome or Edge. All tabs share one browser (a %TEMP% folder
/// deleted when the last tab closes). A saved account's sign-out on the site would end the saved session too, so it is
/// stopped and explained.
///
/// This is the tab as the controller holds it (a handle that crosses threads); the tab itself lives on its website
/// window's thread once added (`WebsiteForm::add_tab`).
#[derive(Clone)]
pub struct WebsiteTab(pub(super) Arc<TabShared>);

impl PartialEq for WebsiteTab {
    fn eq(&self, other: &WebsiteTab) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for WebsiteTab {}

pub(super) struct TabShared {
    pub(super) id: u64,
    mirror: Mutex<Mirror>,
    session_fn: u64,
    save_fn: u64,
    closed_fns: Mutex<Vec<u64>>,
}

/// What the controller reads of the tab, and what it was given before the tab was added to a window.
struct Mirror {
    attached: Option<Invoker>,
    account_id: Option<Guid>,
    title: String,
    avatar: Option<String>,
    torn_down: bool,
    cookie: String,
    start_url: Option<String>,
    identify: Identify,
    ask_close_on_logout: fn(isize) -> bool,
}

impl WebsiteTab {
    /// `account_id`: the saved account to open signed in; None for a blank tab at the sign-in page.
    /// `on_session_changed`: gets a renewed cookie of a saved account's site to check and save; false = couldn't tell
    /// yet (no internet), ask again at the next read.
    /// `save`: Save to Bloxkeep: keeps the account signed in on a blank tab and says how that went.
    /// From the core's thread (the callbacks stay there).
    pub fn new(
        account_id: Option<Guid>,
        name: Option<&str>,
        cookie: &str,
        on_session_changed: impl Fn(Guid, String) -> LocalBoxFuture<'static, bool> + 'static,
        save: impl Fn(String, RobloxUser) -> Res<(Guid, String, String)> + 'static,
    ) -> WebsiteTab {
        let session_fn = host::keep_on_core::<SessionChanged>(Box::new(on_session_changed));
        let save_fn = host::keep_on_core::<SaveAccount>(Box::new(save));
        let identify: Identify = Arc::new(|cookie: String| roblox_api::get_authenticated_user(&cookie, realm::GLOBAL, &Cancel::new()));
        WebsiteTab(Arc::new(TabShared {
            id: NEXT_TAB.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            mirror: Mutex::new(Mirror {
                attached: None,
                account_id,
                // The title: the account's name, else Roblox (SetName before any sign-in on the site).
                title: name.map(str::to_string).unwrap_or_else(|| "Roblox".into()),
                avatar: None,
                torn_down: false,
                cookie: cookie.to_string(),
                start_url: None,
                identify,
                ask_close_on_logout,
            }),
            session_fn,
            save_fn,
            closed_fns: Mutex::new(Vec::new()),
        }))
    }

    fn mirror(&self) -> std::sync::MutexGuard<'_, Mirror> {
        self.0.mirror.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Runs `f` with the tab on its window's thread; false when it isn't in a window (yet, or any more).
    fn on_tab(&self, f: impl FnOnce(&Rc<TabCore>) + Send + 'static) -> bool {
        let invoker = self.mirror().attached;
        let Some(invoker) = invoker else { return false };
        let id = self.0.id;
        invoker.run(move || {
            if let Some(tab) = tab_core(id) {
                f(&tab);
            }
        })
    }

    /// Closed += f: the tab is gone (from the core's thread; `f` runs there).
    pub fn on_closed(&self, f: impl Fn() + 'static) {
        let id = host::keep_on_core::<ClosedFn>(Box::new(f));
        self.0.closed_fns.lock().unwrap_or_else(|e| e.into_inner()).push(id);
    }

    /// The saved account this tab is signed in as; None for a blank tab not saved (yet).
    pub fn account_id(&self) -> Option<Guid> {
        self.mirror().account_id
    }

    pub fn is_closed(&self) -> bool {
        self.mirror().torn_down
    }

    pub fn set_avatar(&self, value: Option<String>) {
        let v = value.clone();
        if !self.on_tab(move |t| t.set_avatar(v)) {
            self.mirror().avatar = value;
        }
    }

    /// Where a saved account's tab opens instead of its home page (Appeal: Roblox's Violations & Appeals).
    pub fn set_start_url(&self, url: Option<String>) {
        let u = url.clone();
        if !self.on_tab(move |t| *t.start_url.borrow_mut() = u) {
            self.mirror().start_url = url;
        }
    }

    /// The title: the account's name (follows Hide account names), whoever signed in on a blank tab, or Roblox.
    pub fn set_name(&self, name: Option<&str>) {
        let n = name.map(str::to_string);
        if !self.on_tab(move |t| t.set_name(n)) {
            self.mirror().title = name.map(str::to_string).unwrap_or_else(|| "Roblox".into());
        }
    }

    pub fn focus_page(&self) {
        self.on_tab(|t| t.focus_page());
    }

    /// An open tab goes to `url`, a page on roblox.com.
    pub fn go(&self, url: &str) {
        let u = url.to_string();
        if !self.on_tab(move |t| t.go(&u)) {
            self.mirror().start_url = Some(url.to_string());
        }
    }

    /// The account picker on the window's top: this tab signs in as another saved account, on the page it shows (its
    /// home when that isn't Roblox's). The account it was is read once more first, as when a tab closes, then its
    /// sign-in goes from the tab's profile: cookies and the site's storage, so nothing of it is left for the next one.
    pub fn switch_to(&self, id: Guid, name: &str, cookie: &str, avatar: Option<String>) {
        let (n, c, a) = (name.to_string(), cookie.to_string(), avatar.clone());
        if !self.on_tab(move |t| host::spawn(t.clone().switch_to(id, n, c, a))) {
            // Not started yet: it signs in as this account when it is (OpenAsync).
            let mut m = self.mirror();
            if m.torn_down {
                return;
            }
            m.account_id = Some(id);
            m.cookie = cookie.to_string();
            m.start_url = None;
            m.avatar = avatar;
            m.title = name.to_string();
        }
    }

    /// Closes without the last read of the session (the vault is locking, or the app quitting).
    pub fn close_now(&self) {
        self.on_tab(|t| t.close_now());
    }

    /// Removes folders left behind if the app was closed before deletion finished.
    pub fn cleanup_leftovers() {
        let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else { return };
        for entry in entries.flatten() {
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_dir && entry.file_name().to_string_lossy().starts_with(FOLDER_PREFIX) {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }
}

/// The tab of that number on this window thread.
pub(super) fn tab_core(id: u64) -> Option<Rc<TabCore>> {
    TABS.try_with(|t| t.borrow().get(&id).cloned()).ok().flatten()
}

/// The tab itself, on its website window's thread (the C# UserControl).
pub(super) struct TabCore {
    pub(super) shared: Arc<TabShared>,
    web: Rc<WebHost>,
    slow: Rc<Timer>,
    sync: Rc<Timer>,
    cookie: RefCell<String>,
    /// Who is signed in on a blank tab's site, until saved; None when nobody (or not known yet).
    user: RefCell<Option<RobloxUser>>,
    identify: Identify,
    closing: Cell<bool>,
    synced: Cell<bool>,
    syncing: Cell<bool>,
    asking_logout: Cell<bool>,
    has_browser: Cell<bool>,
    torn_down: Cell<bool>,
    saving: Cell<bool>,
    /// Goes up at each switch to another account (`switch_to`): a read of the session begun before it, or an older switch
    /// still on its way, is left.
    generation: Cell<i32>,
    /// A session Roblox couldn't be asked about (no answer, too many questions, a notice on the account): asked again
    /// after a while, not at every read (2026-10-30 review: a blank tab asked every 2 s, ~30 times a minute, while Roblox
    /// answered "User is moderated", and harder still while it said "too many"). A notice to accept waits for the next
    /// page the tab shows (accepting it loads one).
    failed_session: RefCell<Option<String>>,
    ask_again_at: Cell<i64>,
    failures: Cell<i32>,
    ask_close_on_logout: Cell<fn(isize) -> bool>,

    account_id: Cell<Option<Guid>>,
    title: RefCell<String>,
    avatar: RefCell<Option<String>>,
    // What the window's top (WebsiteForm's ChromeView) shows for this tab: words and switches only, never the page.
    pub(super) can_go_back: Cell<bool>,
    pub(super) can_go_forward: Cell<bool>,
    /// The page's host name (www.roblox.com), shown so it's plain which site this is; None before one loads.
    pub(super) host: RefCell<Option<String>>,
    /// The status line, and its colour: "", "ok" or "bad".
    pub(super) message: RefCell<String>,
    pub(super) tone: RefCell<String>,
    pub(super) show_retry: Cell<bool>,
    /// Save to Bloxkeep: a blank tab someone signed in on.
    pub(super) can_save: Cell<bool>,
    /// Where a saved account's tab opens instead of its home page (Appeal: Roblox's Violations & Appeals).
    start_url: RefCell<Option<String>>,

    /// The title changed (signed in on a blank tab, saved, renamed, Hide account names).
    pub(super) title_changed: RefCell<Option<Rc<dyn Fn()>>>,
    /// Something the window's top shows changed (back/forward, the site, the status line, Retry, Save).
    pub(super) state_changed: RefCell<Option<Rc<dyn Fn()>>>,
    /// The tab wants to go: its browser couldn't start, or the person chose Close tab after a stopped sign-out.
    pub(super) close_requested: RefCell<Option<Rc<dyn Fn()>>>,
}

fn werr(e: windows::core::Error) -> (i32, String) {
    (e.code().0, e.message().to_string())
}

fn now_ms() -> i64 {
    Stamp::utc_now().unix_ms()
}

impl TabCore {
    /// The tab joins a window (on that window's thread): its control in `parent`, hidden until chosen.
    pub(super) fn attach(tab: &WebsiteTab, parent: isize) -> Rc<TabCore> {
        let shared = tab.0.clone();
        let (account_id, title, avatar, cookie, start_url, identify, ask) = {
            let mut m = shared.mirror.lock().unwrap_or_else(|e| e.into_inner());
            m.attached = Some(Invoker::current());
            (m.account_id, m.title.clone(), m.avatar.clone(), m.cookie.clone(), m.start_url.clone(), m.identify.clone(), m.ask_close_on_logout)
        };
        let theme = themes::current();
        let web = WebHost::new(parent, theme.background);
        web.set_visible(false);
        let me = Rc::new(TabCore {
            shared: shared.clone(),
            web,
            slow: Timer::new(SLOW_AFTER.as_millis() as u32),
            sync: Timer::new(if account_id.is_none() { BLANK_SYNC_MS } else { SAVED_SYNC_MS }),
            cookie: RefCell::new(cookie),
            user: RefCell::new(None),
            identify,
            closing: Cell::new(false),
            synced: Cell::new(false),
            syncing: Cell::new(false),
            asking_logout: Cell::new(false),
            has_browser: Cell::new(false),
            torn_down: Cell::new(false),
            saving: Cell::new(false),
            generation: Cell::new(0),
            failed_session: RefCell::new(None),
            ask_again_at: Cell::new(AT_MIN),
            failures: Cell::new(0),
            ask_close_on_logout: Cell::new(ask),
            account_id: Cell::new(account_id),
            title: RefCell::new(title),
            avatar: RefCell::new(avatar),
            can_go_back: Cell::new(false),
            can_go_forward: Cell::new(false),
            host: RefCell::new(None),
            message: RefCell::new(String::new()),
            tone: RefCell::new(String::new()),
            show_retry: Cell::new(false),
            can_save: Cell::new(false),
            start_url: RefCell::new(start_url),
            title_changed: RefCell::new(None),
            state_changed: RefCell::new(None),
            close_requested: RefCell::new(None),
        });
        let weak = Rc::downgrade(&me);
        me.slow.on_tick({
            let weak = weak.clone();
            move || {
                if let Some(me) = weak.upgrade() {
                    me.slow.stop();
                    me.show_unreachable();
                }
            }
        });
        me.sync.on_tick(move || {
            if let Some(me) = weak.upgrade() {
                host::spawn(me.sync_session());
            }
        });
        TABS.with(|t| t.borrow_mut().insert(shared.id, me.clone()));
        me
    }

    pub(super) fn handle(&self) -> WebsiteTab {
        WebsiteTab(self.shared.clone())
    }

    pub(super) fn control(&self) -> &Rc<WebHost> {
        &self.web
    }

    pub(super) fn account_id(&self) -> Option<Guid> {
        self.account_id.get()
    }

    pub(super) fn title(&self) -> String {
        self.title.borrow().clone()
    }

    pub(super) fn avatar(&self) -> Option<String> {
        self.avatar.borrow().clone()
    }

    fn set_account(&self, id: Option<Guid>) {
        self.account_id.set(id);
        self.shared.mirror.lock().unwrap_or_else(|e| e.into_inner()).account_id = id;
    }

    fn fire(slot: &RefCell<Option<Rc<dyn Fn()>>>) {
        let f = slot.borrow().clone();
        if let Some(f) = f {
            f();
        }
    }

    fn changed(&self) {
        TabCore::fire(&self.state_changed);
    }

    fn set_avatar(&self, value: Option<String>) {
        if value == *self.avatar.borrow() {
            return;
        }
        *self.avatar.borrow_mut() = value.clone();
        self.shared.mirror.lock().unwrap_or_else(|e| e.into_inner()).avatar = value;
        TabCore::fire(&self.title_changed);
    }

    /// Starts the browser even while hidden: a tab hidden at once (another opened right after) would otherwise never get
    /// its Load.
    pub(super) fn start(self: &Rc<Self>) {
        if self.has_browser.get() || self.torn_down.get() {
            return;
        }
        host::spawn(self.clone().open());
    }

    /// The title: the account's name (follows Hide account names), whoever signed in on a blank tab, or Roblox.
    pub(super) fn set_name(&self, name: Option<String>) {
        let name = name.or_else(|| {
            let hide = prefs::lock().hide_names;
            self.user.borrow().as_ref().filter(|_| !hide).map(|u| u.name.clone())
        });
        let title = name.unwrap_or_else(|| "Roblox".into());
        if title == *self.title.borrow() {
            return;
        }
        *self.title.borrow_mut() = title.clone();
        self.shared.mirror.lock().unwrap_or_else(|e| e.into_inner()).title = title;
        TabCore::fire(&self.title_changed);
    }

    pub(super) fn focus_page(&self) {
        if !self.web.is_disposed() {
            self.web.focus();
        }
    }

    // The window's top asks for these (WebsiteForm): only to the page this tab already shows.

    pub(super) fn go_back(&self) {
        if let Some(core) = self.web.core() {
            // SAFETY: plain COM call on this window thread.
            unsafe {
                let _ = core.GoBack();
            }
        }
    }

    pub(super) fn go_forward(&self) {
        if let Some(core) = self.web.core() {
            // SAFETY: plain COM call on this window thread.
            unsafe {
                let _ = core.GoForward();
            }
        }
    }

    pub(super) fn reload(&self) {
        if let Some(core) = self.web.core() {
            // SAFETY: plain COM call on this window thread.
            unsafe {
                let _ = core.Reload();
            }
        }
    }

    /// Retry after roblox.com couldn't be reached: its home (or sign-in) page again.
    pub(super) fn retry(&self) {
        self.open_home();
    }

    /// Save to Bloxkeep, on a blank tab someone signed in on.
    pub(super) fn save(self: &Rc<Self>) {
        self.save_account();
    }

    async fn open(self: Rc<Self>) {
        if self.has_browser.get() || self.torn_down.get() {
            return;
        }
        TABS_OPEN.with(|n| n.set(n.get() + 1));
        self.has_browser.set(true);
        let result: Result<(), (i32, String)> = async {
            let env = shared_browser().await?;
            // A profile of its own per tab, kept in memory only: nothing of the session reaches the disk.
            // SAFETY: COM calls on this window thread.
            let options = unsafe {
                let options = env.cast::<ICoreWebView2Environment10>().map_err(werr)?.CreateCoreWebView2ControllerOptions().map_err(werr)?;
                options.SetProfileName(&HSTRING::from(format!("w{}", new_guid().simple()))).map_err(werr)?;
                options.SetIsInPrivateModeEnabled(true).map_err(werr)?;
                options
            };
            let core = self.web.ensure(&env, Some(&options)).await.map_err(werr)?;
            if self.closing.get() || self.web.is_disposed() {
                return Ok(());
            }
            self.configure(&core, &env).map_err(werr)?;
            self.open_home();
            self.sync.start();
            Ok(())
        }
        .await;
        if let Err((code, message)) = result {
            let owner = Some(host::root(self.web.hwnd));
            if code as u32 == 0x8007_0002 {
                ui::error(owner, &l::t("Microsoft Edge WebView2 isn't installed. Install it from Microsoft's website, then reopen Bloxkeep."));
                TabCore::fire(&self.close_requested);
            } else if !self.closing.get() {
                ui::error(owner, &l::f("Couldn't open the Roblox website: {0}", &[&message]));
                TabCore::fire(&self.close_requested);
            }
        }
    }

    fn configure(self: &Rc<Self>, core: &ICoreWebView2, env: &ICoreWebView2Environment) -> host::WinResult<()> {
        // SAFETY: COM calls on this window thread.
        unsafe {
            let settings = core.Settings()?;
            let settings4 = settings.cast::<ICoreWebView2Settings4>()?;
            settings4.SetIsPasswordAutosaveEnabled(false)?;
            settings4.SetIsGeneralAutofillEnabled(false)?;
            // DevTools would show the session cookie, which is how cookie thieves talk people into sharing it.
            settings.SetAreDevToolsEnabled(false)?;
        }
        self.add_session(core)?;
        self.guard_sign_out(core, env)?;

        let weak = Rc::downgrade(self);
        let mut token = 0i64;
        // SAFETY: COM calls on this window thread; every handler runs here too.
        unsafe {
            let c = core.clone();
            core.add_NewWindowRequested(
                &NewWindowRequestedEventHandler::create(Box::new(move |_, args| {
                    let Some(args) = args else { return Ok(()) };
                    // Roblox pages stay in this tab; anything else opens in the person's own browser, without this account.
                    args.SetHandled(true)?;
                    let uri = host::text(|p| args.Uri(p));
                    if is_roblox(&uri) {
                        c.Navigate(&HSTRING::from(uri))?;
                    } else {
                        open_outside(&uri);
                    }
                    Ok(())
                })),
                &mut token,
            )?;
            // The site's name only (never the address, which can carry a one-time code), so it's plain this is roblox.com.
            let (w, c) = (weak.clone(), core.clone());
            core.add_SourceChanged(
                &SourceChangedEventHandler::create(Box::new(move |_, _| {
                    let Some(me) = w.upgrade() else { return Ok(()) };
                    let source = host::text(|p| c.Source(p));
                    let host_name = if is_roblox(&source) { uri_host(&source) } else { None };
                    if host_name == *me.host.borrow() {
                        return Ok(());
                    }
                    *me.host.borrow_mut() = host_name;
                    me.changed();
                    Ok(())
                })),
                &mut token,
            )?;
            let (w, c) = (weak.clone(), core.clone());
            core.add_HistoryChanged(
                &HistoryChangedEventHandler::create(Box::new(move |_, _| {
                    let Some(me) = w.upgrade() else { return Ok(()) };
                    me.can_go_back.set(host::flag(|b| c.CanGoBack(b)));
                    me.can_go_forward.set(host::flag(|b| c.CanGoForward(b)));
                    me.changed();
                    Ok(())
                })),
                &mut token,
            )?;
            // Only Roblox's own pages here, as in the sign-in window (BrowserLoginForm): this window shows no address, so
            // another site, signed-in looking, would pass for Roblox's (2026-10-30 review: a link or a redirect off the
            // site loaded inside it). A link elsewhere opens in the person's own browser, and only when they clicked it.
            core.add_NavigationStarting(
                &NavigationStartingEventHandler::create(Box::new(move |_, args| {
                    let Some(args) = args else { return Ok(()) };
                    let uri = host::text(|p| args.Uri(p));
                    if may_open_here(&uri) {
                        return Ok(());
                    }
                    args.SetCancel(true)?;
                    if host::flag(|b| args.IsUserInitiated(b)) {
                        open_outside(&uri);
                    }
                    Ok(())
                })),
                &mut token,
            )?;
            let w = weak.clone();
            core.add_NavigationStarting(
                &NavigationStartingEventHandler::create(Box::new(move |_, args| {
                    let (Some(me), Some(args)) = (w.upgrade(), args) else { return Ok(()) };
                    me.slow.stop();
                    if host::flag(|b| args.Cancel(b)) {
                        return Ok(());
                    }
                    me.slow.start();
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
                    me.show_retry.set(false);
                    me.say("", "");
                    Ok(())
                })),
                &mut token,
            )?;
            let w = weak.clone();
            core.add_NavigationCompleted(
                &NavigationCompletedEventHandler::create(Box::new(move |_, args| {
                    let (Some(me), Some(args)) = (w.upgrade(), args) else { return Ok(()) };
                    me.slow.stop();
                    let success = host::flag(|b| args.IsSuccess(b));
                    // A notice on the account may have been accepted on the way (Failed): asked again at the next read.
                    if success && me.ask_again_at.get() == AT_MAX {
                        me.ask_again_at.set(now_ms());
                    }
                    let mut status = COREWEBVIEW2_WEB_ERROR_STATUS::default();
                    let _ = args.WebErrorStatus(&mut status);
                    let mut http = 0i32;
                    if let Ok(args2) = args.cast::<ICoreWebView2NavigationCompletedEventArgs2>() {
                        let _ = args2.HttpStatusCode(&mut http);
                    }
                    // Canceled = superseded by another navigation (redirects); a status code = Roblox answered (a 404 page).
                    if success || http > 0 || status == COREWEBVIEW2_WEB_ERROR_STATUS_OPERATION_CANCELED || me.closing.get() {
                        return Ok(());
                    }
                    me.show_unreachable();
                    Ok(())
                })),
                &mut token,
            )?;
        }
        Ok(())
    }

    /// A saved account's session, only in this tab's profile, as a cookie for this browser session only.
    fn add_session(&self, core: &ICoreWebView2) -> host::WinResult<()> {
        let cookie = self.cookie.borrow().clone();
        if self.account_id.get().is_none() || cookie.is_empty() {
            return Ok(());
        }
        // SAFETY: COM calls on this window thread.
        unsafe {
            let manager = core.cast::<ICoreWebView2_2>()?.CookieManager()?;
            let session = manager.CreateCookie(&HSTRING::from(COOKIE_NAME), &HSTRING::from(cookie), &HSTRING::from(".roblox.com"), &HSTRING::from("/"))?;
            session.SetIsSecure(true)?;
            session.SetIsHttpOnly(true)?;
            manager.AddOrUpdateCookie(&session)?;
        }
        Ok(())
    }

    fn guard_sign_out(self: &Rc<Self>, core: &ICoreWebView2, env: &ICoreWebView2Environment) -> host::WinResult<()> {
        const FILTER: &str = "https://auth.roblox.com/v*/logout*";
        let filter = HSTRING::from(FILTER);
        // SAFETY: COM calls on this window thread; the handlers run here too.
        unsafe {
            // Also requests from workers and frames, on runtimes that can say so.
            let with_kinds = core.cast::<ICoreWebView2_22>().and_then(|c22| {
                c22.AddWebResourceRequestedFilterWithRequestSourceKinds(
                    &filter,
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
                    COREWEBVIEW2_WEB_RESOURCE_REQUEST_SOURCE_KINDS_ALL,
                )
            });
            if with_kinds.is_err() {
                core.AddWebResourceRequestedFilter(&filter, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL)?;
            }
            let weak = Rc::downgrade(self);
            let mut token = 0i64;
            let e = env.clone();
            core.add_WebResourceRequested(
                &WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
                    let (Some(me), Some(args)) = (weak.upgrade(), args) else { return Ok(()) };
                    let request = args.Request()?;
                    let uri = host::text(|p| request.Uri(p));
                    let method = host::text(|p| request.Method(p));
                    // "Log out of all other sessions" goes through (sync_session saves the new cookie). An unsaved tab signs
                    // out freely.
                    if me.account_id.get().is_none() || !is_sign_out(&uri) || !method.eq_ignore_ascii_case("POST") {
                        return Ok(());
                    }
                    let response = host::response(&e, None, 403, "Forbidden", "Content-Type: application/json")?;
                    args.SetResponse(&response)?;
                    let w = weak.clone();
                    host::defer(move || {
                        if let Some(me) = w.upgrade() {
                            me.sign_out_stopped();
                        }
                    });
                    Ok(())
                })),
                &mut token,
            )?;
            let weak = Rc::downgrade(self);
            core.add_NavigationStarting(
                &NavigationStartingEventHandler::create(Box::new(move |_, args| {
                    let (Some(me), Some(args)) = (weak.upgrade(), args) else { return Ok(()) };
                    let uri = host::text(|p| args.Uri(p));
                    if me.account_id.get().is_none() || !is_sign_out(&uri) {
                        return Ok(());
                    }
                    args.SetCancel(true)?;
                    let w = weak.clone();
                    host::defer(move || {
                        if let Some(me) = w.upgrade() {
                            me.sign_out_stopped();
                        }
                    });
                    Ok(())
                })),
                &mut token,
            )?;
        }
        Ok(())
    }

    fn sign_out_stopped(&self) {
        if self.asking_logout.get() || self.closing.get() || self.torn_down.get() {
            return;
        }
        self.asking_logout.set(true);
        let close = (self.ask_close_on_logout.get())(host::root(self.web.hwnd));
        self.asking_logout.set(false);
        if close && !self.torn_down.get() {
            TabCore::fire(&self.close_requested);
        }
    }

    fn open_home(&self) {
        let Some(core) = self.web.core() else { return };
        self.show_retry.set(false);
        self.say(&l::t("Opening roblox.com…"), "");
        let url = if self.account_id.get().is_none() && self.user.borrow().is_none() {
            LOGIN_URL.to_string()
        } else {
            self.start_url.borrow().clone().unwrap_or_else(|| HOME_URL.to_string())
        };
        // SAFETY: plain COM call on this window thread.
        unsafe {
            let _ = core.Navigate(&HSTRING::from(url));
        }
    }

    /// An open tab goes to `url`, a page on roblox.com.
    pub(super) fn go(&self, url: &str) {
        *self.start_url.borrow_mut() = Some(url.to_string());
        if let Some(core) = self.web.core() {
            // SAFETY: plain COM call on this window thread.
            unsafe {
                let _ = core.Navigate(&HSTRING::from(url));
            }
        }
    }

    fn show_unreachable(&self) {
        if self.closing.get() {
            return;
        }
        self.show_retry.set(true);
        self.say(&l::t("Can't reach roblox.com. Check your internet connection, then press Retry."), "bad");
    }

    fn say(&self, text: &str, tone: &str) {
        if tone == "bad" {
            crate::core::app::App::note_shown("Website tab", text);
        }
        *self.message.borrow_mut() = text.to_string();
        *self.tone.borrow_mut() = tone.to_string();
        self.changed();
    }

    /// The account picker (see WebsiteTab::switch_to), on the tab's thread.
    async fn switch_to(self: Rc<Self>, id: Guid, name: String, cookie: String, avatar: Option<String>) {
        if self.torn_down.get() {
            return;
        }
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        if self.account_id.get().is_some() && self.web.core().is_some() {
            self.clone().sync_for_at_most(Duration::from_millis(3000)).await;
        }
        if generation != self.generation.get() || self.torn_down.get() {
            return;
        }
        self.set_account(Some(id));
        *self.cookie.borrow_mut() = cookie;
        *self.user.borrow_mut() = None;
        self.can_save.set(false);
        *self.failed_session.borrow_mut() = None;
        self.ask_again_at.set(AT_MIN);
        self.failures.set(0);
        self.sync.set_interval(SAVED_SYNC_MS);
        *self.start_url.borrow_mut() = None;
        self.set_avatar(avatar);
        self.set_name(Some(name));
        // Not started yet: it signs in as this account when it is (open).
        let Some(core) = self.web.core() else {
            self.changed();
            return;
        };
        let source = host::text(|p| unsafe { core.Source(p) });
        let here = if is_roblox(&source) { Some(source) } else { None };
        let result: host::WinResult<()> = async {
            self.sync.stop();
            // SAFETY: plain COM call on this window thread.
            unsafe { core.Stop()? };
            self.say(&l::t("Opening roblox.com…"), "");
            clear_session_data(&core).await?;
            if generation != self.generation.get() || self.torn_down.get() {
                return Ok(());
            }
            self.add_session(&core)?;
            self.show_retry.set(false);
            // SAFETY: plain COM call on this window thread.
            unsafe { core.Navigate(&HSTRING::from(here.unwrap_or_else(|| HOME_URL.to_string())))? };
            self.sync.start();
            Ok(())
        }
        .await;
        if let Err(e) = result {
            if !self.torn_down.get() {
                self.say(&l::f("Couldn't open the Roblox website: {0}", &[&e.message()]), "bad");
            }
        }
    }

    /// Task.WhenAny(SyncSessionAsync(), Task.Delay(d)): the read goes on by itself if it takes longer.
    async fn sync_for_at_most(self: Rc<Self>, d: Duration) {
        let (tx, rx) = oneshot::channel::<()>();
        host::spawn(async move {
            self.sync_session().await;
            let _ = tx.send(());
        });
        let _ = select(rx, Box::pin(host::delay(d))).await;
    }

    /// Reads the session the site holds; a different one (Roblox renewed it) goes to the app to check and save.
    async fn sync_session(self: Rc<Self>) {
        if self.syncing.get() {
            return;
        }
        let Some(core) = self.web.core() else { return };
        self.syncing.set(true);
        let generation = self.generation.get();
        if let Ok(cookies) = host::cookies(&core, "https://www.roblox.com").await {
            let now = cookies.into_iter().find(|(name, value)| name == COOKIE_NAME && !value.is_empty()).map(|(_, value)| value);
            // Switched to another account meanwhile: what was read may be either one's.
            if generation == self.generation.get() {
                match self.account_id.get() {
                    None => self.clone().track_sign_in(now).await,
                    Some(id) => {
                        // None: signed out some other way, or not yet; the saved session is left as it is.
                        let fresh = now.filter(|n| *n != *self.cookie.borrow() && !self.waiting(n));
                        if let Some(now) = fresh {
                            let sent = now.clone();
                            let saved = host::ask_core::<SessionChanged, bool>(self.shared.session_fn, move |f, tx| {
                                let answer = f(id, sent);
                                rt::spawn(async move {
                                    let _ = tx.send(answer.await);
                                });
                            })
                            .await;
                            if generation == self.generation.get() {
                                match saved {
                                    Some(true) => *self.cookie.borrow_mut() = now,
                                    Some(false) => self.failed(&now, false),
                                    None => {}
                                }
                            }
                        }
                    }
                }
            }
        }
        // An error: the page is navigating or the tab is closing; the next read tries again.
        self.syncing.set(false);
    }

    /// A blank tab: who signed in on the site (or out), shown as its title, with Save to Bloxkeep.
    async fn track_sign_in(self: Rc<Self>, now: Option<String>) {
        let now = now.unwrap_or_default();
        if now == *self.cookie.borrow() || self.waiting(&now) {
            return;
        }
        let mut user = None;
        if !now.is_empty() {
            let identify = self.identify.clone();
            let asked = now.clone();
            match host::blocking(move || identify(asked)).await {
                Some(Ok(found)) => user = found,
                Some(Err(ex)) if roblox_api::is_expected_error(&ex) => {
                    self.failed(&now, is_moderated(&ex));
                    return; // no answer from Roblox: asked again later
                }
                Some(Err(ex)) => {
                    // PORT: any other failure escaped the C#'s timer handler; logged here, and the next read tries again.
                    log::write(&ex);
                    return;
                }
                None => return,
            }
        }
        if self.closing.get() || self.account_id.get().is_some() {
            return;
        }
        *self.cookie.borrow_mut() = now;
        let signed_in = user.clone();
        *self.user.borrow_mut() = user; // a cookie that doesn't work is nobody
        self.set_name(None);
        self.can_save.set(signed_in.is_some());
        match signed_in {
            Some(user) => {
                let text = if prefs::lock().hide_names { l::t("Signed in.") } else { l::f("Signed in as {0}.", &[&user.name]) };
                self.say(&text, "");
            }
            None => self.changed(),
        }
    }

    fn waiting(&self, session: &str) -> bool {
        self.failed_session.borrow().as_deref() == Some(session) && now_ms() < self.ask_again_at.get()
    }

    fn failed(&self, session: &str, moderated: bool) {
        let same = self.failed_session.borrow().as_deref() == Some(session);
        self.failures.set(if same { self.failures.get() + 1 } else { 1 });
        *self.failed_session.borrow_mut() = Some(session.to_string());
        let wait = if self.failures.get() > 1 { LONGEST_WAIT } else { FIRST_WAIT };
        self.ask_again_at.set(if moderated { AT_MAX } else { now_ms() + wait.as_millis() as i64 });
    }

    fn save_account(self: &Rc<Self>) {
        let Some(user) = self.user.borrow().clone() else { return };
        let cookie = self.cookie.borrow().clone();
        if cookie.is_empty() || self.account_id.get().is_some() || self.saving.get() {
            return;
        }
        // PORT: the C# saved at once on the UI thread; here the controller is asked on its thread, so a second press
        // meanwhile is ignored (`saving`).
        self.saving.set(true);
        let me = self.clone();
        host::spawn(async move {
            let answer = host::ask_core::<SaveAccount, Res<(Guid, String, String)>>(me.shared.save_fn, move |f, tx| {
                let _ = tx.send(f(cookie, user));
            })
            .await;
            me.saving.set(false);
            match answer {
                Some(Ok((id, name, message))) => {
                    // From now on the saved account's tab: its sign-out is guarded and a renewed session saved.
                    me.set_account(Some(id));
                    *me.user.borrow_mut() = None;
                    me.sync.set_interval(SAVED_SYNC_MS);
                    me.can_save.set(false);
                    me.set_name(Some(name));
                    me.say(&message, "ok");
                }
                Some(Err(Fail::User(message))) => me.say(&message, "bad"),
                // PORT: only UserError was caught in the C#; anything else is logged here.
                Some(Err(ex)) => log::write(&ex),
                None => {}
            }
        });
    }

    /// Closes after one last read of a saved account's session (a moment at most).
    pub(super) async fn close(self: Rc<Self>) {
        if self.torn_down.get() {
            return;
        }
        if !self.synced.get() && self.account_id.get().is_some() && self.web.core().is_some() {
            self.synced.set(true);
            self.clone().sync_for_at_most(Duration::from_millis(3000)).await;
        }
        self.tear_down();
    }

    /// Closes without the last read of the session (the vault is locking, or the app quitting).
    pub(super) fn close_now(&self) {
        self.synced.set(true);
        self.tear_down();
    }

    fn tear_down(&self) {
        if self.torn_down.get() {
            return;
        }
        self.torn_down.set(true);
        self.closing.set(true);
        self.shared.mirror.lock().unwrap_or_else(|e| e.into_inner()).torn_down = true;
        self.slow.dispose();
        self.sync.dispose();
        self.web.dispose();
        if self.has_browser.get() {
            let left = TABS_OPEN.with(|n| {
                n.set(n.get() - 1);
                n.get()
            });
            let dir = ENV_DIR.with(|d| d.borrow().clone());
            if left == 0 {
                if let Some(dir) = dir {
                    // The last tab: the shared browser exits a moment after, and its folder goes with it.
                    ENV.with(|e| *e.borrow_mut() = None);
                    ENV_DIR.with(|d| *d.borrow_mut() = None);
                    crate::threads::run(move || delete_folder(&dir));
                }
            }
        }
        let _ = TABS.try_with(|t| t.borrow_mut().remove(&self.shared.id));
        // Closed: the controller hears it on its thread, then lets go of this tab's callbacks.
        let closed: Vec<u64> = self.shared.closed_fns.lock().unwrap_or_else(|e| e.into_inner()).drain(..).collect();
        for id in &closed {
            host::call_on_core::<ClosedFn>(*id, |f| f());
        }
        for id in closed.into_iter().chain([self.shared.session_fn, self.shared.save_fn]) {
            host::drop_on_core(id);
        }
    }

    /// Dispose().
    pub(super) fn dispose(&self) {
        self.tear_down();
    }
}

/// The browser all tabs share (of this window thread), made again after a failed start.
fn shared_browser() -> Shared<LocalBoxFuture<'static, Result<ICoreWebView2Environment, (i32, String)>>> {
    ENV.with(|slot| {
        let mut slot = slot.borrow_mut();
        let faulted = matches!(slot.as_ref().and_then(|f| f.peek()), Some(Err(_)));
        if slot.is_none() || faulted {
            let dir = std::env::temp_dir().join(format!("{FOLDER_PREFIX}{}", new_guid().simple()));
            ENV_DIR.with(|d| *d.borrow_mut() = Some(dir.clone()));
            let start: LocalBoxFuture<'static, Result<ICoreWebView2Environment, (i32, String)>> =
                Box::pin(async move { host::environment(&dir.to_string_lossy(), web_view2_setup::environment_options().map(Into::into)).await.map_err(werr) });
            *slot = Some(start.shared());
        }
        slot.clone().expect("set above")
    })
}

/// Profile.ClearBrowsingDataAsync(Cookies | AllDomStorage).
async fn clear_session_data(core: &ICoreWebView2) -> host::WinResult<()> {
    // SAFETY: COM calls on this window thread.
    let profile = unsafe { core.cast::<ICoreWebView2_13>()?.Profile()?.cast::<ICoreWebView2Profile2>()? };
    let (tx, rx) = oneshot::channel();
    let handler = ClearBrowsingDataCompletedHandler::create(Box::new(move |result| {
        let _ = tx.send(result);
        Ok(())
    }));
    let kinds = COREWEBVIEW2_BROWSING_DATA_KINDS(COREWEBVIEW2_BROWSING_DATA_KINDS_COOKIES.0 | COREWEBVIEW2_BROWSING_DATA_KINDS_ALL_DOM_STORAGE.0);
    // SAFETY: plain COM call; the answer comes through the handler.
    unsafe { profile.ClearBrowsingData(kinds, &handler)? };
    rx.await.unwrap_or_else(|_| Err(E_ABORT.into()))
}

/// RobloxApiException { Moderated: true }.
fn is_moderated(ex: &Fail) -> bool {
    ex.moderated()
}

/// Uri.TryCreate(url, Absolute) and its Host; None when it isn't an absolute address with a host.
pub fn uri_host(url: &str) -> Option<String> {
    url::Url::parse(url).ok()?.host_str().map(str::to_string)
}

pub fn is_roblox(url: &str) -> bool {
    let Ok(uri) = url::Url::parse(url) else { return false };
    let host = uri.host_str().unwrap_or("");
    uri.scheme() == "https" && (host.eq_ignore_ascii_case("roblox.com") || host.to_ascii_lowercase().ends_with(".roblox.com"))
}

/// What this tab may show: Roblox's pages; Play's roblox-player: link (Windows asks which app opens it, as in any
/// browser); a blank page, or one Bloxkeep writes in itself (the checks do; a page can't go to a data: address).
pub fn may_open_here(url: &str) -> bool {
    if is_roblox(url) {
        return true;
    }
    match url.find(':') {
        Some(colon) if colon > 0 => matches!(url[..colon].to_lowercase().as_str(), "roblox-player" | "roblox" | "about" | "data"),
        _ => false,
    }
}

/// A web link, in the person's own browser; anything else (a file, another scheme) isn't opened.
pub fn open_outside(url: &str) {
    let Ok(uri) = url::Url::parse(url) else { return };
    if uri.scheme() != "https" && uri.scheme() != "http" {
        return;
    }
    let target = wide(uri.as_str());
    let verb = wide("open");
    // SAFETY: plain call with zero-ended strings; Windows opens the link with the person's default browser.
    unsafe { ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), target.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL) };
}

/// The site's sign-out: it would end this session on Roblox's side, and with it the account saved in Bloxkeep.
fn is_sign_out(url: &str) -> bool {
    let Ok(uri) = url::Url::parse(url) else { return false };
    if !is_roblox(url) {
        return false;
    }
    let path = uri.path();
    if uri.host_str().unwrap_or("").eq_ignore_ascii_case("auth.roblox.com") {
        let path = path.trim_end_matches('/');
        path.eq_ignore_ascii_case("/v1/logout") || path.eq_ignore_ascii_case("/v2/logout")
    } else {
        path.to_ascii_lowercase().starts_with("/logout")
    }
}

/// The question after a stopped sign-out: true closes the tab.
fn ask_close_on_logout(owner: isize) -> bool {
    ui::modal(|| {
        dialog::show(
            Some(owner),
            &l::t("Signing out here would also sign this account out of Bloxkeep, and you'd have to add it again. When you're done, just close this tab."),
            Some(&l::t("Sign-out stopped")),
            dialog::Kind::Info,
            &[Choice::new(l::t("Close tab")), Choice::new(l::t("Keep browsing"))],
            0,
            None,
            None,
            None,
        )
    }) == 0
}

/// The browser process exits a moment after the control is disposed, so deletion is retried.
fn delete_folder(dir: &std::path::Path) {
    for _ in 0..30 {
        if !dir.exists() {
            return;
        }
        match std::fs::remove_dir_all(dir) {
            Ok(()) => return,
            Err(_) => std::thread::sleep(Duration::from_millis(500)),
        }
    }
}
