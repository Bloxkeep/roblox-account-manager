//! The interface's own small choices kept between runs (the web page kept them in localStorage: rm.statsTab,
//! rm.statsDays…): a little JSON file in this PC's Bloxkeep folder, never in the vault. A file that can't be read or
//! written just means the defaults.

use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

use serde_json::{Map, Value};

static FILE: LazyLock<PathBuf> = LazyLock::new(|| {
    std::env::var("BLOXKEEP_KEPT")
        .or_else(|_| std::env::var("BLOXKEEP_DATA"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into())).join("Bloxkeep"))
        .join("ui-prefs.json")
});

static ALL: LazyLock<Mutex<Map<String, Value>>> =
    LazyLock::new(|| Mutex::new(crate::net::saved("Interface choices", &FILE).unwrap_or_default()));

pub fn get(key: &str) -> Option<Value> {
    ALL.lock().ok()?.get(key).cloned()
}

/// Keeps it, written at once (these change only on a click).
pub fn set(key: &str, v: Value) {
    let Ok(mut all) = ALL.lock() else { return };
    if all.get(key) == Some(&v) {
        return;
    }
    all.insert(key.to_string(), v);
    if let Ok(text) = serde_json::to_string_pretty(&*all) {
        if let Some(dir) = FILE.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        crate::net::wrote("Interface choices", &FILE, std::fs::write(&*FILE, text));
    }
}
