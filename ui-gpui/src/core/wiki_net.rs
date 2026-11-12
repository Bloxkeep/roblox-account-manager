// WikiNet.cs: a game's fan wiki read straight from this PC.
//! A game's fan wiki read straight from this PC, as a browser would read it: any wiki on Fandom or wiki.gg (both are
//! MediaWiki, so one API reads all of them, and a new game's wiki needs nothing here). Only reading: the page asks for
//! a query or a parsed page (WikiSite.tsx turns the answer into the Wiki part), a list of wikis for a game's name, or
//! what wiki a pasted link is. Nothing else is sent, and only to those hosts. Answers are kept a while, so going back
//! to a page asks nothing again.
use std::collections::HashMap;
use std::io::Read;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

use crate::core::base::{Cancel, Fail, Res};
use crate::core::roblox_api::{HTTP_REQUEST_EXCEPTION, TASK_CANCELED_EXCEPTION, escape_data_string, fail_of, lock, try_prop};
use crate::core::{l, log};

// PORT: ureq 2 has no cookie store unless asked (UseCookies = false); gzip as its default feature, brotli with its feature.
static HTTP: LazyLock<ureq::Agent> = LazyLock::new(|| ureq::AgentBuilder::new().redirects(0).timeout(Duration::from_secs(25)).build());
static CACHE: LazyLock<Mutex<HashMap<String, (Instant, Value)>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
const KEEP: Duration = Duration::from_secs(20 * 60);
const CACHE_MAX: usize = 120;

/// Application.ProductVersion without its "+…" build part.
// PORT: the C# read Application.ProductVersion; the crate's version stands in.
fn user_agent() -> String {
    format!("Bloxkeep/{} (+https://bloxkeep.github.io/roblox-account-manager)", env!("CARGO_PKG_VERSION").split('+').next().unwrap_or(""))
}

/// A Fandom or wiki.gg host: lower-case letters, digits, dots and dashes under one of the two.
static HOST_RULE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)*\.(fandom\.com|wiki\.gg)$").unwrap());
/// A Fandom wiki in another language lives under its code ("/vi", "/pt-br").
static LANG_RULE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^/[a-z]{2,3}(-[a-z]{2,4})?$").unwrap());
static LINK_LANG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(/[a-z]{2,3}(-[a-z]{2,4})?)/(wiki/|$)").unwrap());
static WIKI_SUFFIX: LazyLock<Regex> = LazyLock::new(|| RegexBuilder::new(r"\s*wiki\s*$").case_insensitive(true).build().unwrap());
static SPIN_OFF: LazyLock<Regex> = LazyLock::new(|| RegexBuilder::new(r"\b(fanon|ideas?|fan ?made|rp|roleplay|but)\b").case_insensitive(true).build().unwrap());
static NOT_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-z0-9]+").unwrap());

fn host_of(u: &Url) -> String {
    u.host_str().unwrap_or("").to_lowercase()
}

/// The wiki a site address names ("https://x.fandom.com" or "https://x.fandom.com/vi"), or None.
pub fn site(site: Option<&str>) -> Option<String> {
    let text = site?.trim().trim_end_matches('/');
    let u = Url::parse(text).ok()?;
    // PORT: Uri.IsDefaultPort: the url crate drops a default port, so any port left is another.
    if u.scheme() != "https" || u.port().is_some() || !u.username().is_empty() || u.password().is_some() || u.query().is_some() || u.fragment().is_some() {
        return None;
    }
    let host = host_of(&u);
    let path = u.path().trim_end_matches('/').to_lowercase();
    if !HOST_RULE.is_match(&host) || host.starts_with("services.") || host.starts_with("community.") {
        return None;
    }
    if !path.is_empty() && !LANG_RULE.is_match(&path) {
        return None;
    }
    Some(format!("https://{host}{path}"))
}

/// The wiki a pasted link points into ("deepwoken.fandom.com/wiki/Shotel", "https://x.wiki.gg/"), or None.
pub fn site_of_link(link: &str) -> Option<String> {
    let mut s = link.trim().to_string();
    if !s.contains("://") {
        s = format!("https://{s}");
    }
    let u = Url::parse(&s).ok()?;
    if u.scheme() != "https" && u.scheme() != "http" {
        return None;
    }
    let path = u.path().to_lowercase();
    let lang = LINK_LANG.captures(&path).map(|m| m[1].to_string()).unwrap_or_default();
    site(Some(&format!("https://{}{lang}", u.host_str().unwrap_or(""))))
}

// expandtemplates: a wiki's own short codes in words ({{Rarity|L}} → Legendary), for the tables (WikiSite.tsx).
const ACTIONS: [&str; 3] = ["query", "parse", "expandtemplates"];
/// Settings a request may carry: what is asked of MediaWiki, never how it answers or who asks.
const NEVER: [&str; 13] =
    ["format", "callback", "origin", "token", "centralauthtoken", "assert", "assertuser", "requestid", "servedby", "curtimestamp", "maxlag", "uselang", "errorformat"];

fn not_a_wiki() -> Fail {
    Fail::user(l::t("That isn't a wiki Bloxkeep can read."))
}

fn unreachable() -> Fail {
    Fail::user(l::t("The wiki can't be reached right now."))
}

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// One read of a wiki's API (action query, parse or expandtemplates), as JSON. Fails with Fail::User when it can't be read.
pub fn api(site_text: &str, q: &HashMap<String, String>, ct: &Cancel) -> Res<Value> {
    let wiki = site(Some(site_text)).ok_or_else(not_a_wiki)?;
    if !q.get("action").is_some_and(|a| ACTIONS.contains(&a.as_str())) {
        return Err(not_a_wiki());
    }
    let mut pairs: Vec<(&String, &String)> =
        q.iter().filter(|(k, v)| !NEVER.contains(&k.as_str()) && utf16_len(k) <= 40 && utf16_len(v) <= 2000).collect();
    pairs.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
    let query: Vec<String> = pairs.iter().map(|(k, v)| format!("{}={}", escape_data_string(k), escape_data_string(v))).collect();
    let url = format!("{wiki}/api.php?{}&format=json&formatversion=2", query.join("&"));
    get_json(&url, ct)
}

/// Fandom's own list of wikis for a game's name, best first: a wiki named after the game, then the most read.
pub fn find(name: &str, ct: &Cancel) -> Res<Vec<WikiFound>> {
    let words = name.trim();
    if words.is_empty() || utf16_len(words) > 80 {
        return Ok(Vec::new());
    }
    let url = format!("https://services.fandom.com/unified-search/community-search?query={}&lang=en&limit=25", escape_data_string(words));
    let d = get_json(&url, ct)?;
    let mut found = Vec::new();
    let Some(Value::Array(results)) = try_prop(&d, "results")? else { return Ok(found) };
    let want = fold(words);
    for r in results {
        let (Some(site_text), title) = (site(text(r, "url").as_deref()), text(r, "name")) else { continue };
        let Some(title) = title.filter(|t| !t.trim().is_empty()) else { continue };
        if text(r, "language").is_some_and(|lang| lang != "en") {
            continue;
        }
        let (views, pages) = (number(r, "pageViews"), number(r, "pageCount"));
        let bare = fold(&WIKI_SUFFIX.replace_all(&title, ""));
        // Named after the game outright, then with its name in it; a fan-made spin-off ("… Fanon", "… Ideas") last.
        let mut matched = if bare == want {
            3
        } else if bare.starts_with(&want) || want.starts_with(&bare) {
            2
        } else if bare.contains(&want) {
            1
        } else {
            0
        };
        if SPIN_OFF.is_match(&title) {
            matched = matched.min(0);
        }
        let img = text(r, "thumbnail").filter(|t| t.starts_with("https://static.wikia.nocookie.net/"));
        found.push(WikiFound { site: site_text, name: title.trim().to_string(), pages, views, img, r#match: matched });
    }
    found.sort_by(|a, b| b.r#match.cmp(&a.r#match).then(b.views.cmp(&a.views)));
    found.truncate(8);
    Ok(found)
}

/// What wiki a site is: its name, articles, main page and logo; None when it isn't one.
pub fn about(site_text: &str, ct: &Cancel) -> Res<Option<WikiAbout>> {
    let Some(wiki) = site(Some(site_text)) else { return Ok(None) };
    let q: HashMap<String, String> =
        [("action", "query"), ("meta", "siteinfo"), ("siprop", "general|statistics")].iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    let d = match api(&wiki, &q, ct) {
        Ok(d) => d,
        Err(Fail::User(_)) => return Ok(None),
        Err(e) => return Err(e),
    };
    // TryGetProperty on an answer that isn't an object throws in the C# (InvalidOperationException): so does try_prop.
    let Some(query) = try_prop(&d, "query")? else { return Ok(None) };
    let Some(g) = try_prop(query, "general")? else { return Ok(None) };
    // A wiki that moved answers from its new address: that one is kept.
    let server = match text(g, "server") {
        Some(s) => {
            let s = if s.starts_with("//") { format!("https:{s}") } else { s };
            site(Some(&format!("{s}{}", lang_path(&wiki)))).unwrap_or_else(|| wiki.clone())
        }
        None => site(Some(&wiki)).unwrap_or_else(|| wiki.clone()),
    };
    let articles = match try_prop(query, "statistics")? {
        Some(st) => number(st, "articles"),
        None => 0,
    };
    let logo = text(g, "logo").and_then(|l| {
        let l = if l.starts_with("//") { format!("https:{l}") } else { l };
        let lu = Url::parse(&l).ok()?;
        let host = host_of(&lu);
        (lu.scheme() == "https" && (host == "static.wikia.nocookie.net" || HOST_RULE.is_match(&host))).then(|| lu.as_str().to_string())
    });
    let name = text(g, "sitename").unwrap_or_else(|| Url::parse(&server).map(|u| host_of(&u)).unwrap_or_default());
    Ok(Some(WikiAbout { site: server, name, articles, main_page: text(g, "mainpage").unwrap_or_else(|| "Main Page".into()), logo }))
}

fn lang_path(wiki: &str) -> String {
    Url::parse(wiki).map(|u| u.path().trim_end_matches('/').to_string()).unwrap_or_default()
}

/// What failed, as AppController.LogKindOf words it.
fn log_kind(f: &Fail) -> String {
    crate::core::app::App::log_kind_of(f)
}

/// A GET of JSON, kept a while; a move to another wiki host (a renamed wiki) is followed, nowhere else.
fn get_json(url: &str, ct: &Cancel) -> Res<Value> {
    if let Some((at, value)) = lock(&CACHE).get(url)
        && at.elapsed() < KEEP
    {
        return Ok(value.clone());
    }
    let mut at = url.to_string();
    let mut hop = 0;
    loop {
        // A cancelled read fails as the C#'s TaskCanceledException did, without a note.
        if ct.is_cancelled() {
            return Err(unreachable());
        }
        let result = HTTP.get(&at).set("User-Agent", &user_agent()).set("Accept", "application/json").call();
        let res = match result {
            Ok(res) => res,
            Err(ureq::Error::Status(_, res)) => res,
            Err(ureq::Error::Transport(t)) => {
                if !ct.is_cancelled() {
                    let text = t.to_string();
                    let kind = if text.contains("timed out") || text.contains("timeout") { TASK_CANCELED_EXCEPTION } else { HTTP_REQUEST_EXCEPTION };
                    log::note(&format!("Wiki: request failed ({}).", log_kind(&fail_of(kind, text))));
                }
                return Err(unreachable());
            }
        };
        let status = res.status();
        if (300..400).contains(&status)
            && let Some(next) = res.header("Location").map(str::to_string)
            && hop < 3
        {
            let to = Url::parse(&at).and_then(|base| base.join(&next));
            let ours = to.as_ref().is_ok_and(|to| to.scheme() == "https" && (HOST_RULE.is_match(&host_of(to)) || host_of(to) == "services.fandom.com"));
            if !ours {
                log::note("Wiki: a redirect to a site that isn't a wiki was refused.");
                return Err(not_a_wiki());
            }
            at = to.map(|t| t.to_string()).unwrap_or_default();
            hop += 1;
            continue;
        }
        if !(200..300).contains(&status) {
            log::note(&format!("Wiki: HTTP {status}."));
            return Err(if matches!(status, 404 | 401 | 410) { not_a_wiki() } else { unreachable() });
        }
        let length = res.header("Content-Length").and_then(|v| v.trim().parse::<i64>().ok());
        if let Some(length) = length.filter(|l| *l > 12_000_000) {
            log::note(&format!("Wiki: an answer of {} MB was too big to read.", length / 1_000_000));
            return Err(Fail::user(l::t("That wiki page is too big to show here.")));
        }
        let mut body = Vec::new();
        if let Err(e) = res.into_reader().read_to_end(&mut body) {
            if !ct.is_cancelled() {
                let kind = if e.kind() == std::io::ErrorKind::TimedOut { TASK_CANCELED_EXCEPTION } else { "IOException" };
                log::note(&format!("Wiki: reading the answer failed ({}).", log_kind(&fail_of(kind, e))));
            }
            return Err(unreachable());
        }
        let bytes = body.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&body);
        let value: Value = match serde_json::from_slice(bytes) {
            Ok(v) => v,
            Err(_) => {
                log::note("Wiki: the answer wasn't JSON.");
                return Err(not_a_wiki());
            }
        };
        let mut cache = lock(&CACHE);
        if cache.len() >= CACHE_MAX {
            let mut by_age: Vec<(Instant, String)> = cache.iter().map(|(k, (t, _))| (*t, k.clone())).collect();
            by_age.sort_by_key(|(t, _)| *t);
            for (_, old) in by_age.into_iter().take(CACHE_MAX / 4) {
                cache.remove(&old);
            }
        }
        cache.insert(url.to_string(), (Instant::now(), value.clone()));
        return Ok(value);
    }
}

fn text(e: &Value, name: &str) -> Option<String> {
    match e {
        Value::Object(o) => match o.get(name) {
            Some(Value::String(v)) => Some(v.clone()),
            _ => None,
        },
        _ => None,
    }
}

fn number(e: &Value, name: &str) -> i64 {
    match e {
        Value::Object(o) => o.get(name).and_then(Value::as_i64).unwrap_or(0),
        _ => 0,
    }
}

fn fold(s: &str) -> String {
    NOT_WORD.replace_all(&s.to_lowercase(), " ").trim().to_string()
}

/// A link the Wiki part may open in the browser: https, anywhere (a wiki's own links go out to the game's
/// Roblox page, its Discord, videos…), as the wiki in a browser would.
pub fn openable(url: &str) -> bool {
    Url::parse(url).is_ok_and(|u| u.scheme() == "https" && u.username().is_empty() && u.password().is_none()) && utf16_len(url) <= 2000
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct WikiFound {
    pub site: String,
    pub name: String,
    pub pages: i64,
    pub views: i64,
    pub img: Option<String>,
    pub r#match: i32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct WikiAbout {
    pub site: String,
    pub name: String,
    pub articles: i64,
    pub main_page: String,
    pub logo: Option<String>,
}
