// A game's passes, a tab of its page (pages/games.rs): each pass with its price, a row opening on what it gives, which
// of the app's accounts own it already (Roblox tells that of one account at a time) and Buy for the account whose tab
// is shown, in two clicks, at the price shown (roblox::buy_pass). The list is Roblox's own and needs no sign-in; with
// one it also says which the account owns.
use gpui_kit::{prelude::*, *};

use crate::{
    images,
    kit::*,
    pages::{friends, games::thousands, profile},
    roblox::{self, Pass},
    shell::Root,
    theme::*,
};

#[derive(Default)]
pub struct Data {
    /// The game (its start place) and the account (its Roblox id; 0: none signed in yet) the list was read for, and
    /// the list (None while it loads).
    place: i64,
    me: i64,
    list: Option<Result<Vec<Pass>, String>>,
    loading: bool,
    /// The pass open, the app's accounts that own it (None while asked), the buyer's Robux, and the buying: armed by
    /// the first click, under way, and how it ended.
    at: Option<i64>,
    owners: Option<Vec<i64>>,
    /// Whether the buyer owns it (None while asked): asked alone and first, so Buy needn't wait for every account.
    mine: Option<bool>,
    robux: Option<i64>,
    armed: bool,
    busy: bool,
    done: Option<Result<(), String>>,
    /// Purchases under way and bought, by account (its Roblox id) and pass: kept when another game or account is
    /// shown, so an answer that comes meanwhile isn't lost and Buy doesn't come back while one is under way.
    buying: Vec<(i64, i64)>,
    bought: Vec<(i64, i64)>,
}

/// The passes of the game shown, read for the account whose tab is shown (again when that changes).
pub fn want(r: &mut Root, place: i64, cx: &mut Context<Root>) {
    let signed = friends::signed(r);
    let me = signed.as_ref().map(|s| s.1).unwrap_or(0);
    if r.passes.place != place || r.passes.me != me {
        let (buying, bought) = (std::mem::take(&mut r.passes.buying), std::mem::take(&mut r.passes.bought));
        r.passes = Data { place, me, buying, bought, ..Default::default() };
    }
    if r.passes.list.is_some() || r.passes.loading {
        return;
    }
    r.passes.loading = true;
    let cookie = signed.map(|s| s.2);
    let rx = roblox::run(move || roblox::passes(place, cookie.as_deref()));
    cx.spawn(async move |this, cx| {
        let got = rx.await.unwrap_or_else(|_| Err(String::new()));
        let _ = this.update(cx, |r, cx| {
            if r.passes.place == place && r.passes.me == me {
                let mut got = got;
                // (What this account bought here already, though Roblox's list may not say so yet.)
                if let Ok(l) = &mut got {
                    for p in l.iter_mut().filter(|p| me > 0 && r.passes.bought.contains(&(me, p.id))) {
                        p.owned = true;
                    }
                }
                r.passes.list = Some(got);
                r.passes.loading = false;
                // BLOXKEEP_SHOT_PASS=1: a hidden shot opens the first pass on sale (=armed: after Buy's first
                // click; nothing is bought).
                #[cfg(feature = "shot")]
                if let (Ok(how), Some(Ok(l))) = (std::env::var("BLOXKEEP_SHOT_PASS"), &r.passes.list) {
                    if let Some(first) = l.iter().find(|p| p.price.is_some()).map(|p| p.id) {
                        open(r, first, cx);
                        r.passes.armed = how == "armed";
                    }
                }
                cx.notify();
            }
        });
    })
    .detach();
}

/// Opens a pass (who owns it and the buyer's Robux are asked), or closes the one open.
fn open(r: &mut Root, id: i64, cx: &mut Context<Root>) {
    let p = &mut r.passes;
    p.armed = false;
    // (A purchase of it still under way keeps Buy away until its answer comes.)
    p.busy = p.buying.contains(&(p.me, id));
    p.done = None;
    p.owners = None;
    p.mine = None;
    p.robux = None;
    if p.at == Some(id) {
        p.at = None;
        return;
    }
    p.at = Some(id);
    let place = p.place;
    let signed = friends::signed(r);
    let users: Vec<i64> = profile::named(r).into_iter().map(|a| a.0).collect();
    // First the buyer's Robux and whether it owns the pass (Buy waits for these only), then every account's.
    let buyer = signed.clone();
    let quick = roblox::run(move || {
        let me = buyer.as_ref().map(|s| (s.1, s.2.as_str()));
        (me.and_then(|m| roblox::robux(m.1, m.0)), me.is_some_and(|m| roblox::owners("GamePass", id, &[m.0], Some(m)).contains(&m.0)))
    });
    cx.spawn(async move |this, cx| {
        let Ok((robux, mine)) = quick.await else { return };
        let _ = this.update(cx, |r, cx| {
            if r.passes.place == place && r.passes.at == Some(id) {
                let me = r.passes.me;
                r.passes.robux = robux;
                r.passes.mine = Some(mine || (me > 0 && r.passes.bought.contains(&(me, id))));
                cx.notify();
            }
        });
    })
    .detach();
    let rx = roblox::run(move || {
        let me = signed.as_ref().map(|s| (s.1, s.2.as_str()));
        roblox::owners("GamePass", id, &users, me)
    });
    cx.spawn(async move |this, cx| {
        let Ok(owners) = rx.await else { return };
        let _ = this.update(cx, |r, cx| {
            if r.passes.place == place && r.passes.at == Some(id) {
                let mut owners = owners;
                let me = r.passes.me;
                if me > 0 && r.passes.bought.contains(&(me, id)) && !owners.contains(&me) {
                    owners.push(me);
                }
                r.passes.owners = Some(owners);
                cx.notify();
            }
        });
    })
    .detach();
}

/// The second click: the open pass is bought for the account whose tab is shown, at the price on the page.
fn buy(r: &mut Root, cx: &mut Context<Root>) {
    let Some((_, user, cookie)) = friends::signed(r) else { return };
    let Some(id) = r.passes.at else { return };
    let pass = match &r.passes.list {
        Some(Ok(l)) => l.iter().find(|p| p.id == id).cloned(),
        _ => None,
    };
    let Some((pass, price)) = pass.and_then(|p| p.price.map(|price| (p, price))) else { return };
    if r.passes.busy || !r.passes.armed || r.passes.me != user || r.passes.buying.contains(&(user, id)) {
        return;
    }
    r.passes.busy = true;
    r.passes.armed = false;
    r.passes.done = None;
    r.passes.buying.push((user, id));
    let place = r.passes.place;
    let rx = roblox::run(move || roblox::buy_pass(&cookie, &pass, price));
    cx.spawn(async move |this, cx| {
        let done = rx.await.unwrap_or_else(|_| Err(String::new()));
        let _ = this.update(cx, |r, cx| {
            // Kept whatever is shown now, so it holds when this pass is shown again.
            r.passes.buying.retain(|b| *b != (user, id));
            if done.is_ok() && !r.passes.bought.contains(&(user, id)) {
                r.passes.bought.push((user, id));
            }
            let here = r.passes.place == place && r.passes.me == user;
            if here && done.is_ok() {
                if let Some(Ok(l)) = &mut r.passes.list {
                    if let Some(p) = l.iter_mut().find(|p| p.id == id) {
                        p.owned = true;
                    }
                }
            }
            if !here || r.passes.at != Some(id) {
                cx.notify();
                return;
            }
            r.passes.busy = false;
            if done.is_ok() {
                r.passes.robux = r.passes.robux.map(|r| r - price);
                if !r.passes.owners.as_ref().is_some_and(|l| l.contains(&user)) {
                    r.passes.owners.get_or_insert_with(Vec::new).push(user);
                }
            }
            r.passes.done = Some(done);
            cx.notify();
        });
    })
    .detach();
}

pub fn view(root: &Root, width: f32, cx: &mut Context<Root>) -> AnyElement {
    let ps = &root.passes;
    let said = |text: String| div().w_full().py(px(28.)).flex().justify_center().rounded(px(R_L)).bg(c(PANEL)).border_1().border_color(c(LINE)).text_size(px(T_M)).text_color(c(MUTED)).child(text);
    let list = match &ps.list {
        None => return said(root.tr("av.reading")).into_any_element(),
        Some(Err(why)) => return said(root.i18n.tv("in.failed", &[("why", if why.is_empty() { "Roblox" } else { why })])).into_any_element(),
        Some(Ok(l)) if l.is_empty() => return said(root.tr("pass.none")).into_any_element(),
        Some(Ok(l)) => l,
    };
    // The passes as tiles on the page's grid, each its picture on the pass's own colour (a list of rows left the right
    // half of a wide window empty). The one open shows what it gives, who owns it and Buy in a panel over the grid.
    let (_, tile_w) = grid_fit(width, 196., GRID_GAP);
    let tiles: Vec<AnyElement> = list.iter().enumerate().map(|(i, p)| tile(root, i, p, tile_w, cx).into_any_element()).collect();
    let opened = ps.at.and_then(|id| list.iter().find(|p| p.id == id));
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(16.))
        .child(section_head(root.tr("gp.passes"), Some(list.len())))
        .children(opened.map(|p| detail(root, p, cx)))
        .child(grid(GRID_GAP).children(tiles))
        .into_any_element()
}

/// The price as Roblox shows it: the Robux mark and the figure, or "off sale".
fn price_of(root: &Root, p: &Pass, size: f32) -> Div {
    match p.price {
        None => div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("shop.off")),
        Some(n) => div()
            .flex()
            .items_center()
            .gap(px(5.))
            .text_size(px(size))
            .font_weight(FontWeight::SEMIBOLD)
            .font_features(tnum())
            .text_color(c(TEXT))
            .child(icon("robux", (size - 1.).max(12.), c(TEXT)))
            .child(thousands(n, root.i18n.vietnamese)),
    }
}

fn tile(root: &Root, i: usize, p: &Pass, w: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let id = p.id;
    let on = root.passes.at == Some(id);
    let pic_h = (w * 0.62).round();
    let side = (pic_h - 24.).clamp(48., 120.);
    let pic = (!p.pic.is_empty()).then(|| images::pic_sq(&p.pic, side, cx)).flatten();
    let tone = (!p.pic.is_empty()).then(|| images::tone(&p.pic, cx)).flatten();
    block_card(("pass", i))
            .w(px(w))
            .overflow_hidden()
            .border_2()
            .map(|d| if on { d.edge(c(FACE), c(FACE_HI)) } else { d })
            .flex()
            .flex_col()
            .on_click(cx.listener(move |r, _, _, cx| {
                open(r, id, cx);
                cx.notify();
            }))
            .child(
                div()
                    .relative()
                    .w_full()
                    .h(px(pic_h))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(match tone {
                        Some(t) => mix(c(t), c(PANEL), 0.55),
                        None => c(RAISED_2),
                    })
                    .map(|d| match pic {
                        Some(f) => d.child(img(f).size(px(side)).object_fit(ObjectFit::Contain)),
                        None => d.child(icon("key", 28., c(MUTED))),
                    })
                    .when(p.owned, |d| d.child(div().absolute().top(px(8.)).right(px(8.)).child(badge(root.tr("shop.have"), Tone::Ok)))),
            )
            .child(
                div()
                    .px(px(12.))
                    .pt(px(10.))
                    .pb(px(12.))
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(div().truncate().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(p.name.clone()))
                    .child(price_of(root, p, T_M)),
            )
            .lift(3.)
}

/// The pass open: what it gives, which accounts own it, and Buy for the account whose tab is shown (two clicks).
fn detail(root: &Root, p: &Pass, cx: &mut Context<Root>) -> impl IntoElement {
    let ps = &root.passes;
    let vi = root.i18n.vietnamese;
    let id = p.id;
    let pic = (!p.pic.is_empty()).then(|| images::pic_sq(&p.pic, 72., cx)).flatten();
    let tone = (!p.pic.is_empty()).then(|| images::tone(&p.pic, cx)).flatten();
    let signed = friends::signed(root);
    let me = signed.as_ref().map(|s| s.1).unwrap_or(0);
    let accounts = profile::named(root);
    let line = |colour: u32, text: String| div().text_size(px(T_M)).text_color(c(colour)).child(text);
    let have: Div = match &ps.owners {
        None => line(MUTED, root.tr("av.reading")),
        Some(l) if l.is_empty() => line(MUTED, root.tr("shop.ownersNone")),
        Some(l) => div().flex().flex_wrap().gap(px(8.)).children(l.iter().filter_map(|user| accounts.iter().find(|a| a.0 == *user)).map(|(_, name)| {
            div().h(px(28.)).px(px(10.)).flex().items_center().gap(px(6.)).rounded(px(R_S)).bg(ca(OK, 0.12)).text_size(px(T_S)).font_weight(FontWeight::SEMIBOLD).text_color(c(OK)).child(icon("check", 12., c(OK))).child(name.clone())
        })),
    };
    let owned = p.owned || (me > 0 && (ps.mine == Some(true) || ps.owners.as_ref().is_some_and(|l| l.contains(&me))));
    let act: Option<Div> = p.price.map(|cost| {
        let short = ps.robux.is_some_and(|r| r < cost);
        if signed.is_none() {
            line(MUTED, root.tr("fr.noAccount"))
        } else if matches!(ps.done, Some(Ok(()))) {
            line(OK, root.tr("shop.bought"))
        } else if owned {
            line(OK, root.tr("shop.have"))
        } else if ps.busy {
            div().flex().child(Button::new("pass-buying", root.tr("shop.buying")).primary().disabled(true))
        } else if short {
            line(WARN, root.i18n.tv("shop.short", &[("n", &thousands(ps.robux.unwrap_or(0), vi))]))
        } else if ps.armed {
            let who = profile::buyer(root);
            let whose = if cost == 0 { root.i18n.tv("shop.chargeFree", &[("who", &who)]) } else { root.i18n.tv("shop.charge", &[("n", &thousands(cost, vi)), ("who", &who)]) };
            let after = ps.robux.filter(|_| cost > 0).map(|r| root.i18n.tv("shop.after", &[("n", &thousands(r - cost, vi))]));
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(div().flex().flex_col().gap(px(2.)).child(line(TEXT, whose)).children(after.map(|a| line(MUTED, a))))
                .child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .child(Button::new("pass-confirm", root.tr("shop.confirm")).primary().on_click(cx.listener(|r, _, _, cx| {
                            buy(r, cx);
                            cx.notify();
                        })))
                        .child(Button::new("pass-cancel", root.tr("gp.cancel")).on_click(cx.listener(|r, _, _, cx| {
                            r.passes.armed = false;
                            cx.notify();
                        }))),
                )
        } else {
            // (Armed only once what the accounts own is known: nothing is bought blind.)
            div().flex().child(Button::new("pass-buy", root.tr(if cost == 0 { "shop.get" } else { "shop.buy" })).primary().disabled(ps.mine.is_none()).on_click(cx.listener(|r, _, _, cx| {
                r.passes.armed = true;
                r.passes.done = None;
                cx.notify();
            })))
        }
    });
    let failed = match &ps.done {
        Some(Err(why)) => Some(match why.as_str() {
            "price" => root.tr("shop.moved"),
            "off" => root.tr("shop.closed"),
            why => root.i18n.tv("shop.refused", &[("why", if why.is_empty() { "Roblox" } else { why })]),
        }),
        _ => None,
    };
    let label = |key: &str| div().text_size(px(T_S)).text_color(c(MUTED)).child(root.tr(key));
    block()
            .w_full()
            .p(px(18.))
            .border_2()
            .border_color(c(FACE))
            .flex()
            .flex_wrap()
            .items_start()
            .gap(px(24.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(14.))
                    .min_w(px(260.))
                    .flex_1()
                    .child(
                        div()
                            .size(px(88.))
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
                                Some(f) => d.child(img(f).size(px(72.)).object_fit(ObjectFit::Contain)),
                                None => d.child(icon("key", 24., c(MUTED))),
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(div().text_size(px(T_GROUP)).line_height(px(24.)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(p.name.clone()))
                            .child(price_of(root, p, T_L))
                            .when(!p.about.is_empty(), |d| d.child(div().text_size(px(T_M)).text_color(c(TEXT_2)).line_height(px(20.)).child(p.about.clone()))),
                    ),
            )
            .child(div().min_w(px(220.)).flex_1().flex().flex_col().gap(px(8.)).child(label("shop.owners")).child(have))
            .child(div().min_w(px(220.)).flex_1().flex().flex_col().gap(px(10.)).children(act).children(failed.map(|f| line(WARN, f))))
            .child(IconButton::new(("pass-close", id as usize), "x").size(32.).tip(root.tr("common.close")).on_click(cx.listener(move |r, _, _, cx| {
                open(r, id, cx);
                cx.notify();
            })))
}
