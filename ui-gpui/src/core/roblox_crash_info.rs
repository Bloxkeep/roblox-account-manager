// RobloxCrashInfo.cs: what a Roblox window that ended on its own left behind, for error.log only: Roblox's crash reports
// (logs\crashes\reports\*.dmp) written since it opened, and Windows' "Application Error" record for its process. Never
// opens the process (Roblox's anti-cheat watches handles to it); read after it is gone.
use std::io::Read;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

use super::roblox_log_watcher;

/// The note for error.log, or None when nothing was left behind. `ran_for`: from its start to its end.
pub fn describe(pid: i32, ran_for: Duration) -> Option<String> {
    let mut parts = Vec::new();
    let since = SystemTime::now().checked_sub(ran_for).unwrap_or(SystemTime::UNIX_EPOCH);
    let dumps = dumps_since(since);
    if let Some(newest) = dumps.first() {
        parts.push(format!("{} Roblox crash report(s) since it opened, newest {newest}", dumps.len()));
    }
    if let Some(fault) = fault_of(pid, ran_for + Duration::from_secs(60)) {
        parts.push(format!("Windows: {fault}"));
    }
    (!parts.is_empty()).then(|| parts.join("; "))
}

/// Crash report names written since `since`, newest first.
pub(crate) fn dumps_since(since: SystemTime) -> Vec<String> {
    let mut found: Vec<(SystemTime, String)> = Vec::new();
    for logs in roblox_log_watcher::folders() {
        let Ok(dir) = std::fs::read_dir(Path::new(logs).join("crashes").join("reports")) else { continue };
        for e in dir.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.to_ascii_lowercase().ends_with(".dmp") {
                continue;
            }
            if let Ok(meta) = e.metadata()
                && meta.is_file()
                && let Ok(at) = meta.modified()
                && at >= since
            {
                found.push((at, name));
            }
        }
    }
    found.sort_by(|a, b| b.0.cmp(&a.0));
    found.into_iter().map(|f| f.1).collect()
}

/// Windows' Application Error record (event 1000) for this process id, written within `within`.
fn fault_of(pid: i32, within: Duration) -> Option<String> {
    let ms = within.as_millis().clamp(60_000, 86_400_000);
    let system = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    let mut run = Command::new(Path::new(&system).join("System32").join("wevtutil.exe"))
        .args([
            "qe",
            "Application",
            &format!("/q:*[System[Provider[@Name='Application Error'] and TimeCreated[timediff(@SystemTime) <= {ms}]]]"),
            "/f:xml",
            "/rd:true",
            "/c:50",
        ])
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut out = run.stdout.take()?;
    let (reader_tx, reader) = std::sync::mpsc::channel();
    crate::threads::run(move || {
        let mut bytes = Vec::new();
        let _ = out.read_to_end(&mut bytes);
        let _ = reader_tx.send(bytes);
    });
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        match run.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < until => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = run.kill();
                let _ = run.wait();
                return None;
            }
        }
    }
    let bytes = reader.recv_timeout(Duration::from_secs(10)).ok()?;
    // Redirected, it may write UTF-16 (every second byte 0) or the console's code page.
    let xml = if bytes.len() >= 2 && bytes[1] == 0 {
        String::from_utf16_lossy(&bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect::<Vec<_>>())
    } else {
        String::from_utf8_lossy(&bytes).into_owned()
    };
    fault_in(&xml, pid)
}

/// The matching record of wevtutil's XML, said in one line.
pub(crate) fn fault_in(xml: &str, pid: i32) -> Option<String> {
    for record in xml.split("<Event ").filter(|r| !r.is_empty()) {
        let Some(hex) = field(record, "ProcessId") else { continue };
        let Some(digits) = hex.strip_prefix("0x").or_else(|| hex.strip_prefix("0X")) else { continue };
        if u32::from_str_radix(digits, 16).ok().map(|n| n as i32) != Some(pid) {
            continue;
        }
        let or_ask = |name: &str| field(record, name).unwrap_or_else(|| "?".into());
        return Some(format!(
            "{} {} faulted in {} (exception {}, offset {})",
            or_ask("AppName"),
            field(record, "AppVersion").unwrap_or_default(),
            or_ask("ModuleName"),
            or_ask("ExceptionCode"),
            or_ask("FaultingOffset")
        ));
    }
    None
}

fn field(record: &str, name: &str) -> Option<String> {
    let start = format!("<Data Name='{name}'>");
    let rest = &record[record.find(&start)? + start.len()..];
    let end = rest.find('<')?;
    if !rest[end..].starts_with("</Data>") {
        return None;
    }
    let value = rest[..end].trim();
    (!value.is_empty()).then(|| value.to_string())
}
