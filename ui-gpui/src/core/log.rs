//! Unexpected errors go to error.log in the data folder instead of crashing the app.
//! Only exception details are written — never account data. At 1 MB the log moves to error.log.1 and the one there to
//! error.log.2 (the one there goes), so a bug report still has days of lines from before (3 MB in all).
//!
//! The same lines, and what Bloxkeep does for each account (activity.rs), are also kept in memory for Statistics ›
//! Console: the last CONSOLE_MOST of them, a line said again folded into the one before it ("×12").
use std::{
    collections::VecDeque,
    io::Write,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

use regex::Regex;
use serde::Serialize;

/// error.log's size before it moves to error.log.1: 1 MB each, about a week of a real log (256 KB held about a day and
/// a half, 2026-11-17). Only appended to and its size looked at while the app runs; a bug report sends the end
/// (FEEDBACK_LOG_MAX), so a bigger file costs nothing there.
pub const MAX_SIZE: u64 = 1024 * 1024;
static SYNC: Mutex<()> = Mutex::new(());

/// Log.Write(Exception): the error's words (the C# wrote the exception's type and stack too; a Fail has its sentence).
pub fn write(ex: &dyn std::fmt::Display) {
    let text = ex.to_string();
    console("error", "", &text);
    append(&text);
}

/// A short diagnostic line (e.g. which kind of check Roblox asked for). Callers pass no account data.
pub fn note(text: &str) {
    console("note", "", text);
    append(text);
}

/// Into error.log only, not the Console: a block of Roblox's own log lines (App::note_roblox_lines).
pub fn note_file(text: &str) {
    append(text);
}

/// A last guard on what is written: a session cookie or a Discord webhook link that reached a message (an exception's
/// text can hold a request's address or a header) never lands in a file that is sent with a bug report and shown on request.
static SECRETS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)_\|WARNING:-DO-NOT-SHARE-THIS[^\s"']*|https?://(?:[a-z]+\.)?discord(?:app)?\.com/api/webhooks/\S+"#).expect("secrets pattern")
});

/// The line written last, and how many times it came again since (not written): a check failing every few seconds
/// filled the 256 KB with one sentence and pushed out what came before it. Written as one line, "(… N more times)",
/// when another line comes, or after REPEAT_SPAN.
struct Repeat {
    text: String,
    times: u32,
    since: Instant,
}
static LAST: Mutex<Option<Repeat>> = Mutex::new(None);
const REPEAT_SPAN: Duration = Duration::from_secs(600);

fn append(text: &str) {
    let text = SECRETS.replace_all(text, "[secret]").into_owned();
    let _held = SYNC.lock().unwrap_or_else(|e| e.into_inner());
    {
        let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(r) = last.as_mut()
            && r.text == text
            && r.since.elapsed() < REPEAT_SPAN
        {
            r.times += 1;
            return;
        }
        if let Some(r) = last.take()
            && r.times > 0
        {
            put(&format!("(the line above came {} more times in {})", r.times, span(r.since.elapsed())));
        }
        *last = Some(Repeat { text: text.clone(), times: 0, since: Instant::now() });
    }
    put(&text);
}

fn span(d: Duration) -> String {
    match d.as_secs() {
        s if s < 90 => format!("{s} s"),
        s => format!("{} min", s.div_ceil(60)),
    }
}

/// One line into error.log, with the time.
fn put(text: &str) {
    // An IOException or UnauthorizedAccessException here: nowhere left to report it.
    let _ = (|| -> std::io::Result<()> {
        let dir = crate::core::vault::data_dir();
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("error.log");
        // At the limit the log moves down one: error.log.1 becomes error.log.2 (the one before that goes) and the log
        // becomes error.log.1, so a report sent days after the trouble still has its lines.
        if let Ok(meta) = std::fs::metadata(&path) {
            // Not a reason to stop logging: a file held open (read, scanned) makes a rename fail, and the line is still
            // written; if the log can't be moved aside it starts over, so it can't grow without end either.
            if meta.len() > MAX_SIZE {
                let (older, oldest) = (dir.join("error.log.1"), dir.join("error.log.2"));
                if older.is_file() {
                    let _ = std::fs::rename(&older, &oldest);
                }
                if std::fs::rename(&path, &older).is_err() {
                    let _ = std::fs::File::create(&path);
                }
            }
        }
        let c = super::base::Stamp::now().local();
        let line = format!("[{:04}-{:02}-{:02} {:02}:{:02}:{:02}] {}\r\n\r\n", c.year, c.month, c.day, c.hour, c.minute, c.second, text);
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
        file.write_all(line.as_bytes())
    })();
}

// ── Each run ────────────────────────────────────────────────────────────────────────────────────────────────────

/// Left in the data folder while Bloxkeep runs and taken away when it closes: still there at the next start means the
/// run before ended without closing (a crash, End task, the PC turned off).
fn running_mark() -> std::path::PathBuf {
    crate::core::vault::data_dir().join("running.mark")
}

/// The first line of a run: which Bloxkeep on which Windows, so a report's lines can be told apart by run; and the
/// run before it, when it didn't close.
pub fn run_started(version: &str, windows_build: u32, language: &str, after_update: bool) {
    let mark = running_mark();
    if let Ok(before) = std::fs::read_to_string(&mark)
        && !after_update
    {
        note(&format!(
            "The last run ({}) ended without closing: a crash, End task in Task Manager, or the PC turned off or restarted.",
            before.trim()
        ));
    }
    if let Some(d) = mark.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(&mark, format!("Bloxkeep {version}, started {}", now_words()));
    note(&format!("Bloxkeep {version} started (Windows build {windows_build}, {language})."));
}

/// The run closed as it should: the mark goes, and a line said again is written out.
pub fn run_ended() {
    let _held = SYNC.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(r) = LAST.lock().unwrap_or_else(|e| e.into_inner()).take()
        && r.times > 0
    {
        put(&format!("(the line above came {} more times in {})", r.times, span(r.since.elapsed())));
    }
    let _ = std::fs::remove_file(running_mark());
}

fn now_words() -> String {
    let c = super::base::Stamp::now().local();
    format!("{:04}-{:02}-{:02} {:02}:{:02}", c.year, c.month, c.day, c.hour, c.minute)
}

/// Windows' own crash (an access violation, a stack overflow, a fault inside a driver's code): a Rust panic is caught
/// by the panic hook, but these end the process at once and left nothing at all. Writes the code and the address
/// before Windows ends it.
#[cfg(windows)]
pub fn catch_crashes() {
    use windows_sys::Win32::System::Diagnostics::Debug::{EXCEPTION_POINTERS, SetUnhandledExceptionFilter};
    unsafe extern "system" fn on_crash(info: *const EXCEPTION_POINTERS) -> i32 {
        // SAFETY: Windows passes a valid record for the exception being handled.
        let (code, at) = unsafe {
            match info.as_ref().and_then(|i| i.ExceptionRecord.as_ref()) {
                Some(r) => (r.ExceptionCode as u32, r.ExceptionAddress as usize),
                None => (0, 0),
            }
        };
        let what = match code {
            0xC000_0005 => "an access violation",
            0xC000_00FD => "a stack overflow",
            0xC000_0409 => "a fail-fast (stack buffer overrun or abort)",
            0xC000_001D => "an illegal instruction",
            0xC000_0374 => "heap corruption",
            0xC000_0094 => "an integer divided by zero",
            _ => "a crash",
        };
        let thread = std::thread::current().name().unwrap_or("unnamed").to_string();
        let base = module_base();
        put(&format!(
            "Bloxkeep crashed: {what} (0x{code:08X}) on thread \"{thread}\" at 0x{at:X} (Bloxkeep.exe +0x{:X}).",
            at.wrapping_sub(base)
        ));
        // A CPU limit outlives Bloxkeep and no later one can reach it: switched off now, while the jobs are in reach.
        super::cpu_cap::lift_all_at_crash();
        0 // EXCEPTION_CONTINUE_SEARCH: Windows ends the process as it would have, and Windows Error Reporting still sees it
    }
    // SAFETY: a plain call; the filter lives as long as the process.
    unsafe {
        SetUnhandledExceptionFilter(Some(on_crash));
    }
}

/// Where the code is now, as the calls that led here: each one's place in Bloxkeep.exe (" stack: +0x1A2B3C +0x…"),
/// the nearest first. A panic's own words name only the line that gave up (GPUI's, say), not Bloxkeep's call that led
/// to it: a start that crashed every time (2026-11-18) could not be traced from error.log. The numbers are read back
/// with the build's bloxkeep_ui.pdb (kept beside each test build).
#[cfg(windows)]
pub fn stack_here() -> String {
    use windows_sys::Win32::System::Diagnostics::Debug::RtlCaptureStackBackTrace;
    let mut frames = [std::ptr::null_mut::<core::ffi::c_void>(); 48];
    // SAFETY: fills at most 48 entries of this array; the first two frames (this function, the hook) are skipped.
    let n = unsafe { RtlCaptureStackBackTrace(2, frames.len() as u32, frames.as_mut_ptr(), std::ptr::null_mut()) } as usize;
    let base = module_base();
    let mut out = String::from(" stack:");
    for at in frames.iter().take(n).map(|f| *f as usize) {
        // (Only Bloxkeep's own code: Windows' frames between say nothing without Windows' symbols.)
        if at >= base && at - base < 0x2000_0000 {
            out.push_str(&format!(" +0x{:X}", at - base));
        }
    }
    out
}

#[cfg(not(windows))]
pub fn stack_here() -> String {
    String::new()
}

#[cfg(windows)]
fn module_base() -> usize {
    // SAFETY: a null name asks for this process's own exe, which stays loaded.
    unsafe { windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null()) as usize }
}

// ── The window stopping ─────────────────────────────────────────────────────────────────────────────────────────

static BEAT: Mutex<Option<Instant>> = Mutex::new(None);

/// Called by the window every half second (shell.rs): it is still answering.
pub fn window_beat() {
    *BEAT.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
}

/// A thread of its own, for as long as the app runs, that notes when the window stops answering for 5 s or more, and for how long. A watch that
/// itself woke late (the PC slept, or was too busy to run it) says nothing: that pause wasn't Bloxkeep's.
pub fn watch_window() {
    let _ = crate::threads::own("window-watch", || {
        let mut stuck_since: Option<Instant> = None;
        let mut woke = Instant::now();
        loop {
            std::thread::sleep(Duration::from_secs(1));
            let late = woke.elapsed() > Duration::from_secs(3);
            woke = Instant::now();
            let Some(beat) = *BEAT.lock().unwrap_or_else(|e| e.into_inner()) else { continue };
            if late {
                stuck_since = None;
                continue;
            }
            let quiet = beat.elapsed();
            match stuck_since {
                None if quiet >= Duration::from_secs(5) => {
                    stuck_since = Some(beat);
                    note("The window stopped answering (5 s so far).");
                }
                Some(since) if quiet < Duration::from_secs(2) => {
                    note(&format!("The window answered again after {}.", span(since.elapsed().saturating_sub(quiet))));
                    stuck_since = None;
                }
                _ => {}
            }
        }
    });
}

// ── Console ─────────────────────────────────────────────────────────────────────────────────────────────────────

/// How many lines the Console keeps.
pub const CONSOLE_MOST: usize = 1500;
/// A line the same as one of the last few (same kind, source, account and words) is folded into it.
const FOLD_WITHIN: usize = 40;

#[derive(Clone, Debug, Serialize)]
pub struct Line {
    pub id: u64,
    /// Unix seconds: the first time, and the last time when said again.
    pub at: i64,
    pub last: i64,
    /// "error", "note", or an activity's kind ("launched", "disconnected", "rejoined"…).
    pub kind: String,
    /// Who said it: the part before the first ": " of a short note ("Rejoin", "Bot check"), else "Bloxkeep".
    pub source: String,
    /// The account's name, for an activity line (never written to a file).
    pub account: String,
    /// How it reads: "bad" (an error), "warn" (a drop, signed out), "ok", "info", "note".
    pub tone: String,
    pub text: String,
    pub times: u32,
}

struct Console {
    lines: VecDeque<Line>,
    next: u64,
    /// Changes with every line: the page asks again only when it moved.
    version: u64,
}

static CONSOLE: Mutex<Console> = Mutex::new(Console { lines: VecDeque::new(), next: 1, version: 0 });

/// A line for the Console (in memory only): Bloxkeep's own, its source taken from the words ("Rejoin: …").
pub fn console(kind: &str, account: &str, text: &str) {
    let tone = if kind == "error" { "bad" } else { "note" };
    console_line(kind, None, tone, account, text);
}

/// A line for the Console with its source and tone given (an account's activity: its group, "Games", "Disconnects").
pub fn console_line(kind: &str, source: Option<&str>, tone: &str, account: &str, text: &str) {
    let text = SECRETS.replace_all(text.trim(), "[secret]").into_owned();
    if text.is_empty() {
        return;
    }
    let (source, words) = match source {
        Some(s) => (s.to_string(), text),
        None => split_source(&text),
    };
    let at = super::base::Stamp::utc_now().unix_secs();
    let mut c = CONSOLE.lock().unwrap_or_else(|e| e.into_inner());
    c.version += 1;
    let len = c.lines.len();
    if let Some(i) = (len.saturating_sub(FOLD_WITHIN)..len)
        .rev()
        .find(|&i| c.lines[i].kind == kind && c.lines[i].account == account && c.lines[i].text == words && c.lines[i].source == source)
    {
        // Said again: counted on it, and it moves to the end (the newest), keeping its id.
        let mut line = c.lines.remove(i).expect("index in range");
        line.times += 1;
        line.last = at;
        c.lines.push_back(line);
        return;
    }
    let id = c.next;
    c.next += 1;
    c.lines.push_back(Line { id, at, last: at, kind: kind.into(), source, account: account.into(), tone: tone.into(), text: words, times: 1 });
    while c.lines.len() > CONSOLE_MOST {
        c.lines.pop_front();
    }
}

fn split_source(text: &str) -> (String, String) {
    if let Some((head, rest)) = text.split_once(": ")
        && !head.is_empty()
        && head.len() <= 24
        && head.split(' ').count() <= 3
        && head.chars().next().is_some_and(char::is_uppercase)
        && !head.contains(['(', '[', '"', '\''])
        && !rest.trim().is_empty()
    {
        return (head.to_string(), rest.trim().to_string());
    }
    ("Bloxkeep".into(), text.to_string())
}

/// The Console's lines when they changed since `version` (None: nothing new), with the version now.
pub fn console_lines(version: u64) -> Option<(u64, Vec<Line>)> {
    let c = CONSOLE.lock().unwrap_or_else(|e| e.into_inner());
    (c.version != version).then(|| (c.version, c.lines.iter().cloned().collect()))
}

/// Clear on the Console: the lines go (error.log stays as it is).
pub fn console_clear() {
    let mut c = CONSOLE.lock().unwrap_or_else(|e| e.into_inner());
    c.lines.clear();
    c.version += 1;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_folds_and_splits() {
        console_clear();
        console("note", "", "Rejoin: account 3's window ended, not rejoined (closed).");
        console("note", "", "Rejoin: account 3's window ended, not rejoined (closed).");
        console("disconnected", "Alt 2", "Lost connection (277)");
        console("note", "", "Rejoin: account 3's window ended, not rejoined (closed).");
        console("error", "", "something odd: (os error 5)");
        let (_, lines) = console_lines(u64::MAX).unwrap();
        assert_eq!(lines.len(), 3);
        let rejoin = lines.iter().find(|l| l.source == "Rejoin").unwrap();
        assert_eq!(rejoin.times, 3);
        assert_eq!(rejoin.text, "account 3's window ended, not rejoined (closed).");
        // Said again moves it to the end; a lower-case head isn't a source.
        assert_eq!(lines[1].source, "Rejoin");
        assert_eq!(lines[2].source, "Bloxkeep");
        assert_eq!(lines[0].account, "Alt 2");
        console_clear();
    }

    // The checks below run in a child of the test (its data folder a temp one, set before anything reads it), one
    // test each, with no window: `cargo test --features core -- --ignored log::tests::real_`.

    /// Runs this test binary again for one ignored test, its data folder `dir`; the child's error.log after.
    fn child(test: &str, dir: &std::path::Path, extra: &[(&str, &str)]) -> (bool, String) {
        let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
        cmd.args(["--ignored", "--exact", "--nocapture", test]).env("BLOXKEEP_DATA", dir).env("BLOXKEEP_LOG_CHILD", "1");
        for (k, v) in extra {
            cmd.env(k, v);
        }
        let ok = cmd.output().map(|o| o.status.success()).unwrap_or(false);
        (ok, std::fs::read_to_string(dir.join("error.log")).unwrap_or_default())
    }

    fn temp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("bloxkeep-log-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    #[ignore]
    fn child_starts() {
        if std::env::var("BLOXKEEP_LOG_CHILD").is_err() {
            return;
        }
        run_started("9.9.9", 26300, "en", false);
        if std::env::var("BLOXKEEP_LOG_CLOSE").is_ok() {
            run_ended();
        }
    }

    #[test]
    #[ignore]
    fn real_unclean_exit() {
        let dir = temp("unclean");
        // A run that didn't close, then the next start says so; a run that closed leaves nothing to say.
        child("core::log::tests::child_starts", &dir, &[]);
        let (_, log) = child("core::log::tests::child_starts", &dir, &[("BLOXKEEP_LOG_CLOSE", "1")]);
        assert!(log.contains("ended without closing"), "{log}");
        assert!(!dir.join("running.mark").exists());
        let (_, log) = child("core::log::tests::child_starts", &dir, &[]);
        assert_eq!(log.matches("ended without closing").count(), 1, "{log}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    #[ignore]
    fn child_crashes() {
        if std::env::var("BLOXKEEP_LOG_CHILD").is_err() {
            return;
        }
        use windows_sys::Win32::System::Diagnostics::Debug::{SEM_NOGPFAULTERRORBOX, SetErrorMode};
        // No "has stopped working" box on the screen.
        unsafe { SetErrorMode(SEM_NOGPFAULTERRORBOX) };
        catch_crashes();
        let bad = std::hint::black_box(8usize) as *mut u8;
        // SAFETY: none; the point is the access violation.
        unsafe { std::ptr::write_volatile(bad, 1) };
    }

    #[test]
    #[ignore]
    fn real_crash_is_written() {
        let dir = temp("crash");
        let (ok, log) = child("core::log::tests::child_crashes", &dir, &[]);
        assert!(!ok);
        assert!(log.contains("Bloxkeep crashed: an access violation (0xC0000005)"), "{log}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    #[ignore]
    fn child_freezes() {
        if std::env::var("BLOXKEEP_LOG_CHILD").is_err() {
            return;
        }
        watch_window();
        window_beat();
        std::thread::sleep(Duration::from_secs(7));
        for _ in 0..6 {
            window_beat();
            std::thread::sleep(Duration::from_millis(500));
        }
    }

    #[test]
    #[ignore]
    fn real_freeze_is_written() {
        let dir = temp("freeze");
        let (ok, log) = child("core::log::tests::child_freezes", &dir, &[]);
        assert!(ok);
        assert!(log.contains("The window stopped answering (5 s so far)."), "{log}");
        assert!(log.contains("The window answered again after 7 s."), "{log}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
