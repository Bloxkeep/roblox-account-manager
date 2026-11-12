//! PhoneAlert.cs: alerts on the phone through ntfy (ntfy.sh, a free app for Android and iPhone, no account): Bloxkeep
//! posts to a "topic" the phone subscribes to. Anyone who knows a topic's name can read it, so Bloxkeep makes a long
//! random one. A person with their own ntfy server pastes its https address with the topic instead.
use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;

use crate::core::base::Cancel;
use crate::core::l;
use crate::core::log;

const SERVER: &str = "https://ntfy.sh/";
// .NET's `$` also matches before a final "\n": `\n?\z` here.
static TOPIC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z0-9_-]{6,64}\n?\z").unwrap());
static HTTP: LazyLock<ureq::Agent> = LazyLock::new(|| ureq::AgentBuilder::new().timeout(Duration::from_secs(10)).redirects(0).build());

/// A new topic no one could guess: "bloxkeep-" and 16 letters and digits.
pub fn new_topic() -> String {
    const CHARS: &[u8] = b"abcdefghijkmnpqrstuvwxyz23456789";
    let mut out = String::from("bloxkeep-");
    for _ in 0..16 {
        out.push(CHARS[random_below(CHARS.len() as u32) as usize] as char);
    }
    out
}

/// RandomNumberGenerator.GetInt32(n): uniform in 0..n (rejection sampling, no bias).
fn random_below(n: u32) -> u32 {
    let limit = u32::MAX - u32::MAX % n;
    loop {
        let mut b = [0u8; 4];
        getrandom::fill(&mut b).expect("the system's random source");
        let v = u32::from_le_bytes(b);
        if v < limit {
            return v % n;
        }
    }
}

/// A topic's name, or https://server/topic, kept as given (trimmed); None for anything else. (C#'s TryNormalize.)
pub fn try_normalize(text: &str) -> Option<String> {
    let topic = text.trim().to_string();
    if TOPIC.is_match(&topic) {
        return Some(topic);
    }
    let uri = url::Url::parse(&topic).ok()?;
    let ok = uri.scheme() == "https" && uri.query().is_none_or(|q| q.is_empty()) && TOPIC.is_match(uri.path().trim_matches('/'));
    if ok { Some(topic) } else { None }
}

/// Where a topic's messages are posted.
pub fn url_of(topic: &str) -> String {
    if topic.get(..8).is_some_and(|p| p.eq_ignore_ascii_case("https://")) { topic.to_string() } else { format!("{SERVER}{topic}") }
}

/// Posts one alert; None when sent, else why not (for Settings to show).
pub fn send(topic: &str, text: &str, urgent: bool, ct: &Cancel) -> Option<String> {
    if ct.is_cancelled() {
        log::note("Phone alert: no answer (TaskCanceledException).");
        return Some(l::t("ntfy didn't answer. Check the internet connection."));
    }
    // At most 1000 UTF-16 units, as C# cuts it.
    let units: Vec<u16> = text.encode_utf16().collect();
    let body = if units.len() > 1000 { String::from_utf16_lossy(&units[..1000]) } else { text.to_string() };
    let mut req = HTTP
        .post(&url_of(topic))
        .set("Content-Type", "text/plain; charset=utf-8")
        .set("User-Agent", "Bloxkeep/1")
        .set("Title", "Bloxkeep")
        .set("Tags", if urgent { "warning" } else { "video_game" });
    if urgent {
        req = req.set("Priority", "high");
    }
    // ureq hands a 3xx (redirects off) back as an answer; C#'s IsSuccessStatusCode is 2xx only.
    let status = match req.send_string(&body) {
        Ok(res) => Ok(res.status()),
        Err(ureq::Error::Status(code, _)) => Ok(code),
        Err(e) => Err(e),
    };
    match status {
        Ok(code) if (200..300).contains(&code) => None,
        Ok(code) => {
            log::note(&format!("Phone alert: HTTP {code}."));
            Some(if code == 429 { l::t("ntfy says too many alerts went out. They go again in a while.") } else { l::f("ntfy didn't take the alert (error {0}).", &[&code]) })
        }
        Err(e) => {
            log::note(&format!("Phone alert: no answer ({:?}).", e.kind()));
            Some(l::t("ntfy didn't answer. Check the internet connection."))
        }
    }
}
