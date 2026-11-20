// AppController.Wiki.cs: the Wiki tab: each game's items (weapons, gear, bosses… from the game's fan wiki) and price
// lists, which Bloxkeep's Discord bot reads once a day (wiki_link.rs, services/DiscordBot/worker/wiki.js and prices.js).
// The page asks for the games when the tab opens and for a game's items when it is picked; links open only to what the
// tab showed. Only an installed Bloxkeep asks (as game news does), or one pointed at a server of its own
// (BLOXKEEP_FEEDS_URL).
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

use serde_json::{Map, Value, json};
use url::Url;

use super::App;
use super::controller::{JsonKind, bool_of, has, long_of, opt_str, str_of};
use super::feeds::feeds_test_url;
use super::presets::utf16_len;
use super::stats::PLAY_HISTORY;
use crate::core::{
    base::{Cancel, Fail, Kind, Res, Stamp},
    l, log,
    models::PlaySession,
    prefs::{self, WikiMark, WikiPick},
    rt, wiki_link, wiki_look, wiki_net,
};

/// A game the app has looked up, as the stays' sums need it: its universe, name and start place.
#[derive(Clone, Debug, PartialEq)]
pub struct KnownGame {
    pub universe: i64,
    pub name: String,
    pub root: i64,
}

/// One row of `played_games_of`: name, place, hours, last start.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayedGame {
    pub name: String,
    pub place: i64,
    pub hours: f64,
    pub last: Stamp,
}

/// Math.Round(x, 1): .NET rounds half to even.
fn round1(x: f64) -> f64 {
    let scaled = x * 10.0;
    let r = scaled.round();
    let r = if (scaled - scaled.trunc()).abs() == 0.5 && r % 2.0 != 0.0 { r - scaled.signum() } else { r };
    r / 10.0
}

impl App {
    pub fn wiki_bot(&self) -> Url {
        wiki_link::base_of(feeds_test_url().as_deref())
    }

    pub async fn wiki_games(self: &Rc<Self>, again: bool) -> Res<Value> {
        if !self.feeds_allowed() {
            return Ok(json!({ "games": [], "off": true }));
        }
        let bot = self.wiki_bot();
        let games = rt::blocking(move || wiki_link::games(&bot, again, &Cancel::new())).await?;
        Ok(json!({
            "games": games.iter().map(|g| json!({
                "id": g.id, "name": g.name, "kind": g.kind, "count": g.count, "at": g.at, "place": g.place, "wiki": g.wiki,
            })).collect::<Vec<_>>(),
        }))
    }

    /// The games this PC's accounts played in the last 90 days (Statistics' stays), most played first, as cards of the Games
    /// tab with their wiki found by name: {name, place (the game's start place when known), hours, last (ms)}. None while locked.
    pub fn played_games(&self) -> Value {
        if self.open().is_none() {
            return json!([]);
        }
        let Ok(sessions) = self.vault(|d| d.play_sessions.clone()) else { return json!([]) };
        let known = |place: i64| self.known_game_of(place);
        Value::Array(
            App::played_games_of(&sessions, &known, Stamp::utc_now())
                .into_iter()
                .map(|g| json!({ "name": g.name, "place": g.place, "hours": g.hours, "last": App::unix_ms(g.last) }))
                .collect(),
        )
    }

    pub fn known_game_of(&self, place: i64) -> Option<KnownGame> {
        self.controller
            .games
            .borrow()
            .get(&place)
            .cloned()
            .flatten()
            .map(|g| KnownGame { universe: g.universe_id, name: g.name, root: g.place_id })
    }

    /// new DateTimeOffset(DateTime.SpecifyKind(utc, DateTimeKind.Utc)).ToUnixTimeMilliseconds(): the time as written,
    /// read as UTC.
    pub fn unix_ms(at: Stamp) -> i64 {
        let offset_minutes: i64 = match at.kind() {
            Kind::Utc => 0,
            Kind::Local(m) => m as i64,
            Kind::Unspecified => (crate::time::utc_offset_secs() / 60) as i64,
        };
        at.unix_ms() + offset_minutes * 60_000
    }

    /// The sum behind `played_games`, apart from where the stays and the looked-up games come from. The GPUI interface
    /// makes the same sum itself from the raw stays (ui-gpui/src/history.rs, since 2026-11-11); one table of cases,
    /// Tests/played-games-cases.json, holds both to the same answers (suite "played-games" here, a test there).
    pub fn played_games_of(sessions: &[PlaySession], known: &dyn Fn(i64) -> Option<KnownGame>, now: Stamp) -> Vec<PlayedGame> {
        let since = now.plus_secs(-PLAY_HISTORY.as_secs_f64());
        let mut universe_of: HashMap<i64, i64> = HashMap::new();
        for s in sessions.iter().filter(|s| s.universe_id > 0) {
            universe_of.insert(s.place_id, s.universe_id);
        }
        let key_of = |s: &PlaySession| -> i64 {
            if s.universe_id > 0 {
                return s.universe_id;
            }
            if let Some(&u) = universe_of.get(&s.place_id)
                && u > 0
            {
                return u;
            }
            match known(s.place_id).map(|k| k.universe) {
                Some(c) if c > 0 => c,
                _ => -s.place_id,
            }
        };
        // GroupBy keeps the order the keys first come in.
        let mut groups: Vec<(i64, Vec<&PlaySession>)> = Vec::new();
        for s in sessions.iter().filter(|s| s.place_id > 0 && s.ended_at.or(s.last_seen_at).unwrap_or(now) > since) {
            let key = key_of(s);
            match groups.iter_mut().find(|g| g.0 == key) {
                Some(g) => g.1.push(s),
                None => groups.push((key, vec![s])),
            }
        }
        let mut rows: Vec<PlayedGame> = Vec::new();
        for (_, g) in groups {
            // MaxBy: the first of the latest.
            let mut last = g[0];
            for s in &g {
                if s.started_at > last.started_at {
                    last = s;
                }
            }
            let mut named: Option<&PlaySession> = None;
            for s in g.iter().filter(|s| !s.game_name.is_empty()) {
                if named.is_none_or(|n| s.started_at > n.started_at) {
                    named = Some(s);
                }
            }
            let name = match named {
                Some(s) => s.game_name.clone(),
                None => known(last.place_id).map(|k| k.name).unwrap_or_default(),
            };
            let root = g.iter().map(|s| known(s.place_id).map_or(0, |k| k.root)).find(|p| *p > 0).unwrap_or(0);
            let place = if root > 0 { root } else { last.place_id };
            let hours: f64 = g
                .iter()
                .map(|s| (s.ended_at.or(s.last_seen_at).unwrap_or(now).secs_since(if s.started_at > since { s.started_at } else { since }) / 3600.0).max(0.0))
                .sum();
            rows.push(PlayedGame { name, place, hours, last: last.started_at });
        }
        rows.retain(|g| !g.name.is_empty());
        // OrderByDescending is a stable sort.
        rows.sort_by(|a, b| b.hours.partial_cmp(&a.hours).unwrap_or(std::cmp::Ordering::Equal));
        rows.truncate(30);
        for r in &mut rows {
            r.hours = round1(r.hours);
        }
        rows
    }

    /// The stays as they are kept, for the GPUI interface to make its sums from (ui-gpui/src/history.rs): each stay's
    /// account, place, universe, game name, start, end and last-seen (ms), why it ended; the games looked up for their
    /// places; and this PC's clock. Account names are left out while Hide account names is on. Nothing while locked.
    pub fn play_sessions_raw(&self) -> Value {
        let now = App::unix_ms(Stamp::utc_now());
        let empty = json!({ "now": now, "sessions": [], "games": {}, "bans": [] });
        if self.open().is_none() {
            return empty;
        }
        let Ok((sessions, bans)) = self.vault(|d| {
            let bans: Vec<(crate::core::base::Guid, i64)> = d.accounts.iter().flat_map(|a| a.game_bans.iter().map(move |b| (a.id, b.place_id))).collect();
            (d.play_sessions.clone(), bans)
        }) else {
            return empty;
        };
        let hide = prefs::lock().hide_names;
        let list: Vec<Value> = sessions
            .iter()
            .map(|s| {
                json!({
                    "account": s.account_id, "label": if hide { None } else { s.account_label.clone() }, "place": s.place_id, "universe": s.universe_id,
                    "game": s.game_name, "start": App::unix_ms(s.started_at), "end": s.ended_at.map(App::unix_ms), "seen": s.last_seen_at.map(App::unix_ms),
                    "reason": s.end_reason, "code": s.reason_code, "preset": s.preset_id,
                })
            })
            .collect();
        let mut places: Vec<i64> = Vec::new();
        let mut seen: HashSet<i64> = HashSet::new();
        for p in sessions.iter().map(|s| s.place_id).chain(bans.iter().map(|b| b.1)) {
            if seen.insert(p) {
                places.push(p);
            }
        }
        let mut games = Map::new();
        for p in places {
            if let Some(g) = self.known_game_of(p) {
                games.insert(p.to_string(), json!({ "universe": g.universe, "name": g.name, "root": g.root }));
            }
        }
        json!({
            "now": now,
            "sessions": list,
            "games": games,
            // The games that banned an account (a game's page says so in place of when the account last played).
            "bans": bans.iter().map(|(account, place)| json!({ "account": account, "place": place })).collect::<Vec<_>>(),
        })
    }

    /// How the Games tab draws each game (its picture, icon and colour), by Roblox place.
    pub async fn game_looks(self: &Rc<Self>, a: &Value) -> Res<Value> {
        let mut places: Vec<i64> = Vec::new();
        if has(a, "places", JsonKind::Array) {
            for x in a["places"].as_array().into_iter().flatten() {
                let v = x.as_i64().unwrap_or(0);
                if v > 0 && !places.contains(&v) {
                    places.push(v);
                    if places.len() >= 100 {
                        break;
                    }
                }
            }
        }
        let looks = rt::blocking(move || wiki_look::of(places)).await?;
        let mut out = Map::new();
        for (place, look) in looks {
            out.insert(place.to_string(), json!({ "icon": look.icon, "cover": look.cover, "tint": look.tint, "ink": look.ink }));
        }
        Ok(Value::Object(out))
    }

    pub async fn wiki_game(self: &Rc<Self>, id: &str) -> Res<Value> {
        let allowed = self.feeds_allowed();
        let found = if allowed {
            let bot = self.wiki_bot();
            let id = id.to_string();
            rt::blocking(move || wiki_link::game(&bot, &id, &Cancel::new())).await?
        } else {
            None
        };
        let Some(g) = found else {
            log::note(&format!("Wiki: a game's items not shown ({}).", if allowed { "the bot had nothing for it" } else { "off for this copy" }));
            return Ok(Value::Null);
        };
        let source = json!({ "name": g.source.name, "url": g.source.url });
        if g.kind == "prices" {
            return Ok(json!({
                "id": g.id, "name": g.name, "kind": g.kind, "at": g.at, "source": source,
                "items": g.prices.iter().map(|i| json!({
                    "id": i.id, "name": i.name, "category": i.category, "rarity": i.rarity, "type": i.r#type, "img": i.img, "value": i.value, "perm": i.perm,
                    "demand": i.demand, "permDemand": i.perm_demand, "trend": i.trend, "permTrend": i.perm_trend, "beli": i.beli, "robux": i.robux, "bestFor": i.best_for,
                    "updated": i.updated, "url": i.url,
                })).collect::<Vec<_>>(),
            }));
        }
        Ok(json!({
            "id": g.id, "name": g.name, "kind": g.kind, "at": g.at, "source": source, "cats": g.cats,
            "license": g.license.as_ref().map(|l| json!({ "name": l.name, "url": l.url })),
            "items": g.items.iter().map(|i| json!({ "id": i.id, "name": i.name, "cat": i.cat, "fields": i.fields, "lead": i.lead, "img": i.img, "url": i.url })).collect::<Vec<_>>(),
        }))
    }

    /// A link from the Wiki part, in the browser: one the bot's list showed, or any https link of a wiki page.
    pub fn open_wiki_link(&self, url: &str) -> Res<Value> {
        match Url::parse(url) {
            Ok(uri) if uri.scheme() == "https" && (wiki_link::knows(url) || wiki_net::openable(url)) => {
                // PORT: Uri.AbsoluteUri is the url crate's serialization (both escape and normalise the same common way).
                App::open_url(uri.as_str())
            }
            _ => Err(Fail::user(l::t("That link isn't in the wiki any more."))),
        }
    }

    // ---- A game's whole wiki, read from this PC (wiki_net.rs, ui/src/WikiSite.tsx) ----

    /// One read of a wiki's API for the page: {site, q: {action, …}}.
    pub async fn wiki_api(a: &Value) -> Res<Value> {
        let mut q: HashMap<String, String> = HashMap::new();
        if has(a, "q", JsonKind::Object)
            && let Some(o) = a["q"].as_object()
        {
            for (name, value) in o.iter().take(30) {
                let v = match value {
                    Value::String(s) => Some(s.clone()),
                    Value::Number(n) => Some(n.to_string()),
                    Value::Bool(true) => Some("1".to_string()),
                    _ => None,
                };
                if let Some(v) = v {
                    q.insert(name.clone(), v);
                }
            }
        }
        let site = str_of(a, "site");
        // The page takes its own copy of the answer; made here, off the core's thread, and only for the page.
        rt::blocking(move || wiki_net::api(&site, &q, &Cancel::new()).map(|v| std::sync::Arc::unwrap_or_clone(v))).await
    }

    /// Wikis for a game's name, best first.
    pub async fn wiki_find(a: &Value) -> Res<Value> {
        let name = str_of(a, "name");
        let found = rt::blocking(move || wiki_net::find(&name, &Cancel::new())).await?;
        Ok(Value::Array(
            found
                .iter()
                .map(|f| json!({ "site": f.site, "name": f.name, "pages": f.pages, "views": f.views, "img": f.img, "match": f.r#match }))
                .collect(),
        ))
    }

    /// The wiki a pasted link (or a site) is, or a message saying it isn't one.
    pub async fn wiki_about(a: &Value) -> Res<Value> {
        let link = str_of(a, "url");
        let site = wiki_net::site(Some(&link))
            .or_else(|| wiki_net::site_of_link(&link))
            .ok_or_else(|| Fail::user(l::t("Paste a link to a page of the game's wiki (Fandom or wiki.gg).")))?;
        let about = rt::blocking(move || wiki_net::about(&site, &Cancel::new()))
            .await?
            .ok_or_else(|| Fail::user(l::t("That isn't a wiki Bloxkeep can read.")))?;
        Ok(json!({ "site": about.site, "name": about.name, "articles": about.articles, "mainPage": about.main_page, "logo": about.logo }))
    }

    /// A game's wiki picked (or, with no site, forgotten): {id, name, site, place, img, added}.
    pub fn set_wiki_pick(&self, a: &Value) -> Value {
        let id = str_of(a, "id").trim().to_string();
        let id_len = utf16_len(&id);
        if id_len == 0 || id_len > 120 {
            return Value::Null;
        }
        {
            let mut p = prefs::lock();
            let games = &mut p.wikis.games;
            games.retain(|g| g.id != id);
            if let Some(site) = wiki_net::site(Some(&str_of(a, "site"))) {
                let img = opt_str(a, "img").filter(|i| {
                    Url::parse(i).is_ok_and(|iu| {
                        let host = iu.host_str().unwrap_or("");
                        iu.scheme() == "https" && (host == "static.wikia.nocookie.net" || host.ends_with(".rbxcdn.com") || host.ends_with(".wiki.gg"))
                    })
                });
                let name = str_of(a, "name").trim().to_string();
                let name_len = utf16_len(&name);
                games.push(WikiPick {
                    id: id.clone(),
                    name: if name_len > 0 && name_len <= 80 {
                        name
                    } else {
                        Url::parse(&site).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_default()
                    },
                    site,
                    place: long_of(a, "place").max(0),
                    img,
                    added: bool_of(a, "added"),
                });
                if games.len() > 200 {
                    games.remove(0);
                }
            }
            let still = p.wikis.games.iter().any(|g| g.id == id);
            p.wikis.saved.retain(|m| !(m.game == id && !still));
        }
        prefs::save();
        self.push_state();
        Value::Null
    }

    /// A page saved or unsaved ({list: "saved", on}), or seen ({list: "recent"}): {site, title, game}.
    pub fn mark_wiki_page(&self, a: &Value) -> Value {
        let site = wiki_net::site(Some(&str_of(a, "site")));
        let title = str_of(a, "title").trim().to_string();
        let game = str_of(a, "game").trim().to_string();
        let title_len = utf16_len(&title);
        let Some(site) = site else { return Value::Null };
        if title_len == 0 || title_len > 250 || utf16_len(&game) > 120 {
            return Value::Null;
        }
        let recent = str_of(a, "list") == "recent";
        {
            let mut p = prefs::lock();
            let list = if recent { &mut p.wikis.recent } else { &mut p.wikis.saved };
            list.retain(|m| !(m.site == site && m.title == title));
            if recent || bool_of(a, "on") {
                list.insert(0, WikiMark { site, title, game, at: Stamp::utc_now().unix_ms() });
                let max = if recent { 30 } else { 300 };
                list.truncate(max);
            }
        }
        prefs::save();
        self.push_state();
        Value::Null
    }

    pub fn wikis_dto() -> Value {
        let p = prefs::lock();
        let marks = |list: &Vec<WikiMark>| -> Vec<Value> { list.iter().map(|m| json!({ "site": m.site, "title": m.title, "game": m.game, "at": m.at })).collect() };
        json!({
            "games": p.wikis.games.iter().map(|g| json!({ "id": g.id, "name": g.name, "site": g.site, "place": g.place, "img": g.img, "added": g.added })).collect::<Vec<_>>(),
            "saved": marks(&p.wikis.saved),
            "recent": marks(&p.wikis.recent),
        })
    }
}
