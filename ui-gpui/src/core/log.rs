//! Unexpected errors go to error.log in the data folder instead of crashing the app.
//! Only exception details are written — never account data. At 256 KB the log moves to error.log.1 (the earlier one of
//! those is replaced), so a bug report still has the lines from before.
use std::{
    io::Write,
    sync::{LazyLock, Mutex},
};

use regex::Regex;

const MAX_SIZE: u64 = 256 * 1024;
static SYNC: Mutex<()> = Mutex::new(());

/// Log.Write(Exception): the error's words (the C# wrote the exception's type and stack too; a Fail has its sentence).
pub fn write(ex: &dyn std::fmt::Display) {
    append(&ex.to_string());
}

/// A short diagnostic line (e.g. which kind of check Roblox asked for). Callers pass no account data.
pub fn note(text: &str) {
    append(text);
}

/// A last guard on what is written: a session cookie or a Discord webhook link that reached a message (an exception's
/// text can hold a request's address or a header) never lands in a file that is sent with a bug report and shown on request.
static SECRETS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)_\|WARNING:-DO-NOT-SHARE-THIS[^\s"']*|https?://(?:[a-z]+\.)?discord(?:app)?\.com/api/webhooks/\S+"#).expect("secrets pattern")
});

fn append(text: &str) {
    let text = SECRETS.replace_all(text, "[secret]");
    let _held = SYNC.lock().unwrap_or_else(|e| e.into_inner());
    // An IOException or UnauthorizedAccessException here: nowhere left to report it.
    let _ = (|| -> std::io::Result<()> {
        let dir = crate::core::vault::data_dir();
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("error.log");
        // At the limit the log is kept as error.log.1 (the one before replaced): a report needs the lines from before it.
        if let Ok(meta) = std::fs::metadata(&path) {
            if meta.len() > MAX_SIZE {
                std::fs::rename(&path, dir.join("error.log.1"))?;
            }
        }
        let c = super::base::Stamp::now().local();
        let line = format!("[{:04}-{:02}-{:02} {:02}:{:02}:{:02}] {}\r\n\r\n", c.year, c.month, c.day, c.hour, c.minute, c.second, text);
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
        file.write_all(line.as_bytes())
    })();
}
