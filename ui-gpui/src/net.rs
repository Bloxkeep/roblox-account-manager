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

/// An agent builder with Windows' proxy and certificates; the caller adds its timeouts and the rest.
pub fn builder() -> ureq::AgentBuilder {
    let mut b = ureq::AgentBuilder::new().tls_config(TLS.clone());
    if let Some(p) = system_proxy() {
        b = b.proxy(p);
    }
    b
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
