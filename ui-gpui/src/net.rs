//! HTTP the way the C# app's HttpClient made it: through the proxy set in Windows, trusting the certificates Windows
//! trusts. ureq's own default knows neither (only its built-in list of public roots, no proxy), so behind an office
//! proxy, or with an antivirus that checks HTTPS with its own root in Windows' store, every call to Roblox failed.
//! Every agent starts from `builder()`.

use std::sync::{Arc, LazyLock};

/// The public roots ureq knew, plus every root in Windows' store (the user's and the machine's).
static TLS: LazyLock<Arc<rustls::ClientConfig>> = LazyLock::new(|| {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let found = rustls_native_certs::load_native_certs();
    let _ = roots.add_parsable_certificates(found.certs);
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = match rustls::ClientConfig::builder_with_provider(provider).with_safe_default_protocol_versions() {
        Ok(b) => b.with_root_certificates(roots).with_no_client_auth(),
        // (Never with ring's provider; ureq's own default then.)
        Err(_) => rustls::ClientConfig::builder().with_root_certificates(rustls::RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() }).with_no_client_auth(),
    };
    Arc::new(config)
});

/// An agent builder with Windows' certificates and no proxy at all: for a call that must reach its host itself (a
/// macro's web step, whose host is checked to be public: through a proxy only the proxy's address would be).
pub fn builder_direct() -> ureq::AgentBuilder {
    ureq::AgentBuilder::new().tls_config(TLS.clone())
}

/// An agent builder with Windows' proxy and certificates; the caller adds its timeouts and the rest.
pub fn builder() -> ureq::AgentBuilder {
    let mut b = ureq::AgentBuilder::new().tls_config(TLS.clone());
    if let Some(p) = system_proxy() {
        b = b.proxy(p);
    }
    b
}

/// How many times a read Roblox refused with 429 is asked in all (`wait_cool`).
pub const RATE_TRIES: u32 = 4;

/// Hosts Roblox is rate-limiting: until when, and how many 429s in a row.
static COOL: LazyLock<std::sync::Mutex<std::collections::HashMap<String, (std::time::Instant, u32)>>> = LazyLock::new(Default::default);

fn host_of(url: &str) -> String {
    url.split("://").nth(1).unwrap_or(url).split('/').next().unwrap_or_default().to_ascii_lowercase()
}

/// Roblox's rate limit (429) holds for a whole host: once a read is refused, every read to that host waits until
/// the time is up (`cooled`), instead of each being refused in turn while a page asks its many parts. Waits at
/// most 15 s.
pub fn wait_cool(url: &str) {
    let until = COOL.lock().unwrap_or_else(|e| e.into_inner()).get(&host_of(url)).map(|c| c.0);
    if let Some(left) = until.and_then(|u| u.checked_duration_since(std::time::Instant::now())) {
        std::thread::sleep(left.min(std::time::Duration::from_secs(15)));
    }
}

/// `url`'s host answered 429: it is waited on for what Roblox said (`said`, its x-ratelimit-reset or Retry-After),
/// else 1.5 s doubling with each 429 in a row; at most 15 s.
pub fn cooled(url: &str, said: Option<std::time::Duration>) {
    let mut map = COOL.lock().unwrap_or_else(|e| e.into_inner());
    let c = map.entry(host_of(url)).or_insert((std::time::Instant::now(), 0));
    c.1 += 1;
    let wait = said.unwrap_or_else(|| std::time::Duration::from_millis(1500 << (c.1 - 1).min(3))).min(std::time::Duration::from_secs(15));
    c.0 = c.0.max(std::time::Instant::now() + wait);
}

/// `url`'s host answered: its run of 429s is over.
pub fn calm(url: &str) {
    let mut map = COOL.lock().unwrap_or_else(|e| e.into_inner());
    if map.get(&host_of(url)).is_some_and(|c| c.1 > 0) {
        map.remove(&host_of(url));
    }
}

/// A call the interface made that failed, in error.log (every failure a person can meet must be there): what
/// asked, the address without its query and with ids as "#", and why. The same failure of the same address is
/// written once every five minutes, so a page polling while offline can't flood the log.
pub fn note_failed(what: &str, url: &str, why: &str) {
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    static SEEN: Mutex<Option<HashMap<String, Instant>>> = Mutex::new(None);
    let ids = |s: &str| -> String {
        let mut out = String::new();
        let mut run = String::new();
        for ch in s.chars().chain(std::iter::once(' ')) {
            if ch.is_ascii_digit() {
                run.push(ch);
                continue;
            }
            if run.len() >= 5 { out.push('#') } else { out.push_str(&run) }
            run.clear();
            out.push(ch);
        }
        out.pop();
        out
    };
    // A file's path names the Windows user (C:\Users\name\…), a PC's name may be the person's.
    let private = |s: String| -> String {
        ["USERNAME", "COMPUTERNAME"].iter().filter_map(|v| std::env::var(v).ok()).filter(|w| w.chars().count() >= 3).fold(s, |s, w| s.replace(&w, "[user]"))
    };
    let place = private(ids(url.split(['?', '#']).next().unwrap_or_default()));
    let why: String = private(ids(why)).chars().take(200).collect();
    let line = if place.is_empty() { format!("{what} failed ({why})") } else { format!("{what}: {place} failed ({why})") };
    {
        let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
        let seen = seen.get_or_insert_with(HashMap::new);
        let now = Instant::now();
        if seen.get(&line).is_some_and(|at| now.duration_since(*at) < Duration::from_secs(300)) {
            return;
        }
        if seen.len() > 200 {
            seen.retain(|_, at| now.duration_since(*at) < Duration::from_secs(300));
        }
        seen.insert(line.clone(), now);
    }
    #[cfg(feature = "core")]
    crate::core::log::note_file(&line);
    #[cfg(not(feature = "core"))]
    eprintln!("{line}");
}

/// One of Bloxkeep's own saved files read back. Missing is normal (nothing saved yet); one that can't be read, or isn't
/// what was written (cut short, edited by hand), is in error.log, as the list then starts empty and the person sees it.
pub fn saved<T: serde::de::DeserializeOwned>(what: &str, path: &std::path::Path) -> Option<T> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            note_failed(what, &path.to_string_lossy(), &format!("couldn't be read: {e}"));
            return None;
        }
    };
    let text = String::from_utf8_lossy(&bytes);
    match serde_json::from_str(text.trim_start_matches('\u{feff}')) {
        Ok(v) => Some(v),
        Err(e) => {
            note_failed(what, &path.to_string_lossy(), &format!("isn't readable, started empty: {e}"));
            None
        }
    }
}

/// A file of Bloxkeep's written (or removed): a failure is in error.log, as what it keeps is lost at the next start.
pub fn wrote<E: std::fmt::Display>(what: &str, path: &std::path::Path, r: Result<(), E>) {
    if let Err(e) = r {
        note_failed(what, &path.to_string_lossy(), &format!("couldn't be written: {e}"));
    }
}

/// Something that had failed before and worked now, said once a run in error.log: a later look at the log can tell
/// "it works" from "it was never opened" (a fix to a call nobody here can make signed in is checked this way).
pub fn note_worked(what: &str) {
    use std::sync::Mutex;
    static SAID: Mutex<Vec<String>> = Mutex::new(Vec::new());
    {
        let mut said = SAID.lock().unwrap_or_else(|e| e.into_inner());
        if said.iter().any(|s| s == what) {
            return;
        }
        said.push(what.to_string());
    }
    #[cfg(feature = "core")]
    crate::core::log::note_file(what);
    #[cfg(not(feature = "core"))]
    eprintln!("{what}");
}

/// A result the interface shows: its failure written as `note_failed` does (ids as "#", once every five minutes).
pub fn noted<T>(what: &str, r: Result<T, String>) -> Result<T, String> {
    if let Err(why) = &r {
        note_failed(what, "", why);
    }
    r
}

/// An agent builder that reaches a host the way Roblox's own client does (its HTTP is libcurl): through a proxy only when
/// the environment names one (https_proxy, HTTPS_PROXY, all_proxy, ALL_PROXY, in libcurl's order, unless no_proxy covers
/// the host), never through Windows' proxy, which the client doesn't read (RobloxPlayerBeta.exe 0.747 has libcurl's
/// variable names and no WinINet or WinHTTP call, no "Internet Settings", no ProxyServer; checked 2026-11-15). A sign-in
/// ticket is bound to the address that asked for it (tested with two real accounts, 2026-10-13), so it is asked this way.
pub fn builder_like_roblox_client(host: &str) -> ureq::AgentBuilder {
    let b = ureq::AgentBuilder::new().tls_config(TLS.clone());
    match env_proxy_for(host, |name| std::env::var(name).ok()).and_then(|p| ureq::Proxy::new(p).ok()) {
        Some(p) => b.proxy(p),
        None => b,
    }
}

/// libcurl's choice of proxy for an https host from the environment (`var` reads a variable).
pub fn env_proxy_for(host: &str, var: impl Fn(&str) -> Option<String>) -> Option<String> {
    let skip = ["no_proxy", "NO_PROXY"].iter().find_map(|n| var(n)).unwrap_or_default();
    let host = host.to_ascii_lowercase();
    let skipped = skip.split(',').map(|e| e.trim().trim_start_matches('.').to_ascii_lowercase()).filter(|e| !e.is_empty()).any(|e| {
        e == "*" || host == e || host.ends_with(&format!(".{e}"))
    });
    if skipped {
        return None;
    }
    ["https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"].iter().find_map(|n| var(n)).map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// Windows' proxy as Bloxkeep's calls use it, "local, port 10809" / "remote, port 8080" (never its address), or None.
pub fn proxy_in_use() -> Option<String> {
    let server = proxy_setting()?;
    let host = server.split(';').find_map(|p| p.split_once('=').map(|(_, v)| v.to_string())).unwrap_or(server);
    let host = host.trim().trim_start_matches("http://").trim_start_matches("https://").trim_end_matches('/').to_string();
    if host.is_empty() {
        return None;
    }
    let (name, port) = host.rsplit_once(':').unwrap_or((host.as_str(), "?"));
    let local = name.eq_ignore_ascii_case("localhost") || name.starts_with("127.") || name == "[::1]";
    Some(format!("{}, port {port}", if local { "local" } else { "remote" }))
}

/// The proxy in Windows' Internet settings (Settings › Network › Proxy, "Use a proxy server"), as .NET's default
/// proxy reads it: "host:port", or per scheme "http=host:port;https=host:port" (the https one is used: every call
/// here is https). A SOCKS-only setting is left out.
fn system_proxy() -> Option<ureq::Proxy> {
    let server = proxy_setting()?;
    let pick = if server.contains('=') {
        let parts: Vec<(&str, &str)> = server.split(';').filter_map(|p| p.split_once('=')).map(|(k, v)| (k.trim(), v.trim())).collect();
        parts.iter().find(|(k, _)| k.eq_ignore_ascii_case("https")).or_else(|| parts.iter().find(|(k, _)| k.eq_ignore_ascii_case("http"))).map(|(_, v)| v.to_string())?
    } else {
        server.trim().to_string()
    };
    let host = pick.trim_start_matches("http://").trim_start_matches("https://").trim_end_matches('/');
    if host.is_empty() {
        return None;
    }
    ureq::Proxy::new(format!("http://{host}")).ok()
}

#[cfg(windows)]
fn proxy_setting() -> Option<String> {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegGetValueW};
    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let key = wide(r"Software\Microsoft\Windows\CurrentVersion\Internet Settings");
    let mut on: u32 = 0;
    let mut size = 4u32;
    // SAFETY: reads a DWORD into `on`, `size` bytes long.
    let got = unsafe { RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), wide("ProxyEnable").as_ptr(), RRF_RT_REG_DWORD, std::ptr::null_mut(), (&mut on as *mut u32).cast(), &mut size) };
    if got != 0 || on == 0 {
        return None;
    }
    let mut buf = vec![0u16; 1024];
    let mut size = (buf.len() * 2) as u32;
    // SAFETY: reads a string into `buf`, `size` bytes long.
    let got = unsafe { RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), wide("ProxyServer").as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut size) };
    if got != 0 {
        return None;
    }
    let len = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    let text = String::from_utf16_lossy(&buf[..len]);
    (!text.trim().is_empty()).then_some(text)
}

#[cfg(not(windows))]
fn proxy_setting() -> Option<String> {
    None
}

/// Whether this PC can reach the internet now, for the "No connection" notice (as Discord's and Facebook's say it).
/// Windows' own answer first (GetNetworkConnectivityHint: no network at all means offline at once; its "local only"
/// is left to the tries below, as Windows says it wrongly where its own test address is blocked); then a
/// plain TCP connect to two public resolvers on 443 (no data sent), and to the proxy when one is set, so a PC whose
/// only way out is a proxy isn't called offline. Any one that answers in 2 s means online. Blocking: a background
/// thread calls it.
pub fn online_now() -> bool {
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::Duration;
    #[cfg(windows)]
    {
        use windows_sys::Win32::NetworkManagement::IpHelper::GetNetworkConnectivityHint;
        use windows_sys::Win32::Networking::WinSock::{NL_NETWORK_CONNECTIVITY_HINT, NetworkConnectivityLevelHintNone};
        // SAFETY: fills the struct it is given.
        let mut hint: NL_NETWORK_CONNECTIVITY_HINT = unsafe { std::mem::zeroed() };
        if unsafe { GetNetworkConnectivityHint(&mut hint) } == 0 && hint.ConnectivityLevel == NetworkConnectivityLevelHintNone {
            return false;
        }
    }
    let mut places: Vec<String> = vec!["1.1.1.1:443".into(), "8.8.8.8:443".into()];
    if let Some(p) = proxy_host_port() {
        places.insert(0, p);
    }
    places.iter().any(|place| {
        place.to_socket_addrs().ok().and_then(|mut a| a.next()).is_some_and(|addr| TcpStream::connect_timeout(&addr, Duration::from_secs(2)).is_ok())
    })
}

/// The proxy's "host:port" (Windows' setting, else the environment's), for `online_now`.
fn proxy_host_port() -> Option<String> {
    let raw = proxy_setting()
        .map(|s| s.split(';').find_map(|p| p.split_once('=').map(|(_, v)| v.to_string())).unwrap_or(s))
        .or_else(|| env_proxy_for("roblox.com", |n| std::env::var(n).ok()))?;
    let after = raw.trim().rsplit("://").next().unwrap_or_default();
    let host = after.rsplit('@').next().unwrap_or_default().trim_end_matches('/');
    (!host.is_empty() && host.contains(':')).then(|| host.to_string())
}

/// What a call to Roblox that never got an answer (no connection, or the connection cut: Roblox blocked on this network)
/// gives the page instead of the transport's own words; the real error is in error.log. Pages show it in the reader's
/// language (`why_words`), and the title bar's notice (shell) looks again at once.
pub const UNREACHABLE: &str = "Roblox can't be reached";

static ROBLOX_FAILED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// A call to Roblox got no answer: written to error.log with its real error, and the connection looked at again soon.
pub fn unreachable(what: &str, url: &str, why: &str) -> String {
    note_failed(what, url, why);
    ROBLOX_FAILED.store(true, std::sync::atomic::Ordering::Relaxed);
    UNREACHABLE.to_string()
}

/// Whether a call to Roblox got no answer since this was last asked.
pub fn take_roblox_failed() -> bool {
    ROBLOX_FAILED.swap(false, std::sync::atomic::Ordering::Relaxed)
}

/// Whether Roblox answers at all (any answer, an error code too, means it is reached), through the same proxy as
/// every other call. Blocking.
pub fn roblox_reachable() -> bool {
    use std::time::Duration;
    static AGENT: std::sync::OnceLock<ureq::Agent> = std::sync::OnceLock::new();
    let agent = AGENT.get_or_init(|| builder().timeout(Duration::from_secs(6)).user_agent("Bloxkeep").build());
    ["https://www.roblox.com/", "https://apis.roblox.com/"].iter().any(|u| matches!(agent.head(u).call(), Ok(_) | Err(ureq::Error::Status(..))))
}

/// A page's "why" in its reader's words: Roblox's name for an empty one, the reader's language for `UNREACHABLE`.
pub fn why_words(i18n: &crate::i18n::I18n, why: &str) -> String {
    if why.is_empty() {
        "Roblox".into()
    } else if why == UNREACHABLE {
        i18n.t("net.unreachableWhy")
    } else {
        why.to_string()
    }
}
