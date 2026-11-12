// WikiLink.cs: the Wiki tab's data from Bloxkeep's Discord bot.
//! The Wiki tab's data from Bloxkeep's Discord bot (/wiki: the games; /wiki/<game>: one game), which reads the games'
//! wikis and price lists once a day: the app never calls a wiki itself, and a game the bot adds shows here without an
//! update. What comes is checked: pictures only from Fandom's picture host (a price list's only through the bot), links
//! only to the wiki or website it names. Kept for half an hour.
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

use crate::core::base::{Cancel, Fail, Res};
use crate::core::feed_link::null_default;
use crate::core::roblox_api::{HttpRequest, JSON_EXCEPTION, fail_of, lock, send_on, try_prop};
use crate::core::{l, wiki_net};

/// A game in the Games tab: a price list (kind "prices"), a wiki's items ("wiki") or only the game ("game": the app finds
/// its wiki), how many, when the bot read them (ms), and its place on Roblox (0 when the bot names none), for its picture
/// and colour (wiki_look). Wiki: the wiki the bot names for it (Fandom's search would find another first), None to find
/// it by name.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct WikiGameInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub count: i32,
    pub at: Option<i64>,
    pub place: i64,
    pub wiki: Option<String>,
}

/// Where a game's items come from: the wiki's (or website's) name and address, and the licence of its words.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct WikiSource {
    pub name: String,
    pub url: String,
}

/// A wiki page as an item (services/DiscordBot/worker/wiki.js): its name, category, the lines of its box of facts in
/// plain words ([name, value]), its first lines, its picture (on Fandom's picture host) and its page.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct WikiItem {
    pub id: String,
    pub name: String,
    pub cat: String,
    pub fields: Vec<[String; 2]>,
    pub lead: String,
    pub img: Option<String>,
    pub url: Option<String>,
}

/// One game of the Wiki tab as the bot gave it: a wiki's items, or (Prices) a price list.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct WikiGame {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub at: Option<i64>,
    pub source: WikiSource,
    pub license: Option<WikiSource>,
    pub cats: Vec<String>,
    pub items: Vec<WikiItem>,
    pub prices: Vec<FeedPriceItem>,
}

/// Where a game's prices come from: the website's name and address (shown and linked under the list).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct FeedPriceSource {
    pub name: String,
    pub url: String,
}

/// An item in a game's price list (services/DiscordBot/worker/prices.js): its name, kind, picture (through Bloxkeep's
/// bot), value and permanent value, demand of 10 and trend, prices in Beli and Robux, and its page on the website.
// PORT: read with the bot's camelCase names (System.Text.Json matched them ignoring case), written PascalCase.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(serialize = "PascalCase", deserialize = "camelCase"), default)]
pub struct FeedPriceItem {
    #[serde(deserialize_with = "null_default")]
    pub id: String,
    #[serde(deserialize_with = "null_default")]
    pub name: String,
    pub category: Option<String>,
    pub rarity: Option<String>,
    pub r#type: Option<String>,
    pub img: Option<String>,
    pub value: Option<f64>,
    pub perm: Option<f64>,
    pub demand: Option<f64>,
    pub perm_demand: Option<f64>,
    pub trend: Option<String>,
    pub perm_trend: Option<String>,
    pub beli: Option<f64>,
    pub robux: Option<f64>,
    pub best_for: Option<String>,
    pub updated: Option<i64>,
    pub url: Option<String>,
}

pub const BASE: &str = "https://bloxkeep-bot.bloxkeep-app.workers.dev";
const MAX_ITEMS: usize = 2000;
const MAX_FIELDS: usize = 30;
const MAX_TEXT: usize = 1000;
const KEEP: Duration = Duration::from_secs(30 * 60);
static HTTP: LazyLock<ureq::Agent> = LazyLock::new(|| ureq::AgentBuilder::new().redirects(0).timeout(Duration::from_secs(30)).build());
static ID_RULE: LazyLock<Regex> = LazyLock::new(|| Regex::new("^[a-z0-9-]{1,40}$").unwrap());

#[derive(Clone)]
enum KeptValue {
    Games(Vec<WikiGameInfo>),
    Game(WikiGame),
}

/// The C#'s Kept, read and written without a lock there.
static KEPT: LazyLock<Mutex<HashMap<String, (Instant, KeptValue)>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// The bot's address: the one game news uses in a test (BLOXKEEP_FEEDS_URL, ws:// as http://), else Bloxkeep's.
pub fn base_of(feeds_url: Option<&str>) -> Url {
    if let Some(u) = feeds_url.filter(|f| !f.is_empty()).and_then(|f| Url::parse(f).ok())
        && let Some(host) = u.host_str()
    {
        let authority = match u.port() {
            Some(port) => format!("{host}:{port}"),
            None => host.to_string(),
        };
        if let Ok(base) = Url::parse(&format!("{}://{authority}", if u.scheme() == "ws" { "http" } else { "https" })) {
            return base;
        }
    }
    Url::parse(BASE).unwrap()
}

pub fn games(bot: &Url, ct: &Cancel) -> Res<Vec<WikiGameInfo>> {
    if let Some(KeptValue::Games(kept)) = fresh("") {
        return Ok(kept);
    }
    let doc = get(&join(bot, "/wiki")?, ct)?;
    let mut games = Vec::new();
    if let Some(Value::Array(list)) = try_prop(&doc, "games")? {
        for g in list.iter().take(100) {
            if let Some(id) = str_of(g, "id").filter(|i| (1..=40).contains(&utf16_len(i)))
                && let Some(name) = str_of(g, "name").filter(|n| !n.is_empty())
                && let Some(kind) = str_of(g, "kind").filter(|k| matches!(k.as_str(), "wiki" | "prices" | "game"))
            {
                games.push(WikiGameInfo {
                    id,
                    name: cut(&name, 60),
                    kind,
                    count: int_of(g, "count")?,
                    at: long_of(g, "at")?,
                    place: 0.max(long_of(g, "place")?.unwrap_or(0)),
                    wiki: wiki_net::site(str_of(g, "wiki").as_deref()),
                });
            }
        }
    }
    lock(&KEPT).insert(String::new(), (Instant::now(), KeptValue::Games(games.clone())));
    Ok(games)
}

pub fn game(bot: &Url, id: &str, ct: &Cancel) -> Res<Option<WikiGame>> {
    if !ID_RULE.is_match(id) {
        return Ok(None);
    }
    if let Some(KeptValue::Game(kept)) = fresh(id) {
        return Ok(Some(kept));
    }
    let root = get(&join(bot, &format!("/wiki/{id}"))?, ct)?;
    let kind = str_of(&root, "kind").unwrap_or_default();
    if !matches!(kind.as_str(), "wiki" | "prices") {
        return Ok(None);
    }
    let Some(src) = try_prop(&root, "source")? else { return Ok(None) };
    let Some(site) = str_of(src, "url").and_then(|u| Url::parse(&u).ok()).filter(|u| u.scheme() == "https") else { return Ok(None) };
    let site_host = site.host_str().unwrap_or("").to_string();
    let on_site = |url: Option<&str>| url.and_then(|u| Url::parse(u).ok()).is_some_and(|u| u.scheme() == "https" && u.host_str() == Some(site_host.as_str()));
    let source = WikiSource { name: cut(&str_of(src, "name").unwrap_or_else(|| site_host.clone()), 80), url: site.as_str().trim_end_matches('/').to_string() };
    let mut game = WikiGame {
        id: id.to_string(),
        name: cut(&str_of(&root, "name").unwrap_or_else(|| id.to_string()), 60),
        kind: kind.clone(),
        at: long_of(&root, "at")?,
        source,
        ..WikiGame::default()
    };
    if kind == "prices" {
        let bot_host = bot.host_str().unwrap_or("").to_string();
        let from_bot = |img: Option<&str>| {
            img.and_then(|i| Url::parse(i).ok()).is_some_and(|u| {
                u.host_str() == Some(bot_host.as_str())
                    && u.port_or_known_default() == bot.port_or_known_default()
                    && (u.scheme() == "https" || (bot.scheme() == "http" && u.scheme() == "http"))
                    && u.path() == "/prices/img"
            })
        };
        let items: Vec<Option<FeedPriceItem>> = match try_prop(&root, "items")? {
            Some(arr @ Value::Array(_)) => serde_json::from_value(arr.clone()).map_err(|e| fail_of(JSON_EXCEPTION, e))?,
            _ => Vec::new(),
        };
        game.prices = items
            .into_iter()
            .flatten()
            .filter(|i| !i.id.is_empty() && !i.name.is_empty())
            .take(MAX_ITEMS)
            .map(|mut i| {
                if !from_bot(i.img.as_deref()) {
                    i.img = None;
                }
                if !on_site(i.url.as_deref()) {
                    i.url = None;
                }
                i
            })
            .collect();
    } else {
        let mut items = Vec::new();
        if let Some(Value::Array(arr)) = try_prop(&root, "items")? {
            for i in arr.iter().take(MAX_ITEMS) {
                let (Some(item_id), Some(name)) = (str_of(i, "id").filter(|s| !s.is_empty()), str_of(i, "name").filter(|s| !s.is_empty())) else { continue };
                let mut fields = Vec::new();
                if let Some(Value::Array(fs)) = try_prop(i, "fields")? {
                    for f in fs.iter().take(MAX_FIELDS) {
                        if let Value::Array(pair) = f
                            && pair.len() == 2
                            && let (Value::String(a), Value::String(b)) = (&pair[0], &pair[1])
                        {
                            fields.push([cut(a, 60), cut(b, MAX_TEXT)]);
                        }
                    }
                }
                let img = str_of(i, "img").filter(|p| {
                    Url::parse(p).is_ok_and(|pu| pu.scheme() == "https" && pu.host_str() == Some("static.wikia.nocookie.net"))
                });
                let url = str_of(i, "url");
                items.push(WikiItem {
                    id: cut(&item_id, 60),
                    name: cut(&name, 100),
                    cat: cut(&str_of(i, "cat").unwrap_or_default(), 40),
                    fields,
                    lead: cut(&str_of(i, "lead").unwrap_or_default(), MAX_TEXT),
                    img,
                    url: if on_site(url.as_deref()) { url } else { None },
                });
            }
        }
        let cats = match try_prop(&root, "cats")? {
            Some(Value::Array(cs)) => cs.iter().filter_map(|c| c.as_str()).map(|c| cut(c, 40)).take(40).collect(),
            _ => Vec::new(),
        };
        let license = match try_prop(&root, "license")? {
            Some(lic) => match (str_of(lic, "name").filter(|n| !n.is_empty()), str_of(lic, "url")) {
                (Some(ln), Some(lu)) if lu.starts_with("https://creativecommons.org/") => Some(WikiSource { name: cut(&ln, 40), url: lu }),
                _ => None,
            },
            None => None,
        };
        game.items = items;
        game.cats = cats;
        game.license = license;
    }
    lock(&KEPT).insert(id.to_string(), (Instant::now(), KeptValue::Game(game.clone())));
    Ok(Some(game))
}

/// Whether a link is one of a game the tab has shown: its wiki or website, or one of its items' pages.
pub fn knows(url: &str) -> bool {
    lock(&KEPT).values().any(|(_, v)| match v {
        KeptValue::Game(g) => {
            g.source.url == url
                || g.license.as_ref().is_some_and(|l| l.url == url)
                || g.items.iter().any(|i| i.url.as_deref() == Some(url))
                || g.prices.iter().any(|i| i.url.as_deref() == Some(url))
        }
        KeptValue::Games(_) => false,
    })
}

fn fresh(key: &str) -> Option<KeptValue> {
    lock(&KEPT).get(key).filter(|(at, _)| at.elapsed() < KEEP).map(|(_, v)| v.clone())
}

/// new Uri(bot, path).
fn join(bot: &Url, path: &str) -> Res<Url> {
    bot.join(path).map_err(|e| fail_of("UriFormatException", e))
}

fn get(url: &Url, ct: &Cancel) -> Res<Value> {
    ct.check()?;
    let res = send_on(&HTTP, &HttpRequest::new("GET", url.as_str()))?;
    if !res.is_success() {
        return Err(Fail::user(l::t("The wiki can't be reached right now. Try again in a moment.")));
    }
    let body = res.body.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&res.body);
    serde_json::from_slice(body).map_err(|e| fail_of(JSON_EXCEPTION, e))
}

fn str_of(e: &Value, name: &str) -> Option<String> {
    match e {
        Value::Object(o) => match o.get(name) {
            Some(Value::String(v)) => Some(v.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// TryGetProperty throws on an element that isn't an object, as in the C#.
fn int_of(e: &Value, name: &str) -> Res<i32> {
    Ok(match try_prop(e, name)? {
        Some(Value::Number(n)) => n.as_i64().and_then(|n| i32::try_from(n).ok()).unwrap_or(0),
        _ => 0,
    })
}

fn long_of(e: &Value, name: &str) -> Res<Option<i64>> {
    Ok(match try_prop(e, name)? {
        Some(Value::Number(n)) => n.as_i64(),
        _ => None,
    })
}

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// The first n UTF-16 units, as s[..n] in the C#.
fn cut(s: &str, n: usize) -> String {
    let units: Vec<u16> = s.encode_utf16().collect();
    if units.len() <= n { s.to_string() } else { String::from_utf16_lossy(&units[..n]) }
}
