// Where a game's public server runs, for the game page's country pick beside Play. Roblox's server list says nothing of
// it; its join answer (gamejoin's join-game-instance, asked as roblox.com asks before it opens a server) holds the
// server's address (joinScript › UdmuxEndpoints), the same one the Roblox log prints as "UDMUX Address = …". Roblox's
// servers sit in 128.116.x.0/24 blocks, one block per site: the table below is the DevForum list of 2024-07-30 ("Roblox
// server region: a list of Roblox IP ranges and its location"), which matches what this PC's own Roblox logs showed
// (128.116.55.33 Tokyo with DatacenterId 377, 128.116.50.33 Singapore with 372, 2026-11-17). Geo-IP services disagree
// on these (ip-api said Portland for the Tokyo one), so a block not in the table is asked of ipinfo.io once and kept.
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use serde_json::Value;

use crate::core::base::Cancel;
use crate::core::log;

/// (country code, city) for a server's address, None when it can't be told.
pub fn region_of(ip: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = ip.split('.').collect();
    if parts.len() != 4 {
        return None;
    }
    let block = format!("{}.{}.{}", parts[0], parts[1], parts[2]);
    if let Some((cc, city)) = TABLE.iter().find(|(b, _, _)| *b == block).map(|(_, cc, city)| (cc.to_string(), city.to_string())) {
        return Some((cc, city));
    }
    if let Some(found) = LOOKED.lock().unwrap_or_else(|p| p.into_inner()).get(&block) {
        return found.clone();
    }
    // Asked once per block (and not again this run, found or not): only the server's address goes out.
    let found = ask_ipinfo(ip);
    if found.is_none() {
        log::note(&format!("Server regions: the block {block}.0/24 isn't in the table and ipinfo.io couldn't place it."));
    }
    LOOKED.lock().unwrap_or_else(|p| p.into_inner()).insert(block, found.clone());
    found
}

static LOOKED: LazyLock<Mutex<HashMap<String, Option<(String, String)>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn ask_ipinfo(ip: &str) -> Option<(String, String)> {
    let res = crate::core::roblox_api::get_plain(&format!("https://ipinfo.io/{ip}/json"), &Cancel::new()).ok()?;
    if !res.is_success() {
        return None;
    }
    let v: Value = serde_json::from_str(&res.text()).ok()?;
    let cc = v.get("country").and_then(Value::as_str).filter(|c| c.len() == 2)?.to_uppercase();
    let city = v.get("city").and_then(Value::as_str).unwrap_or_default().to_string();
    Some((cc, city))
}

const TABLE: &[(&str, &str, &str)] = &[
    ("128.116.115", "US", "Seattle"),
    ("128.116.116", "US", "Los Angeles"),
    ("128.116.1", "US", "Los Angeles"),
    ("128.116.63", "US", "Los Angeles"),
    ("128.116.95", "US", "Dallas"),
    ("128.116.101", "US", "Chicago"),
    ("128.116.48", "US", "Chicago"),
    ("128.116.22", "US", "Atlanta"),
    ("128.116.99", "US", "Atlanta"),
    ("128.116.45", "US", "Miami"),
    ("128.116.127", "US", "Miami"),
    ("128.116.102", "US", "Ashburn"),
    ("128.116.53", "US", "Ashburn"),
    ("128.116.32", "US", "New York"),
    ("128.116.117", "US", "San Jose"),
    ("209.206.42", "US", "San Jose"),
    ("209.206.43", "US", "San Jose"),
    ("128.116.33", "GB", "London"),
    ("128.116.119", "GB", "London"),
    ("128.116.21", "NL", "Amsterdam"),
    ("128.116.4", "FR", "Paris"),
    ("128.116.122", "FR", "Paris"),
    ("128.116.5", "DE", "Frankfurt"),
    ("128.116.44", "DE", "Frankfurt"),
    ("128.116.123", "DE", "Frankfurt"),
    ("128.116.31", "PL", "Warsaw"),
    ("128.116.124", "PL", "Warsaw"),
    ("128.116.104", "IN", "Mumbai"),
    ("128.116.55", "JP", "Tokyo"),
    ("128.116.120", "JP", "Tokyo"),
    ("128.116.50", "SG", "Singapore"),
    ("128.116.97", "SG", "Singapore"),
    ("128.116.51", "AU", "Sydney"),
    ("128.116.30", "HK", "Hong Kong"),
    ("128.116.118", "HK", "Hong Kong"),
];

/// The place of each server found so far (a server stays where it is), by its Job ID.
static JOBS: LazyLock<Mutex<HashMap<String, (String, String)>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn known(job: &str) -> Option<(String, String)> {
    JOBS.lock().unwrap_or_else(|p| p.into_inner()).get(job).cloned()
}

/// Each server's game version and start (Unix ms, 0 unknown), by its Job ID: what gamejoin said when it was placed.
static FACTS: LazyLock<Mutex<HashMap<String, (i64, i64)>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn facts(job: &str) -> Option<(i64, i64)> {
    FACTS.lock().unwrap_or_else(|p| p.into_inner()).get(job).copied()
}

pub fn keep_facts(job: &str, version: i64, started_ms: i64) {
    let mut all = FACTS.lock().unwrap_or_else(|p| p.into_inner());
    if all.len() > 5000 {
        all.clear();
    }
    all.insert(job.to_string(), (version, started_ms));
}

/// The servers asked about this run, placed or not: a server Roblox gave no address for isn't asked again and again
/// while the page finds the rest a few at a time.
static TRIED: LazyLock<Mutex<std::collections::HashSet<String>>> = LazyLock::new(|| Mutex::new(std::collections::HashSet::new()));

pub fn tried(job: &str) -> bool {
    TRIED.lock().unwrap_or_else(|p| p.into_inner()).contains(job)
}

pub fn mark_tried(job: &str) {
    let mut all = TRIED.lock().unwrap_or_else(|p| p.into_inner());
    if all.len() > 5000 {
        all.clear();
    }
    all.insert(job.to_string());
}

pub fn keep(job: &str, at: (String, String)) {
    let mut jobs = JOBS.lock().unwrap_or_else(|p| p.into_inner());
    if jobs.len() > 5000 {
        jobs.clear();
    }
    jobs.insert(job.to_string(), at);
}
