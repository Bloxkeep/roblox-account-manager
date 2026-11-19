// MacroOutside.cs: what a macro reads from or writes to outside the game.
use std::io::Read;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use regex::Regex;

use crate::core::base::{Cancel, Fail, Res};
use crate::core::{l, log};

/// What a macro reads from or writes to outside the game: the clipboard, a text file in its own folder, a web
/// address. Its own interface, so the checks swap it for one that touches nothing.
///
/// Errors: a name or address the step may not use (C#'s MacroExprError) is `Fail::User`, said as it is and failing the
/// run ("badFormula"); anything else (a clipboard another app holds, a file open elsewhere) is `Fail::Other`, noted
/// while the macro goes on.
pub trait IMacroOutside: Send + Sync {
    fn clipboard(&self) -> Res<Option<String>>;
    fn copy(&self, text: &str) -> Res<()>;
    /// The file's last line that isn't empty; None when there's no such file.
    fn read_file(&self, name: &str) -> Res<Option<String>>;
    /// Adds a line at the end of the file (made when it isn't there).
    fn append_file(&self, name: &str, line: &str) -> Res<()>;
    /// GET (post None) or POST {"content": post}; the answer's text, or None when the address refused.
    fn web(&self, url: &str, post: Option<&str>, ct: &Cancel) -> Res<Option<String>>;
}

/// The real one. Files live in Documents\Bloxkeep only, named by the step ("coins log" → "coins log.txt"): a macro shared
/// as a file can't write anywhere else. Web addresses are https and public only (never this PC or the home network), with a
/// short timeout and a small answer kept.
#[derive(Clone, Copy, Debug, Default)]
pub struct MacroOutside;

pub const MAX_READ: usize = 2000;

/// Resolves a host to its first public address only (C#'s ConnectCallback, ConnectPublicAsync): ureq then connects
/// nowhere else.
struct PublicResolver;

impl ureq::Resolver for PublicResolver {
    /// Connects only to a public address: never this PC, the home network or a link-local one.
    fn resolve(&self, netloc: &str) -> std::io::Result<Vec<SocketAddr>> {
        let addresses: Vec<SocketAddr> = netloc.to_socket_addrs()?.collect();
        match addresses.into_iter().find(|a| public(a.ip())) {
            Some(target) => Ok(vec![target]),
            None => Err(std::io::Error::other("Not a public address.")),
        }
    }
}

/// Without the system proxy: through one, ureq resolves only the proxy's address, so PublicResolver would check the
/// proxy and never the step's own host. Trusts the same roots as every call (net.rs: public ones and Windows' store).
static HTTP: LazyLock<ureq::Agent> = LazyLock::new(|| crate::net::builder_direct().redirects(0).timeout(Duration::from_secs(8)).resolver(PublicResolver).build());

/// Documents\Bloxkeep.
pub fn folder() -> PathBuf {
    // Environment.GetFolderPath gives "" when Windows has none: Path.Combine("", "Bloxkeep") is "Bloxkeep".
    documents().map(|d| d.join("Bloxkeep")).unwrap_or_else(|| PathBuf::from("Bloxkeep"))
}

/// A file's name as a step may give it: letters, digits, spaces, - and _, up to 40.
pub static FILE_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[\p{L}\p{N} _\-]{1,40}$").expect("file name pattern"));

/// FILE_NAME as .NET matched it: a character past U+FFFF was two UTF-16 halves there, neither a letter nor a digit.
pub fn file_name_ok(name: &str) -> bool {
    FILE_NAME.is_match(name) && name.chars().all(|c| (c as u32) <= 0xFFFF)
}

fn path_of(name: &str) -> Res<PathBuf> {
    if !file_name_ok(name.trim()) {
        return Err(Fail::user(l::t("A file's name is letters, digits, spaces, - and _.")));
    }
    Ok(folder().join(format!("{}.txt", name.trim())))
}

impl IMacroOutside for MacroOutside {
    fn clipboard(&self) -> Res<Option<String>> {
        sta(|| if clipboard::contains_text() { clipboard::get_text().map(Some) } else { Ok(Some(String::new())) })
    }

    fn copy(&self, text: &str) -> Res<()> {
        let text = text.to_string();
        sta(move || if text.is_empty() { clipboard::clear() } else { clipboard::set_text(&text, 5, 100) })
    }

    fn read_file(&self, name: &str) -> Res<Option<String>> {
        let path = path_of(name)?;
        if !path.is_file() {
            return Ok(None);
        }
        // Polled every second: only the file's end is read, the whole file only when its end holds no full line.
        let last = |lines: &[String]| lines.iter().rev().find(|l| !l.trim().is_empty()).map(|l| utf16_prefix(l.trim(), MAX_READ));
        let found = match last(&read_tail_lines(&path)?) {
            Some(line) => Some(line),
            None => last(&read_all_lines(&path)?),
        };
        Ok(Some(found.unwrap_or_default()))
    }

    fn append_file(&self, name: &str, line: &str) -> Res<()> {
        let path = path_of(name)?;
        std::fs::create_dir_all(folder())?;
        // A log never grows past 5 MB: the older half goes.
        if let Ok(meta) = std::fs::metadata(&path)
            && meta.is_file()
            && meta.len() > 5 * 1024 * 1024
        {
            let all = read_all_lines(&path)?;
            let mut kept = String::new();
            for l in &all[all.len() / 2..] {
                kept.push_str(l);
                kept.push_str("\r\n");
            }
            std::fs::write(&path, kept)?;
        }
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
        file.write_all(format!("{}\r\n", line.replace(['\r', '\n'], " ")).as_bytes())?;
        Ok(())
    }

    /// On a pool worker, waited for with the token: the address's name lookup has no timeout of its own, and the turn (keys
    /// held, the window in front) must stop at once when the run is stopped or Bloxkeep closes. A worker left behind
    /// ends on its own (the 8 s timeout, or the lookup giving up).
    fn web(&self, url: &str, post: Option<&str>, ct: &Cancel) -> Res<Option<String>> {
        let url = url.trim().to_string();
        let Some(host) = https_host(&url) else {
            return Err(Fail::user(l::t("A web address starts with https://.")));
        };
        ct.check()?;
        let post = post.map(str::to_string);
        let (tx, rx) = std::sync::mpsc::channel();
        crate::threads::run(move || {
            let _ = tx.send(web_now(&url, &host, post.as_deref()));
        });
        loop {
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(answer) => {
                    ct.check()?;
                    return answer;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => ct.check()?,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return Err(Fail::other("The web step's worker ended without an answer.")),
            }
        }
    }
}

/// The web step's request itself (blocking).
fn web_now(url: &str, host: &str, post: Option<&str>) -> Res<Option<String>> {
    let req = HTTP.request(if post.is_none() { "GET" } else { "POST" }, url).set("User-Agent", "Bloxkeep-macro");
    let sent = match post {
        // PORT: .NET's serializer wrote non-ASCII as \uXXXX; serde_json writes it as it is (the same JSON).
        Some(p) => req.set("Content-Type", "application/json; charset=utf-8").send_string(&serde_json::json!({ "content": p }).to_string()),
        None => req.call(),
    };
    // PORT: ureq can't be called off halfway; the 8 s timeout bounds it, and the caller stops waiting on the token.
    let res = match sent {
        Ok(res) => res,
        Err(ureq::Error::Status(code, _)) => {
            web_note(&format!("Macro web step: HTTP {code} from {host}."));
            return Ok(None);
        }
        Err(ureq::Error::Transport(t)) => {
            let kind = if transport_timed_out(&t) { "TaskCanceledException" } else { "HttpRequestException" };
            web_note(&format!("Macro web step: {host} didn't answer ({kind})."));
            return Ok(None);
        }
    };
    if !(200..=299).contains(&res.status()) {
        web_note(&format!("Macro web step: HTTP {} from {host}.", res.status()));
        return Ok(None);
    }
    *LAST_WEB_NOTE.lock().unwrap_or_else(|e| e.into_inner()) = None;
    // At most MAX_READ characters: up to 4 bytes each in UTF-8.
    let mut bytes = Vec::new();
    if let Err(e) = res.into_reader().take((MAX_READ * 4) as u64).read_to_end(&mut bytes) {
        let kind = if e.kind() == std::io::ErrorKind::TimedOut { "TaskCanceledException" } else { "IOException" };
        web_note(&format!("Macro web step: {host} didn't answer ({kind})."));
        return Ok(None);
    }
    let text = String::from_utf8_lossy(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes)).into_owned();
    Ok(Some(utf16_prefix(&text, MAX_READ)))
}

fn transport_timed_out(t: &ureq::Transport) -> bool {
    use std::error::Error;
    let mut source = t.source();
    while let Some(s) = source {
        if let Some(io) = s.downcast_ref::<std::io::Error>()
            && matches!(io.kind(), std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock)
        {
            return true;
        }
        source = s.source();
    }
    false
}

/// Uri.TryCreate(…, Absolute) with the https scheme: the host (lower case, as Uri.Host gives it), or None.
/// PORT: .NET's Uri parser refused more malformed addresses than this does; those reach ureq here and come back as
/// "didn't answer" instead of failing the run.
fn https_host(url: &str) -> Option<String> {
    if !url.get(..8).is_some_and(|p| p.eq_ignore_ascii_case("https://")) || url.chars().any(char::is_whitespace) {
        return None;
    }
    let rest = &url[8..];
    let authority = &rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())];
    let host_port = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = if host_port.starts_with('[') {
        &host_port[..host_port.find(']').map_or(host_port.len(), |i| i + 1)]
    } else {
        host_port.split(':').next().unwrap_or("")
    };
    if host.is_empty() { None } else { Some(host.to_lowercase()) }
}

/// error.log: the last web failure written; a step in a loop would write the same one every few seconds.
static LAST_WEB_NOTE: Mutex<Option<String>> = Mutex::new(None);

fn web_note(text: &str) {
    let mut last = LAST_WEB_NOTE.lock().unwrap_or_else(|e| e.into_inner());
    if last.as_deref() != Some(text) {
        *last = Some(text.to_string());
        drop(last);
        log::note(text);
    }
}

/// Whether an address is public: never this PC, the home network or a link-local one.
pub fn public(ip: IpAddr) -> bool {
    let ip = match ip {
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => IpAddr::V4(v4),
            None => IpAddr::V6(v6),
        },
        v4 => v4,
    };
    if ip.is_loopback() || ip.is_unspecified() {
        return false;
    }
    match ip {
        IpAddr::V4(v4) => {
            let b = v4.octets();
            !(b[0] == 10
                || b[0] == 0
                || b[0] >= 224
                || (b[0] == 172 && (16..=31).contains(&b[1]))
                || (b[0] == 192 && b[1] == 168)
                || (b[0] == 169 && b[1] == 254)
                || (b[0] == 100 && (64..=127).contains(&b[1])))
        }
        IpAddr::V6(v6) => {
            let first = v6.segments()[0];
            // IsIPv6LinkLocal fe80::/10, IsIPv6SiteLocal fec0::/10, IsIPv6Multicast ff00::/8, IsIPv6UniqueLocal fc00::/7.
            !((first & 0xFFC0) == 0xFE80 || (first & 0xFFC0) == 0xFEC0 || (first & 0xFF00) == 0xFF00 || (first & 0xFE00) == 0xFC00)
        }
    }
}

/// The work on a pool thread, waited for (C# gave the clipboard an STA thread).
/// PORT: the thread isn't marked STA: the Win32 clipboard calls used here need no COM apartment.
fn sta<T: Send + 'static>(work: impl FnOnce() -> Res<T> + Send + 'static) -> Res<T> {
    crate::threads::scope(|s| s.spawn(work).join()).unwrap_or_else(|_| Err(Fail::other("The clipboard thread stopped.")))
}

/// The first `max` UTF-16 units of a text, as .NET's [..max] (a character isn't cut in half here).
fn utf16_prefix(text: &str, max: usize) -> String {
    let mut used = 0;
    let mut out = String::new();
    for c in text.chars() {
        used += c.len_utf16();
        if used > max {
            break;
        }
        out.push(c);
    }
    out
}

/// File.ReadAllLines: the encoding from its byte order mark (UTF-8 without one), lines split at \r\n, \n or \r.
fn read_all_lines(path: &std::path::Path) -> Res<Vec<String>> {
    let bytes = std::fs::read(path)?;
    let text = if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        String::from_utf8_lossy(rest).into_owned()
    } else if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        String::from_utf16_lossy(&rest.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect::<Vec<_>>())
    } else if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        String::from_utf16_lossy(&rest.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect::<Vec<_>>())
    } else {
        String::from_utf8_lossy(&bytes).into_owned()
    };
    Ok(split_lines(&text))
}

/// The lines in the file's last 64 KB (its byte order mark read from the start), the first one, likely cut, left
/// out. The whole file when it is that small.
fn read_tail_lines(path: &std::path::Path) -> Res<Vec<String>> {
    use std::io::{Seek, SeekFrom};
    const TAIL: u64 = 64 * 1024;
    let mut file = std::fs::File::open(path)?;
    let len = file.metadata()?.len();
    if len <= TAIL {
        return read_all_lines(path);
    }
    let mut head = [0u8; 2];
    file.read_exact(&mut head)?;
    // Even, so a UTF-16 file's halves stay paired.
    file.seek(SeekFrom::Start((len - TAIL) & !1))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let text = match head {
        [0xFF, 0xFE] => String::from_utf16_lossy(&bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect::<Vec<_>>()),
        [0xFE, 0xFF] => String::from_utf16_lossy(&bytes.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect::<Vec<_>>()),
        _ => String::from_utf8_lossy(&bytes).into_owned(),
    };
    let mut lines = split_lines(&text);
    if !lines.is_empty() {
        lines.remove(0);
    }
    Ok(lines)
}

/// Lines split at \r\n, \n or \r, as File.ReadAllLines splits them.
fn split_lines(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                lines.push(std::mem::take(&mut line));
            }
            '\n' => lines.push(std::mem::take(&mut line)),
            c => line.push(c),
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// Environment.GetFolderPath(SpecialFolder.MyDocuments).
fn documents() -> Option<PathBuf> {
    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::{FOLDERID_Documents, SHGetKnownFolderPath};
    let mut path: *mut u16 = std::ptr::null_mut();
    // SAFETY: FOLDERID_Documents is a valid known-folder id; `path` receives a CoTaskMemAlloc'd string freed below.
    let hr = unsafe { SHGetKnownFolderPath(&FOLDERID_Documents, 0, std::ptr::null_mut(), &mut path) };
    let result = if hr >= 0 && !path.is_null() {
        // SAFETY: on success `path` points at a NUL-terminated UTF-16 string.
        let len = unsafe { (0..).take_while(|&i| *path.add(i) != 0).count() };
        // SAFETY: `len` units were just read as part of the string.
        let wide = unsafe { std::slice::from_raw_parts(path, len) };
        Some(PathBuf::from(String::from_utf16_lossy(wide)))
    } else {
        None
    };
    if !path.is_null() {
        // SAFETY: the string came from SHGetKnownFolderPath, which says to free it with CoTaskMemFree.
        unsafe { CoTaskMemFree(path as *const core::ffi::c_void) };
    }
    result.filter(|p| !p.as_os_str().is_empty())
}

/// The Win32 clipboard, as WinForms' Clipboard used it (CF_UNICODETEXT).
mod clipboard {
    use std::time::Duration;

    use windows_sys::Win32::Foundation::{GlobalFree, HANDLE, HWND};
    use windows_sys::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard, SetClipboardData};
    use windows_sys::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock};
    use windows_sys::Win32::UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow, HWND_MESSAGE};

    use crate::core::base::{Fail, Res};

    const CF_UNICODETEXT: u32 = 13;

    /// The open clipboard and its owner, a message-only window made for this call (with no owner, EmptyClipboard
    /// leaves the clipboard ownerless and SetClipboardData fails). Closed, then the window destroyed, on drop.
    struct Opened(HWND);

    impl Drop for Opened {
        fn drop(&mut self) {
            // SAFETY: the clipboard was opened by this thread; the window was made on this thread.
            unsafe {
                CloseClipboard();
                if !self.0.is_null() {
                    DestroyWindow(self.0);
                }
            }
        }
    }

    /// OpenClipboard, tried `times` times `delay_ms` apart (another app may hold it a moment).
    fn open(times: i32, delay_ms: u64) -> Res<Opened> {
        let class: Vec<u16> = "STATIC".encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: a message-only window of the built-in STATIC class; destroyed below or by Opened's drop.
        let owner = unsafe {
            CreateWindowExW(0, class.as_ptr(), std::ptr::null(), 0, 0, 0, 0, 0, HWND_MESSAGE, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null())
        };
        for i in 0..times.max(1) {
            // SAFETY: `owner` is this thread's window (or null); the clipboard is closed again by Opened's drop.
            if unsafe { OpenClipboard(owner) } != 0 {
                return Ok(Opened(owner));
            }
            if i + 1 < times {
                std::thread::sleep(Duration::from_millis(delay_ms));
            }
        }
        if !owner.is_null() {
            // SAFETY: made above on this thread, never handed on.
            unsafe { DestroyWindow(owner) };
        }
        Err(Fail::other("ExternalException: Requested Clipboard operation did not succeed."))
    }

    pub fn contains_text() -> bool {
        // SAFETY: plain query, no clipboard ownership needed.
        unsafe { IsClipboardFormatAvailable(CF_UNICODETEXT) != 0 }
    }

    pub fn get_text() -> Res<String> {
        let _open = open(10, 100)?;
        Ok(read_text())
    }

    fn read_text() -> String {
        // SAFETY: the clipboard is open; the handle is owned by the clipboard and only read here, locked.
        unsafe {
            let h: HANDLE = GetClipboardData(CF_UNICODETEXT);
            if h.is_null() {
                return String::new();
            }
            let p = GlobalLock(h) as *const u16;
            if p.is_null() {
                return String::new();
            }
            let max = GlobalSize(h) / 2;
            let len = (0..max).take_while(|&i| *p.add(i) != 0).count();
            let text = String::from_utf16_lossy(std::slice::from_raw_parts(p, len));
            GlobalUnlock(h);
            text
        }
    }

    pub fn clear() -> Res<()> {
        let _open = open(10, 100)?;
        // SAFETY: the clipboard is open.
        unsafe { EmptyClipboard() };
        Ok(())
    }

    /// Clipboard.SetDataObject(text, copy: true, retryTimes, retryDelay).
    pub fn set_text(text: &str, retry_times: i32, retry_delay_ms: u64) -> Res<()> {
        let _open = open(retry_times, retry_delay_ms)?;
        put(text)
    }

    fn put(text: &str) -> Res<()> {
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: the clipboard is open; the memory is allocated, filled while locked, then handed to the clipboard,
        // which owns it from then on (freed here only when SetClipboardData refused it).
        unsafe {
            if EmptyClipboard() == 0 {
                return Err(Fail::other("ExternalException: Requested Clipboard operation did not succeed."));
            }
            let h = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2);
            if h.is_null() {
                return Err(Fail::other("OutOfMemoryException"));
            }
            let p = GlobalLock(h) as *mut u16;
            if p.is_null() {
                GlobalFree(h);
                return Err(Fail::other("OutOfMemoryException"));
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr(), p, wide.len());
            GlobalUnlock(h);
            if SetClipboardData(CF_UNICODETEXT, h).is_null() {
                GlobalFree(h);
                return Err(Fail::other("ExternalException: Requested Clipboard operation did not succeed."));
            }
        }
        Ok(())
    }
}
