// CaptchaForm.cs: the captcha Roblox put in front of a password sign-in (or a join), shown as roblox.com would.
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    io::Read,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, LazyLock, Mutex},
    time::{Duration, Instant},
};

use futures::channel::oneshot;
use regex::Regex;
use serde_json::{Value, json};
use webview2_com::{Microsoft::Web::WebView2::Win32::*, NewWindowRequestedEventHandler, WebMessageReceivedEventHandler, WebResourceRequestedEventHandler};
use windows::core::{HSTRING, Interface};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    UI::Input::KeyboardAndMouse::EnableWindow,
    UI::WindowsAndMessaging::{
        DestroyWindow, SW_SHOW, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos, WM_CLOSE, WM_DESTROY, WM_DPICHANGED, WM_ERASEBKGND, WM_SIZE,
        WS_CLIPCHILDREN, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_OVERLAPPEDWINDOW,
    },
};

use super::chrome_view::host::{self, DialogResult, Invoker, Timer, WebHost};
use crate::core::{
    l, log,
    roblox_api::realm,
    themes::{self, Color},
    ui, vault, web_view2_setup,
    win_kit::Rect,
};

const PROFILE_PREFIX: &str = "Bloxkeep-captcha-";
/// A blocked connection hangs instead of failing, so don't wait for the puzzle forever.
const SLOW_AFTER: Duration = Duration::from_secs(20);

/// The relay to HUMAN's hosts: no cookies of its own, any compression undone, 20 s at most.
static RELAY: LazyLock<ureq::Agent> = LazyLock::new(|| crate::net::builder().timeout(Duration::from_secs(20)).build());
// PORT: HttpClient's AutomaticDecompression = All; ureq 2 undoes gzip (its default feature) but not brotli or deflate.

/// HUMAN's first-party routes: /{app id without "PX"}/init.js, /xhr/…, /captcha/….
static FIRST_PARTY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^/(?<app>[A-Za-z0-9]{6,12})/(?<kind>init\.js|xhr|captcha)(?<rest>/.*)?$").expect("first-party pattern"));

/// Shows the captcha Roblox put in front of a password sign-in, as roblox.com would:
/// - Arkose Labs' FunCaptcha ("captcha") from arkoselabs.roblox.com; `token` is its token.
/// - HUMAN's press-and-hold ("captchav2") from apis.roblox.com/v2/captcha; `token` is Roblox's redemption token.
///
/// The host page is served by the app under the realm's site (www.roblox.com), which some networks (Vietnam) block while
/// the captcha and API hosts aren't; HUMAN's "first-party" requests there are passed on to HUMAN's own hosts.
///
/// Made and shown from the core's thread; the window runs on a thread of its own.
pub struct CaptchaForm {
    site: String,
    page: String,
    relay_human: bool,
    /// From opening the window to the puzzle being ready, and to it being solved: written to the log.
    clock: Instant,
    window: Arc<Mutex<Slot>>,
    /// Never shown (the checks).
    unseen: bool,
}

#[derive(Default)]
struct Slot {
    invoker: Option<Invoker>,
    hwnd: isize,
    close_asked: bool,
    token: Option<String>,
    result: Option<DialogResult>,
}

impl CaptchaForm {
    fn new(site: &str, page: String, relay_human: bool) -> CaptchaForm {
        CaptchaForm { site: site.to_string(), page, relay_human, clock: Instant::now(), window: Arc::new(Mutex::new(Slot::default())), unseen: false }
    }

    /// The same window, never shown: for the checks.
    #[cfg(any(test, feature = "core"))]
    pub fn unseen(mut self) -> CaptchaForm {
        self.unseen = true;
        self
    }

    /// One kept profile for the puzzle windows, so Arkose's scripts stay cached and its "Verifying browser…" step knows the
    /// browser. It holds only the puzzle pages' data, never a Roblox sign-in.
    pub fn profile_dir() -> PathBuf {
        vault::cache_folder("captcha")
    }

    /// Arkose Labs' FunCaptcha, on the site of the realm whose key it is (as the press-and-hold is).
    pub fn arkose(realm: &str, public_key: &str, blob: &str) -> CaptchaForm {
        CaptchaForm::new(realm::website(realm), arkose_page(public_key, blob), false)
    }

    /// HUMAN's press-and-hold, for the "captchav2" challenge with this id.
    pub fn human(realm: &str, captcha_challenge_id: &str) -> CaptchaForm {
        CaptchaForm::new(realm::website(realm), human_page(&realm::api(realm, "apis"), captcha_challenge_id), true)
    }

    /// Token: set once the puzzle is solved.
    pub fn token(&self) -> Option<String> {
        self.window.lock().unwrap_or_else(|e| e.into_inner()).token.clone()
    }

    /// ShowDialog(owner): over the owner (disabled meanwhile) until solved (Ok) or closed (Cancel).
    pub async fn show_dialog(&self, owner: isize) -> DialogResult {
        let (done_tx, done_rx) = oneshot::channel::<()>();
        let spec = Spec { site: self.site.clone(), page: self.page.clone(), relay_human: self.relay_human, clock: self.clock, slot: self.window.clone(), unseen: self.unseen };
        log::note(if self.relay_human { "Captcha window: opening (press and hold)." } else { "Captcha window: opening (puzzle)." });
        if !host::start_thread("captcha window", move || CaptchaWindow::open(owner, spec, done_tx)) {
            log::note("Captcha window: its thread couldn't start.");
            return DialogResult::Cancel;
        }
        let _ = done_rx.await;
        // The app is usable again however the window went (a window whose thread failed never gave it back).
        if owner != 0 {
            // SAFETY: plain call; enabling an enabled window does nothing.
            unsafe { EnableWindow(owner as HWND, 1) };
        }
        self.window.lock().unwrap_or_else(|e| e.into_inner()).result.unwrap_or(DialogResult::Cancel)
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

    /// Removes the per-window profiles older versions made in %TEMP%.
    pub fn cleanup_leftover_profiles() {
        let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else { return };
        for entry in entries.flatten() {
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_dir && entry.file_name().to_string_lossy().starts_with(PROFILE_PREFIX) {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }
}

struct Spec {
    site: String,
    page: String,
    relay_human: bool,
    clock: Instant,
    slot: Arc<Mutex<Slot>>,
    unseen: bool,
}

/// The window, on its own thread.
struct CaptchaWindow {
    hwnd: isize,
    owner: isize,
    site: String,
    page_url: String,
    page: String,
    relay_human: bool,
    background: Color,
    slow: Rc<Timer>,
    unknown_paths: RefCell<HashSet<String>>,
    web: RefCell<Option<Rc<WebHost>>>,
    profile_dir: RefCell<Option<PathBuf>>,
    closing: Cell<bool>,
    clock: Instant,
    ready_noted: Cell<bool>,
    slot: Arc<Mutex<Slot>>,
    done: RefCell<Option<oneshot::Sender<()>>>,
}

impl CaptchaWindow {
    fn open(owner: isize, spec: Spec, done: oneshot::Sender<()>) {
        let background = themes::current().background;
        let style = (WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN) & !(WS_MINIMIZEBOX | WS_MAXIMIZEBOX);
        let hwnd = host::create_window(host::CLASS_FORM, &l::t("Solve the puzzle"), style, 0, owner, Rect::default());
        if hwnd == 0 {
            log::note("Captcha window: Windows didn't make the window.");
            let _ = done.send(());
            return;
        }
        let unseen = spec.unseen;
        host::set_icon(hwnd);
        let dpi = host::dpi(if owner != 0 { owner } else { hwnd });
        let (w, h) = if spec.relay_human { (host::logical(460, dpi), host::logical(340, dpi)) } else { (host::logical(500, dpi), host::logical(640, dpi)) };
        let around = if owner != 0 { host::window_rect(owner) } else { crate::core::win_kit::screen_from_handle(hwnd).working_area };
        host::set_bounds(hwnd, Rect::new(around.x + (around.width - w) / 2, around.y + (around.height - h) / 2, w, h));

        let me = Rc::new(CaptchaWindow {
            hwnd,
            owner,
            page_url: format!("{}/bloxkeep/captcha", spec.site),
            site: spec.site,
            page: spec.page,
            relay_human: spec.relay_human,
            background,
            slow: Timer::new(SLOW_AFTER.as_millis() as u32),
            unknown_paths: RefCell::new(HashSet::new()),
            web: RefCell::new(None),
            profile_dir: RefCell::new(None),
            closing: Cell::new(false),
            clock: spec.clock,
            ready_noted: Cell::new(false),
            slot: spec.slot,
            done: RefCell::new(Some(done)),
        });
        {
            let mut s = me.slot.lock().unwrap_or_else(|e| e.into_inner());
            s.invoker = Some(Invoker::current());
            s.hwnd = hwnd;
            if s.close_asked {
                host::close(hwnd);
            }
        }
        let weak = Rc::downgrade(&me);
        me.slow.on_tick({
            let weak = weak.clone();
            move || {
                if let Some(me) = weak.upgrade() {
                    me.slow.stop();
                    me.say(&l::t("Couldn't load the puzzle. Check your internet connection and try again."));
                }
            }
        });
        // The window keeps its own state for as long as Windows keeps the window (the handler goes at WM_NCDESTROY):
        // held only by the task that opens the page, it went once the page was up (see the sign-in window).
        host::set_handler(hwnd, {
            let me = me.clone();
            move |h: HWND, msg: u32, w: WPARAM, l: LPARAM| me.on_message(h, msg, w, l)
        });
        // OnHandleCreated: the themed title bar.
        ui::use_themed_title_bar(hwnd, &themes::current());
        // Load.
        {
            let me = me.clone();
            host::spawn(async move { me.open_page().await });
        }
        if owner != 0 {
            // SAFETY: plain call; the owner is enabled again when this window closes.
            unsafe { EnableWindow(owner as HWND, 0) };
        }
        if !unseen {
            host::show(hwnd, SW_SHOW);
        }
        let r = host::window_rect(hwnd);
        log::note(&format!("Captcha window: {}, {}×{}.", if unseen { "made (not shown)" } else { "shown" }, r.width, r.height));
    }

    fn on_message(self: &Rc<Self>, h: HWND, msg: u32, w: WPARAM, l: LPARAM) -> Option<isize> {
        match msg {
            WM_SIZE => {
                if let Some(web) = self.web.borrow().as_ref() {
                    let s = host::client_size(self.hwnd);
                    web.set_bounds(Rect::new(0, 0, s.width, s.height));
                }
                Some(0)
            }
            WM_ERASEBKGND => Some(host::fill_background(h, w, self.background)),
            WM_DPICHANGED => {
                // SAFETY: lparam is the RECT Windows suggests for the new scale.
                let r = unsafe { &*(l as *const windows_sys::Win32::Foundation::RECT) };
                // SAFETY: plain call.
                unsafe { SetWindowPos(h, std::ptr::null_mut(), r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOZORDER | SWP_NOACTIVATE) };
                Some(0)
            }
            WM_CLOSE => {
                if self.owner != 0 {
                    // SAFETY: plain call; enabled before this window goes so the owner gets the activation back.
                    unsafe { EnableWindow(self.owner as HWND, 1) };
                }
                // SAFETY: plain call on this window.
                unsafe { DestroyWindow(h) };
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

    async fn open_page(self: Rc<Self>) {
        let dir = CaptchaForm::profile_dir();
        *self.profile_dir.borrow_mut() = Some(dir.clone());
        let web = WebHost::new(self.hwnd, self.background);
        let s = host::client_size(self.hwnd);
        web.set_bounds(Rect::new(0, 0, s.width, s.height));
        *self.web.borrow_mut() = Some(web.clone());
        let result: host::WinResult<()> = async {
            let env = host::environment(&dir.to_string_lossy(), web_view2_setup::environment_options().map(Into::into)).await?;
            let core = web.ensure(&env, None).await?;
            if self.closing.get() || web.is_disposed() {
                return Ok(());
            }
            let weak = Rc::downgrade(&self);
            let mut token = 0i64;
            // SAFETY: COM calls on this window thread; the handlers run here too.
            unsafe {
                let settings = core.Settings()?;
                let settings4 = settings.cast::<ICoreWebView2Settings4>()?;
                settings4.SetIsPasswordAutosaveEnabled(false)?;
                settings4.SetIsGeneralAutofillEnabled(false)?;
                settings.SetAreDevToolsEnabled(false)?;
                core.add_NewWindowRequested(
                    &NewWindowRequestedEventHandler::create(Box::new(|_, args| {
                        if let Some(args) = args {
                            args.SetHandled(true)?;
                        }
                        Ok(())
                    })),
                    &mut token,
                )?;
                // The page itself, and (for HUMAN) anything else asked of the site, is answered here; the rest goes out.
                let filter = if self.relay_human { format!("{}/*", self.site) } else { self.page_url.clone() };
                core.AddWebResourceRequestedFilter(&HSTRING::from(filter), COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL)?;
                let w = weak.clone();
                let e = env.clone();
                core.add_WebResourceRequested(
                    &WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
                        if let (Some(me), Some(args)) = (w.upgrade(), args) {
                            me.on_site_request(&e, args);
                        }
                        Ok(())
                    })),
                    &mut token,
                )?;
                let w = weak.clone();
                core.add_WebMessageReceived(
                    &WebMessageReceivedEventHandler::create(Box::new(move |_, args| {
                        let (Some(me), Some(args)) = (w.upgrade(), args) else { return Ok(()) };
                        let json = host::text(|p| args.WebMessageAsJson(p));
                        me.on_page_message(&json);
                        Ok(())
                    })),
                    &mut token,
                )?;
                self.slow.start();
                core.Navigate(&HSTRING::from(self.page_url.as_str()))?;
                log::note("Captcha window: its browser is up, the puzzle page asked for.");
            }
            Ok(())
        }
        .await;
        if let Err(e) = result {
            log::note(&format!("Captcha window: its browser couldn't start ({}).", e.message()));
            if host::runtime_missing(&e) {
                ui::error(Some(self.hwnd), &l::t("Microsoft Edge WebView2 isn't installed. Install it, or add the account with a cookie."));
                host::close(self.hwnd);
            } else if !self.closing.get() {
                ui::error(Some(self.hwnd), &l::f("Couldn't open the captcha: {0}", &[&e.message()]));
                host::close(self.hwnd);
            }
        }
    }

    fn on_site_request(self: &Rc<Self>, env: &ICoreWebView2Environment, e: ICoreWebView2WebResourceRequestedEventArgs) {
        // SAFETY: COM calls on this window thread, inside the event.
        let uri = unsafe { e.Request().map(|r| host::text(|p| r.Uri(p))).unwrap_or_default() };
        let Ok(parsed) = url::Url::parse(&uri) else { return };
        let path = parsed.path().to_string();
        if path == "/bloxkeep/captcha" {
            if let Ok(response) = host::response(env, Some(self.page.as_bytes()), 200, "OK", "Content-Type: text/html; charset=utf-8\r\nCache-Control: no-store") {
                // SAFETY: plain COM call.
                unsafe {
                    let _ = e.SetResponse(&response);
                }
            }
            return;
        }
        if !self.relay_human {
            return;
        }
        let Some(m) = FIRST_PARTY.captures(&path) else {
            // Not something this window knows how to serve; it goes to the network (and fails where the site is blocked).
            if self.unknown_paths.borrow_mut().insert(path.clone()) {
                log::note(&format!("Captcha page asked the site for {path}"));
            }
            return;
        };
        let app = format!("PX{}", &m["app"]);
        let query = parsed.query().map(|q| format!("?{q}")).unwrap_or_default();
        let rest = format!("{}{}", m.name("rest").map(|r| r.as_str()).unwrap_or(""), query);
        let target = match &m["kind"] {
            "init.js" => format!("https://client.px-cloud.net/{app}/main.min.js"),
            "xhr" => format!("https://collector-{}.px-cloud.net{rest}", app.to_lowercase()),
            _ => format!("https://captcha.px-cdn.net/{app}{rest}"),
        };
        let me = self.clone();
        let env = env.clone();
        host::spawn(async move { me.relay(env, e, target).await });
    }

    /// Passes one of HUMAN's first-party requests on to HUMAN's own host and hands the answer back to the page.
    async fn relay(self: Rc<Self>, env: ICoreWebView2Environment, e: ICoreWebView2WebResourceRequestedEventArgs, target: String) {
        // SAFETY: COM calls on this window thread.
        let deferral = unsafe { e.GetDeferral() };
        // What the request says, read here (the COM objects stay on this thread); sent from a thread of its own.
        let read: host::WinResult<(String, Option<Vec<u8>>, Vec<(String, String)>)> = (|| {
            // SAFETY: COM calls on this window thread.
            unsafe {
                let request = e.Request()?;
                let method = host::text(|p| request.Method(p));
                let body = request.Content().ok().map(|s| host::read_stream(&s));
                let mut headers = Vec::new();
                let iterator = request.Headers()?.GetIterator()?;
                while host::flag(|b| iterator.HasCurrentHeader(b)) {
                    let mut name = windows::core::PWSTR::null();
                    let mut value = windows::core::PWSTR::null();
                    iterator.GetCurrentHeader(&mut name, &mut value)?;
                    headers.push((webview2_com::take_pwstr(name), webview2_com::take_pwstr(value)));
                    if !host::flag(|b| iterator.MoveNext(b)) {
                        break;
                    }
                }
                Ok((method, body, headers))
            }
        })();
        let answer: Result<(i32, String, Vec<u8>, String), String> = match read {
            Err(err) => Err(err.message().to_string()),
            Ok((method, body, headers)) => {
                let to = target.clone();
                host::blocking(move || send(&method, &to, body, &headers)).await.unwrap_or_else(|| Err("relay thread ended".into()))
            }
        };
        match answer {
            Ok((status, reason, bytes, kind)) => {
                if !self.closing.get() {
                    if let Ok(response) = host::response(&env, Some(&bytes), status, &reason, &format!("Content-Type: {kind}\r\nCache-Control: no-store")) {
                        // SAFETY: plain COM call.
                        unsafe {
                            let _ = e.SetResponse(&response);
                        }
                    }
                }
            }
            Err(message) => {
                let host_name = url::Url::parse(&target).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_default();
                log::note(&format!("Captcha relay to {host_name} failed: {message}"));
                if !self.closing.get() {
                    if let Ok(response) = host::response(&env, None, 502, "Bad Gateway", "") {
                        // SAFETY: plain COM call.
                        unsafe {
                            let _ = e.SetResponse(&response);
                        }
                    }
                }
            }
        }
        if let Ok(deferral) = deferral {
            // SAFETY: plain COM call; the request is answered.
            unsafe {
                let _ = deferral.Complete();
            }
        }
    }

    fn on_page_message(&self, json: &str) {
        if self.closing.get() {
            return;
        }
        let Ok(m) = serde_json::from_str::<Value>(json) else { return };
        let Some(o) = m.as_object() else { return };
        let secs = self.clock.elapsed().as_secs_f64();
        if o.contains_key("ready") {
            self.slow.stop();
            if !self.ready_noted.get() {
                log::note(&format!("Captcha ready after {secs:.1} s"));
            }
            self.ready_noted.set(true);
        } else if let Some(note) = o.get("log") {
            // PORT: GetString() on a value that isn't a string threw in the C# (not caught); read as "" here.
            log::note(&format!("Captcha: {}", note.as_str().unwrap_or("")));
        } else if let Some(value) = o.get("token").and_then(Value::as_str).filter(|v| !v.is_empty()) {
            {
                let mut s = self.slot.lock().unwrap_or_else(|e| e.into_inner());
                s.token = Some(value.to_string());
                s.result = Some(DialogResult::Ok);
            }
            log::note(&format!("Captcha solved after {secs:.1} s"));
            host::close(self.hwnd);
        } else if let Some(error) = o.get("error") {
            self.slow.stop();
            log::note(&format!("Captcha error: {}", error.as_str().unwrap_or("")));
            self.say(&l::t("The puzzle couldn't load. Close this window and sign in again."));
        }
    }

    /// Puts a message in place of the puzzle.
    fn say(&self, text: &str) {
        let core = self.web.borrow().as_ref().and_then(|w| w.core());
        if let Some(core) = core {
            // SAFETY: plain COM call on this window thread.
            unsafe {
                let _ = core.PostWebMessageAsJson(&HSTRING::from(json!({ "say": text }).to_string()));
            }
        }
    }

    fn on_form_closed(&self) {
        log::note(&format!("Captcha window: closed after {:.1} s.", self.clock.elapsed().as_secs_f32()));
        self.closing.set(true);
        self.slow.stop();
        self.slow.dispose();
        if let Some(web) = self.web.borrow_mut().take() {
            web.dispose();
        }
        if let Some(done) = self.done.borrow_mut().take() {
            let _ = done.send(());
        }
    }
}

/// The relay's request, as HttpClient sent it: the page's headers but Host and Content-Length, its body (with its
/// Content-Type) when it had one. Any answer from the host is passed on, an error status too.
fn send(method: &str, target: &str, body: Option<Vec<u8>>, headers: &[(String, String)]) -> Result<(i32, String, Vec<u8>, String), String> {
    let mut req = RELAY.request(method, target);
    let mut content_type = None;
    for (key, value) in headers {
        if key.eq_ignore_ascii_case("Content-Type") {
            content_type = Some(value.clone());
        } else if key.eq_ignore_ascii_case("Cookie") || key.eq_ignore_ascii_case("Cookies") {
            // Only HUMAN's own cookies go on to HUMAN: the page's other roblox.com cookies stay here (the C# app sent
            // them all; HUMAN's own enforcers drop the Cookie header as sensitive, so this is no less than they send).
            if let Some(px) = px_cookies(value) {
                req = req.set("Cookie", &px);
            }
        } else if key.eq_ignore_ascii_case("Accept-Encoding") {
            // Not the page's ("gzip, deflate, br, zstd"): ureq undoes only gzip, so a brotli answer reached the page
            // still packed and without its Content-Encoding (HttpClient undid them all). ureq asks for gzip itself.
        } else if !key.eq_ignore_ascii_case("Host") && !key.eq_ignore_ascii_case("Content-Length") {
            req = req.set(key, value);
        }
    }
    let result = match body {
        Some(body) => {
            if let Some(kind) = &content_type {
                req = req.set("Content-Type", kind);
            }
            req.send_bytes(&body)
        }
        None => req.call(),
    };
    let res = match result {
        Ok(res) => res,
        Err(ureq::Error::Status(_, res)) => res,
        Err(e) => return Err(e.to_string()),
    };
    let status = res.status() as i32;
    let reason = res.status_text().to_string();
    let kind = res.header("Content-Type").unwrap_or("application/octet-stream").to_string();
    let mut bytes = Vec::new();
    res.into_reader().read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    Ok((status, reason, bytes, kind))
}

/// The cookies of a Cookie header that are HUMAN's own (_px…, pxcts), none when there are none.
pub(crate) fn px_cookies(header: &str) -> Option<String> {
    let kept: Vec<&str> = header
        .split(';')
        .map(str::trim)
        .filter(|c| {
            let name = c.split('=').next().unwrap_or_default().trim();
            name.starts_with("_px") || name == "pxcts"
        })
        .collect();
    (!kept.is_empty()).then(|| kept.join("; "))
}

/// JsonSerializer.Serialize(s) with its default encoder: a JS string literal, everything outside printable ASCII and
/// the HTML-sensitive characters written as \uXXXX (safe inside a <script>).
fn js(s: &str) -> String {
    let mut out = String::from("\"");
    for unit in s.encode_utf16() {
        match unit {
            0x22 => out.push_str("\\u0022"),
            0x5C => out.push_str("\\\\"),
            0x0A => out.push_str("\\n"),
            0x0D => out.push_str("\\r"),
            0x09 => out.push_str("\\t"),
            0x08 => out.push_str("\\b"),
            0x0C => out.push_str("\\f"),
            0x3C | 0x3E | 0x26 | 0x27 | 0x2B | 0x60 => out.push_str(&format!("\\u{unit:04X}")),
            u if !(0x20..0x7F).contains(&u) => out.push_str(&format!("\\u{u:04X}")),
            u => out.push(u as u8 as char),
        }
    }
    out.push('"');
    out
}

/// WebUtility.HtmlEncode.
fn esc(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '&' => out.push_str("&amp;"),
            '\'' => out.push_str("&#39;"),
            c if ('\u{A0}'..='\u{FF}').contains(&c) || c as u32 > 0xFFFF => out.push_str(&format!("&#{};", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Uri.EscapeDataString: all but the unreserved characters percent-encoded.
fn escape_data_string(s: &str) -> String {
    const SET: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC.remove(b'-').remove(b'_').remove(b'.').remove(b'~');
    percent_encoding::utf8_percent_encode(s, SET).to_string()
}

fn lang() -> &'static str {
    if l::language() == "vi" { "vi" } else { "en" }
}

fn hex(c: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

/// The page around the puzzle, in the app's theme.
fn style() -> String {
    let t = themes::current();
    fill(STYLE, &[("BACKGROUND", hex(t.background)), ("TEXT2", hex(t.text2)), ("TEXT", hex(t.text))])
}

/// The C#'s interpolated raw strings: each %%NAME%% of the template replaced by its value in one pass, so a value (a
/// blob from Roblox) is never read again as a template.
fn fill(template: &str, values: &[(&str, String)]) -> String {
    let mut out = String::with_capacity(template.len() + 1024);
    let mut rest = template;
    while let Some(start) = rest.find("%%") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find("%%").map(|end| (&after[..end], end)) {
            Some((name, end)) if values.iter().any(|(n, _)| *n == name) => {
                let value = &values.iter().find(|(n, _)| *n == name).expect("found above").1;
                out.push_str(value);
                rest = &after[end + 2..];
            }
            _ => {
                out.push_str("%%");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

const STYLE: &str = r#"html, body { margin: 0; height: 100%; background: %%BACKGROUND%%; color: %%TEXT2%%; font: 14px/1.5 "Segoe UI", system-ui, sans-serif; }
#msg { position: fixed; inset: 0; display: grid; place-items: center; padding: 24px; text-align: center; }
#msg > div { width: min(340px, 100%); }
#px-captcha { margin-top: 16px; }
button { margin-top: 14px; height: 38px; padding: 0 18px; border: 0; border-radius: 11px; background: %%TEXT%%; color: %%BACKGROUND%%; font: 600 13.5px "Segoe UI", sans-serif; cursor: pointer; }
[hidden] { display: none !important; }"#;

fn arkose_page(public_key: &str, blob: &str) -> String {
    fill(
        ARKOSE,
        &[
            ("LANG_ATTR", lang().to_string()),
            ("STYLE", style()),
            ("LOADING", esc(&l::t("Loading the puzzle…"))),
            ("AGAIN", esc(&l::t("Show the puzzle again"))),
            ("BLOB", js(blob)),
            ("LANG", js(lang())),
            ("CLOSED", js(&l::t("The puzzle was closed."))),
            ("KEY", escape_data_string(public_key)),
        ],
    )
}

const ARKOSE: &str = r#"<!doctype html>
<html lang="%%LANG_ATTR%%">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Captcha</title>
<style>%%STYLE%%</style>
</head>
<body>
<div id="msg"><div><div id="text">%%LOADING%%</div><button id="again" hidden>%%AGAIN%%</button></div></div>
<script>
  const post = m => window.chrome.webview.postMessage(m);
  const text = document.getElementById('text'), again = document.getElementById('again');
  let enforcement = null;
  window.chrome.webview.addEventListener('message', e => { if (e.data && e.data.say) { text.textContent = e.data.say; again.hidden = true; } });
  again.onclick = () => { again.hidden = true; enforcement && enforcement.run(); };
  window.setupEnforcement = e => {
    enforcement = e;
    e.setConfig({
      data: { blob: %%BLOB%% },
      language: %%LANG%%,
      onReady: () => { post({ ready: true }); text.textContent = ''; e.run(); },
      onHide: () => { text.textContent = %%CLOSED%%; again.hidden = false; },
      onCompleted: r => post({ token: r.token }),
      onError: r => post({ error: String((r && r.error && r.error.error) || 'unknown') }),
    });
  };
</script>
<script src="https://arkoselabs.roblox.com/v2/%%KEY%%/api.js" data-callback="setupEnforcement" async defer></script>
</body>
</html>"#;

/// Mirrors roblox.com's captchav2 flow (Challenge.js, Oct 2026): load HUMAN's sensor, wait briefly for its _px3 cookie,
/// POST {challenge_id} to /v2/captcha. 200 carries the redemption token; 403 the puzzle (window._px* settings and a
/// block script), after which HUMAN calls _pxOnCaptchaSuccess and the POST is sent again.
fn human_page(api_base: &str, challenge_id: &str) -> String {
    let t = themes::current();
    fill(
        HUMAN,
        &[
            ("LANG_ATTR", lang().to_string()),
            ("STYLE", style()),
            ("LOADING_HTML", esc(&l::t("Loading the puzzle…"))),
            ("TRY_AGAIN", esc(&l::t("Try again"))),
            ("API", js(&format!("{api_base}/v2/captcha"))),
            ("CHALLENGE", js(challenge_id)),
            ("LOADING", js(&l::t("Loading the puzzle…"))),
            ("SURFACE", hex(t.surface)),
            ("TEXT", hex(t.text)),
            ("PRESS", js(&l::t("Press and hold"))),
            ("CHECKING", js(&l::t("Checking…"))),
            ("FAILED", js(&l::t("That didn't work. Try again."))),
            ("HOLD", js(&l::t("Press and hold the button until it fills up."))),
        ],
    )
}

const HUMAN: &str = r#"<!doctype html>
<html lang="%%LANG_ATTR%%">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Captcha</title>
<style>%%STYLE%%</style>
</head>
<body>
<div id="msg"><div><div id="text">%%LOADING_HTML%%</div><div id="px-captcha"></div><button id="again" hidden>%%TRY_AGAIN%%</button></div></div>
<script>
  const API = %%API%%, CHALLENGE = %%CHALLENGE%%;
  const SENSOR = 'https://client.px-cloud.net/PXbf8PROpW/main.min.js';
  const post = m => window.chrome.webview.postMessage(m);
  const log = t => post({ log: t });
  const text = document.getElementById('text'), again = document.getElementById('again');
  const sleep = ms => new Promise(r => setTimeout(r, ms));
  let csrf = '', busy = false;
  window.chrome.webview.addEventListener('message', e => { if (e.data && e.data.say) { text.textContent = e.data.say; again.hidden = true; } });
  again.onclick = () => { again.hidden = true; text.textContent = %%LOADING%%; attempt(); };

  const loadScript = (id, src) => new Promise(done => {
    document.getElementById(id)?.remove();
    const s = document.createElement('script');
    s.id = id; s.src = src; s.async = true;
    s.onload = () => done(true); s.onerror = () => done(false);
    document.head.appendChild(s);
  });
  const hasPx3 = () => document.cookie.split('; ').some(c => c.startsWith('_px3='));

  async function submit() {
    for (let i = 0; i < 3; i++) {
      const headers = { 'Content-Type': 'application/json', 'Accept': 'application/json' };
      if (csrf) headers['x-csrf-token'] = csrf;
      const r = await fetch(API, { method: 'POST', credentials: 'include', headers, body: JSON.stringify({ challenge_id: CHALLENGE }) });
      let body = null;
      try { body = await r.json(); } catch {}
      const fresh = r.headers.get('x-csrf-token');
      const block = body && (body.appId ? body : body.data && body.data.appId ? body.data : null);
      if (r.status === 403 && fresh && fresh !== csrf && !block) { csrf = fresh; continue; }
      return { status: r.status, body, block };
    }
    return { status: 0, body: null, block: null };
  }

  function show(b) {
    window._pxAppId = b.appId; window._pxJsClientSrc = b.jsClientSrc; window._pxFirstPartyEnabled = b.firstPartyEnabled === true;
    window._pxVid = b.vid; window._pxUuid = b.uuid; window._pxHostUrl = b.hostUrl;
    if (typeof b.blockedUrl === 'string') window._pxBlockedUrl = b.blockedUrl;
    window['_' + b.appId] = { challenge: {
      view: { width: '100%', height: 48, backgroundColor: '%%SURFACE%%', fillColor: '#22c55e', borderColor: '%%SURFACE%%', borderWidth: 0,
        innerMargin: 0, textColor: '%%TEXT%%', borderRadius: 12, textFont: 'Segoe UI, sans-serif', texSize: 14, fontWeight: 600,
        textTransform: 'none', animation: false },
      translation: { default: { btn: %%PRESS%% } },
      context: { failed: { color: 'transparent', fontSize: '0px' } },
    } };
    window[b.appId + '_asyncInit'] = px => px.Events.on('captcha', ev => log('press-and-hold ' + ev));
    window._pxOnCaptchaSuccess = ok => {
      if (ok) { text.textContent = %%CHECKING%%; attempt(); }
      else { text.textContent = %%FAILED%%; again.hidden = false; }
    };
    text.textContent = %%HOLD%%;
    post({ ready: true });
    loadScript('captcha-v2-block-script', b.blockScript).then(ok => { if (!ok) post({ error: 'block script failed to load' }); });
  }

  async function attempt() {
    if (busy) return;
    busy = true;
    try {
      const r = await submit();
      log('v2/captcha HTTP ' + r.status + ', fields ' + (r.body ? Object.keys(r.body).join(', ') : 'none')
        + (r.block ? ', puzzle fields ' + Object.keys(r.block).join(', ') : ''));
      if (r.status === 200 && r.body && r.body.redemption_token) return post({ token: r.body.redemption_token });
      if (r.status === 403 && r.block) return show(r.block);
      post({ error: r.status === 404 ? 'session not found' : 'HTTP ' + r.status });
    } catch (e) {
      post({ error: String(e && e.message || e) });
    } finally {
      busy = false;
    }
  }

  (async () => {
    const loaded = await loadScript('captcha-v2-sensor', SENSOR);
    if (!loaded) log('sensor failed to load');
    post({ ready: true });
    for (let t = 0; t < 40 && !hasPx3(); t++) await sleep(50);
    attempt();
  })();
</script>
</body>
</html>"#;
