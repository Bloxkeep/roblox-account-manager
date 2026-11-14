// RobloxWatch.cs: what the Roblox watch found per build.
//! What the Roblox watch (services/RobloxWatch/watch.py, docs/roblox-watch.md) found per build: whether Multi-Roblox's
//! single-window lock changed, and which names to hold if renamed. Read from the roblox-status branch; only verdicts and
//! names shaped like Roblox's lock objects are taken.
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, RwLock};
use std::time::Duration;

use regex::Regex;
use serde_json::Value;

use crate::core::base::{Cancel, Res, Stamp};
use crate::core::roblox_api::{HttpRequest, send_on};
use crate::core::roblox_installs::parse_version;

const STATUS_URL: &str = "https://raw.githubusercontent.com/Bloxkeep/roblox-account-manager/roblox-status/multi-status.json";
pub const DOCS_URL: &str = "https://github.com/Bloxkeep/roblox-account-manager/blob/main/docs/roblox-watch.md";
/// At most this many names are held for a build (Roblox has used one or two).
const MAX_HOLD: usize = 4;
static LOCK_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^ROBLOX_[A-Za-z0-9_]{1,64}$").unwrap());
const VERDICTS: [&str; 5] = ["first", "same", "changed", "renamed", "gone"];

// PORT: ureq 2 has no PooledConnectionLifetime; no cookie store either (UseCookies = false).
static HTTP: LazyLock<ureq::Agent> = LazyLock::new(|| crate::net::builder().timeout(Duration::from_secs(15)).redirects(5).build());

/// `hash`: the build's folder name ("version-…"), as under Roblox's Versions folder.
/// `verdict`: first, same, changed, renamed or gone.
/// `hold`: the lock names to hold for this build; None when the watch didn't say (its own name then).
#[derive(Clone, Debug, PartialEq)]
pub struct Build {
    pub version: String,
    pub hash: String,
    pub verdict: String,
    pub hold: Option<Vec<String>>,
}

/// `live`: the build Roblox serves now, when the watch has checked it.
/// `next`: the build Roblox offers before release (its beta channel), when newer than the live one.
/// `by_hash`: keyed by the hash in upper case (the C#'s OrdinalIgnoreCase); see `get`.
#[derive(Clone, Debug, PartialEq)]
pub struct Status {
    pub live: Option<Build>,
    pub next: Option<Build>,
    pub by_hash: HashMap<String, Build>,
}

impl Status {
    /// ByHash's lookup, ignoring case.
    pub fn get(&self, hash: &str) -> Option<&Build> {
        self.by_hash.get(&hash.to_uppercase())
    }
}

static CURRENT: RwLock<Option<Arc<Status>>> = RwLock::new(None);
static READ_AT: RwLock<Option<Stamp>> = RwLock::new(None);
static LAST_TEXT: RwLock<Option<String>> = RwLock::new(None);

pub fn current() -> Option<Arc<Status>> {
    CURRENT.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// The checks set it as the C# tests set the Current property (its private setter).
pub(crate) fn set_current(status: Option<Status>) {
    *CURRENT.write().unwrap_or_else(|e| e.into_inner()) = status.map(Arc::new);
}

/// When the status was last read; None until it was (offline, or GitHub couldn't be reached).
pub fn read_at() -> Option<Stamp> {
    *READ_AT.read().unwrap_or_else(|e| e.into_inner())
}

/// Application.ProductVersion without its "+…" build part.
// PORT: the C# read Application.ProductVersion; the crate's version stands in.
fn product_version() -> &'static str {
    env!("CARGO_PKG_VERSION").split('+').next().unwrap_or("")
}

/// Returns true when what the watch says changed since the last read.
pub fn refresh(ct: &Cancel) -> Res<bool> {
    ct.check()?;
    let mut req = HttpRequest::new("GET", STATUS_URL);
    req.add_header("User-Agent", &format!("Bloxkeep/{}", product_version()));
    let res = send_on(&HTTP, &req)?;
    ct.check()?;
    if !res.is_success() {
        return Ok(false);
    }
    let text = res.text();
    let Some(status) = parse(&text) else { return Ok(false) };
    *READ_AT.write().unwrap_or_else(|e| e.into_inner()) = Some(Stamp::utc_now());
    let mut last = LAST_TEXT.write().unwrap_or_else(|e| e.into_inner());
    if last.as_deref() == Some(text.as_str()) {
        return Ok(false);
    }
    *last = Some(text);
    *CURRENT.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(status));
    Ok(true)
}

/// The status file's findings, or None when the text isn't one. Entries out of shape are left out.
pub fn parse(json: &str) -> Option<Status> {
    let root: Value = serde_json::from_str(json).ok()?;
    let Value::Object(root) = root else { return None };
    let Some(Value::Array(builds)) = root.get("builds") else { return None };
    let mut by_hash: HashMap<String, Build> = HashMap::new();
    for b in builds {
        let Value::Object(b) = b else { continue };
        let (Some(version), Some(hash), Some(verdict)) = (text(b, "version"), text(b, "hash"), text(b, "verdict")) else { continue };
        // Lengths in UTF-16 units, as the C# counts them.
        let (vl, hl) = (version.encode_utf16().count(), hash.encode_utf16().count());
        if !(1..=40).contains(&vl) || !(1..=80).contains(&hl) || !VERDICTS.contains(&verdict.as_str()) {
            continue;
        }
        let mut hold = None;
        if let Some(Value::Array(h)) = b.get("hold") {
            let mut names: Vec<String> = Vec::new();
            for n in h {
                if let Value::String(n) = n
                    && LOCK_NAME.is_match(n)
                    && !names.contains(n)
                {
                    names.push(n.clone());
                    if names.len() == MAX_HOLD {
                        break;
                    }
                }
            }
            hold = Some(names);
        }
        by_hash.insert(hash.to_uppercase(), Build { version, hash, verdict, hold });
    }
    let named = |property: &str| -> Option<Build> {
        match root.get(property) {
            Some(Value::Object(p)) => text(p, "hash").and_then(|hash| by_hash.get(&hash.to_uppercase()).cloned()),
            _ => None,
        }
    };
    let live = named("live");
    let mut next = named("beta");
    if next.as_ref().is_some_and(|n| live.as_ref().is_none_or(|l| !newer(&n.version, &l.version))) {
        next = None;
    }
    Some(Status { live, next, by_hash })
}

/// What the watch says about a build (its folder name); None when it hasn't checked it.
pub fn for_hash(hash: Option<&str>) -> Option<Build> {
    let hash = hash.filter(|h| !h.is_empty())?;
    current()?.get(hash).cloned()
}

/// Renamed lock names to hold besides Multi-Roblox's own, for the installed and the live build (a launcher
/// updates to it before starting Roblox).
pub fn extra_names(installed_hash: Option<&str>, own_name: &str) -> Vec<String> {
    let live = current().and_then(|c| c.live.clone());
    let mut names: Vec<String> = Vec::new();
    for build in [for_hash(installed_hash), live].into_iter().flatten() {
        for n in build.hold.unwrap_or_default() {
            if n != own_name && !names.contains(&n) {
                names.push(n);
            }
        }
    }
    names.truncate(MAX_HOLD);
    names
}

fn newer(a: &str, b: &str) -> bool {
    match (parse_version(a), parse_version(b)) {
        (Some(x), Some(y)) => x > y,
        _ => a.encode_utf16().cmp(b.encode_utf16()) == std::cmp::Ordering::Greater,
    }
}

fn text(e: &serde_json::Map<String, Value>, name: &str) -> Option<String> {
    match e.get(name) {
        Some(Value::String(v)) => Some(v.clone()),
        _ => None,
    }
}
