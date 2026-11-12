// GpuiHost.cs: Bloxkeep 1.1's interface drawn with GPUI in place of the WebView2 page. The C# started it as a second
// process (bloxkeep-ui.exe, behind the switch BLOXKEEP_UI=gpui), talked to it over two named pipes, one JSON message per
// line, and laid its window inside the shell as a child. Here the interface is in the same process: what still matters is
// kept (what it is given at start: the cache and pictures folders; the messages both ways, the same JSON the page traded
// with the controller; its window, once made, handed to the shell, which keeps the title bar's frame, tray, hotkeys and
// cover); the process, the pipes, the reparenting and the keyboard hand-over went with the second process.
use std::sync::{Mutex, OnceLock};

use windows_sys::Win32::UI::{
    HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi},
    WindowsAndMessaging::{SM_CXPADDEDBORDER, SM_CYSIZEFRAME, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos},
};

use super::{app::App, log, rt, shell_form, vault, win_kit::hwnd};

type Receiver = Box<dyn Fn(String) + Send + Sync>;
type Runner = Box<dyn FnOnce() + Send>;

/// Where the controller's messages go (the interface's side of the old pipe "name").
static RECEIVER: Mutex<Option<Receiver>> = Mutex::new(None);
/// What runs the interface until it closes (the binary's own: GPUI's application), set before program::main_core.
static RUNNER: Mutex<Option<Runner>> = Mutex::new(None);
static STARTED: OnceLock<()> = OnceLock::new();

/// The interface's message loop, given by the binary before it calls program::main_core; run where the C# ran
/// Application.Run(shell).
pub fn set_runner(run: impl FnOnce() + Send + 'static) {
    *RUNNER.lock().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(run));
}

/// What the interface is passed at start (the old command line's --cache and --pictures): where what Roblox said may be
/// kept between runs, and the pictures the user chose (a vault's face, the page's "user/…" files). Once.
pub fn start() {
    if STARTED.set(()).is_err() {
        return;
    }
    crate::roblox::keep_in(Some(vault::cache_dir()));
    crate::images::keep_user(Some(vault::pictures_dir()));
}

/// Runs the interface to its end (Application.Run): the runner the binary gave; false when none was given.
pub fn run() -> bool {
    start();
    let runner = RUNNER.lock().unwrap_or_else(|e| e.into_inner()).take();
    match runner {
        Some(run) => {
            run();
            true
        }
        None => {
            log::note("The interface has no runner (gpui_host::set_runner wasn't called); nothing shown.");
            false
        }
    }
}

/// The interface listens here for the controller's messages (the page's postJson).
pub fn set_receiver(f: impl Fn(String) + Send + Sync + 'static) {
    *RECEIVER.lock().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(f));
}

/// A message for the interface (AppController's postJson); in order, from any thread.
pub fn post(json: String) {
    if let Some(f) = RECEIVER.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        f(json);
    }
}

/// A message from the interface: as the page's messages, on the core's thread, outside the reading, so a command may
/// open a modal window.
pub fn on_message(line: String) {
    if line.is_empty() {
        return;
    }
    rt::post(move || {
        App::with(|app| {
            let app = app.clone();
            rt::spawn(async move {
                app.handle_message(&line).await;
            });
        })
    });
}

/// The interface's window exists (the old "hello" with its handle): the shell takes it over (its place, the tray,
/// hotkeys, the start cover). From the interface's thread, which then is the windows thread.
pub fn window_made(h: isize) {
    shell_form::attach(h);
}

/// The interface closed its window or quit (the old "Gone"): the app quits, as nothing else can show it.
pub fn gone(why: &str) {
    log::note(why);
    shell_form::with(|shell| shell.quit());
}

/// Has Windows measure the window's frame again (after the title bar goes or comes back).
pub fn frame_changed(form: isize) {
    // SAFETY: plain call on our own window.
    unsafe {
        SetWindowPos(hwnd(form), std::ptr::null_mut(), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED);
    }
}

/// How far a maximised window's frame reaches past the screen's edge, at the window's scale.
pub fn frame_thickness(form: isize) -> i32 {
    // SAFETY: plain queries.
    unsafe {
        let dpi = GetDpiForWindow(hwnd(form));
        GetSystemMetricsForDpi(SM_CYSIZEFRAME, dpi) + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi)
    }
}
