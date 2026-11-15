// Profile and Marketplace, as the Roblox app has them, inside Games (its header's switch).
// Profile: the account Friends looks through (the same pick, pages/friends.rs): who it is, its friends and followers,
// its Robux (what only its own sign-in gets) and the communities it is in. Marketplace: Roblox's catalog, its front
// shelf or a search (Enter); an item opens on its page here: what it is, which of the app's accounts own it already
// (Roblox tells that of one account at a time), and Buy for the account whose tab is shown, in two clicks, at the price
// shown (roblox::buy); what Roblox sells another way (a resold Limited, an old kind) opens on roblox.com. And, what
// Roblox has not, New free: the free limiteds still in stock, read again every few seconds while it is shown, each
// with the game that gives it (Open game: that game's page here, to start it with the accounts wanted).
// Asked from here (roblox.rs); the pages on roblox.com are opened by the app (openProfile, openRobloxPage).
use gpui_kit::{prelude::*, *};
use serde_json::json;

use crate::{
    images,
    kit::*,
    pages::{
        avatar::tile_said,
        friends,
        games::thousands,
    },
    roblox::{self, Club, Free, Friend, Offer, Profile, Ware},
    shell::Root,
    theme::*,
};

struct Follow {
    user: i64,
    on: Option<bool>,
    asked: bool,
    busy: bool,
}

#[derive(Default)]
pub struct Data {
    /// The Roblox user the profile is of, and the profile (None while it loads).
    of: i64,
    pub got: Option<Result<Profile, String>>,
    loading: bool,
    /// The tickets (avatar::ticket) of the profile's read and the Marketplace's page under way: an answer with
    /// another is old.
    load_ask: u64,
    page_ask: u64,
    /// The Marketplace: what was asked ("" the front shelf), what it holds so far (None while its first page loads),
    /// the mark for its next page, and whether a page is being read.
    asked: Option<String>,
    wares: Option<Result<Vec<Ware>, String>>,
    next: String,
    reading: bool,
    /// The Marketplace's filters (roblox.com's catalog has them): the kind of thing (CATS) and the order (SORTS); the
    /// order's list open, and when a click outside closed it (so a click on its own button only closes it).
    cat: usize,
    sort: usize,
    sort_open: bool,
    sort_closed: Option<std::time::Instant>,
    /// New free, shown in place of the catalog: the free limiteds in stock (None while the first read runs), when
    /// they were read, whether a read is under way, the ones seen on the first read (None before it) and the ones
    /// that came after it, which carry a New mark.
    pub free: bool,
    /// Watching, shown in place of the catalog (itemwatch.rs): the list as last read from disk.
    pub watch_view: bool,
    pub watch: Vec<crate::itemwatch::Watched>,
    pub watch_read: bool,
    frees: Option<Result<Vec<Free>, String>>,
    free_at: Option<std::time::Instant>,
    free_reading: bool,
    free_known: Option<std::collections::HashSet<i64>>,
    free_fresh: std::collections::HashSet<i64>,
    /// A community opened from the profile, shown in its place until Back.
    club: Option<ClubView>,
    /// A thing of the Marketplace opened from its tile, shown in the catalog's place until Back.
    item: Option<ItemView>,
    /// The followers or the followed, opened from their count: which ("followers", "followings"), and the people
    /// (None while they load). Shown in the profile's place until Back.
    folk: Option<(&'static str, Option<Result<Vec<Friend>, String>>)>,
    /// Someone else's profile (a friend's, from their card): their Roblox id and the profile (None while it loads).
    /// Shown in the profile's place until Back; read without a sign-in, so it has no Robux and no badges.
    person: Option<(i64, Option<Result<Profile, String>>)>,
    /// Whether the account looked through follows that person: who, Roblox's answer (None until it comes; the button
    /// shows once it has), whether it was asked, and whether a move of the button is under way.
    follow: Option<Follow>,
    /// Purchases under way and bought, by account (its Roblox id), thing and whether it is a bundle: kept when the thing
    /// is closed (Back), so an answer that comes meanwhile isn't lost and Buy doesn't come back while one is under way.
    buying: Vec<(i64, i64, bool)>,
    bought: Vec<(i64, i64, bool)>,
}

/// The thing shown: what its maker sells it for (None while that is asked), the account the rest was asked for (its
/// Roblox id; 0: not asked yet), that account's Robux, the app's accounts that own it (None while asked), and the
/// buying: armed by the first click, under way, and how it ended.
struct ItemView {
    ware: Ware,
    offer: Option<Result<Offer, String>>,
    me: i64,
    robux: Option<i64>,
    owners: Option<Vec<i64>>,
    /// Whether the account whose tab is shown owns it (None while asked): asked alone and first, so Buy needn't wait
    /// for every account's answer (the owner waited seconds on "Get for free" with 8 accounts, 2026-11-12).
    mine: Option<bool>,
    armed: bool,
    busy: bool,
    done: Option<Result<(), String>>,
    /// Try on: the account wearing it, as a picture (None until asked; Err: why not), whether it is being drawn, and
    /// whether the picture shows it on the account (else the thing itself).
    tried: Option<Result<String, String>>,
    trying: bool,
    on_me: bool,
    /// On the Watching list.
    watched: bool,
}

/// The community shown: its id and name (known before it loads), its page (None while it loads), and which of the
/// app's accounts are in it, each with its rank's name (None while that is asked).
struct ClubView {
    id: i64,
    name: String,
    got: Option<Result<Club, String>>,
    mine: Option<Vec<(String, String)>>,
}

/// The profile's share of Data, which goes away with its account's tab (pages/tabs.rs); the Marketplace's stays, being
/// no account's.
pub type Kept = (i64, Option<Result<Profile, String>>);

impl Data {
    /// Which page stands in the profile's or the Marketplace's place, as a name (shell.rs: a place steps in when what
    /// it shows changes).
    pub fn part(&self) -> String {
        match (&self.club, &self.item, &self.folk) {
            (Some(c), _, _) => format!("club{}", c.id),
            _ if self.person.is_some() => format!("person{}", self.person.as_ref().map(|p| p.0).unwrap_or(0)),
            (_, Some(i), _) => format!("item{}", i.ware.id),
            (_, _, Some(f)) => f.0.to_string(),
            _ => if self.free { "free".into() } else { String::new() },
        }
    }

    pub fn take_profile(&mut self) -> Kept {
        self.loading = false;
        self.folk = None;
        self.person = None;
        (std::mem::take(&mut self.of), self.got.take())
    }

    pub fn put_profile(&mut self, kept: Kept) {
        self.of = kept.0;
        self.got = kept.1;
        self.loading = false;
    }
}

/// A Marketplace tile's picture is about this wide. A tile carries its own padding (the ground that comes up under the
/// mouse), so tiles stand this close and their row is reckoned 6 px wider each side.
const TILE: f32 = 164.;
const SGAP: f32 = 2.;
const GAP: f32 = 14.;
/// How old New free's list may get while it is shown.
const FREE_FRESH: std::time::Duration = std::time::Duration::from_secs(10);

// ───────────────────────── profile ─────────────────────────

pub fn want(r: &mut Root, link: &crate::link::Link, cx: &mut Context<Root>) {
    friends::want_sign(r, link, cx);
    let Some((_, user, _)) = friends::signed(r) else { return };
    // Another account: the last one's profile (and what is on its way for it) goes at once, even mid-load.
    if r.profile.of != user {
        r.profile.of = user;
        r.profile.got = None;
        r.profile.loading = false;
        r.profile.folk = None;
        r.profile.person = None;
    }
    if r.profile.got.is_none() && !r.profile.loading {
        load(r, cx);
    }
    // (A person opened before the sign-in was in: whether they are followed is asked now.)
    ask_follow(r, cx);
}

/// Asks Roblox, once, whether the account looked through follows the person whose profile is open (never itself).
fn ask_follow(r: &mut Root, cx: &mut Context<Root>) {
    let Some((_, me, cookie)) = friends::signed(r).filter(|s| !s.2.is_empty() || cfg!(feature = "shot")) else { return };
    let Some(f) = r.profile.follow.as_mut().filter(|f| !f.asked && f.user != me) else { return };
    f.asked = true;
    let user = f.user;
    let rx = roblox::run(move || roblox::following(&cookie, user));
    cx.spawn(async move |this, cx| {
        if let Ok(Ok(on)) = rx.await {
            let _ = this.update(cx, |r, cx| {
                if let Some(f) = r.profile.follow.as_mut().filter(|f| f.user == user) {
                    f.on = Some(on);
                    cx.notify();
                }
            });
        }
    })
    .detach();
}

/// Follows the person whose profile is open, or stops, as the account looked through (the app's friendAct: a check
/// Roblox holds it for is shown to solve). Refused, the button stays as it was and a line says so.
fn follow_move(r: &mut Root, user: i64, on: bool, cx: &mut Context<Root>) {
    let (Some(link), Some((id, _, _))) = (r.link.clone(), friends::signed(r)) else { return };
    let Some(f) = r.profile.follow.as_mut().filter(|f| f.user == user && !f.busy) else { return };
    f.busy = true;
    cx.notify();
    let rx = link.call("friendAct", json!({ "id": id, "userId": user, "act": if on { "follow" } else { "unfollow" } }));
    cx.spawn(async move |this, cx| {
        let got = rx.await.ok().and_then(Result::ok);
        let ok = got.as_ref().and_then(|v| v.get("ok")).and_then(serde_json::Value::as_bool).unwrap_or(false);
        let held = got.as_ref().and_then(|v| v.get("held")).and_then(serde_json::Value::as_bool).unwrap_or(false);
        let _ = this.update(cx, |r, cx| {
            if let Some(f) = r.profile.follow.as_mut().filter(|f| f.user == user) {
                f.busy = false;
                if ok {
                    f.on = Some(on);
                }
            }
            if !ok && !held {
                r.toast = Some((r.tr("pf.followNo"), "bad".into()));
            }
            cx.notify();
        });
    })
    .detach();
}

fn load(r: &mut Root, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = friends::signed(r) else { return };
    r.profile.loading = true;
    let ask = crate::pages::avatar::ticket();
    r.profile.load_ask = ask;
    let rx = roblox::run(move || roblox::profile(&cookie, user));
    cx.spawn(async move |this, cx| {
        let got = rx.await.unwrap_or_else(|_| Err(String::new()));
        let _ = this.update(cx, |r, cx| {
            // Overtaken (another account, or asked again since): dropped.
            if r.profile.load_ask != ask || r.profile.of != user {
                return;
            }
            r.profile.loading = false;
            r.profile.got = Some(got);
            cx.notify();
        });
    })
    .detach();
}

/// Opens a Roblox user's profile in the Roblox tab's Hồ sơ (a friend's, from their card), over the account's own.
pub fn open_person(r: &mut Root, user: i64, cx: &mut Context<Root>) {
    if user <= 0 {
        return;
    }
    r.profile.club = None;
    r.profile.folk = None;
    r.profile.person = Some((user, None));
    r.profile.follow = Some(Follow { user, on: None, asked: false, busy: false });
    ask_follow(r, cx);
    r.games.view = crate::pages::games::View::Profile;
    let rx = roblox::run(move || roblox::profile("", user));
    cx.spawn(async move |this, cx| {
        let got = rx.await.unwrap_or_else(|_| Err(String::new()));
        let _ = this.update(cx, |r, cx| {
            if let Some(p) = r.profile.person.as_mut().filter(|p| p.0 == user) {
                p.1 = Some(got);
                cx.notify();
            }
        });
    })
    .detach();
    cx.notify();
}

fn again(r: &mut Root, cx: &mut Context<Root>) {
    if !r.profile.loading {
        if r.profile.got.as_ref().is_some_and(|g| g.is_err()) {
            r.profile.got = None;
        }
        load(r, cx);
        cx.notify();
    }
}

fn note(text: String) -> Div {
    div().flex_1().flex().items_center().justify_center().text_size(px(T_M)).text_color(c(MUTED)).child(text)
}

/// 2006-03-08T… as 08/03/2006.
pub fn day(iso: &str) -> String {
    let d: Vec<&str> = iso.get(..10).unwrap_or_default().split('-').collect();
    if d.len() == 3 { format!("{}/{}/{}", d[2], d[1], d[0]) } else { String::new() }
}

/// Opens the followers or the followed of the account whose tab is shown, in the profile's place.
fn open_folk(r: &mut Root, which: &'static str, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = friends::signed(r) else { return };
    r.profile.folk = Some((which, None));
    let rx = roblox::run(move || roblox::follows(&cookie, user, which));
    cx.spawn(async move |this, cx| {
        let got = rx.await.unwrap_or_else(|_| Err(String::new()));
        let _ = this.update(cx, |r, cx| {
            if r.profile.of == user {
                if let Some(f) = r.profile.folk.as_mut().filter(|f| f.0 == which) {
                    f.1 = Some(got);
                    cx.notify();
                }
            }
        });
    })
    .detach();
}

/// The followers or the followed: each a card that opens their profile on roblox.com.
fn folk_page(root: &Root, which: &str, got: &Option<Result<Vec<Friend>, String>>, width: f32, cx: &mut Context<Root>) -> AnyElement {
    let inner = width - 2. * (PAD + 4.);
    let cols = ((inner + GAP) / (260. + GAP)).floor().max(1.);
    let card_w = ((inner - GAP * (cols - 1.)) / cols).floor();
    let title = root.tr(if which == "followers" { "pf.followers" } else { "pf.following" });
    let back = div()
        .flex_none()
        .px(px(PAD + 4.))
        .pb(px(14.))
        .flex()
        .items_center()
        .gap(px(14.))
        .child(Button::new("folk-back", root.tr("nav.profile")).icon("chevronLeft").small().on_click(cx.listener(|r, _, _, cx| {
            r.profile.folk = None;
            cx.notify();
        })))
        .child(div().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).child(match got {
            Some(Ok(l)) => format!("{title} · {}", l.len()),
            _ => title,
        }));
    let body: AnyElement = match got {
        None => note(root.tr("av.reading")).into_any_element(),
        Some(Err(why)) => note(root.i18n.tv("in.failed", &[("why", if why.is_empty() { "Roblox" } else { why })])).into_any_element(),
        Some(Ok(l)) if l.is_empty() => note(root.tr("pf.nobody")).into_any_element(),
        Some(Ok(l)) => {
            let cards: Vec<AnyElement> = l
                .iter()
                .enumerate()
                .map(|(n, f)| {
                    let user = f.id;
                    let face = (!f.face.is_empty()).then(|| images::pic_sq(&f.face, 38., cx)).flatten();
                    hot(("folk", n))
                        .w(px(card_w))
                        .h(px(60.))
                        .px(px(12.))
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .rounded(px(R_L))
                        .bg(ca(WHITE, 0.045))
                        .border_1()
                        .border_color(c(LINE))
                        .cursor_pointer()
                        .on_click(cx.listener(move |r, _, _, _| r.send("openProfile", json!({ "userId": user }))))
                        .child(div().relative().size(px(38.)).flex_none().rounded_full().bg(c(RAISED_2)).overflow_hidden().when_some(face, |d, p| d.child(img(p).absolute().inset_0().size_full().rounded_full())))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(div().truncate().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).child(f.display.clone()))
                                .when(!f.name.is_empty(), |d| d.child(div().truncate().text_size(px(T_S)).text_color(c(MUTED)).child(format!("@{}", f.name)))),
                        )
                        .child(icon("external", 14., c(DIM)))
                        .into_any_element()
                })
                .collect();
            crate::scrolled::body("folk-body", div().px(px(PAD + 4.)).pb(px(24.)).flex().flex_wrap().gap(px(GAP)).children(cards)).into_any_element()
        }
    };
    div().flex_1().min_h_0().flex().flex_col().child(back).child(body).into_any_element()
}

/// Opens a community's page in the profile's place: its page is read, then which of the app's accounts are in it.
fn open_club(r: &mut Root, id: i64, name: String, cx: &mut Context<Root>) {
    r.profile.club = Some(ClubView { id, name, got: None, mine: None });
    // Every account with a Roblox id, whatever its sign-in's state: membership is public.
    let accounts: Vec<(i64, String)> = r
        .accounts()
        .iter()
        .filter_map(|a| {
            let user = a.get("userId").and_then(serde_json::Value::as_i64).filter(|u| *u > 0)?;
            let name = ["label", "alias", "displayName", "username"].iter().filter_map(|k| a.get(*k).and_then(serde_json::Value::as_str)).find(|n| !n.is_empty()).unwrap_or_default().to_string();
            Some((user, name))
        })
        .collect();
    let rx = roblox::run(move || roblox::club(id));
    cx.spawn(async move |this, cx| {
        let got = rx.await.unwrap_or_else(|_| Err(String::new()));
        let shown = this.update(cx, |r, cx| match &mut r.profile.club {
            Some(c) if c.id == id => {
                c.got = Some(got);
                cx.notify();
                true
            }
            _ => false,
        });
        if !matches!(shown, Ok(true)) {
            return;
        }
        let users: Vec<i64> = accounts.iter().map(|a| a.0).collect();
        let mine = roblox::run(move || roblox::club_mine(id, &users)).await.unwrap_or_default();
        let _ = this.update(cx, |r, cx| {
            if let Some(c) = r.profile.club.as_mut().filter(|c| c.id == id) {
                c.mine = Some(mine.into_iter().map(|(user, role)| (accounts.iter().find(|a| a.0 == user).map(|a| a.1.clone()).unwrap_or_default(), role)).collect());
                cx.notify();
            }
        });
    })
    .detach();
}

/// A game's own colour for its letter tile, from its place ID (the vault faces' colours, lock.rs, with indigo for
/// their red, which says trouble here).
fn letter_colour(place: i64) -> u32 {
    const COLORS: [u32; 8] = [0x2563eb, 0x15803d, 0xb45309, 0x4f46e5, 0x7c3aed, 0x0e7490, 0xbe185d, 0x0f766e];
    let mut h = place.unsigned_abs().wrapping_mul(0x9e37_79b9_7f4a_7c15);
    h ^= h >> 29;
    COLORS[(h % 8) as usize]
}

/// Games as a board of their wide pictures (the Games tab's board, games.rs): the picture, the name, how often it was
/// played. A press opens the game's page here. `inner` is the width the board has.
fn game_tiles(root: &Root, games: &[roblox::ClubGame], inner: f32, key: &'static str, cx: &mut Context<Root>) -> Vec<AnyElement> {
    const PADT: f32 = 6.;
    const GAPT: f32 = 6.;
    if games.is_empty() {
        return Vec::new();
    }
    let vi = root.i18n.vietnamese;
    let outer = inner + 2. * PADT;
    let cols = ((outer + GAPT) / (232. + 2. * PADT + GAPT)).floor().max(2.);
    let w = ((outer - GAPT * (cols - 1.)) / cols).floor();
    let art_h = ((w - 2. * PADT) * 9. / 16.).round();
    let tiles: Vec<AnyElement> = games
        .iter()
        .enumerate()
        .map(|(n, g)| {
            let (place, name) = (g.place, g.name.clone());
            // Its wide picture at the size it is drawn; a game with none (Roblox's grey stand-in is left out by
            // roblox.rs) shows its icon on the block, else its name; one that couldn't be had says so, quietly.
            let art_w = w - 2. * PADT;
            let wide = !g.cover.is_empty();
            let pic = if wide { images::pic_pair(&g.cover, art_w, art_h, cx) } else { (!g.pic.is_empty()).then(|| images::pic_pair(&g.pic, art_h * 0.6, art_h * 0.6, cx)).flatten() };
            let lost = { let u = if wide { &g.cover } else { &g.pic }; !u.is_empty() && images::failed(u, cx) };
            let lost_text = root.tr("news.noPicture");
            hot((key, n))
                .w(px(w))
                .flex_none()
                .p(px(PADT))
                .flex()
                .flex_col()
                .rounded(px(R_L + 4.))
                .cursor_pointer()
                .lift(2.)
                .on_click(cx.listener(move |r, _, _, cx| {
                    open_game(r, place, &name);
                    cx.notify();
                }))
                .child(
                    div()
                        .w_full()
                        .h(px(art_h))
                        .flex_none()
                        .rounded(px(R_L))
                        .overflow_hidden()
                        .bg(c(RAISED))
                        .border_1()
                        .border_color(c(LINE))
                        .flex()
                        .items_center()
                        .justify_center()
                        .map(|d| match pic {
                            Some((src, key)) if wide => d.child(fade_in(img(src).size_full().rounded(px(R_L)).object_fit(ObjectFit::Cover), &key)),
                            Some((src, key)) => d.child(fade_in(img(src).size(px(art_h * 0.6)).rounded(px(R_M)).object_fit(ObjectFit::Cover), &key)),
                            None if lost => d.gap(px(6.)).text_size(px(T_S)).text_color(c(MUTED)).child(icon("image", 13., c(MUTED))).child(lost_text),
                            // No picture at all (old games Roblox has none for): its letter on a colour of its own,
                            // the same one every time, as a vault's face, so it reads as the game's tile and not as a
                            // picture that broke (review 3e.1: grey letters on grey looked like failed thumbnails).
                            // Its whole name on the block, not its first letter: two games starting with "T" side by
                            // side looked like one shown twice (review 3e.2).
                            None => {
                                let col = letter_colour(place);
                                d.bg(tinted(RAISED, col, 0.3))
                                    .border_color(ca(col, 0.35))
                                    .flex_col()
                                    .gap(px(8.))
                                    .px(px(18.))
                                    .child(icon("gamepad", 22., mix(c(col), c(WHITE), 0.45)))
                                    .child(div().max_h(px(44.)).overflow_hidden().text_center().text_size(px(T_L)).line_height(px(22.)).font_weight(FontWeight::SEMIBOLD).text_color(mix(c(col), c(WHITE), 0.6)).child(g.name.clone()))
                            }
                        }),
                )
                .child(div().mt(px(8.)).h(px(18.)).flex_none().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).line_height(px(18.)).truncate().child(g.name.clone()))
                .child(div().mt(px(3.)).h(px(15.)).flex_none().text_size(px(T_S)).line_height(px(15.)).text_color(c(MUTED)).font_features(tnum()).child(root.i18n.tv("club.visits", &[("n", &thousands(g.visits, vi))])))
                .into_any_element()
        })
        .collect();
    // One thing for the shelf it stands on: the board, its pictures in line with the shelf's title.
    vec![div().ml(px(-PADT)).w(px(outer)).flex_none().flex().flex_wrap().gap(px(GAPT)).children(tiles).into_any_element()]
}

/// BLOXKEEP_SHOT_CLUB=34216693: a hidden shot's profile opens on that community; BLOXKEEP_SHOT_FOLK=followers: on
/// that list.
#[cfg(feature = "shot")]
pub fn shot_club(r: &mut Root, cx: &mut Context<Root>) {
    if let (None, Ok(which), true) = (&r.profile.folk, std::env::var("BLOXKEEP_SHOT_FOLK"), friends::signed(r).is_some()) {
        open_folk(r, if which == "followings" { "followings" } else { "followers" }, cx);
    }
    if let (None, Some(id)) = (&r.profile.club, std::env::var("BLOXKEEP_SHOT_CLUB").ok().and_then(|v| v.parse().ok())) {
        open_club(r, id, String::new(), cx);
    }
    if let (None, Some(id)) = (&r.profile.person, std::env::var("BLOXKEEP_SHOT_PERSON").ok().and_then(|v| v.parse().ok())) {
        open_person(r, id, cx);
    }
}

/// A community's page: what it is, which of the app's accounts are in it (Roblox's page tells that of one account at
/// a time), the games it made (Open game: the game's page here) and its ranks.
fn club_page(root: &Root, view: &ClubView, width: f32, cx: &mut Context<Root>) -> AnyElement {
    let vi = root.i18n.vietnamese;
    let id = view.id;
    let inner = width - 2. * (PAD + 4.);
    let club = match &view.got {
        None => return div().flex_1().flex().flex_col().child(note(root.tr("av.reading"))).into_any_element(),
        Some(Err(why)) => return div().flex_1().flex().flex_col().child(note(root.i18n.tv("club.failed", &[("why", if why.is_empty() { "Roblox" } else { why })]))).into_any_element(),
        Some(Ok(c)) => c,
    };
    let pic = (!club.pic.is_empty()).then(|| images::pic_sq(&club.pic, 72., cx)).flatten();
    let title = |key: &str| div().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).child(root.tr(key));
    let who = surface()
        .p(px(16.))
        .flex()
        .items_start()
        .gap(px(16.))
        .child(div().relative().size(px(72.)).flex_none().rounded(px(R_L)).bg(c(RAISED_2)).overflow_hidden().when_some(pic, |d, f| d.child(img(f).absolute().inset_0().size_full().object_fit(ObjectFit::Cover))))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(div().text_size(px(T_TITLE)).font_weight(FontWeight::BOLD).truncate().child(club.name.clone()))
                .child(div().text_size(px(T_M)).text_color(c(MUTED)).truncate().child(if club.owner.is_empty() { root.i18n.tv("pf.members", &[("n", &thousands(club.members, vi))]) } else { format!("@{} · {}", club.owner, root.i18n.tv("pf.members", &[("n", &thousands(club.members, vi))])) }))
                .when(!club.about.is_empty(), |d| d.child(div().max_h(px(80.)).overflow_hidden().text_size(px(T_M)).text_color(c(TEXT_2)).line_height(px(20.)).child(club.about.clone())))
                .when(!club.shout.is_empty(), |d| d.child(div().mt(px(4.)).px(px(12.)).py(px(9.)).rounded(px(R_M)).bg(c(INSET)).text_size(px(T_M)).text_color(c(TEXT)).line_height(px(20.)).child(club.shout.clone()))),
        )
        .child(IconButton::new("club-open", "external").size(34.).tip(root.tr("shop.open")).on_click(cx.listener(move |r, _, _, _| r.send("openRobloxPage", json!({ "kind": "group", "id": id })))));

    let mine: AnyElement = match &view.mine {
        None => div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("av.reading")).into_any_element(),
        Some(l) if l.is_empty() => div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("club.mineNone")).into_any_element(),
        Some(l) => div()
            .flex()
            .flex_wrap()
            .gap(px(8.))
            .children(l.iter().map(|(name, role)| {
                div()
                    .h(px(34.))
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .rounded(px(R_M))
                    .bg(ca(WHITE, 0.04))
                    .border_1()
                    .border_color(c(LINE))
                    .child(div().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).child(name.clone()))
                    .child(div().text_size(px(T_S)).text_color(c(MUTED)).child(role.clone()))
            }))
            .into_any_element(),
    };

    let games = game_tiles(root, &club.games, inner, "club-game", cx);

    let roles = div().flex().flex_col().children(club.roles.iter().map(|(name, count)| {
        div()
            .h(px(38.))
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(c(LINE))
            .child(div().text_size(px(T_M)).text_color(c(TEXT_2)).truncate().child(name.clone()))
            .child(div().text_size(px(T_M)).text_color(c(MUTED)).font_features(tnum()).child(thousands(*count, vi)))
    }));

    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .child(
            crate::scrolled::body("club-body", 
                div()
                    .px(px(PAD + 4.))
                    .pb(px(24.))
                    .flex()
                    .flex_col()
                    .gap(px(26.))
                    .child(who)
                    .child(div().flex().flex_col().gap(px(12.)).child(title("club.mine")).child(mine))
                    .when(!games.is_empty(), |d| d.child(div().flex().flex_col().gap(px(12.)).child(title("news.title")).child(div().flex().flex_wrap().gap(px(GAP)).children(games))))
                    .when(!club.roles.is_empty(), |d| d.child(div().flex().flex_col().gap(px(6.)).child(title("club.roles")).child(roles))),
            ),
        )
        .into_any_element()
}

pub fn profile(root: &Root, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let pf = &root.profile;
    let (able, viewer) = friends::looking_ref(root);
    // Roblox's own page of what is shown: the person's when one is open.
    let user = pf.person.as_ref().map(|p| p.0).unwrap_or(pf.of);
    let shell = div().id("profile").relative().size_full().rounded(px(R_PANEL)).bg(c(PANEL)).overflow_hidden().flex().flex_col();
    let header = div()
        .flex_none()
        .px(px(PAD + 4.))
        .pt(px(18.))
        .pb(px(18.))
        .flex()
        .items_center()
        .gap(px(10.))
        .child(crate::pages::games::brand(root))
        // Back, beside the logo while a person's page or a community's is over the profile (the owner's mock-up,
        // 2026-11-15: it was a row of its own under the header, pushing the page down).
        .when(pf.person.is_some() || pf.club.is_some(), |d| {
            d.child(Button::new("pf-back", root.tr("wiki.back")).icon("chevronLeft").small().on_click(cx.listener(|r, _, _, cx| {
                // (The community first: it may have been opened from the person's page.)
                if r.profile.club.is_some() {
                    r.profile.club = None;
                } else {
                    r.profile.person = None;
                }
                cx.notify();
            })))
        })
        .relative().child(crate::pages::games::island(root, cx))
        .child(div().flex_1())
        // (A community's page has its own way out to roblox.com, and nothing of the profile's to read again.)
        .when(viewer.is_some() && pf.club.is_none() && pf.folk.is_none() && pf.person.is_none(), |d| {
            d.child(IconButton::new("pf-refresh", "refresh").size(34.).busy(pf.loading).tip(root.tr("fr.refresh")).on_click(cx.listener(|r, _, _, cx| again(r, cx))))
        })
        // A person's profile: follow them, or no longer (shown once Roblox said which it is).
        .when_some(pf.follow.as_ref().filter(|f| pf.person.as_ref().is_some_and(|p| p.0 == f.user && matches!(p.1, Some(Ok(_))))).and_then(|f| f.on.map(|on| (f.user, on, f.busy))), |d, (who, on, busy)| {
            d.child(
                Button::new("pf-follow", root.tr(if on { "pf.followed" } else { "pf.follow" }))
                    .icon(if on { "check" } else { "plus" })
                    .small()
                    .disabled(busy)
                    .tip(root.tr(if on { "pf.unfollowTip" } else { "pf.followTip" }))
                    .on_click(cx.listener(move |r, _, _, cx| follow_move(r, who, !on, cx))),
            )
        })
        .when((matches!(pf.got, Some(Ok(_))) || matches!(pf.person, Some((_, Some(Ok(_)))))) && pf.club.is_none() && pf.folk.is_none(), |d| {
            d.child(IconButton::new("pf-open", "external").size(34.).tip(root.tr("shop.open")).on_click(cx.listener(move |r, _, _, _| r.send("openProfile", json!({ "userId": user })))))
        });

    let body: AnyElement = if let Some(view) = &pf.club {
        club_page(root, view, width, cx)
    } else if let Some((which, got)) = &pf.folk {
        folk_page(root, which, got, width, cx)
    } else if let Some((_, got)) = &pf.person {
        let content = match got {
            None => note(root.tr("pf.loading")).into_any_element(),
            Some(Err(why)) => note(root.i18n.tv("pf.failed", &[("why", if why.is_empty() { "Roblox" } else { why })])).into_any_element(),
            Some(Ok(p)) => filled(root, p, user, width, false, cx).into_any_element(),
        };
        div().flex_1().min_h_0().flex().flex_col().child(content).into_any_element()
    } else if able.is_empty() || friends::nobody(root) {
        friends::need_account(root, "pf-add-acc", "pf.noAccount", "user", cx)
    } else {
        match &pf.got {
            None => note(root.tr("pf.loading")).into_any_element(),
            Some(Err(why)) => div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(14.))
                .child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.i18n.tv("pf.failed", &[("why", if why.is_empty() { "Roblox" } else { why })])))
                .child(Button::new("pf-retry", root.tr("fr.retry")).icon("refresh").on_click(cx.listener(|r, _, _, cx| again(r, cx))))
                .into_any_element(),
            Some(Ok(p)) => filled(root, p, pf.of, width, true, cx).into_any_element(),
        }
    };
    shell.child(header).child(body)
}

/// A profile: the account's own (`own`: its follow counts open their lists, its Robux shows), or someone else's.
fn filled(root: &Root, p: &Profile, user: i64, width: f32, own: bool, cx: &mut Context<Root>) -> impl IntoElement {
    let vi = root.i18n.vietnamese;
    let inner = width - 2. * (PAD + 4.);
    // (Wide enough for a rank and a member count side by side: "ROBLOX Staff · 303,126 members".)
    let (_, card_w) = grid_fit(inner, 360., GRID_GAP);
    let figure = |label: String, value: String| div().flex().flex_col().gap(px(2.)).child(div().text_size(px(T_S)).text_color(c(MUTED)).child(label)).child(div().text_size(px(T_FIG)).line_height(px(28.)).font_weight(FontWeight::SEMIBOLD).font_features(tnum()).child(value));

    // At the right of a wide header, what Roblox's page says of them in facts: how long they have been on Roblox, their
    // Roblox ID, the visits to the games they made, the communities they are in.
    let mut known: Vec<(String, String)> = Vec::new();
    if let Some(at) = crate::time::iso_secs(&p.joined) {
        let days = ((crate::time::now_secs() - at) / 86_400).max(0);
        let age = if days >= 365 { root.i18n.tn("pf.years", days / 365, &[]) } else { root.i18n.tn("pf.months", (days / 30).max(1), &[]) };
        known.push((root.tr("pf.age"), age));
    }
    if user > 0 {
        known.push((root.tr("pf.id"), user.to_string()));
    }
    if !p.made.is_empty() {
        known.push((root.tr("pf.visits"), thousands(p.made.iter().map(|g| g.visits).sum(), vi)));
    }
    known.push((root.tr("pf.groups"), p.groups.len().to_string()));
    // (The facts take the rest of the header's width, spread over it: at a fixed 380 px at the far right they left
    // some 600 px empty between them and the numbers, review 3e.1.)
    let wide_head = inner >= 1000.;
    // (Two by two, beside the numbers, behind a hairline: spread in one row over the rest of the width they read as
    // a second header; the owner's mock-up, 2026-11-15.)
    let side = wide_head.then(|| {
        div()
            .flex_none()
            .w(px(440.))
            .self_stretch()
            .pl(px(40.))
            .border_l_1()
            .border_color(c(LINE))
            .flex()
            .flex_wrap()
            .content_center()
            .gap_x(px(20.))
            .gap_y(px(18.))
            .children(known.into_iter().map(|(name, value)| {
                div()
                    .w(px(180.))
                    .flex_none()
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .child(div().text_size(px(T_S)).text_color(c(MUTED)).child(name))
                    .child(div().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).font_features(tnum()).truncate().child(value))
            }))
    });
    // A banner on top (the owner's reference, 2026-11-13): a game they like, sharp, edge to edge; their face in a big
    // ring over its foot; their words in a bubble beside it; under it a greeting with their name, then the figures.
    const BANNER: f32 = 190.;
    const AV: f32 = 148.;
    // The games they like side by side, each near its own size (one game's cover stretched across the whole width
    // came out smeared, 2026-11-13); with none, the page's game.
    let mut covers: Vec<String> = p.favs.iter().chain(p.made.iter()).map(|g| g.cover.clone()).filter(|u| !u.is_empty()).collect();
    covers.dedup();
    if covers.is_empty() {
        covers.extend(crate::pages::home::wash(root, cx).map(|(u, _)| u));
    }
    let k = covers.len().min((inner / (BANNER * 16. / 9.)).ceil().max(1.) as usize).max(1);
    let tile_w = ((inner - 2. * (k as f32 - 1.)) / k as f32).floor();
    let banner: Vec<AnyElement> = covers
        .iter()
        .take(k)
        .map(|u| {
            div()
                .flex_1()
                .h_full()
                .overflow_hidden()
                .bg(c(RAISED_2))
                .children(images::pic_pair(u, tile_w, BANNER, cx).map(|(src, key)| fade_in(img(src).size_full().object_fit(ObjectFit::Cover), &key).into_any_element()))
                .into_any_element()
        })
        .collect();
    let face = [&p.face, &p.body].into_iter().find(|u| !u.is_empty()).and_then(|u| images::pic_pair(u, AV, AV, cx));
    let hour = ((crate::time::now_secs() + crate::time::utc_offset_secs()).rem_euclid(86_400)) / 3600;
    let hello = root.tr(match hour {
        5..=11 => "pf.hiMorning",
        12..=17 => "pf.hiAfternoon",
        _ => "pf.hiEvening",
    });
    let title = div()
        .flex()
        .items_baseline()
        .min_w_0()
        .text_size(px(T_STAGE))
        .font_weight(FontWeight::BOLD)
        .line_height(px(36.))
        .when(own, |d| d.child(div().flex_none().whitespace_nowrap().child(hello)))
        .child(div().truncate().when(own, |d| d.text_color(c(FACE))).child(p.display.clone()));
    // Their "About", its first line, under their name (Roblox has no status any more; this is theirs).
    let about = p.about.lines().map(str::trim).find(|l| !l.is_empty()).map(str::to_string);
    let words = div()
        .when(wide_head, |d| d.flex_none().w(px(560.)))
        .when(!wide_head, |d| d.flex_1())
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(10.))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(title)
                .child(div().text_size(px(T_M)).text_color(c(MUTED)).child(format!("@{} · {}", p.name, root.i18n.tv("pf.joined", &[("d", &day(&p.joined))]))))
                .when_some(about, |d, t| d.child(div().pt(px(4.)).text_size(px(T_M)).text_color(c(TEXT_2)).truncate().child(t))),
        )
        .child(
            div()
                .pt(px(4.))
                .flex()
                .flex_wrap()
                .gap_x(px(40.))
                .gap_y(px(10.))
                // (Roblox counts someone else's friends only for a signed-in ask: 0 is left out.)
                .when(own || p.friends > 0, |d| d.child(figure(root.tr("nav.friends"), thousands(p.friends, vi))))
                // (The account's two counts open their lists.)
                .when(own, |d| {
                    d.child(focusable(div().id("pf-followers")).cursor_pointer().rounded(px(R_S)).hover(|d| d.opacity(0.8)).on_click(cx.listener(|r, _, _, cx| open_folk(r, "followers", cx))).child(figure(root.tr("pf.followers"), thousands(p.followers, vi))))
                        .child(focusable(div().id("pf-following")).cursor_pointer().rounded(px(R_S)).hover(|d| d.opacity(0.8)).on_click(cx.listener(|r, _, _, cx| open_folk(r, "followings", cx))).child(figure(root.tr("pf.following"), thousands(p.following, vi))))
                })
                .when(!own, |d| d.child(figure(root.tr("pf.followers"), thousands(p.followers, vi))).child(figure(root.tr("pf.following"), thousands(p.following, vi))))
                .when_some(p.robux, |d, r| {
                    d.child(div().flex().flex_col().gap(px(2.)).child(div().text_size(px(T_S)).text_color(c(MUTED)).child("Robux")).child(
                        div().flex().items_center().gap(px(6.)).child(icon("robux", 18., c(TEXT))).child(div().text_size(px(T_FIG)).line_height(px(28.)).font_weight(FontWeight::SEMIBOLD).font_features(tnum()).child(thousands(r, vi))),
                    ))
                }),
        );
    let who = div()
        .relative()
        .rounded(px(R_L))
        .overflow_hidden()
        .bg(c(RAISED))
        .border_1()
        .border_color(c(LINE))
        .child(div().w_full().h(px(BANNER)).flex().gap(px(2.)).bg(c(RAISED)).overflow_hidden().children(banner))
        .child(
            div()
                .pl(px(28. + AV + 24.))
                .pr(px(24.))
                .pt(px(14.))
                .pb(px(22.))
                .min_h(px(AV * 0.45 + 40.))
                .flex()
                .items_start()
                .gap(px(24.))
                .child(words)
                .children(side),
        )
        .child(
            div()
                .absolute()
                .left(px(28.))
                .top(px(BANNER - AV * 0.55))
                .size(px(AV))
                .rounded_full()
                .border_4()
                .border_color(c(RAISED))
                .bg(c(RAISED_2))
                .overflow_hidden()
                // (GPUI cuts children to the box, not to its round corners: the picture is rounded itself.)
                .when_some(face, |d, (src, key)| d.child(fade_in(img(src).size_full().rounded_full().object_fit(ObjectFit::Cover), &key))),
        );

    let groups: Vec<AnyElement> = p
        .groups
        .iter()
        .map(|g| {
            let (id, name) = (g.id, g.name.clone());
            let pic = (!g.pic.is_empty()).then(|| images::pic_sq(&g.pic, 44., cx)).flatten();
            block_card(("pf-group", id as usize))
                .w(px(card_w))
                .h(px(68.))
                .px(px(12.))
                .flex()
                .items_center()
                .gap(px(12.))
                .cursor_pointer()
                .on_click(cx.listener(move |r, _, _, cx| {
                    open_club(r, id, name.clone(), cx);
                    cx.notify();
                }))
                .child(div().size(px(44.)).flex_none().rounded(px(R_M)).bg(c(RAISED_2)).overflow_hidden().map(|d| match pic {
                    Some(f) => d.child(img(f).size_full().object_fit(ObjectFit::Cover)),
                    None => d.flex().items_center().justify_center().child(icon("users", 18., c(MUTED))),
                }))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(div().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).truncate().child(g.name.clone()))
                        .child(div().text_size(px(T_S)).text_color(c(MUTED)).truncate().child(format!("{} · {}", g.role, root.i18n.tv("pf.members", &[("n", &thousands(g.members, vi))])))),
                )
                .child(icon("chevronRight", 14., c(DIM)))
                .into_any_element()
        })
        .collect();

    // The shelves under the communities, each only when it holds something: favourites, the games they made, badges.
    let shelf = |key: &str, count: usize, tiles: Vec<AnyElement>| {
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(section_head(root.tr(key), Some(count)))
            .child(div().flex().flex_wrap().gap(px(GRID_GAP)).children(tiles))
    };
    let favs = game_tiles(root, &p.favs, inner, "pf-fav", cx);
    let made = game_tiles(root, &p.made, inner, "pf-made", cx);
    let badges: Vec<AnyElement> = p
        .badges
        .iter()
        .map(|b| {
            let pic = (!b.pic.is_empty()).then(|| images::pic_sq(&b.pic, 40., cx)).flatten();
            div()
                .w(px(card_w))
                .h(px(60.))
                .px(px(12.))
                .flex()
                .items_center()
                .gap(px(12.))
                .rounded(px(R_L))
                .bg(c(RAISED))
                .border_1()
                .border_color(c(LINE))
                .child(div().size(px(40.)).flex_none().rounded_full().bg(c(RAISED_2)).overflow_hidden().when_some(pic, |d, f| d.child(img(f).size_full().object_fit(ObjectFit::Cover))))
                .child(div().flex_1().min_w_0().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).truncate().child(b.name.clone()))
                .into_any_element()
        })
        .collect();

    crate::scrolled::body("pf-body", 
        div()
            .px(px(PAD + 4.))
            .pt(px(4.))
            .pb(px(24.))
            .flex()
            .flex_col()
            .gap(px(SECTION_GAP))
            .child(who)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(section_head(root.tr("pf.groups"), Some(p.groups.len())))
                    .map(|d| if groups.is_empty() { d.child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("pf.noGroups"))) } else { d.child(div().flex().flex_wrap().gap(px(GRID_GAP)).children(groups)) }),
            )
            .when(!favs.is_empty(), |d| d.child(shelf("pf.favs", p.favs.len(), favs)))
            .when(!made.is_empty(), |d| d.child(shelf("pf.made", p.made.len(), made)))
            .when(!badges.is_empty(), |d| d.child(shelf("pf.badges", p.badges.len(), badges))),
    )
}

// ───────────────────────── Marketplace ─────────────────────────

/// The front shelf when nothing was asked yet.
pub fn want_shop(r: &mut Root, cx: &mut Context<Root>) {
    // An open thing: who owns it and the buyer's Robux, asked for the account whose tab is shown (again for another).
    if let Some(item) = &r.profile.item {
        if let Some((_, user, cookie)) = friends::signed(r).filter(|s| s.1 != item.me) {
            ask_item(r, user, cookie, cx);
        }
    }
    if r.profile.free {
        if r.profile.free_at.is_none_or(|t| t.elapsed() >= FREE_FRESH) && !r.profile.free_reading {
            read_free(r, cx);
        }
        return;
    }
    if r.profile.asked.is_none() && !r.profile.reading {
        r.profile.asked = Some(String::new());
        // BLOXKEEP_SHOT_SHOP_FIND="wings": a hidden shot's Marketplace starts on that search.
        #[cfg(feature = "shot")]
        if let Ok(q) = std::env::var("BLOXKEEP_SHOT_SHOP_FIND") {
            r.profile.asked = Some(q);
        }
        page(r, cx);
    }
}

/// The name the app shows for the account whose tab is shown (the one a purchase is for).
pub fn buyer(root: &Root) -> String {
    friends::signed(root)
        .and_then(|s| root.accounts().iter().find(|a| a.get("id").and_then(serde_json::Value::as_str) == Some(s.0.as_str())).cloned())
        .and_then(|a| ["label", "alias", "displayName", "username"].iter().filter_map(|k| a.get(*k).and_then(serde_json::Value::as_str)).find(|n| !n.is_empty()).map(str::to_string))
        .unwrap_or_default()
}

/// The app's accounts with a Roblox id, each with the name the app shows for it.
pub fn named(r: &Root) -> Vec<(i64, String)> {
    r.accounts()
        .iter()
        .filter_map(|a| {
            let user = a.get("userId").and_then(serde_json::Value::as_i64).filter(|u| *u > 0)?;
            let name = ["label", "alias", "displayName", "username"].iter().filter_map(|k| a.get(*k).and_then(serde_json::Value::as_str)).find(|n| !n.is_empty()).unwrap_or_default().to_string();
            Some((user, name))
        })
        .collect()
}

/// Opens a thing's page in the catalog's place, and asks what its maker sells it for.
fn open_item(r: &mut Root, ware: Ware, cx: &mut Context<Root>) {
    let (id, collectible) = (ware.id, ware.collectible.clone());
    // (An old kind has no offer to ask for: it opens on roblox.com.)
    let offer = collectible.is_empty().then(|| Err(String::new()));
    r.profile.item = Some(ItemView { ware, offer, me: 0, robux: None, owners: None, mine: None, armed: false, busy: false, done: None, tried: None, trying: false, on_me: false, watched: crate::itemwatch::watching(id) });
    if collectible.is_empty() {
        return;
    }
    let rx = roblox::run(move || roblox::offer(&collectible));
    cx.spawn(async move |this, cx| {
        let got = rx.await.unwrap_or_else(|_| Err(String::new()));
        let _ = this.update(cx, |r, cx| {
            if let Some(i) = r.profile.item.as_mut().filter(|i| i.ware.id == id) {
                i.offer = Some(got);
                cx.notify();
            }
        });
    })
    .detach();
}

/// For the thing shown and the account whose tab is shown: that account's Robux, and which accounts own the thing.
fn ask_item(r: &mut Root, user: i64, cookie: String, cx: &mut Context<Root>) {
    let users: Vec<i64> = named(r).into_iter().map(|a| a.0).collect();
    let pf = &mut r.profile;
    let Some(item) = pf.item.as_mut() else { return };
    item.me = user;
    item.robux = None;
    item.mine = None;
    item.armed = false;
    let (id, bundle) = (item.ware.id, item.ware.bundle);
    // (A purchase of it for this account still under way keeps Buy away until its answer comes.)
    item.busy = pf.buying.contains(&(user, id, bundle));
    let kind = if bundle { "Bundle" } else { "Asset" };
    // First this account's Robux and whether it owns it (Buy waits for these only), then every account's.
    let mine_cookie = cookie.clone();
    let quick = roblox::run(move || (roblox::robux(&mine_cookie, user), roblox::owns(kind, id, user, &mine_cookie)));
    cx.spawn(async move |this, cx| {
        // (No answer at all: left unknown, so Buy stays off; taking it as "not owned" could buy a second copy.)
        let Ok((robux, mine)) = quick.await else { return };
        let _ = this.update(cx, |r, cx| {
            let pf = &mut r.profile;
            if let Some(i) = pf.item.as_mut().filter(|i| i.ware.id == id && i.me == user) {
                i.robux = robux;
                // Roblox didn't say (and not bought here): left unknown, Buy stays off.
                i.mine = if pf.bought.contains(&(user, id, bundle)) { Some(true) } else { mine };
                cx.notify();
            }
        });
    })
    .detach();
    let rx = roblox::run(move || roblox::owners(kind, id, &users, Some((user, &cookie))));
    cx.spawn(async move |this, cx| {
        let Ok(owners) = rx.await else { return };
        let _ = this.update(cx, |r, cx| {
            let pf = &mut r.profile;
            if let Some(i) = pf.item.as_mut().filter(|i| i.ware.id == id && i.me == user) {
                let mut owners = owners;
                // (What this account bought already, though Roblox may not say so yet.)
                if pf.bought.contains(&(user, id, bundle)) && !owners.contains(&user) {
                    owners.push(user);
                }
                i.owners = Some(owners);
                // BLOXKEEP_SHOT_ITEM=armed: a hidden shot shows the page as after the first Buy click.
                #[cfg(feature = "shot")]
                if std::env::var("BLOXKEEP_SHOT_ITEM").as_deref() == Ok("armed") {
                    i.armed = true;
                }
                cx.notify();
            }
        });
    })
    .detach();
}

/// The second click: the thing is bought for the account whose tab is shown, at the price on the page.
fn buy(r: &mut Root, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = friends::signed(r) else { return };
    let pf = &mut r.profile;
    let Some(item) = pf.item.as_mut() else { return };
    let Some(Ok(offer)) = &item.offer else { return };
    let (id, bundle) = (item.ware.id, item.ware.bundle);
    if item.busy || !item.armed || item.me != user || !offer.open || pf.buying.contains(&(user, id, bundle)) {
        return;
    }
    let (collectible, price) = (item.ware.collectible.clone(), offer.price);
    item.busy = true;
    item.armed = false;
    item.done = None;
    pf.buying.push((user, id, bundle));
    let rx = roblox::run(move || roblox::buy(&cookie, user, &collectible, price));
    cx.spawn(async move |this, cx| {
        let done = rx.await.unwrap_or_else(|_| Err(String::new()));
        let _ = this.update(cx, |r, cx| {
            // Kept whatever is shown now, so it holds when this thing is shown again.
            crate::pages::games::stale_corner(r);
            let pf = &mut r.profile;
            pf.buying.retain(|b| *b != (user, id, bundle));
            if done.is_ok() && !pf.bought.contains(&(user, id, bundle)) {
                pf.bought.push((user, id, bundle));
            }
            // (Shown for another account by now: that one's Buy is its own.)
            if let Some(i) = pf.item.as_mut().filter(|i| i.ware.id == id && i.ware.bundle == bundle && i.me == user) {
                i.busy = false;
                if done.is_ok() {
                    i.robux = i.robux.map(|r| r - price);
                    i.mine = Some(true);
                    if !i.owners.as_ref().is_some_and(|l| l.contains(&user)) {
                        i.owners.get_or_insert_with(Vec::new).push(user);
                    }
                }
                i.done = Some(done);
            }
            cx.notify();
        });
    })
    .detach();
}

/// A thing's page: its picture and what it is, its maker's price, Buy (a first click arms it and says whose Robux, a
/// second buys), and the app's accounts that own it already.
/// Try on: the account shown, wearing the thing open, drawn by Roblox (nothing is changed on the account).
fn try_on(r: &mut Root, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = friends::signed(r) else { return };
    let Some(item) = r.profile.item.as_mut().filter(|i| !i.ware.bundle && !i.trying) else { return };
    // (Asked once per thing and account: shown again from what came.)
    if matches!(item.tried, Some(Ok(_))) && item.me == user {
        item.on_me = !item.on_me;
        return;
    }
    let id = item.ware.id;
    item.trying = true;
    item.on_me = true;
    item.tried = None;
    let rx = roblox::run(move || roblox::try_on(&cookie, id));
    cx.spawn(async move |this, cx| {
        let got = rx.await.unwrap_or_else(|_| Err(String::new()));
        let _ = this.update(cx, |r, cx| {
            if let Some(i) = r.profile.item.as_mut().filter(|i| i.ware.id == id) {
                i.trying = false;
                i.tried = Some(got);
                cx.notify();
            }
        });
    })
    .detach();
}

fn item_page(root: &Root, view: &ItemView, inner: f32, cx: &mut Context<Root>) -> AnyElement {
    let vi = root.i18n.vietnamese;
    let w = &view.ware;
    let (id, kind) = (w.id, if w.bundle { "bundle" } else { "asset" });
    let back = div().flex_none().px(px(PAD + 4.)).pb(px(14.)).flex().child(Button::new("item-back", root.tr("nav.shop")).icon("chevronLeft").small().on_click(cx.listener(|r, _, _, cx| {
        r.profile.item = None;
        cx.notify();
    })));
    let signed = friends::signed(root);
    let accounts = named(root);
    let me = signed.as_ref().map(|s| s.1).unwrap_or(0);
    let my_name = buyer(root);
    let said = |colour: u32, text: String| div().text_size(px(T_M)).text_color(c(colour)).child(text);
    let on_roblox = || Button::new("item-open", root.tr("shop.open")).icon("external").on_click(cx.listener(move |r, _, _, _| r.send("openRobloxPage", json!({ "kind": kind, "id": id }))));

    let price_of = |p: i64| {
        if p == 0 {
            div().text_size(px(T_STAGE)).font_weight(FontWeight::BOLD).text_color(c(OK)).child(root.tr("shop.free"))
        } else {
            div().flex().items_center().gap(px(7.)).child(icon("robux", 18., c(TEXT))).child(div().text_size(px(T_TITLE)).font_weight(FontWeight::BOLD).font_features(tnum()).child(thousands(p, vi)))
        }
    };
    let buying: Div = match &view.offer {
        None => div().child(said(MUTED, root.tr("av.reading"))),
        Some(Ok(o)) if o.open => {
            let price = o.price;
            let owned = me > 0 && (view.mine == Some(true) || view.owners.as_ref().is_some_and(|l| l.contains(&me)));
            let short = view.robux.is_some_and(|r| r < price);
            let act: Div = if signed.is_none() {
                // What is missing, and the way to it: an account signed in is added here (the app's own Add account).
                div()
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap(px(10.))
                    .child(said(MUTED, root.tr("shop.needAccount")))
                    .child(Button::new("item-add-acc", root.tr("acc.add")).icon("plus").primary().on_click(cx.listener(|r, _, _, cx| r.add_account(cx))))
            } else if matches!(view.done, Some(Ok(()))) {
                div().child(said(OK, root.tr("shop.bought")))
            } else if owned {
                div().child(said(OK, root.tr("shop.have")))
            } else if view.busy {
                div().flex().child(Button::new("item-buying", root.tr("shop.buying")).primary().disabled(true))
            } else if short {
                div().child(said(WARN, root.i18n.tv("shop.short", &[("n", &thousands(view.robux.unwrap_or(0), vi))])))
            } else if view.armed {
                let whose = if price == 0 { root.i18n.tv("shop.chargeFree", &[("who", &my_name)]) } else { root.i18n.tv("shop.charge", &[("n", &thousands(price, vi)), ("who", &my_name)]) };
                let after = view.robux.filter(|_| price > 0).map(|r| root.i18n.tv("shop.after", &[("n", &thousands(r - price, vi))]));
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(div().flex().flex_col().gap(px(2.)).child(said(TEXT, whose)).children(after.map(|a| said(MUTED, a))))
                    .child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .child(Button::new("item-confirm", root.tr("shop.confirm")).primary().on_click(cx.listener(|r, _, _, cx| {
                                buy(r, cx);
                                cx.notify();
                            })))
                            .child(Button::new("item-cancel", root.tr("gp.cancel")).on_click(cx.listener(|r, _, _, cx| {
                                if let Some(i) = r.profile.item.as_mut() {
                                    i.armed = false;
                                }
                                cx.notify();
                            }))),
                    )
            } else {
                // (Armed only once the account's Robux and what it owns are known: nothing is bought blind.)
                div().flex().child(Button::new("item-buy", root.tr(if price == 0 { "shop.get" } else { "shop.buy" })).primary().disabled(view.me != me || view.mine.is_none()).on_click(cx.listener(|r, _, _, cx| {
                    if let Some(i) = r.profile.item.as_mut() {
                        i.armed = true;
                        i.done = None;
                    }
                    cx.notify();
                })))
            };
            let failed = match &view.done {
                Some(Err(why)) => Some(match why.as_str() {
                    "price" => root.tr("shop.moved"),
                    "off" => root.tr("shop.closed"),
                    why => root.i18n.tv("shop.refused", &[("why", if why.is_empty() { "Roblox" } else { why })]),
                }),
                _ => None,
            };
            div().flex().flex_col().gap(px(14.)).child(price_of(price)).child(act).children(failed.map(|f| said(WARN, f)))
        }
        // Off sale, sold only inside a game, resold by owners, or an old kind: Roblox's own page has what there is.
        Some(_) => div().flex().flex_col().gap(px(12.)).items_start().child(said(MUTED, root.tr("shop.notHere"))).child(on_roblox()),
    };

    // (On the account, once tried: Roblox's drawing of it wearing the thing, in the thing's place.)
    let worn = view.tried.as_ref().and_then(|t| t.as_ref().ok()).filter(|_| view.on_me).cloned();
    let shown = worn.clone().unwrap_or_else(|| w.pic.clone());
    let pic = (!shown.is_empty()).then(|| images::pic_pair(&shown, 260., 260., cx)).flatten();
    let lost = !shown.is_empty() && images::failed(&shown, cx);
    let watch_btn = (!w.bundle).then(|| {
        let ware = w.clone();
        Button::new("item-watch", root.tr(if view.watched { "watch.on" } else { "watch.add" }))
            .icon("bell")
            .small()
            .when(view.watched, |b| b.tint())
            .on_click(cx.listener(move |r, _, _, cx| {
                match crate::itemwatch::toggle(ware.id, &ware.name, &ware.pic, &ware.maker, &ware.collectible) {
                    Ok(on) => {
                        if let Some(i) = r.profile.item.as_mut() {
                            i.watched = on;
                        }
                        r.profile.watch = crate::itemwatch::list();
                        r.toast = Some((r.tr(if on { "watch.added" } else { "watch.removed" }), "ok".into()));
                    }
                    Err(why) => r.toast = Some((if why == "full" { r.i18n.tv("watch.full", &[("n", &crate::itemwatch::MOST.to_string())]) } else { why }, "bad".into())),
                }
                cx.notify();
            }))
    });
    let try_row = (!w.bundle).then(|| {
        let why = view.tried.as_ref().and_then(|t| t.as_ref().err()).filter(|_| !view.trying).map(|_| root.tr("shop.tryFailed"));
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .children(watch_btn)
            .when(signed.is_some(), |d| d.child(
                Button::new("item-try", root.tr(if view.trying { "shop.trying" } else if worn.is_some() { "shop.tryOff" } else { "shop.try" }))
                    .icon("user")
                    .small()
                    .disabled(view.trying)
                    .on_click(cx.listener(|r, _, _, cx| {
                        try_on(r, cx);
                        cx.notify();
                    })),
            ))
            .children(why.map(|t| div().text_size(px(T_S)).text_color(c(WARN)).child(t)))
    });
    // At the right of a wide card, what it is in facts (as a profile's header has them): its kind, its maker, its
    // Roblox ID, and which of the accounts have it. Without them the right 60 % of the card stood empty (review 3f.2).
    let facts_side = (inner >= 1000.).then(|| {
        let mut known: Vec<(String, String)> = vec![(root.tr("shop.f.kind"), root.tr(if w.bundle { "shop.f.bundle" } else { "shop.f.item" }))];
        if !w.maker.is_empty() {
            known.push((root.tr("shop.f.maker"), w.maker.clone()));
        }
        known.push((root.tr("pf.id"), w.id.to_string()));
        if let Some(l) = view.owners.as_ref() {
            known.push((root.tr("shop.f.owners"), l.iter().filter(|u| accounts.iter().any(|a| a.0 == **u)).count().to_string()));
        }
        div()
            .w(px(320.))
            .flex_none()
            .self_stretch()
            .pl(px(28.))
            .border_l_1()
            .border_color(c(LINE))
            .flex()
            .flex_wrap()
            .content_center()
            .gap_x(px(20.))
            .gap_y(px(18.))
            .children(known.into_iter().map(|(name, value)| {
                div()
                    .w(px(130.))
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .child(div().text_size(px(T_S)).text_color(c(MUTED)).child(name))
                    .child(div().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).font_features(tnum()).truncate().child(value))
            }))
    });
    let card = surface()
        .p(px(18.))
        .flex()
        .items_start()
        .gap(px(22.))
        // (The thing on a lit square with its hairline, as its tile in the Marketplace is: on the card's own ground it
        // had no edge, review 3f.2.)
        .child(div().size(px(280.)).flex_none().rounded(px(R_L)).bg(c(RAISED)).border_1().border_color(c(LINE)).overflow_hidden().flex().items_center().justify_center().map(|d| match pic {
                Some((src, key)) => d.child(fade_in(img(src).size(px(260.)).object_fit(ObjectFit::Contain), &key)),
                None if lost => d.gap(px(6.)).text_size(px(T_S)).text_color(c(MUTED)).child(icon("image", 13., c(MUTED))).child(root.tr("news.noPicture")),
                None => d.child(icon("image", 28., c(DIM))),
            }))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(14.))
                .child(div().flex().flex_col().gap(px(4.)).child(div().text_size(px(T_TITLE)).font_weight(FontWeight::BOLD).child(w.name.clone())).child(div().text_size(px(T_M)).text_color(c(MUTED)).truncate().child(w.maker.clone())))
                .child(buying)
                .children(try_row)
                .when(!w.about.is_empty(), |d| d.child(div().max_h(px(100.)).overflow_hidden().text_size(px(T_M)).text_color(c(TEXT_2)).line_height(px(20.)).child(w.about.clone()))),
        )
        .children(facts_side)
        .when(matches!(&view.offer, Some(Ok(o)) if o.open), |d| d.child(IconButton::new("item-ext", "external").size(34.).tip(root.tr("shop.open")).on_click(cx.listener(move |r, _, _, _| r.send("openRobloxPage", json!({ "kind": kind, "id": id }))))));

    let have: AnyElement = match &view.owners {
        None if signed.is_none() => div().into_any_element(),
        None => said(MUTED, root.tr("av.reading")).into_any_element(),
        Some(l) if l.is_empty() => said(MUTED, root.tr("shop.ownersNone")).into_any_element(),
        Some(l) => div()
            .flex()
            .flex_wrap()
            .gap(px(8.))
            .children(l.iter().filter_map(|user| accounts.iter().find(|a| a.0 == *user)).map(|(_, name)| {
                div().h(px(TOOL_H)).px(px(12.)).flex().items_center().gap(px(6.)).rounded(px(R_M)).bg(ca(OK, 0.12)).text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).text_color(c(OK)).child(icon("check", 13., c(OK))).child(name.clone())
            }))
            .into_any_element(),
    };

    // More of what the Marketplace was showing when this was picked (two rows of it), so the next thing is one press
    // away and the page isn't an island.
    let (cols, tile_w) = ware_fit(inner);
    let more: Vec<AnyElement> = match &root.profile.wares {
        Some(Ok(l)) => l.iter().filter(|x| !(x.id == w.id && x.bundle == w.bundle)).take(cols as usize * 2).enumerate().map(|(n, x)| ware_tile(root, "item-more", n, x, tile_w, cx)).collect(),
        _ => Vec::new(),
    };

    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .child(back)
        .child(
            crate::scrolled::body("item-body", 
                div()
                    .px(px(PAD + 4.))
                    .pb(px(24.))
                    .flex()
                    .flex_col()
                    .gap(px(SECTION_GAP))
                    .child(card)
                    .when(signed.is_some(), |d| d.child(div().flex().flex_col().gap(px(12.)).child(section_head(root.tr("shop.owners"), None)).child(have)))
                    .when(!more.is_empty(), |d| {
                        d.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(12.))
                                .child(section_head(root.tr("shop.more"), None))
                                .child(div().mx(px(-6.)).flex().flex_wrap().gap(px(SGAP)).children(more)),
                        )
                    }),
            ),
        )
        .into_any_element()
}

/// New free's list, read (roblox::free_new); and once it is in, the page is drawn again when the list has got old, which
/// reads it again if New free is still what is shown (nothing is read while it isn't).
fn read_free(r: &mut Root, cx: &mut Context<Root>) {
    r.profile.free_reading = true;
    let rx = roblox::run(roblox::free_new);
    cx.spawn(async move |this, cx| {
        let got = rx.await.unwrap_or_else(|_| Err(String::new()));
        let _ = this.update(cx, |r, cx| {
            let p = &mut r.profile;
            p.free_reading = false;
            p.free_at = Some(std::time::Instant::now());
            match got {
                Ok(list) => {
                    match &mut p.free_known {
                        Some(known) => {
                            for f in &list {
                                if known.insert(f.id) {
                                    p.free_fresh.insert(f.id);
                                }
                            }
                        }
                        None => p.free_known = Some(list.iter().map(|f| f.id).collect()),
                    }
                    // What came since the first read leads, then the most left: a real giveaway (many of a thing)
                    // over the one-of-a-kind things that fill Roblox's own order.
                    let mut list = list;
                    list.sort_by_key(|f| (!p.free_fresh.contains(&f.id), std::cmp::Reverse(f.left)));
                    #[cfg(feature = "shot")]
                    crate::near::shot_many(&mut list);
                    p.frees = Some(Ok(list));
                    // BLOXKEEP_SHOT_FREE_OPEN=1: a hidden shot opens the first one's game, as its button does.
                    #[cfg(feature = "shot")]
                    if std::env::var("BLOXKEEP_SHOT_FREE_OPEN").is_ok() {
                        let first = match &r.profile.frees {
                            Some(Ok(l)) => l.iter().find(|f| f.place > 0).map(|f| (f.place, f.game.clone())),
                            _ => None,
                        };
                        if let Some((place, game)) = first {
                            open_game(r, place, &game);
                        }
                    }
                }
                // A later read that fails leaves what is shown.
                Err(e) if !matches!(p.frees, Some(Ok(_))) => p.frees = Some(Err(e)),
                Err(_) => {}
            }
            cx.notify();
        });
        cx.background_executor().timer(FREE_FRESH + std::time::Duration::from_millis(200)).await;
        let _ = this.update(cx, |r, cx| {
            if r.profile.free && r.page == crate::pages::Page::Games && r.games.view == crate::pages::games::View::Shop {
                cx.notify();
            }
        });
    })
    .detach();
}

/// The search box's words (Enter), or the front shelf again when it was emptied.
pub fn shop_find(r: &mut Root, cx: &mut Context<Root>) {
    let q = r.shop_search.read(cx).value().trim().to_string();
    // (The same words again after a failed read ask again.)
    if r.profile.asked.as_deref() == Some(q.as_str()) && !matches!(r.profile.wares, Some(Err(_))) {
        return;
    }
    r.profile.asked = Some(q);
    r.profile.wares = None;
    r.profile.next.clear();
    r.profile.reading = false;
    page(r, cx);
    cx.notify();
}

/// The kinds of thing the Marketplace can be narrowed to (roblox.com's catalog's own): the word, and how the search
/// says it.
const CATS: [(&str, &str); 5] = [
    ("shop.all", "&Category=1"),
    ("shop.cat.clothing", "&Category=3"),
    ("shop.cat.accessories", "&Category=11"),
    ("shop.cat.animations", "&Category=12"),
    ("shop.cat.limiteds", "&Category=1&SalesTypeFilter=2"),
];
/// The orders: the word, and how the search says it. The first is Roblox's own: the best sellers on the front shelf,
/// the best match for words.
const SORTS: [(&str, &str); 4] = [("shop.sort.top", ""), ("shop.sort.fav", "&SortType=1"), ("shop.sort.new", "&SortType=3"), ("shop.sort.low", "&SortType=4&MinPrice=1")];

/// The filters as the search says them, for words asked (or none).
fn filters(cat: usize, sort: usize, words: bool) -> String {
    let by = match SORTS.get(sort).map(|s| s.1) {
        Some("") | None if !words => "&SortType=2",
        Some(s) => s,
        None => "",
    };
    format!("{}{by}", CATS.get(cat).map(|c| c.1).unwrap_or("&Category=1"))
}

/// The Marketplace narrowed or ordered another way: what it holds goes, and its first page is read again.
fn refilter(r: &mut Root, cat: usize, sort: usize, cx: &mut Context<Root>) {
    r.profile.free = false;
    r.profile.watch_view = false;
    if r.profile.cat == cat && r.profile.sort == sort {
        return;
    }
    r.profile.cat = cat;
    r.profile.sort = sort;
    r.profile.wares = None;
    r.profile.next.clear();
    r.profile.reading = false;
    if r.profile.asked.is_none() {
        r.profile.asked = Some(String::new());
    }
    page(r, cx);
}

/// The next page of what is asked (its first, when nothing is held yet).
fn page(r: &mut Root, cx: &mut Context<Root>) {
    let Some(q) = r.profile.asked.clone() else { return };
    let cursor = if r.profile.wares.is_some() { r.profile.next.clone() } else { String::new() };
    r.profile.reading = true;
    let ask = q.clone();
    let ticket = crate::pages::avatar::ticket();
    r.profile.page_ask = ticket;
    let (cat, sort) = (r.profile.cat, r.profile.sort);
    let by = filters(cat, sort, !q.is_empty());
    let rx = roblox::run(move || roblox::catalog(&ask, &cursor, &by));
    cx.spawn(async move |this, cx| {
        let got = rx.await.unwrap_or_else(|_| Err(String::new()));
        let _ = this.update(cx, |r, cx| {
            // A page for words, or filters, no longer asked is dropped; so is one overtaken by a newer ask (the same
            // words asked again after others: their first page would come twice).
            if r.profile.asked.as_deref() != Some(q.as_str()) || (r.profile.cat, r.profile.sort) != (cat, sort) || r.profile.page_ask != ticket {
                return;
            }
            r.profile.reading = false;
            match (got, &mut r.profile.wares) {
                (Ok((more, next)), Some(Ok(have))) => {
                    have.extend(more);
                    r.profile.next = next;
                }
                (Ok((first, next)), _) => {
                    // BLOXKEEP_SHOT_ITEM=1: a hidden shot opens the first thing that has a price (the first of
                    // all when none has).
                    #[cfg(feature = "shot")]
                    let shown = std::env::var("BLOXKEEP_SHOT_ITEM").ok().and_then(|_| first.iter().find(|w| w.price.is_some_and(|p| p > 0) && !w.collectible.is_empty()).or(first.iter().find(|w| !w.collectible.is_empty())).cloned());
                    #[cfg(feature = "shot")]
                    let mut first = first;
                    #[cfg(feature = "shot")]
                    crate::near::shot_many(&mut first);
                    r.profile.wares = Some(Ok(first));
                    r.profile.next = next;
                    #[cfg(feature = "shot")]
                    if let Some(w) = shown {
                        open_item(r, w, cx);
                    }
                }
                // A later page that fails leaves what is shown; More asks for it again.
                (Err(_), Some(Ok(_))) => {}
                (Err(e), _) => r.profile.wares = Some(Err(e)),
            }
            cx.notify();
        });
    })
    .detach();
}

/// How the Marketplace's tiles fit a width: how many a row, and a tile's width (with its padding).
fn ware_fit(inner: f32) -> (f32, f32) {
    let cols = ((inner + 12. + SGAP) / (TILE + 12. + SGAP)).floor().max(1.);
    (cols, ((inner + 12. - SGAP * (cols - 1.)) / cols).floor())
}

/// A thing on sale as a tile: its picture, name and price. A press opens its page here.
fn ware_tile(root: &Root, key: &'static str, n: usize, w: &Ware, tile_w: f32, cx: &mut Context<Root>) -> AnyElement {
    let ware = w.clone();
    let price = match w.price {
        None => div().text_color(c(MUTED)).child(root.tr("shop.off")),
        Some(0) => div().text_color(c(OK)).child(root.tr("shop.free")),
        Some(r) => div().flex().items_center().gap(px(5.)).text_color(c(TEXT)).font_features(tnum()).child(icon("robux", 13., c(TEXT))).child(thousands(r, root.i18n.vietnamese)),
    };
    // (Its own padding is the ground that comes up under the mouse; the focus ring takes a pixel each side.)
    hot((key, n))
        .w(px(tile_w))
        .p(px(6.))
        .cursor_pointer()
        .rounded(px(R_L + 4.))
        .lift(2.)
        // (A column, so the tile stands as tall as its row's tallest and the prices line up under one-line names.)
        .flex()
        .flex_col()
        .on_click(cx.listener(move |r, _, _, cx| {
            open_item(r, ware.clone(), cx);
            cx.notify();
        }))
        .child(crate::pages::avatar::tile_zoomed(tile_w - 14., &w.pic, w.name.clone(), false, Some(root.tr("news.noPicture").into()), if w.bundle { 1.3 } else { 1. }, cx).flex_1().child(div().px(px(2.)).pb(px(4.)).text_size(px(T_M)).font_weight(FontWeight::MEDIUM).child(price)))
        .into_any_element()
}

pub fn shop(root: &Root, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let pf = &root.profile;
    let inner = width - 2. * (PAD + 4.);
    let (cols, tile_w) = ware_fit(inner);
    let shell = div().id("shop").relative().size_full().rounded(px(R_PANEL)).bg(c(PANEL)).overflow_hidden().flex().flex_col();
    let header = div()
        .flex_none()
        .px(px(PAD + 4.))
        .pt(px(18.))
        .pb(px(18.))
        .flex()
        .items_center()
        .gap(px(10.))
        .child(crate::pages::games::brand(root))
        .relative().child(crate::pages::games::island(root, cx))
        .child(div().flex_1())
        .when(!pf.free && pf.item.is_none(), |d| d.child(div().w(px(if width < 900. { 220. } else { 300. })).min_w(px(140.)).flex_shrink(1.).child(field(&root.shop_search, Some("search")))));
    // Under it: the whole catalog, or the free limiteds in stock.
    // The kinds of thing as the page's filters (the one shown the yellow block), then, past a hairline, New free (a list
    // of its own); the order at the right, a quiet tool that opens its short list.
    let sort = pf.sort;
    let kinds = div()
        .flex_none()
        .px(px(PAD + 4.))
        .pb(px(16.))
        .flex()
        .items_center()
        .gap(px(8.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(4.))
                .children(CATS.iter().enumerate().map(|(i, (key, _))| {
                    let on = !pf.free && !pf.watch_view && pf.cat == i;
                    chip(("shop-cat", i), on).h(px(TOOL_H)).px(px(12.)).child(root.tr(key)).on_click(cx.listener(move |r, _, _, cx| {
                        refilter(r, i, sort, cx);
                        cx.notify();
                    }))
                }))
                .child(tool_rule())
                .child(chip("shop-kind-free", pf.free).h(px(TOOL_H)).px(px(12.)).child(icon("tag", 15., c(if pf.free { INK } else { MUTED }))).child(root.tr("shop.freeNew")).on_click(cx.listener(|r, _, _, cx| {
                    r.profile.free = true;
                    r.profile.watch_view = false;
                    cx.notify();
                })))
                .child({
                    let on = pf.watch_view;
                    let fresh = pf.watch.iter().any(|w| w.fresh);
                    chip("shop-kind-watch", on)
                        .h(px(TOOL_H))
                        .px(px(12.))
                        .child(icon("bell", 15., c(if on { INK } else { MUTED })))
                        .child(root.tr("watch.title"))
                        .when(!pf.watch.is_empty(), |d| d.child(chip_count(pf.watch.len(), on)))
                        .when(fresh && !on, |d| d.child(div().size(px(7.)).rounded_full().bg(c(OK))))
                        .on_click(cx.listener(|r, _, _, cx| {
                            r.profile.watch_view = true;
                            r.profile.free = false;
                            r.profile.watch = crate::itemwatch::list();
                            crate::itemwatch::seen();
                            cx.notify();
                        }))
                }),
        )
        .when(!pf.free && !pf.watch_view, |d| d.child(sort_drop(root, cx)));
    if let Some(view) = &pf.item {
        return shell.child(header).child(item_page(root, view, inner, cx));
    }
    if pf.free {
        return shell.child(header).child(kinds).child(frees(root, tile_w, cx));
    }
    if pf.watch_view {
        return shell.child(header).child(kinds).child(watch_list(root, inner, cx));
    }

    // BLOXKEEP_SHOT_WAITING=1: a hidden shot keeps the Marketplace as it is while its first page is on its way.
    #[cfg(feature = "shot")]
    let held = std::env::var("BLOXKEEP_SHOT_WAITING").is_ok();
    #[cfg(not(feature = "shot"))]
    let held = false;
    let body: AnyElement = match &pf.wares {
        _ if held => div().flex_1().px(px(PAD + 4.)).child(waiting_tiles(cols as usize * 2, tile_w - 12., 14.)).into_any_element(),
        None => div().flex_1().px(px(PAD + 4.)).child(waiting_tiles(cols as usize * 2, tile_w - 12., 14.)).into_any_element(),
        Some(Err(why)) => note(root.i18n.tv("shop.failed", &[("why", if why.is_empty() { "Roblox" } else { why })])).into_any_element(),
        Some(Ok(l)) if l.is_empty() => note(root.tr("shop.none")).into_any_element(),
        Some(Ok(l)) => {
            // (Only the lines near the view are made once there are many: each More adds a page of Roblox's.)
            let key = crate::near::sig((l.len(), l.first().map(|w| w.id)));
            let tiles = crate::near::tiles("shop-wares", l.len(), tile_w, SGAP, key, false, tile_w + 80., |n| ware_tile(root, "shop-ware", n, &l[n], tile_w, cx));
            crate::scrolled::body("shop-body", 
                    div().px(px(PAD + 4.)).pt(px(4.)).pb(px(24.)).flex().flex_col().gap(px(18.)).child(div().mx(px(-6.)).child(tiles)).when(!pf.next.is_empty(), |d| {
                        d.child(div().flex().child(Button::new("shop-more", root.tr("av.more")).disabled(pf.reading).on_click(cx.listener(|r, _, _, cx| {
                            if !r.profile.reading {
                                page(r, cx);
                                cx.notify();
                            }
                        }))))
                    }),
                )
                .into_any_element()
        }
    };
    shell.child(header).child(kinds).child(body)
}

/// The Marketplace's order: "Sort: Best selling ▾", its short list under it (the one in use ticked).
fn sort_drop(root: &Root, cx: &mut Context<Root>) -> Div {
    let pf = &root.profile;
    let open = pf.sort_open;
    let words = pf.asked.as_deref().is_some_and(|q| !q.is_empty());
    // (Roblox's own order is the best match for words, the best sellers without.)
    let name_of = |i: usize| if i == 0 && words { root.tr("shop.sort.match") } else { root.tr(SORTS[i].0) };
    let button = tool("shop-sort", Some("sort"), open).child(tool_words(Some(root.tr("shop.sort")), name_of(pf.sort.min(SORTS.len() - 1)))).child(tool_chev()).on_click(cx.listener(|r, _, _, cx| {
        // A click outside the open list closed it just now: this click on its own button only closes it.
        let just = r.profile.sort_closed.is_some_and(|at| at.elapsed().as_millis() < 300);
        r.profile.sort_closed = None;
        r.profile.sort_open = !just && !r.profile.sort_open;
        cx.notify();
    }));
    let mut menu = div()
        .id("shop-sort-menu")
        .occlude()
        .min_w(px(200.))
        .p(px(4.))
        .rounded(px(R_M))
        .bg(c(RAISED))
        .border_1()
        .border_color(c(LINE))
        .flex()
        .flex_col()
        .on_mouse_down_out(cx.listener(|r, _, _, cx| {
            if r.profile.sort_open {
                r.profile.sort_open = false;
                r.profile.sort_closed = Some(std::time::Instant::now());
                cx.notify();
            }
        }));
    for i in 0..SORTS.len() {
        let on = pf.sort == i;
        let cat = pf.cat;
        menu = menu.child(
            menu_row(("shop-sort-row", i))
                .text_color(if on { c(TEXT) } else { c(TEXT_2) })
                .child(div().min_w_0().flex_1().whitespace_nowrap().child(name_of(i)))
                .child(div().w(px(14.)).flex_none().when(on, |d| d.child(icon("check", 14., c(TEXT)))))
                .on_click(cx.listener(move |r, _, _, cx| {
                    r.profile.sort_open = false;
                    refilter(r, cat, i, cx);
                    cx.notify();
                })),
        );
    }
    div().relative().flex_none().child(button).child(
        div()
            .absolute()
            .top(px(TOOL_H + 4.))
            .right_0()
            .child(deferred(anchored().anchor(Anchor::TopRight).snap_to_window_with_margin(px(8.)).child(pop("shop-sort-pop", open, menu))).with_priority(1)),
    )
}

/// The game that gives a free thing, opened on its page here (Games' own view), where it is started with the accounts
/// wanted.
fn open_game(r: &mut Root, place: i64, game: &str) {
    r.games.view = crate::pages::games::View::Games;
    crate::pages::games::open_place(r, place, game);
}

/// New free's tiles: the thing, how many are left, the game that gives it, and Open game (the game's page here) or,
/// for one its Marketplace page gives too, Open on Roblox. The tile itself opens its page on roblox.com.
fn frees(root: &Root, tile_w: f32, cx: &mut Context<Root>) -> AnyElement {
    let pf = &root.profile;
    let list = match &pf.frees {
        None => return div().flex_1().px(px(PAD + 4.)).child(waiting_tiles(14, tile_w - 12., 14.)).into_any_element(),
        Some(Err(why)) => return note(root.i18n.tv("shop.failed", &[("why", if why.is_empty() { "Roblox" } else { why })])).into_any_element(),
        Some(Ok(l)) if l.is_empty() => return note(root.tr("shop.freeNone")).into_any_element(),
        Some(Ok(l)) => l,
    };
    let key = crate::near::sig((list.len(), list.first().map(|f| f.id)));
    let tile = |n: usize| {
            let f = &list[n];
            let id = f.id;
            let few = f.left * 4 <= f.total || f.left <= 2;
            let left = root.i18n.tv("shop.left", &[("n", &thousands(f.left, root.i18n.vietnamese)), ("m", &thousands(f.total, root.i18n.vietnamese))]);
            // Its Marketplace page gives it too, or Roblox names no game for it: its page on roblox.com.
            let act = if f.shop || (f.known && f.place == 0) {
                Button::new(("free-shop", n), root.tr("shop.open")).small().on_click(cx.listener(move |r, _, _, cx| {
                    cx.stop_propagation();
                    r.send("openRobloxPage", json!({ "kind": "asset", "id": id }));
                }))
            } else {
                let (place, game) = (f.place, f.game.clone());
                Button::new(("free-game", n), root.tr("shop.openGame")).small().disabled(place == 0).on_click(cx.listener(move |r, _, _, cx| {
                    cx.stop_propagation();
                    open_game(r, place, &game);
                    cx.notify();
                }))
            };
            hot(("free", n))
                .w(px(tile_w))
                .p(px(6.))
                .cursor_pointer()
                .rounded(px(R_L + 4.))
                .flex()
                .flex_col()
                .on_click(cx.listener(move |r, _, _, _| r.send("openRobloxPage", json!({ "kind": "asset", "id": id }))))
                .child(
                    tile_said(tile_w - 14., &f.pic, f.name.clone(), false, Some(root.tr("news.noPicture").into()), cx)
                        .flex_1()
                        .child(
                            div()
                                .px(px(2.))
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(div().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).font_features(tnum()).text_color(c(if few { WARN } else { TEXT })).child(left))
                                .child(div().h(px(18.)).text_size(px(T_S)).text_color(c(MUTED)).truncate().child(f.game.clone())),
                        )
                        .child(div().pt(px(8.)).pb(px(4.)).flex().child(act))
                        .when(pf.free_fresh.contains(&id), |d| {
                            d.child(div().absolute().top(px(8.)).left(px(8.)).px(px(7.)).h(px(20.)).flex().items_center().rounded(px(6.)).bg(c(FACE)).text_color(c(INK)).text_size(px(T_S)).font_weight(FontWeight::SEMIBOLD).child(root.tr("shop.new")))
                        }),
                )
                .into_any_element()
        };
    let tiles = crate::near::tiles("free-tiles", list.len(), tile_w, SGAP, key, false, tile_w + 90., tile);
    crate::scrolled::body("free-body", div().px(px(PAD + 4. - 6.)).pt(px(4.)).pb(px(24.)).child(tiles))
        .into_any_element()
}

/// A change to a watched thing in words: "Cheaper: 120 → 90 R$".
pub fn change_words(root: &Root, ch: &crate::itemwatch::Change) -> String {
    let vi = root.i18n.vietnamese;
    let n = |x: i64| thousands(x, vi);
    match ch.kind.as_str() {
        "sale" if ch.to == 0 => root.tr("watch.c.saleFree"),
        "sale" => root.i18n.tv("watch.c.sale", &[("to", &n(ch.to))]),
        "off" => root.tr("watch.c.off"),
        "price" => root.i18n.tv("watch.c.price", &[("from", &n(ch.from)), ("to", &n(ch.to))]),
        "resale" if ch.from == 0 => root.i18n.tv("watch.c.resaleNew", &[("to", &n(ch.to))]),
        "resale" => root.i18n.tv("watch.c.resale", &[("from", &n(ch.from)), ("to", &n(ch.to))]),
        "stock" => root.i18n.tv("watch.c.stock", &[("to", &n(ch.to))]),
        _ => String::new(),
    }
}

/// Marketplace › Watching: each thing with its numbers now, its last change, and how many of the accounts here have the
/// Robux for it (the thing Roblox's own page can't say: it only knows the one signed in).
fn watch_list(root: &Root, inner: f32, cx: &mut Context<Root>) -> AnyElement {
    let list = &root.profile.watch;
    if list.is_empty() {
        return div()
            .flex_1()
            .px(px(PAD + 4.))
            .py(px(60.))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(8.))
            .child(icon("bell", 26., c(DIM)))
            .child(div().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).child(root.tr("watch.empty")))
            .child(div().max_w(px(420.)).text_center().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("watch.emptySub")))
            .into_any_element();
    }
    let vi = root.i18n.vietnamese;
    let robux: Vec<i64> = root.accounts().iter().filter_map(|a| a.get("robux").and_then(serde_json::Value::as_i64)).collect();
    let now = crate::time::now_secs();
    let rows = list.iter().enumerate().map(|(i, w)| {
        let pic = (!w.pic.is_empty()).then(|| images::pic_pair(&w.pic, 56., 56., cx)).flatten();
        let cost = match (w.price.filter(|_| w.sale), w.resale) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        let state = match cost {
            Some(0) => div().text_color(c(OK)).child(root.tr("shop.free")),
            Some(p) => div().flex().items_center().gap(px(5.)).font_features(tnum()).child(icon("robux", 13., c(TEXT))).child(thousands(p, vi)),
            None => div().text_color(c(MUTED)).child(root.tr("shop.off")),
        };
        let can = cost.filter(|p| *p > 0).map(|p| robux.iter().filter(|r| **r >= p).count());
        let last = w.log.first().map(|ch| format!("{} · {}", change_words(root, ch), crate::time::ago(now - ch.at, &root.i18n)));
        let ware = Ware { id: w.id, bundle: false, name: w.name.clone(), maker: w.maker.clone(), price: w.price, pic: w.pic.clone(), about: String::new(), collectible: w.collectible.clone() };
        let id = w.id;
        hot(("watch-row", i))
            .px(px(12.))
            .py(px(10.))
            .flex()
            .items_center()
            .gap(px(14.))
            .rounded(px(R_L))
            .tones(c(RAISED), c(RAISED_2))
            .border_1()
            .border_color(if w.fresh { ca(OK, 0.5) } else { c(LINE) })
            .cursor_pointer()
            .child(div().size(px(56.)).flex_none().rounded(px(R_M)).bg(c(INSET)).overflow_hidden().flex().items_center().justify_center().map(|d| match pic {
                Some((p, k)) => d.child(fade_in(img(p).size_full().object_fit(ObjectFit::Contain), &k)),
                None => d.child(icon("image", 18., c(DIM))),
            }))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .child(div().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).truncate().child(w.name.clone()))
                    .child(div().text_size(px(T_S)).text_color(if w.fresh { c(OK) } else { c(MUTED) }).truncate().child(last.unwrap_or_else(|| root.tr("watch.noChange")))),
            )
            .children(w.left.map(|n| div().flex_none().text_size(px(T_S)).text_color(c(MUTED)).font_features(tnum()).child(root.i18n.tv("watch.left", &[("n", &thousands(n, vi))]))))
            .children(can.filter(|_| !robux.is_empty()).map(|n| {
                div().flex_none().text_size(px(T_S)).text_color(if n > 0 { c(TEXT_2) } else { c(MUTED) }).child(if n > 0 { root.i18n.tv("watch.canPay", &[("n", &n.to_string())]) } else { root.tr("watch.nonePay") })
            }))
            .child(div().flex_none().w(px(90.)).flex().justify_end().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).child(state))
            .child(IconButton::new(("watch-x", i), "x").size(28.).tip(root.tr("watch.stop")).on_click(cx.listener(move |r, _, _, cx| {
                cx.stop_propagation();
                if let Ok(false) = crate::itemwatch::toggle(id, "", "", "", "") {
                    r.profile.watch = crate::itemwatch::list();
                }
                cx.notify();
            })))
            .on_click(cx.listener(move |r, _, _, cx| {
                open_item(r, ware.clone(), cx);
                cx.notify();
            }))
    });
    let _ = inner;
    crate::scrolled::body(
        "watch-body",
        div()
            .px(px(PAD + 4.))
            .pt(px(4.))
            .pb(px(24.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .children(rows)
            .child(div().pt(px(8.)).text_size(px(T_S)).text_color(c(DIM)).child(root.i18n.tv("watch.how", &[("n", &(crate::itemwatch::EVERY.as_secs() / 60).to_string())]))),
    )
    .into_any_element()
}
