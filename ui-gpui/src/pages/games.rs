// Games: the games Bloxkeep's bot names (each with its wiki, some with news or a price list) as cards in their own
// pictures: yours first (followed, news unread, played here), then the popular ones. Opened, a game has its page: its
// picture with Follow and its roblox.com page on top, Roblox's figures under it, then Overview (your accounts in it
// over 7 days, the hours it is quiet, what it is: the old Game info), its News, Prices and Wiki. Since 2026-11-09 the
// old Game info page lives here (Home opens a game on it). The rules are GamesView.tsx's: a game shows only with the
// wiki the bot names; the commands are the app's (wikiGames, feedCards, playedGames, gameLooks, gameDetails, wikiGame,
// feedFollow, feedRead, feedSeen, wikiOpen, openGamePage, feedOpen, launch).
use std::collections::{HashMap, HashSet};

use gpui_kit::component::carousel::{Carousel, CarouselContent, CarouselItem, CarouselState};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};
use crate::val::s;
use crate::val::int;
use crate::ask::{answer, found, reply};

use crate::{
    images,
    kit::*,
    pages::{Page, home::clean},
    roblox,
    shell::{HomeTab, Root},
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
    /// Its badges, and which accounts have them (pages/badges.rs).
    Badges,
    /// Its private servers the accounts can join (pages/servers.rs).
    Servers,
    /// Its wiki, read here (pages/wiki.rs).
    Wiki,
    /// Its codes (giftcodes), working and expired: a tab of its own (the owner, 2026-11-19: on the overview it was
    /// "bẩn mắt"), there once a list has the game.
    Codes,
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

thread_local! {
    /// Where each view's pill stood in the capsule as it was last drawn (its left from the capsule's, its width):
    /// the yellow under the chosen one goes there.
    static VIEW_SPOTS: std::cell::RefCell<Vec<(f32, f32)>> = const { std::cell::RefCell::new(Vec::new()) };
    /// The yellow's own spring (left, width, their speeds, when last stepped): kept here, as the capsule is drawn anew
    /// by each view's header.
    static SLIDE: std::cell::Cell<Option<(f32, f32, f32, f32, std::time::Instant)>> = const { std::cell::Cell::new(None) };
}

/// The yellow stepped on toward `to` (left, width) on the soft spring (kit::BOUNCE_SOFT: 260 stiff, 20 damping, a
/// little past and back); true while it still moves.
fn slide_to(to: (f32, f32)) -> ((f32, f32), bool) {
    let now = std::time::Instant::now();
    let (mut x, mut w, mut vx, mut vw, at) = SLIDE.with(|s| s.get()).unwrap_or((to.0, to.1, 0., 0., now));
    let dt = now.duration_since(at).as_secs_f32();
    let moving = spring_step(&mut x, &mut vx, to.0, 260., 20., dt) | spring_step(&mut w, &mut vw, to.1, 260., 20., dt);
    SLIDE.with(|s| s.set(Some((x, w, vx, vw, now))));
    ((x, w), moving)
}

/// `x` (at speed `v`) carried `dt` seconds on toward `to` by a spring (stiffness `k`, damping `d`), in 2 ms steps and
/// at most 50 ms at once (a frame late after a pause doesn't throw it); true while it still moves, else it is put at
/// `to`, at rest.
fn spring_step(x: &mut f32, v: &mut f32, to: f32, k: f32, d: f32, dt: f32) -> bool {
    let mut dt = dt.min(0.05);
    while dt > 0. {
        let h = dt.min(0.002);
        *v += (-k * (*x - to) - d * *v) * h;
        *x += *v * h;
        dt -= h;
    }
    let moving = (*x - to).abs() > 0.3 || v.abs() > 4.;
    if !moving {
        (*x, *v) = (to, 0.);
    }
    moving
}

thread_local! {
    /// The capsule's moment as drawn: the one shown (kept after it ends, to close with), its own width as last laid
    /// out, and the box's width on its spring (width, speed, when last stepped).
    static LIVE_BOX: std::cell::RefCell<LiveBox> = const { std::cell::RefCell::new(LiveBox { shown: None, own: 0., w: 0., v: 0., at: None }) };
}

struct LiveBox {
    shown: Option<Live>,
    own: f32,
    w: f32,
    v: f32,
    at: Option<std::time::Instant>,
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
    // Each count stands in one place only, as on roblox.com: the unread messages here, the notifications on the bell, the
    // games' unread news on the games; the rail's Roblox button adds them up (shell.rs). The messages used to be
    // counted on the bell and here at once, and the news only on the rail, found nowhere inside (owner, 2026-11-20).
    let unread = root.games.corner.unread.1.max(0) as usize;
    let news = root.state.pointer("/feeds/unread").and_then(Value::as_i64).unwrap_or(0).max(0) as usize;
    let fresh = root.profile.watch.iter().any(|w| w.fresh);
    // (view, icon, words, a count beside it and its colour, a dot)
    let items: [(View, &str, &str, usize, u32, bool); 9] = [
        (View::Games, "grid", "news.title", news, FACE, false),
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
    // The capsule is black on every theme (ISLAND): what stands on it is white, whatever the theme's own text colour.
    // In the theme's text colour the eight icons not chosen were dark on black on a light theme, nothing to see or
    // aim at (the shots of every theme, 2026-11-18).
    let on_island = |a: f32| Hsla { a, ..rgb(0xffffff).into() };
    // The chosen view's yellow: one pill under them all that slides (and stretches) to the one chosen, where it was
    // last drawn; it follows as that one opens into its name. Before the first drawing each chosen pill is yellow itself.
    let chosen = items.iter().position(|it| it.0 == root.games.view).unwrap_or(0);
    let spots = VIEW_SPOTS.with(|s| s.borrow().clone());
    let (at, moving) = match spots.get(chosen) {
        Some(&to) => {
            let (at, moving) = slide_to(to);
            (Some(at), moving)
        }
        None => (None, false),
    };
    // (It goes a little past where it lands and back, but never out of the capsule: held to the first pill's left and
    // the last one's right.)
    let inside = spots.first().zip(spots.last()).map(|(a, b)| (a.0, b.0 + b.1));
    let at = at.map(|(x, w)| match inside {
        Some((lo, hi)) => {
            let x = x.max(lo);
            (x, w.min(hi - x))
        }
        None => (x, w),
    });
    let slider = at.map(|(x, w)| div().absolute().top(px(4.)).left(px(x)).w(px(w.max(0.))).h(px(32.)).rounded_full().bg(c(FACE)));
    let sliding = slider.is_some();
    // How much of each pill the yellow lies under now: its icon turns dark as much (iOS's segmented control).
    let under = move |i: usize| -> f32 {
        match (at, spots.get(i)) {
            (Some((x, w)), Some(&(l, pw))) if pw > 0. => ((x + w).min(l + pw) - x.max(l)).max(0.) / pw,
            _ => 0.,
        }
    };
    div()
        .relative()
        .on_children_prepainted(move |bounds, window, _| {
            if moving {
                window.request_animation_frame();
            }
            // The pills follow the slider (its own box first, when there is one); `live` may come after them.
            let pills = &bounds[usize::from(sliding)..];
            let Some(first) = pills.first() else { return };
            let origin = first.left() - px(4.);
            let now: Vec<(f32, f32)> = pills.iter().take(9).map(|b| (f32::from(b.left() - origin), f32::from(b.size.width))).collect();
            let moved = VIEW_SPOTS.with(|s| {
                let mut s = s.borrow_mut();
                let moved = *s != now;
                *s = now;
                moved
            });
            // (A frame being drawn can't be refreshed: the part it is in is asked for the next one.)
            if moved {
                window.request_animation_frame();
            }
        })
        .id("rbx-view").flex().flex_none().items_center().gap(px(2.)).h(px(40.)).p(px(4.)).rounded_full().bg(c(ISLAND))
        .children(slider)
        .children(items.into_iter().enumerate().map(
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
                // (The view shown: Bloxkeep's yellow, as every chosen pill: the owner, 2026-11-15, "đừng để màu trắng nữa".)
                .map(|d| match (on, sliding) {
                    (true, true) => d.tones(ca(WHITE, 0.), ca(WHITE, 0.)).text_color(crate::theme::on_text()),
                    (true, false) => d.tones(c(FACE), c(FACE)).text_color(crate::theme::on_text()),
                    _ => d.tones(on_island(0.), on_island(0.1)).cursor_pointer(),
                })
                // (Its icon turns dark as the yellow comes under it, and light again as it leaves.)
                .child(icon(ic, 16., if sliding { mix(on_island(0.6), crate::theme::on_text(), under(i).clamp(0., 1.)) } else if on { crate::theme::on_text() } else { on_island(0.6) }))
                .when(on, |d| {
                    d.child(
                        div()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .child(name.clone())
                            .with_animation(("rbx-view-name", i), Animation::new(std::time::Duration::from_millis(260)).with_easing(ease_out_quint()), |d, t| d.when(t < 1., |d| d.max_w(px(150. * t))).opacity(t)),
                    )
                })
                .when(!on && n > 0, |d| d.child(div().text_size(px(T_S)).font_weight(FontWeight::BOLD).font_features(tnum()).text_color(c(tone)).child(if n > 99 { "99+".to_string() } else { n.to_string() })))
                .when(!on && dot, |d| d.child(div().absolute().top(px(6.)).right(px(6.)).size(px(5.)).rounded_full().bg(c(OK))))
                .when(!on, |d| d.tooltip(move |_, cx| cx.new(|_| Tip(name.clone())).into()))
                .on_click(move |_, _, cx| {
                    let _ = this.update(cx, |r, cx| {
                        // The inbox's count is its unread messages: it opens on them, not on the chats it was last
                        // on (the owner, 2026-11-20: "it shows 7 and inside there is nothing", on Chat).
                        if view == View::Inbox && n > 0 {
                            r.inbox.tab = 1;
                        }
                        r.games.view = view;
                        r.games.home_library = None;
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
    // The box opens to the moment's own width and closes to nothing on a spring (critically damped: it doesn't open
    // past its words), the last moment still in it as it closes; a new one in place of another changes width from
    // where it stands. Its own width comes from its last layout (nothing yet: it opens from 0).
    let open = root.games.live.as_ref().filter(|l| l.busy || l.at.elapsed() < LIVE_FOR || (cfg!(feature = "shot") && std::env::var_os("BLOXKEEP_SHOT_LIVE_SHUT").is_none())).cloned();
    let now = std::time::Instant::now();
    let (l, w, own, moving) = LIVE_BOX.with(|b| {
        let mut b = b.borrow_mut();
        let opening = open.is_some();
        if let Some(l) = open {
            b.shown = Some(l);
        }
        let dt = b.at.map_or(0., |at| now.duration_since(at).as_secs_f32());
        let to = if opening { b.own } else { 0. };
        let (mut w, mut v) = (b.w, b.v);
        // (Still being laid out the first time: kept shut a frame, then it opens to what it measured.)
        let moving = spring_step(&mut w, &mut v, to, 220., 30., dt) || (opening && b.own == 0.);
        (b.w, b.v, b.at) = (w, v, moving.then_some(now));
        if !opening && !moving {
            b.shown = None;
        }
        b.shown.clone().map(|l| (l, w, b.own, moving))
    })?;
    let l = &l;
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
            // (Going: three dots that breathe in turn, each 0.9 s, in the motion layer (motion.rs): their frames draw
            // the window again, not the page. Still at 0.6 while the window is behind.)
            d.child(div().flex().gap(px(3.)).children((0..3).map(|k| {
                if !looping() {
                    return div().size(px(4.)).rounded_full().bg(c(TEXT_2)).opacity(0.6).into_any_element();
                }
                crate::motion::spot(4., 4., move |b, t, window, _| {
                    let seen = t.map_or(0.6, |t| {
                        let p = (((t / 0.9).fract() + k as f32 / 3.) % 1.) * std::f32::consts::TAU;
                        0.25 + 0.75 * (0.5 + 0.5 * p.sin())
                    });
                    crate::motion::fill_round(window, b, 2., c(TEXT_2).opacity(seen));
                })
                .into_any_element()
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
    let seen = if own > 0. { (w / own).clamp(0., 1.) } else { 0. };
    Some(
        div()
            .overflow_hidden()
            .flex_none()
            .flex()
            .on_children_prepainted(move |bounds, window, _| {
                let Some(b) = bounds.first() else { return };
                let measured = f32::from(b.size.width);
                let changed = LIVE_BOX.with(|x| {
                    let mut x = x.borrow_mut();
                    let changed = (x.own - measured).abs() > 0.5;
                    x.own = measured;
                    if changed && x.at.is_none() {
                        x.at = Some(std::time::Instant::now());
                    }
                    changed
                });
                if moving || changed {
                    window.request_animation_frame();
                }
            })
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    // The box is as wide as the moment less this (a width on the box would squeeze the moment's words
                    // as it lays out, and the moment would never measure its own width); before its first measure,
                    // all of it.
                    .mr(px(-if own > 0. { (own - w).max(0.) } else { 10_000. }))
                    .opacity(seen * seen)
                    // A hairline between the views and the moment: they are two things in one capsule.
                    .child(div().w(px(1.)).h(px(16.)).mx(px(6.)).flex_none().bg(ca(WHITE, 0.14)))
                    .child(part),
            )
            .into_any_element(),
    )
}

/// The room a header `width` wide has at each side of the capsule in its middle (as it was last drawn; about 410 px
/// before that): what stands there is given no more, or the capsule lay over it (the search box in a window about
/// 1100 px wide, the shots of 2026-11-18).
pub fn beside_island(width: f32) -> f32 {
    let island = VIEW_SPOTS.with(|s| s.borrow().last().map(|l| l.0 + l.1 + 8.)).unwrap_or(410.);
    ((width - island) / 2. - 12.).max(0.)
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
    pub home_library: Option<HomeTab>,
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
    /// Each game's codes (game_codes.rs: Roblox Den and its wiki), by place: None while asked. Whether every working
    /// one is shown (else the first few), and the expired ones.
    pub codes: HashMap<i64, Option<Value>>,
    pub codes_all: bool,
    pub codes_old: bool,
    social: Option<GameSocial>,
    community: Option<GameCommunity>,
    /// The game page's scroll (a shot scrolls it down with BLOXKEEP_SHOT_SCROLL).
    pub page_scroll: ScrollHandle,
    join_search: Option<Entity<InputState>>,
    join_scroll: UniformListScrollHandle,
    pub kind: Option<String>,
    /// A game's news: only this source's posts (none: every source).
    pub source: Option<String>,
    /// The news read before the game was shown this time, and when that was: what came after keeps its New mark for
    /// a minute on screen; then the mark moves up to what has been read since.
    pub mark: Option<(String, f64, std::time::Instant)>,
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
    /// Each chart row's page (the arrows): the first game shown, the way it last went (-1 back, 1 on) and which step
    /// it was (its slide starts again for the next).
    row_at: HashMap<String, (usize, f32, u32)>,
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
    /// The games whose wide picture was asked for, and those drawn wide (art) with none yet, noted while drawing:
    /// ask_covers asks Roblox for them once the frame is made. What is drawn decides: a list kept apart from the drawing
    /// left games shown wide with their 150 px icon blown up, blurred (Throw a Seed, the owner 2026-11-20 and -21).
    asked_covers: HashSet<i64>,
    need_covers: std::cell::RefCell<HashSet<i64>>,
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
    /// A game's events that are on or to come (Roblox's Events), by place: asked once the account's sign-in is in.
    pub events: HashMap<i64, Vec<roblox::GameEvent>>,
    asked_events: HashSet<i64>,
    /// How often each asking that came back empty was asked again (may_retry): an empty answer can be Roblox's true
    /// one (a game with no pictures, no details), so it isn't asked forever.
    retries: HashMap<String, u8>,
    /// The gameplay clips the tiles play under the mouse (pages/clips.rs).
    pub clips: crate::pages::clips::Clips,
    pub stays: HashMap<i64, Vec<Value>>,
    /// The country pick beside Play (region_button): each game's servers with their country (None while asked), its
    /// menu open, when it last closed, a Play waiting on the answer to take a server, and a line saying none was there.
    pub regions: HashMap<i64, Option<Result<Value, String>>>,
    pub region_menu: bool,
    /// Each game's favourite and vote of the account looked through, by universe.
    pub mine: HashMap<i64, Mine>,
    region_closed: Option<std::time::Instant>,
    region_play: Option<i64>,
    region_note: Option<String>,
    pub found: Option<(String, Option<Result<Vec<roblox::Game>, String>>)>,
    /// The communities the same search found (Roblox's Search › Communities), under its games.
    pub clubs_found: Option<(String, Option<Result<Vec<roblox::ClubHit>, String>>)>,
    /// Each change of the box's words, so a search asked while typing goes only for the last of them.
    pub typed: u64,
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
    home_games: Option<(i64, roblox::HomeGames)>,
    home_asked: Option<(i64, std::time::Instant)>,
    recommended_all: bool,
    /// The game page's switch to another game (GamesView.tsx's Switcher): open, when it was last closed by a click
    /// outside (so a click on the name closes it rather than opening it again), and its search box (made while it
    /// is open with more than 6 games; a box needs the window, so `settle_words` makes it).
    switching: bool,
    switch_closed: Option<std::time::Instant>,
    switch_q: Option<Entity<InputState>>,
    /// Roblox's own search for what the switch's box holds (asked a third of a second after the last key): the
    /// words asked and the answer, None while it comes.
    switch_found: Option<(String, Option<Result<Vec<roblox::Game>, String>>)>,
    switch_typed: u64,
}

/// What the account looked through has done with a game: favourited it, voted it up or down (None: neither, or not
/// known yet), and whether a change is on its way.
#[derive(Clone, Copy, Default)]
pub struct Mine {
    of: i64,
    asked: bool,
    fav: Option<bool>,
    vote: Option<bool>,
    busy: bool,
    /// Roblox takes no vote from this account (it hasn't played the game): the thumbs are dimmed and say why.
    no_vote: bool,
}

struct GameSocial {
    universe: i64,
    viewer: String,
    asked: std::time::Instant,
    links: Vec<Value>,
}

fn social_viewer(root: &Root) -> Option<&str> {
    let eligible = crate::pages::friends::able_ref(root);
    match root.friends.account.as_deref() {
        Some(id) => eligible.into_iter().find(|a| s(a, "id") == id).map(|a| s(a, "id")),
        None => eligible.first().map(|a| s(a, "id")),
    }
}

fn community_viewer(root: &Root) -> &str {
    let (_, viewer) = crate::pages::friends::looking_ref(root);
    viewer.filter(|a| s(a, "realm").is_empty()).map_or("", |a| s(a, "id"))
}

struct GameCommunity {
    group: i64,
    viewer: String,
    request: std::time::Instant,
    busy: bool,
    data: Option<Value>,
    error: Option<String>,
}

fn request_community(root: &mut Root, group: i64, action: &'static str, cx: &mut Context<Root>) {
    let Some(link) = root.link.clone() else { return };
    let viewer = community_viewer(root).to_string();
    let data = root.games.community.take().filter(|p| p.group == group && p.viewer == viewer).and_then(|p| p.data);
    let request = std::time::Instant::now();
    root.games.community = Some(GameCommunity { group, viewer: viewer.clone(), request, busy: true, data, error: None });
    let rx = link.call("gameCommunity", json!({ "id": viewer, "groupId": group, "action": action }));
    answer(rx, cx, move |r, result, cx| {
        let unavailable = r.tr("gp.communityError");
        let Some(p) = r.games.community.as_mut().filter(|p| p.request == request) else { return };
        p.busy = false;
        match result {
            Ok(Ok(v)) => p.data = Some(v),
            Ok(Err(e)) => p.error = Some(e.to_string()),
            Err(_) => p.error = Some(unavailable),
        }
        cx.notify();
    });
    cx.notify();
}

fn open_community(root: &mut Root, group: i64, cx: &mut Context<Root>) {
    if root.games.community.as_ref().is_some_and(|p| p.group == group && p.viewer == community_viewer(root)) {
        root.games.community = None;
        cx.notify();
    } else {
        request_community(root, group, "", cx);
    }
}

fn community_panel(root: &Root, group: i64, cx: &mut Context<Root>) -> Option<Div> {
    let p = root.games.community.as_ref().filter(|p| p.group == group && p.viewer == community_viewer(root))?;
    let data = p.data.as_ref().unwrap_or(&Value::Null);
    let title = s(data, "name");
    let head = section_head(if title.is_empty() { root.tr("gp.communityTitle") } else { title.to_string() }, None)
        .child(IconButton::new("gp-community-close", "x").size(30.).tip(root.tr("common.close")).on_click(cx.listener(|r, _, _, cx| {
            r.games.community = None;
            cx.notify();
        })));
    let state = s(data, "state");
    let viewer = root.accounts().iter().find(|a| s(a, "id") == p.viewer).map(crate::pages::accounts::label);
    let mut panel = surface().p(px(16.)).flex().flex_col().gap(px(9.)).child(head);
    if let Some(n) = data["members"].as_i64() {
        panel = panel.child(div().text_size(px(T_S)).text_color(c(MUTED)).child(
            root.i18n.tv("gp.communityMembers", &[("n", &thousands(n))])));
    }
    if !s(data, "description").is_empty() {
        panel = panel.child(div().text_size(px(T_M)).line_clamp(3).text_color(c(TEXT_2)).child(s(data, "description").to_string()));
    }
    let mut actions = div().flex().flex_wrap().items_center().gap(px(8.)).text_size(px(T_S)).text_color(c(MUTED));
    if let Some(viewer) = viewer {
        actions = actions.child(root.i18n.tv("gp.communityAs", &[("name", &viewer)]));
    }
    match state {
        "join" => {
            actions = actions.child(Button::new("gp-community-join", root.tr(if data["approval"] == true { "gp.communityRequest" } else { "gp.communityJoin" }))
                .small().disabled(p.busy).on_click(cx.listener(move |r, _, _, cx| request_community(r, group, "join", cx))));
        }
        "member" => {
            let group_name = title.to_string();
            let id = p.viewer.clone();
            actions = actions.child(Button::new("gp-community-leave", root.tr("gp.communityLeave")).small().disabled(p.busy)
                .on_click(cx.listener(move |r, _, window, cx| {
                    let id = id.clone();
                    r.confirm(r.tr("gp.communityLeave"), group_name.clone(), r.tr("gp.communityLeave"), true, None, window, cx, move |r, cx| {
                        if community_viewer(r) == id {
                            request_community(r, group, "leave", cx);
                        }
                    });
                })));
        }
        "pending" => actions = actions.child(root.tr("gp.communityPending")),
        "owner" => actions = actions.child(root.tr("gp.communityOwner")),
        "guest" => actions = actions.child(root.tr("gp.communityGuest")),
        _ => {}
    }
    if p.busy {
        actions = actions.child(spinner(13., c(MUTED))).child(root.tr("av.reading"));
    } else if p.error.is_some() {
        actions = actions.child(Button::new("gp-community-retry", root.tr("srv.retry")).small()
            .on_click(cx.listener(move |r, _, _, cx| request_community(r, group, "", cx))));
    }
    Some(panel.child(actions).when_some(p.error.clone(), |d, text| d.child(div().text_size(px(T_S)).text_color(c(BAD)).child(text))))
}

/// Whether the account looked through has favourited or voted this game: asked once for each (its own sign-in).
fn want_mine(r: &mut Root, universe: i64, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = crate::pages::friends::signed(r).filter(|(_, _, c)| universe > 0 && !c.is_empty()) else { return };
    if r.games.mine.get(&universe).is_some_and(|m| m.of == user && m.asked) {
        return;
    }
    r.games.mine.insert(universe, Mine { of: user, asked: true, ..Default::default() });
    let rx = roblox::run(move || roblox::game_mine(&cookie, universe));
    crate::ask::reply(rx, cx, move |r, got, cx| {
        if let (Ok((fav, vote, can)), Some(m)) = (got, r.games.mine.get_mut(&universe).filter(|m| m.of == user && !m.busy)) {
            (m.fav, m.vote, m.no_vote) = (fav, vote, can == Some(false));
            cx.notify();
        }
    });
}

/// Favourite (`vote` None) or vote the game up or down as the account looked through: shown at once with its count,
/// put back with Roblox's reason when it refuses. A click on the thumb already chosen takes the vote back, as Roblox's
/// own page does (it said "a vote given stays", and nothing could be unliked: the owner, 2026-11-20).
fn set_mine(r: &mut Root, place: i64, universe: i64, vote: Option<bool>, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = crate::pages::friends::signed(r).filter(|(_, _, c)| !c.is_empty()) else {
        r.toast = Some((r.tr("gp.needSign"), "bad".into()));
        cx.notify();
        return;
    };
    let before = r.games.mine.get(&universe).copied().filter(|m| m.of == user).unwrap_or(Mine { of: user, asked: true, ..Default::default() });
    // (The same thumb again: the vote taken back.)
    let again = vote.is_some() && before.vote == vote;
    if before.busy {
        return;
    }
    if vote.is_some() && before.no_vote && !again {
        r.toast = Some((r.tr("gp.votePlayFirst"), "bad".into()));
        cx.notify();
        return;
    }
    let mut now = Mine { busy: true, ..before };
    let fav_to = !before.fav.unwrap_or(false);
    match vote {
        None => now.fav = Some(fav_to),
        Some(up) => now.vote = if again { None } else { Some(up) },
    }
    r.games.mine.insert(universe, now);
    // The counts follow at once (and go back with the rest when Roblox refuses).
    let counts = move |r: &mut Root, back: bool| {
        let Some(d) = r.games.details.get_mut(&place) else { return };
        let mut add = |key: &str, by: i64| {
            if let Some(n) = d.get(key).and_then(Value::as_i64) {
                d[key] = json!((n + if back { -by } else { by }).max(0));
            }
        };
        match vote {
            None => add("favorites", if fav_to { 1 } else { -1 }),
            Some(up) => {
                if !again {
                    add(if up { "upVotes" } else { "downVotes" }, 1);
                }
                if let Some(was) = before.vote {
                    add(if was { "upVotes" } else { "downVotes" }, -1);
                }
            }
        }
    };
    counts(r, false);
    cx.notify();
    let cookie_back = cookie.clone();
    let rx = roblox::run(move || match vote {
        None => roblox::set_favorite(&cookie, universe, fav_to),
        Some(_) if again => roblox::clear_vote(&cookie, universe, place),
        Some(up) => roblox::set_vote(&cookie, universe, up),
    });
    crate::ask::reply(rx, cx, move |r, done, cx| {
        let ours = r.games.mine.get(&universe).is_some_and(|m| m.of == user);
        if let Err(why) = done {
            // (A vote's 403 is Roblox's "Play game required": the account hasn't played it.)
            let play_first = vote.is_some() && !again && why.contains("403");
            if ours {
                r.games.mine.insert(universe, Mine { no_vote: before.no_vote || play_first, ..before });
                counts(r, true);
            }
            r.toast = Some(if play_first {
                (r.tr("gp.votePlayFirst"), "bad".into())
            } else {
                (r.i18n.tv("rs.failed", &[("why", if why.is_empty() { "Roblox" } else { &why })]), "bad".into())
            });
        } else if let Some(m) = r.games.mine.get_mut(&universe).filter(|_| ours) {
            m.busy = false;
            // A vote taken back: what Roblox holds now, read back, so the thumbs show its answer, not a guess.
            if again {
                let rx = roblox::run(move || roblox::game_mine(&cookie_back, universe));
                crate::ask::reply(rx, cx, move |r, got, cx| {
                    if let (Ok((_, held, _)), Some(m)) = (got, r.games.mine.get_mut(&universe).filter(|m| m.of == user && !m.busy)) {
                        m.vote = held;
                        // (Roblox kept the vote after all: its count back too.)
                        if held.is_some() && held == before.vote {
                            counts(r, true);
                        }
                        cx.notify();
                    }
                });
            }
        }
        cx.notify();
    });
}

fn want_social(r: &mut Root, universe: i64, link: &crate::link::Link, cx: &mut Context<Root>) {
    let Some(viewer) = social_viewer(r).filter(|_| universe > 0).map(str::to_string) else { return };
    let same = |social: &GameSocial| social.universe == universe && social.viewer == viewer;
    if r.games.social.as_ref().is_some_and(|social| same(social) && social.asked.elapsed().as_secs() < 90) {
        return;
    }
    let asked = std::time::Instant::now();
    let links = r.games.social.take().filter(same).map_or_else(Vec::new, |social| social.links);
    r.games.social = Some(GameSocial { universe, viewer: viewer.clone(), asked, links });
    let generation = r.games.sign_gen;
    let rx = link.call("gameSocialLinks", json!({ "id": viewer, "universeId": universe }));
    cx.spawn(async move |this, cx| {
        let links = rx.await.ok().and_then(Result::ok).and_then(|v| v.as_array().cloned());
        let _ = this.update(cx, |r, cx| {
            if r.games.sign_gen == generation
                && let Some(social) = &mut r.games.social
                && social.asked == asked
            {
                social.links = links.unwrap_or_default();
                cx.notify();
            }
        });
    }).detach();
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

fn icon_of(root: &Root, g: &Game) -> Option<String> {
    crate::pages::home::look(root, g.place, "icon").map(str::to_string).or_else(|| g.icon.clone().filter(|u| !u.is_empty()))
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
                // (A logo too small to read as one, a favicon-sized mark, is left for the name in words; so is one about as
                // tall as wide, a mascot or an egg rather than lettering: the owner's screenshot of 2026-11-15 had an egg in
                // place of the game's name, "what ?". A name drawn in letters runs wide.)
                .filter(|i| {
                    let (w, h) = (i.get("width").and_then(Value::as_i64).unwrap_or(0), i.get("height").and_then(Value::as_i64).unwrap_or(0));
                    w >= 120 && h > 0 && w as f64 >= 1.6 * h as f64
                })
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
    // BLOXKEEP_SHOT_NOLOGO=1: a shot's games have no logo, their names stand in words (a long name's room).
    #[cfg(feature = "shot")]
    if std::env::var("BLOXKEEP_SHOT_NOLOGO").is_ok() {
        return None;
    }
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

/// Where a game's details from last time are kept (Roblox's public words on it; never the accounts' names): the cache
/// folder, a hidden shot's own (BLOXKEEP_KEPT).
fn details_file(place: i64) -> std::path::PathBuf {
    images::kept_dir("game-details").join(format!("{place}.json"))
}

/// The details kept from last time, if any.
fn details_kept(place: i64) -> Option<Value> {
    let mut v: Value = crate::net::saved("Game details", &details_file(place))?;
    // (The accounts' week on it is the vault's, asked fresh.)
    v["accounts"] = json!([]);
    Some(v)
}

/// Keeps a game's details for the next start, without the accounts on it (they are the vault's, and named).
fn keep_details(place: i64, v: &Value, cx: &mut Context<Root>) {
    let mut kept = v.clone();
    if let Some(o) = kept.as_object_mut() {
        o.remove("accounts");
    }
    let path = details_file(place);
    cx.background_spawn(async move {
        let r = path.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|_| std::fs::write(&path, kept.to_string()));
        crate::net::wrote("Game details", &path, r);
    })
    .detach();
}

/// A game's details from Roblox (its words, players, votes, creator), asked once for `place`: its page, and Home's
/// hero for the game on the stage.
pub fn want_details(r: &mut Root, link: &crate::link::Link, place: i64, cx: &mut Context<Root>) {
    if place > 0 && r.games.asked_details.insert(place) {
        // What Roblox said last time shows at once (Home's words and figures, the page's head), and Roblox's answer
        // replaces it when it comes: it took a second or more to come each start (the owner, 2026-11-20: "có cách nào
        // khiến nó tải ngay lập tức không").
        if !r.games.details.contains_key(&place)
            && let Some(v) = details_kept(place)
        {
            r.games.details.insert(place, v);
        }
        let rx = link.call("gameDetails", json!({ "placeId": place }));
        cx.spawn(async move |this, cx| {
            let got = rx.await.ok().and_then(Result::ok).filter(|v| !v.is_null());
            let failed = got.is_none();
            let _ = this.update(cx, |r, cx| {
                if let Some(v) = got {
                    let universe = int(&v, "universe");
                    keep_details(place, &v, cx);
                    r.games.details.insert(place, v);
                    settle_stays(r, place);
                    want_shots(r, place, universe, cx);
                    want_events(r, place, universe, cx);
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
    }
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
    // (Its events once more here: the sign-in may come after the details did.)
    if let Some(universe) = r.games.details.get(&place).map(|v| int(v, "universe")).filter(|_| place > 0) {
        want_events(r, place, universe, cx);
        want_mine(r, universe, cx);
        let name = r.games.details.get(&place).map(|v| s(v, "name").to_string()).unwrap_or_default();
        want_codes(r, place, universe, name, cx);
        if r.games.tab == Tab::Overview {
            want_social(r, universe, &link, cx);
        }
    }
    if place > 0 && !r.games.stays.contains_key(&place) {
        settle_stays(r, place);
    }
    want_details(r, &link, place, cx);
    // The countries its servers run in, asked as soon as its page shows, once a run (the owner, 2026-11-20: not only
    // when the country menu beside Play is opened). Each round shows what it found at once, so the menu fills while
    // the rest are asked; a failed or cut-short asking is asked again when the menu opens.
    if place > 0 && r.games.view == View::Games && !r.games.regions.contains_key(&place) {
        load_regions(r, place, cx);
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
            // (Never read before: nothing is New against that, or every post was "New" at once, the owner, 2026-11-15.
            // The mark goes when the page is left (shell), so a later visit counts from the last read.)
            // (A game that posts every few minutes had every card "New" while its page stayed open: a card is New for
            // its first minute on screen.)
            let stale = r.games.mark.as_ref().is_none_or(|(id, _, at)| *id != open.id || at.elapsed().as_secs() >= 60);
            if stale {
                let next = if follow && read > 0. { read } else { f64::INFINITY };
                r.games.mark = Some((open.id.clone(), next, std::time::Instant::now()));
                // (Redrawn when the minute is up while some card shows New, so the marks go by themselves.)
                let marked = r.games.cards.iter().flatten().any(|c| c.get("game").and_then(Value::as_str) == Some(open.id.as_str()) && c.get("at").and_then(Value::as_f64).is_some_and(|at| at > next));
                if marked {
                    cx.spawn(async move |this, cx| {
                        cx.background_executor().timer(std::time::Duration::from_secs(61)).await;
                        let _ = this.update(cx, |_, cx| cx.notify());
                    })
                    .detach();
                }
            }
            if follow && int(n, "unread") > 0 && r.games.read_sent.as_deref() != Some(open.id.as_str()) {
                r.games.read_sent = Some(open.id.clone());
                link.send("feedRead", json!({ "game": open.id }));
            }
        } else if r.games.tab != Tab::News {
            r.games.mark = None;
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
    if !r.games.picking.as_ref().zip(r.games.open.as_ref()).is_some_and(|((game, _), open)| *game == open.id) {
        r.games.join_search = None;
    } else if r.games.join_search.is_none() {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(r.tr("gp.pickSearch")));
        cx.subscribe(&search, |r: &mut Root, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                r.games.join_scroll.0.borrow().base_handle.set_offset(point(px(0.), px(0.)));
                cx.notify();
            }
        }).detach();
        r.games.join_scroll.0.borrow().base_handle.set_offset(point(px(0.), px(0.)));
        // Hidden shots use the same editor Change event as typed input.
        #[cfg(feature = "shot")]
        if let Ok(query) = std::env::var("BLOXKEEP_SHOT_JOIN_QUERY") {
            search.update(cx, |input, cx| input.replace_all(query, window, cx));
        }
        r.games.join_search = Some(search);
    }
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
        r.games.switch_found = None;
        return;
    }
    // (Always a box now, however few games there are: it finds any game on Roblox too.)
    if r.games.switch_q.is_some() {
        return;
    }
    let hint = r.i18n.tv("games.search", &[("n", &list(r).len().to_string())]);
    let e = cx.new(|cx| InputState::new(window, cx).placeholder(hint));
    cx.subscribe(&e, |r: &mut Root, _, ev: &InputEvent, cx| match ev {
        InputEvent::PressEnter { .. } => {
            // The best of yours, else Roblox's first.
            if let Some(id) = switch_list(r, cx).first().map(|g| g.id.clone()) {
                switch_to(r, &id);
            } else if let Some(g) = switch_roblox(r).into_iter().next() {
                r.games.switching = false;
                open_place(r, g.place, &g.name);
            }
            cx.notify();
        }
        InputEvent::Change => switch_ask_soon(r, cx),
        _ => {}
    })
    .detach();
    e.update(cx, |st, cx| st.focus(window, cx));
    r.games.switch_q = Some(e);
}

/// What the switch's box holds, as typed.
fn switch_words(root: &Root, cx: &App) -> String {
    root.games.switch_q.as_ref().map(|e| e.read(cx).value().trim().to_string()).unwrap_or_default()
}

/// The games the switch lists: every game, the ones that answer the words in its box best first (name_match).
fn switch_list(root: &Root, cx: &App) -> Vec<Game> {
    let q = switch_words(root, cx);
    ranked(list(root).iter().cloned(), &q, |g| g.name.as_str())
}

/// Roblox's games for the box's words that aren't already among yours.
fn switch_roblox(root: &Root) -> Vec<roblox::Game> {
    let mine: HashSet<i64> = list(root).iter().map(|g| g.place).filter(|p| *p > 0).collect();
    match &root.games.switch_found {
        Some((_, Some(Ok(l)))) => l.iter().filter(|g| !mine.contains(&g.place)).take(8).cloned().collect(),
        _ => Vec::new(),
    }
}

/// Roblox asked for the box's words a third of a second after the last key (two letters or more).
fn switch_ask_soon(r: &mut Root, cx: &mut Context<Root>) {
    let q = switch_words(r, cx);
    r.games.switch_typed += 1;
    cx.notify();
    if q.chars().count() < 2 {
        r.games.switch_found = None;
        return;
    }
    if r.games.switch_found.as_ref().is_none_or(|(at, _)| *at != q) {
        // (The last answer's games stay until the new one comes.)
        let kept = r.games.switch_found.take().and_then(|(_, got)| got).filter(|g| g.as_ref().is_ok_and(|g| !g.is_empty()));
        r.games.switch_found = Some((q.clone(), kept));
    }
    let seq = r.games.switch_typed;
    cx.spawn(async move |this, cx| {
        cx.background_executor().timer(std::time::Duration::from_millis(330)).await;
        let go = this.update(cx, |r, _| r.games.switch_typed == seq).unwrap_or(false);
        if !go {
            return;
        }
        let ask = q.clone();
        let got = roblox::run(move || roblox::search(&ask)).await.unwrap_or_else(|_| Err(String::new()));
        if let Err(e) = &got {
            crate::net::note_failed("Game switch: Roblox's search didn't answer.", "https://apis.roblox.com/search-api/omni-search", e);
        }
        let _ = this.update(cx, |r, cx| {
            if r.games.switch_found.as_ref().is_some_and(|(at, _)| *at == q) {
                r.games.switch_found = Some((q, Some(got)));
                cx.notify();
            }
        });
    })
    .detach();
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

/// A game's events, asked once its details have named its universe and an account's sign-in is in (Roblox gives
/// them to a signed-in account only). A refusal is in error.log (the Roblox tab's own line) and isn't asked again.
fn want_events(r: &mut Root, place: i64, universe: i64, cx: &mut Context<Root>) {
    // (A hidden shot with BLOXKEEP_SHOT_EVENTS reads its file, signed in or not.)
    let shot = cfg!(feature = "shot") && std::env::var("BLOXKEEP_SHOT_EVENTS").is_ok();
    let Some(cookie) = crate::pages::friends::signed(r).map(|s| s.2).filter(|c| !c.is_empty()).or_else(|| shot.then(String::new)) else { return };
    if universe <= 0 || !r.games.asked_events.insert(place) {
        return;
    }
    let rx = roblox::run(move || roblox::game_events(&cookie, universe));
    cx.spawn(async move |this, cx| {
        if let Ok(Ok(list)) = rx.await {
            let _ = this.update(cx, |r, cx| {
                r.games.events.insert(place, list);
                cx.notify();
            });
        }
    })
    .detach();
}

/// The game's codes, asked once a run for each game (the answer is kept a while there too).
fn want_codes(r: &mut Root, place: i64, universe: i64, name: String, cx: &mut Context<Root>) {
    if universe <= 0 || name.is_empty() || r.games.codes.contains_key(&place) {
        return;
    }
    let Some(link) = r.link.clone() else { return };
    r.games.codes.insert(place, None);
    let rx = link.call("gameCodes", json!({ "name": name, "universe": universe, "placeId": place }));
    cx.spawn(async move |this, cx| {
        let got = rx.await;
        let _ = this.update(cx, |r, cx| {
            r.games.codes.insert(place, Some(match got {
                Ok(Ok(v)) => v,
                _ => json!({ "codes": [] }),
            }));
            cx.notify();
        });
    })
    .detach();
}

/// A code to copy, as About shows it: "Code", the code in the accent, Copy (the owner picked it, 2026-11-19). An
/// expired one quieter, its code struck out, still copyable.
fn code_chip(root: &Root, id: (&'static str, usize), code: &str, works: bool, cx: &mut Context<Root>) -> Stateful<Div> {
    let copy = code.to_string();
    div()
        .id(id)
        .flex_none()
        .h(px(32.))
        .pl(px(12.))
        .pr(px(5.))
        .flex()
        .items_center()
        .gap(px(8.))
        .rounded_full()
        .when(works, |d| d.bg(ca(FACE, 0.14)).border_1().border_color(ca(FACE, 0.35)).hover(|d| d.bg(ca(FACE, 0.2))))
        .when(!works, |d| d.bg(ca(WHITE, 0.04)).border_1().border_color(ca(WHITE, 0.12)).hover(|d| d.bg(ca(WHITE, 0.08))))
        .cursor_pointer()
        .text_size(px(T_S + 0.5))
        .child(div().text_color(c(MUTED)).child(root.tr("gp.code")))
        .child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if works { c(FACE) } else { c(MUTED) })
                .when(!works, |d| d.line_through())
                .child(copy.clone()),
        )
        .child(
            div()
                .h(px(22.))
                .px(px(9.))
                .flex()
                .items_center()
                .rounded_full()
                .when(works, |d| d.bg(c(FACE)).text_color(c(INK)))
                .when(!works, |d| d.bg(ca(WHITE, 0.1)).text_color(c(TEXT_2)))
                .text_size(px(T_XS + 0.5))
                .font_weight(FontWeight::SEMIBOLD)
                .child(root.tr("gp.copy")),
        )
        .on_click(cx.listener(move |r, _, _, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()));
            r.toast = Some((r.i18n.tv("gp.codeCopied", &[("code", &copy)]), "ok".into()));
            cx.notify();
        }))
}

/// The Codes table's heads, for the expired codes' table under it.
fn heads_again(root: &Root, col: f32, added: f32, copy: f32) -> Div {
    let head = |words: String| div().text_size(px(T_XS)).font_weight(FontWeight::SEMIBOLD).text_color(c(MUTED)).child(words.to_uppercase());
    div()
        .flex()
        .items_center()
        .gap(px(16.))
        .px(px(16.))
        .h(px(36.))
        .child(head(root.tr("gc.colCode")).w(px(col)).flex_none())
        .child(head(root.tr("gc.colReward")).flex_1().min_w_0())
        .child(head(root.tr("gc.colAdded")).w(px(added)).flex_none())
        .child(div().w(px(copy)).flex_none())
}

/// How many working codes show before "Show all".
const CODES_FIRST: usize = 8;

/// The game's codes (giftcodes): the working ones, each to copy with what it gives and "New" for a week, the rest under
/// Show all; the expired ones behind their own button, as Roblox Den's page keeps them; where they came from at the
/// foot. None while asked or when no list has the game.
fn codes_block(root: &Root, place: i64, cx: &mut Context<Root>) -> Option<Div> {
    let v = root.games.codes.get(&place)?.as_ref()?;
    let list: Vec<&Value> = v.get("codes").and_then(Value::as_array).map(|l| l.iter().collect()).unwrap_or_default();
    if list.is_empty() {
        return None;
    }
    let works = |c: &&Value| c.get("works").and_then(Value::as_bool).unwrap_or(false);
    let live: Vec<&Value> = list.iter().copied().filter(works).collect();
    let old: Vec<&Value> = list.iter().copied().filter(|c| !works(c)).collect();
    let now = crate::time::now_secs();
    // A table (the owner, 2026-11-20: "sao ko làm dạng bảng đi", the chips and rewards apart read as two lists): the
    // code, what it gives, when it was added, Copy; a line between rows, the heads small above them, as the code sites'
    // tables. The code's column as wide as the longest code needs (about 8.4 px a letter), within reason.
    let longest = list.iter().map(|c| s(c, "code").chars().count()).max().unwrap_or(8) as f32;
    let col = (longest * 8.4 + 70.).clamp(170., 340.);
    const ADDED: f32 = 120.;
    const COPY: f32 = 84.;
    let row = |i: usize, code: &Value, on: bool, cx: &mut Context<Root>| {
        let added = crate::time::iso_secs(s(code, "added"));
        let new = on && added.is_some_and(|t| now - t < 7 * 86_400);
        let copy = s(code, "code").to_string();
        div()
            .id((if on { "gc-live" } else { "gc-old" }, i))
            .flex()
            .items_center()
            .gap(px(16.))
            .px(px(16.))
            .min_h(px(46.))
            .py(px(6.))
            .border_t_1()
            .border_color(ca(WHITE, 0.06))
            .hover(|d| d.bg(ca(WHITE, 0.03)))
            .child(
                div()
                    .w(px(col))
                    .flex_none()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(div().min_w_0().truncate().font_weight(FontWeight::SEMIBOLD).text_size(px(T_M)).text_color(if on { c(FACE) } else { c(MUTED) }).when(!on, |d| d.line_through()).child(copy.clone()))
                    .when(new, |d| {
                        d.child(div().flex_none().h(px(20.)).px(px(7.)).flex().items_center().rounded(px(5.)).bg(ca(ONLINE, 0.16)).text_color(c(ONLINE)).text_size(px(T_XS)).font_weight(FontWeight::SEMIBOLD).child(root.tr("gc.new")))
                    }),
            )
            .child(div().flex_1().min_w_0().text_size(px(T_M - 0.5)).line_height(px(20.)).text_color(if on { c(TEXT_2) } else { c(MUTED) }).child(s(code, "reward").to_string()))
            .child(div().w(px(ADDED)).flex_none().text_size(px(T_S)).text_color(c(MUTED)).child(added.map(|t| crate::time::ago(now - t, &root.i18n)).unwrap_or_else(|| "—".into())))
            .child(
                div().w(px(COPY)).flex_none().flex().justify_end().child(
                    Button::new(SharedString::from(format!("gc-copy-{}-{i}", if on { "l" } else { "o" })), root.tr("gp.copy"))
                        .small()
                        .icon("copy")
                        .on_click(cx.listener(move |r, _, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()));
                            r.toast = Some((r.i18n.tv("gp.codeCopied", &[("code", &copy)]), "ok".into()));
                            cx.notify();
                        })),
                ),
            )
    };
    let shown = if root.games.codes_all { live.len() } else { live.len().min(CODES_FIRST) };
    let mut rows = crate::kit::block().flex().flex_col().overflow_hidden().child(heads_again(root, col, ADDED, COPY));
    for (i, code) in live.iter().take(shown).enumerate() {
        rows = rows.child(row(i, code, true, cx));
    }
    if live.is_empty() {
        rows = rows.child(div().px(px(16.)).py(px(12.)).border_t_1().border_color(ca(WHITE, 0.06)).text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("gc.noneWorking")));
    }
    let link = |id: &'static str, words: String, open: bool, cx: &mut Context<Root>, f: fn(&mut Root)| {
        div()
            .id(id)
            .flex()
            .items_center()
            .gap(px(4.))
            .cursor_pointer()
            .text_size(px(T_M))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(c(FACE))
            .hover(|d| d.opacity(0.8))
            .child(words)
            .child(icon(if open { "chevronUp" } else { "chevronDown" }, 14., c(FACE)))
            .on_click(cx.listener(move |r, _, _, cx| {
                f(r);
                cx.notify();
            }))
    };
    let mut more = div().mt(px(6.)).flex().flex_wrap().gap_x(px(22.)).gap_y(px(8.));
    if live.len() > CODES_FIRST {
        let open = root.games.codes_all;
        more = more.child(link("gc-all", if open { root.tr("gp.less") } else { root.i18n.tv("gc.all", &[("n", &live.len().to_string())]) }, open, cx, |r| r.games.codes_all = !r.games.codes_all));
    }
    if !old.is_empty() {
        let open = root.games.codes_old;
        more = more.child(link("gc-old", root.i18n.tv(if open { "gc.hideOld" } else { "gc.showOld" }, &[("n", &old.len().to_string())]), open, cx, |r| r.games.codes_old = !r.games.codes_old));
    }
    let mut olds = crate::kit::block().flex().flex_col().overflow_hidden().child(heads_again(root, col, ADDED, COPY));
    if root.games.codes_old {
        for (i, code) in old.iter().enumerate() {
            olds = olds.child(row(i, code, false, cx));
        }
    }
    // Where they came from: each list, a click opens it.
    let mut foot = div().mt(px(10.)).flex().flex_wrap().items_center().gap_x(px(6.)).gap_y(px(4.)).text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("gc.from"));
    for (i, src) in v.get("sources").and_then(Value::as_array).into_iter().flatten().enumerate() {
        let url = s(src, "page").to_string();
        if i > 0 {
            foot = foot.child("·");
        }
        foot = foot.child(div().id(("gc-src", i)).cursor_pointer().text_color(c(TEXT_2)).hover(|d| d.text_color(c(TEXT))).child(s(src, "name").to_string()).on_click(move |_, _, cx| cx.open_url(&url)));
    }
    if v.get("caseSensitive").and_then(Value::as_bool).unwrap_or(false) {
        foot = foot.child("·").child(root.tr("gc.case"));
    }
    let count = root.i18n.tv("gc.count", &[("n", &live.len().to_string())]);
    Some(
        div()
            .flex()
            .flex_col()
            .child(heading(root.tr("gc.title"), Some(count)))
            .child(rows.mt(px(10.)))
            .child(more)
            .when(root.games.codes_old, |d| d.child(olds.mt(px(8.))))
            .child(foot),
    )
}

/// How long until a moment, in days and hours past a day ("2d 5h", "2 ngày 5 giờ"), else the short span.
fn left(secs: i64) -> String {
    let s = secs.max(0);
    let (d, h) = (s / 86_400, s % 86_400 / 3_600);
    let l = crate::lang::cur();
    let (day, hour) = (l.day_abbr, if l.code == "en" { "h" } else { l.units.2 });
    match d {
        0 => crate::time::span(s),
        _ if h == 0 => format!("{d}{day}"),
        _ => format!("{d}{day} {h}{hour}"),
    }
}

/// A word on a picture (an event's kind, its date): a solid chip on the picture, as the Home tiles' window count.
fn photo_chip(text: String, dot: Option<u32>) -> Div {
    div()
        .h(px(24.))
        .px(px(9.))
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6.))
        .rounded(px(7.))
        .bg(ca(GROUND, 0.86))
        .text_size(px(T_S))
        .font_weight(FontWeight::SEMIBOLD)
        .font_features(tnum())
        .text_color(c(TEXT))
        .when_some(dot, |d, tone| d.child(div().size(px(7.)).rounded_full().bg(c(tone))))
        .child(text)
}

/// Going to an event or not, from its Notify me: shown at once, sent to Roblox, put back (with a notice) when Roblox
/// refuses.
fn toggle_going(r: &mut Root, place: i64, id: String, cx: &mut Context<Root>) {
    let Some((_, _, cookie)) = crate::pages::friends::signed(r).filter(|s| !s.2.is_empty()) else { return };
    let ask = id.clone();
    let flip = move |r: &mut Root| -> Option<bool> {
        let e = r.games.events.get_mut(&place)?.iter_mut().find(|e| e.id == id)?;
        e.going = !e.going;
        e.count = e.count.map(|n| (n + if e.going { 1 } else { -1 }).max(0));
        Some(e.going)
    };
    let Some(going) = flip(r) else { return };
    cx.notify();
    let rx = roblox::run(move || roblox::set_going(&cookie, &ask, going));
    cx.spawn(async move |this, cx| {
        let got = rx.await.unwrap_or_else(|_| Err(String::new()));
        if let Err(why) = got {
            let _ = this.update(cx, |r, cx| {
                crate::net::note_failed("Game events: Notify me wasn't taken.", "https://apis.roblox.com/virtual-events/v1/virtual-events/#/rsvps", &why);
                let _ = flip(r);
                r.toast = Some((r.tr("gp.evNotTaken"), "bad".into()));
                cx.notify();
            });
        }
    })
    .detach();
}

/// An event's page on roblox.com, from its picture or its Event page button.
fn event_page(id: &str, cx: &mut Context<Root>) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    let id = id.to_string();
    cx.listener(move |r, _, _, _| r.send("openRobloxPage", json!({ "kind": "event", "id": id })))
}

/// Join from an event that is on: the game's Play row opens at the top of the page, the accounts picked on the
/// Accounts page ticked.
fn join_event(r: &mut Root, cx: &mut Context<Root>) {
    let Some(open) = r.games.open.as_ref().map(|o| o.id.clone()) else { return };
    r.games.picking = Some((open, preselected_join_accounts(r)));
    r.games.pick_job = None;
    r.games.pick_link = None;
    r.games.pick_place = None;
    r.games.page_scroll.set_offset(point(px(0.), px(0.)));
    cx.notify();
}

/// A game's events with everything Roblox's game page shows for each (its picture, kind, name, line, the going
/// count, Notify me or Join), and more: the one on now as a large block with its words and the time left, the rest as
/// picture tiles with their start and how long until it. None when nothing is on or to come.
fn events_block(root: &Root, place: i64, width: f32, cx: &mut Context<Root>) -> Option<AnyElement> {
    let list = root.games.events.get(&place).filter(|l| !l.is_empty())?;
    let now = crate::time::now_secs();
    let signed = crate::pages::friends::signed(root).is_some_and(|s| !s.2.is_empty());
    // (No box round it now: its tiles take the column's width, 2 px short so a row of two never wraps.)
    let inner = (width - 2.).max(200.);
    // (A kind Roblox adds later, with no word here yet: its own name in words, "specialEvent" as "Special event", not
    // the key.)
    let kind_word = |k: &str| {
        let key = format!("gp.evKind.{k}");
        if k.is_empty() {
            String::new()
        } else if root.i18n.has(&key) {
            root.tr(&key)
        } else {
            let mut out = String::new();
            for (i, ch) in k.chars().enumerate() {
                match (i, ch.is_uppercase()) {
                    (0, _) => out.extend(ch.to_uppercase()),
                    (_, true) => {
                        out.push(' ');
                        out.extend(ch.to_lowercase());
                    }
                    _ => out.push(ch),
                }
            }
            out
        }
    };
    let going_word = |n: Option<i64>| n.map(|n| root.i18n.tv("gp.evPeople", &[("n", &thousands(n))]));
    let fact = |name: &'static str, text: String| div().flex().flex_none().items_center().gap(px(6.)).child(icon(name, 14., c(TEXT_2))).child(text);
    let picture = |e: &roblox::GameEvent, w: f32, h: f32, idx: usize, dated: bool, cx: &mut Context<Root>| {
        let pic = (!e.pic.is_empty()).then(|| images::pic_pair(&e.pic, w, h, cx)).flatten();
        let on = e.start <= now;
        let kind = kind_word(&e.kind);
        let when = if on { root.tr("gp.evNow") } else { crate::time::day_time(e.start) };
        div()
            .id(("gp-ev-pic", idx))
            .relative()
            .w(px(w))
            .h(px(h))
            .flex_none()
            .rounded(px(R_L))
            .overflow_hidden()
            .bg(ca(WHITE, 0.045))
            .cursor_pointer()
            .on_click(event_page(&e.id, cx))
            .when_some(pic, |d, p| d.child(fade_in(img(p.0).size_full().rounded(px(R_L)).object_fit(ObjectFit::Cover), &p.1)))
            .when(!kind.is_empty(), |d| d.child(div().absolute().left(px(10.)).top(px(10.)).child(photo_chip(kind, None))))
            .when(dated, |d| d.child(div().absolute().left(px(10.)).bottom(px(10.)).child(photo_chip(when, Some(if on { ONLINE } else { FACE })))))
    };
    let notify = |e: &roblox::GameEvent, idx: usize, big: bool, cx: &mut Context<Root>| {
        let (id, going) = (e.id.clone(), e.going);
        let b = Button::new(("gp-ev-rsvp", idx), root.tr(if going { "gp.evGoing" } else { "gp.evNotify" })).icon(if going { "check" } else { "bell" });
        let b = if going { b.tint() } else { b.quiet() };
        let b = if big { b.big() } else { b.small() };
        b.disabled(!signed).on_click(cx.listener(move |r, _, _, cx| toggle_going(r, place, id.clone(), cx)))
    };

    // Every event in one look, as Roblox's own page has them (the owner, 2026-11-19, "Kiểu A"): two on at once had
    // been one big and one small tile alone in half a row ("bạn ko thấy có vấn đề à?"). Only a game with one event
    // gives it the whole width, its picture beside its words.
    let shown: Vec<(usize, &roblox::GameEvent)> = list.iter().take(9).enumerate().collect();
    // "On now · ends in 10d 20h" / "Coming · in 2d": the state in the accent, the time after it (no dot: the owner's
    // rule against plain status dots).
    let status = |e: &roblox::GameEvent, size: f32| {
        let on = e.start <= now;
        let (state, when) = if on {
            (root.tr("gp.evNow"), root.i18n.tv("gp.evEndsIn", &[("t", &left(e.end - now))]))
        } else {
            (root.tr("gp.evSoon"), root.i18n.tv("gp.evStarts", &[("t", &left(e.start - now))]))
        };
        div()
            .flex()
            .min_w_0()
            .items_center()
            .gap(px(6.))
            .text_size(px(size))
            .font_features(tnum())
            .child(div().flex_none().font_weight(FontWeight::SEMIBOLD).text_color(c(FACE)).child(state))
            .child(div().min_w_0().truncate().text_color(c(TEXT_2)).child(format!("· {when}")))
    };
    let act = |e: &roblox::GameEvent, idx: usize, big: bool, cx: &mut Context<Root>| -> Button {
        if e.start <= now {
            let b = Button::new(("gp-ev-join", idx), root.tr("gp.evJoin")).icon("play").primary().on_click(cx.listener(|r, _, _, cx| join_event(r, cx)));
            if big { b.big() } else { b }
        } else {
            notify(e, idx, big, cx)
        }
    };
    let mut body = div().flex().flex_col().gap(px(18.));

    if let [(idx, e)] = shown[..] {
        let wide = inner >= 760.;
        let pw = if wide { (inner * 0.52).floor() } else { inner };
        let ph = (pw * 9. / 16.).round();
        let about = Some(e.about.clone()).filter(|a| !a.is_empty() && *a != e.sub);
        let words = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(status(e, T_M))
            .child(div().text_size(px(T_FIG)).line_height(px(28.)).font_weight(FontWeight::BOLD).line_clamp(2).child(e.title.clone()))
            .when(!e.sub.is_empty(), |d| d.child(div().text_size(px(T_L)).line_height(px(20.)).text_color(c(TEXT_2)).line_clamp(2).child(e.sub.clone())))
            .when_some(about, |d, a| d.child(div().text_size(px(T_M)).line_height(px(20.)).text_color(c(MUTED)).line_clamp(4).child(a)))
            .when_some(going_word(e.count), |d, w| d.child(div().text_size(px(T_M)).text_color(c(TEXT_2)).font_features(tnum()).child(fact("users", w))))
            .child(
                div()
                    .pt(px(4.))
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(8.))
                    .child(act(e, idx, true, cx))
                    .child(Button::new(("gp-ev-page", idx), root.tr("gp.evPage")).icon_end("external").quiet().big().on_click(event_page(&e.id, cx))),
            );
        body = body.child(div().flex().when(wide, |d| d.flex_row().items_start()).when(!wide, |d| d.flex_col()).gap(px(20.)).child(picture(e, pw, ph, idx, false, cx)).child(words));
    } else {
        // Equal tiles sharing the row: as many across as there are (up to 3 on a wide page, 2 on a middling one), and
        // 4 as two rows of two, so no row is left with one tile and a hole.
        let n = shown.len();
        let most = if inner >= 900. { 3 } else if inner >= 520. { 2 } else { 1 };
        let per = match n.min(most) {
            3 if n % 3 != 0 && n % 2 == 0 => 2,
            p => p.max(1),
        };
        let gap = GAP * 1.5;
        let tw = ((inner - gap * (per as f32 - 1.)) / per as f32).floor();
        let th = (tw * 9. / 16.).round();
        let mut grid = div().flex().flex_wrap().gap_x(px(gap)).gap_y(px(24.));
        for (idx, e) in shown {
            let sub = Some(e.sub.clone()).filter(|t| !t.is_empty()).or_else(|| Some(e.about.clone()).filter(|t| !t.is_empty()));
            grid = grid.child(
                div()
                    .w(px(tw))
                    .flex()
                    .flex_col()
                    .child(picture(e, tw, th, idx, false, cx))
                    .child(div().mt(px(12.)).child(status(e, T_S)))
                    .child(div().mt(px(4.)).text_size(px(T_H)).line_height(px(22.)).font_weight(FontWeight::SEMIBOLD).truncate().child(e.title.clone()))
                    .when_some(sub, |d, t| d.child(div().text_size(px(T_S)).line_height(px(18.)).text_color(c(MUTED)).truncate().child(t)))
                    .child(
                        div()
                            .mt(px(12.))
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .child(act(e, idx, false, cx).wide())
                            .when_some(e.count, |d, n| d.child(div().flex_none().text_size(px(T_S)).text_color(c(TEXT_2)).font_features(tnum()).child(fact("users", thousands(n))))),
                    ),
            );
        }
        body = body.child(grid);
    }

    let sub = root.i18n.tv("gp.evCount", &[("n", &list.len().min(9).to_string())]);
    let head = if rich() { heading_rich("calendar", FACE, root.tr("gp.events"), Some(sub)).into_any_element() } else { heading(root.tr("gp.events"), Some(sub)).into_any_element() };
    Some(div().flex().flex_col().gap(px(16.)).child(head).child(body).into_any_element())
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
                // (Shots: the row moved to picture n, BLOXKEEP_SHOT_PIC.)
                #[cfg(feature = "shot")]
                if let Some(n) = std::env::var("BLOXKEEP_SHOT_PIC").ok().and_then(|v| v.parse::<usize>().ok()) {
                    state.update(cx, |s, cx| s.set_selected_index(n, cx));
                }
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
        // The last picture ends at the row's edge: with a gap after it too, the row had one more 24 px stop past the
        // last full view, and Next stayed lit there (owner: "cái nút tiến chỉ ảnh cuối mới bị").
        let last = i + 1 == pics.len();
        CarouselItem::new(("gp-shot", i), i, state).w(px(if last { w } else { w + GAP })).pl(px(0.)).pr(px(if last { 0. } else { GAP })).child(
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
            // An arrow shows only while it can move the row (none over the first or the last picture).
            .when(arrows && back, |d| {
                d.child(div().absolute().top(px(h / 2. - 17.)).left(px(10.)).child(IconButton::new("gp-shot-back", "chevronLeft").size(34.).glass().on_click(step(false))))
            })
            .when(arrows && on, |d| {
                d.child(div().absolute().top(px(h / 2. - 17.)).right(px(10.)).child(IconButton::new("gp-shot-on", "chevronRight").size(34.).glass().on_click(step(true))))
            })
            .into_any_element(),
    )
}

/// A game's accounts over 7 days, summed here from the raw stays once both they and the game's details (which name
/// its universe) are in: history::game_accounts, rows shaped as the app's gameDetails gave them. An account still in
/// the app goes by its name there; one deleted, by the name its last stay kept.
pub fn set_play_history(r: &mut Root, raw: Value) {
    r.games.history = Some(raw);
    r.games.stays.clear();
    if let Some(place) = shown_place(r) {
        settle_stays(r, place);
    }
}

fn settle_stays(r: &mut Root, place: i64) {
    let Some(raw) = &r.games.history else { return };
    let (sessions, known, now) = crate::history::read(raw);
    let universe = r.games.details.get(&place).map(|d| int(d, "universe")).filter(|u| *u > 0)
        .unwrap_or_else(|| crate::history::universe_of(&sessions, &known, place));
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
        self.community = None;
        // (A reply still on its way is for the sign-in before: it is dropped.)
        self.sign_gen += 1;
        self.social = None;
        self.history = None;
        self.stays.clear();
        self.home_games = None;
        self.home_asked = None;
        self.recommended_all = false;
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
/// The active Roblox account's Continue and Recommended for you rows share one request.
fn want_home_games(r: &mut Root, cx: &mut Context<Root>) {
    let Some((id, user, cookie)) = crate::pages::friends::signed(r) else { return };
    if r.games.open.is_some() || r.games.view != View::Games || cookie.is_empty()
        || r.games.home_asked.is_some_and(|(u, t)| u == user && t.elapsed().as_secs() < 90)
    {
        return;
    }
    let asked_at = std::time::Instant::now();
    let sign_gen = r.games.sign_gen;
    r.games.home_asked = Some((user, asked_at));
    let rx = roblox::run(move || roblox::home_games(&cookie));
    reply(rx, cx, move |r, got, cx| {
        if r.games.sign_gen != sign_gen || r.games.home_asked != Some((user, asked_at))
            || crate::pages::friends::signed(r).is_none_or(|(active, _, _)| active != id)
        {
            return;
        }
        if let Ok(home) = got {
            let places = home.continued.iter().map(|g| g.place).collect();
            r.games.home_games = Some((user, home));
            r.want_looks(places, cx);
        }
        cx.notify();
    });
}

fn active_home_games(root: &Root) -> Option<&roblox::HomeGames> {
    let (_, user, _) = crate::pages::friends::signed(root)?;
    root.games.home_games.as_ref().filter(|(id, _)| *id == user).map(|(_, home)| home)
}

fn want_roblox(r: &mut Root, link: &crate::link::Link, cx: &mut Context<Root>) {
    want_home_games(r, cx);
    let sign = r.games.sign_gen;
    if !r.games.asked_cookie {
        r.games.asked_cookie = true;
        let rx = link.call("robloxSignIn", json!({}));
        found(rx, cx, move |r, got, cx| {
            if r.games.sign_gen != sign {
                return;
            }
            r.games.cookie = Some(got.and_then(|v| v.get("cookie").and_then(Value::as_str).map(str::to_string)));
            cx.notify();
        });
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
                    // (BLOXKEEP_SHOT_SORT=up-and-coming: a hidden shot with that chart picked in the filter.)
                    #[cfg(feature = "shot")]
                    if let Ok(id) = std::env::var("BLOXKEEP_SHOT_SORT") {
                        r.games.sort_at = Some(id);
                    }
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
    // Every chart is a row of its own now (the owner's pick, 2026-11-19: rows that step sideways, as Roblox's
    // charts page has them), so each is asked; the first dozen.
    let every: Vec<roblox::Sort> = match &r.games.sorts {
        Some(Ok(sorts)) => sorts.iter().enumerate().filter(|(k, s)| *k < CHART_ROWS || r.games.sort_at.as_deref() == Some(s.id.as_str())).map(|(_, s)| s.clone()).collect(),
        _ => Vec::new(),
    };
    for sort in every {
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
    if let Some(home) = active_home_games(r) {
        want.extend(home.recommended.iter().map(|g| g.universe));
    }
    // A row's pictures as far as its next page (24 at least): the rest come as it is stepped through.
    // (Icons only: a tile drawn wide asks for its own wide picture as it is drawn, art and ask_covers.)
    if let Some(Ok(sorts)) = &r.games.sorts {
        for sort in sorts.iter().take(CHART_ROWS) {
            if let Some(Ok(list)) = r.games.rows.get(&sort.id) {
                let at = r.games.row_at.get(&sort.id).map_or(0, |a| a.0);
                let upto = (at + 24).min(list.len());
                want.extend(list[..upto].iter().map(|g| g.universe));
            }
        }
        if let Some(Ok(list)) = r.games.sort_at.as_ref().and_then(|id| r.games.rows.get(id)) {
            want.extend(list.iter().map(|g| g.universe));
        }
    }
    if let Some((_, Some(Ok(list)))) = &r.games.found {
        want.extend(list.iter().map(|g| g.universe));
    }
    // (The first games of the chart shown, under a game's page.)
    if let Some(Ok(list)) = shown_sort(r).and_then(|s| r.games.rows.get(&s.id)) {
        want.extend(list.iter().take(12).map(|g| g.universe));
    }
    if let Some((_, Some(Ok(list)))) = &r.games.switch_found {
        want.extend(list.iter().take(8).map(|g| g.universe));
    }
    want.retain(|u| *u > 0 && !r.games.icons.contains_key(u) && !r.games.asked_icons.contains(u));
    want.sort_unstable();
    want.dedup();
    if !want.is_empty() {
        r.games.asked_icons.extend(want.iter().copied());
        let asked = want.clone();
        let rx = roblox::run(move || roblox::icons(&want));
        cx.spawn(async move |this, cx| {
            let icons = rx.await.unwrap_or_default();
            // (Roblox answers every game, a placeholder for one without an icon: one left out wasn't answered.)
            let missed: Vec<i64> = asked.into_iter().filter(|u| !icons.contains_key(u)).collect();
            let _ = this.update(cx, |r, cx| {
                // (Icons are the same whoever is signed in: one asked before a vault switch is still right.)
                r.games.icons.extend(icons);
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

/// Roblox's search as the box is typed in: its words held a moment (a third of a second without a key) are asked,
/// as Roblox's own search box does, not only on Enter (the owner, 2026-11-15: "nhấn enter mới ra các game"). Until
/// the answer comes, the page says it is looking, not "Nothing matches".
pub fn search_soon(r: &mut Root, cx: &mut Context<Root>) {
    let q = r.game_search.read(cx).value().trim().to_string();
    r.games.typed += 1;
    if q.is_empty() {
        r.games.found = None;
        cx.notify();
        return;
    }
    // (Shown as being looked for at once, the last answer's games kept until the new one comes.)
    if r.games.found.as_ref().is_none_or(|(at, _)| *at != q) {
        let kept = r.games.found.take().and_then(|(_, got)| got).filter(|g| g.as_ref().is_ok_and(|g| !g.is_empty()));
        r.games.found = Some((q.clone(), kept));
    }
    cx.notify();
    let seq = r.games.typed;
    cx.spawn(async move |this, cx| {
        cx.background_executor().timer(std::time::Duration::from_millis(330)).await;
        let _ = this.update(cx, |r, cx| {
            if r.games.typed == seq {
                search(r, cx);
            }
        });
    })
    .detach();
}

/// Roblox's search for what Games' box holds (Enter, or a moment after the last key).
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
    r.games.home_library = None;
    r.games.found = Some((q.clone(), None));
    r.games.clubs_found = Some((q.clone(), None));
    cx.notify();
    let (ask, q2) = (q.clone(), q.clone());
    let rx = roblox::run(move || roblox::club_search(&ask));
    reply(rx, cx, move |r, got, cx| {
        if r.games.clubs_found.as_ref().is_some_and(|(at, _)| *at == q2) {
            r.games.clubs_found = Some((q2, Some(got)));
            cx.notify();
        }
    });
    let ask = q.clone();
    let rx = roblox::run(move || roblox::search(&ask));
    reply(rx, cx, move |r, got, cx| {
        if r.games.found.as_ref().is_some_and(|(at, _)| *at == q) {
            r.games.found = Some((q, Some(got)));
            cx.notify();
        }
    });
}

// ───────────────────────── the corner ─────────────────────────

/// What Roblox's own top bar has, for the account looked as: its Robux, a bell with its unread notifications and
/// messages, and whether Roblox holds it (banned, warned), read once a minute at most while Roblox is shown.
#[derive(Default)]
pub struct Corner {
    of: i64,
    robux: Option<i64>,
    pub(crate) unread: (i64, i64),
    held: Option<roblox::Held>,
    asked: Option<std::time::Instant>,
    busy: bool,
    /// "I agree" on its way, and why Roblox turned the last one down.
    waking: bool,
    refused: Option<String>,
    /// What else Roblox's own pages would stop the account with (notice_bars): agreements waiting for its "I Agree",
    /// a reminder of the rules, an age check Roblox wants by a day. Read every ten minutes at most.
    deals: Vec<roblox::Agreement>,
    remind: Option<roblox::Reminder>,
    age: Option<roblox::Age>,
    noticed: Option<std::time::Instant>,
    agreeing: bool,
}

/// Reminders read with "I understand" (Roblox asks nothing more: its page only counts the click), not shown again.
static REMINDED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

pub fn want_corner(r: &mut Root, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = crate::pages::friends::signed(r) else { return };
    let k = &mut r.games.corner;
    if k.of != user {
        *k = Corner { of: user, ..Default::default() };
    }
    // Roblox's notices, every ten minutes (they change seldom; the corner's figures every minute).
    if !k.noticed.is_some_and(|t| t.elapsed().as_secs() < 600) {
        k.noticed = Some(std::time::Instant::now());
        let c2 = cookie.clone();
        let rx = roblox::run(move || (roblox::agreements(&c2), roblox::reminder(&c2), roblox::age(&c2)));
        cx.spawn(async move |this, cx| {
            let Ok((deals, remind, age)) = rx.await else { return };
            let _ = this.update(cx, |r, cx| {
                let k = &mut r.games.corner;
                if k.of != user {
                    return;
                }
                // (A read that failed keeps what was known. The agreements only with the age read: Roblox answers a
                // sign-in it doesn't take as nobody's, with every agreement there is, which would ask for an
                // "I Agree" that isn't owed; the age needs the sign-in.)
                if let (Ok(d), true) = (deals, age.is_ok()) {
                    k.deals = d;
                }
                if let Ok(m) = remind {
                    k.remind = m;
                }
                if let Ok(a) = age {
                    k.age = Some(a);
                }
                cx.notify();
            });
        })
        .detach();
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
        k.held = w.get(3).map(|_| roblox::Held { kind: "Ban 3 Days".into(), ends: Some(now + 3 * 86400 - 3600), note: "Bullying or harassment".into(), back: false });
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

/// The bell's notifications seen at Roblox (the inbox's Notifications shown) for `user`: counted no more.
pub fn notes_seen(r: &mut Root, user: i64) {
    if r.games.corner.of == user {
        r.games.corner.unread.0 = 0;
    }
}

/// A message read in the inbox: one fewer on the bell at once.
pub fn letter_read(r: &mut Root) {
    let k = &mut r.games.corner;
    k.unread.1 = (k.unread.1 - 1).max(0);
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
        .child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).font_features(tnum()).child(k.robux.map(|n| thousands(n)).unwrap_or_else(|| "–".into())))
        .on_click(cx.listener(|r, _, _, cx| {
            to_inbox(r, 3);
            cx.notify();
        }));
    // The bell counts the notifications only, as Roblox's own does: the messages have their count on the inbox.
    let n = k.unread.0;
    let bell = div()
        .relative()
        .flex_none()
        .child(IconButton::new("rbx-bell", "bell").size(34.).tip(root.tr("rb.bellTip")).on_click(cx.listener(move |r, _, _, cx| {
            to_inbox(r, 2);
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
    crate::pages::friends::signed(root)?;
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

/// What Roblox's own pages would stop the account with, as bars over the page in held_bar's look, each with what
/// Roblox's own window offers: its updated agreements and "I Agree" (site-status' UserAgreementsChecker), its reminder
/// of the rules and "I understand" (reminderOfNorms), the age check it wants by a day (account info's AgeGroup).
fn notice_bars(root: &Root, cx: &mut Context<Root>) -> Vec<AnyElement> {
    let mut bars = Vec::new();
    if crate::pages::friends::signed(root).is_none() {
        return bars;
    }
    let k = &root.games.corner;
    let (_, viewer) = crate::pages::friends::looking_ref(root);
    let id = viewer.and_then(|a| a.get("id")).and_then(Value::as_str).unwrap_or_default().to_string();
    let bar = |key: &'static str, mark: &'static str, tone: u32, head: String, sub: AnyElement| {
        div()
            .id(key)
            .flex_none()
            .px(px(16.))
            .py(px(12.))
            .rounded(px(R_PANEL))
            .bg(c(PANEL))
            .border_1()
            .border_color(ca(tone, 0.55))
            .flex()
            .items_center()
            .gap(px(14.))
            .child(icon(mark, 20., c(tone)).flex_none())
            .child(div().flex_1().min_w_0().flex().flex_col().gap(px(2.)).child(div().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).child(head)).child(sub))
    };
    let muted = |t: String| div().text_size(px(T_M)).line_height(px(18.)).text_color(c(MUTED)).child(t).into_any_element();
    if !k.deals.is_empty() {
        // Roblox's words over the list when it sends them (English only: in another language, ours), then each
        // agreement to read, opened in the browser as Roblox's links open.
        let said = k.deals.iter().find_map(|d| d.text.clone()).filter(|_| root.i18n.lang == "en").unwrap_or_else(|| root.tr("rn.agreeBody"));
        let mut links = div().flex().flex_wrap().gap_x(px(14.)).gap_y(px(2.)).text_size(px(T_M));
        for (i, d) in k.deals.iter().enumerate() {
            let key = format!("rn.doc.{}", d.kind);
            let name = if root.i18n.has(&key) { root.tr(&key) } else { d.kind.clone() };
            let url = d.url.clone();
            links = links.child(
                div()
                    .id(("rn-doc", i))
                    .cursor_pointer()
                    .text_color(c(FACE))
                    .hover(|d| d.underline())
                    .child(name)
                    .when(url.starts_with("https://"), |d| d.on_click(move |_, _, cx| cx.open_url(&url))),
            );
        }
        let sub = div().flex().flex_col().gap(px(4.)).child(muted(said)).child(links).into_any_element();
        bars.push(
            bar("rn-agree-bar", "book", WARN, root.tr("rn.agreeTitle"), sub)
                .child(Button::new("rn-agree", root.tr("rn.agreeBtn")).icon("check").primary().small().disabled(k.agreeing).on_click(cx.listener(|r, _, _, cx| accept_deals(r, cx))))
                .into_any_element(),
        );
    }
    if let Some(m) = k.remind.as_ref().filter(|m| !REMINDED.lock().unwrap_or_else(|e| e.into_inner()).contains(&m.id)) {
        // The reminder of the rules, or the chat-filter lesson (Roblox's title and words for its kind, its OK).
        let (mark, head, sub, ok) = match &m.lesson {
            Some(l) => {
                let tip = root.tr(&format!("rn.tip.{}", l.tip));
                let (title, body) = if l.changed { ("rn.lessonChanged", "rn.lessonBodyChanged") } else { ("rn.lessonClose", "rn.lessonBodyClose") };
                ("message", root.tr(title), root.i18n.tv(body, &[("tip", &tip)]), root.tr("rn.ok"))
            }
            None => {
                let rule = root.tr(&format!("rn.rule.{}", rule_of(&m.rule)));
                ("shield", root.tr("rn.remindTitle"), root.i18n.tv("rn.remindBody", &[("rule", &rule)]), root.tr("rn.remindOk"))
            }
        };
        let seen = m.id.clone();
        bars.push(
            bar("rn-remind-bar", mark, WARN, head, muted(sub))
                .child(Button::new("rn-remind-ok", ok).small().on_click(cx.listener(move |_, _, _, cx| {
                    REMINDED.lock().unwrap_or_else(|e| e.into_inner()).push(seen.clone());
                    cx.notify();
                })))
                .into_any_element(),
        );
    }
    if let Some(a) = k.age.as_ref().filter(|a| a.deadline.is_some() || a.soon) {
        let head = match a.deadline {
            Some(t) => root.i18n.tv("rn.ageBy", &[("t", &crate::time::day_time(t))]),
            None => root.tr("rn.ageSoon"),
        };
        bars.push(
            bar("rn-age-bar", "user", WARN, head, muted(root.tr("rn.ageBody")))
                .child(Button::new("rn-age-open", root.tr("rs.verify")).icon("external").small().on_click(cx.listener(move |r, _, _, _| r.send("openWebsite", json!({ "id": id, "page": "settings" })))))
                .into_any_element(),
        );
    }
    bars
}

/// Roblox's key for a broken rule ("Experiment.Reminders.ViolationScams") as the name of ours (rn.rule.*): Roblox
/// names several the same, and one it may add later is "other".
fn rule_of(key: &str) -> &'static str {
    let v = key.rsplit("Violation").next().unwrap_or_default().trim_end_matches(".V2");
    match v {
        "SexualContent" => "sexual",
        "IrlDangerousActivities" | "EncouragingDangerousBehavior" => "harm",
        "AskingForPII" | "SharingYourOwnPii" => "pii",
        "BullyingAndHarassment" | "MisleadingImpersonationMisrepresentation" | "ExtortionAndBlackmail" | "ThreatsOrAbuseOfRobloxEmployeesOrAffiliates" => "bullying",
        "DiscriminationAndHate" => "hate",
        "ViolentContentAndGore" => "violence",
        "IllegalAndRegulatedGoodsAndActivities" | "ContestsAndSweepstakes" => "illegal",
        "RomanticContent" => "romance",
        "Profanity" => "profanity",
        "PoliticalContent" => "politics",
        "Scams" => "scams",
        "Spam" => "spam",
        "DirectingUsersOffPlatform" => "offPlatform",
        "DisruptiveAudio" | "MisusingRobloxSystems" => "misuse",
        "CheatingAndExploiting" => "cheating",
        "Dmca" | "IpViolation" | "Trademark" => "ip",
        "RobloxEconomy" => "economy",
        "Advertising" => "ads",
        "RealWorldTragedy" => "tragedy",
        "OffPlatformSpeechAndBehavior" => "offPlatformSpeech",
        "TerrorismExtremism" => "extremism",
        "VirtualCasino" => "casino",
        _ => "other",
    }
}

/// "I Agree" to every agreement waiting, as Roblox's window sends it; read again after.
fn accept_deals(r: &mut Root, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = crate::pages::friends::signed(r) else { return };
    let k = &mut r.games.corner;
    if k.agreeing || k.deals.is_empty() {
        return;
    }
    k.agreeing = true;
    let ids: Vec<String> = k.deals.iter().map(|d| d.id.clone()).collect();
    let rx = roblox::run(move || roblox::accept_agreements(&cookie, &ids));
    reply(rx, cx, move |r, done, cx| {
        if r.games.corner.of != user {
            return;
        }
        r.games.corner.agreeing = false;
        match done {
            Ok(()) => {
                r.games.corner.deals.clear();
                r.games.corner.noticed = None;
                r.toast = Some((r.tr("rn.agreed"), "ok".into()));
            }
            Err(why) => r.toast = Some((r.i18n.tv("rs.failed", &[("why", if why.is_empty() { "Roblox" } else { &why })]), "bad".into())),
        }
        cx.notify();
    });
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
    reply(rx, cx, move |r, done, cx| {
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
}

// ───────────────────────── the cards ─────────────────────────

pub fn games(root: &Root, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let all = list(root);
    let shell = div().id("games").size_full().rounded(px(R_PANEL)).bg(c(PANEL)).overflow_hidden().flex().flex_col();
    let content = match root.games.open.clone() {
        Some(o) => {
            // (On the panel's own ground: 6 % of the game's colour under it all turned the page brown, see kit::block_tones.)
            shell.child(page(root, &all, o, width, cx)).into_any_element()
        }
        None if crate::pages::tabs::picking(root) => shell.child(crate::pages::tabs::picker(root, cx)).into_any_element(),
        None if root.games.view == View::Friends => crate::pages::friends::friends(root, width, cx).into_any_element(),
        None if root.games.view == View::Avatar => crate::pages::avatar::avatar(root, width, cx).into_any_element(),
        None if root.games.view == View::Profile => crate::pages::profile::profile(root, width, cx).into_any_element(),
        None if root.games.view == View::Shop => crate::pages::profile::shop(root, width, cx).into_any_element(),
        None if root.games.view == View::Inbox => crate::pages::inbox::inbox(root, width, cx).into_any_element(),
        None if root.games.view == View::Settings => crate::pages::rsettings::page(root, width, cx).into_any_element(),
        None if root.games.view == View::Mods => crate::pages::mods::page(root, width, cx).into_any_element(),
        None if root.games.view == View::Servers => crate::pages::owned::page(root, width, cx).into_any_element(),
        None => match root.games.home_library {
            Some(tab) => shell.child(super::home::library(root, tab, width, cx)).into_any_element(),
            None => shell.child(grid(root, &all, width, cx)).into_any_element(),
        },
    };
    // (The page is built: what is drawn after it, a dialog or another window, is no game's.)
    set_game_tint(None);
    // The account tabs over it all (pages/tabs.rs): what is under them is the tab shown.
    // (Robux and the bell at the strip's right, as Roblox's top bar; over the page, Roblox holding the account.)
    let top = div().flex_none().flex().items_center().gap(px(8.)).child(crate::pages::tabs::strip(root, width, cx)).child(div().flex_1()).children(corner(root, cx));
    div().size_full().flex().flex_col().gap(px(8.)).child(top)
        .when(crate::pages::tabs::is_vng(root) && (root.games.view == View::Games || root.games.open.is_some()), |d| d.child(div().flex_none().px(px(12.)).py(px(8.)).rounded(px(R_M)).bg(ca(ONLINE, 0.08)).text_size(px(T_S)).text_color(c(TEXT_2)).child(root.tr("tabs.vngBrowse"))))
        .children(held_bar(root, cx)).children(notice_bars(root, cx)).child(div().flex_1().min_h_0().child(content))
}

fn grid(root: &Root, all: &[Game], width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let inner = width - 2. * (PAGE_X);
    // (Without accents or case, as the web page's search: "hoang" finds "Hoàng".)
    let query = folded(root.game_search.read(cx).value().trim());
    // (Ranked as the switch's box is: initials, a letter wrong, words run together; best first.)
    #[allow(unused_mut)]
    let mut shown: Vec<&Game> = ranked(all.iter(), &query, |g| g.name.as_str());
    // (BLOXKEEP_SHOT_MANY=N: a hidden shot's games made N, to time the page with many.)
    #[cfg(feature = "shot")]
    crate::near::shot_many(&mut shown);
    let card_w = home_fit(inner).1 + 2. * CPAD;

    let header = div()
        .flex_none()
        .px(px(PAGE_X))
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
                // (In a window that isn't small the capsule stands in the header's middle: the box ends before it.)
                .w(px(if root.narrow.get() { 220. } else { beside_island(inner).clamp(150., 280.) }))
                .min_w(px(150.))
                .flex_shrink(1.)
                .child(field(&root.game_search, Some("search"))),
        );

    let mut body = div().flex().flex_col().gap(px(28.)).px(px(PAGE_X)).pt(px(4.)).pb(px(24.));
    // A search sent to Roblox (Enter): its games first, the bot's that match after.
    if let Some((q, got)) = &root.games.found {
        body = body.child(roblox_section(root, root.i18n.tv("games.results", &[("q", q)]), None, None, got.as_ref(), inner, cx));
        // Its communities (only when there are some: a search for a game rarely wants them, so no empty heading).
        if let Some((_, Some(Ok(hits)))) = root.games.clubs_found.as_ref().filter(|(at, _)| at == q).filter(|f| f.1.as_ref().is_some_and(|g| g.as_ref().is_ok_and(|l| !l.is_empty()))) {
            body = body.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(section_head(root.tr("gp.communityTitle"), Some(hits.len())))
                    .child(crate::pages::profile::club_hits(root, hits, inner, cx)),
            );
        }
        if !shown.is_empty() {
            body = body.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(div().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT_2)).child(root.tr("games.withWiki")))
                    .child(game_cards(root, "games-found", &shown, card_w, cx)),
            );
        }
        return div().size_full().flex().flex_col().child(header).child(crate::scrolled::body("games-body", body));
    }
    // As Roblox's home has it: the friends' faces, the games played lately, Roblox's charts; then the bot's games
    // (yours: followed, read or played; then the others).
    let (yours, others): (Vec<&Game>, Vec<&Game>) = shown.iter().copied().partition(|g| mine(g));
    if query.is_empty() {
        body = body.children(radar_all(root)).children(friends_strip(root, inner, cx)).children(keep_playing(root, inner, cx));
        body = body.children(recommended(root, inner, cx)).child(charts(root, inner, cx));
        if !yours.is_empty() {
            body = body.child(section(root, root.tr("games.mine"), "games-mine", &yours, card_w, cx));
        }
        if !others.is_empty() {
            body = body.child(section(root, root.tr("games.withWiki"), "games-wiki", &others, card_w, cx));
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
        body = body.child(div().w_full().py(px(60.)).flex().justify_center().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("wiki.nothing")));
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
        for (k, (title, games)) in parts.into_iter().enumerate() {
            body = body.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .when_some(title, |d, t| d.child(div().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT_2)).child(t)))
                    .child(game_cards(root, ["games-part-0", "games-part-1"][k.min(1)], &games, card_w, cx)),
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
/// Keep playing shows open and recently played local games before supplementing them with Roblox's Continue list.
fn keep_list(root: &Root) -> Vec<crate::pages::home::Game> {
    use crate::pages::home::{clean, Game as HomeGame};
    let here = crate::pages::home::games(root);
    let hours = |place: i64| -> (f64, f64) {
        root.played.iter().flatten().find(|p| p.get("place").and_then(Value::as_i64) == Some(place)).map_or((0., 0.), |p| (p.get("hours").and_then(Value::as_f64).unwrap_or(0.), p.get("last").and_then(Value::as_f64).unwrap_or(0.)))
    };
    let Some(home) = active_home_games(root) else { return here };
    let (mut out, rest): (Vec<HomeGame>, Vec<HomeGame>) = here.into_iter().partition(|g| !g.accounts.is_empty() || g.last > 0.);
    for g in &home.continued {
        if !out.iter().any(|x| x.place == g.place) {
            let (h, last) = hours(g.place);
            out.push(HomeGame { place: g.place, name: clean(&g.name), icon: None, accounts: Vec::new(), hours: h, last });
        }
    }
    for g in rest {
        if !out.iter().any(|x| x.place == g.place) {
            out.push(g);
        }
    }
    out
}

fn keep_playing(root: &Root, width: f32, cx: &mut Context<Root>) -> Option<AnyElement> {
    let all = keep_list(root);
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
        let icon_url = crate::pages::home::look(root, g.place, "icon").map(str::to_string).or(g.icon.clone());
        cascade(
            "gk-in",
            i,
            crate::pages::home::cover_tile(("gk-tile", i), crate::pages::home::look(root, g.place, "cover").map(str::to_string), icon_url, g.name.clone(), played(root, g), g.accounts.len(), false, w, false, crate::pages::clips::tile_clip(root, place, cx), cx).on_click(
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
    } else if g.hours > 0. {
        crate::pages::home::played_time(root, g.hours)
    } else {
        root.tr("home.recentShort")
    };
    (sub, running)
}

/// The first game big, its gameplay clip playing on its picture while the mouse rests there, as on every other tile
/// (muted, pages/clips.rs; it played on its own at first, the owner 2026-11-18: only under the mouse), its name large
/// on a plate at its foot; the next three stacked at its right, each a picture and its name.
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
    let clip = crate::pages::clips::tile_clip(root, place, cx);
    let (sub, running) = played(root, g);
    let cover = crate::pages::home::look(root, place, "cover").map(str::to_string);
    let picture = cover.as_deref().and_then(|u| crate::images::pic_pair(u, big_w, art_h, cx));
    let badge = crate::pages::home::look(root, place, "icon").map(str::to_string).or(g.icon.clone()).and_then(|u| crate::images::pic_sq(&u, 44., cx));
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
                    None => div().truncate().text_size(px(T_SEC + 2.)).line_height(px(26.)).font_weight(FontWeight::SEMIBOLD).child(game_name(name.clone(), crate::pages::home::look(root, place, "icon").map(str::to_string).or(g.icon.clone()).as_deref(), cx)),
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(7.))
                        .text_size(px(T_S))
                        .text_color(c(MUTED))
                        .child(state_mark("game-state", running, 6., c(MUTED)))
                        .child(sub),
                ),
        )
        .child(div().flex_none().child(acts));
    let art = div()
        .id("gs-big-art")
        .on_hover(clip.hover)
        .relative()
        .w_full()
        .h(px(big_h - PLATE_H))
        .flex_none()
        .overflow_hidden()
        .bg(c(RAISED_2))
        // (A picture is cut to its own corners, not to its box's: the card's top two.)
        .children(picture.map(|(p, k)| fade_in(img(p).size_full().rounded_t(px(R_L)).object_fit(ObjectFit::Cover), &k).into_any_element()))
        .children(clip.layer);
    let big = hot("gs-big")
        .tones(c(RAISED), c(RAISED_2))
        .w(px(big_w))
        .h(px(big_h))
        .flex_none()
        .flex()
        .flex_col()
        .rounded(px(R_L))
        .overflow_hidden()
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
        let pic = crate::pages::home::look(root, place, "cover").map(str::to_string).and_then(|u| crate::images::pic_pair(&u, pic_w, pic_h, cx));
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
            .children(pic.map(|(p, k)| fade_in(img(p).size_full().rounded(px(R_L)).object_fit(ObjectFit::Cover), &k).into_any_element()))
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
                        .child(div().truncate().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).child(game_name(name.clone(), crate::pages::home::look(root, place, "icon").map(str::to_string).as_deref(), cx)))
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

fn section(root: &Root, title: String, id: &'static str, games: &[&Game], card_w: f32, cx: &mut Context<Root>) -> impl IntoElement {
    div().flex().flex_col().gap(px(12.)).child(section_head(title, Some(games.len()))).child(game_cards(root, id, games, card_w, cx))
}

/// The bot's games as cards in lines, only the lines in view made once there are many (crate::near).
fn game_cards(root: &Root, id: &'static str, games: &[&Game], card_w: f32, cx: &mut Context<Root>) -> Div {
    let key = crate::near::sig((games.len(), games.first().map(|g| g.id.as_str()), card_w.to_bits()));
    // (A card: its picture, 16:9, and the name and line beside the 36 px icon under it.)
    let guess = (card_w - 2. * CPAD) * 9. / 16. + 2. * CPAD + 12. + 40.;
    let scroll = crate::scrolled::handle("games-body");
    div().mx(px(-CPAD)).child(crate::near::tiles_in(Some(&scroll), id, games.len(), card_w, CGAP, key, false, guess, |i| card(root, games[i], card_w, cx).into_any_element()))
}

/// Roblox's charts: a pill a row (Trending, Up-and-coming…), and the row's games.
fn recommended(root: &Root, width: f32, cx: &mut Context<Root>) -> Option<AnyElement> {
    let games = &active_home_games(root)?.recommended;
    if games.is_empty() {
        return None;
    }
    // (Four rows before Show all: owner 2026-11-17, "hiện nhiều hơn… cho cái trang nó dài ra tí".)
    let visible = board_fit(width).0 * 4;
    let open = root.games.recommended_all;
    let shown = if open { games.len() } else { visible.min(games.len()) };
    Some(div().id("recommended-games").flex().flex_col().gap(px(12.))
        .child(div().flex().items_center().justify_between().gap(px(8.))
            .child(section_head(root.tr("games.recommended"), None))
            .when(games.len() > visible, |d| d.child(
                Button::new("recommended-all", root.tr(if open { "gp.less" } else { "games.keepAll" }))
                    .small().quiet().icon_end(if open { "chevronUp" } else { "chevronDown" })
                    .on_click(cx.listener(|r, _, _, cx| {
                        r.games.recommended_all = !r.games.recommended_all;
                        cx.notify();
                    }))
            )))
        .child(board(root, &games[..shown], None, width, cx))
        .into_any_element())
}

/// How many of Roblox's charts are rows here.
const CHART_ROWS: usize = 12;
/// A square tile's picture (the rows after the first).
const SQUARE: f32 = 132.;

/// Roblox's charts, each a row of its own that steps sideways with its arrows, as Roblox's charts page has them (the
/// owner's pick, 2026-11-19, from a page made that way): the first row's games wide, the others' square. A row Roblox
/// gave nothing for is left out; no sign-in, or the charts failed: one line saying so.
fn charts(root: &Root, width: f32, cx: &mut Context<Root>) -> AnyElement {
    let sorts = match &root.games.sorts {
        Some(Ok(sorts)) if !sorts.is_empty() => sorts,
        other => {
            let got = other.as_ref().map(|r| r.clone().map(|_| Vec::new()));
            return roblox_section(root, root.tr("games.charts"), None, None, got.as_ref(), width, cx);
        }
    };
    let label = |s: &roblox::Sort| {
        let key = format!("rx.{}", s.id);
        if root.i18n.has(&key) { root.tr(&key) } else { s.name.clone() }
    };
    let pills = chart_pills(root, sorts, width, cx);
    // One chart picked in the filter: it alone, every game of it, as before the rows (its leader big).
    if let Some(picked) = root.games.sort_at.as_ref().and_then(|id| sorts.iter().find(|s| s.id == *id)) {
        let got = root.games.rows.get(&picked.id).cloned();
        return roblox_section(root, root.tr("games.charts"), Some(pills), Some(label(picked)), got.as_ref(), width, cx);
    }
    let rows: Vec<AnyElement> = sorts.iter().take(CHART_ROWS).enumerate().filter_map(|(k, s)| chart_row(root, k == 0, s, label(s), width, cx)).collect();
    if rows.is_empty() {
        // (Every row empty with no account to ask with: Roblox's rule, the line with Add account.)
        if matches!(root.games.cookie, Some(None)) && let Some(first) = root.games.rows.get(&sorts[0].id).cloned() {
            return roblox_section(root, root.tr("games.charts"), Some(div()), None, Some(&first), width, cx);
        }
        let failed = sorts.iter().take(CHART_ROWS).all(|s| matches!(root.games.rows.get(&s.id), Some(Err(_))));
        let got: Option<Result<Vec<roblox::Game>, String>> = failed.then(|| Err(String::new()));
        return roblox_section(root, root.tr("games.charts"), None, None, got.as_ref(), width, cx);
    }
    div().flex().flex_col().gap(px(12.))
        .child(section_head(root.tr("games.charts"), None))
        .child(pills)
        .child(div().mt(px(8.)).flex().flex_col().gap(px(28.)).children(rows))
        .into_any_element()
}

/// The charts' filter: All (every chart, a row each), then the charts that fit, the rest under More.
fn chart_pills(root: &Root, sorts: &[roblox::Sort], width: f32, cx: &mut Context<Root>) -> Div {
    let label = |s: &roblox::Sort| {
        let key = format!("rx.{}", s.id);
        if root.i18n.has(&key) { root.tr(&key) } else { s.name.clone() }
    };
    let picked = root.games.sort_at.clone().filter(|id| sorts.iter().any(|s| s.id == *id));
    let active = picked.as_ref().and_then(|id| sorts.iter().find(|s| s.id == *id));
    let cap = (width * 0.46).clamp(100., 194.);
    let wide = |t: &str| (t.chars().count() as f32 * 8. + 26.).min(cap) + 6.;
    let all = root.tr("games.allCharts");
    let room = (width - 116.).max(0.);
    let mut used = wide(&all) + active.map_or(0., |s| wide(&label(s)));
    let mut shown: Vec<&roblox::Sort> = Vec::new();
    for s in sorts {
        if active.is_some_and(|a| a.id == s.id) {
            shown.push(s);
        } else if shown.len() < 5 && used + wide(&label(s)) <= room {
            used += wide(&label(s));
            shown.push(s);
        }
    }
    let rest: Vec<&roblox::Sort> = sorts.iter().filter(|s| !shown.iter().any(|x| x.id == s.id)).collect();
    let pick = |id: Option<String>| {
        move |r: &mut Root, _: &ClickEvent, _: &mut Window, cx: &mut Context<Root>| {
            r.games.sort_at = id.clone();
            r.games.more_sorts = false;
            cx.notify();
        }
    };
    let mut row = div().flex().flex_none().items_center().gap(px(4.));
    row = row.child(
        chip("rx-all", picked.is_none())
            .h(px(TOOL_H))
            .px(px(12.))
            .cursor_pointer()
            .child(all)
            .on_click(cx.listener(pick(None))),
    );
    for s in shown {
        let on = picked.as_deref() == Some(s.id.as_str());
        row = row.child(
            chip(SharedString::from(format!("rx-{}", s.id)), on)
                .h(px(TOOL_H))
                .px(px(12.))
                .max_w(px(cap))
                .min_w_0()
                .cursor_pointer()
                .child(div().truncate().child(label(s)))
                .on_click(cx.listener(pick(Some(s.id.clone())))),
        );
    }
    if !rest.is_empty() {
        let open = root.games.more_sorts;
        let columns: f32 = if rest.len() > 10 && width >= 560. { 2. } else { 1. };
        let menu_w = (columns * 232. + 8.).min((width - 14.).max(165.));
        let trigger = std::rc::Rc::new(std::cell::Cell::new(None::<Bounds<Pixels>>));
        let at = trigger.clone();
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
        ).child(canvas(move |bounds, _, _| at.set(Some(bounds)), |_, _, _, _| {}).absolute().inset_0());
        if crate::pages::accounts::pop_alive("rx-more-pop", open) {
            let menu = div()
                .id("rx-more-menu")
                .occlude()
                .w(px(menu_w))
                .p(px(4.))
                .rounded(px(R_M))
                .bg(c(RAISED))
                .border_1()
                .border_color(c(LINE))
                .shadow_lg()
                .flex()
                .flex_wrap()
                .on_mouse_down_out(cx.listener(move |r, event: &MouseDownEvent, _, cx| {
                    if r.games.more_sorts && !trigger.get().is_some_and(|b| b.contains(&event.position)) {
                        r.games.more_sorts = false;
                        cx.notify();
                    }
                }))
                .children(rest.into_iter().map(|s| {
                    menu_row(SharedString::from(format!("rxm-{}", s.id)))
                        .w(px(((menu_w - 10.) / columns).floor()))
                        .child(div().truncate().child(label(s)))
                        .on_click(cx.listener(pick(Some(s.id.clone()))))
                }));
            more = more.child(popup(PopAt::BelowRight(34.), "rx-more-pop", open, menu));
        }
        row = row.child(more);
    }
    div().flex().child(row)
}

/// How a row of square tiles fits a width: tiles a page, a tile's width (with its padding).
fn square_fit(width: f32) -> (usize, f32) {
    let outer = width + 2. * TPAD;
    let cols = ((outer + TGAP) / (SQUARE + 2. * TPAD + TGAP)).floor().max(2.) as usize;
    (cols, ((outer - TGAP * (cols as f32 - 1.)) / cols as f32).floor())
}

/// One chart: its name, the arrows at the right (greyed at either end), and a page of its games that slides in the
/// way it was stepped. Still loading: the tiles' shapes breathing; failed: one quiet line; empty: left out.
fn chart_row(root: &Root, wide: bool, sort: &roblox::Sort, title: String, width: f32, cx: &mut Context<Root>) -> Option<AnyElement> {
    let (cols, w, art_h) = if wide {
        let (cols, w, art_h, _) = board_fit(width);
        (cols, w, art_h)
    } else {
        let (cols, w) = square_fit(width);
        (cols, w, w - 2. * TPAD)
    };
    // Name on one line under a wide picture, on up to two under a square one; then the figures.
    let tile_h = art_h + 2. * TPAD + if wide { 8. + 18. + 4. + 15. } else { 8. + 36. + 4. + 15. };
    let got = root.games.rows.get(&sort.id);
    let list: &[roblox::Game] = match got {
        Some(Ok(l)) if l.is_empty() => return None,
        Some(Ok(l)) => &l[..l.len().min(120)],
        _ => &[],
    };
    let (at, dir, step) = root.games.row_at.get(&sort.id).copied().unwrap_or((0, 0., 0));
    // (The window grew since: the last page stays full.)
    let at = at.min(list.len().saturating_sub(cols));
    let can_back = at > 0;
    let can_on = at + cols < list.len();
    let go = |on: bool| {
        let id = sort.id.clone();
        move |r: &mut Root, _: &ClickEvent, _: &mut Window, cx: &mut Context<Root>| {
            let (now, _, n) = r.games.row_at.get(&id).copied().unwrap_or((0, 0., 0));
            let len = match r.games.rows.get(&id) {
                Some(Ok(l)) => l.len().min(120),
                _ => 0,
            };
            let now = now.min(len.saturating_sub(cols));
            let to = if on { (now + cols).min(len.saturating_sub(cols)) } else { now.saturating_sub(cols) };
            if to != now {
                r.games.row_at.insert(id.clone(), (to, if on { 1. } else { -1. }, n + 1));
                cx.notify();
            }
        }
    };
    let arrows = (list.len() > cols).then(|| {
        div().flex().flex_none().items_center().gap(px(6.))
            .child(IconButton::new(SharedString::from(format!("rxr-back-{}", sort.id)), "chevronLeft").size(32.).tip(root.tr("rx.prev")).disabled(!can_back).on_click(cx.listener(go(false))))
            .child(IconButton::new(SharedString::from(format!("rxr-on-{}", sort.id)), "chevronRight").size(32.).tip(root.tr("rx.next")).disabled(!can_on).on_click(cx.listener(go(true))))
    });
    let head = div().flex().items_center().justify_between().gap(px(8.)).child(section_head(title, None)).children(arrows);
    let body = match got {
        None => waiting_rows(cols, w - 2. * TPAD, tile_h - 2. * TPAD, TGAP + 2. * TPAD),
        Some(Err(_)) => div().h(px(36.)).flex().items_center().gap(px(8.)).text_size(px(T_M)).text_color(c(MUTED))
            .child(icon("info", 14., c(MUTED))).child(root.tr("games.chartsFailed")).into_any_element(),
        Some(Ok(_)) => {
            let page = div().mx(px(-TPAD)).flex().gap(px(TGAP)).children(list[at..(at + cols).min(list.len())].iter().enumerate().map(|(n, g)| {
                chart_tile(root, &sort.id, at + n, g, wide, w, art_h, tile_h, cx).into_any_element()
            }).collect::<Vec<_>>());
            if step == 0 {
                page.into_any_element()
            } else {
                page.with_animation(SharedString::from(format!("rxr-slide-{}-{step}", sort.id)), Animation::new(std::time::Duration::from_millis(260)).with_easing(ease_out_quint()), move |d, t| d.left(px(dir * 48. * (1. - t))).opacity(0.35 + 0.65 * t)).into_any_element()
            }
        }
    };
    Some(div().id(SharedString::from(format!("rxr-{}", sort.id))).flex().flex_col().gap(px(12.)).child(head).child(div().relative().child(body)).into_any_element())
}

/// Players as Roblox's charts write them: 301.5K, 1.2M.
fn short_count(n: i64) -> String {
    let f = n as f64;
    let (v, unit) = if f >= 1e6 {
        (f / 1e6, "M")
    } else if f >= 1e3 {
        (f / 1e3, "K")
    } else {
        return n.to_string();
    };
    let t = if v >= 100. { format!("{v:.0}") } else { format!("{v:.1}") };
    format!("{}{unit}", t.trim_end_matches(".0"))
}

/// A game in a chart row: its picture (wide in the first row, its square icon in the others), its name, and who plays
/// it now and how many like it, each with its mark. A click opens its page here.
#[allow(clippy::too_many_arguments)]
fn chart_tile(root: &Root, row: &str, i: usize, g: &roblox::Game, wide: bool, w: f32, art_h: f32, tile_h: f32, cx: &mut Context<Root>) -> Hot {
    let pic_w = w - 2. * TPAD;
    let image = if wide {
        art(root, g, false, pic_w, art_h, cx)
    } else {
        let u = &g.universe;
        root.games.icons.get(u).cloned().or_else(|| crate::pages::home::look(root, g.place, "icon").map(str::to_string)).and_then(|u| images::pic_pair(&u, pic_w, art_h, cx))
    };
    let tone = art_tone(root, g, cx);
    let votes = g.up + g.down;
    let (place, name) = (g.place, g.name.clone());
    let clip = crate::pages::clips::tile_clip(root, place, cx);
    let figure = |mark: &'static str, text: String| div().flex().items_center().gap(px(4.)).child(icon(mark, 12., c(MUTED))).child(text);
    hot(SharedString::from(format!("rxt-{row}-{i}-{place}")))
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
                .id(SharedString::from(format!("rxt-art-{row}-{i}-{place}")))
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
                    None => d.child(letter(&g.name, if wide { 34. } else { 30. })),
                })
                .children(clip.layer),
        )
        .child(
            div()
                .mt(px(8.))
                // (As tall as the name is, one line or two, and the figures right under it.)
                .max_h(px(if wide { 18. } else { 36. }))
                .flex_none()
                .text_size(px(T_M))
                .font_weight(FontWeight::SEMIBOLD)
                .line_height(px(18.))
                .overflow_hidden()
                .map(|d| if wide { d.truncate() } else { d.line_clamp(2) })
                .child(clean(&g.name)),
        )
        .child(
            div()
                .mt(px(4.))
                .h(px(15.))
                .flex_none()
                .flex()
                .items_center()
                .gap(px(10.))
                .text_size(px(T_S))
                .line_height(px(15.))
                .text_color(c(MUTED))
                .font_features(tnum())
                .when(g.playing > 0, |d| d.child(figure("user", short_count(g.playing))))
                .when(votes > 0, |d| d.child(figure("like", format!("{}%", (g.up * 100 + votes / 2) / votes)))),
        )
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

/// The leading game and the next four ranks set the pace of the chart. The rest use the search grid.
fn board(root: &Root, list: &[roblox::Game], chart: Option<String>, width: f32, cx: &mut Context<Root>) -> AnyElement {
    let (cols, w, art_h, tile_h) = board_fit(width);
    let list = &list[..list.len().min(120)];
    let ranked = chart.is_some();
    let scope: SharedString = format!("rx-in-{}", chart.unwrap_or_default()).into();
    let mut out = div().flex().flex_col().gap(px(16.));
    let mut from = 0;
    if ranked && let Some(leader) = list.first() {
        from = list.len().min(5);
        let gap = 12.;
        let side_by_side = width >= 868. && from > 1;
        let hero_w = if side_by_side { (width * 0.62).floor() } else { width };
        let hero_h = (hero_w * 9. / 16.).round().clamp(224., 372.);
        let side_w = if side_by_side { width - hero_w - gap } else { width };
        let rail_cols = if !side_by_side && side_w >= 560. { 2 } else { 1 };
        let row_h = if side_by_side { ((hero_h - 8. * (from - 2) as f32) / (from - 1) as f32).clamp(55., 84.) } else { 64. };
        let row_w = ((side_w - gap * (rail_cols - 1) as f32) / rail_cols as f32).floor();
        let rail = div().w(px(side_w)).min_w_0().flex_none().flex().flex_col().gap(px(8.)).children(
            list[1..from].chunks(rail_cols).enumerate().map(|(row, games)| {
                div().flex().gap(px(gap)).children(games.iter().enumerate().map(|(col, game)| {
                    rank_row(root, row * rail_cols + col + 2, game, row_w, row_h, cx)
                }).collect::<Vec<_>>())
            }).collect::<Vec<_>>(),
        );
        out = out.child(div().flex().when(!side_by_side, |d| d.flex_col()).gap(px(gap))
            .child(lead(root, leader, hero_w, hero_h, cx))
            .when(from > 1, |d| d.child(rail)));
    }
    out.child(div().mx(px(-TPAD)).flex().flex_col().gap(px(TGAP)).children(
        list[from..].chunks(cols).enumerate().map(|(row, games)| {
            div().flex().gap(px(TGAP)).children(games.iter().enumerate().map(|(col, game)| {
                let i = from + row * cols + col;
                cascade(scope.clone(), i, tile(root, i, game, ranked, w, art_h, tile_h, cx))
            }).collect::<Vec<_>>())
        }).collect::<Vec<_>>(),
    )).into_any_element()
}

fn rank_row(root: &Root, rank: usize, game: &roblox::Game, width: f32, height: f32, cx: &mut Context<Root>) -> Hot {
    let image_h = height - 8.;
    let image_w = (image_h * 16. / 9.).min((width * 0.32).max(54.)).round();
    let picture = art(root, game, false, image_w, image_h, cx);
    let (place, name) = (game.place, game.name.clone());
    hot(SharedString::from(format!("rx-rank-{rank}-{place}")))
        .w(px(width)).h(px(height)).flex_none().rounded(px(R_L)).bg(c(RAISED))
        .border_1().border_color(c(LINE)).hover(|d| d.bg(c(RAISED_2)))
        .flex().items_center().gap(px(9.)).px(px(9.)).cursor_pointer()
        .child(div().w(px(20.)).flex_none().text_size(px(T_S)).font_features(tnum()).text_color(c(MUTED)).child(rank.to_string()))
        .child(div().w(px(image_w)).h(px(image_h)).flex_none().rounded(px(R_S)).overflow_hidden().bg(c(INSET))
            .flex().items_center().justify_center()
            .map(|d| match picture {
                Some((picture, key)) => d.child(fade_in(img(picture).size_full().object_fit(ObjectFit::Cover), &key)),
                None => d.child(letter(&game.name, 22.)),
            }))
        .child(div().flex_1().min_w_0().flex().flex_col().gap(px(4.))
            .child(div().truncate().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).child(clean(&game.name)))
            .child(chart_stats(root, game, false)))
        .on_click(cx.listener(move |root, _, _, cx| { open_place(root, place, &name); cx.notify(); }))
}

fn chart_stats(root: &Root, game: &roblox::Game, full: bool) -> Div {
    let votes = game.up + game.down;
    let players = thousands(game.playing);
    div().flex().items_center().gap(px(7.)).text_size(px(T_S)).font_features(tnum())
        .text_color(if full { on_picture(0.88) } else { c(MUTED) })
        .when(game.playing > 0, |d| d.child(div().flex_none().size(px(6.)).rounded_full().bg(c(OK)))
            .child(if full { root.i18n.tv("rx.playingN", &[("n", &players)]) } else { players }))
        .when(game.playing > 0 && votes > 0, |d| d.child(sep()))
        .when(votes > 0, |d| d.child(root.i18n.tv("games.likesP", &[("n", &((game.up * 100 + votes / 2) / votes).to_string())])))
}

/// A game's wide picture (its square icon until that has come), or the first letter of its name.
/// (Made at the `w`×`h` it is shown at, and with the picture itself, which keys its fade-in.)
fn art(root: &Root, g: &roblox::Game, sharp: bool, w: f32, h: f32, cx: &mut Context<Root>) -> Option<(ImageSource, std::sync::Arc<Image>)> {
    let u = &g.universe;
    let wide = if sharp { root.games.covers_big.get(u).or_else(|| root.games.covers.get(u)) } else { root.games.covers.get(u) };
    if wide.is_none() {
        need_cover(root, *u, cx);
    }
    let pic = wide.or_else(|| root.games.icons.get(u)).cloned().or_else(|| crate::pages::home::look(root, g.place, "icon").map(str::to_string));
    pic.as_deref().and_then(|u| images::pic_pair(u, w, h, cx))
}

/// Notes a game drawn wide with no cover yet. The first one noted in a frame asks for all of them once the frame is
/// made (one call to Roblox for the frame's tiles). Until it comes, the tile shows the icon (150×150) as a stand-in.
fn need_cover(root: &Root, universe: i64, cx: &mut Context<Root>) {
    if universe <= 0 || root.games.asked_covers.contains(&universe) {
        return;
    }
    let mut need = root.games.need_covers.borrow_mut();
    let first = need.is_empty();
    if need.insert(universe) && first {
        cx.spawn(async move |this, cx| {
            let _ = this.update(cx, |r, cx| ask_covers(r, cx));
        })
        .detach();
    }
}

/// The wide pictures (480×270) of the games noted by need_cover, once each; one Roblox didn't answer is let go after a
/// while, so it is noted again if it is still drawn.
fn ask_covers(r: &mut Root, cx: &mut Context<Root>) {
    let need = r.games.need_covers.take();
    let mut want: Vec<i64> = need.into_iter().filter(|u| !r.games.covers.contains_key(u)).collect();
    want.retain(|u| r.games.asked_covers.insert(*u));
    if want.is_empty() {
        return;
    }
    let asked = want.clone();
    let rx = roblox::run(move || roblox::covers(&want, "480x270"));
    cx.spawn(async move |this, cx| {
        let covers = rx.await.unwrap_or_default();
        let missed: Vec<i64> = asked.into_iter().filter(|u| !covers.contains_key(u)).collect();
        let _ = this.update(cx, |r, cx| {
            r.games.covers.extend(covers);
            cx.notify();
        });
        if !missed.is_empty() {
            cx.background_executor().timer(RETRY).await;
            let _ = this.update(cx, |r, cx| {
                for u in &missed {
                    if may_retry(r, format!("cover:{u}")) {
                        r.games.asked_covers.remove(u);
                    }
                }
                cx.notify();
            });
        }
    })
    .detach();
}

/// A chart game's own colour, from its icon (else its cover): the ground of its picture.
fn art_tone(root: &Root, g: &roblox::Game, cx: &mut App) -> Option<u32> {
    let u = &g.universe;
    let pic = root.games.icons.get(u).or_else(|| root.games.covers.get(u)).cloned().or_else(|| crate::pages::home::look(root, g.place, "icon").map(str::to_string));
    pic.as_deref().and_then(|u| images::tone(u, cx))
}

fn letter(name: &str, size: f32) -> Div {
    let first: String = name.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().collect()).unwrap_or_default();
    div().text_size(px(size)).font_weight(FontWeight::BOLD).text_color(c(MUTED)).child(first)
}

/// Who plays it now, on its picture: a green dot and the number.
fn playing_pill(n: i64) -> Div {
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
        .child(thousands(n))
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
                .when(!ranked && g.playing > 0, |d| d.child(div().absolute().left(px(8.)).bottom(px(8.)).child(playing_pill(g.playing)))),
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
                .when(ranked, |d| d.child(chart_stats(root, g, false)))
                .when(!ranked && votes > 0, |d| d.child(root.i18n.tv("games.likesP", &[("n", &((g.up * 100 + votes / 2) / votes).to_string())]))),
        )
}

fn lead(root: &Root, g: &roblox::Game, w: f32, h: f32, cx: &mut Context<Root>) -> Hot {
    let image = art(root, g, true, w, h, cx);
    let (place, name) = (g.place, g.name.clone());
    let clip = crate::pages::clips::tile_clip(root, place, cx);
    hot(SharedString::from(format!("rxg-lead-{}", g.place)))
        .w(px(w))
        .h(px(h))
        .flex_none()
        .rounded(px(R_L))
        .cursor_pointer()
        // (No lift: a band this big rising off its place read as a gap under it, the owner 2026-11-17.)
        .lift(0.)
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
                        .left(px(22.))
                        .right(px(22.))
                        .bottom(px(20.))
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(div().flex().child(div().h(px(24.)).px(px(9.)).flex().items_center()
                            .rounded(px(R_S)).bg(c(FACE)).text_size(px(T_S)).font_weight(FontWeight::BOLD)
                            .text_color(crate::theme::on_text()).child("#1")))
                        .child(div().text_size(px(T_STAGE)).font_weight(FontWeight::BOLD).line_height(px(33.)).text_color(on_picture(1.)).line_clamp(2).child(clean(&g.name)))
                        .child(chart_stats(root, g, true))
                        .child(div().mt(px(4.)).flex().child(div().px(px(10.)).h(px(30.)).flex().items_center()
                            .gap(px(5.)).rounded(px(R_S)).bg(ca(GROUND, 0.82))
                            .text_size(px(T_S)).font_weight(FontWeight::SEMIBOLD).text_color(on_picture(1.))
                            .child(root.tr("home.gamePage")).child(icon("chevronRight", 13., on_picture(0.92))))),
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
    // (On the game's page the lines stand open under its tabs, no box: one box was all its parts, the owner 2026-11-15.)
    radar_lines(root, radar(root, place, name), false).map(|d| d.px(px(0.)).py(px(0.)).bg(gpui::transparent_black()).border_0())
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
    let mut strip = div().px(px(12.)).py(px(8.)).rounded(px(R_L + 2.)).bg(block_ground()).border_1().border_color(ca(WARN, 0.3)).flex().flex_col().gap(px(4.));
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
    let cover_url = crate::pages::home::look(root, g.place, "cover").map(str::to_string);
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
    let own = game.and_then(|g| icon_of(root, g)).or_else(|| crate::pages::home::look(root, place, "icon").map(str::to_string)).or_else(|| crate::pages::home::look(root, place, "cover").map(str::to_string));
    set_game_tint(own.as_deref().and_then(|u| images::tone(u, cx)));
    let details = root.games.details.get(&place);
    let name = details.map(|d| s(d, "name").to_string()).filter(|n| !n.is_empty()).map(|n| clean(&n)).unwrap_or_else(|| open.name.clone());
    let has_news = game.is_some_and(|g| g.news.is_some());
    let has_prices = game.is_some_and(|g| g.prices);
    let has_wiki = game.is_some_and(|g| g.wiki.is_some());
    let working = |v: &Value| v.get("codes").and_then(Value::as_array).map(|l| l.iter().filter(|c| c.get("works").and_then(Value::as_bool).unwrap_or(false)).count());
    let codes = root.games.codes.get(&place).and_then(Option::as_ref).filter(|v| v.get("codes").and_then(Value::as_array).is_some_and(|l| !l.is_empty()));
    let tab = match root.games.tab {
        Tab::Codes if place > 0 => Tab::Codes,
        Tab::News if has_news => Tab::News,
        Tab::Prices if has_prices => Tab::Prices,
        Tab::Passes if place > 0 => Tab::Passes,
        Tab::Badges if place > 0 => Tab::Badges,
        Tab::Servers if place > 0 => Tab::Servers,
        Tab::Wiki if has_wiki => Tab::Wiki,
        _ => Tab::Overview,
    };
    let inner = width - 2. * (PAGE_X);
    // BLOXKEEP_SHOT_SCROLL=700: a shot's page scrolled down that far (asked again until the page is long enough).
    #[cfg(feature = "shot")]
    if let Ok(y) = std::env::var("BLOXKEEP_SHOT_SCROLL").map(|v| v.parse::<f32>().unwrap_or(0.)) {
        if (root.games.page_scroll.offset().y + px(y)).abs() > px(1.) {
            root.games.page_scroll.set_offset(point(px(0.), px(-y)));
        }
    }

    // The page's parts as a row of choices at a toolbar's height, the one shown a yellow block (the Accounts page's
    // filters): no underline under a tab.
    let mut tabs = div().mx(px(PAGE_X)).flex().flex_wrap().items_center().gap(px(4.));
    for (t, icon_name, label, count) in [
        (Tab::Overview, "info", root.tr("gp.overview"), 0),
        (Tab::News, "news", root.tr("games.news"), game.map(unread).unwrap_or(0)),
        (Tab::Prices, "tag", root.tr("prices.tab"), 0),
        (Tab::Passes, "key", root.tr("gp.passes"), 0),
        (Tab::Badges, "award", root.tr("bd.title"), 0),
        (Tab::Servers, "server", root.tr("gp.servers"), 0),
        (Tab::Codes, "gift", root.tr("gc.title"), codes.and_then(working).unwrap_or(0) as i64),
        (Tab::Wiki, "book", root.tr("gp.wiki"), 0),
    ] {
        if (t == Tab::News && !has_news)
            || (t == Tab::Prices && !has_prices)
            || (matches!(t, Tab::Passes | Tab::Badges | Tab::Servers) && place <= 0)
            || (t == Tab::Wiki && !has_wiki)
            // (Codes is there at once, as Passes and Servers: it says it is looking, then what it found; the owner,
            // 2026-11-20: "sao ko cho nó hiện ra trước rồi nội dung trong đó load ra sau".)
            || (t == Tab::Codes && place <= 0)
        {
            continue;
        }
        tabs = tabs.child(
            tab_button(t, icon_name, label, tab == t)
                .when(count > 0, |d| d.child(chip_count(count as usize, false)))
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
        Tab::Badges => crate::pages::badges::view(root, width, cx),
        Tab::Servers => crate::pages::servers::view(root, details.map_or(0, |d| int(d, "rootPlaceId")), inner, cx),
        Tab::Wiki => crate::pages::wiki::view(root, inner, cx),
        Tab::Codes => codes_block(root, place, cx)
            .unwrap_or_else(|| {
                let asked = root.games.codes.get(&place).is_some_and(Option::is_some);
                div()
                    .flex()
                    .flex_col()
                    .child(heading(root.tr("gc.title"), None))
                    .child(div().mt(px(10.)).text_size(px(T_M)).text_color(c(MUTED)).child(root.tr(if asked { "gc.none" } else { "gc.finding" })))
            })
            .into_any_element(),
    };

    // BLOXKEEP_SHOT_BODY=game-page with BLOXKEEP_SHOT_SCROLL=<px>: a hidden shot of the game page scrolled down that far.
    #[cfg(feature = "shot")]
    if std::env::var("BLOXKEEP_SHOT_BODY").is_ok_and(|b| b == "game-page")
        && let Ok(y) = std::env::var("BLOXKEEP_SHOT_SCROLL").map(|v| v.parse::<f32>().unwrap_or(0.))
        && (root.games.page_scroll.offset().y + px(y)).abs() > px(1.)
    {
        root.games.page_scroll.set_offset(point(px(0.), px(-y)));
    }
    crate::scrolled::with_bar("game-page", &root.games.page_scroll, div().id("game-page").size_full().overflow_y_scroll().track_scroll(&root.games.page_scroll).child(
        div()
            .flex()
            .flex_col()
            .gap(px(18.))
            .pb(px(28.))
            .child(hero(root, all, game, details, &open, &name, place, width, cx))
            .children(details.and_then(|d| community_panel(root, int(d, "creatorGroupId"), cx))
                .map(|panel| div().px(px(PAGE_X)).child(panel)))
            .when_some(root.games.picking.as_ref().filter(|(g, _)| *g == open.id).map(|(_, p)| p.clone()), |d, ticked| {
                d.child(div().px(px(PAGE_X)).child(join(root, place, ticked, inner, cx)))
            })
            .when(!only_overview, |d| d.child(tabs))
            .child(div().px(px(PAGE_X)).child(content)),
    ))
}

fn preselected_join_accounts(root: &Root) -> HashSet<String> {
    root.accounts()
        .iter()
        .filter(|account| crate::pages::accounts::can_join_game(account))
        .filter_map(|account| account.get("id").and_then(Value::as_str))
        .filter(|id| crate::pages::tabs::play_selected(root, id))
        .map(str::to_owned)
        .collect()
}

/// A server picked in the Servers tab: open Play's account picker for that server.
pub fn pick_for_server(r: &mut Root, job: String) {
    let Some(open) = r.games.open.as_ref().map(|o| o.id.clone()) else { return };
    if !r.games.picking.as_ref().is_some_and(|(g, _)| *g == open) {
        r.games.picking = Some((open, preselected_join_accounts(r)));
    }
    r.games.pick_job = Some(job);
    r.games.pick_place = None;
    // (A link pasted before would send the server's Job ID to that link's place.)
    r.games.pick_link = None;
}

/// The country Play joins a server in (ui-prefs "serverRegion", a two-letter code); None: any, Roblox picks.
pub fn region_pref() -> Option<String> {
    crate::prefs::get("serverRegion").and_then(|v| v.as_str().map(str::to_string)).filter(|s| s.len() == 2)
}

/// A country's name for the menu (every ISO code, countries.rs; a code not there by itself).
pub(crate) fn country(root: &Root, cc: &str) -> String {
    // (The languages after Vietnamese have the names Windows' own ICU gives, in assets/lang/<code>.json.)
    let own = root.i18n.own_word(&format!("country.{}", cc.trim().to_ascii_uppercase())).map(str::to_string);
    own.or_else(|| crate::countries::name(cc, root.i18n.vietnamese).map(str::to_string)).unwrap_or_else(|| cc.to_string())
}

/// A country's English name (a city of the same name is left out beside it).
fn country_en(cc: &str) -> &'static str {
    crate::countries::english(cc).unwrap_or("")
}

/// A country's flag (flagcdn.com's picture of it), `w` wide at a flag's 4:3; a grey stand-in while it loads.
pub(crate) fn flag(cc: &str, w: f32, cx: &mut App) -> AnyElement {
    let url = format!("https://flagcdn.com/w40/{}.png", cc.to_lowercase());
    match images::pic_at(&url, w, w * 0.75, cx) {
        Some(src) => img(src).w(px(w)).h(px(w * 0.75)).flex_none().rounded(px(2.)).object_fit(ObjectFit::Cover).into_any_element(),
        None => div().w(px(w)).h(px(w * 0.75)).flex_none().rounded(px(2.)).bg(c(RAISED_2)).into_any_element(),
    }
}

/// Where a server of this game runs (country, city), when the country pick has found it.
fn server_at(root: &Root, place: i64, job: &str) -> Option<(String, String)> {
    let v = root.games.regions.get(&place)?.as_ref()?.as_ref().ok()?;
    let row = v.get("servers")?.as_array()?.iter().find(|s| s.get("jobId").and_then(Value::as_str) == Some(job))?;
    Some((row.get("cc")?.as_str()?.to_string(), row.get("city").and_then(Value::as_str).unwrap_or_default().to_string()))
}

/// Play's server in a country: the fullest one there that still has room for the accounts picked.
fn region_server(r: &Root, place: i64, cc: &str) -> Option<String> {
    let v = r.games.regions.get(&place)?.as_ref()?.as_ref().ok()?;
    let party = r.games.picking.as_ref().map(|(_, t)| t.len()).unwrap_or(1).max(1) as i64;
    v.get("servers")?
        .as_array()?
        .iter()
        .filter(|s| s.get("cc").and_then(Value::as_str) == Some(cc))
        .filter(|s| int(s, "maxPlayers") - int(s, "playing") >= party)
        .max_by_key(|s| int(s, "playing"))
        .and_then(|s| s.get("jobId").and_then(Value::as_str))
        .map(str::to_string)
}

/// The open Play row of this game takes a server in the country picked, or says there is none with room.
fn region_apply(r: &mut Root, place: i64, cc: &str) {
    r.games.region_note = None;
    if r.games.picking.is_none() {
        return;
    }
    match region_server(r, place, cc) {
        Some(job) => r.games.pick_job = Some(job),
        None => {
            r.games.pick_job = None;
            let name = country(r, cc);
            r.games.region_note = Some(r.i18n.tv("rg.noneThere", &[("c", &name)]));
        }
    }
}

/// Asks the app where this game's servers are (serverRegions); a Play waiting on it takes its server then.
pub fn load_regions(r: &mut Root, place: i64, cx: &mut Context<Root>) {
    if matches!(r.games.regions.get(&place), Some(None)) {
        return;
    }
    r.games.regions.insert(place, None);
    ask_regions(r, place, 0, cx);
}

/// One round of `load_regions`: the first asks of a few servers (8, shown at once: some 3 s), each next one of more
/// while the menu shows what is found, until all are placed or Roblox asks for a pause. Asking all 60 before showing
/// any took half a minute (the owner, 2026-11-19: "lâu mới hiện server").
fn ask_regions(r: &mut Root, place: i64, round: u32, cx: &mut Context<Root>) {
    let Some(link) = r.link.clone() else { return };
    let ids: Vec<String> = r.games.picking.as_ref().map(|(_, t)| t.iter().cloned().collect()).unwrap_or_else(|| preselected_join_accounts(r).into_iter().collect());
    let rx = link.call("serverRegions", json!({ "placeId": place, "ids": ids, "ask": if round == 0 { 8 } else { 16 } }));
    answer(rx, cx, move |r, answer, cx| {
        // (A later round that failed keeps what the earlier ones found.)
        if round > 0 && !matches!(answer, Ok(Ok(_))) {
            if let Some(Some(Ok(v))) = r.games.regions.get_mut(&place) {
                v["more"] = json!(0);
            }
            cx.notify();
            return;
        }
        let more = matches!(&answer, Ok(Ok(v)) if int(v, "more") > 0 && v.get("busy").and_then(Value::as_bool) != Some(true));
        let got = match answer {
            Ok(Ok(v)) if v.get("busy").and_then(Value::as_bool) == Some(true) => {
                let n = v.get("retryIn").and_then(Value::as_i64).unwrap_or(20);
                Err(r.i18n.tv("srv.wait", &[("n", &n.to_string())]))
            }
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err(e.to_string()),
            Err(_) => Err(r.tr("srv.loadError")),
        };
        r.games.regions.insert(place, Some(got));
        // (A Play waiting for its country takes a server as soon as one there is found, or when all are asked.)
        if r.games.region_play == Some(place)
            && let Some(cc) = region_pref()
            && (!more || region_server(r, place, &cc).is_some())
        {
            r.games.region_play = None;
            region_apply(r, place, &cc);
        }
        if more && round < 8 {
            ask_regions(r, place, round + 1, cx);
        }
        cx.notify();
    });
}

/// The country pick beside Play (owner 2026-11-17, after another launcher's flag key beside its Play): the flag and
/// code of the country Play joins a server in, or Any; its menu lists the countries this game's servers run in now,
/// how many servers each, found through server_regions.rs.
fn region_button(root: &Root, place: i64, cx: &mut Context<Root>) -> AnyElement {
    let chosen = region_pref();
    let open = root.games.region_menu;
    let tip: SharedString = root.tr("rg.tip").into();
    let key = div()
        .id("gm-region")
        .h(px(36.))
        .px(px(10.))
        .flex()
        .items_center()
        .gap(px(7.))
        .rounded(px(R_M))
        .border_1()
        .border_color(c(if open { FACE } else { LINE }))
        .text_size(px(T_M))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c(TEXT))
        .cursor_pointer()
        .hover(|d| d.bg(c(RAISED_2)))
        .child(match &chosen {
            Some(cc) => flag(cc, 20., cx),
            None => icon("globe", 16., c(MUTED)).into_any_element(),
        })
        .child(chosen.clone().unwrap_or_else(|| root.tr("rg.any")))
        .child(icon(if open { "chevronUp" } else { "chevronDown" }, 14., c(MUTED)))
        .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
        .on_click(cx.listener(move |r, _, _, cx| {
            // (The click that closed it by landing outside the menu doesn't open it again.)
            if r.games.region_closed.is_some_and(|t| t.elapsed().as_millis() < 250) {
                return;
            }
            r.games.region_menu = !r.games.region_menu;
            // (Asked again when it failed, and when Roblox cut the last asking short.)
            let again = match r.games.regions.get(&place) {
                Some(None) => false,
                Some(Some(Ok(v))) => v.get("stopped").and_then(Value::as_bool).unwrap_or(false),
                _ => true,
            };
            if r.games.region_menu && again {
                load_regions(r, place, cx);
            }
            cx.notify();
        }));
    let pick = move |r: &mut Root, cc: Option<String>| {
        crate::prefs::set("serverRegion", cc.clone().map(Value::String).unwrap_or(Value::Null));
        r.games.region_menu = false;
        r.games.region_closed = Some(std::time::Instant::now());
        match cc {
            Some(cc) => region_apply(r, place, &cc),
            None => {
                r.games.region_note = None;
                r.games.pick_job = None;
            }
        }
    };
    let mut rows = div().flex().flex_col();
    rows = rows.child(div().px(px(10.)).pt(px(6.)).pb(px(4.)).text_size(px(T_XS)).font_weight(FontWeight::SEMIBOLD).text_color(c(MUTED)).child(root.tr("rg.title").to_uppercase()));
    rows = rows.child(
        menu_row("rg-any")
            .h(px(46.))
            .child(div().w(px(20.)).flex_none().flex().justify_center().child(icon("globe", 16., c(MUTED))))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(div().truncate().text_color(c(TEXT)).child(root.tr("rg.anyRow")))
                    .child(div().truncate().text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("rg.anySub"))),
            )
            .when(chosen.is_none(), |d| d.child(tick_mark(16.)))
            .on_click(cx.listener(move |r, _, _, cx| {
                pick(r, None);
                cx.notify();
            })),
    );
    let said = |text: String| div().px(px(10.)).py(px(8.)).text_size(px(T_S)).text_color(c(MUTED)).child(text);
    match root.games.regions.get(&place) {
        None | Some(None) => rows = rows.child(said(root.tr("rg.finding"))),
        Some(Some(Err(e))) => {
            rows = rows.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .pr(px(6.))
                    .child(div().flex_1().min_w_0().child(said(e.clone())))
                    .child(Button::new("rg-retry", root.tr("srv.retry")).small().on_click(cx.listener(move |r, _, _, cx| {
                        r.games.regions.remove(&place);
                        load_regions(r, place, cx);
                        cx.notify();
                    }))),
            )
        }
        Some(Some(Ok(v))) => {
            let list: Vec<Value> = v.get("regions").and_then(Value::as_array).cloned().unwrap_or_default();
            if v.get("blocked").and_then(Value::as_bool).unwrap_or(false) {
                rows = rows.child(said(root.tr("rg.blocked")));
            } else if list.is_empty() {
                rows = rows.child(said(root.tr("rg.none")));
            }
            for (i, g) in list.iter().enumerate() {
                let cc = s(g, "cc").to_string();
                let cities: Vec<String> = g.get("cities").and_then(Value::as_array).into_iter().flatten().filter_map(|c| c.as_str().map(str::to_string)).collect();
                let name = country(root, &cc);
                // (A city named as its country, Singapore or Hong Kong, is said once.)
                let cities: Vec<String> = cities.into_iter().filter(|c| *c != name && *c != country_en(&cc)).collect();
                let words = if cities.is_empty() { name } else { format!("{name} · {}", cities.join(", ")) };
                let on = chosen.as_deref() == Some(cc.as_str());
                let to = cc.clone();
                rows = rows.child(
                    menu_row(("rg-cc", i))
                        .child(flag(&cc, 20., cx))
                        .child(div().flex_1().min_w_0().truncate().when(on, |d| d.text_color(c(TEXT))).child(words))
                        .child(div().flex_none().text_size(px(T_S)).text_color(c(MUTED)).font_features(tnum()).child(root.i18n.tv("rg.servers", &[("n", &int(g, "servers").to_string())])))
                        .when(on, |d| d.child(tick_mark(16.)))
                        .on_click(cx.listener(move |r, _, _, cx| {
                            pick(r, Some(to.clone()));
                            cx.notify();
                        })),
                );
            }
            if int(v, "more") > 0 {
                rows = rows.child(said(root.tr("rg.findingMore")));
            }
            let unknown = int(v, "unknown");
            if unknown > 0 {
                rows = rows.child(said(root.i18n.tv("rg.unknown", &[("n", &unknown.to_string())])));
            }
            if v.get("stopped").and_then(Value::as_bool).unwrap_or(false) {
                rows = rows.child(said(root.tr("rg.limited")));
            }
        }
    }
    let menu = div()
        .id("gm-region-menu")
        .occlude()
        .w(px(300.))
        .p(px(4.))
        .rounded(px(R_M))
        .bg(c(RAISED))
        .border_1()
        .border_color(c(LINE))
        .on_mouse_down_out(cx.listener(|r, _, _, cx| {
            if r.games.region_menu {
                r.games.region_menu = false;
                r.games.region_closed = Some(std::time::Instant::now());
                cx.notify();
            }
        }))
        .child(rows);
    div()
        .relative()
        .child(key)
        .child(popup(PopAt::BelowLeft(42.), "gm-region-pop", open, menu))
        .into_any_element()
}

/// Play's row: the free accounts as ticks, then Join with those ticked.
fn join(root: &Root, place: i64, ticked: HashSet<String>, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    use crate::pages::accounts::can_join_game;
    // Not the ones this game has banned (its Overview says Banned for them).
    let banned: HashSet<String> = root
        .games
        .stays
        .get(&place)
        .map(|l| l.iter().filter(|a| s(a, "now") == "banned").map(|a| s(a, "id").to_string()).collect())
        .unwrap_or_default();
    let eligible: Vec<&Value> = root.accounts().iter().filter(|a| can_join_game(a) && !banned.contains(s(a, "id"))).collect();
    let ids: Vec<String> = eligible.iter().filter_map(|a| a.get("id").and_then(Value::as_str)).map(str::to_string).collect();
    let n = ids.iter().filter(|i| ticked.contains(*i)).count();
    let query = root.games.join_search.as_ref().map(|search| search.read(cx).value().trim().to_string()).unwrap_or_default();
    let rows: Vec<_> = eligible.into_iter().filter(|account| crate::pages::accounts::found(account, &query)).collect();
    let visible: Vec<String> = rows.iter().map(|account| s(account, "id").to_string()).collect();
    let all_on = !visible.is_empty() && visible.iter().all(|id| ticked.contains(id));
    let head = div()
        .flex()
        .items_center()
        .gap(px(12.))
        .child(div().flex_1().min_w_0().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).child(root.tr("gp.pickWho")))
        .flex_wrap()
        .when(ids.len() > 1 && !visible.is_empty(), |d| {
            let all = visible.clone();
            let filtered = !query.is_empty();
            d.child(Check::new("gm-pick-all", all_on).label(root.tr(if query.is_empty() { "gp.pickAll" } else { "gp.pickMatches" })).on_toggle({
                let this = cx.entity().downgrade();
                move |on, _, cx| {
                    let all = all.clone();
                    let on = *on;
                    let _ = this.update(cx, |r, cx| {
                        if let Some((_, t)) = r.games.picking.as_mut() {
                            if !filtered {
                                *t = if on { all.into_iter().collect() } else { HashSet::new() };
                            } else if on {
                                t.extend(all);
                            } else {
                                t.retain(|id| !all.contains(id));
                            }
                        }
                        cx.notify();
                    });
                }
            }))
        })
        // (Opened from the Servers tab: which server it joins.)
        .when_some(root.games.pick_job.clone().filter(|j| !j.is_empty()), |d, job| {
            // (With the country pick: the server's flag and city before its id.)
            let short: String = job.chars().take(8).collect();
            let line = div().flex().items_center().gap(px(6.)).text_size(px(T_S)).text_color(c(MUTED)).font_features(tnum());
            d.child(match server_at(root, place, &job) {
                Some((cc, city)) => line.child(flag(&cc, 16., cx)).child(if city.is_empty() { short } else { format!("{city} · {short}") }),
                None => line.child(icon("server", 13., c(MUTED))).child(short),
            })
        })
        .when(root.games.region_play == Some(place), |d| {
            let name = region_pref().map(|cc| country(root, &cc)).unwrap_or_default();
            d.child(div().text_size(px(T_S)).text_color(c(MUTED)).child(root.i18n.tv("rg.findingRow", &[("c", &name)])))
        })
        .when_some(root.games.region_note.clone(), |d, note| d.child(div().text_size(px(T_S)).text_color(c(WARN)).child(note)))
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
    let body = if !ids.is_empty() && visible.is_empty() {
        div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("gp.pickNoMatch")).into_any_element()
    } else if !ids.is_empty() {
        let this = cx.weak_entity();
        let rows: Vec<_> = rows.into_iter().cloned().collect();
        let space = (width - 52.).max(0.);
        let columns = grid_fit(space, 220., 8.).0.min(4);
        let tile = ((space - (columns - 1) as f32 * 8.) / columns as f32).floor();
        let count = rows.len().div_ceil(columns);
        let max_rows = ((root.window_h.get() * 0.31) / 56.).floor().clamp(2., 4.) as usize;
        let height = (count.min(max_rows) * 56) as f32;
        let list = uniform_list("gm-pick-accounts", count, move |range, _, cx| {
            range.map(|row| {
                let first = row * columns;
                div().h(px(56.)).w_full().flex().gap(px(8.)).children(
                    (first..(first + columns).min(rows.len())).map(|i| {
                        div().w(px(tile)).flex_none().child(join_account(i, &rows[i], &ticked, this.clone(), cx))
                    }),
                )
            }).collect::<Vec<_>>()
        }).track_scroll(&root.games.join_scroll).h(px(height)).w_full();
        div().id("gm-pick-area").relative()
            .on_scroll(move |_, _, cx| {
                if count > max_rows {
                    cx.stop_propagation();
                }
            })
            .h(px(height)).overflow_hidden().child(list)
            .child(crate::pages::auto::scroll_bar("gm-pick-scroll", &root.games.join_scroll.0.borrow().base_handle))
            .into_any_element()
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
        div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("gp.noneCanJoin")).into_any_element()
    };
    surface().p(px(18.)).flex().flex_col().gap(px(12.)).child(head)
        .when(ids.len() > 8 || !query.is_empty(), |d| {
            d.children(root.games.join_search.as_ref().map(|search| {
                div().max_w(px(320.)).w_full().child(field(search, Some("search")).h(px(34.)))
            }))
        })
        .child(body)
}

fn join_account(i: usize, account: &Value, ticked: &HashSet<String>, root: WeakEntity<Root>, cx: &mut App) -> Hot {
    use crate::pages::accounts::{avatar, is_local, label};
    let id = s(account, "id").to_string();
    let on = ticked.contains(&id);
    let name = label(account);
    let group = s(account, "group");
    let tooltip: SharedString = format!("{name} · @{}", s(account, "username")).into();
    hot(("gm-pick", i))
        .h(px(48.)).w_full().px(px(8.)).rounded(px(R_M)).flex().items_center().gap(px(8.))
        .cursor_pointer()
        .border_1().border_color(if on { ca(FACE, 0.55) } else { c(LINE) })
        .tones(if on { tinted(RAISED, FACE, 0.12) } else { c(INSET) }, c(RAISED_2))
        .child(avatar(account, 28., RAISED, cx))
        .child(div().min_w_0().flex_1().flex().flex_col()
            .child(div().truncate().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).text_color(c(TEXT)).child(name))
            .when(!group.is_empty(), |d| d.child(div().truncate().text_size(px(T_S)).text_color(c(MUTED)).child(group.to_string()))))
        .when(is_local(account), |d| d.child(div().flex_none().child(icon("monitor", 14., c(ONLINE)))))
        .child(div().size(px(16.)).flex_none().rounded(px(5.)).border_1()
            .border_color(if on { c(FACE) } else { c(LINE) }).flex().items_center().justify_center()
            .when(on, |d| d.bg(c(FACE)).child(icon("check", 12., c(INK)))))
        .tooltip(move |_, cx| cx.new(|_| Tip(tooltip.clone())).into())
        .on_click(move |_, _, cx| {
            let _ = root.update(cx, |r, cx| {
                if let Some((_, selection)) = r.games.picking.as_mut() {
                    if !selection.remove(&id) {
                        selection.insert(id.clone());
                    }
                }
                cx.notify();
            });
        })
}

fn tab_button(t: Tab, icon_name: &'static str, label: String, on: bool) -> Hot {
    let id = match t {
        Tab::Overview => "gt-overview",
        Tab::News => "gt-news",
        Tab::Prices => "gt-prices",
        Tab::Passes => "gt-passes",
        Tab::Badges => "gt-badges",
        Tab::Servers => "gt-servers",
        Tab::Wiki => "gt-wiki",
        Tab::Codes => "gt-codes",
    };
    // (The tab shown: a yellow ring on a faint yellow ground, not a solid yellow block: that was Play's colour, and the
    // two pulled at the eye together, the owner's review 2026-11-15.)
    let base = hot(id).h(px(TOOL_H)).px(px(12.)).flex().flex_none().items_center().gap(px(8.)).rounded(px(R_M)).border_1().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).cursor_pointer();
    let base = if on { base.tones(ca(FACE, 0.10), ca(FACE, 0.14)).edge(c(FACE), c(FACE)).text_color(c(TEXT)) } else { base.tones(ca(WHITE, 0.), ca(WHITE, 0.06)).edge(ca(WHITE, 0.), ca(WHITE, 0.)).text_color(c(TEXT_2)) };
    // (The row's own icons: Lucide's, in assets/icons/tab, the owner's pick 2026-11-19 for this row only; the rail and
    // the rest of the app keep Bloxkeep's set.)
    let glyph = svg().path(SharedString::from(format!("icons/tab/{icon_name}.svg"))).size(px(15.)).flex_none().text_color(if on { c(FACE) } else { c(MUTED) });
    base.child(glyph).child(label)
}

/// The open game's name, as a button to any other game (GamesView.tsx's Switcher): a list of them, found by name in a
/// box above 6, the one open ticked; one picked opens. Alone (no other game), the name is only words.
fn switcher(root: &Root, all: &[Game], open: &Open, name: &str, width: f32, cx: &mut Context<Root>) -> AnyElement {
    // The title belongs to the experience, while wiki artwork belongs on its own tab.
    let own = all.iter().find(|g| g.id == open.id).and_then(|g| icon_of(root, g)).or_else(|| crate::pages::home::look(root, open.place, "icon").map(str::to_string));
    let title = div().min_w_0().text_size(px(if width < 900. { 32. } else { 42. })).font_weight(FontWeight::BOLD)
        .line_height(px(50.)).truncate().child(game_name(name.to_string(), own.as_deref(), cx));
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
        // (No "Other games" tip while the list is open: it sat on the list's search box, owner 2026-11-15.)
        .when(!on, |d| d.tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()))
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
        // Roblox's own answer for the words, under yours: any game, not only the ones in the list.
        let typed = switch_words(root, cx);
        let theirs = switch_roblox(root);
        let looking = root.games.switch_found.as_ref().is_some_and(|(_, got)| got.is_none());
        let label = |t: String| div().px(px(10.)).pt(px(10.)).pb(px(4.)).text_size(px(T_S)).font_weight(FontWeight::SEMIBOLD).text_color(c(MUTED)).child(t);
        if !theirs.is_empty() || (looking && typed.chars().count() >= 2) {
            rows = rows.child(label(root.tr("gp.swRoblox")).flex_none());
        }
        for (i, g) in theirs.iter().enumerate() {
            let (place, name) = (g.place, g.name.clone());
            let pic = root.games.icons.get(&g.universe).cloned();
            rows = rows.child(
                menu_row(("gm-sw-rx", i))
                    .flex_none()
                    .child(div().w(px(14.)).flex_none())
                    .child(game_icon(pic, &g.name, 22., cx))
                    .child(div().flex_1().min_w_0().truncate().child(g.name.clone()))
                    .when(g.playing > 0, |d| d.child(div().flex_none().text_size(px(T_S)).text_color(c(MUTED)).font_features(tnum()).child(big(root, g.playing as f64))))
                    .on_click(cx.listener(move |r, _, _, cx| {
                        r.games.switching = false;
                        open_place(r, place, &name);
                        cx.notify();
                    })),
            );
        }
        if theirs.is_empty() && looking && typed.chars().count() >= 2 {
            rows = rows.child(div().flex_none().px(px(10.)).py(px(6.)).text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("gp.swLooking")));
        }
        if shown.is_empty() && theirs.is_empty() && !looking {
            rows = rows.child(div().px(px(10.)).py(px(8.)).text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("gp.swNone")));
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
        wrap = wrap.child(popup(PopAt::BelowLeft(50.), "gm-switch-pop", on, menu));
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
    let cover = crate::pages::home::look(root, place, "cover").map(str::to_string).and_then(|u| images::pic(&u, cx));
    let badge = crate::pages::home::look(root, place, "icon").map(str::to_string)
        .or_else(|| game.and_then(|g| g.icon.clone()))
        .or_else(|| details.map(|d| s(d, "iconUrl").to_string()).filter(|u| !u.is_empty()));
    // While the wide picture isn't there (not loaded yet, or the game has none): the game's own colour from its icon,
    // not a blank dark band.
    let ground = if cover.is_none() { badge.as_deref().and_then(|u| images::tone(u, cx)) } else { None };
    let mut line = div().flex().flex_wrap().items_center().gap(px(8.)).text_size(px(T_M)).text_color(c(TEXT_2));
    if let Some(d) = details {
        let creator = s(d, "creator");
        let genre = s(d, "genre");
        if !creator.is_empty() {
            let group = int(d, "creatorGroupId");
            let author = div().flex().items_center().gap(px(5.)).child(creator.to_string())
                .when(d.get("creatorVerified").and_then(Value::as_bool) == Some(true), |row| {
                    let tip: SharedString = root.tr("gp.creatorVerified").into();
                    row.child(div().id("gm-creator-verified").flex_none().size(px(16.)).flex().items_center().justify_center()
                        .child(div().absolute().size(px(8.)).bg(c(WHITE)))
                        .child(icon("creatorVerified", 16., rgb(0x335fff).into()))
                        .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()))
                });
            line = if group > 0 {
                let tip: SharedString = root.tr("gp.openCommunity").into();
                line.child(hot("gm-creator-community").flex().items_center().gap(px(4.)).tab_index(0)
                    .cursor_pointer().hover(|d| d.text_color(c(TEXT)))
                    .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
                    .on_click(cx.listener(move |r, _, _, cx| open_community(r, group, cx)))
                    .child(author).child(icon("chevronRight", 12., c(MUTED))))
            } else {
                line.child(author)
            };
        }
        if !genre.is_empty() {
            line = line.when(!creator.is_empty(), |d| d.child(sep())).child(genre.to_string());
        }
    }
    // Yours in it, from the Overview's rows (7 days): how many play it now, your time in it, when last.
    let rows: Vec<Value> = if place > 0 {
        root.games.stays.get(&place).cloned().unwrap_or_default()
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
    let mut acts = div().flex().flex_wrap().items_center().gap(px(8.));
    if place > 0 {
        let (id, picking) = (open.id.clone(), root.games.picking.as_ref().is_some_and(|(g, _)| *g == open.id));
        acts = acts.child(Button::new("gm-play", root.tr("gp.play")).icon("play").primary().on_click(cx.listener(move |r, _, _, cx| {
            r.games.picking = if picking { None } else { Some((id.clone(), preselected_join_accounts(r))) };
            r.games.pick_job = None;
            r.games.pick_link = None;
            r.games.pick_place = None;
            r.games.region_note = None;
            // A country picked: the row opens with a server there (asked first when this game's aren't known yet).
            if !picking && let Some(cc) = region_pref() {
                match r.games.regions.get(&place) {
                    // (Still finding the rest, none there yet: waits for them.)
                    Some(Some(Ok(v))) if int(v, "more") > 0 && region_server(r, place, &cc).is_none() => r.games.region_play = Some(place),
                    Some(Some(Ok(_))) => region_apply(r, place, &cc),
                    Some(None) => r.games.region_play = Some(place),
                    _ => {
                        r.games.region_play = Some(place);
                        load_regions(r, place, cx);
                    }
                }
            }
            cx.notify();
        })));
        acts = acts.child(region_button(root, place, cx));
        // Privacy for this game, read from the first account Play would take.
        let pre = preselected_join_accounts(root);
        let first = root
            .accounts()
            .iter()
            .map(|a| s(a, "id").to_string())
            .find(|id| pre.contains(id))
            .or_else(|| root.accounts().iter().find(|a| crate::pages::accounts::can_join_game(a)).map(|a| s(a, "id").to_string()));
        acts = acts.child(crate::pages::game_privacy::button(root, place, first, cx));
    }
    // Your windows in it, closed from here (asked first for two or more, as a lane's Close is).
    if !in_now.is_empty() {
        let n = in_now.len();
        let ids = in_now.clone();
        let game_name = name.to_string();
        acts = acts.child(Button::new("gm-stop", format!("{} · {n}", root.tr("card.close"))).icon("power").quiet().on_click(cx.listener(move |r, _, window, cx| {
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
        let b = Button::new("gm-follow", root.tr(if follow_on { "news.following" } else { "news.follow" })).icon(if follow_on { "check" } else { "bell" });
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
            .min_w_0()
            .items_center()
            .gap(px(8.))
            .child(div().w(px(260.)).min_w_0().child(field(&e, Some("link")).h(px(36.))))
            .child(Button::new("gm-link-join", root.tr("gp.linkJoin")).icon("logIn").on_click(cx.listener(|r, _, _, cx| {
                join_link(r, cx);
                cx.notify();
            })))
            .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
    });

    // The game's wide picture whole, beside its name (owner 2026-11-13: the old band cut a 16:9 picture to a strip and
    // darkened it, "khuất"): no crop, no fade, no words on it; the game's colour stands in until it loads.
    // (Larger than the 136 it was: the picture and the name lead the page, as on Roblox's, the owner's review
    // 2026-11-15: "ảnh game lại nhỏ, trong khi nửa phải phần đầu còn trống".)
    // (The owner, 2026-11-15, again: "giống 1 trang html thuần": the picture leads now, near half the width, the
    // name, Roblox's figures and the buttons fill the other half; on a narrow window the picture goes on top.)
    let wide = width >= 755.;
    // (At the tabs' and the parts' edge: 8 px in from them, the page's left edge was ragged.)
    let room = width - 2. * (PAGE_X);
    // (Narrow, it goes on top at no more than 500 wide: at the full width it was 540 high and pushed the rest down.)
    let pic_w = if wide { (room * 0.45).clamp(315., 720.) } else { room.min(440.) }.round();
    let pic_h = (pic_w * 9. / 16.).round();
    let picture = crate::pages::home::look(root, place, "cover").map(str::to_string).and_then(|u| images::pic_at(&u, pic_w, pic_h, cx));
    let frame = div()
        .w(px(pic_w))
        .h(px(pic_h))
        .flex_none()
        .rounded(px(16.))
        .overflow_hidden()
        .when_some(ground, |d, t| d.bg(ca(t, 0.55)))
        .when(cover.is_none() && ground.is_none(), |d| d.bg(ca(WHITE, 0.04)))
        .when_some(picture, |d, p| d.child(img(p).size_full().rounded(px(R_L)).object_fit(ObjectFit::Cover)));
    let note = details.and_then(|d| fresh(root, d)).map(|n| {
        div().flex().items_start().gap(px(8.))
            .text_size(px(T_S)).line_height(px(18.)).text_color(c(TEXT_2))
            .child(icon("refresh", 14., c(WARN)).mt(px(2.)))
            .child(div().flex_1().min_w_0().child(n))
    });
    let (engagement, activity) = details.map(|d| figures(root, d, place, note.is_none(), cx)).unwrap_or_default();
    let footer = (activity.is_some() || link_row.is_some() || note.is_some()).then(|| {
        div().flex().flex_col().gap(px(10.)).border_t_1().border_color(c(LINE)).pt(px(12.))
            .child(div().flex().flex_wrap().items_center().justify_between().gap(px(14.))
                .children(activity).children(link_row))
            .children(note)
    });
    div()
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(18.))
        .px(px(PAGE_X))
        .pt(px(14.))
        .pb(px(12.))
        .child(
            div().flex().child(Button::new("gm-back", root.tr(match root.games.home_library {
                Some(HomeTab::Games) => "ui.myGames",
                Some(HomeTab::Community) => "ui.community",
                None => "news.title",
            })).icon("chevronLeft").small().on_click(cx.listener(|r, _, _, cx| {
                r.games.open = None;
                cx.notify();
            }))),
        )
        .child(
            div()
                .flex()
                .map(|d| if wide { d.items_center().gap(px(32.)) } else { d.flex_col().gap(px(16.)) })
                .child(frame)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(20.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(14.))
                                .child(game_icon(badge, name, 60., cx))
                                .child(div().flex_1().min_w_0().flex().flex_col().gap(px(3.))
                                    .child(switcher(root, all, open, name, width, cx))
                                    .child(line)),
                        )
                        .child(acts)
                        .children(engagement)
                        .children(mine_line),
                ),
        )
        .children(footer)
}

/// 48.2 billion, 8.7 million, 412,873 (in the app's words for billion and million).
fn big(root: &Root, n: f64) -> String {
    let one = |x: f64| {
        let t = format!("{:.1}", x);
        crate::lang::cur().decimal(t.trim_end_matches(".0"))
    };
    if n >= 1e9 {
        format!("{} {}", one(n / 1e9), root.tr("gp.billion"))
    } else if n >= 1e6 {
        format!("{} {}", one(n / 1e6), root.tr("gp.million"))
    } else {
        thousands(n.round() as i64)
    }
}

fn ago_iso(root: &Root, iso: &str) -> Option<String> {
    crate::time::iso_secs(iso).map(|t| crate::time::ago(crate::time::now_secs() - t, &root.i18n))
}

/// The rating sits by the title. Activity figures are read in the footer beside the server field.
fn figures(root: &Root, details: &Value, place: i64, show_updated: bool, cx: &mut Context<Root>) -> (Option<Div>, Option<Div>) {
    let count = |key: &str| details.get(key).and_then(Value::as_i64).filter(|v| *v >= 0);
    let mut engagement = Vec::new();
    let mut activity = Vec::new();
    // Favourite, Like and Dislike are the account's own, pressed here (they looked like buttons and weren't; the
    // owner, 2026-11-18: "mấy nút này làm cảnh à"): lit when it has, as Roblox's game page lights them.
    let universe = int(details, "universe");
    let mine = root.games.mine.get(&universe).copied().unwrap_or_default();
    let no_vote = mine.no_vote;
    let press = |id: &'static str, chip: Div, tip: String, vote: Option<bool>, cx: &mut Context<Root>| {
        let off = vote.is_some() && no_vote;
        let tip: SharedString = if off { root.tr("gp.votePlayFirst") } else { tip }.into();
        chip.id(id)
            .when(off, |d| d.opacity(0.5))
            .when(!off, |d| d.cursor_pointer().hover(|d| d.bg(ca(WHITE, 0.07))))
            .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
            .on_click(cx.listener(move |r, _, _, cx| set_mine(r, place, universe, vote, cx)))
    };
    if let Some(n) = count("favorites") {
        let on = mine.fav == Some(true);
        let chip = figure_chip(if on { "starFill" } else { "star" }, root.tr("gp.favorites"), thousands(n), on);
        engagement.push(press("gp-fav", chip, root.tr(if on { "gp.unfavorite" } else { "gp.favorite" }), None, cx).into_any_element());
    }
    let votes_up = count("upVotes");
    let votes_down = count("downVotes");
    let liked = details.get("likes").and_then(Value::as_f64).or_else(|| {
        let (up, down) = (votes_up? as f64, votes_down? as f64);
        (up + down > 0.).then(|| 100. * up / (up + down))
    });
    if votes_up.is_some() || votes_down.is_some() || liked.is_some() {
        let rating = div().flex().items_center().gap(px(7.))
            .when_some(votes_up, |d, n| d.child(press("gp-like", figure_chip("like", String::new(), thousands(n), mine.vote == Some(true)), root.tr(if mine.vote == Some(true) { "gp.unlike" } else { "gp.like" }), Some(true), cx)))
            .when_some(liked, |d, pct| d.child(div().px(px(2.)).text_size(px(T_S))
                .font_weight(FontWeight::SEMIBOLD).font_features(tnum()).text_color(c(TEXT_2))
                .child(if votes_up.is_some() || votes_down.is_some() { format!("{pct:.0}%") } else { format!("{pct:.0}% {}", root.tr("gp.likes")) })))
            .when_some(votes_down, |d, n| d.child(press("gp-dislike", figure_chip("dislike", String::new(), thousands(n), mine.vote == Some(false)), root.tr(if mine.vote == Some(false) { "gp.undislike" } else { "gp.dislike" }), Some(false), cx)));
        engagement.push(rating.into_any_element());
    }
    if let Some(n) = count("playing") {
        activity.push(figure_value("users", root.tr("gp.playing"), thousands(n), false));
    }
    if let Some(n) = count("visits") {
        activity.push(figure_value("eye", root.tr("gp.visits"), big(root, n as f64), false));
    }
    if show_updated && let Some(when) = details.get("updated").and_then(Value::as_str).and_then(|iso| ago_iso(root, iso)) {
        activity.push(figure_value("clock", root.tr("gp.updated"), when, false));
    }
    (
        (!engagement.is_empty()).then(|| div().flex().flex_wrap().items_center().gap(px(8.)).children(engagement)),
        (!activity.is_empty()).then(|| div().flex().flex_wrap().items_center().gap_x(px(18.)).gap_y(px(6.)).children(activity)),
    )
}

fn figure_chip(mark: &str, label: String, value: String, accent: bool) -> Div {
    figure_value(mark, label, value, accent).h(px(32.)).px(px(9.))
        .rounded(px(R_M)).border_1()
        .border_color(if accent { ca(WARN, 0.44) } else { c(LINE) })
        .bg(if accent { ca(WARN, 0.08) } else { ca(WHITE, 0.025) })
}

fn figure_value(mark: &str, label: String, value: String, accent: bool) -> Div {
    let tone = if accent { WARN } else { TEXT_2 };
    div().flex_none().flex().items_center().gap(px(6.))
        .text_size(px(T_S)).whitespace_nowrap()
        .child(icon(mark, 14., c(tone)))
        .when(!label.is_empty(), |d| d.child(div().text_color(c(tone)).child(label)))
        .child(div().font_weight(FontWeight::SEMIBOLD).font_features(tnum())
            .text_color(c(if accent { WARN } else { TEXT })).child(value))
}

/// 2019-01-16T03:29:19Z as a day: 16/01/2019, or Jan 16, 2019.
fn day_of(root: &Root, iso: &str) -> Option<String> {
    let (y, m, d) = (iso.get(0..4)?, iso.get(5..7)?.parse::<usize>().ok()?, iso.get(8..10)?);
    if !(1..=12).contains(&m) || y.starts_with("000") {
        return None;
    }
    let (y, d) = (y.parse::<i64>().ok()?, d.parse::<i64>().ok()?);
    let _ = root;
    let l = crate::lang::cur();
    Some(if l.code == "vi" { format!("{d}/{m:02}/{y}") } else { l.date_short(d, m as i64, Some(y)) })
}

/// The game's details, each its word over its value, two to a row (as Roblox's About lists them): who makes it, its genre, when it came
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
    div().flex().flex_col().gap(px(14.)).child(heading(root.tr("gp.info"), None)).child(div().flex().flex_wrap().gap_y(px(16.)).children(rows.into_iter().map(|(k, v)| {
        div()
            .w(relative(0.5))
            .pr(px(12.))
            .flex()
            .flex_col()
            .gap(px(3.))
            .child(div().text_size(px(T_S)).text_color(c(MUTED)).truncate().child(k))
            .child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).font_features(tnum()).truncate().child(v))
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

/// The game's About, as Google Play's "About this game" lays it out (the owner picked it, 2026-11-19, over a plain
/// paragraph that "only has words"): the description's first line as a lead, the rest three lines high with Show more,
/// a row of when it was updated, its genre and players a server, then chips: each code the description gives (click
/// to copy, a thing Roblox's page doesn't do) and Roblox's genre and subgenre.
fn about_block(root: &Root, d: &Value, place: i64, about: &str, cx: &mut Context<Root>) -> Div {
    let open = root.games.about;
    let (lead, rest) = about_lead(about);
    // (Folded at three lines when longer; cut by hand: a clamp counts wrapped lines, not the description's own breaks.)
    // (Blank lines don't count: folded, the three lines are three lines of words.)
    let filled: Vec<&str> = rest.lines().map(str::trim_end).filter(|l| !l.trim().is_empty()).collect();
    let long = filled.len() > 3 || rest.chars().count() > 300;
    let shown = if long && !open {
        let head: String = filled.iter().take(3).copied().collect::<Vec<_>>().join("\n").chars().take(300).collect();
        format!("{}…", head.trim_end())
    } else {
        rest.clone()
    };
    let mut meta: Vec<(String, String)> = Vec::new();
    if let Some(day) = d.get("updated").and_then(Value::as_str).and_then(|t| day_of(root, t)) {
        meta.push((root.tr("gp.updatedOn"), day));
    }
    let genres: Vec<String> = d.get("genres").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect();
    // (With Roblox's genre chips below, the genre isn't said twice.)
    let genre = Some(s(d, "genre").to_string()).filter(|g| genres.is_empty() && !g.is_empty() && g != "All");
    if let Some(g) = genre {
        meta.push((root.tr("gp.genre"), g));
    }
    // (Players a server is in Details beside it: not said twice.)
    // (A code the Codes block above lists already isn't shown twice.)
    let listed: Vec<String> = root
        .games
        .codes
        .get(&place)
        .and_then(Option::as_ref)
        .and_then(|v| v.get("codes"))
        .and_then(Value::as_array)
        .map(|l| l.iter().map(|c| s(c, "code").to_lowercase()).collect())
        .unwrap_or_default();
    let codes: Vec<String> = about_codes(about).into_iter().filter(|c| !listed.contains(&c.to_lowercase())).collect();
    let mut chips = div().flex().flex_wrap().gap(px(8.));
    for (i, code) in codes.iter().enumerate() {
        chips = chips.child(code_chip(root, ("gp-code", i), code, true, cx));
    }
    for g in &genres {
        chips = chips.child(div().h(px(32.)).px(px(14.)).flex().items_center().rounded_full().border_1().border_color(ca(WHITE, 0.16)).text_size(px(T_S + 0.5)).text_color(c(TEXT_2)).child(g.clone()));
    }
    let any_chip = !codes.is_empty() || !genres.is_empty();
    div()
        .flex()
        .flex_col()
        .child(heading(root.tr("gp.aboutTitle"), None))
        .when_some(lead, |dv, lead| dv.child(div().mt(px(14.)).max_w(px(780.)).text_size(px(T_M + 1.)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(lead)))
        .when(!shown.is_empty(), |dv| dv.child(div().mt(px(6.)).max_w(px(780.)).text_size(px(T_M)).line_height(px(22.)).text_color(c(TEXT_2)).child(shown)))
        .when(long, |dv| {
            dv.child(
                // Show more as words in the accent, level with the text (Google Play's and the App Store's "more").
                div().mt(px(6.)).flex().child(
                    div()
                        .id("gp-about")
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .cursor_pointer()
                        .text_size(px(T_M))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(FACE))
                        .hover(|d| d.opacity(0.8))
                        .child(root.tr(if open { "gp.less" } else { "gp.more" }))
                        .child(icon(if open { "chevronUp" } else { "chevronDown" }, 14., c(FACE)))
                        .on_click(cx.listener(|r, _, _, cx| {
                            r.games.about = !r.games.about;
                            cx.notify();
                        })),
                ),
            )
        })
        .when(!meta.is_empty(), |dv| {
            let mut row = div().mt(px(18.)).flex().flex_wrap().gap_x(px(34.)).gap_y(px(10.));
            for (k, v) in meta {
                row = row.child(div().flex().flex_col().gap(px(3.)).child(div().text_size(px(T_S)).text_color(c(MUTED)).child(k)).child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(v)));
            }
            dv.child(row)
        })
        .when(any_chip, |dv| dv.child(chips.mt(px(14.))))
}

/// The description's lead: its first line when that is short and more follows (as Google Play shows a store text's
/// first line apart), else its first sentence when short; and the rest.
fn about_lead(about: &str) -> (Option<String>, String) {
    let about = about.trim();
    let (first, after) = about.split_once('\n').map_or((about, ""), |(a, b)| (a.trim(), b.trim()));
    if !after.is_empty() && !first.is_empty() && first.chars().count() <= 140 {
        return (Some(first.to_string()), after.to_string());
    }
    // (A sentence ends at ". ", "! " or "? ".)
    let mut end = None;
    for (i, ch) in first.char_indices() {
        if matches!(ch, '.' | '!' | '?') && first[i + ch.len_utf8()..].starts_with(' ') {
            end = Some(i + ch.len_utf8());
            break;
        }
    }
    match end {
        Some(i) if first[..i].chars().count() <= 140 => {
            let rest = format!("{}{}{}", first[i..].trim(), if after.is_empty() { "" } else { "\n" }, after);
            (Some(first[..i].to_string()), rest.trim().to_string())
        }
        _ => (None, about.to_string()),
    }
}

/// The codes a game's description gives ("Use this code: FirstCodeEver", "CODES: GEMS100, LUCKY"): the word or words
/// after "code"/"codes", when they look like a code (a colon before them, or a digit or capital in them), at most six.
fn about_codes(about: &str) -> Vec<String> {
    const NOT: [&str; 22] = ["to", "for", "in", "and", "the", "is", "are", "of", "on", "at", "with", "from", "a", "an", "or", "will", "can", "you", "your", "here", "below", "redeem"];
    let mut out: Vec<String> = Vec::new();
    let lower = about.to_lowercase();
    let bytes = about.as_bytes();
    let mut at = 0;
    while let Some(i) = lower[at..].find("code") {
        let start = at + i;
        at = start + 4;
        // A word of its own ("code" or "codes"), not "barcode" or "coded".
        if start > 0 && (bytes[start - 1] as char).is_ascii_alphanumeric() {
            continue;
        }
        let mut j = at;
        let many = lower[j..].starts_with('s');
        if many {
            j += 1;
        }
        if lower[j..].chars().next().is_some_and(|ch| ch.is_alphanumeric()) {
            continue;
        }
        let tail = &about[j..];
        let skipped = tail.trim_start_matches([' ', '\t']);
        let colon = skipped.starts_with([':', '-', '–', '=']);
        let mut rest = skipped.trim_start_matches([':', '-', '–', '=', ' ', '\t', '"', '\'', '“']);
        loop {
            let word: String = rest.chars().take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_').collect();
            let n = word.chars().count();
            let looks = (3..=30).contains(&n)
                && !NOT.contains(&word.to_lowercase().as_str())
                && (colon || word.chars().skip(1).any(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit()));
            if !looks {
                break;
            }
            if !out.contains(&word) {
                out.push(word.clone());
            }
            if !many || out.len() >= 6 {
                break;
            }
            rest = rest[word.len()..].trim_start_matches(['"', '\'', '”']);
            let next = rest.trim_start_matches([',', ';', '/', '|', ' ']).trim_start();
            if next.len() == rest.len() {
                break;
            }
            rest = next;
        }
        if out.len() >= 6 {
            break;
        }
    }
    out
}

/// A block's head in the fuller look: its mark, its name, and what the name leaves unsaid as a quiet pill at the right.
fn heading_rich(_mark: &'static str, _tone: u32, title: String, sub: Option<String>) -> Div {
    heading(title, sub)
}

/// A part's head on the game page: its name, large, and its note in quiet words after it (under it when long).
/// No icon square and no pill: every block had the same square, name and pill, the owner 2026-11-15: "bố cục trông
/// nó như nhau, dùng đi dụng lại 1 khung".
fn heading(title: String, sub: Option<String>) -> Div {
    // A long note goes under the title, wrapping in its column ("When it is quiet" in its narrow column ran off the
    // edge on one line).
    let under = sub.as_ref().is_some_and(|s| s.chars().count() > 36);
    div()
        .flex()
        .map(|d| if under { d.flex_col().gap(px(3.)) } else { d.items_baseline().gap(px(12.)) })
        .child(div().text_size(px(19.)).font_weight(FontWeight::BOLD).line_height(px(24.)).child(title))
        .when_some(sub, |d, s| d.child(div().min_w_0().text_size(px(T_S)).text_color(c(MUTED)).child(s)))
}

/// The social links Roblox returned for the viewer of this experience.
fn social_media(root: &Root, universe: i64) -> Option<Div> {
    let viewer = social_viewer(root)?;
    let social = root.games.social.as_ref().filter(|social| {
        social.viewer == viewer && social.universe == universe && !social.links.is_empty()
    })?;
    let buttons = social.links.iter().enumerate().map(|(i, link)| {
        let (label, mark) = match s(link, "type") {
            "Twitter" | "X" => ("X", "x"),
            "YouTube" | "Youtube" => ("YouTube", "play"),
            "Discord" => ("Discord", "discord"),
            "Facebook" => ("Facebook", "external"),
            "Twitch" => ("Twitch", "external"),
            _ => (s(link, "title"), "external"),
        };
        let url = s(link, "url").to_string();
        Button::new(("gp-social", i), if label.is_empty() { root.tr("gp.openRobloxLink") } else { label.to_string() })
            .icon(mark).small().tip(s(link, "title").to_string())
            .on_click(move |_, _, cx| cx.open_url(&url))
    });
    Some(div().flex().flex_col().gap(px(12.))
        .child(heading(root.tr("gp.socialMedia"), None))
        .child(div().flex().flex_wrap().gap(px(8.)).children(buttons)))
}

/// Your accounts in it over 7 days, the hours it is quiet, and what it is.
fn overview(root: &Root, details: Option<&Value>, place: i64, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let accounts = root.games.stays.get(&place);
    if accounts.is_none() && details.is_none() {
        let key = if place > 0 { "news.connecting" } else { "gp.noneYet" };
        return div().pt(px(8.)).text_size(px(T_M)).text_color(c(MUTED)).child(root.tr(key));
    }
    let accounts = accounts.map(Vec::as_slice).unwrap_or_default();
    let d = details.unwrap_or(&Value::Null);
    let narrow = width < 760.;
    // Each account its own row in the block: a faint ground of its own, its face, its name with how it is now under
    // it, then its week's figures each with its word under it. No column heads or lines between rows ("như exel",
    // 2026-11-15), and not loose on the page either: without its block the rows floated, "sát rạt trông loạn".
    let mut table = div().flex().flex_col().gap(px(8.));
    let fig = |value: String, word: String| {
        div()
            .w(px(if narrow { 74. } else { 112. }))
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(1.))
            .child(div().text_size(px(15.)).font_weight(FontWeight::BOLD).font_features(tnum()).truncate().child(value))
            .child(div().text_size(px(T_S)).text_color(c(MUTED)).truncate().child(word))
    };
    for a in accounts {
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
            "ingame" => div().flex().child(tint_pill(root.tr("gp.inGame"), OK)),
            "banned" => div().flex().child(tint_pill(root.tr("gp.banned"), BAD)),
            _ => div().text_size(px(T_S)).text_color(c(MUTED)).truncate().child(
                a.get("lastMs")
                    .and_then(Value::as_i64)
                    .map(|ms| crate::time::ago(crate::time::now_secs() - ms / 1000, &root.i18n))
                    .or_else(|| a.get("lastPlayed").and_then(Value::as_str).and_then(|t| ago_iso(root, t)))
                    .map(|t| root.i18n.tv("gp.mineLast", &[("t", &t)]))
                    .unwrap_or_else(|| "–".into()),
            ),
        };
        // (Its face, when the account is still in the app.)
        let face = root.accounts().iter().find(|x| s(x, "id") == s(a, "id")).map(|x| crate::pages::accounts::avatar(x, 40., RAISED, cx));
        table = table.child(
            div()
                .h(px(66.))
                .px(px(14.))
                .rounded(px(R_M + 2.))
                .bg(ca(WHITE, 0.035))
                .flex()
                .items_center()
                .gap(px(14.))
                .text_size(px(T_M))
                .children(face)
                .child(div().flex_1().min_w_0().flex().flex_col().gap(px(4.)).child(div().min_w_0().truncate().font_weight(FontWeight::SEMIBOLD).child(s(a, "name").to_string())).child(now))
                .child(fig(played, root.tr("gp.played")))
                .child(fig(kicks, root.tr("gp.kicks")))
                .child(fig(int(a, "drops").to_string(), root.tr("gp.drops"))),
        );
    }

    let hours: Option<Vec<Option<f64>>> = d
        .get("hours")
        .and_then(Value::as_array)
        .map(|l| l.iter().map(Value::as_f64).collect::<Vec<_>>())
        .filter(|l| l.len() == 24 && l.iter().filter(|h| h.is_some()).count() >= 12);
    let about = s(d, "description").trim().to_string();
    let words = (!about.is_empty()).then(|| about_block(root, d, place, &about, cx));

    let yours_head = if rich() { heading_rich("users", ONLINE, root.tr("gp.yours"), Some(root.tr("gp.week"))).into_any_element() } else { heading(root.tr("gp.yours"), Some(root.tr("gp.week"))).into_any_element() };
    // (In its block again, the owner 2026-11-15: "sao mày bỏ ô của cái tài khoản đi? làm lại ô đẹp hơn".)
    let yours = surface().p(px(20.)).flex().flex_col().gap(px(14.)).child(yours_head).child(if accounts.is_empty() {
        div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("gp.noneYet"))
    } else {
        table
    });
    let hush_head = if rich() { heading_rich("moon", FACE, root.tr("gp.quiet"), Some(root.tr("gp.quietFrom"))).into_any_element() } else { heading(root.tr("gp.quiet"), Some(root.tr("gp.quietFrom"))).into_any_element() };
    let hush = div().flex().flex_col().gap(px(16.)).child(hush_head).child(match hours {
        Some(h) => quiet(root, &h).into_any_element(),
        None => div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("gp.quietNotYet")).into_any_element(),
    });
    // (Its details and quiet hours one panel beside the open main column: the one block on the page.)
    let side_panel = surface().p(px(20.)).flex().flex_col().gap(px(30.)).child(info(root, d, place)).child(hush);
    // Wide: the pictures, the words and your accounts in the main column; its details and quiet hours beside it.
    let beside = width >= 1100.;
    let side = 380.;
    let main_w = if beside { width - side - GAP } else { width };
    // Your accounts in it first, under the alert: what this page is mostly for (C#'s Game info), it was two scrolls
    // down under the pictures (review 3b.1).
    // (The parts apart by room, not boxes.)
    let main = div()
        .flex()
        .flex_col()
        .gap(px(36.))
        .children(radar_strip(root, place, s(d, "name")))
        .child(yours)
        .children(events_block(root, place, main_w, cx))
        .children(gallery(root, place, main_w, cx))
        .children(words)
        .children(social_media(root, int(d, "universe")));
    div()
        .flex()
        .flex_col()
        .gap(px(GAP))
        .pt(px(4.))
        .child(if details.is_none() {
            main.child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("news.connecting")))
        } else if beside {
            div()
                .flex()
                .items_start()
                .gap(px(GAP))
                .child(main.w(px(main_w)).flex_none())
                .child(div().w(px(side)).flex_none().child(side_panel))
        } else {
            main.child(side_panel)
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
                .child(thousands(v.round() as i64));
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
        // (Ten hours apart or more they share one line; closer, each has its own line: under 10 hours apart in a
        // narrow column, "quiet 5,387" at 17h and "busy 8,879" at 22h were drawn over each other, the owner 2026-11-22.)
        let row = || div().relative().h(px(18.));
        let quiet_mark = mark(l, "gp.quietEnd", c(OK));
        let busy_mark = mark(hi, "gp.busyEnd", c(TEXT));
        let marks = div().flex().flex_col().text_size(px(T_S)).text_color(c(TEXT_2)).font_features(tnum());
        if l.0.abs_diff(hi.0) >= 10 {
            marks.child(row().child(quiet_mark).child(busy_mark))
        } else {
            marks.child(row().child(quiet_mark)).child(row().child(busy_mark))
        }
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
    let before = root.games.mark.as_ref().filter(|(g, _, _)| *g == id).map(|(_, r, _)| *r).unwrap_or(f64::INFINITY);
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
