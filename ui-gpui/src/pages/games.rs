// Games: the games Bloxkeep's bot names (each with its wiki, some with news or a price list) as cards in their own
// pictures: yours first (followed, news unread, played here), then the popular ones. Opened, a game has its page: its
// picture with Follow, Wiki and its roblox.com page on top, Roblox's figures under it, then Overview (your accounts in it
// over 7 days, the hours it is quiet, what it is: the old Game info), its News and its Prices. Since 2026-11-09 the
// old Game info page lives here (Home opens a game on it). The rules are GamesView.tsx's: a game shows only with the
// wiki the bot names; the commands are the app's (wikiGames, feedCards, playedGames, gameLooks, gameDetails, wikiGame,
// feedFollow, feedRead, feedSeen, wikiOpen, openGameWiki, openGamePage, feedOpen, launch).
use std::collections::{HashMap, HashSet};

use gpui_kit::component::carousel::{Carousel, CarouselContent, CarouselItem, CarouselState};
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use crate::{
    images,
    kit::*,
    pages::{Page, home::clean},
    roblox,
    shell::Root,
    theme::*,
};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Overview,
    News,
    Prices,
    /// The game's passes (pages/passes.rs).
    Passes,
    /// Its private servers the accounts can join (pages/servers.rs).
    Servers,
    /// Its wiki, read here (pages/wiki.rs).
    Wiki,
}

/// The game open on the page: a card's id (the bot's), or "place:N" for one opened from Home that the bot doesn't name.
#[derive(Clone)]
pub struct Open {
    pub id: String,
    pub place: i64,
    pub name: String,
}

/// What Games shows: the games, the friends (pages/friends.rs), the avatar (pages/avatar.rs), the profile with its
/// Robux and communities, the Marketplace (both pages/profile.rs), or the inbox: messages, notifications and the Robux
/// lines (pages/inbox.rs). One place for what the Roblox app has, so the rail
/// keeps its few buttons (the owner's call, 2026-11-10); what comes next of Roblox's joins this switch.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Games,
    Friends,
    Avatar,
    Profile,
    Shop,
    Inbox,
    /// The account's own settings (pages/rsettings.rs).
    Settings,
}

/// The header's switch between them.
pub fn views(root: &Root, cx: &mut Context<Root>) -> impl IntoElement {
    let at = match root.games.view {
        View::Games => "g",
        View::Friends => "f",
        View::Avatar => "a",
        View::Profile => "p",
        View::Shop => "s",
        View::Inbox => "i",
        View::Settings => "c",
    };
    Segmented::new("rbx-view", at)
        .item("g", root.tr("news.title"))
        .item("f", root.tr("nav.friends"))
        .item("a", root.tr("nav.avatar"))
        .item("p", root.tr("nav.profile"))
        .item("s", root.tr("nav.shop"))
        .item("i", root.tr("nav.inbox"))
        .item("c", root.tr("nav.settings"))
        .on_select({
            let this = cx.entity().downgrade();
            move |v, _, cx| {
                let to = match v.as_ref() {
                    "f" => View::Friends,
                    "a" => View::Avatar,
                    "p" => View::Profile,
                    "s" => View::Shop,
                    "i" => View::Inbox,
                    "c" => View::Settings,
                    _ => View::Games,
                };
                let _ = this.update(cx, |r, cx| {
                    r.games.view = to;
                    r.friends.menu = false;
                    cx.notify();
                });
            }
        })
}

/// What the page asked the app for, and what it is showing.
#[derive(Default)]
pub struct Data {
    pub view: View,
    pub wiki_games: Option<Vec<Value>>,
    pub wiki_failed: bool,
    pub cards: Option<Vec<Value>>,
    pub details: HashMap<i64, Value>,
    pub prices: HashMap<String, Value>,
    asked_wiki: bool,
    asked_cards: bool,
    asked_details: HashSet<i64>,
    asked_prices: HashSet<String>,
    seen_sent: bool,
    read_sent: Option<String>,
    pub open: Option<Open>,
    pub tab: Tab,
    pub about: bool,
    /// The game page's scroll (a shot scrolls it down with BLOXKEEP_SHOT_SCROLL).
    pub page_scroll: ScrollHandle,
    pub kind: Option<String>,
    /// A game's news: only this source's posts (none: every source).
    pub source: Option<String>,
    /// The news read before the game was shown this time: what came after keeps its New mark while it is open.
    pub mark: Option<(String, f64)>,
    /// News alerts' words box: the game it holds the words of, the words the app has saved, and (set while drawing)
    /// what it should hold next, which the frame puts in before it draws again.
    pub words_for: Option<String>,
    pub words_saved: String,
    pub words_want: std::cell::RefCell<Option<(String, String)>>,
    /// Roblox's charts and search, asked from here (roblox.rs): the sign-in the app lent for them (None: not asked yet),
    /// the rows, the row shown, each row's games, the icons, and a search with its answer (None while it runs).
    cookie: Option<Option<String>>,
    asked_cookie: bool,
    pub sorts: Option<Result<Vec<roblox::Sort>, String>>,
    asked_sorts: bool,
    pub sort_at: Option<String>,
    pub rows: HashMap<String, Result<Vec<roblox::Game>, String>>,
    asked_rows: HashSet<String>,
    pub icons: HashMap<i64, String>,
    asked_icons: HashSet<i64>,
    /// Their wide pictures, for the board's tiles; and a sharper one for the game that leads a chart.
    pub covers: HashMap<i64, String>,
    pub covers_big: HashMap<i64, String>,
    asked_big: HashSet<i64>,
    /// A game's page: the play history as the app handed it last (raw stays, history.rs), and each game's accounts
    /// summed from it here (by place), which the app's gameDetails summed until 2026-11-11.
    history: Option<Value>,
    /// A game's own pictures (by place) and the carousel that turns through them (GPUI Component's), once Roblox
    /// has given two or more.
    pub shots: HashMap<i64, (Vec<String>, Entity<CarouselState>)>,
    asked_shots: HashSet<i64>,
    pub stays: HashMap<i64, Vec<Value>>,
    pub found: Option<(String, Option<Result<Vec<roblox::Game>, String>>)>,
    /// Play on a game's page: the game (its Open id) and the accounts ticked to join it.
    pub picking: Option<(String, HashSet<String>)>,
    /// The server Play's row joins (its Job ID), when it was opened from the Servers tab; none: any server.
    pub pick_job: Option<String>,
    /// The charts' rows that don't fit their line are shown (the More menu is open).
    pub more_sorts: bool,
}

/// A game card's ground reaches this far past its picture (it eases in under the mouse); cards stand so close that
/// their pictures are `GRID_GAP` apart, on the same columns as every row of tiles on Games home ([`home_fit`]).
const CPAD: f32 = 6.;
const CGAP: f32 = GRID_GAP - 2. * CPAD;

/// Every row of pictures on Games home (keep playing, the charts, your games, with a wiki) on one grid: its columns
/// and a picture's width, so the rows line up down the page and each ends where the room does.
fn home_fit(width: f32) -> (usize, f32) {
    grid_fit(width, TILE, GRID_GAP)
}

/// A game on the page, gathered from every side: the bot's news and lists, the picks, this PC's play, the community.
pub struct Game {
    pub id: String,
    pub name: String,
    pub place: i64,
    pub news: Option<Value>,
    pub prices: bool,
    pub wiki: Option<String>,
    pub played: Option<f64>,
    pub playing: i64,
    pub added: bool,
    pub icon: Option<String>,
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or_default()
}

fn int(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(Value::as_i64).unwrap_or(0)
}

/// A game's name to match the same game from two sides ("[🎃] Blox Fruits", "🔥 UPD 24 Blox Fruits", "Blox Fruits").
fn same_name(name: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for ch in name.chars() {
        match ch {
            '[' | '(' | '【' => depth += 1,
            ']' | ')' | '】' => depth = (depth - 1).max(0),
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    let words: Vec<String> = out
        .split_whitespace()
        .map(|w| w.to_lowercase())
        .filter(|w| !matches!(w.as_str(), "upd" | "update" | "new" | "event") && !(w.starts_with('x') && w[1..].chars().all(|c| c.is_ascii_digit()) && w.len() > 1))
        .collect();
    let joined: String = words.join("").chars().filter(|c| c.is_alphanumeric()).collect();
    // "UPD 24 Blox Fruits": the number after UPD goes too.
    let trimmed = joined.trim_start_matches(|c: char| c.is_ascii_digit());
    if trimmed.is_empty() { joined } else { trimmed.to_string() }
}

fn same(a_place: i64, a_name: &str, b_place: i64, b_name: &str) -> bool {
    let n = same_name(a_name);
    (a_place > 0 && a_place == b_place) || (!n.is_empty() && n == same_name(b_name))
}

fn mine(g: &Game) -> bool {
    g.news.is_some() || g.played.is_some() || g.added
}

fn followed(g: &Game) -> bool {
    g.news.as_ref().and_then(|n| n.get("followed")).and_then(Value::as_bool).unwrap_or(false)
}

fn unread(g: &Game) -> i64 {
    g.news.as_ref().map(|n| int(n, "unread")).unwrap_or(0)
}

/// One card per game, as GamesView: followed first, then news unread, played here, added here, played by others.
pub fn list(root: &Root) -> Vec<Game> {
    let mut by: Vec<Game> = Vec::new();
    let picks: Vec<Value> = root.state.pointer("/wikis/games").and_then(Value::as_array).cloned().unwrap_or_default();
    for n in root.state.pointer("/feeds/games").and_then(Value::as_array).into_iter().flatten() {
        by.push(Game {
            id: s(n, "id").into(),
            name: s(n, "name").into(),
            place: int(n, "place"),
            news: Some(n.clone()),
            prices: false,
            wiki: None,
            played: None,
            playing: 0,
            added: false,
            icon: n.get("iconUrl").and_then(Value::as_str).map(str::to_string),
        });
    }
    for w in root.games.wiki_games.iter().flatten() {
        let id = s(w, "id");
        let site = Some(s(w, "wiki")).filter(|u| !u.is_empty()).map(str::to_string).or_else(|| {
            picks.iter().find(|p| s(p, "id") == id).map(|p| s(p, "site").to_string()).filter(|u| !u.is_empty())
        });
        let prices = s(w, "kind") == "prices";
        match by.iter_mut().find(|g| g.id == id) {
            Some(g) => {
                g.wiki = site;
                g.prices |= prices;
                if g.place <= 0 {
                    g.place = int(w, "place");
                }
            }
            None => by.push(Game {
                id: id.into(),
                name: s(w, "name").into(),
                place: int(w, "place"),
                news: None,
                prices,
                wiki: site,
                played: None,
                playing: 0,
                added: false,
                icon: None,
            }),
        }
    }
    // Games added here before (their wiki picked then) stay.
    for p in &picks {
        if p.get("added").and_then(Value::as_bool) == Some(true) && !s(p, "site").is_empty() && !by.iter().any(|g| g.id == s(p, "id")) {
            by.push(Game {
                id: s(p, "id").into(),
                name: s(p, "name").into(),
                place: int(p, "place"),
                news: None,
                prices: false,
                wiki: Some(s(p, "site").into()),
                played: None,
                playing: 0,
                added: true,
                icon: p.get("img").and_then(Value::as_str).map(str::to_string),
            });
        }
    }
    for p in root.played.iter().flatten() {
        let hours = p.get("hours").and_then(Value::as_f64).unwrap_or(0.);
        if let Some(g) = by.iter_mut().find(|g| same(int(p, "place"), s(p, "name"), g.place, &g.name)) {
            g.played = Some(g.played.unwrap_or(0.) + hours);
        }
    }
    let community: Vec<Value> = root.state.pointer("/community/data/games").and_then(Value::as_array).cloned().unwrap_or_default();
    for g in by.iter_mut() {
        g.playing = community.iter().filter(|c| same(int(c, "place"), s(c, "name"), g.place, &g.name)).map(|c| int(c, "pcs")).max().unwrap_or(0);
    }
    by.retain(|g| g.wiki.is_some());
    by.sort_by(|a, b| {
        mine(b)
            .cmp(&mine(a))
            .then(followed(b).cmp(&followed(a)))
            .then(unread(b).cmp(&unread(a)))
            .then(b.played.unwrap_or(0.).total_cmp(&a.played.unwrap_or(0.)))
            .then(a.added.cmp(&b.added))
            .then(b.playing.cmp(&a.playing))
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    by
}

fn look(root: &Root, place: i64, part: &str) -> Option<String> {
    root.looks.get(&place).and_then(|l| l.get(part)).and_then(Value::as_str).filter(|u| !u.is_empty()).map(str::to_string)
}

fn icon_of(root: &Root, g: &Game) -> Option<String> {
    look(root, g.place, "icon").or_else(|| g.icon.clone().filter(|u| !u.is_empty()))
}

/// The start place of the game whose page is shown, when it has one.
pub fn shown_place(r: &Root) -> Option<i64> {
    let open = r.games.open.as_ref()?;
    let place = list(r).into_iter().find(|g| g.id == open.id).map(|g| g.place).filter(|p| *p > 0).unwrap_or(open.place);
    (place > 0).then_some(place)
}

/// Opens a game's page from elsewhere (Home): its card when the bot names it, else its page with Overview only.
pub fn open_place(r: &mut Root, place: i64, name: &str) {
    let open = list(r)
        .into_iter()
        .find(|g| same(place, name, g.place, &g.name))
        .map(|g| Open { id: g.id, place: if g.place > 0 { g.place } else { place }, name: g.name })
        .unwrap_or_else(|| Open { id: format!("place:{place}"), place, name: clean(name) });
    r.games.open = Some(open);
    r.games.tab = Tab::Overview;
    r.games.about = false;
    r.go(Page::Games);
}

/// What the page needs from the app, asked once each: the bot's games, the news, the games' pictures, and for the
/// open game its figures and its price list. Read and seen marks go back as the page shows them.
pub fn want(r: &mut Root, cx: &mut Context<Root>) {
    let Some(link) = r.link.clone() else { return };
    if !r.games.asked_wiki {
        r.games.asked_wiki = true;
        let rx = link.call("wikiGames", json!({}));
        cx.spawn(async move |this, cx| {
            let got = rx.await.ok();
            let _ = this.update(cx, |r, cx| {
                match got {
                    Some(Ok(v)) => r.games.wiki_games = Some(v.get("games").and_then(Value::as_array).cloned().unwrap_or_default()),
                    _ => {
                        r.games.wiki_failed = true;
                        r.games.wiki_games = Some(Vec::new());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    if !r.games.asked_cards {
        r.games.asked_cards = true;
        let rx = link.call("feedCards", json!({}));
        cx.spawn(async move |this, cx| {
            let got = rx.await.ok().and_then(Result::ok);
            let _ = this.update(cx, |r, cx| {
                r.games.cards = Some(got.and_then(|v| v.as_array().cloned()).unwrap_or_default());
                cx.notify();
            });
        })
        .detach();
    }
    want_roblox(r, &link, cx);
    let fresh = r.state.pointer("/feeds/fresh").and_then(Value::as_i64).unwrap_or(0);
    if fresh > 0 && !r.games.seen_sent {
        r.games.seen_sent = true;
        link.send("feedSeen", json!({}));
    } else if fresh == 0 {
        r.games.seen_sent = false;
    }
    let all = list(r);
    let mut places: Vec<i64> = all.iter().map(|g| g.place).collect();
    if let Some(o) = &r.games.open {
        places.push(o.place);
    }
    r.want_looks(places, cx);

    let Some(mut open) = r.games.open.clone() else { return };
    // Opened by its place before the bot's games had come (open_place found none then): once they have, the page is
    // that game's, with its News, Values and Wiki tabs and its Follow button.
    if open.id.starts_with("place:")
        && open.place > 0
        && let Some(g) = all.iter().find(|g| g.place == open.place)
    {
        open = Open { id: g.id.clone(), place: open.place, name: g.name.clone() };
        r.games.open = Some(open.clone());
    }
    let game = all.iter().find(|g| g.id == open.id);
    let place = game.map(|g| g.place).filter(|p| *p > 0).unwrap_or(open.place);
    if r.games.tab == Tab::Wiki
        && let Some(g) = game
        && let Some(site) = g.wiki.clone()
    {
        let name = g.name.to_string();
        crate::pages::wiki::want(r, &site, &name, cx);
    }
    if place > 0 && r.games.asked_details.insert(place) {
        let rx = link.call("gameDetails", json!({ "placeId": place }));
        cx.spawn(async move |this, cx| {
            let got = rx.await.ok().and_then(Result::ok);
            let _ = this.update(cx, |r, cx| {
                if let Some(v) = got.filter(|v| !v.is_null()) {
                    let universe = int(&v, "universe");
                    r.games.details.insert(place, v);
                    settle_stays(r, place);
                    want_shots(r, place, universe, cx);
                }
                cx.notify();
            });
        })
        .detach();
        // The stays as they are now, for the page's accounts (the list read for Home may be minutes old).
        let rx = link.call("playSessions", json!({}));
        cx.spawn(async move |this, cx| {
            let got = rx.await.ok().and_then(Result::ok);
            let _ = this.update(cx, |r, cx| {
                if let Some(raw) = got.filter(|v| v.is_object()) {
                    r.games.history = Some(raw);
                    settle_stays(r, place);
                    cx.notify();
                }
            });
        })
        .detach();
    }
    if game.is_some_and(|g| g.prices) && r.games.asked_prices.insert(open.id.clone()) {
        let rx = link.call("wikiGame", json!({ "id": open.id }));
        let id = open.id.clone();
        cx.spawn(async move |this, cx| {
            let got = rx.await.ok().and_then(Result::ok).unwrap_or(Value::Null);
            let _ = this.update(cx, |r, cx| {
                r.games.prices.insert(id, got);
                cx.notify();
            });
        })
        .detach();
    }
    // The news: read once shown (what came after the last read keeps its New mark while the game stays open).
    if let Some(n) = game.and_then(|g| g.news.as_ref()) {
        let read = n.get("read").and_then(Value::as_f64).unwrap_or(0.);
        let follow = n.get("followed").and_then(Value::as_bool).unwrap_or(false);
        if r.games.tab == Tab::News && r.games.cards.is_some() {
            if r.games.mark.as_ref().is_none_or(|(id, _)| *id != open.id) {
                r.games.mark = Some((open.id.clone(), if follow { read } else { f64::INFINITY }));
            }
            if follow && int(n, "unread") > 0 && r.games.read_sent.as_deref() != Some(open.id.as_str()) {
                r.games.read_sent = Some(open.id.clone());
                link.send("feedRead", json!({ "game": open.id }));
            }
        }
        if int(n, "unread") == 0 && r.games.read_sent.as_deref() == Some(open.id.as_str()) {
            r.games.read_sent = None;
        }
    }
}

/// The words box takes the words of the game now shown (news' alerts asked for it while drawing).
pub fn settle_words(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let Some((game, words)) = r.games.words_want.borrow_mut().take() else { return };
    r.games.words_for = Some(game);
    r.games.words_saved = words.clone();
    r.news_words.update(cx, |s, cx| s.set_value(words, window, cx));
}

/// "Notify me": off, every post, or only posts with some words (kept as the box is left), as NewsView.tsx's Alerts.
fn alerts(root: &Root, feed: &Value, cx: &mut Context<Root>) -> Div {
    let id = s(feed, "id").to_string();
    let mode = feed.pointer("/alert/mode").and_then(Value::as_str).unwrap_or("off").to_string();
    let words = feed.pointer("/alert/words").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")).unwrap_or_default();
    if mode == "words" && root.games.words_for.as_deref() != Some(id.as_str()) && root.games.words_want.borrow().is_none() {
        *root.games.words_want.borrow_mut() = Some((id.clone(), words));
        let this = cx.entity();
        cx.defer(move |cx| this.update(cx, |_, cx| cx.notify()));
    }
    let mut seg = div().flex().gap(px(2.)).p(px(3.)).rounded(px(R_M)).bg(c(RAISED));
    for m in ["off", "all", "words"] {
        let game = id.clone();
        seg = seg.child(
            segment(SharedString::from(format!("news-tell-{m}")), mode == m)
                .h(px(26.))
                .px(px(10.))
                .text_size(px(T_S + 0.5))
                .cursor_pointer()
                .child(root.tr(&format!("news.tell.{m}")))
                .on_click(cx.listener(move |r, _, _, cx| {
                    r.send("feedAlert", json!({ "game": game, "mode": m }));
                    cx.notify();
                })),
        );
    }
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(12.))
        .child(div().flex().items_center().gap(px(7.)).text_size(px(T_M)).text_color(c(TEXT_2)).child(icon("bell", 14., c(TEXT_2))).child(root.tr("news.tellMe")))
        .child(seg)
        .when(mode == "words", |d| d.child(field(&root.news_words, None).w(px(300.))))
}

/// A game's own pictures, asked once its details have named its universe.
fn want_shots(r: &mut Root, place: i64, universe: i64, cx: &mut Context<Root>) {
    if universe <= 0 || !r.games.asked_shots.insert(place) {
        return;
    }
    let rx = roblox::run(move || roblox::shots(universe));
    cx.spawn(async move |this, cx| {
        let Ok(got) = rx.await else { return };
        let _ = this.update(cx, |r, cx| {
            if got.len() >= 2 {
                let state = cx.new(|_| CarouselState::new(got.len()));
                r.games.shots.insert(place, (got, state));
                cx.notify();
            }
        });
    })
    .detach();
}

/// The game's own pictures in a row you drag, wheel or step through (GPUI Component's carousel: it snaps to a
/// picture on a spring); three across on a wide page, two, then one. The arrows are the kit's.
fn gallery(root: &Root, place: i64, width: f32, cx: &mut Context<Root>) -> Option<AnyElement> {
    let (pics, state) = root.games.shots.get(&place)?;
    let per = if width >= 1300. { 3. } else if width >= 620. { 2. } else { 1. };
    let w = ((width - GAP * (per - 1.)) / per).floor();
    let h = (w * 9. / 16.).round();
    let items: Vec<CarouselItem> = pics.iter().enumerate().map(|(i, url)| {
        let pic = images::pic_pair(url, w, h, cx);
        CarouselItem::new(("gp-shot", i), i, state).w(px(w + GAP)).pl(px(0.)).pr(px(GAP)).child(
            div().w(px(w)).h(px(h)).rounded(px(R_L)).overflow_hidden().bg(ca(WHITE, 0.045)).when_some(pic, |d, p| {
                d.child(fade_in(img(p.0).size_full().rounded(px(R_L)).object_fit(ObjectFit::Cover), &p.1))
            }),
        )
    }).collect();
    let (back, on) = { let s = state.read(cx); (s.has_previous(), s.has_next()) };
    let step = |next: bool| {
        let state = state.clone();
        move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
            state.update(cx, |s, cx| {
                if next { s.select_next(cx) } else { s.select_previous(cx) };
            })
        }
    };
    let arrows = pics.len() as f32 > per;
    Some(
        div()
            .relative()
            .w(px(width))
            .h(px(h))
            // (The next picture is cut at the column's edge, not drawn under what stands beside it.)
            .overflow_hidden()
            .child(Carousel::new(("gp-shots", place as usize), state).w(px(width)).h(px(h)).child(CarouselContent::new(state).h(px(h)).children(items)))
            .when(arrows, |d| {
                d.child(div().absolute().top(px(h / 2. - 17.)).left(px(10.)).child(IconButton::new("gp-shot-back", "chevronLeft").size(34.).glass().disabled(!back).on_click(step(false))))
                    .child(div().absolute().top(px(h / 2. - 17.)).right(px(10.)).child(IconButton::new("gp-shot-on", "chevronRight").size(34.).glass().disabled(!on).on_click(step(true))))
            })
            .into_any_element(),
    )
}

/// A game's accounts over 7 days, summed here from the raw stays once both they and the game's details (which name
/// its universe) are in: history::game_accounts, rows shaped as the app's gameDetails gave them. An account still in
/// the app goes by its name there; one deleted, by the name its last stay kept.
fn settle_stays(r: &mut Root, place: i64) {
    let (Some(raw), Some(details)) = (&r.games.history, r.games.details.get(&place)) else { return };
    let (sessions, known, now) = crate::history::read(raw);
    let universe = match int(details, "universe") {
        0 => crate::history::universe_of(&sessions, &known, place),
        u => u,
    };
    let bans = crate::history::bans(raw);
    let rows: Vec<Value> = crate::history::game_accounts(&sessions, &known, place, universe, now)
        .into_iter()
        .map(|stay| {
            let account = r.accounts().iter().find(|a| s(a, "id") == stay.account);
            let name = account.map(crate::pages::accounts::label).filter(|n| !n.is_empty()).or(stay.label.clone()).unwrap_or_else(|| "–".into());
            let banned = account.is_some() && bans.iter().any(|b| b.account == stay.account && crate::history::same_game(&known, b.place, place));
            json!({
                "id": stay.account, "name": name, "minutes": (stay.seconds / 60.).round_ties_even() as i64, "kicks": stay.kicks,
                "kickWhy": if stay.idle { Value::from("AFK") } else { Value::Null }, "drops": stay.drops,
                "now": if stay.now { "ingame" } else if banned { "banned" } else { "" },
                "lastMs": if stay.now { Value::Null } else { Value::from(stay.last) },
            })
        })
        .collect();
    r.games.stays.insert(place, rows);
}

impl Data {
    /// The vault or its accounts changed: the sign-in lent for the charts may be another vault's (or none was lent
    /// while there was no account), so it is asked again, with the rows it got.
    pub fn forget_sign_in(&mut self) {
        if !self.asked_cookie {
            return;
        }
        self.cookie = None;
        self.asked_cookie = false;
        self.sorts = None;
        self.asked_sorts = false;
        self.rows.clear();
        self.asked_rows.clear();
    }
}

/// Roblox's charts: a sign-in from the app first, then the rows, then the row shown, then the icons of what's shown.
fn want_roblox(r: &mut Root, link: &crate::link::Link, cx: &mut Context<Root>) {
    if !r.games.asked_cookie {
        r.games.asked_cookie = true;
        let rx = link.call("robloxSignIn", json!({}));
        cx.spawn(async move |this, cx| {
            let got = rx.await.ok().and_then(Result::ok);
            let _ = this.update(cx, |r, cx| {
                r.games.cookie = Some(got.and_then(|v| v.get("cookie").and_then(Value::as_str).map(str::to_string)));
                cx.notify();
            });
        })
        .detach();
    }
    let Some(cookie) = r.games.cookie.clone() else { return };
    if !r.games.asked_sorts {
        r.games.asked_sorts = true;
        let c = cookie.clone();
        let rx = roblox::run(move || roblox::sorts(c.as_deref()));
        cx.spawn(async move |this, cx| {
            let got = rx.await.unwrap_or_else(|_| Err(String::new()));
            let _ = this.update(cx, |r, cx| {
                r.games.sorts = Some(got);
                cx.notify();
            });
        })
        .detach();
    }
    if let Some(sort) = shown_sort(r) {
        if r.games.asked_rows.insert(sort.id.clone()) {
            let c = cookie.clone();
            let id = sort.id.clone();
            let rx = roblox::run(move || roblox::sort_games(c.as_deref(), &sort));
            cx.spawn(async move |this, cx| {
                let got = rx.await.unwrap_or_else(|_| Err(String::new()));
                let _ = this.update(cx, |r, cx| {
                    r.games.rows.insert(id, got);
                    cx.notify();
                });
            })
            .detach();
        }
    }
    let mut want: Vec<i64> = Vec::new();
    if let Some(Ok(list)) = shown_sort(r).and_then(|s| r.games.rows.get(&s.id)) {
        want.extend(list.iter().map(|g| g.universe));
    }
    if let Some((_, Some(Ok(list)))) = &r.games.found {
        want.extend(list.iter().map(|g| g.universe));
    }
    want.retain(|u| *u > 0 && !r.games.icons.contains_key(u) && !r.games.asked_icons.contains(u));
    want.sort_unstable();
    want.dedup();
    if !want.is_empty() {
        r.games.asked_icons.extend(want.iter().copied());
        let rx = roblox::run(move || roblox::both(|| roblox::icons(&want), || roblox::covers(&want, "480x270")));
        cx.spawn(async move |this, cx| {
            let Ok((icons, covers)) = rx.await else { return };
            let _ = this.update(cx, |r, cx| {
                r.games.icons.extend(icons);
                r.games.covers.extend(covers);
                cx.notify();
            });
        })
        .detach();
    }
    // The game that leads the chart shown is drawn big: a sharper picture for it.
    let lead = shown_sort(r).and_then(|s| r.games.rows.get(&s.id).and_then(|l| l.as_ref().ok()).and_then(|l| l.first()).map(|g| g.universe)).filter(|u| *u > 0);
    if let Some(u) = lead {
        if r.games.asked_big.insert(u) {
            let rx = roblox::run(move || roblox::covers(&[u], "768x432"));
            cx.spawn(async move |this, cx| {
                let Ok(got) = rx.await else { return };
                let _ = this.update(cx, |r, cx| {
                    r.games.covers_big.extend(got);
                    cx.notify();
                });
            })
            .detach();
        }
    }
}

/// The charts' row shown: the one picked, else the first.
fn shown_sort(r: &Root) -> Option<roblox::Sort> {
    let Some(Ok(sorts)) = &r.games.sorts else { return None };
    r.games.sort_at.as_ref().and_then(|id| sorts.iter().find(|s| s.id == *id)).or(sorts.first()).cloned()
}

/// Roblox's search for what Games' box holds (Enter).
pub fn search(r: &mut Root, cx: &mut Context<Root>) {
    let q = r.game_search.read(cx).value().trim().to_string();
    if q.is_empty() {
        r.games.found = None;
        cx.notify();
        return;
    }
    r.go(Page::Games);
    r.games.open = None;
    r.games.view = View::Games;
    r.games.found = Some((q.clone(), None));
    cx.notify();
    let ask = q.clone();
    let rx = roblox::run(move || roblox::search(&ask));
    cx.spawn(async move |this, cx| {
        let got = rx.await.unwrap_or_else(|_| Err(String::new()));
        let _ = this.update(cx, |r, cx| {
            if r.games.found.as_ref().is_some_and(|(at, _)| *at == q) {
                r.games.found = Some((q, Some(got)));
                cx.notify();
            }
        });
    })
    .detach();
}

// ───────────────────────── the cards ─────────────────────────

pub fn games(root: &Root, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let all = list(root);
    let shell = div().id("games").size_full().rounded(px(R_PANEL)).bg(c(PANEL)).overflow_hidden().flex().flex_col();
    let content = match root.games.open.clone() {
        Some(o) => shell.child(page(root, &all, o, width, cx)).into_any_element(),
        None if root.games.view == View::Friends => crate::pages::friends::friends(root, width, cx).into_any_element(),
        None if root.games.view == View::Avatar => crate::pages::avatar::avatar(root, width, cx).into_any_element(),
        None if root.games.view == View::Profile => crate::pages::profile::profile(root, width, cx).into_any_element(),
        None if root.games.view == View::Shop => crate::pages::profile::shop(root, width, cx).into_any_element(),
        None if root.games.view == View::Inbox => crate::pages::inbox::inbox(root, width, cx).into_any_element(),
        None if root.games.view == View::Settings => crate::pages::rsettings::page(root, width, cx).into_any_element(),
        None => shell.child(grid(root, &all, width, cx)).into_any_element(),
    };
    // The account tabs over it all (pages/tabs.rs): what is under them is the tab shown.
    div().size_full().flex().flex_col().gap(px(8.)).child(crate::pages::tabs::strip(root, cx)).child(div().flex_1().min_h_0().child(content))
}

/// Where the news come from, as a dot and a few words.
fn status(root: &Root) -> (bool, String) {
    let f = |k: &str| root.state.pointer(&format!("/feeds/{k}")).and_then(Value::as_bool).unwrap_or(false);
    let key = if !f("on") {
        "news.offHere"
    } else if !f("connected") {
        if f("loaded") { "news.offline" } else { "news.connecting" }
    } else {
        "news.live"
    };
    (f("connected"), root.tr(key))
}

fn grid(root: &Root, all: &[Game], width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let inner = width - 2. * (PAD + 4.);
    let query = root.game_search.read(cx).value().trim().to_lowercase();
    let shown: Vec<&Game> = all.iter().filter(|g| query.is_empty() || g.name.to_lowercase().contains(&query)).collect();
    let card_w = home_fit(inner).1 + 2. * CPAD;
    let (live, words) = status(root);

    let header = div()
        .flex_none()
        .px(px(PAD + 4.))
        .pt(px(18.))
        .pb(px(16.))
        .flex()
        .items_center()
        .gap(px(10.))
        .child(div().text_size(px(T_TITLE)).font_weight(FontWeight::BOLD).child("Roblox"))
        .child(div().ml(px(6.)).child(views(root, cx)))
        // (Not on a narrow page: it squeezed the search to "Search Robl", the owner's 1200 px window, 2026-11-12.)
        .when(width >= 1150., |d| d.child({
            // (What "the games' own alerts" are, on a hover: review 3a.3.)
            let tip: SharedString = root.tr("news.liveTip").into();
            div()
                .id("news-live")
                .ml(px(8.))
                .min_w_0()
                .flex()
                .items_center()
                .gap(px(7.))
                .text_size(px(T_S))
                .text_color(c(MUTED))
                .child(div().size(px(7.)).flex_none().rounded_full().bg(if live { c(OK) } else { c(DIM) }))
                .child(div().truncate().child(words))
                .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
        }))
        .child(div().flex_1())
        .child(
            div()
                .w(px(if width < 900. { 220. } else { 280. }))
                .min_w(px(200.))
                .flex_shrink(1.)
                .child(field(&root.game_search, Some("search"))),
        );

    let mut body = div().flex().flex_col().gap(px(28.)).px(px(PAD + 4.)).pt(px(4.)).pb(px(24.));
    // A search sent to Roblox (Enter): its games first, the bot's that match after.
    if let Some((q, got)) = &root.games.found {
        body = body.child(roblox_section(root, root.i18n.tv("games.results", &[("q", q)]), None, None, got.as_ref(), inner, cx));
        if !shown.is_empty() {
            body = body.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(div().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT_2)).child(root.tr("games.withWiki")))
                    .child(div().mx(px(-CPAD)).flex().flex_wrap().gap(px(CGAP)).children(shown.into_iter().map(|g| card(root, g, card_w, cx).into_any_element()).collect::<Vec<_>>())),
            );
        }
        return div().size_full().flex().flex_col().child(header).child(crate::scrolled::body("games-body", body));
    }
    // As Roblox's home has it: the friends' faces, the games played lately, Roblox's charts; then the bot's games
    // (yours: followed, read or played; then the others).
    let (yours, others): (Vec<&Game>, Vec<&Game>) = shown.iter().copied().partition(|g| mine(g));
    if query.is_empty() {
        body = body.children(friends_strip(root, inner, cx)).children(keep_playing(root, inner, cx));
        body = body.child(charts(root, inner, cx));
        if !yours.is_empty() {
            body = body.child(section(root.tr("games.mine"), yours.into_iter().map(|g| card(root, g, card_w, cx).into_any_element()).collect()));
        }
        if !others.is_empty() {
            body = body.child(section(root.tr("games.withWiki"), others.into_iter().map(|g| card(root, g, card_w, cx).into_any_element()).collect()));
        }
        return div().size_full().flex().flex_col().child(header).child(crate::scrolled::body("games-body", body));
    }
    if all.is_empty() {
        let key = if root.games.wiki_games.is_some() { "news.noneTitle" } else { "news.connecting" };
        body = body.child(
            div()
                .pt(px(40.))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(6.))
                .child(div().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).child(root.tr(key)))
                .child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("news.noneText"))),
        );
    } else if shown.is_empty() {
        body = body.child(div().pt(px(24.)).text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("wiki.nothing")));
    } else {
        let split = query.is_empty() && shown.iter().any(|g| mine(g)) && !shown.iter().all(|g| mine(g));
        let parts: Vec<(Option<String>, Vec<&Game>)> = if split {
            vec![
                (Some(root.tr("games.mine")), shown.iter().copied().filter(|g| mine(g)).collect()),
                (Some(root.tr("games.popular")), shown.iter().copied().filter(|g| !mine(g)).collect()),
            ]
        } else {
            vec![(None, shown)]
        };
        for (title, games) in parts {
            body = body.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .when_some(title, |d, t| d.child(div().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT_2)).child(t)))
                    .child(div().mx(px(-CPAD)).flex().flex_wrap().gap(px(CGAP)).children(games.into_iter().map(|g| card(root, g, card_w, cx).into_any_element()).collect::<Vec<_>>())),
            );
        }
    }
    div().size_full().flex().flex_col().child(header).child(crate::scrolled::body("games-body", body))
}

/// A row's heading that opens more (as Roblox's "Friends (184) ›"): its words, a count, and a chevron.
fn more_heading(id: &'static str, title: String) -> Hot {
    hot(id)
        .flex()
        .flex_none()
        .items_center()
        .gap(px(4.))
        .px(px(6.))
        .mx(px(-6.))
        .h(px(28.))
        .rounded(px(R_S))
        .cursor_pointer()
        .text_size(px(T_H))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c(TEXT_2))
        .hover(|d| d.text_color(c(TEXT)))
        .child(title)
        .child(icon("chevronRight", 15., c(MUTED)))
}

/// The friends of the account whose tab is shown, faces in a row (those in a game first, then online, then the
/// rest), as many as fit: a face opens their profile here, the heading opens Friends, and the first circle adds one.
/// Only once the friends are in (they need the account's sign-in).
fn friends_strip(root: &Root, width: f32, cx: &mut Context<Root>) -> Option<AnyElement> {
    let Some(Ok(list)) = &root.friends.list else { return None };
    if list.is_empty() {
        return None;
    }
    let rank = |k: i64| match k {
        2 => 0,
        1 | 3 => 1,
        _ => 2,
    };
    let mut people: Vec<&roblox::Friend> = list.iter().collect();
    people.sort_by_key(|f| rank(f.kind));
    const FACE: f32 = 72.;
    const CELL: f32 = 92.;
    const GAP_F: f32 = 10.;
    let fit = (((width + GAP_F) / (CELL + GAP_F)).floor() as usize).max(2);
    let face_circle = |inner: AnyElement| div().relative().size(px(FACE)).flex_none().rounded_full().bg(c(RAISED_2)).flex().items_center().justify_center().child(inner);
    let add = hot("gf-add")
        .w(px(CELL))
        .flex_none()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(8.))
        .cursor_pointer()
        .lift(2.)
        .child(face_circle(icon("userPlus", 26., c(TEXT_2)).into_any_element()))
        .child(div().text_size(px(T_S)).font_weight(FontWeight::MEDIUM).text_color(c(TEXT_2)).child(root.tr("games.addFriend")))
        .on_click(cx.listener(|r, _, _, cx| {
            r.games.view = View::Friends;
            r.friends.adding = true;
            cx.notify();
        }));
    let faces = people.into_iter().take(fit - 1).enumerate().map(|(i, f)| {
        let pic = (!f.face.is_empty()).then(|| images::pic_pair(&f.face, FACE, FACE, cx)).flatten();
        let dot = match f.kind {
            2 => Some(OK),
            1 | 3 => Some(ONLINE),
            _ => None,
        };
        let who = f.id;
        let shown = if f.display.is_empty() { f.name.clone() } else { f.display.clone() };
        hot(("gf-face", i))
            .w(px(CELL))
            .flex_none()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(8.))
            .cursor_pointer()
            .lift(2.)
            .child(
                face_circle(match pic {
                    Some((p, k)) => fade_in(img(p).size_full().rounded_full().object_fit(ObjectFit::Cover), &k).into_any_element(),
                    None => letter(&shown, 26.).into_any_element(),
                })
                .when_some(dot, |d, col| d.child(div().absolute().right(px(2.)).bottom(px(2.)).size(px(16.)).rounded_full().bg(c(PANEL)).flex().items_center().justify_center().child(div().size(px(10.)).rounded_full().bg(c(col))))),
            )
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(1.))
                    .child(div().max_w_full().truncate().text_size(px(T_S)).font_weight(FontWeight::MEDIUM).child(shown))
                    // (Where they are, when Roblox says: the one thing Roblox's row of faces doesn't show.)
                    .when(f.kind == 2 && !f.game.is_empty(), |d| d.child(div().max_w_full().truncate().text_size(px(T_XS)).text_color(c(OK)).child(clean(&f.game)))),
            )
            .on_click(cx.listener(move |r, _, _, cx| crate::pages::profile::open_person(r, who, cx)))
    }).collect::<Vec<_>>();
    let title = root.i18n.tv("games.friendsN", &[("n", &list.len().to_string())]);
    Some(
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(div().flex().child(more_heading("gf-all", title).on_click(cx.listener(|r, _, _, cx| {
                r.games.view = View::Friends;
                cx.notify();
            }))))
            .child(div().flex().gap(px(GAP_F)).child(add).children(faces))
            .into_any_element(),
    )
}

/// The games this PC's accounts play now and played lately (Home's list), wide pictures on the page's grid (Home's
/// tiles): a click opens the game's page. Running ones say how many windows; the others, the hours played.
fn keep_playing(root: &Root, width: f32, cx: &mut Context<Root>) -> Option<AnyElement> {
    let all = crate::pages::home::games(root);
    if all.is_empty() {
        return None;
    }
    let (cols, w) = home_fit(width);
    let vi = root.i18n.vietnamese;
    let n = all.len();
    let tiles: Vec<AnyElement> = all
        .into_iter()
        .take(cols)
        .enumerate()
        .map(|(i, g)| {
            let running = !g.accounts.is_empty();
            let sub = if running {
                root.i18n.tv("home.runningN", &[("n", &g.accounts.len().to_string())])
            } else if g.hours >= 1. {
                root.i18n.tv("home.hours", &[("n", &thousands(g.hours.round() as i64, vi))])
            } else {
                root.tr("home.recentShort")
            };
            let (place, name) = (g.place, g.name.clone());
            let icon_url = look(root, g.place, "icon").or(g.icon.clone());
            cascade(
                "gk-in",
                i,
                crate::pages::home::cover_tile(("gk-tile", i), look(root, g.place, "cover"), icon_url, g.name.clone(), (sub, running), g.accounts.len(), false, w, cx).on_click(
                    cx.listener(move |r, _, _, cx| {
                        open_place(r, place, &name);
                        cx.notify();
                    }),
                ),
            )
            .into_any_element()
        })
        .collect();
    Some(div().flex().flex_col().gap(px(12.)).child(section_head(root.tr("games.keepPlaying"), Some(n))).child(crate::kit::grid(GRID_GAP).children(tiles)).into_any_element())
}

fn section(title: String, cards: Vec<AnyElement>) -> impl IntoElement {
    let n = cards.len();
    div().flex().flex_col().gap(px(12.)).child(section_head(title, Some(n))).child(div().mx(px(-CPAD)).flex().flex_wrap().gap(px(CGAP)).children(cards))
}

/// Roblox's charts: a pill a row (Trending, Up-and-coming…), and the row's games.
fn charts(root: &Root, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let at = shown_sort(root);
    let pills = match &root.games.sorts {
        Some(Ok(sorts)) => {
            let label = |s: &roblox::Sort| {
                let key = format!("rx.{}", s.id);
                if root.i18n.has(&key) { root.tr(&key) } else { s.name.clone() }
            };
            // One line of them. A signed-in account gets some thirty rows from Roblox, which wrapped into three
            // lines of pills (the owner's screenshot, 2026-11-11): the ones that fit the line show, the rest are in
            // a menu under More; the row shown always has its pill. (A pill's width is reckoned from its words.)
            let room = width - 2. * (PAD + 4.) - 8. - 96.;
            let wide = |s: &roblox::Sort| label(s).chars().count() as f32 * 7.4 + 24. + 2.;
            let mut used = 0.;
            let mut shown: Vec<&roblox::Sort> = Vec::new();
            for s in sorts {
                if used + wide(s) > room {
                    break;
                }
                used += wide(s);
                shown.push(s);
            }
            let all_fit = shown.len() == sorts.len();
            if let Some(a) = at.as_ref().and_then(|a| sorts.iter().find(|s| s.id == a.id)) {
                if !shown.iter().any(|s| s.id == a.id) {
                    shown.pop();
                    shown.push(a);
                }
            }
            let rest: Vec<&roblox::Sort> = sorts.iter().filter(|s| !shown.iter().any(|x| x.id == s.id)).collect();
            let mut row = div().flex().flex_none().items_center().gap(px(4.));
            for s in shown {
                let on = at.as_ref().is_some_and(|a| a.id == s.id);
                let id = s.id.clone();
                row = row.child(
                    chip(SharedString::from(format!("rx-{}", s.id)), on)
                        .h(px(TOOL_H))
                        .px(px(12.))
                        .cursor_pointer()
                        .child(label(s))
                        .on_click(cx.listener(move |r, _, _, cx| {
                            r.games.sort_at = Some(id.clone());
                            r.games.more_sorts = false;
                            cx.notify();
                        })),
                );
            }
            if !all_fit && !rest.is_empty() {
                let open = root.games.more_sorts;
                let mut more = div().relative().flex_none().child(
                    tool("rx-more", None, open)
                        .h(px(TOOL_H))
                        .px(px(10.))
                        .gap(px(4.))
                        .cursor_pointer()
                        .child(root.tr("games.moreSorts"))
                        .child(icon(if open { "chevronUp" } else { "chevronDown" }, 13., c(MUTED)))
                        .on_click(cx.listener(|r, _, _, cx| {
                            r.games.more_sorts = !r.games.more_sorts;
                            cx.notify();
                        })),
                );
                {
                    // Two columns of them when there are many. (Built open or not: kit::pop draws it while it is.)
                    let menu = div()
                        .id("rx-more-menu")
                        .occlude()
                        .w(px(if rest.len() > 10 { 468. } else { 240. }))
                        .p(px(4.))
                        .rounded(px(R_M))
                        .bg(c(RAISED))
                        .border_1()
                        .border_color(c(LINE))
                        .shadow_lg()
                        .flex()
                        .flex_wrap()
                        .on_mouse_down_out(cx.listener(|r, _, _, cx| {
                            r.games.more_sorts = false;
                            cx.notify();
                        }))
                        .children(rest.into_iter().map(|s| {
                            let id = s.id.clone();
                            menu_row(SharedString::from(format!("rxm-{}", s.id)))
                                .w(px(230.))
                                .child(div().truncate().child(label(s)))
                                .on_click(cx.listener(move |r, _, _, cx| {
                                    r.games.sort_at = Some(id.clone());
                                    r.games.more_sorts = false;
                                    cx.notify();
                                }))
                        }));
                    more = more.child(div().absolute().top(px(34.)).right_0().child(deferred(anchored().anchor(Anchor::TopRight).snap_to_window_with_margin(px(8.)).child(pop("rx-more-pop", open, menu))).with_priority(1)));
                }
                row = row.child(more);
            }
            Some(div().flex().child(row))
        }
        _ => None,
    };
    let got = match (&root.games.sorts, at.as_ref().and_then(|s| root.games.rows.get(&s.id))) {
        (Some(Err(e)), _) => Some(Err(e.clone())),
        (_, Some(r)) => Some(r.clone()),
        _ => None,
    };
    // The chart's own name, said on the game that leads it.
    let chart = at.as_ref().map(|s| {
        let key = format!("rx.{}", s.id);
        if root.i18n.has(&key) { root.tr(&key) } else { s.name.clone() }
    });
    roblox_section(root, root.tr("games.charts"), pills, chart, got.as_ref(), width, cx)
}

/// A heading, what to pick under it (the charts' rows), and Roblox's games as a board of pictures: loading (the
/// tiles' shapes, breathing), failed, or empty said in words. An empty row with no sign-in to send is Roblox's rule,
/// said so. `chart` is the name of the chart shown (none for a search's answer): its games are numbered, and the
/// first one leads, big.
fn roblox_section(root: &Root, title: String, pills: Option<Div>, chart: Option<String>, got: Option<&Result<Vec<roblox::Game>, String>>, width: f32, cx: &mut Context<Root>) -> AnyElement {
    // What there is instead of games, on one quiet line of its own (not a grey sentence over an empty stretch).
    let note = |t: String, add: Option<AnyElement>| {
        div()
            .min_h(px(52.))
            .px(px(16.))
            .py(px(12.))
            .flex()
            .items_center()
            .gap(px(10.))
            .rounded(px(R_L))
            .bg(c(RAISED))
            .border_1()
            .border_color(c(LINE))
            .text_size(px(T_M))
            .text_color(c(TEXT_2))
            .child(icon("info", 15., c(MUTED)))
            .child(div().flex_1().min_w_0().child(t))
            .children(add)
            .into_any_element()
    };
    let charts = pills.is_some();
    // No account signed in to ask with: the charts' pills would pick among nothing. The heading and one line that says
    // why, with Add account in it (five pills over an info line looked unfinished, review 3a.1).
    let locked = charts && matches!(got, Some(Ok(l)) if l.is_empty()) && matches!(root.games.cookie, Some(None));
    let pills = if locked { None } else { pills };
    let body = match got {
        None => {
            let (cols, w, art_h, tile_h) = board_fit(width);
            let _ = art_h;
            waiting_rows(cols * 2, w - 2. * TPAD, tile_h - 2. * TPAD, TGAP + 2. * TPAD).into_any_element()
        }
        Some(Err(_)) => note(root.tr(if charts { "games.chartsFailed" } else { "games.noResults" }), None),
        Some(Ok(_)) if locked => note(
            root.tr("games.chartsSignIn"),
            Some(Button::new("rx-add-acc", root.tr("acc.add")).icon("plus").small().on_click(cx.listener(|r, _, _, cx| r.add_account(cx))).into_any_element()),
        ),
        Some(Ok(list)) if list.is_empty() => note(root.tr(if !charts { "games.noResults" } else { "games.chartsFailed" }), None),
        Some(Ok(list)) => board(root, list, chart, width, cx),
    };
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(section_head(title, None))
        .children(pills)
        .child(body)
        .into_any_element()
}

/// A tile's picture is about this wide; a tile has its own padding (the ground that comes up under the mouse), and
/// tiles stand this far apart.
const TILE: f32 = 232.;
const TPAD: f32 = 6.;
const TGAP: f32 = GRID_GAP - 2. * TPAD;

/// How the board fits a width: tiles a row, a tile's width (with its padding), its picture's height, its height.
fn board_fit(width: f32) -> (usize, f32, f32, f32) {
    let outer = width + 2. * TPAD;
    let cols = ((outer + TGAP) / (TILE + 2. * TPAD + TGAP)).floor().max(2.) as usize;
    let w = ((outer - TGAP * (cols as f32 - 1.)) / cols as f32).floor();
    let art_h = ((w - 2. * TPAD) * 9. / 16.).round();
    (cols, w, art_h, art_h + 2. * TPAD + 46.)
}

/// Roblox's games as pictures, a board of them. A chart is numbered, and its first game leads: its picture takes two
/// tiles by two, with its name and who plays it written on it.
fn board(root: &Root, list: &[roblox::Game], chart: Option<String>, width: f32, cx: &mut Context<Root>) -> AnyElement {
    let (cols, w, art_h, tile_h) = board_fit(width);
    let list: Vec<&roblox::Game> = list.iter().take(60).collect();
    let ranked = chart.is_some();
    // (The tiles come in one after another, anew for another chart or search.)
    let scope: SharedString = format!("rx-in-{}", chart.clone().unwrap_or_default()).into();
    let beside = 2 * cols.saturating_sub(2);
    let mut out = div().mx(px(-TPAD)).flex().flex_col().gap(px(TGAP));
    let mut from = 0;
    if ranked && cols >= 4 && list.len() > beside {
        let side_w = (cols - 2) as f32 * w + (cols - 3) as f32 * TGAP;
        let side: Vec<AnyElement> = list[1..=beside].iter().enumerate().map(|(i, g)| cascade(scope.clone(), i + 1, tile(root, i + 1, g, ranked, w, art_h, tile_h, cx)).into_any_element()).collect();
        out = out.child(
            div()
                .flex()
                .gap(px(TGAP))
                .child(cascade(scope.clone(), 0, lead(root, list[0], chart.unwrap_or_default(), 2. * w + TGAP, 2. * tile_h + TGAP, cx)))
                .child(div().w(px(side_w)).flex_none().flex().flex_wrap().gap(px(TGAP)).children(side)),
        );
        from = beside + 1;
    }
    let rest: Vec<AnyElement> = list[from..].iter().enumerate().map(|(i, g)| cascade(scope.clone(), from + i, tile(root, from + i, g, ranked, w, art_h, tile_h, cx)).into_any_element()).collect();
    out.child(div().flex().flex_wrap().gap(px(TGAP)).children(rest)).into_any_element()
}

/// A game's wide picture (its square icon until that has come), or the first letter of its name.
/// (Made at the `w`×`h` it is shown at, and with the picture itself, which keys its fade-in.)
fn art(root: &Root, g: &roblox::Game, sharp: bool, w: f32, h: f32, cx: &mut App) -> Option<(ImageSource, std::sync::Arc<Image>)> {
    let u = &g.universe;
    let wide = if sharp { root.games.covers_big.get(u).or_else(|| root.games.covers.get(u)) } else { root.games.covers.get(u) };
    let pic = wide.or_else(|| root.games.icons.get(u)).cloned().or_else(|| look(root, g.place, "icon"));
    pic.as_deref().and_then(|u| images::pic_pair(u, w, h, cx))
}

/// A chart game's own colour, from its icon (else its cover): the ground of its picture.
fn art_tone(root: &Root, g: &roblox::Game, cx: &mut App) -> Option<u32> {
    let u = &g.universe;
    let pic = root.games.icons.get(u).or_else(|| root.games.covers.get(u)).cloned().or_else(|| look(root, g.place, "icon"));
    pic.as_deref().and_then(|u| images::tone(u, cx))
}

fn letter(name: &str, size: f32) -> Div {
    let first: String = name.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().collect()).unwrap_or_default();
    div().text_size(px(size)).font_weight(FontWeight::BOLD).text_color(c(MUTED)).child(first)
}

/// Who plays it now, on its picture: a green dot and the number.
fn playing_pill(root: &Root, n: i64) -> Div {
    div()
        .h(px(22.))
        .px(px(8.))
        .flex()
        .items_center()
        .gap(px(6.))
        .rounded(px(7.))
        .bg(ca(GROUND, 0.8))
        .text_size(px(T_S))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c(TEXT))
        .font_features(tnum())
        .child(div().size(px(6.)).flex_none().rounded_full().bg(c(OK)))
        .child(thousands(n, root.i18n.vietnamese))
}

/// One of Roblox's games on the board: its picture with its place in the chart and who plays it now on it, its name,
/// and how many like it. A click opens its page here.
#[allow(clippy::too_many_arguments)]
fn tile(root: &Root, i: usize, g: &roblox::Game, ranked: bool, w: f32, art_h: f32, tile_h: f32, cx: &mut Context<Root>) -> Hot {
    let image = art(root, g, false, w - 2. * TPAD, art_h, cx);
    let tone = art_tone(root, g, cx);
    let votes = g.up + g.down;
    let (place, name) = (g.place, g.name.clone());
    hot(SharedString::from(format!("rxg-{i}-{}", g.place)))
        .w(px(w))
        .h(px(tile_h))
        .flex_none()
        .p(px(TPAD))
        .flex()
        .flex_col()
        .rounded(px(R_L + 4.))
        .cursor_pointer()
        .lift(2.)
        .on_click(cx.listener(move |r, _, _, cx| {
            open_place(r, place, &name);
            cx.notify();
        }))
        .child(
            div()
                .relative()
                .w_full()
                .h(px(art_h))
                .flex_none()
                .rounded(px(R_L))
                .overflow_hidden()
                .bg(match tone {
                    Some(t) => mix(c(t), c(PANEL), 0.55),
                    None => c(RAISED_2),
                })
                .flex()
                .items_center()
                .justify_center()
                .map(|d| match image {
                    Some((p, k)) => d.child(fade_in(img(p).size_full().rounded(px(R_L)).object_fit(ObjectFit::Cover), &k)),
                    None => d.child(letter(&g.name, 34.)),
                })
                .when(ranked, |d| {
                    d.child(
                        div()
                            .absolute()
                            .top(px(8.))
                            .left(px(8.))
                            .h(px(22.))
                            .min_w(px(22.))
                            .px(px(6.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(7.))
                            .bg(ca(GROUND, 0.8))
                            .text_size(px(T_S))
                            .font_weight(FontWeight::BOLD)
                            .text_color(c(TEXT))
                            .font_features(tnum())
                            .child((i + 1).to_string()),
                    )
                })
                .when(g.playing > 0, |d| d.child(div().absolute().left(px(8.)).bottom(px(8.)).child(playing_pill(root, g.playing)))),
        )
        .child(div().mt(px(8.)).h(px(18.)).flex_none().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).line_height(px(18.)).truncate().child(clean(&g.name)))
        .child(
            div()
                .mt(px(3.))
                .h(px(15.))
                .flex_none()
                .text_size(px(T_S))
                .line_height(px(15.))
                .text_color(c(MUTED))
                .font_features(tnum())
                .when(votes > 0, |d| d.child(root.i18n.tv("games.likesP", &[("n", &((g.up * 100 + votes / 2) / votes).to_string())]))),
        )
}

/// The game that leads a chart: its picture two tiles by two, and on it the chart it leads, its name, who plays it
/// and how many like it.
fn lead(root: &Root, g: &roblox::Game, chart: String, w: f32, h: f32, cx: &mut Context<Root>) -> Hot {
    let image = art(root, g, true, w - 2. * TPAD, h - 2. * TPAD, cx);
    let votes = g.up + g.down;
    let (place, name) = (g.place, g.name.clone());
    hot(SharedString::from(format!("rxg-lead-{}", g.place)))
        .w(px(w))
        .h(px(h))
        .flex_none()
        .p(px(TPAD))
        .rounded(px(R_L + 4.))
        .cursor_pointer()
        .lift(2.)
        .on_click(cx.listener(move |r, _, _, cx| {
            open_place(r, place, &name);
            cx.notify();
        }))
        .child(
            div()
                .relative()
                .size_full()
                .rounded(px(R_L))
                .overflow_hidden()
                .bg(c(RAISED_2))
                .flex()
                .items_center()
                .justify_center()
                .map(|d| match image {
                    Some((p, k)) => d.child(fade_in(img(p).size_full().rounded(px(R_L)).object_fit(ObjectFit::Cover), &k)),
                    None => d.child(letter(&g.name, 64.)),
                })
                // The words' ground: the picture darkens toward its foot. (Laid twice: GPUI's see-through gradients
                // come out about half as strong as their stops say.)
                .children((0..2).map(|_| div().absolute().inset_0().rounded(px(R_L)).bg(linear_gradient(180., linear_color_stop(ca(GROUND, 0.), 0.4), linear_color_stop(ca(GROUND, 0.9), 1.)))))
                .child(
                    div()
                        .absolute()
                        .left(px(20.))
                        .right(px(20.))
                        .bottom(px(18.))
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(div().h(px(22.)).px(px(8.)).flex().items_center().rounded(px(7.)).bg(c(FACE)).text_size(px(T_S)).font_weight(FontWeight::BOLD).text_color(c(INK)).child("#1"))
                                .child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).text_color(on_picture(0.82)).child(chart)),
                        )
                        .child(div().text_size(px(T_STAGE)).font_weight(FontWeight::BOLD).line_height(px(33.)).text_color(on_picture(1.)).line_clamp(2).child(clean(&g.name)))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .text_size(px(T_M))
                                .text_color(on_picture(0.8))
                                .font_features(tnum())
                                .when(g.playing > 0, |d| {
                                    d.child(div().size(px(7.)).flex_none().rounded_full().bg(c(OK))).child(root.i18n.tv("rx.playingN", &[("n", &thousands(g.playing, root.i18n.vietnamese))]))
                                })
                                .when(g.playing > 0 && votes > 0, |d| d.child(sep()))
                                .when(votes > 0, |d| d.child(root.i18n.tv("games.likesP", &[("n", &((g.up * 100 + votes / 2) / votes).to_string())]))),
                        ),
                ),
        )
}

/// The little that says what a game has here, on one line: news (how many new), your hours, who plays it, prices.
/// "Radar": the games where several Bloxkeep users dropped out the same way just now (the community's figures).
fn radar(root: &Root, place: i64, name: &str) -> Vec<Value> {
    root.state
        .pointer("/community/data/radar")
        .and_then(Value::as_array)
        .map(|l| l.iter().filter(|r| same(int(r, "place"), s(r, "name"), place, name)).cloned().collect())
        .unwrap_or_default()
}

/// Those warnings for one game, on its page: what happened, to how many PCs, in how many minutes.
fn radar_strip(root: &Root, place: i64, name: &str) -> Option<Div> {
    let lines = radar(root, place, name);
    if lines.is_empty() {
        return None;
    }
    // The Accounts page's calm panel for what needs a look: a faint ground of the warning's colour, the mark on its
    // own small block, the words in plain text.
    let mut strip = div().px(px(12.)).py(px(8.)).rounded(px(R_L + 2.)).bg(tinted(PANEL, WARN, 0.07)).border_1().border_color(ca(WARN, 0.26)).flex().flex_col().gap(px(4.));
    for r in &lines {
        let text = root.i18n.tv(&format!("cm.radar.{}", s(r, "why")), &[("n", &int(r, "pcs").to_string()), ("m", &int(r, "minutes").to_string())]);
        strip = strip.child(
            div()
                .min_h(px(32.))
                .flex()
                .items_center()
                .gap(px(10.))
                .text_size(px(T_M))
                .line_height(px(19.))
                .text_color(c(TEXT))
                .child(div().size(px(24.)).flex_none().rounded(px(7.)).bg(ca(WARN, 0.18)).flex().items_center().justify_center().child(icon("alert", 14., c(WARN))))
                .child(div().min_w_0().flex_1().child(text)),
        );
    }
    Some(strip)
}

fn tags(root: &Root, g: &Game) -> Vec<(String, bool)> {
    let mut v = Vec::new();
    if !radar(root, g.place, &g.name).is_empty() {
        v.push((root.tr("cm.radarTag"), true));
    }
    if g.news.is_some() {
        let n = unread(g);
        v.push(if n > 0 { (root.i18n.tv("news.unreadN", &[("n", &n.to_string())]), true) } else { (root.tr("games.news"), false) });
    }
    if let Some(h) = g.played {
        let n = if h < 1. { "<1".to_string() } else { format!("{:.0}", h) };
        v.push((root.i18n.tv("games.playedH", &[("n", &n)]), false));
    }
    if g.playing > 0 {
        v.push((root.i18n.tv("cm.playingN", &[("n", &g.playing.to_string())]), false));
    }
    if g.prices {
        v.push((root.tr("prices.tab"), false));
    }
    // Two fit a card's line at any width; the game's page says the rest.
    v.truncate(2);
    v
}

fn game_icon(pic: Option<String>, name: &str, size: f32, cx: &mut App) -> Div {
    let image = pic.as_deref().and_then(|u| images::pic_sq(u, size, cx));
    let first: String = name.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().collect()).unwrap_or_default();
    div()
        .size(px(size))
        .flex_none()
        .rounded(px(if size > 40. { R_L } else { R_M }))
        .overflow_hidden()
        .bg(c(RAISED_2))
        .flex()
        .items_center()
        .justify_center()
        .map(|d| match image {
            Some(i) => d.child(img(i).size_full().object_fit(ObjectFit::Cover)),
            None => d.text_size(px(size * 0.42)).font_weight(FontWeight::BOLD).text_color(c(MUTED)).child(first),
        })
}

/// A game: its picture, then its icon, name and what it has; a tick when followed, "New" when the bot just added it.
fn card(root: &Root, g: &Game, w: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let cover_url = look(root, g.place, "cover");
    let cover = cover_url.as_deref().and_then(|u| images::pic_at(u, w - 12., (w - 12.) * 9. / 16., cx));
    // The game's own colour (from its icon, a PNG; else its cover): the ground under its icon when no cover came.
    let tone = icon_of(root, g).as_deref().and_then(|u| images::tone(u, cx)).or_else(|| cover_url.as_deref().and_then(|u| images::tone(u, cx)));
    let h = (w - 12.) * 9. / 16.;
    let is_new = g.news.as_ref().and_then(|n| n.get("isNew")).and_then(Value::as_bool).unwrap_or(false);
    let open = Open { id: g.id.clone(), place: g.place, name: g.name.clone() };
    let group = SharedString::from(format!("gm-{}", g.id));
    let tags = tags(root, g);
    let mut line = div().flex().items_center().gap(px(7.)).overflow_hidden().text_size(px(T_S)).text_color(c(MUTED));
    // Followed: said in words at the head of the line (a lone round tick at the card's edge had no label, review 3a.2).
    let follows = followed(g);
    if follows {
        line = line.child(div().flex_none().flex().items_center().gap(px(4.)).text_color(c(TEXT_2)).child(icon("check", 12., c(TEXT_2))).child(root.tr("news.following")));
    }
    for (i, (t, hot)) in tags.into_iter().enumerate() {
        if i > 0 || follows {
            line = line.child(sep());
        }
        line = line.child(div().flex_none().when(hot, |d| d.text_color(c(TEXT)).font_weight(FontWeight::SEMIBOLD)).child(t));
    }
    hot(group.clone())
        .group(group.clone())
        .w(px(w))
        .p(px(CPAD))
        .flex()
        .flex_col()
        // (The text under the picture as Keep playing's tiles have it: the same gaps and line heights, so the rows of
        // Games home share one rhythm, review 3a.2.)
        .gap(px(12.))
        .rounded(px(R_L))
        .cursor_pointer()
        .on_click(cx.listener(move |r, _, _, cx| {
            r.games.open = Some(open.clone());
            r.games.tab = Tab::Overview;
            r.games.about = false;
            cx.notify();
        }))
        .lift(3.)
        .child(
            div()
                .w_full()
                .h(px(h))
                .rounded(px(R_M))
                .overflow_hidden()
                .bg(c(RAISED))
                .flex()
                .items_center()
                .justify_center()
                .map(|d| match cover {
                    Some(p) => d.child(img(p).size_full().object_fit(ObjectFit::Cover)),
                    // No wide picture (not come yet, or none): the game's own icon on its colour, not an empty slab.
                    None => d.when_some(tone, |d, t| d.bg(ca(t, 0.35))).child(game_icon(icon_of(root, g), &g.name, (h * 0.42).min(96.), cx)),
                }),
        )
        .child(
            div()
                .px(px(2.))
                .flex()
                .items_center()
                .gap(px(10.))
                .child(game_icon(icon_of(root, g), &g.name, 36., cx))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(div().text_size(px(T_L)).line_height(px(19.)).font_weight(FontWeight::SEMIBOLD).truncate().child(g.name.clone()))
                        .child(line),
                )
                .when(is_new && !follows, |d| d.child(div().flex_none().text_size(px(T_S)).font_weight(FontWeight::SEMIBOLD).text_color(c(FACE)).child(root.tr("news.new")))),
        )
}

// ───────────────────────── one game ─────────────────────────

fn page(root: &Root, all: &[Game], open: Open, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let game = all.iter().find(|g| g.id == open.id);
    let place = game.map(|g| g.place).filter(|p| *p > 0).unwrap_or(open.place);
    let details = root.games.details.get(&place);
    let name = details.map(|d| s(d, "name").to_string()).filter(|n| !n.is_empty()).map(|n| clean(&n)).unwrap_or_else(|| open.name.clone());
    let has_news = game.is_some_and(|g| g.news.is_some());
    let has_prices = game.is_some_and(|g| g.prices);
    let has_wiki = game.is_some_and(|g| g.wiki.is_some());
    let tab = match root.games.tab {
        Tab::News if has_news => Tab::News,
        Tab::Prices if has_prices => Tab::Prices,
        Tab::Passes if place > 0 => Tab::Passes,
        Tab::Servers if place > 0 => Tab::Servers,
        Tab::Wiki if has_wiki => Tab::Wiki,
        _ => Tab::Overview,
    };
    let inner = width - 2. * (PAD + 4.);
    // BLOXKEEP_SHOT_SCROLL=700: a shot's page scrolled down that far (asked again until the page is long enough).
    #[cfg(feature = "shot")]
    if let Ok(y) = std::env::var("BLOXKEEP_SHOT_SCROLL").map(|v| v.parse::<f32>().unwrap_or(0.)) {
        if (root.games.page_scroll.offset().y + px(y)).abs() > px(1.) {
            root.games.page_scroll.set_offset(point(px(0.), px(-y)));
        }
    }

    // The page's parts as a row of choices at a toolbar's height, the one shown a yellow block (the Accounts page's
    // filters): no underline under a tab.
    let mut tabs = div().mx(px(PAD + 4.)).flex().flex_wrap().items_center().gap(px(4.));
    for (t, icon_name, label, count) in [
        (Tab::Overview, "info", root.tr("gp.overview"), 0),
        (Tab::News, "news", root.tr("games.news"), game.map(unread).unwrap_or(0)),
        (Tab::Prices, "tag", root.tr("prices.tab"), 0),
        (Tab::Passes, "key", root.tr("gp.passes"), 0),
        (Tab::Servers, "server", root.tr("gp.servers"), 0),
        (Tab::Wiki, "book", root.tr("gp.wiki"), 0),
    ] {
        if (t == Tab::News && !has_news) || (t == Tab::Prices && !has_prices) || (matches!(t, Tab::Passes | Tab::Servers) && place <= 0) || (t == Tab::Wiki && !has_wiki) {
            continue;
        }
        tabs = tabs.child(
            tab_button(t, icon_name, label, tab == t)
                .when(count > 0, |d| d.child(chip_count(count as usize, tab == t)))
                .on_click(cx.listener(move |r, _, _, cx| {
                    r.games.tab = t;
                    cx.notify();
                })),
        );
    }
    let only_overview = !has_news && !has_prices && place <= 0;

    let content = match tab {
        Tab::Overview => overview(root, details, place, inner, cx).into_any_element(),
        Tab::News => news(root, game.and_then(|g| g.news.clone()).unwrap_or(Value::Null), inner, cx).into_any_element(),
        Tab::Prices => prices(root, &open.id, inner, cx).into_any_element(),
        Tab::Passes => crate::pages::passes::view(root, width, cx),
        Tab::Servers => crate::pages::servers::view(root, inner, cx),
        Tab::Wiki => crate::pages::wiki::view(root, inner, cx),
    };

    crate::scrolled::with_bar("game-page", &root.games.page_scroll, div().id("game-page").size_full().overflow_y_scroll().track_scroll(&root.games.page_scroll).child(
        div()
            .flex()
            .flex_col()
            .gap(px(18.))
            .pb(px(28.))
            .child(hero(root, game, details, &open, &name, place, width, cx))
            .when_some(root.games.picking.as_ref().filter(|(g, _)| *g == open.id).map(|(_, p)| p.clone()), |d, ticked| {
                d.child(div().px(px(PAD + 4.)).child(join(root, place, ticked, cx)))
            })
            // Roblox's figures belong to the Overview: on the other parts (a wiki table, the values, the servers) what is
            // read starts right under the game's band.
            .when_some(details.filter(|_| tab == Tab::Overview), |d, det| d.child(div().px(px(PAD + 4.)).child(facts(root, det))))
            .when_some(details.and_then(|d| fresh(root, d)), |d, note| {
                d.child(
                    div()
                        .mx(px(PAD + 4.))
                        .px(px(14.))
                        .py(px(10.))
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .rounded(px(R_L + 2.))
                        .bg(tinted(PANEL, WARN, 0.07))
                        .border_1()
                        .border_color(ca(WARN, 0.26))
                        .text_size(px(T_M))
                        .text_color(c(TEXT_2))
                        .child(div().size(px(24.)).flex_none().rounded(px(7.)).bg(ca(WARN, 0.18)).flex().items_center().justify_center().child(icon("refresh", 14., c(WARN))))
                        .child(div().flex_1().min_w_0().child(note)),
                )
            })
            .when(!only_overview, |d| d.child(tabs))
            .child(div().px(px(PAD + 4.)).child(content)),
    ))
}

/// A server picked in the Servers tab: Play's row opens (the accounts picked on the Accounts page ticked, when they
/// are free) and its Join goes into that server.
pub fn pick_for_server(r: &mut Root, job: String) {
    let Some(open) = r.games.open.as_ref().map(|o| o.id.clone()) else { return };
    if !r.games.picking.as_ref().is_some_and(|(g, _)| *g == open) {
        let picked: HashSet<String> = r
            .accounts()
            .iter()
            .filter(|a| crate::pages::accounts::can_play(a))
            .filter_map(|a| a.get("id").and_then(Value::as_str))
            .filter(|id| r.selected.contains(*id))
            .map(str::to_string)
            .collect();
        r.games.picking = Some((open, picked));
    }
    r.games.pick_job = Some(job);
}

/// Play's row: the free accounts as ticks, then Join with those ticked.
fn join(root: &Root, place: i64, ticked: HashSet<String>, cx: &mut Context<Root>) -> impl IntoElement {
    use crate::pages::accounts::{avatar, can_play, label};
    // Not the ones this game has banned (its Overview says Banned for them).
    let banned: HashSet<String> = root
        .games
        .details
        .get(&place)
        .and_then(|d| d.get("accounts"))
        .and_then(Value::as_array)
        .map(|l| l.iter().filter(|a| s(a, "now") == "banned").map(|a| s(a, "id").to_string()).collect())
        .unwrap_or_default();
    let free: Vec<Value> = root.accounts().iter().filter(|a| can_play(a) && !banned.contains(s(a, "id"))).cloned().collect();
    let ids: Vec<String> = free.iter().filter_map(|a| a.get("id").and_then(Value::as_str)).map(str::to_string).collect();
    let n = ids.iter().filter(|i| ticked.contains(*i)).count();
    let all_on = n > 0 && n == ids.len();
    let mut chips = div().flex().flex_wrap().gap(px(8.));
    for (i, a) in free.iter().enumerate() {
        let id = s(a, "id").to_string();
        let on = ticked.contains(&id);
        chips = chips.child(
            hot(("gm-pick", i))
                .h(px(36.))
                .pl(px(4.))
                .pr(px(12.))
                .rounded_full()
                .flex()
                .items_center()
                .gap(px(8.))
                .cursor_pointer()
                .border_1()
                .border_color(if on { ca(WHITE, 0.55) } else { ca(WHITE, 0.) })
                .bg(if on { ca(WHITE, 0.1) } else { ca(WHITE, 0.04) })
                .child(avatar(a, 28., RAISED, cx))
                .child(div().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).text_color(if on { c(TEXT) } else { c(TEXT_2) }).child(label(a)))
                .when(on, |d| d.child(icon("check", 14., c(TEXT))))
                .on_click(cx.listener(move |r, _, _, cx| {
                    if let Some((_, t)) = r.games.picking.as_mut() {
                        if !t.remove(&id) {
                            t.insert(id.clone());
                        }
                    }
                    cx.notify();
                })),
        );
    }
    let head = div()
        .flex()
        .items_center()
        .gap(px(12.))
        .child(div().flex_1().min_w_0().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).child(root.tr("gp.pickWho")))
        .when(ids.len() > 1, |d| {
            let all = ids.clone();
            d.child(Check::new("gm-pick-all", all_on).label(root.tr("gp.pickAll")).on_toggle({
                let this = cx.entity().downgrade();
                move |on, _, cx| {
                    let all = all.clone();
                    let on = *on;
                    let _ = this.update(cx, |r, cx| {
                        if let Some((_, t)) = r.games.picking.as_mut() {
                            *t = if on { all.into_iter().collect() } else { HashSet::new() };
                        }
                        cx.notify();
                    });
                }
            }))
        })
        // (Opened from the Servers tab: which server it joins.)
        .when_some(root.games.pick_job.clone(), |d, job| {
            d.child(div().flex().items_center().gap(px(6.)).text_size(px(T_S)).text_color(c(MUTED)).font_features(tnum()).child(icon("server", 13., c(MUTED))).child(job.chars().take(8).collect::<String>()))
        })
        .child(Button::new("gm-pick-cancel", root.tr("gp.cancel")).quiet().on_click(cx.listener(|r, _, _, cx| {
            r.games.picking = None;
            r.games.pick_job = None;
            cx.notify();
        })))
        .when(!ids.is_empty(), |d| {
            let go: Vec<String> = ids.iter().filter(|i| ticked.contains(*i)).cloned().collect();
            d.child(Button::new("gm-join", root.i18n.tv("gp.joinN", &[("n", &n.to_string())])).icon("play").primary().disabled(n == 0).on_click(cx.listener(
                move |r, _, _, cx| {
                    if go.is_empty() {
                        return;
                    }
                    r.games.picking = None;
                    let link = format!("https://www.roblox.com/games/{place}");
                    match r.games.pick_job.take() {
                        Some(job) => r.launch("launch", json!({ "ids": go, "appOnly": false, "placeInput": link, "jobId": job }), false, cx),
                        None => r.play_at(&go, link, cx),
                    }
                    cx.notify();
                },
            )))
        });
    let body = if !ids.is_empty() {
        chips.into_any_element()
    } else if root.accounts().is_empty() {
        div()
            .flex()
            .items_center()
            .gap(px(12.))
            .text_size(px(T_M))
            .text_color(c(MUTED))
            .child(root.tr("gp.noAccounts"))
            .child(Button::new("gm-add", root.tr("gp.addAccount")).icon("plus").small().on_click(cx.listener(|r, _, _, cx| r.add_account(cx))))
            .into_any_element()
    } else {
        div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("gp.allBusy")).into_any_element()
    };
    surface().p(px(18.)).flex().flex_col().gap(px(14.)).child(head).child(body)
}

fn tab_button(t: Tab, icon_name: &'static str, label: String, on: bool) -> Hot {
    let id = match t {
        Tab::Overview => "gt-overview",
        Tab::News => "gt-news",
        Tab::Prices => "gt-prices",
        Tab::Passes => "gt-passes",
        Tab::Servers => "gt-servers",
        Tab::Wiki => "gt-wiki",
    };
    chip(id, on).h(px(TOOL_H)).px(px(12.)).child(icon(icon_name, 15., if on { c(INK) } else { c(MUTED) })).child(label)
}

/// The game on its own picture (as Home's stage): back to all games, its icon and name, who makes it, and what to do.
#[allow(clippy::too_many_arguments)]
fn hero(root: &Root, game: Option<&Game>, details: Option<&Value>, open: &Open, name: &str, place: i64, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    // (Its height follows the width, as Home's picture does: at 220 it was a strip of a 16:9 picture on a wide
    // window.)
    // On the other tabs (a wiki table, the values, the servers) the page under it is what is read: the picture keeps
    // to a band, so the first screen shows more than a row of it.
    // One height on every tab (review 3b.3: the title jumped from one place to another between tabs), low enough that
    // the Overview's figures, tabs and your accounts come on the first screen (3b.1).
    let h = ((width * 0.12).clamp(150., 200.) / 2.).round() * 2.;
    let cover = look(root, place, "cover").and_then(|u| images::pic(&u, cx));
    let badge = look(root, place, "icon")
        .or_else(|| game.and_then(|g| g.icon.clone()))
        .or_else(|| details.map(|d| s(d, "iconUrl").to_string()).filter(|u| !u.is_empty()));
    // While the wide picture isn't there (not loaded yet, or the game has none): the game's own colour from its icon,
    // not a blank dark band.
    let ground = if cover.is_none() { badge.as_deref().and_then(|u| images::tone(u, cx)) } else { None };
    let mut facts: Vec<String> = Vec::new();
    if let Some(d) = details {
        facts.extend([s(d, "creator"), s(d, "genre")].into_iter().filter(|x| !x.is_empty()).map(str::to_string));
    }
    if let Some(g) = game.filter(|g| g.playing > 0) {
        facts.push(root.i18n.tv("cm.playingN", &[("n", &g.playing.to_string())]));
    }
    let mut line = div().flex().items_center().gap(px(8.)).text_size(px(T_M)).text_color(on_picture(0.86));
    for (i, f) in facts.into_iter().enumerate() {
        if i > 0 {
            line = line.child(sep());
        }
        line = line.child(f);
    }

    let news = game.and_then(|g| g.news.clone());
    let follow_on = news.as_ref().and_then(|n| n.get("followed")).and_then(Value::as_bool).unwrap_or(false);
    let game_id = open.id.clone();
    let wiki = game.and_then(|g| g.wiki.clone());
    let wiki_name = name.to_string();
    let mut acts = div().flex().flex_none().items_center().gap(px(8.));
    if place > 0 {
        let (id, picking) = (open.id.clone(), root.games.picking.as_ref().is_some_and(|(g, _)| *g == open.id));
        acts = acts.child(Button::new("gm-play", root.tr("gp.play")).icon("play").primary().on_click(cx.listener(move |r, _, _, cx| {
            // The accounts picked on the Accounts page start ticked, when they are free.
            let picked: HashSet<String> = r
                .accounts()
                .iter()
                .filter(|a| crate::pages::accounts::can_play(a))
                .filter_map(|a| a.get("id").and_then(Value::as_str))
                .filter(|id| r.selected.contains(*id))
                .map(str::to_string)
                .collect();
            r.games.picking = if picking { None } else { Some((id.clone(), picked)) };
            r.games.pick_job = None;
            cx.notify();
        })));
    }
    if news.is_some() {
        let b = Button::new("gm-follow", root.tr(if follow_on { "news.following" } else { "news.follow" })).icon(if follow_on { "check" } else { "plus" });
        acts = acts.child(if follow_on || place > 0 { b.glass() } else { b.primary() }.on_click(cx.listener(move |r, _, _, _| {
            r.send("feedFollow", json!({ "game": game_id, "on": !follow_on }))
        })));
    }
    // A wiki Bloxkeep knows opens here, on its tab; another is looked for in the browser.
    acts = acts.child(Button::new("gm-wiki", root.tr("gp.wiki")).icon("book").glass().on_click(cx.listener(move |r, _, _, cx| match &wiki {
        Some(_) => {
            r.games.tab = Tab::Wiki;
            cx.notify();
        }
        None => r.send("openGameWiki", json!({ "name": wiki_name })),
    })));
    if place > 0 {
        acts = acts.child(IconButton::new("gm-roblox", "external").size(36.).glass().tip(root.tr("gp.roblox")).on_click(cx.listener(move |r, _, _, _| {
            r.send("openGamePage", json!({ "placeId": place }))
        })));
    }

    div()
        .relative()
        .h(px(h))
        .flex_none()
        .overflow_hidden()
        .when_some(ground, |d, t| d.bg(ca(t, 0.55)))
        // (The picture stops 4 px above the foot, which is the surface's own colour anyway: drawn to the edge, its
        // last row still peeked out as a dashed hairline under the strip below, review 3b.1.)
        .when_some(cover, |d, p| d.child(div().absolute().top_0().left_0().right_0().bottom(px(4.)).overflow_hidden().child(fade_in(img(p.clone()).size_full().object_fit(ObjectFit::Cover), &p))))
        // As on Home's stage: the picture runs into the surface below, and darkens towards the words.
        // (Darkened only where the words stand, and only its foot runs into the surface: as Home's stage, whose
        // picture the owner found hidden under heavier fades, 2026-11-11.)
        .children((0..2).map(|_| div().absolute().inset_0().bg(linear_gradient(180., linear_color_stop(ca(PANEL, 0.), 0.5), linear_color_stop(c(PANEL), 1.)))))
        .children((0..2).map(|_| div().absolute().inset_0().bg(linear_gradient(90., linear_color_stop(ca(PANEL, 0.75), 0.), linear_color_stop(ca(PANEL, 0.), 0.5)))))
        // (Its last two pixels the surface's own colour: at a height that isn't a whole number of screen pixels the
        // picture showed as a bright hairline under the fade.)
        .child(div().absolute().left_0().right_0().bottom_0().h(px(6.)).bg(c(PANEL)))
        .child(
            div().absolute().top(px(14.)).left(px(PAD)).child(
                Button::new("gm-back", root.tr("news.title")).icon("chevronLeft").glass().small().on_click(cx.listener(|r, _, _, cx| {
                    r.games.open = None;
                    cx.notify();
                })),
            ),
        )
        .child(
            div()
                .absolute()
                .inset_0()
                .px(px(PAD + 12.))
                .pb(px(4.))
                .flex()
                .items_end()
                .gap(px(16.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(10.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(14.))
                                .child(game_icon(badge, name, 52., cx).border_1().border_color(ca(WHITE, 0.14)))
                                .child(div().min_w_0().text_size(px(if width < 900. { 30. } else { 36. })).font_weight(FontWeight::BOLD).line_height(px(44.)).truncate().child(name.to_string())),
                        )
                        .child(line),
                )
                .child(acts),
        )
}

pub fn thousands(n: i64, vi: bool) -> String {
    let digits = n.abs().to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(if vi { '.' } else { ',' });
        }
        out.push(ch);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// 48.2 billion, 8.7 million, 412,873 (in the app's words for billion and million).
fn big(root: &Root, n: f64) -> String {
    let vi = root.i18n.vietnamese;
    let one = |x: f64| {
        let t = format!("{:.1}", x);
        let t = t.trim_end_matches(".0").to_string();
        if vi { t.replace('.', ",") } else { t }
    };
    if n >= 1e9 {
        format!("{} {}", one(n / 1e9), root.tr("gp.billion"))
    } else if n >= 1e6 {
        format!("{} {}", one(n / 1e6), root.tr("gp.million"))
    } else {
        thousands(n.round() as i64, vi)
    }
}

fn ago_iso(root: &Root, iso: &str) -> Option<String> {
    crate::time::iso_secs(iso).map(|t| crate::time::ago(crate::time::now_secs() - t, &root.i18n))
}

/// What Roblox says of it in one strip, as Roblox's About lists its figures: how many play now and in all, how many
/// keep it as a favourite, how many like it (the share drawn, thumbs up and down under it) and when it was last
/// updated. Roblox gives the play counts only to a signed-in ask: a figure not given is left out, not shown as 0.
fn facts(root: &Root, d: &Value) -> impl IntoElement {
    let f = |k: &str| d.get(k).and_then(Value::as_f64).unwrap_or(0.);
    let vi = root.i18n.vietnamese;
    let mut cells: Vec<(String, String, Option<AnyElement>)> = Vec::new();
    if f("playing") > 0. {
        cells.push((root.tr("gp.playing"), thousands(f("playing") as i64, vi), None));
    }
    if f("visits") > 0. {
        cells.push((root.tr("gp.visits"), big(root, f("visits")), None));
    }
    if f("favorites") > 0. {
        cells.push((root.tr("gp.favorites"), big(root, f("favorites")), None));
    }
    if let Some(l) = d.get("likes").and_then(Value::as_f64) {
        let (up, down) = (f("upVotes"), f("downVotes"));
        let bar = (up + down > 0.).then(|| {
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(div().h(px(4.)).w_full().rounded_full().bg(ca(WHITE, 0.1)).child(div().h_full().w(relative((up / (up + down)) as f32)).rounded_full().bg(c(OK))))
                .child(div().text_size(px(T_S)).text_color(c(MUTED)).font_features(tnum()).truncate().child(root.i18n.tv("gp.votes", &[("up", &big(root, up)), ("down", &big(root, down))])))
                .into_any_element()
        });
        cells.push((root.tr("gp.likes"), format!("{l:.0}%"), bar));
    }
    if let Some(u) = d.get("updated").and_then(Value::as_str).and_then(|u| ago_iso(root, u)) {
        cells.push((root.tr("gp.updated"), u, None));
    }
    div().flex().flex_wrap().rounded(px(R_L)).bg(ca(WHITE, 0.03)).border_1().border_color(c(LINE)).children(cells.into_iter().enumerate().map(|(i, (label, value, under))| {
        // (The likes take a little more room, for their bar.)
        let wide = under.is_some();
        div()
            .flex_1()
            .min_w(px(if wide { 220. } else { 150. }))
            .px(px(18.))
            .py(px(13.))
            .flex()
            .flex_col()
            .justify_center()
            .gap(px(4.))
            .when(i > 0, |d| d.border_l_1().border_color(c(LINE)))
            .child(div().text_size(px(T_S)).font_weight(FontWeight::MEDIUM).text_color(c(MUTED)).truncate().child(label))
            .child(div().text_size(px(T_FIG)).font_weight(FontWeight::SEMIBOLD).line_height(px(28.)).font_features(tnum()).truncate().child(value))
            .children(under)
    }))
}

/// 2019-01-16T03:29:19Z as a day: 16/01/2019, or Jan 16, 2019.
fn day_of(root: &Root, iso: &str) -> Option<String> {
    let (y, m, d) = (iso.get(0..4)?, iso.get(5..7)?.parse::<usize>().ok()?, iso.get(8..10)?);
    if !(1..=12).contains(&m) || y.starts_with("000") {
        return None;
    }
    Some(if root.i18n.vietnamese {
        format!("{d}/{m:02}/{y}")
    } else {
        const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
        format!("{} {}, {y}", MONTHS[m - 1], d.trim_start_matches('0'))
    })
}

/// The game's details, label and value a row (as Roblox's About lists them): who makes it, its genre, when it came
/// out, players a server and its Place ID. Only what Roblox said.
fn info(root: &Root, d: &Value, place: i64) -> Div {
    let mut rows: Vec<(String, String)> = Vec::new();
    for (key, value) in [("gp.creator", s(d, "creator").to_string()), ("gp.genre", s(d, "genre").to_string())] {
        if !value.is_empty() {
            rows.push((root.tr(key), value));
        }
    }
    if let Some(day) = d.get("created").and_then(Value::as_str).and_then(|t| day_of(root, t)) {
        rows.push((root.tr("gp.created"), day));
    }
    if int(d, "maxPlayers") > 0 {
        rows.push((root.tr("gp.maxPlayers"), int(d, "maxPlayers").to_string()));
    }
    if place > 0 {
        rows.push((root.tr("gp.placeId"), place.to_string()));
    }
    surface().p(px(18.)).flex().flex_col().gap(px(6.)).child(heading(root.tr("gp.info"), None)).child(div().flex().flex_col().children(rows.into_iter().enumerate().map(|(i, (k, v))| {
        div()
            .h(px(38.))
            .flex()
            .items_center()
            .justify_between()
            .gap(px(14.))
            .text_size(px(T_M))
            .when(i > 0, |d| d.border_t_1().border_color(ca(WHITE, 0.05)))
            .child(div().flex_none().text_color(c(MUTED)).child(k))
            .child(div().min_w_0().truncate().font_weight(FontWeight::MEDIUM).font_features(tnum()).child(v))
    })))
}

/// More of Roblox's games under the page (the chart the Game page shows, this game left out): one row of them, when
/// that chart is in.
fn more_games(root: &Root, place: i64, width: f32, cx: &mut Context<Root>) -> Option<AnyElement> {
    let sort = shown_sort(root)?;
    let list = root.games.rows.get(&sort.id)?.as_ref().ok()?;
    let (cols, w, art_h, tile_h) = board_fit(width);
    let games: Vec<&roblox::Game> = list.iter().filter(|g| g.place != place).take(cols).collect();
    if games.is_empty() {
        return None;
    }
    let key = format!("rx.{}", sort.id);
    let chart = if root.i18n.has(&key) { root.tr(&key) } else { sort.name.clone() };
    Some(
        div()
            .pt(px(10.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(heading(root.tr("gp.trending"), Some(chart)))
            .child(div().mx(px(-TPAD)).flex().gap(px(TGAP)).children(games.into_iter().enumerate().map(|(i, g)| tile(root, 1000 + i, g, false, w, art_h, tile_h, cx))))
            .into_any_element(),
    )
}

/// A game updated in the last day: Roblox may be shutting old servers down.
fn fresh(root: &Root, d: &Value) -> Option<String> {
    let iso = d.get("updated").and_then(Value::as_str)?;
    let t = crate::time::iso_secs(iso)?;
    let age = crate::time::now_secs() - t;
    if !(0..24 * 3600).contains(&age) {
        return None;
    }
    // The hour as the PC's clock says it: the offset the app's own times carry is UTC, so the PC's is asked of Windows.
    let local = t + crate::time::utc_offset_secs();
    let time = format!("{:02}:{:02}", local.rem_euclid(86400) / 3600, local.rem_euclid(3600) / 60);
    Some(root.i18n.tv("gp.fresh", &[("time", &time), ("ago", &crate::time::ago(age, &root.i18n))]))
}

fn heading(title: String, sub: Option<String>) -> impl IntoElement {
    // A long note goes under the title, wrapping in the card ("When it is quiet" in its narrow column ran off the
    // card's edge on one line).
    let under = sub.as_ref().is_some_and(|s| s.chars().count() > 36);
    div()
        .flex()
        .map(|d| if under { d.flex_col().gap(px(3.)) } else { d.items_baseline().gap(px(10.)) })
        .child(div().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).child(title))
        .when_some(sub, |d, s| d.child(div().min_w_0().text_size(px(T_S)).text_color(c(MUTED)).child(s)))
}

/// Your accounts in it over 7 days, the hours it is quiet, and what it is.
fn overview(root: &Root, details: Option<&Value>, place: i64, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let Some(d) = details else {
        let key = if place > 0 { "news.connecting" } else { "gp.noneYet" };
        return div().pt(px(8.)).text_size(px(T_M)).text_color(c(MUTED)).child(root.tr(key));
    };
    let narrow = width < 760.;
    let accounts: Vec<Value> = root.games.stays.get(&place).cloned().unwrap_or_else(|| d.get("accounts").and_then(Value::as_array).cloned().unwrap_or_default());
    let mut table = div().flex().flex_col();
    let row = |cells: Vec<AnyElement>, head: bool| {
        let mut r = div()
            .h(px(if head { 30. } else { 44. }))
            .flex()
            .items_center()
            .gap(px(12.))
            .px(px(4.))
            .text_size(px(if head { T_S } else { T_M }))
            .when(head, |d| d.text_color(c(MUTED)).font_weight(FontWeight::MEDIUM))
            .when(!head, |d| d.border_t_1().border_color(ca(WHITE, 0.05)));
        for (i, cell) in cells.into_iter().enumerate() {
            r = r.child(match i {
                0 => div().flex_1().min_w_0().truncate().child(cell),
                4 => div().w(px(if narrow { 96. } else { 130. })).flex_none().truncate().child(cell),
                _ => div().w(px(if narrow { 64. } else { 96. })).flex_none().truncate().font_features(tnum()).child(cell),
            });
        }
        r
    };
    let txt = |t: String| div().child(t).into_any_element();
    table = table.child(row(
        vec![txt(root.tr("gp.account")), txt(root.tr("gp.played")), txt(root.tr("gp.kicks")), txt(root.tr("gp.drops")), txt(root.tr("gp.now"))],
        true,
    ));
    for a in &accounts {
        let minutes = int(a, "minutes");
        let played = if minutes >= 60 {
            root.i18n.tv("gp.hours", &[("n", &((minutes as f64 / 60.).round() as i64).to_string())])
        } else {
            root.i18n.tv("gp.minutes", &[("n", &minutes.to_string())])
        };
        let kicks = int(a, "kicks");
        let why = s(a, "kickWhy");
        let kicks = if kicks > 0 && !why.is_empty() { format!("{kicks} · {why}") } else { kicks.to_string() };
        let now = match s(a, "now") {
            "ingame" => div().flex().items_center().gap(px(7.)).child(div().size(px(7.)).rounded_full().bg(c(OK))).child(root.tr("gp.inGame")),
            "banned" => div().text_color(c(BAD)).child(root.tr("gp.banned")),
            _ => div().text_color(c(MUTED)).child(
                a.get("lastMs")
                    .and_then(Value::as_i64)
                    .map(|ms| crate::time::ago(crate::time::now_secs() - ms / 1000, &root.i18n))
                    .or_else(|| a.get("lastPlayed").and_then(Value::as_str).and_then(|t| ago_iso(root, t)))
                    .unwrap_or_else(|| "–".into()),
            ),
        };
        // (Its face, when the account is still in the app.)
        let face = root.accounts().iter().find(|x| s(x, "id") == s(a, "id")).map(|x| crate::pages::accounts::avatar(x, 26., PANEL, cx));
        table = table.child(row(
            vec![
                div().flex().items_center().gap(px(10.)).children(face).child(div().min_w_0().truncate().font_weight(FontWeight::MEDIUM).child(s(a, "name").to_string())).into_any_element(),
                txt(played),
                txt(kicks),
                txt(int(a, "drops").to_string()),
                now.into_any_element(),
            ],
            false,
        ));
    }

    let hours: Option<Vec<Option<f64>>> = d
        .get("hours")
        .and_then(Value::as_array)
        .map(|l| l.iter().map(Value::as_f64).collect::<Vec<_>>())
        .filter(|l| l.len() == 24 && l.iter().filter(|h| h.is_some()).count() >= 12);
    let about = s(d, "description").trim().to_string();
    let open_about = root.games.about;
    // (Folded at five lines when it is longer.)
    let long = about.lines().count() > 5 || about.chars().count() > 460;
    // (Cut by hand: a clamp counts wrapped lines, not the description's own line breaks.)
    let shown = if long && !open_about {
        let head: Vec<&str> = about.lines().take(5).collect();
        let head = head.join("\n");
        let head: String = head.chars().take(460).collect();
        format!("{}…", head.trim_end())
    } else {
        about.clone()
    };
    let words = (!about.is_empty()).then(|| {
        surface()
            .p(px(18.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(heading(root.tr("gp.aboutTitle"), None))
            .child(div().max_w(px(780.)).text_size(px(T_M)).line_height(px(21.)).text_color(c(TEXT_2)).child(shown))
            .when(long, |d| {
                d.child(
                    div().flex().child(
                        Button::new("gp-about", root.tr(if open_about { "gp.less" } else { "gp.more" }))
                            .icon_end(if open_about { "chevronUp" } else { "chevronDown" })
                            .quiet()
                            .small()
                            .on_click(cx.listener(|r, _, _, cx| {
                                r.games.about = !r.games.about;
                                cx.notify();
                            })),
                    ),
                )
            })
    });

    let yours = surface().p(px(18.)).flex().flex_col().gap(px(12.)).child(heading(root.tr("gp.yours"), Some(root.tr("gp.week")))).child(if accounts.is_empty() {
        div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("gp.noneYet"))
    } else {
        table
    });
    let hush = surface().p(px(18.)).flex().flex_col().gap(px(16.)).child(heading(root.tr("gp.quiet"), Some(root.tr("gp.quietFrom")))).child(match hours {
        Some(h) => quiet(root, &h).into_any_element(),
        None => div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("gp.quietNotYet")).into_any_element(),
    });
    let details = info(root, d, place);
    // Wide: the pictures, the words and your accounts in the main column; its details and quiet hours beside it.
    let beside = width >= 1100.;
    let side = 380.;
    let main_w = if beside { width - side - GAP } else { width };
    // Your accounts in it first, under the alert: what this page is mostly for (C#'s Game info), it was two scrolls
    // down under the pictures (review 3b.1).
    let main = div()
        .flex()
        .flex_col()
        .gap(px(GAP))
        .children(radar_strip(root, place, s(d, "name")))
        .child(yours)
        .children(gallery(root, place, main_w, cx))
        .children(words);
    div()
        .flex()
        .flex_col()
        .gap(px(GAP))
        .pt(px(4.))
        .child(if beside {
            div()
                .flex()
                .items_start()
                .gap(px(GAP))
                .child(main.w(px(main_w)).flex_none())
                .child(div().w(px(side)).flex_none().flex().flex_col().gap(px(GAP)).child(details).child(hush))
        } else {
            main.child(details).child(hush)
        })
        .children(more_games(root, place, width, cx))
}

/// The three hours in a row with the fewest players (past midnight too): from, to.
fn quietest(h: &[Option<f64>]) -> Option<(usize, usize)> {
    let mut best = f64::INFINITY;
    let mut at = None;
    for i in 0..24 {
        let span: Option<Vec<f64>> = (0..3).map(|k| h[(i + k) % 24]).collect();
        if let Some(span) = span {
            let sum: f64 = span.iter().sum();
            if sum < best {
                best = sum;
                at = Some(i);
            }
        }
    }
    at.map(|i| (i, (i + 3) % 24))
}

/// Players by hour of the day as Bloxkeep saw them: a bar an hour, the quiet three hours in green.
fn quiet(root: &Root, h: &[Option<f64>]) -> impl IntoElement {
    let peak = h.iter().flatten().copied().fold(0., f64::max).max(1.);
    let span = quietest(h);
    let in_span = |i: usize| span.is_some_and(|(from, _)| (0..3).any(|k| (from + k) % 24 == i));
    let bars: Vec<AnyElement> = h
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let part = v.map(|v| (v / peak) as f32).unwrap_or(0.);
            div()
                .flex_1()
                .h_full()
                .flex()
                .flex_col()
                .justify_end()
                .child(grow_in("quiet-bar", i, part.max(0.04), div().rounded_t(px(3.)).bg(if v.is_none() { ca(WHITE, 0.05) } else if in_span(i) { c(OK) } else { ca(TEXT, 0.16 + 0.5 * part) })))
                .into_any_element()
        })
        .collect();
    let axis = div()
        .flex()
        .justify_between()
        .text_size(px(T_S))
        .text_color(c(MUTED))
        .font_features(tnum())
        .children(["0h", "6h", "12h", "18h", "23h"].map(|t| div().child(t)));
    // The quietest and the busiest hour, each with its count, under its own bar (the web page's marks, review 3b.2).
    let seen: Vec<(usize, f64)> = h.iter().enumerate().filter_map(|(i, v)| v.map(|v| (i, v))).collect();
    let low = seen.iter().copied().min_by(|a, b| a.1.total_cmp(&b.1));
    let high = seen.iter().copied().max_by(|a, b| a.1.total_cmp(&b.1));
    let marks = low.zip(high).filter(|(l, h)| l.0 != h.0).map(|(l, hi)| {
        let mark = |(i, v): (usize, f64), key: &str, tone: Hsla| {
            let at = (i as f32 + 0.5) / 24.;
            let words = div()
                .flex()
                .items_center()
                .gap(px(4.))
                .whitespace_nowrap()
                .child(div().font_weight(FontWeight::SEMIBOLD).text_color(tone).child(root.tr(key)))
                .child(thousands(v.round() as i64, root.i18n.vietnamese));
            // (Near an end it keeps inside the chart; elsewhere centred on its bar.)
            let place = div().absolute().top_0().flex();
            if i < 3 {
                place.left(relative(at)).ml(px(-6.)).child(words)
            } else if i > 20 {
                place.right(relative(1. - at)).mr(px(-6.)).child(words)
            } else {
                place.left(relative(at)).ml(px(-80.)).w(px(160.)).justify_center().child(words)
            }
        };
        div().relative().h(px(18.)).text_size(px(T_S)).text_color(c(TEXT_2)).font_features(tnum()).child(mark(l, "gp.quietEnd", c(OK))).child(mark(hi, "gp.busyEnd", c(TEXT)))
    });
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(div().h(px(110.)).flex().items_end().gap(px(4.)).children(bars))
        .children(marks)
        .child(axis)
        .when_some(span, |d, (from, to)| {
            d.child(div().mt(px(4.)).text_size(px(T_M)).text_color(c(TEXT_2)).child(root.i18n.tv("gp.quietSpan", &[("from", &from.to_string()), ("to", &to.to_string())])))
        })
}

// ───────────────────────── news ─────────────────────────

/// Seconds from now to a time, in words: "in 5 min" ahead, "5 minutes ago" past.
fn rel(root: &Root, t: i64) -> String {
    let d = t - crate::time::now_secs();
    if d <= 0 {
        return crate::time::ago(-d, &root.i18n);
    }
    if d < 3600 {
        root.i18n.tv("ui.inMin", &[("n", &(d / 60).max(1).to_string())])
    } else {
        root.i18n.tv("ui.inHour", &[("n", &(d / 3600).to_string())])
    }
}

/// Discord's words as plain text: the times said in words, the servers' emoji and the marks left out, a masked link
/// its words. (The pictures of the emoji in a field's name come in front of it: emoji_of.)
fn plain(root: &Root, text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find(['<', '[', '*', '_', '~', '|', '`']) {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        if let Some(t) = tail.strip_prefix("<t:") {
            if let Some(end) = t.find('>') {
                let secs = t[..end].split(':').next().and_then(|n| n.parse::<i64>().ok());
                if let Some(secs) = secs {
                    out.push_str(&rel(root, secs));
                    rest = &t[end + 1..];
                    continue;
                }
            }
        }
        if tail.starts_with("<:") || tail.starts_with("<a:") {
            if let Some(end) = tail.find('>') {
                rest = &tail[end + 1..];
                continue;
            }
        }
        if tail.starts_with('[') {
            if let (Some(close), true) = (tail.find("]("), true) {
                if let Some(end) = tail[close..].find(')') {
                    out.push_str(&tail[1..close]);
                    rest = &tail[close + end + 1..];
                    continue;
                }
            }
        }
        let ch = tail.chars().next().unwrap_or_default();
        if matches!(ch, '*' | '_' | '~' | '|' | '`') {
            // Marks come doubled ("**", "__", "~~", "||"); one alone is a mark too, except a "_" inside a word.
            let n = tail.chars().take_while(|c| *c == ch).count();
            let inside = ch == '_' && out.chars().last().is_some_and(char::is_alphanumeric);
            if inside {
                out.push_str(&tail[..n]);
            }
            rest = &tail[n..];
            continue;
        }
        out.push(ch);
        rest = &tail[ch.len_utf8()..];
    }
    out.push_str(rest);
    out.lines()
        .map(|l| {
            let l = l.trim_end();
            let l = l.strip_prefix("-# ").unwrap_or(l);
            match l.strip_prefix("- ") {
                Some(item) => format!("•  {item}"),
                None => l.trim_start_matches('>').trim_start().to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The first of a server's emoji in some words, as a picture's address on Discord's servers.
fn emoji_of(text: &str) -> Option<String> {
    let i = text.find("<:").or_else(|| text.find("<a:"))?;
    let t = &text[i + 1..];
    let end = t.find('>')?;
    let id = t[..end].rsplit(':').next()?;
    id.chars().all(|c| c.is_ascii_digit()).then(|| format!("https://cdn.discordapp.com/emojis/{id}.png?size=32"))
}

fn news(root: &Root, feed: Value, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let id = s(&feed, "id").to_string();
    let Some(cards) = &root.games.cards else {
        return div().pt(px(8.)).text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("news.connecting"));
    };
    // The game's sources (its Discord channels, its site…): with more than one, a row to show one of them alone.
    let sources: Vec<(String, String)> = feed.get("sources").and_then(Value::as_array).map(|l| l.iter().map(|v| (s(v, "id").to_string(), s(v, "title").to_string())).filter(|(i, _)| !i.is_empty()).collect()).unwrap_or_default();
    let source = root.games.source.clone().filter(|now| sources.len() > 1 && sources.iter().any(|(i, _)| i == now));
    let mut list: Vec<&Value> = cards.iter().filter(|c| s(c, "game") == id).filter(|c| source.as_deref().is_none_or(|src| s(c, "src") == src)).collect();
    list.sort_by(|a, b| b.get("at").and_then(Value::as_f64).unwrap_or(0.).total_cmp(&a.get("at").and_then(Value::as_f64).unwrap_or(0.)));
    let mut tell = div().flex().flex_wrap().items_center().gap(px(16.)).child(alerts(root, &feed, cx));
    if sources.len() > 1 {
        let mut seg = div().flex().flex_wrap().gap(px(2.)).p(px(3.)).rounded(px(R_M)).bg(c(RAISED));
        let all: Vec<(Option<String>, String)> = std::iter::once((None, root.tr("news.all"))).chain(sources.iter().map(|(i, t)| (Some(i.clone()), t.clone()))).collect();
        for (n, (key, title)) in all.into_iter().enumerate() {
            let on = key == source;
            seg = seg.child(segment(("news-src", n), on).h(px(26.)).px(px(10.)).text_size(px(T_S + 0.5)).cursor_pointer().child(title).on_click(cx.listener(move |r, _, _, cx| {
                r.games.source = key.clone();
                cx.notify();
            })));
        }
        tell = tell.child(seg);
    }
    if list.is_empty() {
        return div()
            .pt(px(8.))
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(div().pb(px(10.)).child(tell))
            .child(div().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).child(root.tr("news.nothingYet")))
            .child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("news.nothingYetText")));
    }
    let before = root.games.mark.as_ref().filter(|(g, _)| *g == id).map(|(_, r)| *r).unwrap_or(f64::INFINITY);
    // As many columns as fit (a message reads best about 560 wide), the messages dealt across them in turn, so the
    // newest stand along the top and a wide window isn't half empty.
    let cols = ((width + GAP) / (520. + GAP)).floor().clamp(1., 3.) as usize;
    let w = ((width - GAP * (cols as f32 - 1.)) / cols as f32).floor();
    let mut columns: Vec<Vec<AnyElement>> = (0..cols).map(|_| Vec::new()).collect();
    for (i, card) in list.into_iter().enumerate() {
        columns[i % cols].push(news_card(root, card, before, w, cx).into_any_element());
    }
    div()
        .pt(px(4.))
        .flex()
        .flex_col()
        .gap(px(GAP))
        .child(tell)
        .child(div().flex().items_start().gap(px(GAP)).children(columns.into_iter().map(|col| div().w(px(w)).flex_none().flex().flex_col().gap(px(GAP)).children(col))))
}

/// One message from the game's bot: who sent it and when, its title and words, its fields in columns, its picture.
fn news_card(root: &Root, card: &Value, before: f64, w: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let at = card.get("at").and_then(Value::as_f64).unwrap_or(0.);
    let is_new = before.is_finite() && at > before;
    let author = card.pointer("/author/name").and_then(Value::as_str).unwrap_or_default().to_string();
    let face = card.pointer("/author/icon").and_then(Value::as_str).filter(|u| u.starts_with("http")).and_then(|u| images::pic_sq(u, 18., cx));
    let title = plain(root, s(card, "title"));
    let card_id = s(card, "id").to_string();
    let parts: Option<Vec<Value>> = card.get("blocks").and_then(Value::as_array).filter(|l| !l.is_empty()).cloned();
    let blocks = parts.as_ref().map(|l| super::blocks::blocks(root, &card_id, l, card.get("spoiler").and_then(Value::as_bool).unwrap_or(false), w - 34., cx));
    let text = if blocks.is_some() { String::new() } else { plain(root, s(card, "text")) };
    let until = card.get("until").and_then(Value::as_f64).map(|u| (u / 1000.) as i64).filter(|u| *u > crate::time::now_secs());
    let picture = card.get("image").and_then(Value::as_str).filter(|u| u.starts_with("http") && parts.is_none()).and_then(|u| images::pic(u, cx));
    let link = card.get("link").and_then(Value::as_str).filter(|u| u.starts_with("http")).map(str::to_string);
    let fields: Vec<(Option<String>, String, String)> = card
        .get("fields")
        .and_then(Value::as_array)
        .filter(|_| parts.is_none())
        .into_iter()
        .flatten()
        .map(|f| (emoji_of(s(f, "name")), plain(root, s(f, "name")).trim().to_string(), plain(root, s(f, "value")).trim().to_string()))
        .collect();
    let cols = if w > 560. { 3. } else { 2. };
    let field_w = ((w - 32. - 16. * (cols - 1.)) / cols).floor();
    div()
        .id(SharedString::from(format!("nc-{card_id}")))
        .w(px(w))
        .px(px(16.))
        .py(px(14.))
        .flex()
        .flex_col()
        .gap(px(10.))
        .rounded(px(R_L))
        .bg(ca(WHITE, 0.045))
        .border_1()
        .border_color(if is_new { ca(TEXT, 0.25) } else { c(LINE) })
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .text_size(px(T_S))
                .text_color(c(MUTED))
                .when_some(face, |d, f| d.child(img(f).size(px(18.)).rounded_full()))
                .child(div().min_w_0().truncate().font_weight(FontWeight::MEDIUM).text_color(c(TEXT_2)).child(author))
                .child(div().flex_1())
                .when(is_new, |d| d.child(div().text_color(c(TEXT)).font_weight(FontWeight::SEMIBOLD).child(root.tr("news.new"))))
                .when_some(until, |d, u| d.child(div().text_color(c(TEXT_2)).font_features(tnum()).child(root.i18n.tv("news.left", &[("t", &rel_bare(root, u))]))))
                .child(div().font_features(tnum()).child(rel(root, (at / 1000.) as i64))),
        )
        .when(!title.is_empty(), |d| {
            // With a link the title opens it, as the old page's did (the card's blocks carry their own buttons).
            let head = div().id(SharedString::from(format!("nc-title-{card_id}"))).text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).child(title);
            d.child(match link.clone() {
                Some(url) => head
                    .cursor_pointer()
                    .hover(|s| s.underline())
                    .on_click(cx.listener(move |r, _, _, _| r.send("feedOpen", json!({ "url": url }))))
                    .into_any_element(),
                None => head.into_any_element(),
            })
        })
        // Drawn as Discord drew it when the bot sent its blocks; else the words, fields and picture of before.
        .when_some(blocks, |d, b| d.child(b))
        .when(!text.trim().is_empty(), |d| d.child(div().text_size(px(T_M)).line_height(px(19.)).text_color(c(TEXT_2)).child(text)))
        .when(!fields.is_empty(), |d| {
            d.child(div().flex().flex_wrap().gap_x(px(16.)).gap_y(px(10.)).children(fields.into_iter().map(|(emoji, name, value)| {
                let pic = emoji.as_deref().and_then(|u| images::pic_sq(u, 16., cx));
                div()
                    .w(px(field_w))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_size(px(T_S))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c(MUTED))
                            .when_some(pic, |d, p| d.child(img(p).size(px(16.))))
                            .child(div().min_w_0().truncate().child(name)),
                    )
                    .child(div().text_size(px(T_M)).font_features(tnum()).child(value))
                    .into_any_element()
            })))
        })
        .when_some(picture, |d, p| d.child(div().max_h(px(260.)).rounded(px(R_M)).overflow_hidden().child(img(p).w_full().object_fit(ObjectFit::Cover))))
        .when_some(link.filter(|_| parts.is_none()), |d, url| {
            d.child(div().flex().child(Button::new(SharedString::from(format!("nc-open-{card_id}")), root.tr("ui.openLink")).icon("external").quiet().small().on_click(
                cx.listener(move |r, _, _, _| r.send("feedOpen", json!({ "url": url }))),
            )))
        })
}

/// How long until a time, without "in": 5 min, 2 h, 0:42.
fn rel_bare(_root: &Root, t: i64) -> String {
    let d = (t - crate::time::now_secs()).max(0);
    if d < 3600 { format!("{}:{:02}", d / 60, d % 60) } else { format!("{}:{:02}:{:02}", d / 3600, d / 60 % 60, d % 60) }
}

// ───────────────────────── prices ─────────────────────────

fn prices(root: &Root, id: &str, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let Some(info) = root.games.prices.get(id).filter(|v| !v.is_null()) else {
        let key = if root.games.prices.contains_key(id) { "wiki.offline" } else { "news.connecting" };
        return div().pt(px(8.)).text_size(px(T_M)).text_color(c(MUTED)).child(root.tr(key));
    };
    let items: Vec<Value> = info.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut kinds: Vec<String> = Vec::new();
    for it in &items {
        let k = s(it, "category").to_string();
        if !k.is_empty() && !kinds.contains(&k) {
            kinds.push(k);
        }
    }
    let kind = root.games.kind.clone().filter(|k| kinds.contains(k));
    // The search: every word typed, in the item's name, rarity or kind, accents aside (PriceList.tsx); dearest first.
    let query = root.price_search.read(cx).value().trim().to_string();
    let q = folded(&query);
    let mut shown: Vec<&Value> = items
        .iter()
        .filter(|it| kind.as_deref().is_none_or(|k| s(it, "category") == k))
        .filter(|it| {
            let hay = folded(&format!("{} {} {}", s(it, "name"), s(it, "rarity"), s(it, "category")));
            q.split_whitespace().all(|w| hay.contains(w))
        })
        .collect();
    shown.sort_by(|a, b| b.get("value").and_then(Value::as_f64).unwrap_or(0.).total_cmp(&a.get("value").and_then(Value::as_f64).unwrap_or(0.)));
    let site_url = info.pointer("/source/url").and_then(Value::as_str).filter(|u| u.starts_with("http")).map(str::to_string);
    let narrow = width < 760.;
    let site = info.pointer("/source/name").and_then(Value::as_str).unwrap_or_default().to_string();
    let when = info.get("at").and_then(Value::as_f64).map(|t| crate::time::ago(crate::time::now_secs() - (t / 1000.) as i64, &root.i18n)).unwrap_or_default();
    let vi = root.i18n.vietnamese;
    let money = |v: Option<f64>| match v {
        Some(v) if v > 0. => big(root, v),
        _ => "–".to_string(),
    };

    // The kinds as the Accounts page's filters: no track, the one shown a yellow block, each with its count.
    let count = |k: Option<&str>| items.iter().filter(|it| k.is_none_or(|k| s(it, "category") == k)).count();
    let pick = |key: Option<String>, label: String, on: bool, n: usize, cx: &mut Context<Root>| {
        chip(SharedString::from(format!("pk-{}", key.clone().unwrap_or_default())), on).h(px(TOOL_H)).px(px(12.)).child(label).child(chip_count(n, on)).on_click(cx.listener(move |r, _, _, cx| {
            r.games.kind = key.clone();
            cx.notify();
        }))
    };
    let mut chips = div().flex().flex_wrap().items_center().gap(px(4.)).child(pick(None, root.tr("news.all"), kind.is_none(), count(None), cx));
    for k in &kinds {
        chips = chips.child(pick(Some(k.clone()), k.clone(), kind.as_deref() == Some(k), count(Some(k)), cx));
    }

    // The items as cards on the page's grid, dearest first: the picture on its own colour, the name and kind, the value
    // large, then the permanent price, the demand and how it moves. (A table put the value 1300 px from its name on a
    // wide window, or left the right half empty.)
    let (_, card_w) = grid_fit(width, 280., GRID_GAP);
    let cards: Vec<AnyElement> = shown
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let url = Some(s(it, "img")).filter(|u| u.starts_with("http"));
            let pic = url.and_then(|u| images::pic_sq(u, 52., cx));
            let tone = url.and_then(|u| images::tone(u, cx));
            // (Its kind and rarity, the rarity alone when it only repeats the kind: "Limiteds · Limited" said it twice,
            // review 3b.5.)
            let (kind, rarity) = (s(it, "category"), s(it, "rarity"));
            let same = !rarity.is_empty() && kind.to_lowercase().starts_with(&rarity.to_lowercase());
            let sub = if same { rarity.to_string() } else { [kind, rarity].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" · ") };
            let trend = s(it, "trend");
            let trend_col = match trend {
                "Stable" => Some(OK),
                "Unstable" | "Fluctuating" => Some(WARN),
                "Overpaid" => Some(ONLINE),
                "Underpaid" => Some(MUTED),
                _ => None,
            };
            let trend_word = if trend.is_empty() { String::new() } else { root.tr(&format!("prices.trend.{trend}")) };
            let demand = it.get("demand").and_then(Value::as_f64);
            let perm = it.get("perm").and_then(Value::as_f64).filter(|v| *v > 0.);
            let name = div().id(("pk-item", i)).text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).truncate().child(s(it, "name").to_string());
            let name = match Some(s(it, "url").to_string()).filter(|u| u.starts_with("http")) {
                Some(url) => focusable(name).cursor_pointer().hover(|st| st.underline()).on_click(cx.listener(move |r, _, _, _| r.send("wikiOpen", json!({ "url": url })))).into_any_element(),
                None => name.into_any_element(),
            };
            let mut figures: Vec<(String, Option<Hsla>)> = Vec::new();
            if let Some(p) = perm {
                figures.push((format!("{} {}", root.tr("prices.perm"), big(root, p)), None));
            }
            if let Some(d) = demand {
                figures.push((format!("{} {d:.0}/10", root.tr("prices.demand")), None));
            }
            block()
                    .w(px(card_w))
                    .p(px(14.))
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .child(
                                div()
                                    .size(px(52.))
                                    .flex_none()
                                    .rounded(px(R_M))
                                    .overflow_hidden()
                                    .bg(match tone {
                                        Some(t) => mix(c(t), c(PANEL), 0.6),
                                        None => ca(WHITE, 0.05),
                                    })
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .map(|d| match pic {
                                        Some(p) => d.child(img(p).size_full().object_fit(ObjectFit::Contain)),
                                        None => d.child(icon("tag", 18., c(MUTED))),
                                    }),
                            )
                            .child(div().flex_1().min_w_0().flex().flex_col().gap(px(2.)).child(name).child(div().text_size(px(T_S)).text_color(c(MUTED)).truncate().child(sub))),
                    )
                    .child(
                        div()
                            .flex()
                            .items_end()
                            .gap(px(10.))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.))
                                    .child(div().text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("prices.value")))
                                    .child(div().text_size(px(T_FIG)).line_height(px(28.)).font_weight(FontWeight::SEMIBOLD).font_features(tnum()).child(money(it.get("value").and_then(Value::as_f64)))),
                            )
                            .when_some(trend_col.filter(|_| !trend_word.is_empty()), |d, col| {
                                d.child(
                                    div()
                                        .mb(px(4.))
                                        .h(px(22.))
                                        .px(px(8.))
                                        .flex()
                                        .flex_none()
                                        .items_center()
                                        .gap(px(6.))
                                        .rounded(px(7.))
                                        .bg(ca(col, 0.12))
                                        .text_size(px(T_S))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(c(col))
                                        .child(div().size(px(6.)).rounded_full().bg(c(col)))
                                        .child(trend_word.clone()),
                                )
                            }),
                    )
                    .when(!figures.is_empty(), |d| d.child(crate::kit::facts(figures).text_size(px(T_S))))
            .into_any_element()
        })
        .collect();
    let _ = (vi, narrow);
    div()
        .pt(px(4.))
        .flex()
        .flex_col()
        .gap(px(16.))
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(12.))
                .child(div().w(px(260.)).flex_shrink(1.).min_w(px(160.)).child(field(&root.price_search, Some("search"))))
                .when(kinds.len() > 1, |d| d.child(chips))
                .child(div().flex_1())
                .when(!site.is_empty(), |d| {
                    let from = div().id("pk-from").min_w_0().truncate().text_size(px(T_S)).text_color(c(MUTED)).child(root.i18n.tv("prices.from", &[("site", &site), ("t", &when)]));
                    d.child(match site_url.clone() {
                        Some(url) => focusable(from).cursor_pointer().hover(|st| st.text_color(c(TEXT_2))).on_click(cx.listener(move |r, _, _, _| r.send("wikiOpen", json!({ "url": url })))).into_any_element(),
                        None => from.into_any_element(),
                    })
                }),
        )
        .child(if shown.is_empty() {
            div().pt(px(8.)).text_size(px(T_M)).text_color(c(MUTED)).child(root.i18n.tv("prices.none", &[("q", &query)]))
        } else {
            crate::kit::grid(GRID_GAP).pb(px(4.)).children(cards)
        })
}
