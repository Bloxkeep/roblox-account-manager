// A game's badges, a tab of its page (pages/games.rs), laid out as Roblox's game page lists them (each badge's
// picture, name and what it is for, then its Rarity with Roblox's difficulty word, Won Yesterday and Won Ever), with
// what Roblox shows only of the one signed-in account: which of the app's accounts have each badge and since when, and
// how far each account is through the game's badges (a progress line per account, as Steam's achievements page gives
// each player). The list needs no sign-in; who has what is asked with the account whose tab is shown.
use std::collections::HashMap;

use gpui_kit::{prelude::*, *};

use crate::{
    images,
    kit::*,
    pages::{friends, profile},
    roblox::{self, GameBadge as Badge},
    shell::Root,
    theme::*,
};

#[derive(Default)]
pub struct Data {
    /// The game (its start place) and the account (its Roblox id; 0: none signed in) the list was read for.
    place: i64,
    me: i64,
    list: Option<Result<Vec<Badge>, String>>,
    loading: bool,
    /// Who has which badge and since when (seconds, 0: date not said), by account; None while asked.
    won: Option<HashMap<i64, HashMap<i64, i64>>>,
    /// Only the badges the account whose tab is shown hasn't won yet.
    missing: bool,
    /// The order: Roblox's own, or easiest first, or rarest first.
    order: Order,
}

#[derive(Default, Clone, Copy, PartialEq)]
pub enum Order {
    #[default]
    Roblox,
    Easy,
    Rare,
}

/// The badges of the game shown, and then who of the app's accounts has each (again for another account or game).
pub fn want(r: &mut Root, place: i64, cx: &mut Context<Root>) {
    let signed = friends::signed(r);
    let me = signed.as_ref().map(|s| s.1).unwrap_or(0);
    if r.badges.place != place || r.badges.me != me {
        let (missing, order) = (r.badges.missing, r.badges.order);
        r.badges = Data { place, me, missing, order, ..Default::default() };
    }
    if r.badges.list.is_some() || r.badges.loading {
        return;
    }
    r.badges.loading = true;
    let users: Vec<i64> = profile::named(r).into_iter().map(|a| a.0).collect();
    let cookie = signed.map(|s| s.2);
    let rx = roblox::run(move || roblox::badges(place));
    cx.spawn(async move |this, cx| {
        let got = rx.await.unwrap_or_else(|_| Err(String::new()));
        let ids: Vec<i64> = got.as_ref().map(|l| l.iter().map(|b| b.id).collect()).unwrap_or_default();
        let failed = got.is_err();
        let _ = this.update(cx, |r, cx| {
            if r.badges.place == place && r.badges.me == me {
                r.badges.list = Some(got);
                r.badges.loading = false;
                #[cfg(feature = "shot")]
                if std::env::var("BLOXKEEP_SHOT_BADGES_MISSING").is_ok() {
                    r.badges.missing = true;
                }
                cx.notify();
            }
        });
        if failed {
            cx.background_executor().timer(crate::pages::games::RETRY).await;
            let _ = this.update(cx, |r, cx| {
                if r.badges.place == place && r.badges.me == me && matches!(r.badges.list, Some(Err(_))) {
                    r.badges.list = None;
                    cx.notify();
                }
            });
            return;
        }
        if ids.is_empty() || users.is_empty() {
            let _ = this.update(cx, |r, cx| {
                if r.badges.place == place && r.badges.me == me {
                    r.badges.won = Some(HashMap::new());
                    cx.notify();
                }
            });
            return;
        }
        let won = roblox::run(move || roblox::badge_owners(&ids, &users, cookie.as_deref())).await.unwrap_or_default();
        let _ = this.update(cx, |r, cx| {
            if r.badges.place == place && r.badges.me == me {
                r.badges.won = Some(won);
                cx.notify();
            }
        });
    })
    .detach();
}

/// Roblox's word for how hard a badge is, from the share of players who won it (its page's Rarity: Freebie at 90 % and
/// up down to Impossible under 1 %).
fn difficulty(rate: f64) -> &'static str {
    let p = rate * 100.;
    match p {
        p if p >= 90. => "bd.freebie",
        p if p >= 80. => "bd.cakewalk",
        p if p >= 50. => "bd.easy",
        p if p >= 30. => "bd.moderate",
        p if p >= 20. => "bd.challenging",
        p if p >= 10. => "bd.hard",
        p if p >= 5. => "bd.extreme",
        p if p >= 1. => "bd.insane",
        _ => "bd.impossible",
    }
}

/// The share as Roblox writes it: one decimal, "< 0.1%" for what rounds to nothing.
fn share(rate: f64, vi: bool) -> String {
    let p = rate * 100.;
    let t = if p > 0. && p < 0.1 || p == 0. { "< 0.1".to_string() } else { format!("{p:.1}") };
    let t = if vi { t.replace('.', ",") } else { t };
    format!("{t}%")
}

pub fn view(root: &Root, width: f32, cx: &mut Context<Root>) -> AnyElement {
    let bd = &root.badges;
    let vi = root.i18n.vietnamese;
    let said = |text: String| div().w_full().py(px(28.)).flex().justify_center().rounded(px(R_L)).bg(c(PANEL)).border_1().border_color(c(LINE)).text_size(px(T_M)).text_color(c(MUTED)).child(text);
    let list = match &bd.list {
        None => return said(root.tr("av.reading")).into_any_element(),
        Some(Err(why)) => return said(root.i18n.tv("in.failed", &[("why", crate::net::why_words(&root.i18n, why).as_str())])).into_any_element(),
        Some(Ok(l)) if l.is_empty() => return said(root.tr("bd.none")).into_any_element(),
        Some(Ok(l)) => l,
    };
    let accounts = profile::named(root);
    let me = bd.me;
    let mine = bd.won.as_ref().and_then(|w| w.get(&me));
    let lacking = mine.map(|m| list.iter().filter(|b| !m.contains_key(&b.id)).count());

    // The head: the title and count, the order, and "Not won yet" for the account whose tab is shown.
    let order_seg = |id: &'static str, o: Order, key: &str| {
        chip(id, bd.order == o).child(root.tr(key)).on_click(cx.listener(move |r, _, _, cx| {
            r.badges.order = o;
            cx.notify();
        }))
    };
    let head = section_head(root.tr("bd.title"), Some(list.len()))
        .child(order_seg("bd-o-roblox", Order::Roblox, "bd.orderRoblox"))
        .child(order_seg("bd-o-easy", Order::Easy, "bd.orderEasy"))
        .child(order_seg("bd-o-rare", Order::Rare, "bd.orderRare"))
        .when_some(lacking.filter(|_| me > 0), |d, n| {
            d.child(div().w(px(1.)).h(px(18.)).mx(px(4.)).bg(c(LINE))).child(
                chip("bd-missing", bd.missing).child(root.tr("bd.missing")).child(chip_count(n, bd.missing)).on_click(cx.listener(|r, _, _, cx| {
                    r.badges.missing = !r.badges.missing;
                    cx.notify();
                })),
            )
        });

    // Each account's way through the game's badges: its face, name, "won N of M" and a line that fills.
    let progress: Option<AnyElement> = bd.won.as_ref().filter(|w| !w.is_empty()).map(|won| {
        let total = list.len().max(1);
        let (cols, card_w) = grid_fit(width - 2. * PAGE_X, 220., 12.);
        let _ = cols;
        let mut rows: Vec<(i64, String, usize)> = accounts.iter().filter_map(|(u, name)| won.get(u).map(|w| (*u, name.clone(), list.iter().filter(|b| w.contains_key(&b.id)).count()))).collect();
        rows.sort_by(|a, b| b.2.cmp(&a.2).then(a.1.cmp(&b.1)));
        let cards: Vec<AnyElement> = rows
            .into_iter()
            .map(|(user, name, n)| {
                let url = root.accounts().iter().find(|a| a.get("userId").and_then(serde_json::Value::as_i64) == Some(user)).and_then(|a| a.get("avatarUrl").and_then(serde_json::Value::as_str)).unwrap_or_default().to_string();
                let pic = (!url.is_empty()).then(|| images::pic(&url, cx)).flatten();
                let part = n as f32 / total as f32;
                let done = n >= total;
                div()
                    .w(px(card_w))
                    .p(px(12.))
                    .rounded(px(R_L))
                    .bg(c(PANEL))
                    .border_1()
                    .border_color(if user == me { ca(FACE, 0.55) } else { c(LINE) })
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(face(&name, pic, 36.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .child(div().flex_1().min_w_0().truncate().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(name))
                                    .child(div().flex_none().text_size(px(T_S)).font_features(tnum()).text_color(c(if done { OK } else { TEXT_2 })).child(root.i18n.tv("bd.of", &[("n", &n.to_string()), ("t", &total.to_string())]))),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .h(px(4.))
                                    .rounded_full()
                                    .bg(c(RAISED_2))
                                    .child(div().h_full().rounded_full().bg(if done { c(OK) } else { ca(TEXT, 0.55) }).with_spring(("bd-prog", user as u64 as usize), SpringAnimation::new(EASE).to(part), |d, v| d.w(relative(v.clamp(0., 1.))))),
                            ),
                    )
                    .into_any_element()
            })
            .collect();
        div().flex().flex_col().gap(px(10.)).child(div().text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("bd.yours"))).child(div().flex().flex_wrap().gap(px(12.)).children(cards)).into_any_element()
    });

    let mut shown: Vec<&Badge> = list.iter().filter(|b| !(bd.missing && mine.is_some_and(|m| m.contains_key(&b.id)))).collect();
    match bd.order {
        Order::Roblox => {}
        Order::Easy => shown.sort_by(|a, b| b.rate.partial_cmp(&a.rate).unwrap_or(std::cmp::Ordering::Equal).then(b.ever.cmp(&a.ever))),
        Order::Rare => shown.sort_by(|a, b| a.rate.partial_cmp(&b.rate).unwrap_or(std::cmp::Ordering::Equal).then(a.ever.cmp(&b.ever))),
    }
    let wide = width >= 900.;
    let rows: Vec<AnyElement> = shown.iter().enumerate().map(|(i, b)| row(root, i, b, &accounts, wide, vi, cx).into_any_element()).collect();
    let body: AnyElement = if rows.is_empty() { said(root.tr("bd.allWon")).into_any_element() } else { div().flex().flex_col().gap(px(8.)).children(rows).into_any_element() };
    div().w_full().flex().flex_col().gap(px(16.)).child(head).children(progress).child(body).into_any_element()
}

/// One badge: picture, name and what it is for, who of the app's accounts has it, then Roblox's three figures.
fn row(root: &Root, i: usize, b: &Badge, accounts: &[(i64, String)], wide: bool, vi: bool, cx: &mut Context<Root>) -> impl IntoElement {
    let bd = &root.badges;
    let pic = (!b.pic.is_empty()).then(|| images::pic_sq(&b.pic, 64., cx)).flatten();
    let tone = (!b.pic.is_empty()).then(|| images::tone(&b.pic, cx)).flatten();
    let me_has = bd.won.as_ref().and_then(|w| w.get(&bd.me)).is_some_and(|m| m.contains_key(&b.id));
    // Who has it: each account with a tick, and the day it won it under the mouse.
    let owners: Vec<AnyElement> = match &bd.won {
        None => vec![div().text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("av.reading")).into_any_element()],
        Some(won) => accounts
            .iter()
            .filter_map(|(u, name)| won.get(u).and_then(|w| w.get(&b.id)).map(|at| (*u, name.clone(), *at)))
            .map(|(u, name, at)| {
                let tip = if at > 0 { root.i18n.tv("bd.wonOn", &[("d", &crate::time::day_time(at))]) } else { root.tr("bd.won") };
                hot(("bd-who", i * 1000 + (u as u64 % 997) as usize))
                    .h(px(24.))
                    .px(px(8.))
                    .rounded(px(R_S))
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .tones(ca(OK, 0.10), ca(OK, 0.16))
                    .text_size(px(T_S))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(c(TEXT))
                    .child(icon("check", 12., c(OK)))
                    .child(name)
                    .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone().into())).into())
                    .into_any_element()
            })
            .collect(),
    };
    let nobody = bd.won.is_some() && owners.is_empty();
    let figure = |key: &str, value: String, under: Option<String>| {
        div()
            .w(px(if wide { 128. } else { 104. }))
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(div().text_size(px(T_S)).text_color(c(MUTED)).child(root.tr(key)))
            .child(div().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).font_features(tnum()).text_color(c(TEXT)).child(value))
            .children(under.map(|u| div().text_size(px(T_S)).text_color(c(TEXT_2)).child(u)))
    };
    let figures = div()
        .flex()
        .flex_none()
        .gap(px(12.))
        .child(figure("bd.rarity", share(b.rate, vi), Some(root.tr(difficulty(b.rate)))))
        .child(figure("bd.yesterday", thousands(b.yesterday, vi), None))
        .child(figure("bd.ever", thousands(b.ever, vi), None));
    let words = div()
        .flex_1()
        .min_w(px(220.))
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(div().flex().items_center().gap(px(8.)).child(div().min_w_0().truncate().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(b.name.clone())).when(me_has, |d| d.child(badge(root.tr("bd.have"), Tone::Ok))))
        .when(!b.about.is_empty(), |d| d.child(div().text_size(px(T_M)).line_height(px(19.)).line_clamp(2).text_color(c(TEXT_2)).child(b.about.clone())))
        .child(div().pt(px(4.)).flex().flex_wrap().gap(px(6.)).children(owners).when(nobody, |d| d.child(div().text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("bd.nobody")))));
    div()
        .w_full()
        .p(px(14.))
        .rounded(px(R_L))
        .bg(c(PANEL))
        .border_1()
        .border_color(c(LINE))
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(16.))
        .child(
            div()
                .size(px(76.))
                .flex_none()
                .rounded(px(R_L))
                .flex()
                .items_center()
                .justify_center()
                .bg(match tone {
                    Some(t) => mix(c(t), c(PANEL), 0.55),
                    None => c(RAISED_2),
                })
                .map(|d| match pic {
                    Some(f) => d.child(img(f).size(px(64.)).rounded_full().object_fit(ObjectFit::Contain)),
                    None => d.child(icon("star", 26., c(MUTED))),
                }),
        )
        .child(words)
        .child(figures)
}
