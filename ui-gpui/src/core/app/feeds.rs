// AppController.Feeds.cs: the Game news tab: news other games' bots post (a stock, a boss spawning, a full moon), brought
// by Bloxkeep's Discord bot (feed_link.rs, services/DiscordBot/worker/feeds.js). Which games and sources there are is the
// bot's business: Bloxkeep's developers add and remove them there, and apps already out get them at once. Here: the games
// the person follows (their unread news is the tab's badge; a game not seen yet in the list marks the tab too), how far
// each was read, and when to be told (every piece, or one with a word they chose) through the usual notifications
// (Windows, the phone). Only an installed Bloxkeep connects (as the online count): not a development copy or the checks,
// unless BLOXKEEP_FEEDS_URL points them at a server of their own.
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
};

use serde_json::{Value, json};
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};
use url::Url;

use super::App;
use super::controller::{JsonKind, has, opt_str, str_of};
use super::presets::utf16_len;
use crate::core::{
    base::{Guid, Res},
    feed_link::{FeedCard, FeedGame, FeedLink},
    l,
    prefs::{self, FeedAlert},
    rt,
};

pub const MAX_FEED_WORDS: usize = 20;
pub const MAX_FEED_WORD: usize = 40;
pub const FEED_CARDS_PER_SOURCE: usize = 30;

#[derive(Default)]
pub struct FeedsState {
    pub feeds: RefCell<Option<FeedLink>>,
    pub feed_games: RefCell<Vec<FeedGame>>,
    /// Newest first.
    pub feed_cards: RefCell<Vec<FeedCard>>,
    /// The bot has told the games once since the app started.
    pub feeds_loaded: Cell<bool>,
    /// The newest card heard of (ms): after a drop, what came since is news to be told of; before the first, nothing is.
    pub feed_newest: Cell<i64>,
}

/// BLOXKEEP_FEEDS_URL when set and not empty.
pub fn feeds_test_url() -> Option<String> {
    std::env::var("BLOXKEEP_FEEDS_URL").ok().filter(|u| !u.is_empty())
}

/// The first n UTF-16 units of a string (s[..n]).
fn cut16(s: &str, n: usize) -> String {
    let units: Vec<u16> = s.encode_utf16().collect();
    if units.len() <= n { s.to_string() } else { String::from_utf16_lossy(&units[..n]) }
}

/// Lower case, without accents (đ as d): "Kitsune" finds "KITSUNE", "rong" finds "Rồng".
pub fn fold(text: &str) -> String {
    let lowered: String = text.to_lowercase().replace('đ', "d");
    // PORT: .NET drops UnicodeCategory.NonSpacingMark (Mn) only; is_combining_mark is Mn, Mc and Me. The accents NFD
    // splits off (the ones that matter here) are Mn either way.
    let plain: String = lowered.nfd().filter(|c| !is_combining_mark(*c)).collect();
    plain.trim().to_string()
}

/// [\p{L}\p{N}]: a letter or a digit.
// PORT: char::is_alphanumeric is Unicode's Alphabetic or Numeric: \p{L} and \p{N} plus a few letter-like marks and
// letter numbers; the same for every word an alert is likely to hold.
fn word_char(c: char) -> bool {
    c.is_alphanumeric()
}

/// `(?<![\p{L}\p{N}])word(?![\p{L}\p{N}])` (Rust's regex has no lookaround): the word, not inside another.
fn has_word(all: &str, word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    let mut from = 0;
    while let Some(i) = all[from..].find(word).map(|i| i + from) {
        let before = all[..i].chars().next_back();
        let after = all[i + word.len()..].chars().next();
        if !before.is_some_and(word_char) && !after.is_some_and(word_char) {
            return true;
        }
        // Next char boundary after i: a match may start inside this one.
        from = i + all[i..].chars().next().map_or(1, char::len_utf8);
    }
    false
}

/// `(?<![^\s(<\[])url(?=$|[\s)>\]])`: the link standing alone in words (start or a space or ( < [ before, end or a space
/// or ) > ] after).
fn has_link_in(s: &str, url: &str) -> bool {
    if url.is_empty() {
        return false;
    }
    let mut from = 0;
    while let Some(i) = s[from..].find(url).map(|i| i + from) {
        let before = s[..i].chars().next_back();
        let after = s[i + url.len()..].chars().next();
        let before_ok = before.is_none_or(|c| c.is_whitespace() || matches!(c, '(' | '<' | '['));
        let after_ok = after.is_none_or(|c| c.is_whitespace() || matches!(c, ')' | '>' | ']'));
        if before_ok && after_ok {
            return true;
        }
        from = i + s[i..].chars().next().map_or(1, char::len_utf8);
    }
    false
}

fn link_in(e: &Value, url: &str) -> bool {
    match e {
        Value::String(s) => s == url || has_link_in(s, url),
        Value::Array(list) => list.iter().any(|x| link_in(x, url)),
        Value::Object(o) => o.values().any(|v| link_in(v, url)),
        _ => false,
    }
}

impl App {
    pub fn feeds_allowed(&self) -> bool {
        self.updates.updater.borrow().supported() || feeds_test_url().is_some()
    }

    pub fn tick_feeds(&self) {
        if self.feeds.feeds.borrow().is_none() && !self.controller.disposed.get() && self.feeds_allowed() {
            // The link calls from its own thread: each call is handed to the core's.
            let link = FeedLink::new(
                Box::new(|games, cards| rt::post(move || App::with(|app| app.on_feeds(games, cards)))),
                Box::new(|card| rt::post(move || App::with(|app| app.on_feed_card(card)))),
                Box::new(|id| rt::post(move || App::with(|app| app.on_feed_gone(&id)))),
                Box::new(|| rt::post(|| App::with(|app| app.push_state()))),
                feeds_test_url().as_deref(),
            );
            *self.feeds.feeds.borrow_mut() = Some(link);
        }
    }

    pub fn on_feeds(self: &Rc<Self>, games: Vec<FeedGame>, cards: Vec<FeedCard>) {
        let first = !self.feeds.feeds_loaded.get();
        self.feeds.feeds_loaded.set(true);
        *self.feeds.feed_games.borrow_mut() = games;
        let before: HashSet<(String, i64)> = self.feeds.feed_cards.borrow().iter().map(|c| (c.id.clone(), c.at)).collect();
        let mut sorted = cards;
        // OrderByDescending is stable.
        sorted.sort_by(|a, b| b.at.cmp(&a.at));
        *self.feeds.feed_cards.borrow_mut() = sorted;
        // Back after a drop: what came meanwhile (newer than the newest heard of) is told as it would have been.
        if !first {
            let newest = self.feeds.feed_newest.get();
            let missed: Vec<FeedCard> =
                self.feeds.feed_cards.borrow().iter().filter(|c| c.at > newest && !before.contains(&(c.id.clone(), c.at))).cloned().collect();
            for card in missed.iter().rev() {
                self.alert_feed(card);
            }
        }
        let top = self.feeds.feed_cards.borrow().first().map_or(0, |c| c.at);
        self.feeds.feed_newest.set(self.feeds.feed_newest.get().max(top));
        let app = self.clone();
        rt::spawn(async move {
            // C#: `_ = LoadFeedGamesAsync()`: its failure goes unobserved.
            let _ = app.load_feed_games().await;
        });
        self.feeds_changed();
    }

    pub fn on_feed_card(&self, card: FeedCard) {
        {
            let mut cards = self.feeds.feed_cards.borrow_mut();
            let at = cards.iter().position(|c| c.id == card.id);
            if let Some(i) = at {
                if cards[i].at >= card.at {
                    return;
                }
                cards.remove(i);
            }
            cards.insert(0, card.clone());
            cards.sort_by(|a, b| b.at.cmp(&a.at));
            // The bot keeps 30 a source: so does the app.
            let mut count: HashMap<String, usize> = HashMap::new();
            cards.retain(|c| {
                let n = count.entry(c.src.clone()).or_insert(0);
                *n += 1;
                *n <= FEED_CARDS_PER_SOURCE
            });
        }
        self.feeds.feed_newest.set(self.feeds.feed_newest.get().max(card.at));
        self.alert_feed(&card);
        self.feeds_changed();
    }

    pub fn on_feed_gone(&self, id: &str) {
        let removed = {
            let mut cards = self.feeds.feed_cards.borrow_mut();
            let before = cards.len();
            cards.retain(|c| c.id != id);
            cards.len() != before
        };
        if removed {
            self.feeds_changed();
        }
    }

    /// The page's list of news changed (it asks for it with feedCards and hears of it here); the badge too.
    pub fn feeds_changed(&self) {
        self.emit("feedCards", self.feed_cards_dto());
        self.push_state();
    }

    /// The games' names and pictures from Roblox, for those the bot gave a place.
    pub async fn load_feed_games(self: &Rc<Self>) -> Res<()> {
        let places: Vec<(i64, i64)> = self.feeds.feed_games.borrow().iter().filter_map(|g| g.place.filter(|p| *p > 0)).map(|p| (p, 0)).collect();
        if places.is_empty() {
            return Ok(());
        }
        self.load_games(places, false).await?;
        self.push_state();
        Ok(())
    }

    /// A card the person asked to be told of: their game, followed, and every piece or one with their word.
    pub fn alert_feed(&self, card: &FeedCard) {
        let alert: FeedAlert = {
            let p = prefs::lock();
            let feeds = &p.game_feeds;
            match feeds.alerts.get(&card.game) {
                Some(a) if feeds.follow.contains(&card.game) && (a.mode == "all" || a.mode == "words") => a.clone(),
                _ => return,
            }
        };
        let mut word: Option<String> = None;
        if alert.mode == "words" {
            let mut parts: Vec<String> = vec![card.title.clone(), card.text.clone()];
            // PORT: a field the bot sent as null failed the C# here (f.Name of null); it is left out.
            parts.extend(card.fields.iter().flatten().map(|f| format!("{} {}", f.name, f.value)));
            let all = fold(&parts.join("\n"));
            word = alert.words.iter().find(|w| {
                let f = fold(w);
                !f.is_empty() && has_word(&all, &f)
            }).cloned();
            if word.is_none() {
                return;
            }
        }
        let game = self.feeds.feed_games.borrow().iter().find(|g| g.id == card.game).map(|g| g.name.clone()).unwrap_or_else(|| card.game.clone());
        let title = if !card.title.is_empty() { card.title.clone() } else { card.text.split('\n').next().unwrap_or("").to_string() };
        let text = match &word {
            None => format!("{game} · {title}"),
            Some(w) => format!("{game} · {title} · {w}"),
        };
        let text = if utf16_len(&text) > 200 { cut16(&text, 199) + "…" } else { text };
        self.notify(&text, "info", "feed", Guid::nil(), true, Some(&format!("feed:{}:{}", card.id, card.at)));
    }

    pub fn feed_unread(&self, game: &str) -> i32 {
        let read = {
            let p = prefs::lock();
            if !p.game_feeds.follow.iter().any(|f| f == game) {
                return 0;
            }
            p.game_feeds.read.get(game).copied().unwrap_or(0)
        };
        self.feeds.feed_cards.borrow().iter().filter(|c| c.game == game && c.at > read).count() as i32
    }

    pub fn feeds_dto(&self) -> Value {
        let (follow, read, seen, alerts) = {
            let p = prefs::lock();
            let f = &p.game_feeds;
            (f.follow.clone(), f.read.clone(), f.seen.clone(), f.alerts.clone())
        };
        let feed_games = self.feeds.feed_games.borrow().clone();
        let mut unread_sum = 0;
        let mut fresh = 0;
        let games: Vec<Value> = feed_games
            .iter()
            .map(|g| {
                let icon = g.place.and_then(|place| self.controller.games.borrow().get(&place).cloned().flatten()).and_then(|i| i.icon_url);
                let alert = alerts.get(&g.id);
                let unread = self.feed_unread(&g.id);
                let is_new = !seen.contains(&g.id);
                unread_sum += unread;
                if is_new {
                    fresh += 1;
                }
                json!({
                    "id": g.id, "name": g.name, "place": g.place, "iconUrl": icon,
                    // PORT: a source the bot sent as null failed the C# here (s.Id of null); it is left out.
                    "sources": g.sources.iter().flatten().map(|s| json!({ "id": s.id, "title": s.title })).collect::<Vec<_>>(),
                    "followed": follow.contains(&g.id), "unread": unread, "read": read.get(&g.id).copied().unwrap_or(0), "isNew": is_new,
                    "alert": { "mode": alert.map_or("off", |a| a.mode.as_str()), "words": alert.map(|a| a.words.clone()).unwrap_or_default() },
                })
            })
            .collect();
        json!({
            "on": self.feeds_allowed(),
            "connected": self.feeds.feeds.borrow().as_ref().is_some_and(|f| f.connected()),
            "loaded": self.feeds.feeds_loaded.get(),
            "games": games,
            "unread": unread_sum,
            "fresh": fresh,
        })
    }

    pub fn feed_cards_dto(&self) -> Value {
        Value::Array(
            self.feeds
                .feed_cards
                .borrow()
                .iter()
                .map(|c| {
                    json!({
                        "id": c.id, "src": c.src, "game": c.game, "at": c.at, "title": c.title, "text": c.text,
                        // PORT: a null field failed the C# here; it is left out.
                        "fields": c.fields.iter().flatten().map(|f| json!({ "name": f.name, "value": f.value })).collect::<Vec<_>>(),
                        "picture": App::discord_picture(c.picture.as_deref()), "image": App::discord_picture(c.image.as_deref()),
                        "author": c.author.as_ref().filter(|by| !by.name.is_empty()).map(|by| json!({ "name": by.name, "icon": App::discord_picture(by.icon.as_deref()), "url": by.url })),
                        "link": c.link, "color": c.color, "until": c.until,
                        "blocks": c.blocks, "spoiler": c.spoiler,
                    })
                })
                .collect(),
        )
    }

    /// A link in a piece of news (a button, a masked link, its title): opened in the browser, if it is one of the news's
    /// own and https. The page can open nothing else this way.
    pub fn open_feed_link(&self, url: &str) -> Res<Value> {
        let known = self.feeds.feed_cards.borrow().iter().any(|c| App::feed_has_link(c, url));
        match Url::parse(url) {
            // PORT: Uri.AbsoluteUri is the url crate's serialization.
            Ok(uri) if uri.scheme() == "https" && known => App::open_url(uri.as_str()),
            _ => Err(crate::core::base::Fail::user(l::t("That link isn't in the news any more."))),
        }
    }

    pub fn feed_has_link(c: &FeedCard, url: &str) -> bool {
        if c.link.as_deref() == Some(url) || c.author.as_ref().and_then(|a| a.url.as_deref()) == Some(url) {
            return true;
        }
        // In a block: a button's link, or in its words ([words](link), or the link itself).
        c.blocks.as_ref().is_some_and(|b| link_in(b, url))
    }

    /// A picture from Discord's own servers only (the page's rules allow no other): nothing loads from wherever a game's bot points.
    pub fn discord_picture(url: Option<&str>) -> Option<String> {
        let text = url?;
        let uri = Url::parse(text).ok()?;
        let host = uri.host_str().unwrap_or("");
        let ok = uri.scheme() == "https"
            && uri.port().is_none()
            && uri.username().is_empty()
            && uri.password().is_none()
            && (host == "media.discordapp.net" || host == "cdn.discordapp.com" || host.ends_with(".discordapp.net"));
        if ok { Some(text.to_string()) } else { None }
    }

    /// Followed from now: what is there already counts as read (the badge is for news that comes after).
    pub fn set_feed_follow(&self, game: &str, on: bool) -> Value {
        let listed = self.feeds.feed_games.borrow().iter().any(|g| g.id == game);
        let newest = self.feeds.feed_cards.borrow().iter().filter(|c| c.game == game).map(|c| c.at).max().unwrap_or(0);
        {
            let mut p = prefs::lock();
            let feeds = &mut p.game_feeds;
            if on && listed && !feeds.follow.iter().any(|f| f == game) {
                feeds.follow.push(game.to_string());
                feeds.read.insert(game.to_string(), newest);
            } else if !on {
                if let Some(i) = feeds.follow.iter().position(|f| f == game) {
                    feeds.follow.remove(i);
                }
            }
        }
        prefs::save();
        self.push_state();
        Value::Null
    }

    /// The game's news shown: read up to its newest.
    pub fn mark_feed_read(&self, game: &str) -> Value {
        let newest = self.feeds.feed_cards.borrow().iter().filter(|c| c.game == game).map(|c| c.at).max().unwrap_or(0);
        {
            let mut p = prefs::lock();
            if newest <= p.game_feeds.read.get(game).copied().unwrap_or(0) {
                return Value::Null;
            }
            p.game_feeds.read.insert(game.to_string(), newest);
        }
        prefs::save();
        self.push_state();
        Value::Null
    }

    /// The tab opened: the games in it are no longer new. Games the bot no longer lists are forgotten.
    pub fn mark_feeds_seen(&self) -> Value {
        let ids: Vec<String> = self.feeds.feed_games.borrow().iter().map(|g| g.id.clone()).collect();
        {
            let mut p = prefs::lock();
            let feeds = &mut p.game_feeds;
            if ids.iter().all(|id| feeds.seen.contains(id)) {
                return Value::Null;
            }
            let mut seen: Vec<String> = Vec::new();
            for id in ids.iter().chain(feeds.seen.iter()) {
                if !seen.contains(id) {
                    seen.push(id.clone());
                }
            }
            seen.truncate(200);
            feeds.seen = seen;
        }
        prefs::save();
        self.push_state();
        Value::Null
    }

    /// game, mode ("off", "all", "words") and/or words (a list, or text split at commas and new lines).
    pub fn set_feed_alert(&self, a: &Value) -> Res<Value> {
        let game = str_of(a, "game");
        if !self.feeds.feed_games.borrow().iter().any(|g| g.id == game) {
            return Err(crate::core::base::Fail::user(l::t("That game isn't in the news any more.")));
        }
        let mut alert = prefs::lock().game_feeds.alerts.get(&game).cloned().unwrap_or_default();
        if let Some(mode) = opt_str(a, "mode").filter(|m| matches!(m.as_str(), "off" | "all" | "words")) {
            alert.mode = mode;
        }
        if has(a, "words", JsonKind::Array) {
            alert.words = a["words"].as_array().into_iter().flatten().filter_map(|w| w.as_str().map(str::to_string)).collect();
        } else if let Some(text) = opt_str(a, "words") {
            alert.words = text.split([',', '\n', ';']).map(str::to_string).collect();
        }
        // PORT: Distinct(StringComparer.CurrentCultureIgnoreCase) compares the words lower-cased.
        let mut kept: Vec<String> = Vec::new();
        let mut keys: HashSet<String> = HashSet::new();
        for w in alert.words.iter().map(|w| w.trim()).filter(|w| !w.is_empty()) {
            let w = if utf16_len(w) > MAX_FEED_WORD { cut16(w, MAX_FEED_WORD) } else { w.to_string() };
            if keys.insert(w.to_lowercase()) {
                kept.push(w);
            }
            if kept.len() >= MAX_FEED_WORDS {
                break;
            }
        }
        alert.words = kept;
        let mode = alert.mode.clone();
        let followed = {
            let mut p = prefs::lock();
            p.game_feeds.alerts.insert(game.clone(), alert);
            p.game_feeds.follow.iter().any(|f| *f == game)
        };
        // Asked to be told of a game's news: it is followed (the badge too).
        if mode != "off" && !followed {
            self.set_feed_follow(&game, true);
        }
        prefs::save();
        self.push_state();
        Ok(Value::Null)
    }
}
