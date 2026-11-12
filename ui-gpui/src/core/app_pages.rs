//! AppPages.cs: what every window showing one of the app's pages shares: the pages' own address, the one browser
//! profile, the files served from the app (and the custom theme's picture from the data folder), and the browser locked
//! down to them. The main window (ShellForm), the small page windows (WebToolWindow, TrayWindow) and the sign-in
//! browser's bar (ChromeView).
use std::path::PathBuf;

use webview2_com::{
    Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL, ICoreWebView2, ICoreWebView2Environment, ICoreWebView2EnvironmentOptions, ICoreWebView2Settings,
        ICoreWebView2Settings3, ICoreWebView2Settings4, ICoreWebView2Settings6, ICoreWebView2Settings8,
    },
    NavigationStartingEventHandler, NewWindowRequestedEventHandler, WebResourceRequestedEventHandler,
};
use windows::core::{HSTRING, Interface};

use crate::core::base::{Fail, Res};
use crate::core::forms::chrome_view::host;
use crate::core::themes;
use crate::core::vault;
use crate::core::web_view2_setup;

/// The pages' address; nothing else is ever shown in the app's browsers.
pub const ORIGIN: &str = "https://bloxkeep.local/";

/// The main window's browser profile folder, which every page window shares (one browser and GPU process).
pub fn environment_folder() -> PathBuf {
    // Separate profile folders, because a running browser can't be reopened with different switches.
    let safe = crate::core::prefs::lock().safe_rendering;
    vault::cache_folder(if safe { "webview-safe" } else { "webview" })
}

fn com(e: windows::core::Error) -> Fail {
    Fail::Other(format!("{} (0x{:08X})", e.message(), e.code().0))
}

/// The main window's browser profile, which every page window shares (one browser and GPU process).
/// CoreWebView2Environment.CreateAsync: awaited on a window thread (its answer comes through that thread's messages).
pub async fn create_environment() -> Res<ICoreWebView2Environment> {
    let folder = environment_folder();
    let options: ICoreWebView2EnvironmentOptions = web_view2_setup::main_window_options().into();
    host::environment(&folder.to_string_lossy(), Some(options)).await.map_err(com)
}

/// No browser parts: dev tools, status bar, zoom, its own keys, saved passwords and forms, swipes. Only our own
/// embedded pages are ever shown, so SmartScreen lookups are wasted work.
pub fn settings(settings: &ICoreWebView2Settings) -> Res<()> {
    // SAFETY: COM calls on the browser's own (window) thread.
    unsafe {
        settings.SetAreDevToolsEnabled(false).map_err(com)?;
        settings.SetIsStatusBarEnabled(false).map_err(com)?;
        settings.SetIsZoomControlEnabled(false).map_err(com)?;
        settings.cast::<ICoreWebView2Settings3>().and_then(|s| s.SetAreBrowserAcceleratorKeysEnabled(false)).map_err(com)?;
        let s4 = settings.cast::<ICoreWebView2Settings4>().map_err(com)?;
        s4.SetIsPasswordAutosaveEnabled(false).map_err(com)?;
        s4.SetIsGeneralAutofillEnabled(false).map_err(com)?;
        settings.cast::<ICoreWebView2Settings6>().and_then(|s| s.SetIsSwipeNavigationEnabled(false)).map_err(com)?;
        settings.cast::<ICoreWebView2Settings8>().and_then(|s| s.SetIsReputationCheckingRequired(false)).map_err(com)?;
    }
    Ok(())
}

/// A small page's browser: `settings` and no right-click menu, the app's files served, only its own pages, never a
/// new window.
pub fn configure(core: &ICoreWebView2, env: &ICoreWebView2Environment) -> Res<()> {
    // SAFETY: COM calls on the browser's own (window) thread; the handlers run there too.
    unsafe {
        let s = core.Settings().map_err(com)?;
        settings(&s)?;
        s.SetAreDefaultContextMenusEnabled(false).map_err(com)?;
        core.AddWebResourceRequestedFilter(&HSTRING::from(format!("{ORIGIN}*")), COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL).map_err(com)?;
        let mut token = 0i64;
        let e = env.clone();
        core.add_WebResourceRequested(
            &WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else { return Ok(()) };
                let request = args.Request()?;
                let uri = host::text(|p| request.Uri(p));
                let answer = serve_asset(&uri);
                let body = if answer.status == 200 { Some(answer.body.as_slice()) } else { None };
                let response = host::response(&e, body, answer.status as i32, answer.reason, &answer.headers)?;
                args.SetResponse(&response)
            })),
            &mut token,
        )
        .map_err(com)?;
        core.add_NavigationStarting(
            &NavigationStartingEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else { return Ok(()) };
                let uri = host::text(|p| args.Uri(p));
                args.SetCancel(!may_navigate(&uri))
            })),
            &mut token,
        )
        .map_err(com)?;
        core.add_NewWindowRequested(
            &NewWindowRequestedEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else { return Ok(()) };
                args.SetHandled(true)
            })),
            &mut token,
        )
        .map_err(com)?;
    }
    Ok(())
}

/// Whether a navigation may go ahead: only the app's own pages (NavigationStarting cancels anything else).
pub fn may_navigate(uri: &str) -> bool {
    uri.len() >= ORIGIN.len() && uri.is_char_boundary(ORIGIN.len()) && uri[..ORIGIN.len()].eq_ignore_ascii_case(ORIGIN)
}

/// An answer to a page's request (CreateWebResourceResponse's parts).
#[derive(Clone, Debug)]
pub struct Response {
    pub status: u16,
    pub reason: &'static str,
    /// "Content-Type: …\r\nCache-Control: no-store", or "" for a 404.
    pub headers: String,
    pub body: Vec<u8>,
}

/// A file the C# app carried in its assembly as "wwwroot/<path>": the small pages that are still web pages (the app's
/// own message boxes, the tray's list, a macro's panel, the mini window, the name over a Roblox window, the website
/// window's top).
// PORT: nothing is embedded here; the files sit in a "wwwroot" folder beside the program (BLOXKEEP_WWWROOT names another,
// for a run from the source tree). Only a plain file inside that folder is ever read: no "..", no drive, no backslash.
fn manifest_resource(name: &str) -> Option<Vec<u8>> {
    let path = name.strip_prefix("wwwroot/")?;
    if path.is_empty() || path.contains("..") || path.contains(':') || path.contains('\\') || path.starts_with('/') {
        return None;
    }
    let root = match std::env::var_os("BLOXKEEP_WWWROOT") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => std::env::current_exe().ok()?.parent()?.join("wwwroot"),
    };
    let file = root.join(path);
    if !file.is_file() {
        return None;
    }
    std::fs::read(file).ok()
}

/// What is served for a request to `uri` (ServeAsset).
pub fn serve_asset(uri: &str) -> Response {
    let path = url::Url::parse(uri).map(|u| u.path().to_string()).unwrap_or_default();
    let path = path.trim_start_matches('/').to_string();
    // The custom theme's picture: only a file of that name, read whole so no handle stays open (a new picture deletes the old one).
    let user = path.strip_prefix("user/");
    let mut picture = user.filter(|name| themes::is_picture_name(name)).map(|name| vault::pictures_dir().join(name));
    // One Vault.Tidy couldn't move yet is still at the top.
    if let (Some(p), Some(name)) = (&picture, user) {
        if !p.is_file() {
            picture = Some(vault::data_dir().join(name));
        }
    }
    let stream = if user.is_some() {
        // The picture went away meanwhile: the theme's colour shows instead.
        picture.filter(|p| p.is_file()).and_then(|p| std::fs::read(p).ok())
    } else {
        manifest_resource(&format!("wwwroot/{path}"))
    };
    let Some(mut body) = stream else {
        return Response { status: 404, reason: "Not Found", headers: String::new(), body: Vec::new() };
    };
    if path == "index.html" {
        body = with_theme(&body);
    }
    let ext = path.rfind('.').filter(|&i| !path[i..].contains('/')).map(|i| &path[i..]).unwrap_or("");
    let kind = match ext {
        ".html" => "text/html; charset=utf-8",
        ".css" => "text/css; charset=utf-8",
        ".js" => "text/javascript; charset=utf-8",
        ".svg" => "image/svg+xml",
        ".png" => "image/png",
        ".jpg" | ".jpeg" => "image/jpeg",
        ".webp" => "image/webp",
        ".gif" => "image/gif",
        ".bmp" => "image/bmp",
        ".mp4" => "video/mp4",
        ".webm" => "video/webm",
        _ => "application/octet-stream",
    };
    Response { status: 200, reason: "OK", headers: format!("Content-Type: {kind}\r\nCache-Control: no-store"), body }
}

/// The page starts in the right theme, before its first state arrives, so it never flashes dark.
fn with_theme(page: &[u8]) -> Vec<u8> {
    let theme = themes::current();
    // A light custom theme takes Light's words and lines (app.css): from the first frame, not once its colours come.
    let tone = if theme.id == themes::CUSTOM && !theme.dark { " data-tone=\"light\"" } else { "" };
    // StreamReader: UTF-8 (a byte order mark dropped); written back as UTF-8.
    let text = String::from_utf8_lossy(page);
    let html = text.trim_start_matches('\u{feff}').replace("<html ", &format!("<html data-theme=\"{}\"{tone} ", theme.id));
    html.into_bytes()
}
