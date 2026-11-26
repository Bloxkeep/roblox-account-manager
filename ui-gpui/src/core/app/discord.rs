// AppController.Discord.cs.
//! Settings › Discord: a channel webhook the person pastes gets a play-time summary (daily, weekly, and/or after a
//! scheduled preset), an optional status message edited in place, and the same events as the Windows notifications.
//! Opt-in, off by default; the link lives in the vault. Messages wait in a short in-memory queue that follows Discord's
//! rate limits and retries with a growing delay while offline; nothing is sent or kept waiting while locked. Only
//! account names, game names, times, counts, process ids and RAM go out, with Roblox's picture links of games and
//! accounts (not while names are hidden); never cookies, passwords or the link itself. Look: docs/discord-style.md.
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::rc::Rc;
use std::time::Duration;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::core::app::App;
use crate::core::app::discord_alerts::DiscordAlert;
use crate::core::app::discord_presence::DISCORD_APPLICATION_ID;
use crate::core::app::notify::{NOTIFY_KINDS, NOTIFY_REPEAT, notify_kind};
use crate::core::app::schedule::try_time;
use crate::core::app::stats::local_day_hour;
use crate::core::base::{Cancel, Fail, Guid, Res, Stamp};
use crate::core::discord_webhook::{self as webhook, DiscordField, DiscordFile, DiscordMessage, DiscordOutcome, DiscordPing, DiscordResult};
use crate::core::models::{DiscordSettings, LaunchPreset, stamp_min};
use crate::core::roblox_processes::PlayerProcess;
use crate::core::{l, log, prefs, rt};

/// Alerts that come together go as one message. 3 s, not 10: a user found a ban's alert slow (2026-10-21).
pub const DISCORD_BATCH: Duration = Duration::from_secs(3);
/// A summary for a scheduled preset's stop waits for its clients to close, so their stays are counted whole.
pub const DISCORD_AFTER_STOP: Duration = Duration::from_secs(30);
pub const DISCORD_MAX_BACKOFF: Duration = Duration::from_secs(10 * 60);
pub const DISCORD_QUEUE_MAX: usize = 50;
pub const MAX_BOARD_ACCOUNTS: usize = 24;
pub const DISCORD_PARTS: [&str; 5] = ["total", "accounts", "games", "sessions", "kicks"];
pub const DISCORD_BOARD_MINUTES: [i32; 6] = [2, 5, 10, 15, 30, 60];
/// A summary sent more than this after its time says it is late.
pub const DISCORD_LATE: Duration = Duration::from_secs(10 * 60);

/// `new DiscordMessage(title, text, fields, footer, color, timestamp)`: the C# record's positional parts in one call.
pub fn discord_message(title: impl Into<String>, text: impl Into<String>, fields: Vec<DiscordField>, footer: Option<String>, color: i32, timestamp: Option<Stamp>) -> DiscordMessage {
    let mut m = DiscordMessage::new(title, text, fields);
    m.footer = footer;
    m.color = color;
    m.timestamp = timestamp;
    m
}

/// `kind`: "events", "summary" or "board" (at most one board update waits; a newer one replaces it).
/// `id`: which item this is (C# told them apart by reference).
#[derive(Clone)]
pub struct DiscordItem {
    pub kind: String,
    pub message: DiscordMessage,
    pub ping: DiscordPing,
    pub daily_day: Option<String>,
    pub weekly_day: Option<String>,
    pub route: Option<Guid>,
    pub id: u64,
}

/// `status`: "ingame", "joining", "menu", "disconnected" or "signedout" (see Connection).
/// `reason`: Roblox's code for why a window was cut off, when its log gave one.
/// `banned`: the game banned the account: put out of it, or stuck on Join Error 600 on the way in.
/// `queue`: its place in a full server's line, when Roblox gave one.
#[derive(Clone, Debug)]
pub struct BoardRow {
    pub name: String,
    pub place_id: Option<i64>,
    pub game: Option<String>,
    pub status: String,
    pub seconds: i64,
    pub pid: i32,
    pub ram: i64,
    pub reason: Option<i32>,
    pub banned: bool,
    pub phase: Option<String>,
    pub queue: Option<i32>,
}

pub struct DiscordState {
    pub queue: RefCell<Vec<DiscordItem>>,
    pub events: RefCell<Vec<(String, DiscordAlert, DiscordPing, Option<Guid>)>>,
    /// When each event type was last queued per account (like the Windows notifications: a rejoin loop can't spam).
    pub notified: RefCell<HashMap<String, Stamp>>,
    /// The vault the queue belongs to (`vault_token`): after a lock (or another vault) whatever waited is dropped.
    pub vault: Cell<Option<usize>>,
    pub batch_due: Cell<Stamp>,
    pub next_try: Cell<Stamp>,
    pub next_board: Cell<Stamp>,
    pub last_sent: Cell<Option<Stamp>>,
    /// Summaries after a scheduled preset's stop, by preset: each one covers that preset's accounts.
    pub summaries_due: RefCell<HashMap<Guid, Stamp>>,
    /// A status message id that came back after the vault locked, handed over at the next unlock. The link is a
    /// vault secret: only its hash (`webhook_key`) stays in memory while locked.
    pub board_posted: RefCell<Option<(String, String)>>,
    /// Summaries that went out after the vault locked (their days); noted as sent at the next unlock.
    pub summaries_sent: RefCell<Vec<(String, Option<String>, Option<String>)>>,
    pub error: RefCell<Option<String>>,
    pub sending: Cell<bool>,
    pub failures: Cell<i32>,
    pub dropped: Cell<i32>,
    pub next_item: Cell<u64>,
}

impl Default for DiscordState {
    fn default() -> DiscordState {
        DiscordState {
            queue: RefCell::default(),
            events: RefCell::default(),
            notified: RefCell::default(),
            vault: Cell::new(None),
            batch_due: Cell::new(stamp_min()),
            next_try: Cell::new(stamp_min()),
            next_board: Cell::new(stamp_min()),
            last_sent: Cell::new(None),
            summaries_due: RefCell::default(),
            board_posted: RefCell::default(),
            summaries_sent: RefCell::default(),
            error: RefCell::default(),
            sending: Cell::new(false),
            failures: Cell::new(0),
            dropped: Cell::new(0),
            next_item: Cell::new(0),
        }
    }
}

/// "yyyy-MM-dd" of a local moment.
pub fn day_key(local: Stamp) -> String {
    let c = local.local();
    format!("{:04}-{:02}-{:02}", c.year, c.month, c.day)
}

/// The latest local time at or before `local` a daily (no weekday) or weekly summary was due.
pub fn last_summary_due(local: Stamp, time: &str, weekday: Option<i32>) -> Option<Stamp> {
    let at = try_time(time)?;
    // The time on the clock that day (as the C#'s local.Date + at), not seconds after midnight: on a day the clocks
    // change, "09:00" would otherwise come at 10:00 or 08:00. Noon names the day, as midnight may not exist.
    let on_day = |days: i64| {
        let c = local_day_hour(local, days, 12).local();
        Stamp::from_local(c.year, c.month, c.day, (at / 3600) as u32, (at % 3600 / 60) as u32, 0)
    };
    let mut back = 0i64;
    if on_day(0) > local {
        back = -1;
    }
    let mut due = on_day(back);
    if let Some(w) = weekday {
        // A weekday from an old vault or backup isn't checked: one outside 0..=6 has no due day.
        if !(0..=6).contains(&w) {
            return None;
        }
        while due.local().weekday as i32 != w {
            back -= 1;
            due = on_day(back);
        }
    }
    Some(due)
}

/// The ping chosen for a kind of message, or none during the quiet hours.
pub fn ping_for(d: &DiscordSettings, kind: &str) -> DiscordPing {
    let ping = DiscordPing::parse(d.pings.get(kind).map(String::as_str)).unwrap_or_else(DiscordPing::none);
    if ping.any() && in_quiet_hours(d, Stamp::now()) { DiscordPing::none() } else { ping }
}

pub fn in_quiet_hours(d: &DiscordSettings, local: Stamp) -> bool {
    let (Some(from), Some(to)) = (try_time(&d.quiet_from), try_time(&d.quiet_to)) else { return false };
    if !d.quiet || from == to {
        return false;
    }
    let c = local.local();
    let now = c.hour as i64 * 3600 + c.minute as i64 * 60 + c.second as i64;
    if from < to { now >= from && now < to } else { now >= from || now < to } // across midnight (23:00–07:00)
}

/// A webhook link's hash: enough to tell it is the same link, without keeping the link itself.
pub fn webhook_key(url: &str) -> String {
    Sha256::digest(url.as_bytes()).iter().map(|b| format!("{b:02X}")).collect()
}

pub fn discord_offline_text(result: &DiscordResult) -> String {
    match &result.error {
        Some(error) => l::f("Couldn't reach Discord: {0}", &[error]),
        None => l::t("Couldn't reach Discord. Check your internet connection."),
    }
}

/// .NET's TimeSpan.TotalSeconds rounded up (Math.Ceiling).
fn ceil_secs(d: Duration) -> f64 {
    d.as_secs_f64().ceil()
}

/// The text after Discord's "That isn't a Discord webhook link." refusal.
fn not_a_webhook() -> Fail {
    Fail::user(l::t(
        "That isn't a Discord webhook link. It looks like https://discord.com/api/webhooks/… (Channel settings › Integrations › Webhooks › Copy Webhook URL).",
    ))
}

/// Uri.TryCreate(absolute) with the https scheme: "https://" and a host after it.
fn is_https_link(text: &str) -> bool {
    text.len() > 8 && text.get(..8).is_some_and(|p| p.eq_ignore_ascii_case("https://")) && !text[8..].starts_with(['/', '?', '#'])
}

impl App {
    /// RAM in the Discord messages' language ("1.7 GB", "1,7 ГБ"): lang.rs › mb.
    fn discord_mb(&self, mb: f64) -> String {
        crate::lang::by_code(&self.discord_language()).unwrap_or_else(crate::lang::cur).mb(mb)
    }

    pub fn discord_language(&self) -> String {
        match self.vault(|d| d.discord.language.clone()).ok().and_then(|l| crate::lang::by_code(&l)) {
            Some(l) => l.code.into(),
            None => l::language(),
        }
    }

    /// L.T in Discord's language.
    pub fn dt(&self, english: &str) -> String {
        l::t_in(english, &self.discord_language())
    }

    /// string.Format of `dt(english)`.
    pub fn df(&self, english: &str, args: &[&dyn Display]) -> String {
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        l::format(&self.dt(english), &args)
    }

    /// `df` about a count of `n`: the form it takes in Discord's language (l::counted_in). A sentence English has only
    /// a plural for gives it twice.
    pub fn dp(&self, n: impl TryInto<i64>, one: &str, many: &str, args: &[&dyn Display]) -> String {
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        l::format(&self.dpt(n, one, many), &args)
    }

    /// A count's word alone in Discord's language ("Account" / "Accounts"), the form `n` takes there.
    pub fn dpt(&self, n: impl TryInto<i64>, one: &str, many: &str) -> String {
        l::counted_in(one, many, n.try_into().unwrap_or(i64::MAX), &self.discord_language())
    }

    /// The vault is open and the screen not locked (C#'s `Open`).
    fn discord_open(&self) -> bool {
        !self.auto_lock.screen_locked.get() && self.has_open_vault()
    }

    pub fn discord_dto(&self) -> Value {
        if !self.discord_open() {
            return Value::Null;
        }
        let Ok(d) = self.vault(|d| d.discord.clone()) else { return Value::Null };
        json!({
            "enabled": d.enabled,
            // Show on my Discord profile: this PC's choice (Prefs), offered once Bloxkeep's Discord application exists.
            "presence": prefs::lock().discord_presence,
            "presenceReady": !DISCORD_APPLICATION_ID.is_empty(),
            "hasWebhook": !d.webhook_url.is_empty(),
            "webhookHint": webhook::mask(&d.webhook_url),
            "stopped": d.stopped,
            "username": d.username,
            "avatarUrl": d.avatar_url,
            "language": d.language,
            "style": d.style,
            "hideNames": d.hide_names,
            "summary": {
                "daily": d.summary_daily, "dailyTime": d.summary_daily_time, "weekly": d.summary_weekly, "weekday": d.summary_weekday,
                "weeklyTime": d.summary_weekly_time, "afterSchedule": d.summary_after_schedule, "days": d.summary_days,
                "parts": d.summary_parts, "accounts": d.summary_accounts,
            },
            "board": { "enabled": d.board, "minutes": d.board_minutes },
            "events": d.events.iter().filter(|e| NOTIFY_KINDS.contains(&e.as_str())).collect::<Vec<_>>(),
            "pings": d.pings,
            "quiet": { "enabled": d.quiet, "from": d.quiet_from, "to": d.quiet_to },
            "lastSent": self.discord.last_sent.get(),
            "error": self.discord.error.borrow().clone(),
            "queued": self.discord.queue.borrow().len() + if self.discord.events.borrow().is_empty() { 0 } else { 1 },
        })
    }

    // ───────────────────────────── settings ─────────────────────────────

    /// Any of the Discord fields; only the ones sent change. Each is checked before anything is kept.
    pub fn set_discord(&self, a: &Value) -> Res<Value> {
        let (d, known) = self.vault(|v| (v.discord.clone(), v.accounts.iter().map(|x| x.id).collect::<HashSet<Guid>>()))?;
        let local = Stamp::now();
        let opt_str = |name: &str| a.get(name).and_then(Value::as_str).map(str::to_string);
        let opt_bool = |name: &str| a.get(name).and_then(Value::as_bool);
        let number = |name: &str| a.get(name).filter(|v| v.is_number()).map(|v| v.as_i64().unwrap_or(0) as i32);
        let time = |name: &str, current: &str| -> Res<String> {
            match opt_str(name) {
                None => Ok(current.to_string()),
                Some(text) if try_time(&text).is_none() => Err(Fail::user(l::t("Enter the time as HH:mm."))),
                Some(text) => Ok(text),
            }
        };
        let daily_time = time("summaryDailyTime", &d.summary_daily_time)?;
        let weekly_time = time("summaryWeeklyTime", &d.summary_weekly_time)?;
        let quiet_from = time("quietFrom", &d.quiet_from)?;
        let quiet_to = time("quietTo", &d.quiet_to)?;
        let mut username = opt_str("username").map(|u| u.trim().to_string());
        if let Some(name) = username.as_mut() {
            if name.is_empty() {
                *name = "Bloxkeep".into();
            }
            // Discord refuses these names for a webhook.
            let lower = name.to_lowercase();
            if name.encode_utf16().count() > webhook::MAX_USERNAME as usize || lower.contains("discord") || lower.contains("clyde") {
                return Err(Fail::user(l::t("Discord doesn't allow that name (up to 80 characters, without \"discord\" or \"clyde\").")));
            }
        }
        let avatar = opt_str("avatarUrl").map(|u| u.trim().to_string());
        if let Some(avatar) = avatar.as_ref().filter(|a| !a.is_empty()) {
            if avatar.encode_utf16().count() > 2048 || !is_https_link(avatar) || avatar.chars().any(char::is_whitespace) {
                return Err(Fail::user(l::t("The picture link must start with https://.")));
            }
        }
        let language = opt_str("language");
        if !language.as_deref().is_none_or(|l| l == "app" || crate::lang::by_code(l).is_some()) {
            return Err(Fail::user(l::t("Unknown language.")));
        }
        let style = opt_str("style");
        if !matches!(style.as_deref(), None | Some("embed" | "plain")) {
            return Err(Fail::user(l::t("Unknown message style.")));
        }
        let mut parts: Option<Vec<String>> = None;
        if let Some(list) = a.get("summaryParts").and_then(Value::as_array) {
            let mut picked: Vec<String> = Vec::new();
            for e in list {
                let s = e.as_str().unwrap_or("");
                if DISCORD_PARTS.contains(&s) && !picked.iter().any(|p| p == s) {
                    picked.push(s.to_string());
                }
            }
            if picked.is_empty() {
                return Err(Fail::user(l::t("Pick at least one thing for the summary.")));
            }
            parts = Some(picked);
        }
        let days = number("summaryDays");
        if !matches!(days, None | Some(1 | 7 | 30)) {
            return Err(Fail::user(l::t("Unknown period.")));
        }
        let weekday = number("summaryWeekday");
        if weekday.is_some_and(|w| !(0..=6).contains(&w)) {
            return Err(Fail::user(l::t("Unknown weekday.")));
        }
        let minutes = number("boardMinutes");
        if minutes.is_some_and(|m| !DISCORD_BOARD_MINUTES.contains(&m)) {
            return Err(Fail::user(l::t("Update the status message every 2 to 60 minutes.")));
        }
        // {event: {kind, enabled}} and {ping: {kind, value}}: one switch or one ping choice at a time.
        let mut ev: Option<(String, bool)> = None;
        if let Some(e) = a.get("event").filter(|e| e.is_object()) {
            let kind = e.get("kind").and_then(Value::as_str).unwrap_or("").to_string();
            if !NOTIFY_KINDS.contains(&kind.as_str()) {
                return Err(Fail::user(l::t("Unknown notification kind.")));
            }
            ev = Some((kind, e.get("enabled") == Some(&Value::Bool(true))));
        }
        let mut ping: Option<(String, String)> = None;
        if let Some(p) = a.get("ping").filter(|p| p.is_object()) {
            let kind = p.get("kind").and_then(Value::as_str).unwrap_or("").to_string();
            let value = p.get("value").and_then(Value::as_str).unwrap_or("").trim().to_string();
            if kind != "summary" && !NOTIFY_KINDS.contains(&kind.as_str()) {
                return Err(Fail::user(l::t("Unknown notification kind.")));
            }
            if DiscordPing::parse(Some(&value)).is_none() {
                return Err(Fail::user(if value.starts_with("role:") {
                    l::t("A role ID is 17 to 20 digits. Copy it with Developer Mode on: right-click the role › Copy Role ID.")
                } else {
                    l::t("A user ID is 17 to 20 digits. Copy it with Developer Mode on: right-click the person › Copy User ID.")
                }));
            }
            ping = Some((kind, value));
        }
        if opt_bool("enabled") == Some(true) && d.webhook_url.is_empty() {
            return Err(Fail::user(l::t("Paste a webhook link first.")));
        }

        // A summary (or sending) switched on starts at its next time, not with one already past: the latest due counts as sent.
        let latest_day = |time: &str, day: Option<i32>| last_summary_due(local, time, day).map(day_key).unwrap_or_default();
        let summary_accounts: Option<Vec<Guid>> = a.get("summaryAccounts").and_then(Value::as_array).map(|list| {
            let mut out: Vec<Guid> = Vec::new();
            for e in list {
                let g = e.as_str().and_then(|s| Guid::parse_str(s).ok()).unwrap_or(Guid::nil());
                if known.contains(&g) && !out.contains(&g) {
                    out.push(g);
                }
            }
            out
        });
        let enabled_arg = opt_bool("enabled");
        let board_arg = opt_bool("board");
        let enabled_now = self.vault_mut(|v| {
            let d = &mut v.discord;
            let mut switched_on = false;
            if let Some(enabled) = enabled_arg {
                switched_on = enabled && !d.enabled;
                d.enabled = enabled;
                if enabled {
                    d.stopped = None;
                }
            }
            if let Some(u) = username.clone() {
                d.username = u;
            }
            if let Some(av) = avatar.clone() {
                d.avatar_url = av;
            }
            if let Some(lang) = language.clone() {
                d.language = lang;
            }
            if let Some(st) = style.clone() {
                d.style = st;
            }
            if let Some(hide) = opt_bool("hideNames") {
                d.hide_names = hide;
            }
            if let Some(daily) = opt_bool("summaryDaily") {
                d.summary_daily = daily;
            }
            if switched_on || opt_bool("summaryDaily") == Some(true) || opt_str("summaryDailyTime").is_some() {
                d.last_daily = latest_day(&daily_time, None);
            }
            d.summary_daily_time = daily_time.clone();
            if let Some(weekly) = opt_bool("summaryWeekly") {
                d.summary_weekly = weekly;
            }
            if let Some(w) = weekday {
                d.summary_weekday = w;
            }
            if switched_on || opt_bool("summaryWeekly") == Some(true) || opt_str("summaryWeeklyTime").is_some() || weekday.is_some() {
                d.last_weekly = latest_day(&weekly_time, Some(d.summary_weekday));
            }
            d.summary_weekly_time = weekly_time.clone();
            if let Some(after) = opt_bool("summaryAfterSchedule") {
                d.summary_after_schedule = after;
            }
            if let Some(n) = days {
                d.summary_days = n;
            }
            if let Some(p) = parts.clone() {
                d.summary_parts = p;
            }
            if let Some(list) = summary_accounts.clone() {
                d.summary_accounts = list;
            }
            if let Some(board) = board_arg {
                d.board = board;
            }
            if let Some(m) = minutes {
                d.board_minutes = m;
            }
            if let Some((kind, on)) = &ev {
                d.events.retain(|e| e != kind);
                if *on {
                    d.events.push(kind.clone());
                }
            }
            if let Some((kind, value)) = &ping {
                if value.is_empty() || value == "none" {
                    d.pings.shift_remove(kind);
                } else {
                    d.pings.insert(kind.clone(), value.clone());
                }
            }
            if let Some(quiet) = opt_bool("quiet") {
                d.quiet = quiet;
            }
            d.quiet_from = quiet_from.clone();
            d.quiet_to = quiet_to.clone();
            d.enabled
        })?;
        if board_arg.is_some() {
            self.discord.next_board.set(Stamp::utc_now()); // the first update right away
        }
        if !enabled_now {
            self.reset_discord_queue();
        }
        self.persist_soon();
        self.push_state();
        Ok(Value::Null)
    }

    /// A new link (checked strictly, kept as discord.com/api/webhooks/{id}/{token}), or "" to forget it.
    pub fn set_discord_webhook(&self, url: &str) -> Res<Value> {
        self.vault(|_| ())?;
        let url = url.trim();
        let normalized = if url.is_empty() { None } else { Some(webhook::try_normalize(Some(&url[..])).ok_or_else(not_a_webhook)?) };
        self.vault_mut(|v| {
            let d = &mut v.discord;
            match &normalized {
                None => {
                    d.webhook_url = String::new();
                    d.enabled = false;
                }
                Some(n) => d.webhook_url = n.clone(),
            }
            // Another webhook (or none): the old status message isn't this one's to edit, and nothing queued goes to it.
            d.board_message_id = String::new();
            d.stopped = None;
        })?;
        *self.discord.error.borrow_mut() = None;
        self.discord.last_sent.set(None);
        self.reset_discord_queue();
        self.persist_soon();
        self.push_state();
        Ok(Value::Null)
    }

    pub fn reveal_discord_webhook(&self) -> Res<Value> {
        Ok(Value::String(self.vault(|d| d.discord.webhook_url.clone())?))
    }

    // ───────────────────────────── what goes out ─────────────────────────────

    /// From Notify, right after L.F made `text`: an event of a kind left on joins the
    /// next message, rebuilt from the same sentence and values (discord_alerts.rs).
    pub fn notify_discord(&self, text: &str, type_: &str, account: Guid) {
        let made = l::made(text);
        // Made now, while L.F's last sentence is still this one (it needs that).
        let discord_text = self.discord_text(text);
        if self.controller.disposed.get() {
            return;
        }
        let Ok(d) = self.vault(|v| v.discord.clone()) else { return };
        if !d.enabled || d.webhook_url.is_empty() {
            return;
        }
        let now = Stamp::utc_now();
        self.sync_discord_vault(now);
        // A schedule's message names its preset (account is the preset's id then).
        let preset = if type_.starts_with("schedule") { Some(account) } else { self.preset_of_account(account) };
        if type_ == "scheduleStop" && d.summary_after_schedule {
            self.discord.summaries_due.borrow_mut().insert(account, now.plus_secs(DISCORD_AFTER_STOP.as_secs_f64()));
        }
        // A macro's alert (its alert step, or onFail "alert") is what the person asked Discord for: no switch of its own.
        let kind: Option<String> = if type_ == "macro" { Some("macro".into()) } else { notify_kind(type_).map(str::to_string) };
        let Some(kind) = kind else { return };
        if kind != "macro" && !d.events.contains(&kind) {
            return;
        }
        let key = format!("{type_}:{account}");
        // A time ahead of now (the clock was set back) doesn't hold the event back.
        if let Some(last) = self.discord.notified.borrow().get(&key).copied() {
            if last <= now && now.secs_since(last) < NOTIFY_REPEAT.as_secs_f64() {
                return;
            }
        }
        {
            let mut notified = self.discord.notified.borrow_mut();
            if notified.len() > 256 {
                notified.retain(|_, at| now.secs_since(*at) < NOTIFY_REPEAT.as_secs_f64());
            }
            notified.insert(key, now);
        }
        if self.discord.events.borrow().is_empty() {
            self.discord.batch_due.set(now.plus_secs(DISCORD_BATCH.as_secs_f64()));
        }
        // A game's ban goes at once (a user wants it told right away), with whatever is waiting already.
        if type_ == "gameBan" {
            self.discord.batch_due.set(now);
        }
        // A ban goes with the sign-in events (their switch and ping) under a title of its own.
        let alert = self.alert_for(type_, account, made.as_ref(), &discord_text, now);
        let event_kind = if type_ == "banned" { type_.to_string() } else { kind.clone() };
        let ping = ping_for(&d, &kind);
        let route = self.route_for(preset);
        self.discord.events.borrow_mut().push((event_kind, alert, ping, route));
    }

    /// The preset whose window the account's is: opened by it, being opened, or waiting to rejoin for it.
    pub fn preset_of_account(&self, account: Guid) -> Option<Guid> {
        if let Some(id) = self.controller.running.borrow().get(&account).and_then(|(_, launch)| launch.preset_id) {
            return Some(id);
        }
        let from_rejoin = self.controller.rejoin_queue.borrow().iter().find(|r| r.account_id == account).and_then(|r| r.preset_id);
        from_rejoin.or_else(|| self.controller.pending.borrow().iter().find(|p| p.account_id == account).and_then(|p| p.preset_id))
    }

    /// A preset with a webhook of its own; None (Settings' webhook) for any other.
    pub fn route_for(&self, preset: Option<Guid>) -> Option<Guid> {
        self.preset_for(preset).filter(|p| !p.discord_webhook.is_empty()).map(|p| p.id)
    }

    /// Where a message goes: its preset's own webhook, or Settings' one (also once the preset's is removed).
    pub fn url_for(&self, d: &DiscordSettings, route: Option<Guid>) -> String {
        match self.preset_for(route).filter(|p| !p.discord_webhook.is_empty()) {
            Some(p) => p.discord_webhook,
            None => d.webhook_url.clone(),
        }
    }

    /// A preset's own webhook changed: what waited for the old one isn't sent to it.
    pub fn discord_route_changed(&self, preset: Guid) {
        self.discord.queue.borrow_mut().retain(|i| i.route != Some(preset));
        self.discord.events.borrow_mut().retain(|e| e.3 != Some(preset));
    }

    pub fn event_title(&self, kind: &str) -> String {
        match kind {
            "drop" => self.dt("Account dropped"),
            "schedule" => self.dt("Scheduled preset"),
            "signin" => self.dt("Sign-in needs renewing"),
            "banned" => self.dt("Account banned"),
            "ram" => self.dt("Low RAM"),
            "macro" => self.dt("Macro alert"),
            _ => self.dt("Launch problem"),
        }
    }

    pub fn duration(&self, seconds: i64) -> String {
        // C#'s long division truncates toward zero, as Rust's.
        let minutes = seconds / 60;
        if minutes < 60 { self.df("{0} min", &[&minutes]) } else { self.df("{0} h {1} min", &[&(minutes / 60), &(minutes % 60)]) }
    }

    /// When a summary is due, neither sent (`last` is its last sent day) nor queued; else None.
    /// A day before the last sent one (clock set back) isn't due again.
    pub fn due_summary(&self, on: bool, time: &str, weekday: Option<i32>, last: &str, local: Stamp, queued_day: fn(&DiscordItem) -> Option<&String>) -> Option<Stamp> {
        if !on {
            return None;
        }
        let at = last_summary_due(local, time, weekday)?;
        let day = day_key(at);
        // string.CompareOrdinal over these ASCII days is the bytes' order.
        let queued = self.discord.queue.borrow().iter().any(|i| queued_day(i) == Some(&day));
        if day.as_str() > last && !queued { Some(at) } else { None }
    }

    pub fn board_message(&self, d: &DiscordSettings, players: &[PlayerProcess]) -> DiscordMessage {
        let ram: HashMap<i32, i64> = players.iter().map(|p| (p.pid, p.ram())).collect();
        let now = Stamp::utc_now();
        let running: Vec<(Guid, i32, Rc<crate::core::app::controller::PendingLaunch>)> =
            self.controller.running.borrow().iter().map(|(id, (pid, launch))| (*id, *pid, launch.clone())).collect();
        let mut with_accounts: Vec<_> = running.into_iter().filter_map(|(id, pid, launch)| self.find_account(id).map(|a| (a, id, pid, launch))).collect();
        // PORT: StringComparer.CurrentCultureIgnoreCase: compared here by lower case.
        with_accounts.sort_by_key(|r| r.0.label().to_lowercase());
        let rows: Vec<BoardRow> = with_accounts
            .iter()
            .map(|(account, id, pid, launch)| {
                let log = self.controller.logs.borrow().state_of(*pid);
                let place_id = log.as_ref().and_then(|l| l.place_id).or_else(|| launch.target.as_ref().map(|t| t.place_id));
                let since = self.stats.playing.borrow().get(id).map(|s| s.started_at).unwrap_or(launch.started_at.get());
                let disconnected = log.as_ref().is_some_and(|l| l.status == "disconnected");
                BoardRow {
                    name: self.discord_name(account),
                    place_id,
                    game: place_id.map(|p| self.game_name(p, None)),
                    status: self.connection(account, *pid, launch).to_string(),
                    seconds: now.secs_since(since).max(0.0) as i64,
                    pid: *pid,
                    ram: ram.get(pid).copied().unwrap_or(0),
                    reason: if disconnected { log.as_ref().and_then(|l| l.reason) } else { None },
                    banned: (disconnected && log.as_ref().is_some_and(|l| l.banned_by_game)) || self.banned_window(account, &(*pid, launch.clone())).is_some(),
                    phase: self.phase(*pid),
                    queue: self.join_answer_of(*pid).and_then(|j| j.queue_position),
                }
            })
            .collect();
        self.board_message_of(d.board_minutes, &rows, now)
    }

    /// The status message from its rows: how many in a game in the title (green when all are, yellow when one is
    /// cut off), the others' state and Roblox's RAM below, a field per account.
    pub fn board_message_of(&self, minutes: i32, rows: &[BoardRow], now: Stamp) -> DiscordMessage {
        let footer = self.df("Bloxkeep · Board · updates every {0} min", &[&minutes]);
        if rows.is_empty() {
            let mut m = discord_message(self.dt("Nobody is playing"), self.dt("No Roblox window is open."), Vec::new(), Some(footer), webhook::GREY, Some(now));
            m.thumbnail = Some(webhook::LOGO.to_string());
            return m;
        }
        let status = |status: &str, start: bool| -> String {
            match (status, start) {
                ("ingame", true) => self.dt("In game"),
                ("ingame", false) => self.dt("in game"),
                ("joining", true) => self.dt("Joining"),
                ("joining", false) => self.dt("joining"),
                ("menu", true) => self.dt("In the menu"),
                ("menu", false) => self.dt("in the menu"),
                ("signedout", true) => self.dt("Signed out"),
                ("signedout", false) => self.dt("signed out"),
                (_, true) => self.dt("Disconnected"),
                _ => self.dt("disconnected"),
            }
        };
        // A window cut off says why, as its card does: never a lost connection when Roblox didn't say so.
        let said = |r: &BoardRow| -> String {
            if r.banned {
                self.dt("Banned by the game")
            } else if r.phase.as_deref() == Some("teleporting") {
                self.dt("Teleporting")
            } else if r.phase.as_deref() == Some("full") {
                match r.queue {
                    Some(place) => self.df("Server full · #{0} in line", &[&place]),
                    None => self.dt("Server full, waiting"),
                }
            } else if r.phase.as_deref() == Some("waiting") {
                self.dt("Waiting for a server")
            } else if r.phase.as_deref() == Some("joinFailed") {
                self.dt("Didn't get into the game")
            } else if r.status != "disconnected" {
                status(&r.status, true)
            } else if let Some(code) = r.reason {
                crate::core::app::discord_alerts::capital(&self.kick_words(code).unwrap_or_else(|| self.df("Disconnected · code {0}", &[&code])))
            } else {
                self.dt("Disconnected, reason unknown")
            }
        };
        let mb = crate::core::app::controller::MB;
        let mut fields: Vec<DiscordField> = rows
            .iter()
            .take(MAX_BOARD_ACCOUNTS)
            .map(|r| {
                let game = match r.place_id {
                    Some(id) => webhook::game_link(&r.game.clone().unwrap_or_else(|| self.df("Place {0}", &[&id])), Some(id)),
                    None => self.dt("No game"),
                };
                let ram = if r.ram > 0 { format!(" · {}", self.discord_mb((r.ram / mb) as f64)) } else { String::new() };
                DiscordField::new(webhook::escape(Some(&r.name[..])), format!("{game}\n{} · {}\nPID `{}`{ram}", said(r), self.duration(r.seconds), r.pid), true)
            })
            .collect();
        if rows.len() > MAX_BOARD_ACCOUNTS {
            fields.push(DiscordField::new("…", self.df("and {0} more", &[&(rows.len() - MAX_BOARD_ACCOUNTS)]), true));
        }
        let in_game = rows.iter().filter(|r| r.status == "ingame").count();
        let cut_off = rows.iter().any(|r| r.status == "disconnected" || r.status == "signedout");
        let title = if in_game == rows.len() {
            self.dp(rows.len(), "{0} account in game", "{0} accounts in game", &[&rows.len()])
        } else {
            self.df("{0} of {1} accounts in game", &[&in_game, &rows.len()])
        };
        let mut lines: Vec<String> = Vec::new();
        let mut groups: Vec<(String, usize)> = Vec::new();
        for r in rows.iter().filter(|r| r.status != "ingame") {
            match groups.iter_mut().find(|g| g.0 == r.status) {
                Some(g) => g.1 += 1,
                None => groups.push((r.status.clone(), 1)),
            }
        }
        groups.sort_by_key(|g| match g.0.as_str() {
            "joining" => 2,
            "menu" => 3,
            "signedout" => 1,
            _ => 0,
        });
        let others: Vec<String> = groups.iter().map(|(key, n)| format!("**{n}** {}", status(key, false))).collect();
        if !others.is_empty() {
            lines.push(others.join(" · "));
        }
        let ram: i64 = rows.iter().map(|r| r.ram).sum();
        if ram > 0 {
            let amount = self.discord_mb((ram / mb) as f64);
            lines.push(format!("-# {}", self.df("Roblox is using {0} of RAM", &[&amount])));
        }
        // No logo beside the accounts: their three columns get the whole width.
        let color = if cut_off {
            webhook::YELLOW
        } else if in_game == rows.len() {
            webhook::GREEN
        } else {
            webhook::BLUE
        };
        discord_message(title, lines.join("\n"), fields, Some(footer), color, Some(now))
    }

    /// The status message's last words when Bloxkeep locks or closes: it would say "3 in game" for good otherwise.
    pub fn final_board_message(&self, closing: bool) -> DiscordMessage {
        let mut m = discord_message(
            if closing { self.dt("Bloxkeep was closed") } else { self.dt("Bloxkeep is locked") },
            if closing { self.dt("This board updates again when you open Bloxkeep and unlock it.") } else { self.dt("This board updates again when you unlock Bloxkeep.") },
            Vec::new(),
            Some(self.dt("Bloxkeep · Board")),
            webhook::GREY,
            Some(Stamp::utc_now()),
        );
        m.thumbnail = Some(webhook::LOGO.to_string());
        m
    }

    /// "Send test": Bloxkeep can post here; a preset's says its messages come here.
    pub fn test_message(&self, preset: Option<&LaunchPreset>, for_preset: bool) -> DiscordMessage {
        let mut m = discord_message(
            self.dt("Bloxkeep is connected"),
            if for_preset {
                self.dt("Bloxkeep can post to this channel. This preset's messages will come here.")
            } else {
                self.dt("Bloxkeep can post to this channel. Choose what it sends under Settings › Discord.")
            },
            Vec::new(),
            Some(self.dt("Bloxkeep · Test")),
            webhook::GREEN,
            Some(Stamp::utc_now()),
        );
        m.author = preset.map(|p| p.name.clone()).filter(|n| !n.is_empty());
        m.thumbnail = Some(webhook::LOGO.to_string());
        m
    }

    /// A new queue item (its id tells it apart from the others).
    pub fn discord_item(&self, kind: &str, message: DiscordMessage, ping: DiscordPing, daily_day: Option<String>, weekly_day: Option<String>, route: Option<Guid>) -> DiscordItem {
        let id = self.discord.next_item.get() + 1;
        self.discord.next_item.set(id);
        DiscordItem { kind: kind.to_string(), message, ping, daily_day, weekly_day, route, id }
    }

    pub fn enqueue_discord(&self, item: DiscordItem) {
        let mut queue = self.discord.queue.borrow_mut();
        if item.kind == "board" {
            queue.retain(|i| i.kind != "board");
        }
        queue.push(item);
        // Offline for long: the oldest go, and the next message says how many.
        while queue.len() > DISCORD_QUEUE_MAX {
            if queue[0].kind != "board" {
                let dropped = self.discord.dropped.get();
                self.discord.dropped.set(dropped + 1);
                if dropped == 0 {
                    log::note(&format!("Discord: the queue is full ({DISCORD_QUEUE_MAX}) while sending fails; the oldest messages are dropped."));
                }
            }
            queue.remove(0);
        }
    }

    pub fn reset_discord_queue(&self) {
        self.discord.queue.borrow_mut().clear();
        self.discord.events.borrow_mut().clear();
        self.discord.summaries_due.borrow_mut().clear();
        self.discord.dropped.set(0);
        self.discord.failures.set(0);
        self.discord.next_try.set(stamp_min());
    }

    // ───────────────────────────── sending ─────────────────────────────

    /// Each process check while unlocked: due events, summaries and status updates join the queue; one message goes out at a time.
    pub fn tick_discord(self: &Rc<Self>, now: Stamp, players: &[PlayerProcess]) {
        if !self.has_open_vault() {
            return;
        }
        self.sync_discord_vault(now);
        let Ok(d) = self.vault(|v| v.discord.clone()) else { return };
        if !d.enabled || d.webhook_url.is_empty() {
            return;
        }
        if !self.discord.events.borrow().is_empty() && now >= self.discord.batch_due.get() {
            let events: Vec<(String, DiscordAlert, DiscordPing, Option<Guid>)> = self.discord.events.borrow_mut().drain(..).collect();
            // GroupBy keeps the order each route first appears in.
            let mut routes: Vec<Option<Guid>> = Vec::new();
            for e in &events {
                if !routes.contains(&e.3) {
                    routes.push(e.3);
                }
            }
            for route in routes {
                let group: Vec<&(String, DiscordAlert, DiscordPing, Option<Guid>)> = events.iter().filter(|e| e.3 == route).collect();
                let ping = group.iter().fold(DiscordPing::none(), |all, e| all.merge(&e.2));
                let message = self.events_message(&group.iter().map(|e| e.1.clone()).collect::<Vec<_>>());
                let item = self.discord_item("events", message, ping, None, None, route);
                self.enqueue_discord(item);
            }
        }
        // Due from its time until it went out: one missed (locked, asleep, closed, offline) goes once, marked late; only the latest.
        let local = Stamp::now();
        let daily_at = self.due_summary(d.summary_daily, &d.summary_daily_time, None, &d.last_daily, local, |i| i.daily_day.as_ref());
        let weekly_at = self.due_summary(d.summary_weekly, &d.summary_weekly_time, Some(d.summary_weekday), &d.last_weekly, local, |i| i.weekly_day.as_ref());
        // The daily one covering 7 days at the weekly one's time would say the same twice: one message for both.
        match (daily_at, weekly_at) {
            (Some(both), Some(week)) if week == both && d.summary_days == 7 => {
                if let Ok(message) = self.summary_message(&d, 7, Some(both), Some(local), false, None, "both") {
                    let item = self.discord_item("summary", message, ping_for(&d, "summary"), Some(day_key(both)), Some(day_key(both)), None);
                    self.enqueue_discord(item);
                }
            }
            _ => {
                if let Some(day) = daily_at {
                    if let Ok(message) = self.summary_message(&d, d.summary_days, Some(day), Some(local), false, None, "daily") {
                        let item = self.discord_item("summary", message, ping_for(&d, "summary"), Some(day_key(day)), None, None);
                        self.enqueue_discord(item);
                    }
                }
                if let Some(week) = weekly_at {
                    if let Ok(message) = self.summary_message(&d, 7, Some(week), Some(local), false, None, "weekly") {
                        let item = self.discord_item("summary", message, ping_for(&d, "summary"), None, Some(day_key(week)), None);
                        self.enqueue_discord(item);
                    }
                }
            }
        }
        // After a scheduled preset: today so far, for its accounts, to its own webhook when it has one.
        let due: Vec<Guid> = self.discord.summaries_due.borrow().iter().filter(|x| now >= *x.1).map(|x| *x.0).collect();
        for preset_id in due {
            self.discord.summaries_due.borrow_mut().remove(&preset_id);
            if let Some(preset) = self.preset_for(Some(preset_id)) {
                if let Ok(message) = self.summary_message(&d, 1, None, None, true, Some(&preset), "preset") {
                    let item = self.discord_item("summary", message, ping_for(&d, "summary"), None, None, self.route_for(Some(preset_id)));
                    self.enqueue_discord(item);
                }
            }
        }
        if d.board && now >= self.discord.next_board.get() {
            self.discord.next_board.set(now.plus_secs(d.board_minutes.max(2) as f64 * 60.0));
            let message = self.board_message(&d, players);
            let item = self.discord_item("board", message, DiscordPing::none(), None, None, None); // edits never ping anyway
            self.enqueue_discord(item);
        }
        let first = self.discord.queue.borrow().first().cloned();
        if let Some(item) = first {
            if !self.discord.sending.get() && now >= self.discord.next_try.get() {
                self.send_queued(item);
            }
        }
    }

    /// Unlocked (again), or another vault: nothing queued before the lock goes out now.
    pub fn sync_discord_vault(&self, now: Stamp) {
        let token = self.vault_token();
        if token.is_none() || self.discord.vault.get() == token {
            return;
        }
        self.discord.vault.set(token);
        self.reset_discord_queue();
        self.discord.next_board.set(now.plus_secs(20.0));
        // Sent while the vault was locking: this vault edits that status message and counts those summaries as sent.
        let Ok((url, board_id)) = self.vault(|v| (v.discord.webhook_url.clone(), v.discord.board_message_id.clone())) else { return };
        let key = webhook_key(&url);
        let posted = self.discord.board_posted.borrow_mut().take();
        if let Some((url_key, id)) = posted {
            if url_key == key && board_id.is_empty() {
                let _ = self.vault_mut(|v| v.discord.board_message_id = id);
                self.persist_soon();
            }
        }
        let sent: Vec<(String, Option<String>, Option<String>)> = self.discord.summaries_sent.borrow_mut().drain(..).collect();
        for (_, daily, weekly) in sent.into_iter().filter(|s| s.0 == key) {
            self.note_summary_sent(daily.as_deref(), weekly.as_deref());
        }
    }

    /// The clock was set back: the waits move back with it, and an event noted "later" doesn't mute the next ones.
    pub fn move_discord_back(&self, back: Duration) {
        use crate::core::app::processes::moved_back;
        self.discord.batch_due.set(moved_back(self.discord.batch_due.get(), back));
        self.discord.next_try.set(moved_back(self.discord.next_try.get(), back));
        self.discord.next_board.set(moved_back(self.discord.next_board.get(), back));
        for at in self.discord.summaries_due.borrow_mut().values_mut() {
            *at = moved_back(*at, back);
        }
        for at in self.discord.notified.borrow_mut().values_mut() {
            *at = moved_back(*at, back);
        }
    }

    /// Starts sending the queue's first message (its first part now, as C#'s call ran up to its first await; the rest
    /// on the core's own time). Fire and forget from the timer: never takes the app down.
    pub fn send_queued(self: &Rc<Self>, item: DiscordItem) {
        let Some(token) = self.vault_token() else { return };
        let Ok(d) = self.vault(|v| v.discord.clone()) else { return };
        let mut message = item.message.clone();
        let dropped = if item.kind == "board" { 0 } else { self.discord.dropped.get() };
        if dropped > 0 {
            message = self.with_dropped(&message, dropped);
        }
        let board = item.kind == "board";
        let edit = if board && !d.board_message_id.is_empty() { Some(d.board_message_id.clone()) } else { None };
        let url = self.url_for(&d, item.route);
        self.discord.sending.set(true);
        let app = self.clone();
        rt::spawn(async move {
            if let Err(ex) = app.send_queued_core(token, d, item, message, dropped, edit, url).await {
                // Only the kind is logged (no message could hold the link).
                let kind = match ex {
                    Fail::User(_) => "UserError",
                    Fail::Roblox { .. } => "RobloxApiException",
                    Fail::Other(_) => "Exception",
                };
                log::note(&format!("Discord send failed: {kind}"));
                // Not tried again on every tick (it may have gone out before the failure): a minute later at the soonest.
                let retry = Stamp::utc_now().plus_secs(60.0);
                if app.discord.next_try.get() < retry {
                    app.discord.next_try.set(retry);
                }
            }
        });
    }

    #[allow(clippy::too_many_arguments)]
    async fn send_queued_core(
        self: &Rc<Self>,
        token: usize,
        d: DiscordSettings,
        item: DiscordItem,
        message: DiscordMessage,
        dropped: i32,
        edit: Option<String>,
        url: String,
    ) -> Res<()> {
        let board = item.kind == "board";
        // Set before the task started; cleared however this ends (a panic in the picture or the send too).
        let sending = crate::core::base::Busy::set(&self.discord.sending);
        let result = {
            let picture = summary_picture(&message).await;
            let mut payload = webhook::build(&message, &item.ping, d.style != "plain", Some(&d.username), Some(&d.avatar_url), picture.as_ref().map(|p| p.name.as_str()));
            let (u, e) = (url.clone(), edit.clone());
            let result = rt::blocking(move || webhook::send(&u, &mut payload, e.as_deref(), board, &Cancel::new(), picture.as_ref())).await;
            drop(sending);
            result?
        };
        if self.controller.disposed.get() {
            return Ok(());
        }
        let current_settings = self.vault(|v| v.discord.clone()).ok();
        let same_vault = self.vault_token() == Some(token);
        if !same_vault || current_settings.as_ref().is_none_or(|cur| self.url_for(cur, item.route) != url) {
            // Locked while the first status message or a summary went out: the next unlock edits it and doesn't resend the summary.
            if !self.has_open_vault() && result.outcome == DiscordOutcome::Sent {
                if board && edit.is_none() {
                    if let Some(posted) = &result.message_id {
                        *self.discord.board_posted.borrow_mut() = Some((webhook_key(&url), posted.clone()));
                    }
                }
                if item.daily_day.is_some() || item.weekly_day.is_some() {
                    self.discord.summaries_sent.borrow_mut().push((webhook_key(&url), item.daily_day.clone(), item.weekly_day.clone()));
                }
            }
            return Ok(());
        }
        let d = current_settings.unwrap_or(d);
        // Kept even when a newer update replaced this one meanwhile, so that one edits this message instead of posting another.
        if board && result.outcome == DiscordOutcome::Sent {
            if let Some(id) = result.message_id.clone().filter(|id| *id != d.board_message_id) {
                self.vault_mut(|v| v.discord.board_message_id = id)?;
                self.persist_soon();
            }
        }
        // Noted as sent only once it went out (or never will), so one dropped from the queue (a lock, sending off) goes again.
        if matches!(result.outcome, DiscordOutcome::Sent | DiscordOutcome::Rejected) {
            self.note_summary_sent(item.daily_day.as_deref(), item.weekly_day.as_deref());
        }
        // Sending switched off meanwhile: this answer is about a queue that is gone.
        if !self.vault(|v| v.discord.enabled)? {
            return Ok(());
        }
        // A newer status update replaced this one meanwhile (or the queue was emptied): Discord's answer still counts
        // for what goes out next (a rate limit, being offline, a webhook that is gone).
        let current = self.discord.queue.borrow().first().is_some_and(|first| first.id == item.id);
        let now = Stamp::utc_now();
        match result.outcome {
            DiscordOutcome::Sent => {
                if current {
                    self.discord.queue.borrow_mut().remove(0);
                    self.discord.dropped.set((self.discord.dropped.get() - dropped).max(0));
                }
                if self.discord.failures.get() > 0 {
                    log::note(&format!("Discord: sending recovered after {} failed tries.", self.discord.failures.get()));
                }
                self.discord.failures.set(0);
                self.discord.next_try.set(now.plus_secs(result.wait.as_secs_f64()));
                self.discord.last_sent.set(Some(now));
                *self.discord.error.borrow_mut() = None;
            }
            DiscordOutcome::MessageGone => {
                // Someone deleted the status message: a new one is posted (and its id kept) right away.
                log::note("Discord: the status message was deleted in Discord; a new one is posted.");
                self.vault_mut(|v| v.discord.board_message_id = String::new())?;
                self.persist_soon();
            }
            DiscordOutcome::RateLimited => {
                self.discord.next_try.set(now.plus_secs(result.wait.as_secs_f64()));
                log::note(&format!("Discord: rate-limited on a {} message; next try in {} s.", item.kind, ceil_secs(result.wait)));
            }
            DiscordOutcome::Offline => {
                if self.discord.failures.get() == 0 {
                    log::note(&format!(
                        "Discord: a {} message not sent ({}); tried again with growing waits, up to {} min.",
                        item.kind,
                        result.error.as_deref().unwrap_or("network error"),
                        DISCORD_MAX_BACKOFF.as_secs_f64() / 60.0
                    ));
                }
                let failures = self.discord.failures.get() + 1;
                self.discord.failures.set(failures);
                let wait = 15.0 * 2f64.powi((failures - 1).min(6));
                self.discord.next_try.set(now.plus_secs(wait.min(DISCORD_MAX_BACKOFF.as_secs_f64())));
                *self.discord.error.borrow_mut() = Some(discord_offline_text(&result));
            }
            DiscordOutcome::Gone if item.route.and_then(|gone| self.preset_for(Some(gone))).is_some() => {
                let gone = item.route.unwrap_or_default();
                let name = self.preset_for(Some(gone)).map(|p| p.name).unwrap_or_default();
                // Only that preset's webhook is gone: it goes, and its messages go to Settings' webhook again.
                log::note("Discord: a preset's webhook no longer exists (Discord said so); removed, its messages go to Settings' webhook.");
                self.vault_mut(|v| {
                    if let Some(p) = v.settings.presets.iter_mut().find(|p| p.id == gone) {
                        p.discord_webhook = String::new();
                    }
                })?;
                self.discord_route_changed(gone);
                if current {
                    self.discord.queue.borrow_mut().retain(|i| i.id != item.id);
                }
                self.persist_soon();
                *self.discord.error.borrow_mut() = Some(l::f(
                    "Discord says the webhook of preset {0} no longer exists, so it was removed. Its messages go to the channel in Settings again.",
                    &[&name],
                ));
            }
            DiscordOutcome::Gone => {
                log::note("Discord: the webhook no longer exists (Discord said so); sending stopped until a new link.");
                self.stop_discord();
            }
            _ => {
                // Discord refused this one message (it never will take it): it goes, the rest follow.
                log::note(&format!("Discord: a {} message was refused ({}); dropped.", item.kind, result.error.as_deref().unwrap_or("")));
                if current {
                    self.discord.queue.borrow_mut().remove(0);
                }
                *self.discord.error.borrow_mut() = Some(l::f("Discord refused a message: {0}", &[&result.error.clone().unwrap_or_default()]));
            }
        }
        self.push_state();
        Ok(())
    }

    /// Messages dropped while offline, said in a small line at the end of the next one.
    pub fn with_dropped(&self, message: &DiscordMessage, dropped: i32) -> DiscordMessage {
        let mut m = message.clone();
        let line = self.dp(dropped, "{0} older message couldn't be sent while offline and was dropped.", "{0} older messages couldn't be sent while offline and were dropped.", &[&dropped],
        );
        m.text = format!("{}-# {line}", if message.text.is_empty() { String::new() } else { format!("{}\n", message.text) });
        m
    }

    /// A daily or weekly summary went out: its day is noted, never an older one over a newer.
    pub fn note_summary_sent(&self, daily_day: Option<&str>, weekly_day: Option<&str>) {
        let changed = self
            .vault_mut(|v| {
                let d = &mut v.discord;
                let mut changed = false;
                if let Some(day) = daily_day.filter(|day| *day > d.last_daily.as_str()) {
                    d.last_daily = day.to_string();
                    changed = true;
                }
                if let Some(week) = weekly_day.filter(|week| *week > d.last_weekly.as_str()) {
                    d.last_weekly = week.to_string();
                    changed = true;
                }
                changed
            })
            .unwrap_or(false);
        if changed {
            self.persist_soon();
        }
    }

    /// On lock or close, one last edit says why the status message stopped (else "N accounts open" for good). Best
    /// effort, a few seconds at most, past the queue (dropped with the lock).
    /// Returns the edit on its way (it says when it is done), or None when there is no status message to edit.
    pub fn final_discord_board(&self, closing: bool) -> Option<std::sync::mpsc::Receiver<()>> {
        let d = self.vault(|v| v.discord.clone()).ok()?;
        if !d.enabled || !d.board || d.webhook_url.is_empty() || d.board_message_id.is_empty() {
            return None;
        }
        let mut payload = webhook::build(&self.final_board_message(closing), &DiscordPing::none(), d.style != "plain", Some(&d.username), Some(&d.avatar_url), None);
        let (url, id) = (d.webhook_url.clone(), d.board_message_id.clone());
        // On a worker: the exit waits for it, which must not need the core's thread to finish.
        let (done, finished) = std::sync::mpsc::channel();
        crate::threads::run(move || {
                let cancel = Cancel::new();
                let timer = cancel.clone();
                // The five seconds are kept on a pool thread of their own: the core's timers wouldn't fire while it waits
                // on this edit. Called off as soon as the edit is done, so that thread is free again at once.
                let finished_early = Cancel::new();
                let ended = finished_early.clone();
                crate::threads::run(move || {
                    if ended.sleep(Duration::from_secs(5)) {
                        timer.cancel();
                    }
                });
                match webhook::send(&url, &mut payload, Some(&id), false, &cancel, None) {
                    Ok(r) if r.outcome == webhook::DiscordOutcome::Sent => {}
                    Ok(r) => log::note_file(&format!("Discord: the status board's last edit wasn't made ({:?}{}).", r.outcome, r.error.map(|e| format!(": {e}")).unwrap_or_default())),
                    Err(e) => log::note_file(&format!("Discord: the status board's last edit wasn't made ({}).", crate::core::app::controller::error_text(&e))),
                }
                finished_early.cancel();
                let _ = done.send(());
            });
        Some(finished)
    }

    /// Discord says the webhook no longer exists (deleted, or its link reset): sending stops until a new link.
    pub fn stop_discord(&self) {
        let _ = self.vault_mut(|v| {
            v.discord.enabled = false;
            v.discord.stopped = Some("gone".into());
            v.discord.board_message_id = String::new();
        });
        self.reset_discord_queue();
        *self.discord.error.borrow_mut() = None;
        self.persist_soon();
        self.toast(&l::t("Discord says the webhook no longer exists, so Bloxkeep stopped sending to it. Paste a new link in Settings › Discord."), "warn");
    }

    /// The preset form's "Send test": to the link typed in it, or the preset's. Nothing is saved, and a webhook that
    /// doesn't exist is only reported, not removed.
    pub async fn send_preset_discord_test(self: &Rc<Self>, preset_id: Guid, typed: Option<String>) -> Res<Value> {
        let d = self.vault(|v| v.discord.clone())?;
        let token = self.vault_token();
        let url = match typed.filter(|t| !t.is_empty()) {
            Some(typed) => webhook::try_normalize(Some(&typed[..])).ok_or_else(not_a_webhook)?,
            None => match self.preset_for(Some(preset_id)).map(|p| p.discord_webhook).filter(|w| !w.is_empty()) {
                Some(saved) => saved,
                None => return Err(Fail::user(l::t("Paste a webhook link first."))),
            },
        };
        let message = self.test_message(self.preset_for(Some(preset_id)).as_ref(), true);
        let mut payload = webhook::build(&message, &DiscordPing::none(), d.style != "plain", Some(&d.username), Some(&d.avatar_url), None);
        let result = rt::blocking(move || webhook::send(&url, &mut payload, None, true, &Cancel::new(), None)).await?;
        if self.vault_token() != token {
            return Err(Fail::user(l::t("Bloxkeep was locked.")));
        }
        if result.outcome != DiscordOutcome::Sent {
            log::note(&format!(
                "Discord: a preset's test message wasn't sent ({:?}{}).",
                result.outcome,
                result.error.as_ref().map(|e| format!(", {e}")).unwrap_or_default()
            ));
        }
        match result.outcome {
            DiscordOutcome::Sent => Ok(Value::Null),
            DiscordOutcome::Gone | DiscordOutcome::MessageGone => {
                Err(Fail::user(l::t("Discord says this webhook doesn't exist: it was deleted, or the link is wrong. Paste a new link.")))
            }
            DiscordOutcome::RateLimited => Err(Fail::user(l::f("Discord asks to wait {0} s before the next message.", &[&ceil_secs(result.wait)]))),
            DiscordOutcome::Offline => Err(Fail::user(discord_offline_text(&result))),
            _ => Err(Fail::user(l::f("Discord refused the message: {0}", &[&result.error.clone().unwrap_or_default()]))),
        }
    }

    /// "Send test" and "Send summary now": straight out, past the queue, with the answer shown at once.
    pub async fn send_discord_now(self: &Rc<Self>, summary: bool) -> Res<Value> {
        let d = self.vault(|v| v.discord.clone())?;
        let token = self.vault_token();
        if d.webhook_url.is_empty() {
            return Err(Fail::user(l::t("Paste a webhook link first.")));
        }
        let message = if summary { self.summary_message(&d, d.summary_days, None, None, false, None, "now")? } else { self.test_message(None, false) };
        let picture = summary_picture(&message).await;
        let ping = if summary { ping_for(&d, "summary") } else { DiscordPing::none() };
        let mut payload = webhook::build(&message, &ping, d.style != "plain", Some(&d.username), Some(&d.avatar_url), picture.as_ref().map(|p| p.name.as_str()));
        let url = d.webhook_url.clone();
        let result = rt::blocking(move || webhook::send(&url, &mut payload, None, true, &Cancel::new(), picture.as_ref())).await?;
        if self.vault_token() != token {
            return Err(Fail::user(l::t("Bloxkeep was locked.")));
        }
        if result.outcome != DiscordOutcome::Sent {
            log::note(&format!(
                "Discord: {} wasn't sent ({:?}{}).",
                if summary { "Send summary now" } else { "Send test" },
                result.outcome,
                result.error.as_ref().map(|e| format!(", {e}")).unwrap_or_default()
            ));
        }
        match result.outcome {
            DiscordOutcome::Sent => {
                self.discord.last_sent.set(Some(Stamp::utc_now()));
                *self.discord.error.borrow_mut() = None;
                self.vault_mut(|v| v.discord.stopped = None)?;
                self.push_state();
                Ok(Value::Null)
            }
            DiscordOutcome::Gone | DiscordOutcome::MessageGone => {
                if self.vault(|v| v.discord.enabled)? {
                    self.stop_discord();
                } else {
                    self.vault_mut(|v| v.discord.stopped = Some("gone".into()))?;
                    self.persist_soon();
                }
                self.push_state();
                Err(Fail::user(l::t("Discord says this webhook doesn't exist: it was deleted, or the link is wrong. Paste a new link.")))
            }
            DiscordOutcome::RateLimited => Err(Fail::user(l::f("Discord asks to wait {0} s before the next message.", &[&ceil_secs(result.wait)]))),
            DiscordOutcome::Offline => Err(Fail::user(discord_offline_text(&result))),
            _ => Err(Fail::user(l::f("Discord refused the message: {0}", &[&result.error.clone().unwrap_or_default()]))),
        }
    }
}

/// A summary's picture (see summary_card), or none: the message goes without it rather than not at all.
pub async fn summary_picture(message: &DiscordMessage) -> Option<DiscordFile> {
    let card = message.card.clone()?;
    match rt::blocking(move || crate::core::summary_card::render(&card, &Cancel::new())).await {
        Ok(data) => Some(DiscordFile { name: crate::core::app::discord_summary::SUMMARY_PICTURE.to_string(), data }),
        Err(ex) => {
            log::write(&ex);
            None
        }
    }
}
