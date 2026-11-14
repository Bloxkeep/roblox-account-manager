//! DiscordWebhook.cs: posts to the webhook the person pasted in Settings › Discord: builds the message within Discord's
//! limits, with `allowed_mentions` naming exactly who may be pinged. Only discord.com is ever contacted, and the link
//! (its token is the only key to the webhook) never reaches a log or an error text.
use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;
use serde_json::{Map, Value, json};

use crate::core::base::{Cancel, Fail, Res, Stamp};
use crate::core::log;
use crate::core::models::to_net_json_string;
use crate::core::summary_card::SummaryCardData;

/// What a message says before it is shaped (embed or plain text), as docs/discord-style.md says: a short title,
/// colour by meaning, facts in short fields, "Bloxkeep · …" in the footer.
/// `title`: the news in a few words (markdown: names in it are escaped). `timestamp`: when it happened: shown by
/// Discord under an embed, in each reader's own time zone.
#[derive(Clone)]
pub struct DiscordMessage {
    pub title: String,
    pub text: String,
    pub fields: Vec<DiscordField>,
    pub footer: Option<String>,
    pub color: i32,
    pub timestamp: Option<Stamp>,
    /// A small line above the title: the account (or preset) the message is about. Plain text, not escaped.
    pub author: Option<String>,
    /// The author's picture: the account's avatar from Roblox's image servers (https only).
    pub author_icon: Option<String>,
    /// A picture in the top-right corner: the game's icon from Roblox's servers, or the logo.
    pub thumbnail: Option<String>,
    pub more: Vec<DiscordMessage>,
    /// A summary's picture: drawn when it goes out and attached to the message, shown in this embed.
    pub card: Option<SummaryCardData>,
}

impl DiscordMessage {
    /// `new DiscordMessage(title, text, fields)`: no footer, blue, no time; the init-only parts empty.
    pub fn new(title: impl Into<String>, text: impl Into<String>, fields: Vec<DiscordField>) -> DiscordMessage {
        DiscordMessage {
            title: title.into(),
            text: text.into(),
            fields,
            footer: None,
            color: BLUE,
            timestamp: None,
            author: None,
            author_icon: None,
            thumbnail: None,
            more: Vec::new(),
            card: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DiscordFile {
    pub name: String,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscordField {
    pub name: String,
    pub value: String,
    pub inline: bool,
}

impl DiscordField {
    pub fn new(name: impl Into<String>, value: impl Into<String>, inline: bool) -> DiscordField {
        DiscordField { name: name.into(), value: value.into(), inline }
    }
}

/// Who a message pings: user and role ids (numbers as text), @here, @everyone. Nothing else ever pings.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct DiscordPing {
    pub users: Vec<String>,
    pub roles: Vec<String>,
    pub here: bool,
    pub everyone: bool,
}

impl DiscordPing {
    pub fn none() -> DiscordPing {
        DiscordPing::default()
    }

    pub fn any(&self) -> bool {
        !self.users.is_empty() || !self.roles.is_empty() || self.here || self.everyone
    }

    /// "none", "here", "everyone", "user:<id>" or "role:<id>"; None when it is none of these.
    pub fn parse(value: Option<&str>) -> Option<DiscordPing> {
        match value {
            None | Some("") | Some("none") => Some(DiscordPing::none()),
            Some("here") => Some(DiscordPing { here: true, ..DiscordPing::default() }),
            Some("everyone") => Some(DiscordPing { everyone: true, ..DiscordPing::default() }),
            Some(v) if v.starts_with("user:") && is_id(Some(&v[5..])) => Some(DiscordPing { users: vec![v[5..].to_string()], ..DiscordPing::default() }),
            Some(v) if v.starts_with("role:") && is_id(Some(&v[5..])) => Some(DiscordPing { roles: vec![v[5..].to_string()], ..DiscordPing::default() }),
            _ => None,
        }
    }

    /// Several events in one message ping everyone any of them would have pinged, once each.
    pub fn merge(&self, other: &DiscordPing) -> DiscordPing {
        fn distinct(a: &[String], b: &[String]) -> Vec<String> {
            let mut out: Vec<String> = Vec::new();
            for x in a.iter().chain(b) {
                if !out.contains(x) {
                    out.push(x.clone());
                }
            }
            out
        }
        DiscordPing { users: distinct(&self.users, &other.users), roles: distinct(&self.roles, &other.roles), here: self.here || other.here, everyone: self.everyone || other.everyone }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiscordOutcome {
    Sent,
    RateLimited,
    Gone,
    MessageGone,
    Rejected,
    Offline,
}

/// `wait`: how long Discord asked to wait (a 429), or until its bucket refills after this one. `error`: Discord's own
/// reason (a 400) or the network error; never contains the link.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscordResult {
    pub outcome: DiscordOutcome,
    pub message_id: Option<String>,
    pub wait: Duration,
    pub error: Option<String>,
}

impl DiscordResult {
    fn of(outcome: DiscordOutcome) -> DiscordResult {
        DiscordResult { outcome, message_id: None, wait: Duration::ZERO, error: None }
    }
    fn with_error(outcome: DiscordOutcome, error: Option<String>) -> DiscordResult {
        DiscordResult { error, ..DiscordResult::of(outcome) }
    }
}

// Discord's limits (docs.discord.com, Resources › Message / Webhook).
pub const MAX_EMBEDS: usize = 10;
pub const MAX_CONTENT: usize = 2000;
pub const MAX_TITLE: usize = 256;
pub const MAX_DESCRIPTION: usize = 4096;
pub const MAX_FIELDS: usize = 25;
pub const MAX_FIELD_NAME: usize = 256;
pub const MAX_FIELD_VALUE: usize = 1024;
pub const MAX_FOOTER: usize = 2048;
pub const MAX_EMBED_TOTAL: usize = 6000;
pub const MAX_USERNAME: usize = 80;
/// Discord's own colours, by what a message means: news, good or done, needs a look, broken or blocked, nothing to do.
pub const BLUE: i32 = 0x5865F2;
pub const GREEN: i32 = 0x57F287;
pub const YELLOW: i32 = 0xFEE75C;
pub const RED: i32 = 0xED4245;
pub const GREY: i32 = 0x4E5058;
/// The sender's name and picture when the person set none, and the picture beside every footer.
pub const DEFAULT_NAME: &str = "Bloxkeep";
pub const LOGO: &str = "https://raw.githubusercontent.com/Bloxkeep/roblox-account-manager/main/docs/brand/bloxkeep.png";

static HTTP: LazyLock<ureq::Agent> = LazyLock::new(|| crate::net::builder().timeout(Duration::from_secs(15)).redirects(0).build());
// PORT: PooledConnectionLifetime (5 min) has no ureq setting; ureq keeps idle connections in its own pool.

// discord.com, discordapp.com and their canary./ptb. hosts, an optional API version, then the webhook's id and token.
// (.NET's `$` also matches before a final "\n": `\n?\z` here.)
static WEBHOOK_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^https://(?:(?:canary|ptb)\.)?discord(?:app)?\.com/api(?:/v\d{1,2})?/webhooks/(\d{17,20})/([A-Za-z0-9_\-]{50,100})/?\n?\z").unwrap());
static ID_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d{17,20}\n?\z").unwrap());
static ESCAPE_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\\*_~`|>\[\]<@]").unwrap());
// [text](https://…) → text: a link in plain text would make Discord add a preview of the page under the message.
static LINK_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[((?:\\.|[^\]\\])*)\]\(https://[^)\s]+\)").unwrap());

/// A user or role id: 17–20 digits (what Discord's "Copy ID" gives).
pub fn is_id(text: Option<&str>) -> bool {
    text.is_some_and(|t| ID_PATTERN.is_match(t))
}

/// A pasted webhook link, accepted only in Discord's own form and sent to discord.com whatever host it named; None for
/// anything else. (C#'s TryNormalize.)
pub fn try_normalize(url: Option<&str>) -> Option<String> {
    let m = WEBHOOK_PATTERN.captures(url.unwrap_or("").trim())?;
    Some(format!("https://discord.com/api/webhooks/{}/{}", &m[1], &m[2]))
}

/// What the page shows of a saved link: enough to tell two webhooks apart, never the token.
pub fn mask(url: &str) -> String {
    match WEBHOOK_PATTERN.captures(url) {
        // The id's last four characters: `\d` takes any script's digits, so not always four bytes.
        Some(m) => {
            let start = m[1].char_indices().rev().nth(3).map_or(0, |(i, _)| i);
            format!("discord.com/api/webhooks/…{}/••••••", &m[1][start..])
        }
        None => String::new(),
    }
}

/// Names and game titles show as typed: Discord's formatting characters are escaped, so "@everyone" or
/// "<@123>" shows as text (allowed_mentions already keeps it from pinging).
pub fn escape(text: Option<&str>) -> String {
    ESCAPE_PATTERN.replace_all(text.unwrap_or(""), r"\$0").into_owned()
}

/// A game's name (escaped) linking to its page on Roblox; plain text shows the name only.
pub fn game_link(name: &str, place_id: Option<i64>) -> String {
    match place_id {
        Some(id) if id > 0 => format!("[{}](https://www.roblox.com/games/{id})", escape(Some(name))),
        _ => escape(Some(name)),
    }
}

/// A picture from Roblox's image servers (an avatar, a game's icon): the only kind a message shows besides the logo.
pub fn is_roblox_picture(url: Option<&str>) -> bool {
    url.and_then(|u| url::Url::parse(u).ok())
        .is_some_and(|uri| uri.scheme() == "https" && uri.host_str().is_some_and(|h| h.to_ascii_lowercase().ends_with(".rbxcdn.com")))
}

/// Length as C# counts it (UTF-16 units).
fn len16(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Cut to at most `max` UTF-16 units (Discord counts no fewer), ending with "…" when cut.
pub fn cut(text: Option<&str>, max: usize) -> String {
    let text = text.unwrap_or("");
    if len16(text) <= max {
        return text.to_string();
    }
    crate::core::base::cut_utf16(text, max.saturating_sub(1)).to_string() + "…"
}

/// The JSON Discord gets: an embed (or plain text) within every limit, pings first in `content` and
/// `allowed_mentions` listing exactly those, so "@everyone" inside the text pings nobody.
/// `picture`: the name of a picture sent with it (`DiscordFile`): the first embed shows it.
pub fn build(message: &DiscordMessage, ping: &DiscordPing, embed: bool, username: Option<&str>, avatar_url: Option<&str>, picture: Option<&str>) -> Map<String, Value> {
    let mut mentions: Vec<String> = ping.users.iter().map(|u| format!("<@{u}>")).chain(ping.roles.iter().map(|r| format!("<@&{r}>"))).collect();
    if ping.here {
        mentions.push("@here".into());
    }
    if ping.everyone {
        mentions.push("@everyone".into());
    }
    let pings = mentions.join(" ");
    let mut payload = Map::new();
    let name = match username {
        Some(u) if !u.trim().is_empty() => u.trim(),
        _ => DEFAULT_NAME,
    };
    payload.insert("username".into(), json!(cut(Some(name), MAX_USERNAME)));
    let avatar = match avatar_url {
        Some(a) if !a.trim().is_empty() => a.trim(),
        _ => LOGO,
    };
    payload.insert("avatar_url".into(), json!(avatar));
    if embed {
        if !pings.is_empty() {
            payload.insert("content".into(), json!(pings));
        }
        // Up to 10 embeds and 6000 characters across them all: the last ones go first.
        let mut embeds = vec![embed_of(message, picture)];
        embeds.extend(message.more.iter().take(MAX_EMBEDS - 1).map(|m| embed_of(m, None)));
        while embeds.len() > 1 && embeds.iter().map(length).sum::<usize>() > MAX_EMBED_TOTAL {
            embeds.pop();
        }
        payload.insert("embeds".into(), Value::Array(embeds.into_iter().map(Value::Object).collect()));
    } else {
        let body = std::iter::once(message).chain(message.more.iter()).map(plain_text).collect::<Vec<_>>().join("\n\n");
        let content = if !pings.is_empty() { format!("{pings}\n{}", cut(Some(&body), MAX_CONTENT.saturating_sub(len16(&pings) + 1))) } else { cut(Some(&body), MAX_CONTENT) };
        payload.insert("content".into(), json!(content));
    }
    let mut allowed = Map::new();
    allowed.insert("parse".into(), if ping.here || ping.everyone { json!(["everyone"]) } else { json!([]) });
    if !ping.users.is_empty() {
        allowed.insert("users".into(), json!(ping.users.iter().take(100).collect::<Vec<_>>()));
    }
    if !ping.roles.is_empty() {
        allowed.insert("roles".into(), json!(ping.roles.iter().take(100).collect::<Vec<_>>()));
    }
    payload.insert("allowed_mentions".into(), Value::Object(allowed));
    if let Some(picture) = picture {
        payload.insert("attachments".into(), json!([{ "id": 0, "filename": picture }]));
    }
    payload
}

/// The characters Discord counts towards an embed's 6000.
fn length(e: &Map<String, Value>) -> usize {
    let s = |o: Option<&Value>, name: &str| o.and_then(|o| o.get(name)).and_then(Value::as_str).map(len16).unwrap_or(0);
    let me = |name: &str| e.get(name).and_then(Value::as_str).map(len16).unwrap_or(0);
    me("title")
        + s(e.get("author"), "name")
        + me("description")
        + s(e.get("footer"), "text")
        + e.get("fields").and_then(Value::as_array).map(|fs| fs.iter().filter(|f| f.is_object()).map(|f| s(Some(f), "name") + s(Some(f), "value")).sum()).unwrap_or(0)
}

fn embed_of(m: &DiscordMessage, picture: Option<&str>) -> Map<String, Value> {
    let title = cut(Some(&m.title), MAX_TITLE);
    let mut description = cut(Some(&m.text), MAX_DESCRIPTION);
    let footer = cut(m.footer.as_deref(), MAX_FOOTER);
    let mut fields: Vec<DiscordField> =
        m.fields.iter().map(|f| DiscordField::new(cut(Some(&blank(&f.name)), MAX_FIELD_NAME), cut(Some(&blank(&f.value)), MAX_FIELD_VALUE), f.inline)).collect();
    if fields.len() > MAX_FIELDS {
        let more = fields.len() - (MAX_FIELDS - 1);
        fields.truncate(MAX_FIELDS - 1);
        fields.push(DiscordField::new("…", format!("+{more}"), false));
    }
    // 6000 characters over the whole embed: the last fields go first, then the description is shortened.
    let total = |description: &str, fields: &[DiscordField]| len16(&title) + len16(description) + len16(&footer) + fields.iter().map(|f| len16(&f.name) + len16(&f.value)).sum::<usize>();
    while total(&description, &fields) > MAX_EMBED_TOTAL && !fields.is_empty() {
        fields.pop();
    }
    if total(&description, &fields) > MAX_EMBED_TOTAL {
        let others = total(&description, &fields) - len16(&description);
        description = cut(Some(&description), MAX_EMBED_TOTAL.saturating_sub(others));
    }
    let mut embed = Map::new();
    embed.insert("title".into(), json!(title));
    embed.insert("color".into(), json!(m.color));
    if let Some(author_name) = m.author.as_deref().filter(|a| !a.is_empty()) {
        let mut author = Map::new();
        author.insert("name".into(), json!(cut(Some(author_name), MAX_TITLE)));
        if https(m.author_icon.as_deref()) {
            author.insert("icon_url".into(), json!(m.author_icon));
        }
        embed.insert("author".into(), Value::Object(author));
    }
    if https(m.thumbnail.as_deref()) {
        embed.insert("thumbnail".into(), json!({ "url": m.thumbnail }));
    }
    if let Some(picture) = picture {
        embed.insert("image".into(), json!({ "url": format!("attachment://{picture}") }));
    }
    if !description.is_empty() {
        embed.insert("description".into(), json!(description));
    }
    if !fields.is_empty() {
        embed.insert("fields".into(), Value::Array(fields.iter().map(|f| json!({ "name": f.name, "value": f.value, "inline": f.inline })).collect()));
    }
    if !footer.is_empty() {
        embed.insert("footer".into(), json!({ "text": footer, "icon_url": LOGO }));
    }
    if let Some(at) = m.timestamp {
        // ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ss.fffZ").
        let c = at.utc();
        let ms = at.unix_ms().rem_euclid(1000);
        embed.insert("timestamp".into(), json!(format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{ms:03}Z", c.year, c.month, c.day, c.hour, c.minute, c.second)));
    }
    embed
}

/// Discord refuses an empty field name or value.
fn blank(text: &str) -> String {
    if text.trim().is_empty() { "\u{200B}".into() } else { text.to_string() }
}

fn https(url: Option<&str>) -> bool {
    url.filter(|u| (1..=2048).contains(&len16(u))).and_then(|u| url::Url::parse(u).ok()).is_some_and(|uri| uri.scheme() == "https")
}

/// The embed as plain text: account and title in bold, the description, a line per short fact ("**Game:** …",
/// several lines joined with " · "), a list under its name, the footer in small text.
fn plain_text(m: &DiscordMessage) -> String {
    let mut sb = String::from("**");
    if let Some(author) = m.author.as_deref().filter(|a| !a.is_empty()) {
        sb.push_str(&escape(Some(author)));
        sb.push_str(" · ");
    }
    sb.push_str(&m.title);
    sb.push_str("**");
    if !m.text.is_empty() {
        sb.push('\n');
        sb.push_str(&m.text);
    }
    for f in &m.fields {
        if f.inline || !f.value.contains('\n') {
            sb.push_str("\n**");
            sb.push_str(&f.name);
            sb.push_str(":** ");
            sb.push_str(&f.value.split('\n').filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" · "));
        } else {
            sb.push_str("\n**");
            sb.push_str(&f.name);
            sb.push_str("**\n");
            sb.push_str(&f.value);
        }
    }
    if let Some(footer) = m.footer.as_deref().filter(|f| !f.is_empty()) {
        sb.push_str("\n-# ");
        sb.push_str(footer);
    }
    LINK_PATTERN.replace_all(&sb, "${1}").into_owned()
}

/// multipart/form-data as .NET's MultipartFormDataContent writes it (a GUID boundary).
pub(crate) struct Multipart {
    boundary: String,
    body: Vec<u8>,
}

impl Multipart {
    pub(crate) fn new() -> Multipart {
        Multipart { boundary: crate::core::base::new_guid().to_string(), body: Vec::new() }
    }
    pub(crate) fn part(&mut self, name: &str, file_name: Option<&str>, content_type: &str, data: &[u8]) {
        self.body.extend_from_slice(format!("--{}\r\nContent-Type: {content_type}\r\n", self.boundary).as_bytes());
        // .NET's ContentDispositionHeaderValue: the name as it is, the file name quoted only when it isn't a token.
        let is_token = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_graphic() && !b"()<>@,;:\\\"/[]?={}".contains(&b));
        match file_name {
            Some(f) => {
                let quoted = if is_token(f) { f.to_string() } else { format!("\"{f}\"") };
                self.body.extend_from_slice(format!("Content-Disposition: form-data; name={name}; filename={quoted}; filename*=utf-8''{f}\r\n\r\n").as_bytes())
            }
            None => self.body.extend_from_slice(format!("Content-Disposition: form-data; name={name}\r\n\r\n").as_bytes()),
        }
        self.body.extend_from_slice(data);
        self.body.extend_from_slice(b"\r\n");
    }
    /// (Content-Type header, body).
    pub(crate) fn finish(mut self) -> (String, Vec<u8>) {
        self.body.extend_from_slice(format!("--{}--\r\n", self.boundary).as_bytes());
        (format!("multipart/form-data; boundary=\"{}\"", self.boundary), self.body)
    }
}

/// Posts a new message (`wait`: Discord answers with it, for its id) or edits one this webhook sent. Never fails for
/// Discord's or the network's answers: the outcome says what happened.
/// `file`: sent with a new message (a summary's picture), as multipart form data.
/// Err only where the C# throws past its catch: a "code" or "retry_after" in Discord's answer that isn't a number
/// (JsonElement.TryGetInt32 / TryGetDouble throw InvalidOperationException there).
pub fn send(url: &str, payload: &mut Map<String, Value>, edit_message_id: Option<&str>, wait: bool, ct: &Cancel, file: Option<&DiscordFile>) -> Res<DiscordResult> {
    let edit = edit_message_id.is_some_and(|id| !id.is_empty());
    let Some(target) = try_normalize(Some(url)) else { return Ok(DiscordResult::of(DiscordOutcome::Gone)) };
    if edit && !is_id(edit_message_id) {
        return Ok(DiscordResult::of(DiscordOutcome::Gone));
    }
    // An edit changes only the fields it names: the other form (embed or text) is cleared, or a switch would leave both.
    if edit {
        if payload.get("content").is_none_or(Value::is_null) {
            payload.insert("content".into(), json!(""));
        }
        if payload.get("embeds").is_none_or(Value::is_null) {
            payload.insert("embeds".into(), json!([]));
        }
    }
    let json_text = to_net_json_string(&*payload)?;
    let (content_type, body) = match file {
        Some(file) if !edit => {
            let mut form = Multipart::new();
            form.part("payload_json", None, "application/json; charset=utf-8", json_text.as_bytes());
            form.part("files[0]", Some(&file.name), "image/png", &file.data);
            form.finish()
        }
        _ => ("application/json; charset=utf-8".to_string(), json_text.into_bytes()),
    };
    let edit_id = edit_message_id.unwrap_or("");
    let address = if edit {
        format!("{target}/messages/{edit_id}")
    } else if wait {
        format!("{target}?wait=true")
    } else {
        target.clone()
    };
    if ct.is_cancelled() {
        log::note("Discord: TaskCanceledException: A task was canceled.");
        return Ok(DiscordResult::of(DiscordOutcome::Offline));
    }
    let req = HTTP.request(if edit { "PATCH" } else { "POST" }, &address).set("User-Agent", "Bloxkeep").set("Content-Type", &content_type);
    let resp = match req.send_bytes(&body) {
        Ok(r) => r,
        Err(ureq::Error::Status(_, r)) => r,
        Err(e) => {
            // ureq's messages name the URL; only the kind goes to the log, never the path with the token. The caller
            // says it plainly (no Error).
            log::note(&format!("Discord: {:?}", e.kind()));
            return Ok(DiscordResult::of(DiscordOutcome::Offline));
        }
    };
    let status = resp.status();
    let header = |name: &str| resp.header(name).map(String::from);
    let (remaining, reset_after, retry_header) = (header("X-RateLimit-Remaining"), header("X-RateLimit-Reset-After"), header("Retry-After"));
    let body = match resp.into_string() {
        Ok(b) => b,
        Err(e) => {
            log::note(&format!("Discord: IOException: {e}"));
            return Ok(DiscordResult::of(DiscordOutcome::Offline));
        }
    };
    // An error page from a proxy: the status code says enough.
    let json: Value = if body.is_empty() { Value::Null } else { serde_json::from_str(&body).unwrap_or(Value::Null) };
    let obj = json.as_object();
    let code = match obj.and_then(|o| o.get("code")) {
        None => 0,
        Some(Value::Number(n)) => n.as_i64().and_then(|n| i32::try_from(n).ok()).unwrap_or(0),
        Some(_) => return Err(Fail::other("Discord's answer has a \"code\" that isn't a number (InvalidOperationException).")),
    };
    let reason = obj.and_then(|o| o.get("message")).and_then(Value::as_str).map(String::from);
    let parse_secs = |s: Option<String>| s.and_then(|s| s.trim().parse::<f64>().ok()).filter(|v| v.is_finite());
    match status {
        200..=299 => {
            let id = obj.and_then(|o| o.get("id")).and_then(Value::as_str).map(String::from);
            // The bucket is empty after this one: the next post waits for it rather than earning a 429.
            let refill = match (remaining.as_deref(), parse_secs(reset_after)) {
                (Some("0"), Some(after)) => Duration::from_secs_f64(after.clamp(0.0, 600.0)),
                _ => Duration::ZERO,
            };
            Ok(DiscordResult { outcome: DiscordOutcome::Sent, message_id: id.or_else(|| edit_message_id.map(String::from)), wait: refill, error: None })
        }
        429 => {
            let seconds = match obj.and_then(|o| o.get("retry_after")) {
                Some(Value::Number(n)) => n.as_f64().unwrap_or(5.0),
                Some(_) => return Err(Fail::other("Discord's answer has a \"retry_after\" that isn't a number (InvalidOperationException).")),
                None => parse_secs(retry_header).unwrap_or(5.0),
            };
            Ok(DiscordResult { wait: Duration::from_secs_f64(seconds.clamp(1.0, 3600.0)), ..DiscordResult::of(DiscordOutcome::RateLimited) })
        }
        // Discord's own word that the webhook is gone: 50027 Invalid Webhook Token (its link was reset), 10015
        // Unknown Webhook. Any other 401/403 (a proxy, a firewall) is a connection problem, tried again later.
        401 | 403 => Ok(if code == 50027 || code == 10015 {
            DiscordResult::with_error(DiscordOutcome::Gone, reason)
        } else {
            DiscordResult::with_error(DiscordOutcome::Offline, Some(format!("HTTP {status}")))
        }),
        // 10008 Unknown Message: the board message was deleted, the webhook is fine. 10015 Unknown Webhook.
        404 => Ok(DiscordResult::with_error(if code == 10008 || (edit && code != 10015) { DiscordOutcome::MessageGone } else { DiscordOutcome::Gone }, reason)),
        s if s >= 500 => Ok(DiscordResult::with_error(DiscordOutcome::Offline, Some(format!("HTTP {status}")))),
        _ => Ok(DiscordResult::with_error(DiscordOutcome::Rejected, Some(cut(Some(&reason.unwrap_or_else(|| format!("HTTP {status}"))), 200)))),
    }
}
