// The inventory, inside Games (its header's switch): what the accounts have, a shelf at a time.
//
// Private servers come first and are every account's together: Roblox shows them one account at a time (Inventory ›
// Private Servers); the app asks each account with its own sign-in ("ownedServers") and puts the answers together: what
// they cost a month in all, which renews next, and under each game its servers with the account that owns each. A
// game opens its Servers tab, where its server is joined. Renewing and cancelling stay on Roblox.
//
// The other shelves are the account's whose tab is shown, as Roblox's inventory has them: accessories, clothing, body
// parts, animations, emotes (the avatar editor's own groups, roblox::SHELVES) and badges, fifty at a time, newest
// first. A thing opens its page on Roblox.
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use gpui_kit::{prelude::*, *};
use serde_json::{json, Value};
use crate::val::s;
use crate::val::{flag, int};
use crate::ask::reply;

use crate::{
    kit::*,
    pages::games,
    roblox,
    shell::Root,
    theme::*,
};

/// How old the list may get while it is shown.
const FRESH: Duration = Duration::from_secs(60);

#[derive(Default)]
pub struct Data {
    /// What the app answered ({ servers, asked, failed }; None while the first answer is on its way), when, and
    /// whether an asking is under way.
    got: Option<Result<Value, String>>,
    at: Option<Instant>,
    loading: bool,
    /// The ended ones are shown (else the running ones).
    ended: bool,
    /// The games' pictures by universe, and the universes asked for already.
    icons: HashMap<i64, String>,
    asked: Vec<i64>,
    /// The shelf shown: 0 the private servers, then roblox::SHELVES in their order, then the badges.
    shelf: usize,
    /// The things on the shelves read so far, by (the Roblox user, the shelf).
    things: HashMap<(i64, usize), Shelf>,
}

/// A shelf of one account's: its things so far, the mark for the next page (empty: no more), whether a page is on its
/// way, and why the last one failed.
#[derive(Default)]
struct Shelf {
    list: Vec<roblox::Owned>,
    next: String,
    reading: bool,
    failed: Option<String>,
    asked: bool,
}

/// The shelves after the private servers, as Roblox's inventory has them: the avatar's (roblox::SHELVES), badges,
/// bundles, game passes, then roblox::LIBRARY's (classic faces and heads, audio, decals, models, plugins, meshes,
/// videos). Each a name's key and where it is read from.
#[derive(Clone, Copy, PartialEq)]
enum From {
    Kinds(&'static str),
    /// A creator's things (roblox::LIBRARY): read the same way, opened on Roblox's store.
    Library(&'static str),
    Badges,
    Bundles,
    Passes,
}

fn shelves() -> Vec<(&'static str, From)> {
    let avatar = ["inv.acc", "inv.clothes", "inv.body", "inv.anim", "inv.emote"].into_iter().zip(roblox::SHELVES).map(|(k, (_, kinds))| (k, From::Kinds(kinds)));
    let rest = [("inv.badges", From::Badges), ("inv.bundles", From::Bundles), ("inv.passes", From::Passes)];
    avatar.chain(rest).chain(roblox::LIBRARY.into_iter().map(|(k, kinds)| (k, if k == "inv.faces" { From::Kinds(kinds) } else { From::Library(kinds) }))).collect()
}

/// Where a shelf's thing opens on Roblox (openRobloxPage's kind).
fn page_kind(shelf: usize) -> &'static str {
    match shelves().get(shelf.wrapping_sub(1)).map(|s| s.1) {
        Some(From::Badges) => "badge",
        Some(From::Bundles) => "bundle",
        Some(From::Passes) => "pass",
        Some(From::Library(_)) => "library",
        _ => "catalog",
    }
}

pub fn want(r: &mut Root, link: &crate::link::Link, cx: &mut Context<Root>) {
    // BLOXKEEP_SHOT_OWNED's second word picks the shelf of a hidden shot ("file.json,2").
    if r.owned.shelf == 0 {
        if r.owned.loading || r.owned.at.is_some_and(|t| t.elapsed() < FRESH) {
            return;
        }
        load(r, false, cx);
        return;
    }
    crate::pages::friends::want_sign(r, link, cx);
    let Some((_, user, _)) = crate::pages::friends::signed(r) else { return };
    let key = (user, r.owned.shelf);
    if !r.owned.things.get(&key).is_some_and(|s| s.asked) {
        shelf_page(r, cx);
    }
}

/// The shown shelf's next page (its first, when nothing of it was asked yet) for the account whose tab is shown.
fn shelf_page(r: &mut Root, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = crate::pages::friends::signed(r) else { return };
    let shelf = r.owned.shelf;
    if shelf == 0 {
        return;
    }
    let key = (user, shelf);
    let have = r.owned.things.entry(key).or_default();
    if have.reading {
        return;
    }
    let cursor = have.next.clone();
    have.reading = true;
    have.asked = true;
    have.failed = None;
    let Some((_, from)) = shelves().get(shelf - 1).copied() else { return };
    let rx = roblox::run(move || match from {
        From::Kinds(kinds) | From::Library(kinds) => roblox::inventory(&cookie, user, kinds, &cursor),
        From::Badges => roblox::badges_page(&cookie, user, &cursor),
        From::Bundles => roblox::bundles_page(&cookie, user, &cursor),
        From::Passes => roblox::passes_page(&cookie, user, &cursor),
    });
    reply(rx, cx, move |r, got, cx| {
        let Some(have) = r.owned.things.get_mut(&key) else { return };
        have.reading = false;
        match got {
            Ok((page, next)) => {
                have.list.extend(page);
                have.next = next;
            }
            Err(why) => have.failed = Some(if why.is_empty() { "Roblox".into() } else { why }),
        }
        cx.notify();
    });
}

fn load(r: &mut Root, fresh: bool, cx: &mut Context<Root>) {
    // BLOXKEEP_SHOT_OWNED=file.json: a hidden shot's answer, since only signed-in accounts get one.
    #[cfg(feature = "shot")]
    if let Ok(path) = std::env::var("BLOXKEEP_SHOT_OWNED") {
        r.owned.got = std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).map(Ok);
        r.owned.at = Some(Instant::now());
        pictures(r, cx);
        return;
    }
    let Some(link) = r.link.clone() else { return };
    r.owned.loading = true;
    let rx = link.call("ownedServers", json!({ "fresh": fresh }));
    cx.spawn(async move |this, cx| {
        let got = match rx.await {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err(e.to_string()),
            Err(_) => Err(String::new()),
        };
        let _ = this.update(cx, |r, cx| {
            r.owned.loading = false;
            r.owned.at = Some(Instant::now());
            // A later asking that fails leaves what is shown.
            if got.is_ok() || !matches!(r.owned.got, Some(Ok(_))) {
                r.owned.got = Some(got);
            }
            pictures(r, cx);
            cx.notify();
        });
    })
    .detach();
}

/// The pictures of the games not asked for yet.
fn pictures(r: &mut Root, cx: &mut Context<Root>) {
    let Some(Ok(v)) = &r.owned.got else { return };
    let mut ids: Vec<i64> = v.get("servers").and_then(Value::as_array).into_iter().flatten().map(|x| int(x, "universeId")).filter(|u| *u > 0 && !r.owned.asked.contains(u)).collect();
    ids.sort_unstable();
    ids.dedup();
    if ids.is_empty() {
        return;
    }
    r.owned.asked.extend(&ids);
    let rx = roblox::run(move || roblox::icons(&ids));
    cx.spawn(async move |this, cx| {
        let Ok(got) = rx.await else { return };
        let _ = this.update(cx, |r, cx| {
            r.owned.icons.extend(got);
            cx.notify();
        });
    })
    .detach();
}

/// 2026-12-08T… as "8 Dec" ("8/12" in Vietnamese); with the year when it isn't this one.
fn day(iso: &str, vi: bool) -> String {
    let d: Vec<i64> = iso.get(..10).unwrap_or_default().split('-').filter_map(|p| p.parse().ok()).collect();
    let [y, m, dd] = d[..] else { return String::new() };
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let this_year = crate::time::iso(crate::time::now_secs()).get(..4).and_then(|t| t.parse::<i64>().ok()) == Some(y);
    let month = MONTHS.get((m - 1).clamp(0, 11) as usize).copied().unwrap_or_default();
    match (vi, this_year) {
        (true, true) => format!("{dd}/{m}"),
        (true, false) => format!("{dd}/{m}/{y}"),
        (false, true) => format!("{dd} {month}"),
        (false, false) => format!("{dd} {month} {y}"),
    }
}

/// Days from today to that date (negative: past); None when Roblox gave none.
fn days_to(iso: &str) -> Option<i64> {
    Some((crate::time::iso_secs(iso)? - crate::time::now_secs()).div_euclid(86_400))
}

/// A figure of the summary: its name small over its value.
fn figure(name: String, value: impl IntoElement) -> Div {
    div().flex().flex_col().gap(px(4.)).child(div().text_size(px(T_S)).text_color(c(MUTED)).child(name)).child(div().flex().items_center().gap(px(6.)).text_size(px(T_SEC)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(value))
}

fn note(text: String, looking: bool) -> Div {
    div().w_full().py(px(48.)).flex().items_center().justify_center().gap(px(10.)).text_size(px(T_M)).text_color(c(MUTED)).when(looking, |d| d.child(spinner(14., c(MUTED)))).child(text)
}

pub fn page(root: &Root, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let o = &root.owned;
    let inner = (width - 48.).max(320.);
    let top = div().flex_none().px(px(24.)).pt(px(22.)).pb(px(16.)).flex().items_center().gap(px(10.)).child(games::brand(root)).relative().child(games::island(root, cx)).child(div().flex_1());
    let reading = if o.shelf == 0 {
        o.loading
    } else {
        crate::pages::friends::signed(root).is_some_and(|(_, user, _)| o.things.get(&(user, o.shelf)).is_some_and(|s| s.reading))
    };
    let refresh = IconButton::new("own-refresh", "refresh").size(34.).tip(root.tr("own.refresh")).disabled(reading).on_click(cx.listener(|r, _, _, cx| {
        if r.owned.shelf == 0 {
            load(r, true, cx);
        } else if let Some((_, user, _)) = crate::pages::friends::signed(r) {
            r.owned.things.remove(&(user, r.owned.shelf));
            shelf_page(r, cx);
        }
        cx.notify();
    }));
    let head = div()
        .flex()
        .items_center()
        .gap(px(12.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(div().text_size(px(T_FIG)).font_weight(FontWeight::SEMIBOLD).child(root.tr("inv.title")))
                .child(div().text_size(px(T_M)).text_color(c(MUTED)).truncate().child(root.tr(if o.shelf == 0 { "own.sub" } else { "inv.sub" }))),
        )
        .child(refresh);
    // The shelves: the private servers (every account's), then the things of the account whose tab is shown.
    let shelves = div().flex().flex_wrap().gap(px(6.)).children(std::iter::once("own.title").chain(shelves().into_iter().map(|s| s.0)).enumerate().map(|(i, key)| {
        chip(("inv-shelf", i), o.shelf == i).child(root.tr(key)).on_click(cx.listener(move |r, _, _, cx| {
            r.owned.shelf = i;
            cx.notify();
        }))
    }));
    let body = if o.shelf == 0 { servers(root, inner, cx) } else { things(root, inner, cx) };
    let content = div().w_full().px(px(24.)).pb(px(28.)).flex().flex_col().gap(px(18.)).child(head).child(shelves).child(body);
    div().id("owned").relative().size_full().rounded(px(R_PANEL)).bg(c(PANEL)).overflow_hidden().flex().flex_col().child(top).child(crate::scrolled::body("owned-body", content))
}

/// A shelf of the account whose tab is shown: its things as tiles, and More while Roblox has another page.
fn things(root: &Root, inner: f32, cx: &mut Context<Root>) -> AnyElement {
    let o = &root.owned;
    let Some((_, user, _)) = crate::pages::friends::signed(root) else {
        return if crate::pages::friends::nobody(root) {
            div().h(px(360.)).flex().flex_col().child(crate::pages::friends::need_account(root, "inv-add-acc", "inv.noAccount", "bookmark", cx)).into_any_element()
        } else {
            note(root.tr("inv.reading"), true).into_any_element()
        };
    };
    let Some(have) = o.things.get(&(user, o.shelf)) else { return note(root.tr("inv.reading"), true).into_any_element() };
    if have.list.is_empty() {
        return match (&have.failed, have.reading) {
            (Some(why), _) => note(root.i18n.tv("own.failed", &[("why", why)]), false).into_any_element(),
            (None, true) => note(root.tr("inv.reading"), true).into_any_element(),
            (None, false) => note(root.tr("inv.empty"), false).into_any_element(),
        };
    }
    let kind = page_kind(o.shelf);
    let (_, w) = grid_fit(inner, 124., GRID_GAP);
    let side = w - 18.;
    let tiles: Vec<AnyElement> = have
        .list
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let pic = (!t.pic.is_empty()).then(|| crate::images::pic_pair(&t.pic, side, side, cx)).flatten();
            let id = t.id;
            hot(("inv-thing", i))
                .w(px(w))
                .flex()
                .flex_col()
                .gap(px(6.))
                .cursor_pointer()
                .child(div().w(px(w)).h(px(w)).rounded(px(R_L)).bg(c(RAISED)).border_1().border_color(c(LINE)).flex().items_center().justify_center().map(|d| match pic {
                    Some((src, key)) => d.child(fade_in(img(src).size(px(side)).object_fit(ObjectFit::Contain), &key)),
                    None => d.child(icon("image", 20., c(DIM))),
                }))
                .child(div().text_size(px(T_S)).line_height(px(16.)).text_color(c(TEXT_2)).line_clamp(2).child(t.name.clone()))
                .on_click(cx.listener(move |r, _, _, _| r.send("openRobloxPage", json!({ "kind": kind, "id": id }))))
                .into_any_element()
        })
        .collect();
    let more = (!have.next.is_empty()).then(|| {
        div().flex().justify_center().child(Button::new("inv-more", root.tr(if have.reading { "inv.reading" } else { "mods.more" })).disabled(have.reading).on_click(cx.listener(|r, _, _, cx| {
            shelf_page(r, cx);
            cx.notify();
        })))
    });
    div()
        .flex()
        .flex_col()
        .gap(px(18.))
        .child(grid(GRID_GAP).children(tiles))
        .children(have.failed.as_ref().map(|why| div().text_size(px(T_S)).text_color(c(WARN)).child(root.i18n.tv("own.failed", &[("why", why)]))))
        .children(more)
        .child(div().text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("inv.onRoblox")))
        .into_any_element()
}

/// The private servers of every account: what they come to, then each game with its servers.
fn servers(root: &Root, inner: f32, cx: &mut Context<Root>) -> AnyElement {
    let vi = root.i18n.vietnamese;
    let o = &root.owned;
    let all: Vec<Value> = match &o.got {
        Some(Ok(v)) => v.get("servers").and_then(Value::as_array).cloned().unwrap_or_default(),
        _ => Vec::new(),
    };
    let (asked, failed) = match &o.got {
        Some(Ok(v)) => (int(v, "asked"), int(v, "failed")),
        _ => (0, 0),
    };
    let running: Vec<&Value> = all.iter().filter(|x| flag(x, "active")).collect();
    let ended: Vec<&Value> = all.iter().filter(|x| !flag(x, "active")).collect();
    let shown: &Vec<&Value> = if o.ended { &ended } else { &running };
    let half = |id: &'static str, key: &str, count: usize, which: bool| {
        segment(id, o.ended == which).h(px(28.)).px(px(12.)).gap(px(7.)).child(root.tr(key)).child(div().font_features(tnum()).opacity(0.6).child(count.to_string())).on_click(cx.listener(move |r, _, _, cx| {
            r.owned.ended = which;
            cx.notify();
        }))
    };
    let halves = (!all.is_empty()).then(|| div().flex().flex_none().p(px(3.)).gap(px(2.)).rounded(px(10.)).bg(c(INSET)).border_1().border_color(c(LINE)).child(half("own-on", "own.running", running.len(), false)).child(half("own-off", "own.ended", ended.len(), true)));
    // What they come to: how many renew, what that costs a month, and the next one due.
    let renewing: Vec<&&Value> = running.iter().filter(|x| flag(x, "renews") && int(x, "price") > 0).collect();
    let monthly: i64 = renewing.iter().map(|x| int(x, "price")).sum();
    let next = renewing.iter().filter(|x| !s(x, "expires").is_empty()).min_by_key(|x| s(x, "expires").to_string());
    let soon = running.iter().filter(|x| !flag(x, "renews") && days_to(s(x, "expires")).is_some_and(|d| (0..=7).contains(&d))).count();
    let summary = if o.ended {
        Some(figure(root.tr("own.ended"), div().font_features(tnum()).child(thousands(ended.len() as i64, vi))))
    } else if running.is_empty() {
        None
    } else {
        Some(div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_x(px(40.))
            .gap_y(px(14.))
            .child(figure(root.tr("own.f.servers"), {
                let mut line = div().flex().items_center().gap(px(8.)).child(root.i18n.tn("own.nRunning", running.len() as i64, &[]));
                if !renewing.is_empty() {
                    line = line.child(div().text_color(c(MUTED)).child("·")).child(root.i18n.tn("own.nRenew", renewing.len() as i64, &[]));
                }
                if soon > 0 {
                    line = line.child(div().text_color(c(MUTED)).child("·")).child(div().text_color(c(WARN)).child(root.i18n.tn("own.nSoon", soon as i64, &[])));
                }
                line
            }))
            .child(figure(root.tr("own.f.month"), div().flex().items_center().gap(px(5.)).font_features(tnum()).child(icon("robux", 15., c(TEXT))).child(thousands(monthly, vi))))
            .when_some(next, |d, x| d.child(figure(root.tr("own.f.next"), div().flex().items_center().gap(px(8.)).child(day(s(x, "expires"), vi)).child(div().text_color(c(MUTED)).font_weight(FontWeight::NORMAL).child(s(x, "game").to_string()))))))
    };
    let bar = (!all.is_empty()).then(|| surface().px(px(16.)).py(px(12.)).flex().flex_wrap().items_center().justify_between().gap(px(12.))
        .children(summary).children(halves));

    let body: AnyElement = match &o.got {
        None => note(root.tr("own.reading"), true).into_any_element(),
        Some(Err(why)) => note(root.i18n.tv("own.failed", &[("why", crate::net::why_words(&root.i18n, why).as_str())]), false).into_any_element(),
        Some(Ok(_)) if asked == 0 => note(root.tr("own.noAccount"), false).into_any_element(),
        Some(Ok(_)) if all.is_empty() => note(root.tr("own.none"), false).into_any_element(),
        Some(Ok(_)) if shown.is_empty() => note(root.tr(if o.ended { "own.noneEnded" } else { "own.noneRunning" }), false).into_any_element(),
        Some(Ok(_)) => {
            // Under each game, its servers (the list comes sorted by game).
            let mut groups: Vec<(i64, Vec<&Value>)> = Vec::new();
            for x in shown.iter().copied() {
                let u = int(x, "universeId");
                match groups.last_mut() {
                    Some((last, rows)) if *last == u => rows.push(x),
                    _ => groups.push((u, vec![x])),
                }
            }
            let (cols, _) = grid_fit(inner, 370., GRID_GAP);
            let cols = cols.min(3).min(groups.len()).max(1);
            let card_w = ((inner - GRID_GAP * (cols - 1) as f32) / cols as f32).floor();
            let mut i = 0;
            let mut columns: Vec<(usize, Vec<AnyElement>)> = (0..cols).map(|_| (0, Vec::new())).collect();
            for (universe, rows) in groups {
                    let first = rows[0];
                    let (place, game) = (int(first, "placeId"), s(first, "game").to_string());
                    let pic = o.icons.get(&universe).filter(|u| !u.is_empty()).and_then(|u| crate::images::pic_sq(u, 44., cx));
                    let (g2, count) = (game.clone(), rows.len());
                    let top = hot(("own-game", universe as usize))
                        .w_full()
                        .p(px(12.))
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .rounded(px(R_M))
                        .cursor_pointer()
                        .tones(ca(WHITE, 0.), ca(WHITE, 0.05))
                        .child(div().size(px(44.)).flex_none().rounded(px(R_S)).overflow_hidden().bg(c(RAISED_2)).flex().items_center().justify_center().map(|d| match pic {
                            Some(p) => d.child(img(p).size_full().object_fit(ObjectFit::Cover)),
                            None => d.child(icon("gamepad", 18., c(DIM))),
                        }))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(div().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).truncate().child(if game.is_empty() { format!("#{place}") } else { game }))
                                .child(div().text_size(px(T_S)).text_color(c(MUTED)).child(root.i18n.tn("own.nServers", count as i64, &[]))),
                        )
                        .child(icon("chevronRight", 14., c(MUTED)))
                        .on_click(cx.listener(move |r, _, _, cx| {
                            games::open_place(r, place, &g2);
                            r.games.tab = games::Tab::Servers;
                            cx.notify();
                        }));
                    let lines = rows.into_iter().map(|x| {
                        i += 1;
                        let (price, renews, expires) = (int(x, "price"), flag(x, "renews"), s(x, "expires"));
                        let left = days_to(expires);
                        let cost = if price > 0 {
                            div().flex().flex_none().items_center().gap(px(4.)).font_features(tnum()).text_color(c(TEXT)).child(icon("robux", 12., c(TEXT))).child(thousands(price, vi)).child(div().text_color(c(MUTED)).child(root.tr("own.aMonth")))
                        } else {
                            div().flex_none().text_color(c(OK)).child(root.tr("shop.free"))
                        };
                        let (when, tone) = match (expires.is_empty(), flag(x, "active"), renews) {
                            (true, _, _) => (root.tr("own.noEnd"), MUTED),
                            (_, false, _) => (root.i18n.tv("own.endedOn", &[("d", &day(expires, vi))]), MUTED),
                            (_, true, true) => (root.i18n.tv("own.renewsOn", &[("d", &day(expires, vi))]), MUTED),
                            (_, true, false) => (root.i18n.tv("own.endsOn", &[("d", &day(expires, vi))]), if left.is_some_and(|d| d <= 7) { WARN } else { MUTED }),
                        };
                        let name = s(x, "name");
                        div()
                            .w_full()
                            .px(px(12.))
                            .py(px(10.))
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .border_t_1()
                            .border_color(c(LINE))
                            .text_size(px(T_M))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.))
                                    .child(div().text_color(c(TEXT)).truncate().child(if name.is_empty() { format!("#{}", int(x, "id")) } else { name.to_string() }))
                                    .child(div().flex().items_center().gap(px(6.)).text_size(px(T_S)).text_color(c(MUTED)).child(icon("user", 11., c(MUTED))).child(div().min_w_0().truncate().child(s(x, "account").to_string()))),
                            )
                            .child(div().flex().flex_col().flex_none().items_end().gap(px(2.)).child(cost).child(div().text_size(px(T_S)).text_color(c(tone)).child(when)))
                    });
                    let col = columns.iter_mut().min_by_key(|(height, _)| *height).expect("one column");
                    col.0 += count + 1;
                    col.1.push(surface().w_full().p(px(4.)).flex().flex_col().child(top)
                        .children(lines.collect::<Vec<_>>()).into_any_element());
            }
            div().flex().items_start().gap(px(GRID_GAP)).children(columns.into_iter().map(|(_, cards)| {
                div().w(px(card_w)).flex_none().flex().flex_col().gap(px(GRID_GAP)).children(cards)
            })).into_any_element()
        }
    };
    let missed = (failed > 0).then(|| div().text_size(px(T_S)).text_color(c(WARN)).child(root.i18n.tv("own.missed", &[("n", &failed.to_string()), ("of", &asked.to_string())])));
    div().flex().flex_col().gap(px(18.)).children(bar).children(missed).child(body).child(div().text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("own.onRoblox"))).into_any_element()
}
