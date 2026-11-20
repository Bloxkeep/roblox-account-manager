// WebView2Setup.cs: installs the WebView2 Runtime when missing with Microsoft's bootstrapper, run only after Windows
// confirms a valid signature from Microsoft Corporation; and the switches every browser the app opens gets.

use webview2_com::CoreWebView2EnvironmentOptions;

use super::prefs;

const SAFE_RENDERING_ARGS: &str = "--disable-gpu --disable-gpu-compositing";

fn options(args: &str) -> CoreWebView2EnvironmentOptions {
    let options = CoreWebView2EnvironmentOptions::default();
    // SAFETY: sets a string on our own options object before any browser reads it.
    unsafe { options.set_additional_browser_arguments(args.to_string()) };
    options
}

/// Switches for every browser the app opens: CPU drawing when the graphics driver proved broken
/// (prefs' safe_rendering), else None.
pub fn environment_options() -> Option<CoreWebView2EnvironmentOptions> {
    if prefs::lock().safe_rendering { Some(options(SAFE_RENDERING_ARGS)) } else { None }
}

/// The main window shows only Bloxkeep's page, so the browser's own background fetching and component updates
/// are left out there; the sign-in, captcha and roblox.com windows keep a full browser.
/// Its disk cache is capped at 32 MB: the pages come from the app itself, so a bigger one only fills the disk.
const MAIN_WINDOW_ARGS: &str = "--disable-background-networking --disable-component-update --disk-cache-size=33554432";

/// Even on the GPU, the page is painted by the CPU and only composited on the card: GPU painting held about
/// 100 MB more for no smoother result (2026-10-18, RTX 4070 laptop, 80 accounts: 360 → 265 MB opened, 398 → 265 MB
/// after scrolling every tab, 254 → 225 MB minimised).
const GPU_ARGS: &str = "--disable-gpu-rasterization";

/// For measuring only, off unless set: switches in BLOXKEEP_WEBVIEW_ARGS are added to Bloxkeep's own pages, for
/// example "--process-per-site" (one renderer for the main page and every panel). Nothing ships on until measured.
const EXTRA_ARGS_VARIABLE: &str = "BLOXKEEP_WEBVIEW_ARGS";

/// The switches of the app's own pages' browser (main_window_options' text).
pub fn main_window_args() -> String {
    let safe = prefs::lock().safe_rendering;
    let args = if safe { format!("{MAIN_WINDOW_ARGS} {SAFE_RENDERING_ARGS}") } else { format!("{MAIN_WINDOW_ARGS} {GPU_ARGS}") };
    let extra = std::env::var(EXTRA_ARGS_VARIABLE).ok().map(|e| e.trim().to_string()).unwrap_or_default();
    if extra.is_empty() { args } else { format!("{args} {extra}") }
}

pub fn main_window_options() -> CoreWebView2EnvironmentOptions {
    options(&main_window_args())
}
