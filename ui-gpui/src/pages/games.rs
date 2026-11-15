// Games: the games Bloxkeep's bot names (each with its wiki, some with news or a price list) as cards in their own
// pictures: yours first (followed, news unread, played here), then the popular ones. Opened, a game has its page: its
// picture with Follow, Wiki and its roblox.com page on top, Roblox's figures under it, then Overview (your accounts in it
// over 7 days, the hours it is quiet, what it is: the old Game info), its News and its Prices. Since 2026-11-09 the
// old Game info page lives here (Home opens a game on it). The rules are GamesView.tsx's: a game shows only with the
// wiki the bot names; the commands are the app's (wikiGames, feedCards, playedGames, gameLooks, gameDetails, wikiGame,
// feedFollow, feedRead, feedSeen, wikiOpen, openGameWiki, openGamePage, feedOpen, launch).
use std::collections::{HashMap, HashSet};

use gpui_kit::component::carousel::{Carousel, CarouselContent, CarouselItem, CarouselState};
use gpui_kit::component::input::{InputEvent, InputState};
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
    /// Mods for the Roblox client, for every account (pages/mods.rs).
    Mods,
    /// The private servers the accounts own, all of them together (pages/owned.rs).
    Servers,
}

/// The header's switch between them, as one black capsule in the middle of the header (the owner, 2026-11-14:
/// "ra giữa", Apple's Dynamic Island for its manner). Plain line icons; the one shown opens into its name, which
/// slides out as the capsule grows; beside the others, what waits there now (friends in a game, unread messages, a
/// watched thing that changed). A name shows on hover.
pub fn views(root: &Root, cx: &mut Context<Root>) -> impl IntoElement {
    let in_game = match &root.friends.list {
        Some(Ok(l)) => l.iter().filter(|f| f.kind == 2).count(),
        _ => 0,
    };
    let unread = (root.games.corner.unread.0 + root.games.corner.unread.1).max(0) as usize;
    let fresh = root.profile.watch.iter().any(|w| w.fresh);
    // (view, icon, words, a count beside it and its colour, a dot)
    let items: [(View, &str, &str, usize, u32, bool); 9] = [
        (View::Games, "grid", "news.title", 0, OK, false),
        (View::Friends, "users", "nav.friends", in_game, OK, false),
        (View::Avatar, "user", "nav.avatar", 0, OK, false),
        (View::Profile, "userGo", "nav.profile", 0, OK, false),
        (View::Shop, "tag", "nav.shop", 0, OK, fresh),
        (View::Inbox, "message", "nav.inbox", unread, FACE, false),
        (View::Servers, "bookmark", "inv.title", 0, OK, false),
        (View::Mods, "layers", "nav.mods", 0, OK, false),
        (View::Settings, "sliders", "nav.settings", 0, OK, false),
    ];
    let this = cx.entity().downgrade();
    let live = live_part(root, cx);
    div().id("rbx-view").flex().flex_none().items_center().gap(px(2.)).h(px(40.)).p(px(4.)).rounded_full().bg(c(ISLAND)).children(items.into_iter().enumerate().map(
        move |(i, (view, ic, words, n, tone, dot))| {
            let on = root.games.view == view;
            let this = this.clone();
            let name: SharedString = root.tr(words).into();
            hot(("rbx-view", i))
                .h(px(32.))
                .flex()
                .flex_none()
                .items_center()
                .rounded_full()
                .px(px(if on { 14. } else { 10. }))
                .gap(px(6.))
                .relative()
                .text_size(px(T_M))
                .font_weight(FontWeight::SEMIBOLD)
                .map(|d| if on { d.tones(ca(WHITE, 0.14), ca(WHITE, 0.14)).text_color(c(TEXT)) } else { d.tones(ca(WHITE, 0.), ca(WHITE, 0.07)).cursor_pointer() })
                .child(icon(ic, 16., if on { c(TEXT) } else { ca(TEXT, 0.55) }))
                .when(on, |d| {
                    d.child(
                        div()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .child(name.clone())
                            .with_animation(("rbx-view-name", i), Animation::new(std::time::Duration::from_millis(260)).with_easing(ease_out_quint()), |d, t| d.max_w(px(150. * t)).opacity(t)),
                    )
                })
                .when(!on && n > 0, |d| d.child(div().text_size(px(T_S)).font_weight(FontWeight::BOLD).font_features(tnum()).text_color(c(tone)).child(if n > 99 { "99+".to_string() } else { n.to_string() })))
                .when(!on && dot, |d| d.child(div().absolute().top(px(6.)).right(px(6.)).size(px(5.)).rounded_full().bg(c(OK))))
                .when(!on, |d| d.tooltip(move |_, cx| cx.new(|_| Tip(name.clone())).into()))
                .on_click(move |_, _, cx| {
                    let _ = this.update(cx, |r, cx| {
                        r.games.view = view;
                        r.friends.menu = false;
                        cx.notify();
                    });
                })
        },
    ))
    .children(live)
}

/// The capsule's moment: something happening now that it opens for, in place (a friend went into a game, a watched
/// thing got cheaper, a mod is going in). It shows a few seconds, or while `busy`, then the capsule closes again.
#[derive(Clone)]
pub struct Live {
    pub key: String,
    /// A picture (a friend's face, the thing), "" for the icon in its tone.
    pub pic: String,
    pub round: bool,
    pub icon: &'static str,
    pub tone: u32,
    pub title: String,
    pub sub: String,
    pub act: Option<LiveAct>,
    pub busy: bool,
    pub at: std::time::Instant,
}

/// What clicking the moment does.
#[derive(Clone)]
pub enum LiveAct {
    /// Join the friend's server as the account looked through.
    Join { acc: String, place: i64, job: String, user: i64, name: String },
    /// Marketplace › Watching.
    Watching,
    /// Mods › Installed.
    Mods,
}

/// How long a moment stays open.
pub const LIVE_FOR: std::time::Duration = std::time::Duration::from_secs(7);

/// The capsule opens for `l` (in place of one before it); it closes by itself after LIVE_FOR unless busy.
pub fn live_show(r: &mut Root, l: Live, cx: &mut Context<Root>) {
    let key = l.key.clone();
    let busy = l.busy;
    r.games.live = Some(l);
    cx.notify();
    if !busy {
        live_close_later(key, cx);
    }
}

fn live_close_later(key: String, cx: &mut Context<Root>) {
    cx.spawn(async move |this, cx| {
        cx.background_executor().timer(LIVE_FOR + std::time::Duration::from_millis(60)).await;
        let _ = this.update(cx, |r, cx| {
            if r.games.live.as_ref().is_some_and(|l| l.key == key && !l.busy && l.at.elapsed() >= LIVE_FOR) {
                r.games.live = None;
                cx.notify();
            }
        });
    })
    .detach();
}

/// A busy moment done: said for a few seconds more with its outcome (icon, tone, words), then closed.
pub fn live_done(r: &mut Root, key: &str, icon: &'static str, tone: u32, sub: String, act: Option<LiveAct>, cx: &mut Context<Root>) {
    let Some(l) = r.games.live.as_mut().filter(|l| l.key == key) else { return };
    l.busy = false;
    l.icon = icon;
    l.tone = tone;
    l.pic = String::new();
    l.sub = sub;
    l.act = act;
    l.at = std::time::Instant::now();
    cx.notify();
    live_close_later(key.to_string(), cx);
}

/// The open moment, drawn inside the capsule after the views: its picture, who or what, what happened, and its one
/// action. It slides out as the capsule grows.
fn live_part(root: &Root, cx: &mut Context<Root>) -> Option<AnyElement> {
    let l = root.games.live.as_ref()?;
    if !(l.busy || l.at.elapsed() < LIVE_FOR || cfg!(feature = "shot")) {
        return None;
    }
    let pic = (!l.pic.is_empty()).then(|| crate::images::pic_sq(&l.pic, 24., cx)).flatten();
    let mark = match pic {
        Some(p) => img(p).size(px(24.)).flex_none().map(|p| if l.round { p.rounded_full() } else { p.rounded(px(6.)) }).into_any_element(),
        // A person without their picture yet: their first letter in a ring, not a coloured icon (it read as a sticker).
        None if l.round => div()
            .size(px(24.))
            .flex_none()
            .rounded_full()
            .bg(ca(WHITE, 0.14))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(T_S))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(c(TEXT))
            .child(l.title.chars().next().map(|ch| ch.to_uppercase().to_string()).unwrap_or_default())
            .into_any_element(),
        None => div().size(px(24.)).flex_none().flex().items_center().justify_center().child(icon(l.icon, 16., c(l.tone))).into_any_element(),
    };
    let act = l.act.clone();
    let join = matches!(act, Some(LiveAct::Join { .. }));
    let key = l.key.clone();
    let words = div()
        .min_w_0()
        .flex()
        .items_center()
        .gap(px(6.))
        .whitespace_nowrap()
        .text_size(px(T_M))
        // (One size for both: the smaller second half sat lower than the name and looked unfinished.)
        .child(div().flex_none().font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(l.title.clone()))
        .child(div().min_w_0().max_w(px(240.)).truncate().text_color(c(MUTED)).child(l.sub.clone()));
    let mut part = hot("rbx-live")
        .h(px(32.))
        .pl(px(4.))
        .pr(px(if join { 3. } else { 12. }))
        .flex()
        .flex_none()
        .items_center()
        .gap(px(9.))
        .rounded_full()
        .tones(ca(WHITE, 0.), ca(WHITE, 0.06))
        .when(act.is_some() && !join, |d| d.cursor_pointer())
        .child(mark)
        .child(words)
        .when(l.busy, |d| {
            // (Going: three dots that breathe in turn.)
            d.child(div().flex().gap(px(3.)).children((0..3).map(|k| {
                div().size(px(4.)).rounded_full().bg(c(TEXT_2)).with_animation(("live-dot", k as usize), Animation::new(std::time::Duration::from_millis(900)).repeat().with_max_fps(30.), move |d, t| {
                    let p = ((t + k as f32 / 3.) % 1.) * std::f32::consts::TAU;
                    d.opacity(0.25 + 0.75 * (0.5 + 0.5 * p.sin()))
                })
            })))
        });
    if let Some(LiveAct::Join { acc, place, job, user, name }) = act.clone() {
        part = part.child(
            // As the chosen view's ground in the same capsule (a white pill was the loudest thing on the page).
            hot("rbx-live-join")
                .ml(px(2.))
                .h(px(26.))
                .px(px(12.))
                .flex()
                .items_center()
                .rounded_full()
                .tones(ca(WHITE, 0.14), ca(WHITE, 0.22))
                .text_color(c(TEXT))
                .text_size(px(T_S))
                .font_weight(FontWeight::SEMIBOLD)
                .cursor_pointer()
                .child(root.tr("fr.join"))
                .on_click(cx.listener(move |r, _, _, cx| {
                    r.games.live = None;
                    r.launch("joinServer", json!({ "ids": [acc.clone()], "placeId": place, "jobId": job.clone(), "followUserId": user, "followName": name.clone() }), false, cx)
                })),
        );
    } else if let Some(a) = act {
        part = part.on_click(cx.listener(move |r, _, _, cx| {
            match a {
                LiveAct::Watching => {
                    r.games.view = View::Shop;
                    r.profile.watch_view = true;
                    r.profile.watch = crate::itemwatch::list();
                    crate::itemwatch::seen();
                }
                LiveAct::Mods => {
                    r.games.view = View::Mods;
                    r.mods.mine = true;
                }
                LiveAct::Join { .. } => {}
            }
            if r.games.live.as_ref().is_some_and(|l| l.key == key && !l.busy) {
                r.games.live = None;
            }
            cx.notify();
        }));
    }
    Some(
        div()
            .overflow_hidden()
            .flex_none()
            .flex()
            .items_center()
            // A hairline between the views and the moment: they are two things in one capsule.
            .child(div().w(px(1.)).h(px(16.)).mx(px(6.)).flex_none().bg(ca(WHITE, 0.14)))
            .child(part)
            .with_animation(SharedString::from(format!("live-{}", l.key)), Animation::new(std::time::Duration::from_millis(340)).with_easing(ease_out_quint()), |d, t| d.max_w(px(460. * t)).opacity(t))
            .into_any_element(),
    )
}

/// The capsule laid over a header row, in its middle: the row must be `.relative()`.
pub fn island(root: &Root, cx: &mut Context<Root>) -> impl IntoElement {
    // In a small window it stands in line (the wordmark steps aside, `brand`): centred over the header it lay over
    // the search box and the buttons at the right (860 px wide, 2026-11-15).
    if root.narrow.get() {
        return div().flex_none().child(views(root, cx));
    }
    div().absolute().inset_0().flex().items_center().justify_center().child(views(root, cx))
}

/// Roblox's wordmark at the head of its pages; not in a small window, where the view capsule needs its room.
pub fn brand(root: &Root) -> Div {
    let d = div().h(px(38.)).flex().items_center();
    if root.narrow.get() { d } else { d.child(crate::kit::roblox_word(22., c(TEXT))) }
}

/// What the page asked the app for, and what it is showing.
#[derive(Default)]
pub struct Data {
    pub view: View,
    /// The capsule's open moment, if any (Live).
    pub live: Option<Live>,
    /// Over every Roblox page: the account's Robux, what is unread and whether Roblox holds it (corner, held_bar).
    pub corner: Corner,
    pub wiki_games: Option<Vec<Value>>,
    pub wiki_failed: bool,
    pub cards: Option<Vec<Value>>,
    pub details: HashMap<i64, Value>,
    pub prices: HashMap<String, Value>,
    asked_wiki: bool,
    /// When the list was last asked for again for a game the news brought that it lacked (by its id).
    asked_again: HashMap<String, std::time::Instant>,
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
    /// Counted up by each vault switch (forget_sign_in): a sign-in, chart or row reply asked before it is dropped.
    sign_gen: u64,
    /// The games list (`list`) as last made, by the fingerprint of what it was made from.
    list_cache: std::cell::RefCell<Option<(u64, std::rc::Rc<Vec<Game>>)>>,
    /// The price list's items in the order shown (its fingerprint with the words and kind, then their places), and how
    /// many are drawn for that list.
    price_cache: std::cell::RefCell<Option<(u64, std::rc::Rc<Vec<usize>>)>>,
    price_cap: std::cell::Cell<(u64, usize)>,
    /// How many news cards are drawn, for the game and source shown ("id|source").
    news_cap: std::cell::RefCell<Option<(String, usize)>>,
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
    /// How often each asking that came back empty was asked again (may_retry): an empty answer can be Roblox's true
    /// one (a game with no pictures, no details), so it isn't asked forever.
    retries: HashMap<String, u8>,
    /// The gameplay clips the tiles play under the mouse (pages/clips.rs).
    pub clips: crate::pages::clips::Clips,
    pub stays: HashMap<i64, Vec<Value>>,
    pub found: Option<(String, Option<Result<Vec<roblox::Game>, String>>)>,
    /// Play on a game's page: the game (its Open id) and the accounts ticked to join it.
    pub picking: Option<(String, HashSet<String>)>,
    /// The server Play's row joins (its Job ID), when it was opened from the Servers tab; none: any server.
    pub pick_job: Option<String>,
    /// A server link pasted on the game's page (a private server's, a share link, a game link with its server): Play's
    /// row joins through it instead of the game's own link.
    pub pick_link: Option<String>,
    /// A friend's server in another place of the game (a sub-place): Play's row joins that place, not the game's own.
    pub pick_place: Option<i64>,
    /// The game page's box for that link or a Job ID (made while a game is open: a box needs the window).
    link_box: Option<Entity<InputState>>,
    /// The charts' rows that don't fit their line are shown (the More menu is open).
    pub more_sorts: bool,
    /// Keep playing shows every game, not only the first ones.
    pub keep_all: bool,
    /// The game page's switch to another game (GamesView.tsx's Switcher): open, when it was last closed by a click
    /// outside (so a click on the name closes it rather than opening it again), and its search box (made while it
    /// is open with more than 6 games; a box needs the window, so `settle_words` makes it).
    switching: bool,
    switch_closed: Option<std::time::Instant>,
    switch_q: Option<Entity<InputState>>,
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
#[derive(Clone)]
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

/// A cheap fingerprint of some JSON, to keep a cache by: a walk of the tree, with long strings (pictures in base64)
/// sampled rather than read whole.
pub fn sig_of(v: &Value, h: &mut std::collections::hash_map::DefaultHasher) {
    use std::hash::Hash;
    match v {
        Value::Null => 0u8.hash(h),
        Value::Bool(b) => (1u8, b).hash(h),
        Value::Number(n) => (2u8, n.as_i64(), n.as_f64().map(f64::to_bits)).hash(h),
        Value::String(t) => {
            3u8.hash(h);
            let b = t.as_bytes();
            if b.len() <= 512 {
                b.hash(h);
            } else {
                b.len().hash(h);
                b[..128].hash(h);
                b[b.len() - 128..].hash(h);
                let step = b.len() / 128;
                b.iter().step_by(step).for_each(|x| x.hash(h));
            }
        }
        Value::Array(l) => {
            (4u8, l.len()).hash(h);
            l.iter().for_each(|x| sig_of(x, h));
        }
        Value::Object(o) => {
            (5u8, o.len()).hash(h);
            for (k, x) in o {
                k.hash(h);
                sig_of(x, h);
            }
        }
    }
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
/// Kept until what it is made from changes (it is asked for several times a frame).
pub fn list(root: &Root) -> std::rc::Rc<Vec<Game>> {
    use std::hash::Hasher;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for p in ["/wikis/games", "/feeds/games", "/community/data/games"] {
        sig_of(root.state.pointer(p).unwrap_or(&Value::Null), &mut h);
    }
    h.write_u8(9);
    root.games.wiki_games.iter().flatten().for_each(|w| sig_of(w, &mut h));
    h.write_u8(9);
    root.played.iter().flatten().for_each(|p| sig_of(p, &mut h));
    let key = h.finish();
    if let Some((k, l)) = root.games.list_cache.borrow().as_ref()
        && *k == key
    {
        return l.clone();
    }
    let l = std::rc::Rc::new(build_list(root));
    *root.games.list_cache.borrow_mut() = Some((key, l.clone()));
    l
}

fn build_list(root: &Root) -> Vec<Game> {
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

thread_local! {
    /// Each wiki's site logo (None: it has none, or didn't say), and the wikis asked this run.
    static LOGOS: std::cell::RefCell<HashMap<String, Option<String>>> = std::cell::RefCell::new(HashMap::new());
    static LOGOS_ASKED: std::cell::RefCell<std::collections::HashSet<String>> = std::cell::RefCell::new(std::collections::HashSet::new());
}

/// A game's own logo, as the game draws its name: its wiki's site logo (the wiki's "File:Site-logo.png": Blox Fruits'
/// is the game's blue and orange lettering on nothing). The owner, 2026-11-15, of the name in the game's colours:
/// "sai rồi, thiếu vibe, tôi muốn y chang như này", with that logo. Roblox itself gives no logo, only the icon and the
/// cover. Asked once a run per wiki; None for a game with no wiki, a wiki with no logo, or until the answer comes.
pub fn logo_of(root: &Root, place: i64, name: &str, cx: &mut Context<Root>) -> Option<String> {
    if !rich() {
        return None;
    }
    let site = list(root).iter().find(|g| same(place, name, g.place, &g.name)).and_then(|g| g.wiki.clone()).filter(|w| !w.is_empty())?;
    if let Some(known) = LOGOS.with(|l| l.borrow().get(&site).cloned()) {
        return known;
    }
    if LOGOS_ASKED.with(|a| a.borrow_mut().insert(site.clone())) {
        let link = root.link.clone()?;
        cx.spawn(async move |this, cx| {
            let d = crate::pages::wiki::api(&link, &site, json!({ "action": "query", "titles": "File:Site-logo.png", "prop": "imageinfo", "iiprop": "url|size" })).await;
            let logo = d
                .as_ref()
                .and_then(|d| d.pointer("/query/pages"))
                .and_then(Value::as_array)
                .and_then(|l| l.first())
                .and_then(|p| p.pointer("/imageinfo/0"))
                // (A logo too small to read as one, a favicon-sized mark, is left for the name in words.)
                .filter(|i| i.get("width").and_then(Value::as_i64).unwrap_or(0) >= 120)
                .and_then(|i| i.get("url").and_then(Value::as_str))
                .filter(|u| crate::wikitext::wiki_img(u))
                .map(str::to_string);
            LOGOS.with(|l| l.borrow_mut().insert(site, logo));
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }
    None
}

/// That logo as a picture `h` tall (as wide as its own shape makes it, no wider than `max_w`), once it has loaded.
pub fn logo_img(root: &Root, place: i64, name: &str, h: f32, max_w: f32, cx: &mut Context<Root>) -> Option<AnyElement> {
    let url = logo_of(root, place, name, cx)?;
    images::pic(&url, cx)?;
    let (pw, ph) = images::dims(&url, cx)?;
    let (mut w, mut tall) = (h * pw as f32 / ph.max(1) as f32, h);
    if w > max_w {
        tall *= max_w / w;
        w = max_w;
    }
    let pic = images::pic_at(&url, w, tall, cx)?;
    Some(img(pic).w(px(w.round())).h(px(tall.round())).object_fit(ObjectFit::Contain).flex_none().into_any_element())
}

/// The start place of the game whose page is shown, when it has one.
pub fn shown_place(r: &Root) -> Option<i64> {
    let open = r.games.open.as_ref()?;
    let place = list(r).iter().find(|g| g.id == open.id).map(|g| g.place).filter(|p| *p > 0).unwrap_or(open.place);
    (place > 0).then_some(place)
}

/// Opens a game's page from elsewhere (Home): its card when the bot names it, else its page with Overview only.
pub fn open_place(r: &mut Root, place: i64, name: &str) {
    let open = list(r)
        .iter()
        .find(|g| same(place, name, g.place, &g.name))
        .map(|g| Open { id: g.id.clone(), place: if g.place > 0 { g.place } else { place }, name: g.name.clone() })
        .unwrap_or_else(|| Open { id: format!("place:{place}"), place, name: clean(name) });
    r.games.open = Some(open);
    r.games.tab = Tab::Overview;
    r.games.about = false;
    r.go(Page::Games);
}

/// How long an asking that failed (no network a moment, Roblox busy) waits before it may be asked again: asked once
/// for the session made a passing failure stay; asked at once again would spin while it lasts.
pub const RETRY: std::time::Duration = std::time::Duration::from_secs(30);

/// Times an asking that came back empty is asked again before that answer is taken as it is.
const RETRIES: u8 = 2;

/// One more asking again of this (by its key), while it has some left.
fn may_retry(r: &mut Root, key: String) -> bool {
    let n = r.games.retries.entry(key).or_insert(0);
    *n += 1;
    *n <= RETRIES
}

/// How long before a game still missing from the list is asked about again (the bot keeps its list 10 minutes).
const AGAIN_EVERY: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// A game the news brings (a feed shown on the bot at once) that the list hasn't got, added to the bot meanwhile, asks
/// for the list again past what the app keeps: its card showed only after Bloxkeep was opened again (owner, Steal An
/// Egg, 2026-11-08). Each missing game at most every 10 minutes, so one the bot has no wiki for doesn't ask on and on.
fn ask_again(r: &mut Root, link: &crate::link::Link, cx: &mut Context<Root>) {
    let Some(list) = &r.games.wiki_games else { return };
    let missing: Vec<String> = r
        .state
        .pointer("/feeds/games")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|n| s(n, "id").to_string())
        .filter(|id| !id.is_empty() && !list.iter().any(|g| s(g, "id") == id))
        .collect();
    let now = std::time::Instant::now();
    if !missing.iter().any(|id| r.games.asked_again.get(id).is_none_or(|at| now.duration_since(*at) >= AGAIN_EVERY)) {
        return;
    }
    for id in missing {
        r.games.asked_again.insert(id, now);
    }
    let rx = link.call("wikiGames", json!({ "again": true }));
    cx.spawn(async move |this, cx| {
        if let Some(Ok(v)) = rx.await.ok() {
            let _ = this.update(cx, |r, cx| {
                r.games.wiki_games = Some(v.get("games").and_then(Value::as_array).cloned().unwrap_or_default());
                r.games.wiki_failed = false;
                cx.notify();
            });
        }
    })
    .detach();
}

/// What the page needs from the app, asked once each: the bot's games, the news, the games' pictures, and for the
/// open game its figures and its price list. Read and seen marks go back as the page shows them.
/// The games that have a wiki (and which wiki), asked once: every page wants it, since a game's own logo comes from its
/// wiki (`logo_of`) and Home shows a game first.
pub fn want_wikis(r: &mut Root, cx: &mut Context<Root>) {
    let Some(link) = r.link.clone() else { return };
    if !r.games.asked_wiki {
        r.games.asked_wiki = true;
        let rx = link.call("wikiGames", json!({}));
        cx.spawn(async move |this, cx| {
            let got = rx.await.ok();
            let failed = !matches!(got, Some(Ok(_)));
            let _ = this.update(cx, |r, cx| {
                r.games.wiki_failed = failed;
                match got {
                    Some(Ok(v)) => r.games.wiki_games = Some(v.get("games").and_then(Value::as_array).cloned().unwrap_or_default()),
                    // (What came before stays shown.)
                    _ => {
                        r.games.wiki_games.get_or_insert_with(Vec::new);
                    }
                }
                cx.notify();
            });
            // Failed (no network a moment): asked again, or the tab stays empty until the app restarts.
            if failed {
                cx.background_executor().timer(RETRY).await;
                let _ = this.update(cx, |r, cx| {
                    r.games.asked_wiki = false;
                    cx.notify();
                });
            }
        })
        .detach();
    }
}

pub fn want(r: &mut Root, cx: &mut Context<Root>) {
    let Some(link) = r.link.clone() else { return };
    want_wikis(r, cx);
    ask_again(r, &link, cx);
    if !r.games.asked_cards {
        r.games.asked_cards = true;
        let rx = link.call("feedCards", json!({}));
        cx.spawn(async move |this, cx| {
            let got = rx.await.ok().and_then(Result::ok);
            let failed = got.is_none();
            let _ = this.update(cx, |r, cx| {
                match got {
                    Some(v) => r.games.cards = Some(v.as_array().cloned().unwrap_or_default()),
                    None => {
                        r.games.cards.get_or_insert_with(Vec::new);
                    }
                }
                cx.notify();
            });
            if failed {
                cx.background_executor().timer(RETRY).await;
                let _ = this.update(cx, |r, cx| {
                    r.games.asked_cards = false;
                    cx.notify();
                });
            }
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
            let got = rx.await.ok().and_then(Result::ok).filter(|v| !v.is_null());
            let failed = got.is_none();
            let _ = this.update(cx, |r, cx| {
                if let Some(v) = got {
                    let universe = int(&v, "universe");
                    r.games.details.insert(place, v);
                    settle_stays(r, place);
                    want_shots(r, place, universe, cx);
                }
                cx.notify();
            });
            if failed {
                cx.background_executor().timer(RETRY).await;
                let _ = this.update(cx, |r, cx| {
                    if may_retry(r, format!("details:{place}")) {
                        r.games.asked_details.remove(&place);
                        cx.notify();
                    }
                });
            }
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
            let got = rx.await.ok().and_then(Result::ok);
            let failed = got.is_none();
            let _ = this.update(cx, |r, cx| {
                // (A list had before stays shown while a new asking failed.)
                if !failed || !r.games.prices.get(&id).is_some_and(|v| !v.is_null()) {
                    r.games.prices.insert(id.clone(), got.unwrap_or(Value::Null));
                }
                cx.notify();
            });
            // Failed (offline a moment): asked again, or Prices said "offline" until the app restarted.
            if failed {
                cx.background_executor().timer(RETRY).await;
                let _ = this.update(cx, |r, cx| {
                    r.games.asked_prices.remove(&id);
                    cx.notify();
                });
            }
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
    settle_switch(r, window, cx);
    settle_link(r, window, cx);
    let Some((game, words)) = r.games.words_want.borrow_mut().take() else { return };
    r.games.words_for = Some(game);
    r.games.words_saved = words.clone();
    r.news_words.update(cx, |s, cx| s.set_value(words, window, cx));
}

/// The game page's link box, made while a game is open (Enter joins through it), gone when none is.
fn settle_link(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    if r.games.open.is_none() {
        r.games.link_box = None;
        return;
    }
    if r.games.link_box.is_some() {
        return;
    }
    let e = cx.new(|cx| InputState::new(window, cx).placeholder(r.tr("gp.linkHint")));
    cx.subscribe(&e, |r: &mut Root, _, ev: &InputEvent, cx| match ev {
        InputEvent::PressEnter { .. } => {
            join_link(r, cx);
            cx.notify();
        }
        InputEvent::Change => cx.notify(),
        _ => {}
    })
    .detach();
    r.games.link_box = Some(e);
}

/// What was pasted in the link box: a Roblox link (a private server's, a share link, a game link with its server)
/// goes to the launch as it is (the app reads every kind, game_target.rs); a bare Job ID picks that server. Then Play's
/// row opens to tick the accounts.
fn join_link(r: &mut Root, cx: &mut Context<Root>) {
    let text = r.games.link_box.as_ref().map(|e| e.read(cx).value().trim().to_string()).unwrap_or_default();
    if text.is_empty() {
        return;
    }
    let lower = text.to_lowercase();
    let is_link = lower.contains("roblox.com") || lower.starts_with("roblox://") || lower.contains("ro.blox.com");
    let job = Some(text.as_str()).filter(|t| t.len() == 36 && t.chars().all(|ch| ch.is_ascii_hexdigit() || ch == '-') && t.matches('-').count() == 4);
    if is_link {
        crate::pages::games::pick_for_server(r, String::new());
        r.games.pick_job = None;
        r.games.pick_link = Some(text);
    } else if let Some(j) = job {
        let j = j.to_string();
        pick_for_server(r, j);
        r.games.pick_link = None;
    } else {
        r.toast = Some((r.tr("gp.linkBad"), "warn".into()));
    }
}

/// The switch to another game, while it is open with more than 6 games: its search box, made and typed into at once
/// (GamesView.tsx's Switcher; Enter opens the first game found). Closed, the box goes, so it opens empty next time.
fn settle_switch(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let wanted = r.games.switching && r.games.open.is_some();
    if !wanted {
        r.games.switch_q = None;
        return;
    }
    if r.games.switch_q.is_some() || list(r).len() <= 6 {
        return;
    }
    let hint = r.i18n.tv("games.search", &[("n", &list(r).len().to_string())]);
    let e = cx.new(|cx| InputState::new(window, cx).placeholder(hint));
    cx.subscribe(&e, |r: &mut Root, _, ev: &InputEvent, cx| match ev {
        InputEvent::PressEnter { .. } => {
            if let Some(id) = switch_list(r, cx).first().map(|g| g.id.clone()) {
                switch_to(r, &id);
            }
            cx.notify();
        }
        InputEvent::Change => cx.notify(),
        _ => {}
    })
    .detach();
    e.update(cx, |st, cx| st.focus(window, cx));
    r.games.switch_q = Some(e);
}

/// The games the switch lists: every game, found by the words in its box.
fn switch_list(root: &Root, cx: &App) -> Vec<Game> {
    let q = root.games.switch_q.as_ref().map(|e| folded(e.read(cx).value().trim())).unwrap_or_default();
    list(root).iter().filter(|g| q.is_empty() || folded(&g.name).contains(&q)).cloned().collect()
}

/// Another game picked in the switch: its page, on the same tab when it has one (the page falls back to Overview).
fn switch_to(r: &mut Root, id: &str) {
    r.games.switching = false;
    let all = list(r);
    if let Some(g) = all.iter().find(|g| g.id == id) {
        if r.games.open.as_ref().is_some_and(|o| o.id == g.id) {
            return;
        }
        r.games.open = Some(Open { id: g.id.clone(), place: g.place, name: g.name.clone() });
        r.games.about = false;
        r.games.source = None;
        r.games.picking = None;
        r.games.pick_job = None;
        r.games.pick_link = None;
        r.games.pick_place = None;
        r.games.page_scroll.set_offset(point(px(0.), px(0.)));
    }
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
                .text_size(px(T_M))
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
        let got = rx.await.unwrap_or_default();
        // (None at all is a failed asking: Roblox sends its placeholder for a game without pictures.)
        let failed = got.is_empty();
        let _ = this.update(cx, |r, cx| {
            if got.len() >= 2 {
                let state = cx.new(|_| CarouselState::new(got.len()));
                r.games.shots.insert(place, (got, state));
                cx.notify();
            }
        });
        if failed {
            cx.background_executor().timer(RETRY).await;
            let _ = this.update(cx, |r, _| {
                if may_retry(r, format!("shots:{place}")) {
                    r.games.asked_shots.remove(&place);
                }
            });
        }
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
        // (A reply still on its way is for the sign-in before: it is dropped.)
        self.sign_gen += 1;
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
    let sign = r.games.sign_gen;
    if !r.games.asked_cookie {
        r.games.asked_cookie = true;
        let rx = link.call("robloxSignIn", json!({}));
        cx.spawn(async move |this, cx| {
            let got = rx.await.ok().and_then(Result::ok);
            let _ = this.update(cx, |r, cx| {
                if r.games.sign_gen != sign {
                    return;
                }
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
            let failed = got.is_err();
            let _ = this.update(cx, |r, cx| {
                if r.games.sign_gen == sign {
                    r.games.sorts = Some(got);
                    // An answer: the tries start over, so a few passing failures over a long session don't end them.
                    if !failed {
                        r.games.retries.remove("sorts");
                    }
                    cx.notify();
                }
            });
            // Failed: asked again after a while (the charts said "failed" until the app restarted).
            if failed {
                cx.background_executor().timer(RETRY).await;
                let _ = this.update(cx, |r, cx| {
                    if r.games.sign_gen == sign && may_retry(r, "sorts".into()) {
                        r.games.asked_sorts = false;
                        cx.notify();
                    }
                });
            }
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
                let failed = got.is_err();
                let _ = this.update(cx, |r, cx| {
                    if r.games.sign_gen == sign {
                        r.games.rows.insert(id.clone(), got);
                        if !failed {
                            r.games.retries.remove(&format!("row:{id}"));
                        }
                        cx.notify();
                    }
                });
                if failed {
                    cx.background_executor().timer(RETRY).await;
                    let _ = this.update(cx, |r, cx| {
                        if r.games.sign_gen == sign && may_retry(r, format!("row:{id}")) {
                            r.games.asked_rows.remove(&id);
                            cx.notify();
                        }
                    });
                }
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
        let asked = want.clone();
        let rx = roblox::run(move || roblox::both(|| roblox::icons(&want), || roblox::covers(&want, "480x270")));
        cx.spawn(async move |this, cx| {
            let (icons, covers) = rx.await.unwrap_or_default();
            // (Roblox answers every game, a placeholder for one without an icon: one left out wasn't answered.)
            let missed: Vec<i64> = asked.into_iter().filter(|u| !icons.contains_key(u)).collect();
            let _ = this.update(cx, |r, cx| {
                // (Icons are the same whoever is signed in: one asked before a vault switch is still right.)
                r.games.icons.extend(icons);
                r.games.covers.extend(covers);
                cx.notify();
            });
            if !missed.is_empty() {
                cx.background_executor().timer(RETRY).await;
                let _ = this.update(cx, |r, cx| {
                    for u in &missed {
                        if may_retry(r, format!("icon:{u}")) {
                            r.games.asked_icons.remove(u);
                        }
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }
    // The game that leads the chart shown is drawn big: a sharper picture for it.
    let lead = shown_sort(r).and_then(|s| r.games.rows.get(&s.id).and_then(|l| l.as_ref().ok()).and_then(|l| l.first()).map(|g| g.universe)).filter(|u| *u > 0);
    if let Some(u) = lead {
        if r.games.asked_big.insert(u) {
            let rx = roblox::run(move || roblox::covers(&[u], "768x432"));
            cx.spawn(async move |this, cx| {
                let got = rx.await.unwrap_or_default();
                let failed = !got.contains_key(&u);
                let _ = this.update(cx, |r, cx| {
                    r.games.covers_big.extend(got);
                    cx.notify();
                });
                if failed {
                    cx.background_executor().timer(RETRY).await;
                    let _ = this.update(cx, |r, cx| {
                        if may_retry(r, format!("big:{u}")) {
                            r.games.asked_big.remove(&u);
                            cx.notify();
                        }
                    });
                }
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

// ───────────────────────── the corner ─────────────────────────

/// What Roblox's own top bar has, for the account looked as: its Robux, a bell with its unread notifications and
/// messages, and whether Roblox holds it (banned, warned), read once a minute at most while Roblox is shown.
#[derive(Default)]
pub struct Corner {
    of: i64,
    robux: Option<i64>,
    unread: (i64, i64),
    held: Option<roblox::Held>,
    asked: Option<std::time::Instant>,
    busy: bool,
    /// "I agree" on its way, and why Roblox turned the last one down.
    waking: bool,
    refused: Option<String>,
}

pub fn want_corner(r: &mut Root, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = crate::pages::friends::signed(r) else { return };
    let k = &mut r.games.corner;
    if k.of != user {
        *k = Corner { of: user, ..Default::default() };
    }
    if k.busy || k.asked.is_some_and(|t| t.elapsed().as_secs() < 60) {
        return;
    }
    k.asked = Some(std::time::Instant::now());
    // BLOXKEEP_SHOT_CORNER="robux notes messages [held]": a hidden shot's numbers (held: a 3-day ban).
    #[cfg(feature = "shot")]
    if let Ok(v) = std::env::var("BLOXKEEP_SHOT_CORNER") {
        let w: Vec<&str> = v.split_whitespace().collect();
        let n = |i: usize| w.get(i).and_then(|x| x.parse().ok());
        k.robux = n(0);
        k.unread = (n(1).unwrap_or(0), n(2).unwrap_or(0));
        let now = crate::time::now_secs();
        k.held = w.get(3).map(|_| roblox::Held { kind: "Ban 3 Days".into(), began: Some(now - 3600), ends: Some(now + 3 * 86400 - 3600), note: "Bullying or harassment".into(), back: false });
        return;
    }
    k.busy = true;
    let rx = roblox::run(move || (roblox::robux(&cookie, user), roblox::unread(&cookie), roblox::held(&cookie)));
    cx.spawn(async move |this, cx| {
        let Ok((robux, unread, held)) = rx.await else { return };
        let _ = this.update(cx, |r, cx| {
            let k = &mut r.games.corner;
            if k.of != user {
                return;
            }
            k.busy = false;
            k.robux = robux.or(k.robux);
            k.unread = unread;
            // (A read that failed keeps what was known.)
            if let Ok(h) = held {
                k.held = h;
            }
            cx.notify();
        });
    })
    .detach();
}

/// Read the corner again at the next frame (after a buy, or the inbox read).
pub fn stale_corner(r: &mut Root) {
    r.games.corner.asked = None;
}

fn to_inbox(r: &mut Root, tab: usize) {
    r.games.open = None;
    r.games.view = View::Inbox;
    r.inbox.tab = tab;
    r.friends.menu = false;
}

/// The Robux and the bell, at the right of the account tabs.
fn corner(root: &Root, cx: &mut Context<Root>) -> Option<impl IntoElement> {
    crate::pages::friends::signed(root)?;
    let k = &root.games.corner;
    let vi = root.i18n.vietnamese;
    let robux = hot("rbx-robux")
        .h(px(34.))
        .px(px(11.))
        .flex()
        .items_center()
        .gap(px(7.))
        .rounded(px(R_M))
        .tones(ca(WHITE, 0.), ca(WHITE, 0.06))
        .cursor_pointer()
        .child(icon("robux", 16., c(TEXT)))
        .child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).font_features(tnum()).child(k.robux.map(|n| thousands(n, vi)).unwrap_or_else(|| "–".into())))
        .on_click(cx.listener(|r, _, _, cx| {
            to_inbox(r, 3);
            cx.notify();
        }));
    let (notes, letters) = k.unread;
    let n = notes + letters;
    let bell = div()
        .relative()
        .flex_none()
        .child(IconButton::new("rbx-bell", "bell").size(34.).tip(root.tr("rb.bellTip")).on_click(cx.listener(move |r, _, _, cx| {
            // (To the messages when only they are unread.)
            to_inbox(r, if notes == 0 && letters > 0 { 1 } else { 2 });
            cx.notify();
        })))
        .when(n > 0, |d| {
            d.child(
                div()
                    .absolute()
                    .top(px(1.))
                    .right(px(0.))
                    .min_w(px(16.))
                    .h(px(16.))
                    .px(px(4.))
                    .rounded_full()
                    .bg(c(FACE))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(9.5))
                    .font_weight(FontWeight::BOLD)
                    .text_color(c(INK))
                    .child(if n > 99 { "99+".to_string() } else { n.to_string() }),
            )
        });
    // (The Robux only once Roblox has said them: a "–" read as a broken number.)
    Some(div().flex_none().flex().items_center().gap(px(4.)).when(k.robux.is_some(), |d| d.child(robux)).child(bell))
}

/// Roblox holds the account: what and until when, the moderator's note, and what can be done from here: "I agree"
/// when Roblox lets it back (a warning, a ban run out), else its appeal page or Roblox's own page, signed in as it.
fn held_bar(root: &Root, cx: &mut Context<Root>) -> Option<impl IntoElement> {
    let k = &root.games.corner;
    let h = k.held.as_ref()?;
    let (_, viewer) = crate::pages::friends::looking_ref(root);
    let id = viewer.and_then(|a| a.get("id")).and_then(Value::as_str).unwrap_or_default().to_string();
    let forever = h.ends.is_none() && !h.back;
    let head = if h.back {
        root.tr("rb.heldBack")
    } else if forever {
        root.tr("rb.heldForever")
    } else {
        root.i18n.tv("rb.heldUntil", &[("t", &crate::time::day_time(h.ends.unwrap_or_default()))])
    };
    let sub = [Some(h.kind.clone()), (!h.note.is_empty()).then(|| h.note.clone()), k.refused.clone()].into_iter().flatten().collect::<Vec<_>>().join(" · ");
    let (id2, busy) = (id.clone(), k.waking);
    Some(
        div()
            .flex_none()
            .px(px(16.))
            .py(px(12.))
            .rounded(px(R_PANEL))
            .bg(c(PANEL))
            .border_1()
            .border_color(ca(if h.back { WARN } else { BAD }, 0.55))
            .flex()
            .items_center()
            .gap(px(14.))
            .child(icon("shield", 20., c(if h.back { WARN } else { BAD })).flex_none())
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(div().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).child(head))
                    .child(div().text_size(px(T_M)).text_color(c(MUTED)).truncate().child(sub)),
            )
            .when(h.back, |d| {
                d.child(Button::new("rb-agree", root.tr("rb.agree")).icon("check").primary().small().disabled(busy).on_click(cx.listener(|r, _, _, cx| agree(r, cx))))
            })
            .when(!h.back, |d| d.child(Button::new("rb-appeal", root.tr("card.appeal")).icon("external").small().on_click(cx.listener(move |r, _, _, _| r.send("openAppeal", json!({ "id": id }))))))
            .child(Button::new("rb-site", root.tr("rb.onRoblox")).icon("globe").small().on_click(cx.listener(move |r, _, _, _| r.send("openWebsite", json!({ "id": id2 }))))),
    )
}

/// "I agree": Roblox lets the account back; read again after.
fn agree(r: &mut Root, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = crate::pages::friends::signed(r) else { return };
    if r.games.corner.waking {
        return;
    }
    r.games.corner.waking = true;
    r.games.corner.refused = None;
    let rx = roblox::run(move || roblox::reactivate(&cookie));
    cx.spawn(async move |this, cx| {
        let done = rx.await.unwrap_or_else(|_| Err(String::new()));
        let _ = this.update(cx, |r, cx| {
            let k = &mut r.games.corner;
            if k.of != user {
                return;
            }
            k.waking = false;
            match done {
                Ok(()) => {
                    k.held = None;
                    k.asked = None;
                    r.toast = Some((r.tr("rb.agreed"), "ok".into()));
                }
                Err(why) => k.refused = Some(r.i18n.tv("rs.failed", &[("why", if why.is_empty() { "Roblox" } else { &why })])),
            }
            cx.notify();
        });
    })
    .detach();
}

// ───────────────────────── the cards ─────────────────────────

pub fn games(root: &Root, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let all = list(root);
    let shell = div().id("games").size_full().rounded(px(R_PANEL)).bg(c(PANEL)).overflow_hidden().flex().flex_col();
    let content = match root.games.open.clone() {
        Some(o) => {
            let el = page(root, &all, o, width, cx);
            let ground = game_tint().filter(|_| rich()).map(|t| mix(c(PANEL), Hsla { a: 1., ..t }, 0.06));
            shell.when_some(ground, |d, g| d.bg(g)).child(el).into_any_element()
        }
        None if root.games.view == View::Friends => crate::pages::friends::friends(root, width, cx).into_any_element(),
        None if root.games.view == View::Avatar => crate::pages::avatar::avatar(root, width, cx).into_any_element(),
        None if root.games.view == View::Profile => crate::pages::profile::profile(root, width, cx).into_any_element(),
        None if root.games.view == View::Shop => crate::pages::profile::shop(root, width, cx).into_any_element(),
        None if root.games.view == View::Inbox => crate::pages::inbox::inbox(root, width, cx).into_any_element(),
        None if root.games.view == View::Settings => crate::pages::rsettings::page(root, width, cx).into_any_element(),
        None if root.games.view == View::Mods => crate::pages::mods::page(root, width, cx).into_any_element(),
        None if root.games.view == View::Servers => crate::pages::owned::page(root, width, cx).into_any_element(),
        None => shell.child(grid(root, &all, width, cx)).into_any_element(),
    };
    // (The page is built: what is drawn after it, a dialog or another window, is no game's.)
    set_game_tint(None);
    // The account tabs over it all (pages/tabs.rs): what is under them is the tab shown.
    // (Robux and the bell at the strip's right, as Roblox's top bar; over the page, Roblox holding the account.)
    let top = div().flex_none().flex().items_center().gap(px(8.)).child(crate::pages::tabs::strip(root, cx)).child(div().flex_1()).children(corner(root, cx));
    div().size_full().flex().flex_col().gap(px(8.)).child(top).children(held_bar(root, cx)).child(div().flex_1().min_h_0().child(content))
}

fn grid(root: &Root, all: &[Game], width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let inner = width - 2. * (PAD + 4.);
    // (Without accents or case, as the web page's search: "hoang" finds "Hoàng".)
    let query = folded(root.game_search.read(cx).value().trim());
    let shown: Vec<&Game> = all.iter().filter(|g| query.is_empty() || folded(&g.name).contains(&query)).collect();
    let card_w = home_fit(inner).1 + 2. * CPAD;

    let header = div()
        .flex_none()
        .px(px(PAD + 4.))
        .pt(px(18.))
        .pb(px(16.))
        .flex()
        .items_center()
        .gap(px(10.))
        .child(crate::pages::games::brand(root))
        .relative().child(island(root, cx))
        // (The "Live from the games' own alerts" line was taken out: "no one needs to know", the owner, 2026-11-13.)
        .child(div().flex_1())
        .child(
            div()
                .w(px(if width < 900. { 220. } else { 280. }))
                .min_w(px(150.))
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
        body = body.children(radar_all(root)).children(friends_strip(root, inner, cx)).children(keep_playing(root, inner, cx));
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
    // Join as the account whose friends these are, for one in a game whose server Roblox shows (the Friends page's Join;
    // Roblox's own row of faces joins from a face, owner 2026-11-12).
    let join_as = crate::pages::friends::looking(root).1.and_then(|a| a.get("id").and_then(Value::as_str).map(str::to_string));
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
        // In a game: its icon on the face in place of the green dot, and for how long under the name.
        let game_pic = (f.kind == 2 && !f.game_icon.is_empty()).then(|| images::pic_pair(&f.game_icon, 26., 26., cx)).flatten();
        let dot = if game_pic.is_some() { None } else { dot };
        let span = (f.kind == 2 && f.since > 0).then(|| crate::time::span(crate::time::now_secs() - f.since));
        let joinable = f.kind == 2 && f.place > 0 && !f.job.is_empty();
        let join = join_as.clone().filter(|_| joinable).map(|acc| {
            let (place, job, name) = (f.place, f.job.clone(), f.name.clone());
            Button::new(("gf-join", i), root.tr("fr.join")).small().tint().on_click(cx.listener(move |r, _, _, cx| {
                cx.stop_propagation();
                r.launch("joinServer", json!({ "ids": [acc], "placeId": place, "jobId": job, "followUserId": who, "followName": name }), false, cx)
            }))
        });
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
                .when_some(dot, |d, col| d.child(div().absolute().right(px(2.)).bottom(px(2.)).size(px(16.)).rounded_full().bg(c(PANEL)).flex().items_center().justify_center().child(div().size(px(10.)).rounded_full().bg(c(col)))))
                .when_some(game_pic, |d, (p, k)| {
                    d.child(div().absolute().right(px(-4.)).bottom(px(-2.)).size(px(30.)).rounded(px(8.)).bg(c(OK)).p(px(2.)).child(div().size_full().rounded(px(6.)).overflow_hidden().child(fade_in(img(p).size_full().object_fit(ObjectFit::Cover), &k))))
                }),
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
                    .when(f.kind == 2 && !f.game.is_empty(), |d| {
                        d.child(
                            div()
                                .max_w_full()
                                .flex()
                                .items_center()
                                .gap(px(4.))
                                .text_size(px(T_XS))
                                .when_some(span.clone(), |d, t| d.child(div().flex_none().text_color(c(OK)).font_weight(FontWeight::SEMIBOLD).font_features(tnum()).child(t)))
                                .child(div().min_w_0().truncate().text_color(if span.is_some() { c(MUTED) } else { c(OK) }).child(clean(&f.game))),
                        )
                    })
                    .when_some(join, |d, b| d.child(div().mt(px(4.)).on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()).child(b))),
            )
            .on_click(cx.listener(move |r, _, _, cx| crate::pages::profile::open_person(r, who, cx)))
            // A right click: the friend's menu where the mouse is (pages/friends.rs).
            .on_mouse_down(MouseButton::Right, cx.listener(move |r, e: &MouseDownEvent, _, cx| {
                crate::pages::friends::open_face_menu(r, who, e.position);
                cx.notify();
            }))
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
            .children(crate::pages::friends::face_menu(root, cx))
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
    let n = all.len();
    // A wide page: the first game on a spotlight, big and playing its clip, the next ones stacked beside it (the
    // owner, 2026-11-13: "one layer with cards spread on it … be creative").
    let wide = width >= 900.;
    let first = if wide { 4 } else { cols };
    // The rest were out of reach: the head counted 12 and four showed (the owner, 2026-11-13: "xem full các game đã
    // play kiểu gì"). "Show all" lays them out under the first ones.
    let open = root.games.keep_all && n > first;
    let head = section_head(root.tr("games.keepPlaying"), Some(n)).when(n > first, |d| {
        d.child(
            Button::new("gk-all", root.tr(if open { "gp.less" } else { "games.keepAll" }))
                .icon_end(if open { "chevronUp" } else { "chevronDown" })
                .quiet()
                .small()
                .on_click(cx.listener(|r, _, _, cx| {
                    r.games.keep_all = !r.games.keep_all;
                    cx.notify();
                })),
        )
    });
    let tile = |i: usize, g: &crate::pages::home::Game, cx: &mut Context<Root>| -> AnyElement {
        let (place, name) = (g.place, g.name.clone());
        let icon_url = look(root, g.place, "icon").or(g.icon.clone());
        cascade(
            "gk-in",
            i,
            crate::pages::home::cover_tile(("gk-tile", i), look(root, g.place, "cover"), icon_url, g.name.clone(), played(root, g), g.accounts.len(), false, w, crate::pages::clips::tile_clip(root, place, cx), cx).on_click(
                cx.listener(move |r, _, _, cx| {
                    open_place(r, place, &name);
                    cx.notify();
                }),
            ),
        )
        .into_any_element()
    };
    let top = if wide {
        spotlight(root, &all, width, cx)
    } else {
        let tiles: Vec<AnyElement> = all.iter().take(first).enumerate().map(|(i, g)| tile(i, g, cx)).collect();
        crate::kit::grid(GRID_GAP).children(tiles).into_any_element()
    };
    let rest: Vec<AnyElement> = if open { all.iter().enumerate().skip(first).map(|(i, g)| tile(i, g, cx)).collect() } else { Vec::new() };
    Some(div().flex().flex_col().gap(px(12.)).child(head).child(top).when(open, |d| d.child(crate::kit::grid(GRID_GAP).children(rest))).into_any_element())
}

/// What a game in Keep playing says under its name: its windows now, else its hours, else "lately".
fn played(root: &Root, g: &crate::pages::home::Game) -> (String, bool) {
    let running = !g.accounts.is_empty();
    let sub = if running {
        root.i18n.tv("home.runningN", &[("n", &g.accounts.len().to_string())])
    } else if g.hours >= 1. {
        root.i18n.tv("home.hours", &[("n", &thousands(g.hours.round() as i64, root.i18n.vietnamese))])
    } else {
        root.tr("home.recentShort")
    };
    (sub, running)
}

/// The first game big, its gameplay clip playing on its picture (muted, pages/clips.rs), its name large on a plate at
/// its foot; the next three stacked at its right, each a picture and its name.
fn spotlight(root: &Root, all: &[crate::pages::home::Game], width: f32, cx: &mut Context<Root>) -> AnyElement {
    const GAP_S: f32 = 16.;
    // The picture whole: Roblox's cover is 16:9 and says things at its edges (a title, a price), so the art is 16:9 too.
    // A 3:1 band cut the top and bottom off it (owner 2026-11-15: "banner hiện còn đéo nên hồn"); a 16:9 one as wide
    // as 64 % of the page stood taller than the window (2026-11-13), so its height is capped and its width follows.
    const PLATE_H: f32 = 84.;
    let art_h = ((width - GAP_S) * 0.64 * 9. / 16.).clamp(200., 420.).round();
    let big_w = (art_h * 16. / 9.).round();
    let big_h = art_h + PLATE_H;
    let side_w = width - big_w - GAP_S;
    let Some(g) = all.first() else { return div().into_any_element() };
    let (place, name) = (g.place, g.name.clone());
    crate::pages::clips::want(root, place, cx);
    let (sub, running) = played(root, g);
    let cover = look(root, place, "cover");
    let picture = cover.as_deref().and_then(|u| crate::images::pic_pair(u, big_w, art_h, cx));
    let badge = look(root, place, "icon").or(g.icon.clone()).and_then(|u| crate::images::pic_sq(&u, 44., cx));
    let n = g.accounts.len();
    let mut acts = div().flex().gap(px(8.));
    if running {
        acts = acts.child(Button::new("gs-windows", root.i18n.tn("home.windowsN", n as i64, &[])).icon("screens").primary().on_click(cx.listener(|r, _, _, cx| {
            r.go(crate::pages::Page::Accounts);
            cx.notify();
        })));
    }
    let (p2, n2) = (place, name.clone());
    acts = acts.child(Button::new("gs-page", root.tr("home.gamePage")).icon("info").map(|b| if running { b.quiet() } else { b.primary() }).on_click(cx.listener(move |r, _, _, cx| {
        open_place(r, p2, &n2);
        cx.notify();
    })));
    // One card: the picture whole at the top, its name and buttons on a quiet footer under it (no plate over the
    // picture; owner 2026-11-14: the words under the picture, "đẹp và luxury hơn").
    let plate = div()
        .h(px(PLATE_H))
        .flex_none()
        .px(px(22.))
        .flex()
        .items_center()
        .gap(px(16.))
        .border_t_1()
        .border_color(ca(WHITE, 0.06))
        .children(badge.map(|b| {
            div()
                .size(px(44.))
                .flex_none()
                .relative()
                .rounded(px(R_M))
                .overflow_hidden()
                .child(img(b).size_full().object_fit(ObjectFit::Cover))
                .child(div().absolute().inset_0().rounded(px(R_M)).border_1().border_color(ca(WHITE, 0.10)))
        }))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(3.))
                .child(match logo_img(root, place, &name, 40., 220., cx) {
                    Some(logo) => div().flex().child(logo),
                    None => div().truncate().text_size(px(T_SEC + 2.)).line_height(px(26.)).font_weight(FontWeight::SEMIBOLD).child(game_name(name.clone(), look(root, place, "icon").or(g.icon.clone()).as_deref(), cx)),
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(7.))
                        .text_size(px(T_S))
                        .text_color(c(MUTED))
                        .child(div().size(px(6.)).rounded_full().bg(if running { c(OK) } else { c(MUTED) }))
                        .child(sub),
                ),
        )
        .child(div().flex_none().child(acts));
    let art = div()
        .relative()
        .w_full()
        .h(px(big_h - PLATE_H))
        .flex_none()
        .overflow_hidden()
        .bg(c(RAISED_2))
        .children(picture.map(|(p, k)| fade_in(img(p).size_full().object_fit(ObjectFit::Cover), &k).into_any_element()))
        .children(crate::pages::clips::ready(root, place));
    let big = hot("gs-big")
        .w(px(big_w))
        .h(px(big_h))
        .flex_none()
        .flex()
        .flex_col()
        .rounded(px(R_L))
        .overflow_hidden()
        .bg(c(RAISED))
        .border_1()
        .border_color(ca(WHITE, 0.07))
        .cursor_pointer()
        .child(art)
        .child(plate)
        .on_click(cx.listener(move |r, _, _, cx| {
            open_place(r, place, &name);
            cx.notify();
        }));

    // The next ones, a picture and words each, filling the spotlight's height.
    // Two across when there is room for them (a wide window left the right half of this column empty, the spotlight
    // being as wide as its 16:9 picture now): up to six games, three rows.
    // (Only with more than three to show: three games stay one under the other, as before.)
    let more = all.len().saturating_sub(1);
    let across: usize = if side_w >= 640. && more > 3 { 2 } else { 1 };
    let shown = more.min(3 * across);
    let rows = shown.div_ceil(across);
    let row_h = if rows == 0 { 0. } else { ((big_h - GAP_S * (rows as f32 - 1.)) / rows as f32).min(big_h / 2.).floor() };
    let cell_w = ((side_w - GAP_S * (across as f32 - 1.)) / across as f32).floor();
    // The picture stays 16:9 whatever room the row has (a cover cut to a tall strip lost its words).
    let pic_w = (row_h * 16. / 9.).min(cell_w * 0.6).round();
    let pic_h = (pic_w * 9. / 16.).round();
    let side = all.iter().skip(1).take(shown).enumerate().map(|(i, g)| {
        let (place, name) = (g.place, g.name.clone());
        let (sub, running) = played(root, g);
        let pic = look(root, place, "cover").and_then(|u| crate::images::pic_pair(&u, pic_w, pic_h, cx));
        let clip = crate::pages::clips::tile_clip(root, place, cx);
        let art = div()
            .id(SharedString::from(format!("gs-side-art-{place}")))
            .on_hover(clip.hover)
            .relative()
            .w(px(pic_w))
            .h(px(pic_h))
            .flex_none()
            .rounded(px(R_L))
            .overflow_hidden()
            .bg(c(RAISED))
            .children(pic.map(|(p, k)| fade_in(img(p).size_full().object_fit(ObjectFit::Cover), &k).into_any_element()))
            .children(clip.layer);
        cascade(
            "gs-in",
            i,
            hot(("gs-side", i))
                .w(px(cell_w))
                .h(px(row_h))
                .flex()
                .items_center()
                .gap(px(14.))
                .cursor_pointer()
                .child(art)
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .child(div().truncate().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).child(game_name(name.clone(), look(root, place, "icon").as_deref(), cx)))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .text_size(px(T_S))
                                .text_color(c(MUTED))
                                .when(running, |d| d.child(div().size(px(6.)).rounded_full().bg(c(OK))))
                                .child(sub),
                        ),
                )
                .on_click(cx.listener(move |r, _, _, cx| {
                    open_place(r, place, &name);
                    cx.notify();
                })),
        )
        .into_any_element()
    });
    div().flex().gap(px(GAP_S)).child(big).child(div().w(px(side_w)).min_w_0().flex().flex_wrap().content_start().gap(px(GAP_S)).children(side)).into_any_element()
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
    let clip = crate::pages::clips::tile_clip(root, place, cx);
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
                .id(SharedString::from(format!("rxg-art-{i}-{place}")))
                .on_hover(clip.hover)
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
                .children(clip.layer)
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
    let clip = crate::pages::clips::tile_clip(root, place, cx);
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
                .id(SharedString::from(format!("rxg-lead-art-{}", g.place)))
                .on_hover(clip.hover)
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
                .children(clip.layer)
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
    radar_lines(root, radar(root, place, name), false)
}

/// All of them over the games' list, each with its game's name (GamesView.tsx's RadarStrip).
fn radar_all(root: &Root) -> Option<Div> {
    radar_lines(root, root.state.pointer("/community/data/radar").and_then(Value::as_array).cloned().unwrap_or_default(), true)
}

fn radar_lines(root: &Root, lines: Vec<Value>, named: bool) -> Option<Div> {
    if lines.is_empty() {
        return None;
    }
    // The Accounts page's calm panel for what needs a look: a faint ground of the warning's colour, the mark on its
    // own small block, the words in plain text.
    let mut strip = div().px(px(12.)).py(px(8.)).rounded(px(R_L + 2.)).bg(tinted(PANEL, WARN, 0.07)).border_1().border_color(ca(WARN, 0.26)).flex().flex_col().gap(px(4.));
    for r in &lines {
        let mut text = root.i18n.tv(&format!("cm.radar.{}", s(r, "why")), &[("n", &int(r, "pcs").to_string()), ("m", &int(r, "minutes").to_string())]);
        if named {
            let name = Some(s(r, "name")).filter(|n| !n.is_empty()).map(str::to_string).unwrap_or_else(|| root.tr("cm.aGame"));
            text = format!("{name}: {text}");
        }
        strip = strip.child(
            div()
                .min_h(px(32.))
                .flex()
                .items_center()
                .gap(px(10.))
                .text_size(px(T_M))
                .line_height(px(19.))
                .text_color(c(TEXT))
                .child(icon("alert", 15., c(WARN)).flex_none())
                .child(div().min_w_0().flex_1().child(text)),
        );
    }
    Some(strip)
}

/// What a card says it has: its words, whether they need a look, and what they mean under the mouse (GamesView.tsx's
/// gm-tag titles).
fn tags(root: &Root, g: &Game) -> Vec<(String, bool, Option<String>)> {
    let mut v = Vec::new();
    if !radar(root, g.place, &g.name).is_empty() {
        v.push((root.tr("cm.radarTag"), true, Some(root.tr("cm.radarTip"))));
    }
    if g.news.is_some() {
        let n = unread(g);
        v.push(if n > 0 { (root.i18n.tv("news.unreadN", &[("n", &n.to_string())]), true, None) } else { (root.tr("games.news"), false, None) });
    }
    if let Some(h) = g.played {
        let n = if h < 1. { "<1".to_string() } else { format!("{:.0}", h) };
        v.push((root.i18n.tv("games.playedH", &[("n", &n)]), false, Some(root.tr("games.playedTip"))));
    }
    if g.playing > 0 {
        v.push((root.i18n.tv("cm.playingN", &[("n", &g.playing.to_string())]), false, Some(root.tr("cm.playingTip"))));
    }
    if g.prices {
        v.push((root.tr("prices.tab"), false, None));
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
    for (i, (t, hot, tip)) in tags.into_iter().enumerate() {
        if i > 0 || follows {
            line = line.child(sep());
        }
        let tag = div().flex_none().when(hot, |d| d.text_color(c(TEXT)).font_weight(FontWeight::SEMIBOLD)).child(t);
        line = match tip {
            Some(tip) => {
                let tip = SharedString::from(tip);
                line.child(tag.id(SharedString::from(format!("gm-tag-{}-{i}", g.id))).tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()))
            }
            None => line.child(tag),
        };
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
                        .child(div().text_size(px(T_L)).line_height(px(19.)).font_weight(FontWeight::SEMIBOLD).truncate().child(game_name(g.name.clone(), icon_of(root, g).as_deref(), cx)))
                        .child(line),
                )
                .when(is_new && !follows, |d| d.child(div().flex_none().text_size(px(T_S)).font_weight(FontWeight::SEMIBOLD).text_color(c(FACE)).child(root.tr("news.new")))),
        )
}

// ───────────────────────── one game ─────────────────────────

fn page(root: &Root, all: &[Game], open: Open, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let game = all.iter().find(|g| g.id == open.id);
    let place = game.map(|g| g.place).filter(|p| *p > 0).unwrap_or(open.place);
    // The game's own colour (from its icon, else its cover) for everything on its page.
    let own = game.and_then(|g| icon_of(root, g)).or_else(|| look(root, place, "icon")).or_else(|| look(root, place, "cover"));
    set_game_tint(own.as_deref().and_then(|u| images::tone(u, cx)));
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
            .child(hero(root, all, game, details, &open, &name, place, width, cx))
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
                        .child(icon("refresh", 15., c(WARN)).flex_none())
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
    r.games.pick_place = None;
    // (A link pasted before would send the server's Job ID to that link's place.)
    r.games.pick_link = None;
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
        .when_some(root.games.pick_job.clone().filter(|j| !j.is_empty()), |d, job| {
            d.child(div().flex().items_center().gap(px(6.)).text_size(px(T_S)).text_color(c(MUTED)).font_features(tnum()).child(icon("server", 13., c(MUTED))).child(job.chars().take(8).collect::<String>()))
        })
        // (Through a pasted link: which kind.)
        .when(root.games.pick_link.is_some(), |d| {
            d.child(div().flex().items_center().gap(px(6.)).text_size(px(T_S)).text_color(c(MUTED)).child(icon("link", 13., c(MUTED))).child(root.tr("gp.linkUsed")))
        })
        .child(Button::new("gm-pick-cancel", root.tr("gp.cancel")).quiet().on_click(cx.listener(|r, _, _, cx| {
            r.games.picking = None;
            r.games.pick_job = None;
            r.games.pick_link = None;
            r.games.pick_place = None;
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
                    let at = r.games.pick_place.take().unwrap_or(place);
                    let link = r.games.pick_link.take().unwrap_or_else(|| format!("https://www.roblox.com/games/{at}"));
                    match r.games.pick_job.take().filter(|j| !j.is_empty()) {
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
    chip(id, on).h(px(TOOL_H)).px(px(12.)).child(icon(icon_name, 15., if on { crate::theme::on_text() } else { c(MUTED) })).child(label)
}

/// The open game's name, as a button to any other game (GamesView.tsx's Switcher): a list of them, found by name in a
/// box above 6, the one open ticked; one picked opens. Alone (no other game), the name is only words.
fn switcher(root: &Root, all: &[Game], open: &Open, name: &str, width: f32, cx: &mut Context<Root>) -> AnyElement {
    // (In the game's own colours, as its logo is.)
    let own = all.iter().find(|g| g.id == open.id).and_then(|g| icon_of(root, g)).or_else(|| look(root, open.place, "icon"));
    let title = match logo_img(root, open.place, name, if width < 900. { 56. } else { 72. }, 320., cx) {
        Some(logo) => div().min_w_0().flex().items_center().child(logo),
        None => div().min_w_0().text_size(px(if width < 900. { 30. } else { 36. })).font_weight(FontWeight::BOLD).line_height(px(44.)).truncate().child(game_name(name.to_string(), own.as_deref(), cx)),
    };
    if !all.iter().any(|g| g.id != open.id) {
        return title.into_any_element();
    }
    let on = root.games.switching;
    let tip = SharedString::from(root.tr("games.switch"));
    let button = hot("gm-switch")
        .min_w_0()
        .flex()
        .items_center()
        .gap(px(8.))
        .px(px(6.))
        .ml(px(-6.))
        .rounded(px(R_M))
        .cursor_pointer()
        .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
        .child(title)
        .child(icon(if on { "chevronUp" } else { "chevronDown" }, 20., c(MUTED)))
        .on_click(cx.listener(|r, _, _, cx| {
            // (A click on the name while it is open closes it: the click outside the list closed it a moment ago.)
            let just = r.games.switch_closed.take().is_some_and(|at| at.elapsed().as_millis() < 300);
            r.games.switching = !just && !r.games.switching;
            cx.notify();
        }));
    let mut wrap = div().relative().min_w_0().child(button);
    if on {
        let shown = switch_list(root, cx);
        let mut rows = div().id("gm-switch-list").flex().flex_col().max_h(px(360.)).overflow_y_scroll();
        for (i, g) in shown.iter().enumerate() {
            let here = g.id == open.id;
            let (id, n) = (g.id.clone(), unread(g));
            rows = rows.child(
                menu_row(("gm-sw", i))
                    .flex_none()
                    .when(here, |d| d.text_color(c(TEXT)))
                    .child(div().w(px(14.)).flex_none().when(here, |d| d.child(icon("check", 14., c(TEXT)))))
                    .child(game_icon(icon_of(root, g), &g.name, 22., cx))
                    .child(div().flex_1().min_w_0().truncate().child(g.name.clone()))
                    .when(n > 0, |d| d.child(chip_count(n as usize, false)))
                    .on_click(cx.listener(move |r, _, _, cx| {
                        switch_to(r, &id);
                        cx.notify();
                    })),
            );
        }
        if shown.is_empty() {
            rows = rows.child(div().px(px(10.)).py(px(8.)).text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("wiki.nothing")));
        }
        let menu = div()
            .id("gm-switch-menu")
            .occlude()
            .w(px(320.))
            .p(px(4.))
            .rounded(px(R_M))
            .bg(c(RAISED))
            .border_1()
            .border_color(c(LINE))
            .shadow_lg()
            .flex()
            .flex_col()
            .gap(px(4.))
            .on_mouse_down_out(cx.listener(|r, _, _, cx| {
                if r.games.switching {
                    r.games.switching = false;
                    r.games.switch_closed = Some(std::time::Instant::now());
                    cx.notify();
                }
            }))
            .when_some(root.games.switch_q.as_ref(), |d, e| d.child(div().p(px(4.)).child(field(e, Some("search")).h(px(34.)))))
            .child(rows);
        wrap = wrap.child(div().absolute().top(px(50.)).left_0().child(deferred(anchored().snap_to_window_with_margin(px(8.)).child(pop("gm-switch-pop", on, menu))).with_priority(1)));
    }
    wrap.into_any_element()
}

/// The game on its own picture (as Home's stage): back to all games, its icon and name, who makes it, and what to do.
#[allow(clippy::too_many_arguments)]
fn hero(root: &Root, all: &[Game], game: Option<&Game>, details: Option<&Value>, open: &Open, name: &str, place: i64, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
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
    // (Only who makes it and its kind: where its news, wiki and values come from is said on their own tabs. Eight
    // facts in a row, sources and a web address among them, read cheap: the owner, 2026-11-14.)
    let mut line = div().flex().flex_wrap().items_center().gap(px(8.)).text_size(px(T_M)).text_color(c(TEXT_2));
    for (i, f) in facts.into_iter().enumerate() {
        if i > 0 {
            line = line.child(sep());
        }
        line = line.child(f);
    }
    // Yours in it, from the Overview's rows (7 days): how many play it now, your time in it, when last.
    let rows: Vec<Value> = if place > 0 {
        root.games.stays.get(&place).cloned().unwrap_or_else(|| details.and_then(|d| d.get("accounts")).and_then(Value::as_array).cloned().unwrap_or_default())
    } else {
        Vec::new()
    };
    let in_now: Vec<String> = rows
        .iter()
        .filter(|a| s(a, "now") == "ingame")
        .map(|a| s(a, "id").to_string())
        .filter(|id| root.accounts().iter().any(|x| s(x, "id") == id))
        .collect();
    let minutes: i64 = rows.iter().map(|a| int(a, "minutes")).sum();
    let last = rows.iter().filter_map(|a| a.get("lastMs").and_then(Value::as_i64)).max();
    // One quiet line of numbers: yours in it now (a green dot), your week in it or when last, and how many PCs play
    // it, in words only.
    let mut mine: Vec<(bool, String)> = Vec::new();
    if !in_now.is_empty() {
        mine.push((true, root.i18n.tv("gp.mineNow", &[("n", &in_now.len().to_string())])));
    }
    if minutes > 0 {
        let t = if minutes >= 60 {
            root.i18n.tv("gp.hours", &[("n", &((minutes as f64 / 60.).round() as i64).to_string())])
        } else {
            root.i18n.tv("gp.minutes", &[("n", &minutes.to_string())])
        };
        mine.push((false, root.i18n.tv("gp.mineWeek", &[("t", &t)])));
    }
    if in_now.is_empty()
        && let Some(ms) = last
    {
        let ago = crate::time::ago(crate::time::now_secs() - ms / 1000, &root.i18n);
        mine.push((false, root.i18n.tv("gp.mineLast", &[("t", &ago)])));
    }
    if let Some(g) = game.filter(|g| g.playing > 0) {
        mine.push((false, root.i18n.tv("cm.playingN", &[("n", &g.playing.to_string())])));
    }
    let mine_line = (!mine.is_empty()).then(|| {
        let mut l = div().flex().flex_wrap().items_center().gap(px(8.)).text_size(px(T_M)).text_color(c(MUTED)).font_features(tnum());
        for (i, (live, text)) in mine.into_iter().enumerate() {
            if i > 0 {
                l = l.child(sep());
            }
            l = l.child(div().flex().items_center().gap(px(7.)).when(live, |d| d.text_color(c(TEXT)).child(div().size(px(7.)).rounded_full().bg(c(OK)))).child(text));
        }
        l
    });

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
            r.games.pick_link = None;
            r.games.pick_place = None;
            cx.notify();
        })));
    }
    // Your windows in it, closed from here (asked first for two or more, as a lane's Close is).
    if !in_now.is_empty() {
        let n = in_now.len();
        let ids = in_now.clone();
        let game_name = name.to_string();
        acts = acts.child(Button::new("gm-stop", format!("{} · {n}", root.tr("card.close"))).icon("power").on_click(cx.listener(move |r, _, window, cx| {
            let ids = ids.clone();
            let go = move |r: &mut Root, _: &mut Context<Root>| r.send("closeAccounts", json!({ "ids": ids, "force": false }));
            if n < 2 {
                return go(r, cx);
            }
            let title = format!("{} {game_name}?", r.tr("card.close"));
            let text = r.i18n.tv("t.closeAllN", &[("n", &n.to_string())]);
            r.confirm(title, text, r.tr("card.close"), true, Some("closeAll"), window, cx, go);
        })));
    }
    if news.is_some() {
        let b = Button::new("gm-follow", root.tr(if follow_on { "news.following" } else { "news.follow" })).icon(if follow_on { "check" } else { "plus" });
        acts = acts.child(if follow_on || place > 0 { b } else { b.primary() }.on_click(cx.listener(move |r, _, _, _| {
            r.send("feedFollow", json!({ "game": game_id, "on": !follow_on }))
        })));
    }
    // One added here (no news to follow): Remove, which drops it from the list (GamesView.tsx's Hero).
    if news.is_none() && let Some(g) = game.filter(|g| g.added) {
        let id = g.id.clone();
        acts = acts.child(Button::new("gm-remove", root.tr("games.remove")).icon("x").quiet().on_click(cx.listener(move |r, _, _, cx| {
            r.send("wikiPick", json!({ "id": id, "site": "" }));
            r.games.open = None;
            cx.notify();
        })));
    }
    // A wiki Bloxkeep knows opens here, on its tab; another is looked for in the browser.
    acts = acts.child(Button::new("gm-wiki", root.tr("gp.wiki")).icon("book").on_click(cx.listener(move |r, _, _, cx| match &wiki {
        Some(_) => {
            r.games.tab = Tab::Wiki;
            cx.notify();
        }
        None => r.send("openGameWiki", json!({ "name": wiki_name })),
    })));
    if place > 0 {
        acts = acts.child(IconButton::new("gm-roblox", "external").size(36.).tip(root.tr("gp.roblox")).on_click(cx.listener(move |r, _, _, _| {
            r.send("openGamePage", json!({ "placeId": place }))
        })));
    }
    // A server's link or Job ID pasted at the row's end, then Join: a friend's private server, a server someone shared.
    let link_row = (place > 0).then(|| root.games.link_box.clone()).flatten().map(|e| {
        let tip: SharedString = root.tr("gp.linkTip").into();
        div()
            .id("gm-link")
            .flex()
            .items_center()
            .gap(px(8.))
            .child(div().w(px(260.)).child(field(&e, Some("link")).h(px(36.))))
            .child(Button::new("gm-link-join", root.tr("gp.linkJoin")).icon("logIn").on_click(cx.listener(|r, _, _, cx| {
                join_link(r, cx);
                cx.notify();
            })))
            .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
    });

    // The game's wide picture whole, beside its name (owner 2026-11-13: the old band cut a 16:9 picture to a strip and
    // darkened it, "khuất"): no crop, no fade, no words on it; the game's colour stands in until it loads.
    let pic_h = h - 32.;
    let pic_w = (pic_h * 16. / 9.).round();
    let picture = look(root, place, "cover").and_then(|u| images::pic_at(&u, pic_w, pic_h, cx));
    let frame = div()
        .w(px(pic_w))
        .h(px(pic_h))
        .flex_none()
        .rounded(px(12.))
        .overflow_hidden()
        .border_1()
        .border_color(c(LINE))
        .when_some(ground, |d, t| d.bg(ca(t, 0.55)))
        .when(cover.is_none() && ground.is_none(), |d| d.bg(ca(WHITE, 0.04)))
        .when_some(picture, |d, p| d.child(img(p).size_full().object_fit(ObjectFit::Cover)));
    div()
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(12.))
        .px(px(PAD + 12.))
        .pt(px(14.))
        .pb(px(4.))
        .child(
            div().flex().child(Button::new("gm-back", root.tr("news.title")).icon("chevronLeft").small().on_click(cx.listener(|r, _, _, cx| {
                r.games.open = None;
                cx.notify();
            }))),
        )
        .child(
            div()
                .flex()
                .items_end()
                .gap(px(24.))
                .child(frame)
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
                                .child(game_icon(badge, name, 52., cx).border_1().border_color(c(LINE)))
                                .child(switcher(root, all, open, name, width, cx)),
                        )
                        .child(line)
                        .children(mine_line)
                        .child(div().mt(px(6.)).flex().flex_wrap().items_center().gap(px(12.)).child(acts).child(div().flex_1()).children(link_row)),
                ),
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
    // (Each figure's mark and colour, for the fuller look: in the cells' order.)
    let mut marks: Vec<(&'static str, u32)> = Vec::new();
    if f("playing") > 0. {
        cells.push((root.tr("gp.playing"), thousands(f("playing") as i64, vi), None));
        marks.push(("users", OK));
    }
    if f("visits") > 0. {
        cells.push((root.tr("gp.visits"), big(root, f("visits")), None));
        marks.push(("eye", ONLINE));
    }
    if f("favorites") > 0. {
        cells.push((root.tr("gp.favorites"), big(root, f("favorites")), None));
        marks.push(("starFill", FACE));
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
        marks.push(("activity", OK));
    }
    if let Some(u) = d.get("updated").and_then(Value::as_str).and_then(|u| ago_iso(root, u)) {
        cells.push((root.tr("gp.updated"), u, None));
        marks.push(("clock", TEXT_2));
    }
    if rich() {
        return surface().flex().flex_wrap().children(cells.into_iter().zip(marks).enumerate().map(|(i, ((label, value, under), (mark, tone)))| {
            let wide = under.is_some();
            div()
                .flex_1()
                .min_w(px(if wide { 250. } else { 180. }))
                .px(px(18.))
                .py(px(16.))
                .flex()
                .items_center()
                .gap(px(14.))
                .when(i > 0, |d| d.border_l_1().border_color(c(LINE)))
                .child(mark_chip(mark, tone, 40.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(div().text_size(px(T_S)).font_weight(FontWeight::MEDIUM).text_color(c(MUTED)).truncate().child(label))
                        .child(div().text_size(px(24.)).font_weight(FontWeight::BOLD).line_height(px(30.)).font_features(tnum()).text_color(c(TEXT)).truncate().child(value))
                        .children(under),
                )
        }));
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
    let mut marks: Vec<&'static str> = Vec::new();
    for (key, value, mark) in [("gp.creator", s(d, "creator").to_string(), "user"), ("gp.genre", s(d, "genre").to_string(), "tag")] {
        if !value.is_empty() {
            rows.push((root.tr(key), value));
            marks.push(mark);
        }
    }
    if let Some(day) = d.get("created").and_then(Value::as_str).and_then(|t| day_of(root, t)) {
        rows.push((root.tr("gp.created"), day));
        marks.push("calendar");
    }
    if int(d, "maxPlayers") > 0 {
        rows.push((root.tr("gp.maxPlayers"), int(d, "maxPlayers").to_string()));
        marks.push("users");
    }
    if place > 0 {
        rows.push((root.tr("gp.placeId"), place.to_string()));
        marks.push("link");
    }
    let head = if rich() { heading_rich("info", TEXT_2, root.tr("gp.info"), None).into_any_element() } else { heading(root.tr("gp.info"), None).into_any_element() };
    surface().p(px(18.)).flex().flex_col().gap(px(if rich() { 10. } else { 6. })).child(head).child(div().flex().flex_col().children(rows.into_iter().zip(marks).enumerate().map(|(i, ((k, v), mark))| {
        div()
            .h(px(38.))
            .flex()
            .items_center()
            .justify_between()
            .gap(px(14.))
            .text_size(px(T_M))
            .when(i > 0, |d| d.border_t_1().border_color(ca(WHITE, 0.05)))
            .when(rich(), |d| d.child(icon(mark, 14., c(DIM))))
            .when(rich(), |d| d.child(div().flex_1().min_w_0().truncate().text_color(c(MUTED)).child(k.clone())))
            .when(!rich(), |d| d.child(div().flex_none().text_color(c(MUTED)).child(k.clone())))
            .child(div().min_w_0().truncate().font_weight(if rich() { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM }).font_features(tnum()).child(v))
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

/// A block's head in the fuller look: its mark, its name, and what the name leaves unsaid as a quiet pill at the right.
fn heading_rich(mark: &'static str, tone: u32, title: String, sub: Option<String>) -> Div {
    let under = sub.as_ref().is_some_and(|s| s.chars().count() > 36);
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .child(game_chip(mark, tone, 32.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(div().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).child(title))
                .when_some(sub.clone().filter(|_| under), |d, s| d.child(div().text_size(px(T_S)).text_color(c(MUTED)).child(s))),
        )
        .when_some(sub.filter(|_| !under), |d, s| d.child(div().h(px(22.)).px(px(9.)).flex().flex_none().items_center().rounded_full().bg(c(INSET)).border_1().border_color(c(LINE)).text_size(px(T_S)).text_color(c(MUTED)).child(s)))
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
            "ingame" if rich() => div().flex().child(tint_pill(root.tr("gp.inGame"), OK)),
            "banned" if rich() => div().flex().child(tint_pill(root.tr("gp.banned"), BAD)),
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

    let yours_head = if rich() { heading_rich("users", ONLINE, root.tr("gp.yours"), Some(root.tr("gp.week"))).into_any_element() } else { heading(root.tr("gp.yours"), Some(root.tr("gp.week"))).into_any_element() };
    let yours = surface().p(px(18.)).flex().flex_col().gap(px(12.)).child(yours_head).child(if accounts.is_empty() {
        div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("gp.noneYet"))
    } else {
        table
    });
    let hush_head = if rich() { heading_rich("moon", FACE, root.tr("gp.quiet"), Some(root.tr("gp.quietFrom"))).into_any_element() } else { heading(root.tr("gp.quiet"), Some(root.tr("gp.quietFrom"))).into_any_element() };
    let hush = surface().p(px(18.)).flex().flex_col().gap(px(16.)).child(hush_head).child(match hours {
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
            seg = seg.child(segment(("news-src", n), on).h(px(26.)).px(px(10.)).text_size(px(T_M)).cursor_pointer().child(title).on_click(cx.listener(move |r, _, _, cx| {
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
    // The newest first, a page of them at a time (a game with hundreds of posts drew them all on every frame).
    let at = format!("{id}|{}", source.as_deref().unwrap_or_default());
    let cap = match root.games.news_cap.borrow().as_ref() {
        Some((k, n)) if *k == at => *n,
        _ => NEWS_ROWS,
    };
    let more = list.len().saturating_sub(cap);
    // In rows, each row's cards as tall as its tallest: dealt into columns, the cards' feet stood at every height and
    // the page read as a jumble (the owner, 2026-11-15).
    let mut rows: Vec<Vec<AnyElement>> = Vec::new();
    for (i, card) in list.into_iter().take(cap).enumerate() {
        if i % cols == 0 {
            rows.push(Vec::new());
        }
        if let Some(row) = rows.last_mut() {
            row.push(news_card(root, card, i, before, w, cx).into_any_element());
        }
    }
    div()
        .pt(px(4.))
        .flex()
        .flex_col()
        .gap(px(GAP))
        .child(tell)
        .children(rows.into_iter().map(|row| div().flex().items_stretch().gap(px(GAP)).children(row)))
        .when(more > 0, |d| {
            d.child(div().flex().justify_center().child(Button::new("news-more", root.tr("alog.more")).small().on_click(cx.listener(move |r, _, _, cx| {
                *r.games.news_cap.borrow_mut() = Some((at.clone(), cap + NEWS_ROWS));
                cx.notify();
            }))))
        })
}

/// News cards drawn at first, and added by each Show more.
const NEWS_ROWS: usize = 30;

/// One message from the game's bot: who sent it and when, its title and words, its fields in columns, its picture.
fn news_card(root: &Root, card: &Value, n: usize, before: f64, w: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let at = card.get("at").and_then(Value::as_f64).unwrap_or(0.);
    let is_new = before.is_finite() && at > before;
    let author = card.pointer("/author/name").and_then(Value::as_str).unwrap_or_default().to_string();
    // (Pictures from Discord's own servers only: one from any host would tell it the viewer's IP.)
    let face = card.pointer("/author/icon").and_then(Value::as_str).filter(|u| super::blocks::discord_media(u)).and_then(|u| images::pic_sq(u, 18., cx));
    let title = plain(root, s(card, "title"));
    // (A card with no id goes by its place in the list: two with the same element id would mix up their clicks.)
    let card_id = Some(s(card, "id")).filter(|x| !x.is_empty()).map_or_else(|| format!("#{n}"), str::to_string);
    let parts: Option<Vec<Value>> = card.get("blocks").and_then(Value::as_array).filter(|l| !l.is_empty()).cloned();
    let blocks = parts.as_ref().map(|l| super::blocks::blocks(root, &card_id, l, &author, card.get("spoiler").and_then(Value::as_bool).unwrap_or(false), w - 34., cx));
    let text = if blocks.is_some() { String::new() } else { plain(root, s(card, "text")) };
    let until = card.get("until").and_then(Value::as_f64).map(|u| (u / 1000.) as i64).filter(|u| *u > crate::time::now_secs());
    let picture = card.get("image").and_then(Value::as_str).filter(|u| super::blocks::discord_media(u) && parts.is_none()).and_then(|u| images::pic(u, cx));
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
        .map(|d| {
            if rich() {
                let (ground, line) = block_tones();
                d.bg(ground).border_1().border_color(if is_new { game_ink().unwrap_or(ca(TEXT, 0.25)) } else { line }).shadow(block_edge())
            } else {
                d.bg(ca(WHITE, 0.045)).border_1().border_color(if is_new { ca(TEXT, 0.25) } else { c(LINE) })
            }
        })
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
    let items: &[Value] = info.get("items").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    let mut kinds: Vec<String> = Vec::new();
    for it in items {
        let k = s(it, "category").to_string();
        if !k.is_empty() && !kinds.contains(&k) {
            kinds.push(k);
        }
    }
    let kind = root.games.kind.clone().filter(|k| kinds.contains(k));
    // The search: every word typed, in the item's name, rarity or kind, accents aside (PriceList.tsx); dearest first.
    let query = root.price_search.read(cx).value().trim().to_string();
    // (Worked out again only when the list, the words or the kind change, not on every frame.)
    let key = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        sig_of(info, &mut h);
        (id, &query, &kind).hash(&mut h);
        h.finish()
    };
    let order = match root.games.price_cache.borrow().as_ref() {
        Some((k, o)) if *k == key => Some(o.clone()),
        _ => None,
    };
    let order = order.unwrap_or_else(|| {
        let q = folded(&query);
        let mut o: Vec<usize> = (0..items.len())
            .filter(|i| kind.as_deref().is_none_or(|k| s(&items[*i], "category") == k))
            .filter(|i| {
                let it = &items[*i];
                let hay = folded(&format!("{} {} {}", s(it, "name"), s(it, "rarity"), s(it, "category")));
                q.split_whitespace().all(|w| hay.contains(w))
            })
            .collect();
        let val = |i: usize| items[i].get("value").and_then(Value::as_f64).unwrap_or(0.);
        o.sort_by(|a, b| val(*b).total_cmp(&val(*a)));
        let o = std::rc::Rc::new(o);
        *root.games.price_cache.borrow_mut() = Some((key, o.clone()));
        o
    });
    // A long list is drawn a page at a time.
    let cap = match root.games.price_cap.get() {
        (k, n) if k == key => n,
        _ => PRICE_ROWS,
    };
    let more = order.len().saturating_sub(cap);
    let shown: Vec<&Value> = order.iter().take(cap).map(|i| &items[*i]).collect();
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
        .when(more > 0, |d| {
            d.child(div().flex().justify_center().child(Button::new("pk-more", root.tr("alog.more")).small().on_click(cx.listener(move |r, _, _, cx| {
                r.games.price_cap.set((key, cap + PRICE_ROWS));
                cx.notify();
            }))))
        })
}

/// Price cards drawn at first, and added by each Show more.
const PRICE_ROWS: usize = 120;
