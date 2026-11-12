// AppController.Feedback.cs: Settings › Bloxkeep › Send feedback: an idea or bug report with Bloxkeep's version, Windows' and
// the language, and on request the end of error.log, scrubbed first (`scrub`); Preview shows exactly what goes. A Cloudflare
// Worker (services/Feedback/worker) posts it to the developer's Discord channel and keeps that webhook to itself.
use std::cell::Cell;
use std::rc::Rc;
use std::sync::LazyLock;
use std::time::Duration;

use regex::{Regex, RegexBuilder};
use serde_json::{Value, json};

use super::App;
use super::controller::{bool_of, str_of};
use crate::core::base::{Fail, Res, Stamp};
use crate::core::models::stamp_min;
use crate::core::{l, rt, vault};

/// The feedback Worker (BLOXKEEP_FEEDBACK_URL points elsewhere: the checks' own little server).
pub const FEEDBACK_URL: &str = "https://bloxkeep-feedback.bloxkeep-app.workers.dev/feedback";
pub const FEEDBACK_TEXT_MAX: usize = 2000;
pub const FEEDBACK_CONTACT_MAX: usize = 100;
pub const FEEDBACK_LOG_MAX: usize = 12_000;
pub const FEEDBACK_GAP: Duration = Duration::from_secs(60);

/// No cookies, no redirects followed, 20 s at most (C#'s FeedbackHttp).
static FEEDBACK_HTTP: LazyLock<ureq::Agent> = LazyLock::new(|| ureq::AgentBuilder::new().redirects(0).timeout(Duration::from_secs(20)).build());
static COOKIE_TEXT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"_\|WARNING:-DO-NOT-SHARE-THIS[^\s"']*"#).expect("cookie pattern"));
static WEBHOOK_TEXT: LazyLock<Regex> =
    LazyLock::new(|| RegexBuilder::new(r"https?://(?:[a-z]+\.)?discord(?:app)?\.com/api/webhooks/\S+").case_insensitive(true).build().expect("webhook pattern"));
static EMAIL_TEXT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").expect("email pattern"));
static LONG_TOKEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z0-9+/=_-]{60,}").expect("token pattern"));

/// The instance fields AppController.Feedback.cs declares.
pub struct FeedbackState {
    pub feedback_sent_at: Cell<Stamp>,
    /// A send is on its way: the minute's gap counts only a finished one, so a second press meanwhile would send it twice.
    pub feedback_sending: Cell<bool>,
}

impl Default for FeedbackState {
    fn default() -> Self {
        FeedbackState { feedback_sent_at: Cell::new(stamp_min()), feedback_sending: Cell::new(false) }
    }
}

#[derive(Clone, Debug)]
pub struct Feedback {
    pub kind: String,
    pub text: String,
    pub contact: String,
    pub app: String,
    pub windows: String,
    pub language: String,
    pub log: Option<String>,
}

/// Windows' version numbers (Environment.OSVersion.Version, by RtlGetVersion: GetVersionEx lies without a manifest).
fn os_version() -> (u32, u32, u32) {
    #[repr(C)]
    struct OsVersionInfoW {
        size: u32,
        major: u32,
        minor: u32,
        build: u32,
        platform: u32,
        csd: [u16; 128],
    }
    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn RtlGetVersion(info: *mut OsVersionInfoW) -> i32;
    }
    let mut info = OsVersionInfoW { size: std::mem::size_of::<OsVersionInfoW>() as u32, major: 0, minor: 0, build: 0, platform: 0, csd: [0; 128] };
    // SAFETY: `info` is an OSVERSIONINFOW with its size set, as RtlGetVersion requires.
    if unsafe { RtlGetVersion(&mut info) } == 0 { (info.major, info.minor, info.build) } else { (0, 0, 0) }
}

pub fn windows_text() -> String {
    let (major, minor, build) = os_version();
    let name = if major == 10 { (if build >= 22000 { "11" } else { "10" }).to_string() } else { format!("{major}.{minor}") };
    format!("Windows {name} ({major}.{minor}.{build})")
}

/// File.ReadAllText: UTF-8 (a BOM dropped).
fn read_all_text(path: &std::path::Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    Ok(text.strip_prefix('\u{feff}').map(str::to_string).unwrap_or(text))
}

fn units(s: &str) -> usize {
    s.encode_utf16().count()
}

/// `s[^n..]`: its last `n` UTF-16 units.
fn last_units(s: &str, n: usize) -> String {
    let all: Vec<u16> = s.encode_utf16().collect();
    String::from_utf16_lossy(&all[all.len().saturating_sub(n)..])
}

/// What follows the first '\n'; None when there is none.
fn after_first_line(s: &str) -> Option<String> {
    s.find('\n').map(|at| s[at + 1..].to_string())
}

/// A letter, digit or '_' ([\p{L}\p{N}_]): what a name must not touch on either side to be taken out.
fn is_word_char(c: char) -> bool {
    c.is_alphabetic() || c.is_numeric() || c == '_'
}

/// Regex.Replace(text, (?<![\p{L}\p{N}_])word(?![\p{L}\p{N}_]), mark, IgnoreCase): the regex crate has no look-around,
/// so each match is checked by hand and, when touching a letter, the search goes on one character later (as .NET's does).
fn replace_word(text: &str, word: &str, mark: &str) -> String {
    let Ok(pattern) = RegexBuilder::new(&regex::escape(word)).case_insensitive(true).build() else {
        return text.to_string();
    };
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    let mut pos = 0;
    while pos <= text.len() {
        let Some(m) = pattern.find_at(text, pos) else {
            break;
        };
        let before_ok = !text[..m.start()].chars().next_back().is_some_and(is_word_char);
        let after_ok = !text[m.end()..].chars().next().is_some_and(is_word_char);
        if before_ok && after_ok && m.end() > m.start() {
            out.push_str(&text[copied..m.start()]);
            out.push_str(mark);
            copied = m.end();
            pos = m.end();
        } else {
            let step = text[m.start()..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
            pos = m.start() + step;
        }
    }
    out.push_str(&text[copied..]);
    out
}

impl App {
    pub fn read_feedback(&self, a: &Value) -> Res<Feedback> {
        let kind = if str_of(a, "kind") == "bug" { "bug" } else { "idea" };
        let text = str_of(a, "text").trim().to_string();
        let contact = str_of(a, "contact").trim().to_string();
        let mut log = None;
        if bool_of(a, "log") {
            // Account names are only known once the vault is open: without them the log can't go.
            if self.controller.vault.borrow().is_none() {
                return Err(Fail::user(l::t("Unlock Bloxkeep to add the error log: account names are taken out of it first.")));
            }
            log = self.scrubbed_log();
        }
        // Cut between whole characters: half an emoji at the limit made the request's JSON fail (2026-10-30 review).
        let app = self.updates.updater.borrow().current_version().to_string();
        Ok(Feedback {
            kind: kind.into(),
            text: App::cut_text(&text, FEEDBACK_TEXT_MAX),
            contact: App::cut_text(&contact, FEEDBACK_CONTACT_MAX),
            app,
            windows: windows_text(),
            language: l::language(),
            log,
        })
    }

    /// The end of error.log (whole lines) with what could tell who someone is taken out; None when there's none.
    pub fn scrubbed_log(&self) -> Option<String> {
        let path = vault::data_dir().join("error.log");
        let older = vault::data_dir().join("error.log.1");
        // IOException, UnauthorizedAccessException: no log.
        let mut text = if path.is_file() { read_all_text(&path).ok()? } else { String::new() };
        // A log that has just started over (error.log.1 holds the one before) would send next to nothing: the end of the
        // earlier one fills the rest.
        if units(&text) < FEEDBACK_LOG_MAX && older.is_file() {
            let mut before = read_all_text(&older).ok()?;
            let room = FEEDBACK_LOG_MAX - units(&text);
            if units(&before) > room {
                // Whole lines only.
                before = after_first_line(&last_units(&before, room)).unwrap_or_default();
            }
            text = before + &text;
        }
        if text.is_empty() {
            return None;
        }
        if units(&text) > FEEDBACK_LOG_MAX {
            text = last_units(&text, FEEDBACK_LOG_MAX);
            if let Some(rest) = after_first_line(&text) {
                text = rest;
            }
        }
        if text.trim().is_empty() { None } else { Some(self.scrub(&text)) }
    }

    /// Takes out what could tell whose a text is: each account's password, cookie, names and id, any cookies and
    /// Discord links, e-mail addresses, long tokens, and the Windows user and PC names.
    pub fn scrub(&self, text: &str) -> String {
        let mut words: Vec<(String, &'static str)> = Vec::new();
        let mut add = |word: &str, mark: &'static str| {
            // `word is { Length: >= 3 }`: UTF-16 units.
            if units(word) >= 3 {
                words.push((word.to_string(), mark));
            }
        };
        let accounts = self.controller.vault.borrow().as_ref().map(|v| v.data().accounts.clone()).unwrap_or_default();
        for account in &accounts {
            add(&account.password, "[password]");
            add(&account.cookie, "[cookie]");
            add(&account.username, "[account]");
            add(&account.display_name, "[account]");
            add(&account.alias, "[account]");
            if account.user_id > 0 {
                add(&account.user_id.to_string(), "[account id]");
            }
        }
        add(&std::env::var("USERNAME").unwrap_or_default(), "[user]");
        add(&std::env::var("COMPUTERNAME").unwrap_or_default(), "[pc]");
        let mut text = COOKIE_TEXT.replace_all(text, "[cookie]").into_owned();
        text = WEBHOOK_TEXT.replace_all(&text, "[webhook]").into_owned();
        text = EMAIL_TEXT.replace_all(&text, "[email]").into_owned();
        // Longest first, so a name that holds a shorter one isn't left half replaced (a stable sort, as OrderByDescending).
        words.sort_by_key(|w| std::cmp::Reverse(units(&w.0)));
        for (word, mark) in &words {
            text = replace_word(&text, word, mark);
        }
        LONG_TOKEN.replace_all(&text, "[token]").into_owned()
    }

    /// Exactly what Send would send, as the person reads it.
    pub fn feedback_preview(&self, a: &Value) -> Res<Value> {
        let f = self.read_feedback(a)?;
        let nl = "\r\n"; // Environment.NewLine
        let mut lines = String::new();
        lines += &(if f.kind == "bug" { l::t("Bug report") } else { l::t("Idea") });
        lines += nl;
        lines += &format!("Bloxkeep {} · {} · {}", f.app, f.windows, if f.language == "vi" { "Tiếng Việt" } else { "English" });
        lines += nl;
        if !f.contact.is_empty() {
            lines += &l::f("Contact: {0}", &[&f.contact]);
            lines += nl;
        }
        lines += nl;
        lines += if f.text.is_empty() { "…" } else { f.text.as_str() };
        lines += nl;
        lines += nl;
        match &f.log {
            None => lines += &l::t("(no error log)"),
            Some(log) => lines += &format!("── error-log.txt ──{nl}{}", log.trim_end()),
        }
        lines += nl;
        Ok(json!({ "preview": lines.trim_end() }))
    }

    pub async fn send_feedback(self: &Rc<Self>, a: &Value) -> Res<Value> {
        let f = self.read_feedback(a)?;
        if units(&f.text) < 3 {
            return Err(Fail::user(l::t("Write a few words first.")));
        }
        if self.feedback.feedback_sending.get() || Stamp::utc_now().secs_since(self.feedback.feedback_sent_at.get()) < FEEDBACK_GAP.as_secs_f64() {
            return Err(Fail::user(l::t("Feedback was just sent. Wait a minute before sending more.")));
        }
        self.feedback.feedback_sending.set(true);
        let result = self.post_feedback(f).await;
        self.feedback.feedback_sending.set(false);
        result?;
        Ok(json!({ "sent": true }))
    }

    pub async fn post_feedback(self: &Rc<Self>, f: Feedback) -> Res<()> {
        let url = std::env::var("BLOXKEEP_FEEDBACK_URL").ok().filter(|u| !u.is_empty()).unwrap_or_else(|| FEEDBACK_URL.to_string());
        // The C#'s anonymous object, its names in this order; a missing log is written as null.
        let body = format!(
            "{{\"kind\":{},\"text\":{},\"contact\":{},\"app\":{},\"windows\":{},\"language\":{},\"log\":{}}}",
            json!(f.kind),
            json!(f.text),
            json!(f.contact),
            json!(f.app),
            json!(f.windows),
            json!(f.language),
            json!(f.log)
        );
        // Ok(status) for any answer; Err when it couldn't be reached (HttpRequestException, TaskCanceledException).
        let answer = rt::blocking(move || match FEEDBACK_HTTP.post(&url).set("Content-Type", "application/json; charset=utf-8").send_string(&body) {
            Ok(res) => Ok(res.status()),
            Err(ureq::Error::Status(code, _)) => Ok(code),
            Err(e) => Err(e.to_string()),
        })
        .await;
        let status = answer.map_err(|_| Fail::user(l::t("Couldn't reach the feedback service. Check the internet connection and try again.")))?;
        if status == 429 {
            return Err(Fail::user(l::t("A lot of feedback is arriving right now. Try again in a minute.")));
        }
        if !(200..300).contains(&status) {
            return Err(Fail::user(l::f("Couldn't send it: the feedback service answered {0}. Try again later.", &[&status])));
        }
        self.feedback.feedback_sent_at.set(Stamp::utc_now());
        Ok(())
    }
}
