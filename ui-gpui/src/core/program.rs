// Program.cs: the start. Velopack's install/update/uninstall steps first, then one Bloxkeep per user session (a second
// start brings the running window forward), the files tidied and loaded, the App on its own thread (the core's), and the
// interface run to its end (gpui_host::run, where the C# ran Application.Run(shell)).
use std::{
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

use futures::channel::oneshot;
use windows_sys::Win32::{
    Foundation::{CloseHandle, ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0},
    System::Threading::{
        CreateEventW, CreateMutexW, EVENT_MODIFY_STATE, MUTEX_ALL_ACCESS, OpenEventW, OpenMutexW, ReleaseMutex, SYNCHRONIZATION_SYNCHRONIZE, SetEvent,
        WaitForSingleObject,
    },
    UI::WindowsAndMessaging::AllowSetForegroundWindow,
};

use super::{
    app::App,
    app_updater,
    forms::{browser_login_form::BrowserLoginForm, captcha_form::CaptchaForm, dialog, website_tab::WebsiteTab},
    gpui_host, l, log, prefs, relaunch, roblox_processes, rt, shell_form, start_with_windows, ui, vault, vaults, whats_new,
    win_kit::{Hive, RegKey, wide},
    window_icons,
};

const ACTIVATE_EVENT: &str = r"Local\Bloxkeep.Activate";
const QUIT_EVENT: &str = r"Local\Bloxkeep.Quit";
const SINGLE_INSTANCE_MUTEX: &str = r"Local\Bloxkeep.SingleInstance";
/// How long a start by hand waits for the run an update is starting; after that it starts as usual.
const UPDATE_RUN_WAIT: Duration = Duration::from_secs(90);
const ASFW_ANY: u32 = u32::MAX;

/// A named kernel handle, closed when dropped (the C#'s `using`).
pub struct Named(HANDLE);

// SAFETY: a kernel handle may be used and closed from any thread.
unsafe impl Send for Named {}
unsafe impl Sync for Named {}

impl Named {
    pub fn raw(&self) -> HANDLE {
        self.0
    }

    /// EventWaitHandle.Set.
    pub fn set(&self) {
        // SAFETY: our open handle.
        unsafe { SetEvent(self.0) };
    }

    /// WaitOne: true when signalled (or abandoned, with `abandoned` set), false on the timeout.
    pub fn wait(&self, ms: u32) -> (bool, bool) {
        // SAFETY: our open handle.
        let r = unsafe { WaitForSingleObject(self.0, ms) };
        (r == WAIT_OBJECT_0 || r == WAIT_ABANDONED, r == WAIT_ABANDONED)
    }

    pub fn release_mutex(&self) {
        // SAFETY: our open handle; fails harmlessly when not owned.
        unsafe { ReleaseMutex(self.0) };
    }
}

impl Drop for Named {
    fn drop(&mut self) {
        // SAFETY: closed once, here.
        unsafe { CloseHandle(self.0) };
    }
}

/// What opening or making a named handle gave: the handle, or no right to it (one started as administrator owns it,
/// the C#'s UnauthorizedAccessException), or nothing there.
pub enum Opened {
    Handle(Named),
    Denied,
    Missing,
}

/// Guarded: a named handle the way `open` gets it; None when another Bloxkeep started as administrator owns it and this
/// process has no right to it (or, for an open, there is none).
pub fn guarded(open: impl FnOnce() -> Opened) -> Option<Named> {
    match open() {
        Opened::Handle(h) => Some(h),
        Opened::Denied | Opened::Missing => None,
    }
}

fn after_call(h: HANDLE) -> Opened {
    if !h.is_null() {
        return Opened::Handle(Named(h));
    }
    // SAFETY: plain query right after the failed call.
    if unsafe { GetLastError() } == ERROR_ACCESS_DENIED { Opened::Denied } else { Opened::Missing }
}

/// new EventWaitHandle(false, mode, name).
pub fn create_event(name: &str, manual_reset: bool) -> Opened {
    let n = wide(name);
    // SAFETY: a zero-ended name; default security.
    after_call(unsafe { CreateEventW(std::ptr::null(), manual_reset as i32, 0, n.as_ptr()) })
}

/// new Mutex(true, name, out createdNew): the handle and whether this call made it (then it is owned).
pub fn create_mutex(name: &str) -> (Opened, bool) {
    let n = wide(name);
    // SAFETY: a zero-ended name; default security.
    let h = unsafe { CreateMutexW(std::ptr::null(), 1, n.as_ptr()) };
    // SAFETY: plain query right after the call.
    let existed = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    if !h.is_null() && existed {
        // Not made here, so not owned (the C#'s createdNew false).
        return (Opened::Handle(Named(h)), false);
    }
    (after_call(h), !h.is_null())
}

/// EventWaitHandle.TryOpenExisting.
pub fn open_event(name: &str) -> Opened {
    let n = wide(name);
    // SAFETY: a zero-ended name.
    after_call(unsafe { OpenEventW(EVENT_MODIFY_STATE | SYNCHRONIZATION_SYNCHRONIZE, 0, n.as_ptr()) })
}

/// Mutex.TryOpenExisting.
pub fn open_mutex(name: &str) -> Opened {
    let n = wide(name);
    // SAFETY: a zero-ended name.
    after_call(unsafe { OpenMutexW(MUTEX_ALL_ACCESS, 0, n.as_ptr()) })
}

/// The program's start (Program.Main). `args` without the program's own path.
pub fn main_core(args: &[String]) {
    // Out of the program folder first: programs Bloxkeep starts inherit it, and one still running (Bloxstrap stays
    // while Roblox does) made updates fail, as Velopack can't move a folder a process is in. No relative paths here.
    let _ = std::env::set_current_dir(std::env::temp_dir());
    // Install/update/uninstall steps from Velopack are handled (and exit) before anything else. Uninstalling keeps
    // the accounts and settings in %LOCALAPPDATA%\Bloxkeep unless the person chooses otherwise.
    // A downloaded update is applied at start (Velopack's default) only when no Bloxkeep runs: a second start (the
    // shortcut clicked to bring the window forward) would otherwise apply it, and its Update.exe kills the running one
    // mid-launch, unsaved (2026-10-30 review). That one applies it itself, at a quiet moment or on quitting.
    let another_runs = match open_mutex(SINGLE_INSTANCE_MUTEX) {
        Opened::Handle(_) => true, // dropped (closed) at once
        Opened::Denied => true,    // one started as administrator
        Opened::Missing => false,
    };
    // A downloaded update applied here restarts Bloxkeep through Velopack's Update.exe, and Windows lets the run it
    // starts come to the front only if this one, just started by the person, passes its right on: without it the
    // updated window opened behind the others (the owner, 2026-11-15: "nó ko tự pop up lên nó ở đằng sau").
    if !another_runs {
        // SAFETY: plain call.
        unsafe { AllowSetForegroundWindow(ASFW_ANY) };
    }
    velopack::VelopackApp::build()
        .set_auto_apply_on_startup(!another_runs)
        .on_before_uninstall_fast_callback(|_version| {
            // Velopack ends this step after 30 s, counted from before this process started: the question below is
            // answered by 22 s, which leaves time to delete the data (2026-10-30 review: 50 s were planned).
            let deadline = Instant::now() + Duration::from_secs(22);
            close_running_app();
            // Sign-in and captcha browser profiles in %TEMP%, normally removed at the next start that never comes.
            BrowserLoginForm::cleanup_leftover_profiles();
            CaptchaForm::cleanup_leftover_profiles();
            WebsiteTab::cleanup_leftovers();
            // Bloxkeep's uninstaller window has already asked and says so here; when Velopack's uninstall runs on
            // its own (an entry an update reset), ask now.
            let delete = match std::env::var("BLOXKEEP_UNINSTALL_DATA").ok().as_deref() {
                Some("delete") => true,
                Some("keep") => false,
                _ => ask_to_delete_data(deadline),
            };
            // The caches go once, after the answer (delete_all_data clears them itself): before the question, held files
            // could take up to 24 s of the 30 that Velopack allows, and the question's own page makes the webview cache again.
            if delete {
                vault::delete_all_data();
            } else {
                vault::delete_caches();
            }
        })
        .run();

    // Application.ThreadException and AppDomain.UnhandledException: logged, shown at most every few seconds.
    // PORT: a Rust panic is the nearest thing to an unhandled exception; the hook logs it and shows it (throttled), and
    // the thread it happened on still ends. The C#'s TaskScheduler.UnobservedTaskException has no counterpart (a task's
    // panic ends that task).
    // (A panic caught where the code goes on, as the C#'s try/catch did, is only logged: see catch_unwind.)
    std::panic::set_hook(Box::new(|info| {
        // Which thread: "core", "main" (the window), a watcher's own; the panic's words name only the line.
        let text = format!("{info} (thread \"{}\")", std::thread::current().name().unwrap_or("unnamed"));
        if CAUGHT.with(|c| c.get() > 0) {
            log::write(&text);
        } else {
            on_unexpected(&text);
        }
    }));
    log::catch_crashes();

    if !relaunch::is_update_run(args) {
        wait_for_update_run(UPDATE_RUN_WAIT);
    }

    // A second launch just brings the running window to the front.
    // One started as administrator owns these names, and a normal start can't open them: it says so rather than crash.
    let activate = guarded(|| create_event(ACTIVATE_EVENT, false));
    let quit = guarded(|| create_event(QUIT_EVENT, true));
    let mut first = false;
    let single = guarded(|| {
        let (opened, created) = create_mutex(SINGLE_INSTANCE_MUTEX);
        first = created;
        opened
    });
    let (Some(activate), Some(quit), Some(single)) = (activate, quit, single) else {
        log::note("Another Bloxkeep, started as administrator, owns the single-instance handles; this start gave way.");
        ui::error(None, &l::t("Bloxkeep is already running as administrator. Close it, then start Bloxkeep again."));
        return;
    };
    if !first {
        // This process was just started by the person, so it may pass its right to come to the front on to the
        // running one; without that Windows only flashes the running window's taskbar button.
        // SAFETY: plain call.
        unsafe { AllowSetForegroundWindow(ASFW_ANY) };
        activate.set();
        return;
    }

    vault::tidy(); // before anything opens the files it moves
    roblox_processes::show_left_hidden(); // Roblox windows a run that ended suddenly had hidden
    window_icons::clear_leftovers(); // …and the icons it had put on them (its taskbar groups: at the first window check)
    prefs::load();
    {
        let version = option_env!("BLOXKEEP_VERSION").unwrap_or(env!("CARGO_PKG_VERSION"));
        log::run_started(version.split('+').next().unwrap_or(version), ui::windows_build(), &l::language(), relaunch::is_update_run(args));
    }
    log::watch_window();
    vaults::load(); // the vault the lock screen offers (and an update's restart opens): the last one used
    BrowserLoginForm::cleanup_leftover_profiles();
    CaptchaForm::cleanup_leftover_profiles();
    WebsiteTab::cleanup_leftovers();
    use_our_uninstaller();
    // Started again by an update in the background: what the last run handed over (also removed when not).
    let resume = relaunch::take(args);
    start_with_windows::keep_path_current();
    shell_form::prepare(resume, args.iter().any(|a| a == start_with_windows::ARG));

    // The core's thread: the App lives there to the end. Its controller's window functions are wired before the
    // interface can ask anything (the C# constructor set them).
    let (stop_tx, stop_rx) = oneshot::channel::<()>();
    let (ready_tx, ready_rx) = mpsc::channel::<bool>();
    let core = crate::net::Named::new("core".into()).stack_size(16 << 20).spawn(move || {
        // COM for the window property stores (taskbar groups) and the shell calls made here; free-threaded, as this
        // thread pumps no messages.
        // SAFETY: once, first thing on this thread; it lives as long as the process.
        unsafe {
            windows_sys::Win32::System::Com::CoInitializeEx(std::ptr::null(), windows_sys::Win32::System::Com::COINIT_MULTITHREADED as u32);
        }
        rt::run(move || async move {
            // Nothing is running yet, so report why the App could not be made instead of vanishing.
            let made = std::panic::catch_unwind(std::panic::AssertUnwindSafe(App::start));
            match made {
                Ok(app) => {
                    shell_form::wire(&app);
                    let _ = ready_tx.send(true);
                    let _ = stop_rx.await;
                }
                Err(_) => {
                    let _ = ready_tx.send(false);
                }
            }
        })
    });
    let started = core.is_ok() && ready_rx.recv().unwrap_or(false);
    if !started {
        ui::error(None, &l::f("Unexpected error: {0}", &[&"the app could not start"]));
        return;
    }

    // Listened for at once; a request that comes before the window exists waits for it (the C# listened from
    // HandleCreated: a start by hand that waited for an update's run (wait_for_update_run) asks as soon as that run
    // takes the hand-over, before its window exists).
    let ending = std::sync::Arc::new(AtomicBool::new(false));
    let activate = std::sync::Arc::new(activate);
    let quit = std::sync::Arc::new(quit);
    {
        let (activate, ending) = (activate.clone(), ending.clone());
        let _ = crate::net::Named::new("activate-wait".into()).spawn(move || {
            while !ending.load(Ordering::SeqCst) {
                if !activate.wait(500).0 {
                    continue;
                }
                while !shell_form::is_attached() && !ending.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(100));
                }
                on_shell(|s| s.show_in_front("second start"));
            }
        });
    }
    {
        // The uninstaller asks the app to quit (not hide to the tray), so nothing it uses stays locked.
        // A downloaded update isn't applied on that quit: the installer or uninstaller runs Velopack's Update.exe next,
        // and a second one applying the update would race it for the folder (2026-10-30 review).
        let (quit, ending) = (quit.clone(), ending.clone());
        let _ = crate::net::Named::new("quit-wait".into()).spawn(move || {
            while !ending.load(Ordering::SeqCst) {
                if quit.wait(500).0 {
                    app_updater::HELD_FOR_INSTALLER.store(true, Ordering::SeqCst);
                    on_shell(|s| s.quit());
                    return;
                }
            }
        });
    }
    // Application.Run(shell): the interface to its end.
    let ran = std::panic::catch_unwind(gpui_host::run);
    // Nobody could show them once Bloxkeep is gone.
    roblox_processes::show_windows(None);
    ending.store(true, Ordering::SeqCst);
    let _ = stop_tx.send(());
    if let Ok(core) = core {
        let _ = core.join();
    }
    if ran.is_ok() {
        log::run_ended();
    }
    drop(single); // the single-instance mutex goes with the process, after everything else
    if let Err(e) = ran {
        // The window failed (it couldn't be made, or its thread panicked): what went wrong stays on screen until read.
        if let Some(h) = ERROR_BOX.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = h.join();
        }
        std::panic::resume_unwind(e);
    }
}

/// Runs a request from another process on the window's thread; the window may be going away meanwhile.
fn on_shell(f: impl FnOnce(&std::rc::Rc<shell_form::ShellForm>) + Send + 'static) {
    // Closed between the check and the call: nothing left to bring to the front or quit (shell_form::with drops it).
    if shell_form::is_attached() {
        shell_form::with(f);
    }
}

/// Opened during a background update: waits for the run the update starts, to bring it forward (as any second
/// launch) rather than take its place, which made that run (carrying the unlocked vault) give way, and Bloxkeep came back
/// locked (seen 2026-10-20). Over once that run took the hand-over (or the watch removed it), or after `limit`. Not as
/// soon as some Bloxkeep holds the single-instance mutex: right after "Restart to update" that is the run on its way out,
/// which got the request and vanished, and the update's run then came back in the tray with nothing on screen
/// (2026-10-30 review).
pub(crate) fn wait_for_update_run(limit: Duration) {
    let waiting = Instant::now();
    while waiting.elapsed() < limit && relaunch::hand_over_waiting() {
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// Before uninstalling: asks a running Bloxkeep to close and waits up to 10 s (of the hook's 30). Velopack already ends
/// processes in the program folder; this covers one it couldn't see (running elevated): that one can't be asked either
/// (no access), and the uninstall goes on without it.
fn close_running_app() {
    close_running_app_with(|| open_event(QUIT_EVENT), || open_mutex(SINGLE_INSTANCE_MUTEX));
}

pub fn close_running_app_with(open_quit: impl FnOnce() -> Opened, open_single: impl FnOnce() -> Opened) {
    let Some(quit) = guarded(open_quit) else { return };
    quit.set();
    drop(quit);
    let Some(single) = guarded(open_single) else { return };
    // Abandoned: the app ended without releasing it; it is gone either way, and the mutex is now ours.
    let (got, _abandoned) = single.wait(10_000);
    if got {
        single.release_mutex();
    }
    drop(single);
    // Its browser processes end right after it.
    std::thread::sleep(Duration::from_millis(1500));
}

/// Velopack's uninstall run without Bloxkeep's uninstaller window: asks whether the accounts go too; kept when
/// unanswered by `deadline`, before Velopack's 30 s for this step run out.
fn ask_to_delete_data(deadline: Instant) -> bool {
    prefs::load();
    vaults::load();
    if prefs::lock().vault_list.is_empty() && !vault::vault_path().exists() {
        return false;
    }
    let left = deadline.saturating_duration_since(Instant::now()).as_millis().clamp(5000, 20000) as u64;
    let data_dir = vault::data_dir().display().to_string();
    let picked = dialog::show(
        None,
        &l::t("Bloxkeep is being removed from this PC. Your accounts and settings stay unless you delete them."),
        Some(&l::t("Delete your accounts too?")),
        dialog::Kind::Warning,
        &[
            dialog::Choice::with_detail(l::t("Keep my accounts and settings"), l::t("Installing Bloxkeep again picks up where you left off.")),
            dialog::Choice {
                danger: true,
                ..dialog::Choice::with_detail(
                    l::t("Delete my accounts and settings"),
                    l::f("Deletes the vault with your saved accounts, your settings and logs in {0}. This can't be undone.", &[&data_dir]),
                )
            },
        ],
        0,
        Some(0),
        Some(&l::t("Kept if you don't choose within 20 seconds.")),
        Some(Duration::from_millis(left)),
    );
    picked == 1
}

/// Installed apps › Uninstall opens BloxkeepUninstall.exe rather than Velopack's bare one. Set at every start because an
/// update rewrites the entry; nothing to do for the portable copy or a development build.
fn use_our_uninstaller() {
    let Ok(exe) = std::env::current_exe() else { return };
    let Some(app_dir) = exe.parent() else { return };
    let uninstaller = app_dir.join("BloxkeepUninstall.exe");
    let Some(root) = app_dir.parent() else { return };
    if !uninstaller.exists() || !root.join("Update.exe").exists() {
        return;
    }
    let key = match RegKey::open(Hive::CurrentUser, r"Software\Microsoft\Windows\CurrentVersion\Uninstall\BloxkeepApp", true) {
        Ok(Some(key)) => key,
        Ok(None) => return,
        Err(e) => {
            log::write(&e);
            return;
        }
    };
    let Some(at) = key.get_string("InstallLocation") else { return };
    let at = std::path::Path::new(at.trim_end_matches(['\\', '/']));
    let at = std::path::absolute(at).unwrap_or_else(|_| at.to_path_buf());
    if !at.to_string_lossy().eq_ignore_ascii_case(&root.to_string_lossy()) {
        return;
    }
    // A quiet fix shows in Installed apps as the version it fixes (Velopack writes the full number at each update).
    // PORT: the C# read its AssemblyInformationalVersion; here the crate's version, or BLOXKEEP_VERSION when the build sets it.
    let version = option_env!("BLOXKEEP_VERSION").unwrap_or(env!("CARGO_PKG_VERSION"));
    let version = version.split('+').next().unwrap_or(version);
    if whats_new::is_quiet_fix(Some(version)) {
        let shown = whats_new::shown(version);
        if key.get_string("DisplayVersion").as_deref() != Some(shown.as_str()) {
            if let Err(e) = key.set_string("DisplayVersion", &shown) {
                log::write(&e);
            }
        }
    }
    let command = format!("\"{}\"", uninstaller.display());
    if key.get_string("UninstallString").as_deref() == Some(command.as_str()) {
        return;
    }
    if let Err(e) = key.set_string("UninstallString", &command).and_then(|_| key.set_string("QuietUninstallString", &format!("{command} --silent"))) {
        log::write(&e);
    }
}

static LAST_ERROR_BOX: Mutex<Option<Instant>> = Mutex::new(None);
/// The error box last shown: when the window's own thread fails, the process waits for it to be closed before it ends
/// (else the box would go with the process before anyone saw it).
static ERROR_BOX: Mutex<Option<std::thread::JoinHandle<()>>> = Mutex::new(None);
static INPUT_LANGUAGE_LOGGED: AtomicBool = AtomicBool::new(false);

/// Logs the error; shows it at most every few seconds so a repeating failure can't flood the screen.
thread_local! {
    /// How deep this thread is in catch_unwind's below: a panic there is caught and handled, not unexpected.
    static CAUGHT: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// std::panic::catch_unwind for a panic the code catches and goes on from (the C#'s try/catch): the hook only logs it.
/// The error box was for what nothing caught (Application.ThreadException), and it came up for these too.
pub fn catch_unwind<F: FnOnce() -> R + std::panic::UnwindSafe, R>(f: F) -> std::thread::Result<R> {
    CAUGHT.with(|c| c.set(c.get() + 1));
    let r = std::panic::catch_unwind(f);
    CAUGHT.with(|c| c.set(c.get() - 1));
    r
}

/// For a call from Windows (a window procedure, a hook, a posted closure, a timer): a panic crossing into Windows aborts
/// the whole process. Here it is shown like any unexpected error (the hook) and the call answers `fallback`; the app goes
/// on, as the C#'s Application.ThreadException let it.
pub fn shielded<R>(fallback: impl FnOnce() -> R, f: impl FnOnce() -> R) -> R {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or_else(|_| fallback())
}

fn on_unexpected(message: &str) {
    // PORT: the C# skipped the CultureNotFoundException WinForms raised on a keyboard-language switch (Alt+Shift) under
    // InvariantGlobalization; there is no WinForms InputLanguage here, so the case can't happen. The flag is kept for a
    // panic that names it, logged once.
    if message.contains("InputLanguage") {
        if !INPUT_LANGUAGE_LOGGED.swap(true, Ordering::SeqCst) {
            log::write(&message);
        }
        return;
    }
    log::write(&message);
    {
        let mut last = LAST_ERROR_BOX.lock().unwrap_or_else(|e| e.into_inner());
        if last.is_some_and(|t| t.elapsed() < Duration::from_secs(10)) {
            return;
        }
        *last = Some(Instant::now());
    }
    // The box on its own thread: a panic on the windows thread must not wait on that thread's own messages.
    let text = l::f("Unexpected error: {0}", &[&message]);
    if let Ok(h) = crate::net::Named::new("error-box".into()).spawn(move || ui::error(None, &text)) {
        *ERROR_BOX.lock().unwrap_or_else(|e| e.into_inner()) = Some(h);
    }
}
