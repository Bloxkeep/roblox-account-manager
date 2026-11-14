// RobloxLauncher.cs: opens Roblox for an account with its sign-in ticket, through the option picked in Settings.
use std::io::BufRead;
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::{LazyLock, Mutex};

use regex::Regex;

use crate::core::base::{Fail, Res};
use crate::core::game_target::GameTarget;
use crate::core::roblox_api::{eq_ci, escape_data_string, fail_of};
use crate::core::roblox_installs::{self as installs, combine, dir_name, file_name, has_invalid_file_name_chars};
use crate::core::{l, log};

/// Values of AppSettings.LaunchVia, plus "other:<Name>" for another Bloxstrap-style launcher found here ("custom" is
/// AppSettings.CustomLauncher).
pub const LAUNCH_OPTIONS: [&str; 9] = ["auto", "bloxstrap", "fishstrap", "voidstrap", "roblox", "direct", "store", "custom", "vng"];

pub(crate) const OTHER_PREFIX: &str = "other:";

pub fn other(name: &str) -> String {
    format!("{OTHER_PREFIX}{name}")
}

/// The launcher's name for an "other:<Name>" option, else None.
pub fn other_name(launch_via: &str) -> Option<String> {
    launch_via.strip_prefix(OTHER_PREFIX).filter(|n| !n.is_empty()).map(str::to_string)
}

pub fn is_valid_option(launch_via: &str) -> bool {
    LAUNCH_OPTIONS.contains(&launch_via) || other_name(launch_via).is_some_and(|name| !has_invalid_file_name_chars(&name))
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Path.IsPathFullyQualified on Windows: "C:\…" / "C:/…" or a "\\…" / "//…" path.
fn is_path_fully_qualified(path: &str) -> bool {
    let b = path.as_bytes();
    if b.len() >= 2 && (b[0] == b'\\' || b[0] == b'/') {
        return b[1] == b'\\' || b[1] == b'/' || b[1] == b'?';
    }
    b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')
}

/// A hand-picked launcher on this PC, not the network (\\server\share, \\?\UNC\…, \\host@SSL\…, a mapped drive):
/// merely checking a network path hands the PC's Windows sign-in to that server, and then runs what it serves.
pub fn is_local_program(path: &str) -> bool {
    if path.is_empty() || !is_path_fully_qualified(path) || path.starts_with(r"\\") || path.starts_with("//") {
        return false;
    }
    // Path.GetPathRoot of "C:\…": "C:\".
    let Some(drive) = path.get(..2) else { return false };
    let root = format!("{drive}\\");
    let root = wide(&root);
    // SAFETY: `root` is NUL-terminated.
    let kind = unsafe { windows_sys::Win32::Storage::FileSystem::GetDriveTypeW(root.as_ptr()) };
    // DRIVE_UNKNOWN 0, DRIVE_NO_ROOT_DIR 1, DRIVE_REMOTE 4.
    !matches!(kind, 0 | 1 | 4)
}

/// The launcher picked by hand is a program on this PC (`is_local_program`) and still there.
pub fn custom_launcher_ready(path: &str) -> bool {
    is_local_program(path) && installs::file_exists(path)
}

pub fn is_protocol_registered() -> bool {
    installs::registry::classes_root(r"roblox-player\shell\open\command").is_some()
}

/// Default may hand the link to Windows: a handler is registered, still there, and not Roblox VNG's client
/// (see is_vng_client).
fn links_open() -> bool {
    is_protocol_registered()
        && match installs::protocol_handler_path() {
            None => true,
            Some(handler) => installs::file_exists(&handler) && !installs::is_vng_client(&handler),
        }
}

/// What opens Roblox when the links don't: Bloxstrap, Fishstrap or Voidstrap, another Bloxstrap-style launcher, the newest
/// RobloxPlayerBeta.exe, and VNG's client only when nothing else is there (the owner's rule, 2026-10-28).
pub(crate) fn fallback_launcher() -> Option<(String, Vec<String>)> {
    for name in ["Bloxstrap", "Fishstrap", "Voidstrap"] {
        if let Some(exe) = installs::find_bootstrapper(name) {
            return Some((exe, vec!["-player".into()]));
        }
    }
    for name in installs::find_other_bootstrappers() {
        if let Some(exe) = installs::find_bootstrapper(&name) {
            return Some((exe, Vec::new()));
        }
    }
    installs::find_any_client().or_else(installs::find_vng_client).map(|client| (client, Vec::new()))
}

/// A random number from Random.Shared's place: the bytes of a new v4 Guid.
// PORT: no rand crate; uuid's v4 (the OS's random source) stands in for Random.Shared.
pub(crate) fn random_u64() -> u64 {
    let b = *uuid::Uuid::new_v4().as_bytes();
    u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[9], b[10]])
}

fn new_browser_tracker_id() -> String {
    (100_000_000_000u64 + random_u64() % (999_999_999_999u64 - 100_000_000_000u64)).to_string()
}

fn place_launcher_url(t: &GameTarget, browser_tracker_id: &str) -> String {
    let common = format!("browserTrackerId={browser_tracker_id}&placeId={}&isPlayTogetherGame=false&joinAttemptId={}", t.place_id, uuid::Uuid::new_v4());
    const BASE_URL: &str = "https://www.roblox.com/Game/PlaceLauncher.ashx?";
    // One of the account's own private servers: the access code from its list, as the website's Join button sends it
    // (Roblox's PlaceLauncher.js RequestPrivateGame(placeId, accessCode); Roblox Account Manager joins the same way).
    if let Some(access) = &t.access_code {
        return format!(
            "{BASE_URL}request=RequestPrivateGame&{common}&accessCode={}{}&joinAttemptOrigin=privateServerListJoin",
            escape_data_string(access),
            t.link_code.as_deref().map(|lc| format!("&linkCode={}", escape_data_string(lc))).unwrap_or_default()
        );
    }
    if let Some(lc) = &t.link_code {
        return format!("{BASE_URL}request=RequestPrivateGame&{common}&linkCode={}&joinAttemptOrigin=privateServerListJoin", escape_data_string(lc));
    }
    if let Some(job) = &t.job_id {
        return format!("{BASE_URL}request=RequestGameJob&{common}&gameId={job}&joinAttemptOrigin=publicServerListJoin");
    }
    // A link's launch data goes with a plain join, before isPlayTogetherGame as the website puts it.
    if let Some(data) = t.launch_data.as_deref().filter(|d| !d.is_empty()) {
        let common = format!(
            "browserTrackerId={browser_tracker_id}&placeId={}&launchData={}&isPlayTogetherGame=false&joinAttemptId={}",
            t.place_id,
            escape_data_string(data),
            uuid::Uuid::new_v4()
        );
        return format!("{BASE_URL}request=RequestGame&{common}&joinAttemptOrigin=PlayButton");
    }
    format!("{BASE_URL}request=RequestGame&{common}&joinAttemptOrigin=PlayButton")
}

/// The same roblox-player: URI the website hands to the client when you press Play.
/// `tracker_id`: its browser tracker id (a new one when not given), which the client's command line carries.
pub fn build_launch_uri(ticket: &str, target: Option<&GameTarget>, tracker_id: Option<&str>) -> String {
    let tracker_id = tracker_id.map(str::to_string).unwrap_or_else(new_browser_tracker_id);
    let now = crate::core::base::Stamp::utc_now().unix_ms();
    let tail = format!("+browsertrackerid:{tracker_id}+robloxLocale:en_us+gameLocale:en_us+channel:+LaunchExp:InApp");
    let Some(target) = target else {
        return format!("roblox-player:1+launchmode:app+gameinfo:{ticket}+launchtime:{now}{tail}");
    };
    let place_launcher = escape_data_string(&place_launcher_url(target, &tracker_id));
    format!("roblox-player:1+launchmode:play+gameinfo:{ticket}+launchtime:{now}+placelauncherurl:{place_launcher}{tail}")
}

fn missing_file() -> String {
    l::t("Roblox is missing a file (RobloxPlayerBeta.dll). Install Roblox again from roblox.com/download.")
}

/// The C#'s FileNotFoundException, whose sentence is shown as it is.
// PORT: FileNotFoundException carried a sentence for the person; it is Fail::User here.
fn not_found(text: String) -> Fail {
    Fail::user(text)
}

/// `launch_via`: "auto" = whatever Windows opens roblox-player: links with; "bloxstrap"/"fishstrap"/"voidstrap" = that
/// launcher, keeping its mods; "roblox" = the official client; "direct" = the newest RobloxPlayerBeta.exe;
/// "other:<Name>" and "custom" get the link as their first argument, as Windows would.
/// Returns the link's browser tracker id, new per launch: it is on the client's command line, so its window can be
/// told apart (roblox_processes::launch_tracker_of).
/// `client`: a client to start as it is, past the choice: the build the windows open run on (roblox_installs::kept_copy).
pub fn launch(ticket: &str, target: Option<&GameTarget>, launch_via: &str, custom_launcher: &str, client: Option<&str>) -> Res<String> {
    let tracker = new_browser_tracker_id();
    let uri = build_launch_uri(ticket, target, Some(&tracker));
    if let Some(client) = client {
        start(client, &[&uri])?;
        return Ok(tracker);
    }
    let mut launch_via = launch_via.to_string();
    // Windows would only show its own error for a client missing its DLL: another build opens instead, or this is said.
    if matches!(launch_via.as_str(), "auto" | "") && installs::handler_broken() {
        if installs::find_any_client().is_none() {
            return Err(not_found(missing_file()));
        }
        log::note("Roblox start: the link handler's Roblox is missing RobloxPlayerBeta.dll; opened directly instead.");
        launch_via = "direct".into();
    } else if matches!(launch_via.as_str(), "roblox" | "direct")
        && installs::own_build_broken()
        && (if launch_via == "roblox" { installs::find_official_client() } else { installs::find_any_client().or_else(installs::find_vng_client) }).is_none()
    {
        return Err(not_found(missing_file()));
    }
    if let Some(other) = other_name(&launch_via) {
        let exe = installs::find_bootstrapper(&other).ok_or_else(|| not_found(l::f("{0} is not installed.", &[&other])))?;
        start(&exe, &[&uri])?;
        return Ok(tracker);
    }
    match launch_via.as_str() {
        "custom" => {
            // One on the network (a vault from elsewhere, say) is treated as gone: never looked at, never started.
            if !custom_launcher_ready(custom_launcher) {
                return Err(not_found(l::t("The launcher you picked is gone. Choose it again under Settings › Launching › Open Roblox with.")));
            }
            start(custom_launcher, &[&uri])?;
        }
        "bloxstrap" | "fishstrap" | "voidstrap" => {
            // Voidstrap: "-player" and the link as Bloxstrap takes them (its LaunchSettings.cs, 1.1.3.7).
            let name = match launch_via.as_str() {
                "bloxstrap" => "Bloxstrap",
                "fishstrap" => "Fishstrap",
                _ => "Voidstrap",
            };
            let exe = installs::find_bootstrapper(name).ok_or_else(|| not_found(l::f("{0} is not installed.", &[&name])))?;
            start(&exe, &["-player", &uri])?;
        }
        "roblox" => {
            let exe = installs::find_official_client().ok_or_else(|| not_found(l::t("The official Roblox client is not installed.")))?;
            start(&exe, &[&uri])?;
        }
        "direct" => {
            // Roblox VNG's client only when no other is on the PC: always the last choice.
            let exe = installs::find_any_client().or_else(installs::find_vng_client).ok_or_else(|| {
                not_found(l::t("RobloxPlayerBeta.exe was not found. Pick another option under Settings › Launching › Open Roblox with, or reinstall Roblox."))
            })?;
            start(&exe, &[&uri])?;
        }
        "vng" => {
            let exe = installs::find_vng_client()
                .ok_or_else(|| not_found(l::t("Roblox VNG isn't installed. Pick another option under Settings › Launching › Open Roblox with.")))?;
            start(&exe, &[&uri])?;
        }
        "store" => {
            let exe = installs::find_store_client().ok_or_else(|| {
                not_found(l::t("Roblox from the Microsoft Store isn't installed. Pick another option under Settings › Launching › Open Roblox with."))
            })?;
            start_in_store_package(&exe, &uri)?;
        }
        _ => {
            if !links_open() {
                // No handler, yet a client may remain: another Roblox's uninstaller takes the links with it (seen
                // 2026-10-19: VNG installed over Bloxstrap, then removed). Whatever is left opens it.
                if let Some((exe, args)) = fallback_launcher() {
                    log::note(&format!(
                        "Roblox start: no usable roblox-player link handler; opened with {} ({}) instead.",
                        file_name(&exe),
                        file_name(&dir_name(&exe).unwrap_or_default())
                    ));
                    let mut all: Vec<&str> = args.iter().map(String::as_str).collect();
                    all.push(&uri);
                    start(&exe, &all)?;
                    return Ok(tracker);
                }
                // A FileNotFoundException, like the other options' "not installed": its sentence is shown as it is.
                if let Some(vng) = installs::protocol_handler_path()
                    && installs::is_vng_client(&vng)
                {
                    return Err(not_found(l::t(
                        "Only Roblox VNG is installed. Choose Roblox VNG under Settings › Launching › Open Roblox with, or install Roblox from roblox.com/download.",
                    )));
                }
                return Err(not_found(if installs::store_edition_installed() {
                    l::t("Only Roblox from the Microsoft Store is installed. Choose it under Settings › Launching › Open Roblox with, or install Roblox from roblox.com/download.")
                } else {
                    l::t("Roblox isn't installed. Install it from roblox.com/download, then try again.")
                }));
            }
            // The links on another channel's build with Roblox's live one installed beside it: that one, as Windows would
            // start the links' (roblox_installs::LIVE_BUILD: the other would update and close every Roblox window open).
            if let Some(links) = installs::usable_handler_path().filter(|h| eq_ci(&file_name(h), "RobloxPlayerBeta.exe"))
                && let Some(live) = installs::live_beside(Some(links.clone())).filter(|live| !eq_ci(live, &links))
            {
                log::note(&format!(
                    "Roblox start: the links open build {}; started Roblox's live build {} instead.",
                    file_name(&dir_name(&links).unwrap_or_default()),
                    file_name(&dir_name(&live).unwrap_or_default())
                ));
                start(&live, &[&uri])?;
                return Ok(tracker);
            }
            note_links();
            // Not in Bloxkeep's folder (see Program.Main): the launcher may keep running as long as Roblox does.
            shell_open(&uri)?;
        }
    }
    Ok(tracker)
}

// ───────── Starts through the links ─────────
// A user's report (2026-11-14): Bloxkeep had started Roblox's live build itself for hours (the links on another build),
// then a start went through the links instead, and which program they opened wasn't in error.log. It is now, each time
// it changes: the handler's file and folder, the build its installer registered, and Roblox's live build.

/// What the links opened at the last start through them.
static LINKS_SEEN: Mutex<String> = Mutex::new(String::new());

/// Names only, never a path (it holds the Windows user's name).
fn note_links() {
    let handler = installs::protocol_handler_path().unwrap_or_default();
    let live = installs::live_build();
    let beside = match &live {
        None => "not known yet".to_string(),
        Some(live) if dir_name(&handler).is_some_and(|d| eq_ci(&file_name(&d), live)) => format!("{live}, the same"),
        Some(live) if installs::has_build(Some(&handler), Some(live)) => format!("{live}, installed beside it"),
        Some(live) => format!("{live}, not installed beside it (or broken)"),
    };
    let seen = format!(
        "Roblox start: through the links, which open {} ({}, registered build {}); Roblox's live build: {beside}.",
        file_name(&handler),
        file_name(&dir_name(&handler).unwrap_or_default()),
        installs::registered_build().unwrap_or_else(|| "none".into())
    );
    let mut last = LINKS_SEEN.lock().unwrap_or_else(|e| e.into_inner());
    if *last != seen {
        log::note(&seen);
        *last = seen;
    }
}

/// Process.Start with UseShellExecute: Windows opens the link with its handler, from the temp folder.
fn shell_open(uri: &str) -> Res<()> {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let verb = wide("open");
    let file = wide(uri);
    let dir = wide(&std::env::temp_dir().to_string_lossy());
    // SAFETY: all strings are NUL-terminated and outlive the call; no parent window.
    let r = unsafe { ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), file.as_ptr(), std::ptr::null(), dir.as_ptr(), SW_SHOWNORMAL) };
    // Above 32 is success. A failure as .NET's Win32Exception: Windows' own error number (GetLastError, 1155 when no
    // program opens the link), which the person is told ("Windows couldn't start Roblox (error 1155)"), never a raw text.
    if r as isize <= 32 {
        let last = std::io::Error::last_os_error();
        let code = match last.raw_os_error() {
            Some(code) if code != 0 => code,
            // ShellExecute's own numbers, as Windows' errors.
            _ => match r as isize {
                0 => 8,           // out of memory
                26 => 32,         // SE_ERR_SHARE: a sharing violation
                27 | 31 => 1155,  // SE_ERR_ASSOCINCOMPLETE, SE_ERR_NOASSOC: no program for the link
                28..=30 => 1156,  // SE_ERR_DDE*: DDE failed
                32 => 1157,       // SE_ERR_DLLNOTFOUND
                other => other as i32,
            },
        };
        return Err(fail_of("Win32Exception", std::io::Error::from_raw_os_error(code)));
    }
    Ok(())
}

fn start(exe: &str, args: &[&str]) -> Res<()> {
    let mut cmd = Command::new(exe);
    cmd.args(args).current_dir(dir_name(exe).unwrap_or_default());
    // The child is not waited for; dropping it leaves it running.
    cmd.spawn().map(drop).map_err(|e| fail_of("Win32Exception", e))
}

/// The ticket in a launch link, and any long token-like run (a wrapped line may cut "gameinfo:" off).
static TICKET_TEXT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"gameinfo:[^+\s'"]*|[A-Za-z0-9_\-]{40,}"#).unwrap());

/// The text with the sign-in ticket of any launch link in it hidden (see TICKET_TEXT).
pub fn hide_ticket(text: &str) -> String {
    TICKET_TEXT.replace_all(text, "<hidden>").into_owned()
}

/// Store Roblox: its Start entry's GameLaunchHelper only brings an open Roblox forward; started inside its
/// package (Invoke-CommandInDesktopPackage) the client keeps the package's sign-in and opens its own window (seen
/// 2026-10-16, Store Roblox 2.743).
fn start_in_store_package(exe: &str, uri: &str) -> Res<()> {
    fn quote(s: &str) -> String {
        format!("'{}'", s.replace('\'', "''"))
    }
    let system = system_directory();
    let mut cmd = Command::new(combine(&system, r"WindowsPowerShell\v1.0\powershell.exe"));
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        &format!(
            "Invoke-CommandInDesktopPackage -PackageFamilyName {} -AppId {} -Command {} -Args $env:BK_URI",
            quote(installs::STORE_FAMILY),
            quote(installs::STORE_APP_ID),
            quote(exe),
        ),
    ])
    // The link (sign-in ticket in it) goes in the child's environment, not its command line, which Windows can log
    // (process creation 4688, PowerShell script blocks 4104) and other programs can read.
    .env("BK_URI", uri)
    .current_dir(std::env::temp_dir())
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::piped())
    .creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let mut shell = cmd.spawn().map_err(|_| not_found(l::t("Couldn't start Roblox from the Microsoft Store.")))?;
    // Not waited for (a second or two): a failure goes to the log, and the window that doesn't come is reported as usual.
    let stderr = shell.stderr.take();
    std::thread::spawn(move || {
        let mut error = String::new();
        if let Some(stderr) = stderr {
            for line in std::io::BufReader::new(stderr).lines().map_while(Result::ok) {
                if !line.is_empty() && error.len() < 2000 {
                    error.push_str(&line);
                    error.push_str("\r\n");
                }
            }
        }
        let code = shell.wait().ok().and_then(|s| s.code()).unwrap_or(-1);
        // PowerShell's error repeats the command, sign-in ticket included: that part never reaches the log.
        if code != 0 || !error.is_empty() {
            log::note(&format!("Store Roblox start: exit {code} {}", hide_ticket(error.trim())));
        }
    });
    Ok(())
}

/// Environment.SystemDirectory.
fn system_directory() -> String {
    let mut buf = vec![0u16; 260];
    // SAFETY: `buf` holds 260 characters.
    let n = unsafe { windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW(buf.as_mut_ptr(), buf.len() as u32) };
    if n == 0 || n as usize >= buf.len() {
        return r"C:\Windows\System32".into();
    }
    String::from_utf16_lossy(&buf[..n as usize])
}
