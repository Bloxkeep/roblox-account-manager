// A game's servers, a tab of its page (pages/games.rs). On top, the picked accounts' friends in this game and the
// server each is in (friendServers; a public row shows how many of them are there). Then the private ones (below), then the
// public ones, as the web page's server list had them
// (ui/src/Servers.tsx): how full each is, the ping and frame rate Roblox reports for it, sorted by players (Roblox
// sorts), ping or FPS (sorted here), more pages on demand, a random one; Roblox lets an app load these lists only a few
// times a minute, and the app says how long to wait ("servers": { servers, next, busy, retryIn }). Join opens the
// page's Play row for that server.
//
// The private servers, under the friends (above the public list since 2026-11-13): the ones the app's accounts can join, read by the app
// itself (its "privateServers": each account asked with its own sign-in, the answers put together), each with who owns
// it, how full it is, and a button for every account that can join it. Roblox's own page shows them one account at a
// time. Joining is the app's launch, into the server by its access code.
use std::time::{Duration, Instant};

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{prelude::*, *};
use serde_json::{json, Value};
use crate::val::s;
use crate::ask::answer;

use crate::{
    kit::*,
    pages::{accounts::can_join_game, friends},
    shell::Root,
    theme::*,
};

/// How old the list may get while its tab is shown.
const FRESH: Duration = Duration::from_secs(60);

#[derive(Default)]
pub struct Data {
    /// The game (its start place) the servers are of, what the app answered (None while the first answer is on its
    /// way), when, and whether an asking is under way.
    place: i64,
    got: Option<Result<Value, String>>,
    at: Option<Instant>,
    loading: bool,
    /// Public servers: the rows so far and Roblox's mark for the next page; the order ("" and "most": fullest
    /// first, "fewest": emptiest first, both Roblox's; "ping", "fps": the rows here, sorted); whether the first page
    /// was asked for, whether a page is on its way, whether the last asking failed, and until when Roblox said to wait.
    rows: Vec<Value>,
    next: Option<String>,
    sort: &'static str,
    asked: bool,
    more: bool,
    failed: bool,
    wait_until: Option<Instant>,
    /// Friends in this game: what the app answered ("friendServers": { friends, asked }; Err when it failed, None while
    /// the asking is on its way), when, and whether an asking is under way.
    friends: Option<Result<Value, ()>>,
    friends_at: Option<Instant>,
    friends_asking: bool,
    /// The accounts the private servers and the friends were last asked with: other accounts picked ask again at once,
    /// not a minute later.
    private_ids: Vec<String>,
    private_fresh: bool,
    create_private: bool,
    vip_opening: bool,
    vip_owner: Option<String>,
    vip_error: Option<String>,
    /// The private servers' search box (by name, owner or account), made the first time the tab is shown.
    search: Option<Entity<InputState>>,
    /// Create Private Server in the app, as Roblox's own box has it: the account it is bought for, the game's universe
    /// and price a month (None while asked; Some(None): the game sells none), Buy clicked once (a second click buys),
    /// a buying on its way.
    buy: Option<Buy>,
    friend_ids: Vec<String>,
    /// "Room for all": only servers with a seat for every account picked (Servers.tsx's room); None: on when more
    /// than one is picked.
    room: Option<bool>,
    /// The page Roblox said to wait for was a next one (asked again by itself then, the rows so far kept).
    wait_more: bool,
    /// How many more lists Roblox gives this minute (None: not known): a page for ping or FPS only while one is left
    /// for the next click (Servers.tsx's spare).
    left: Option<i64>,
    /// What gamejoin said of each server shown ("serverFacts"): country and city, game version, start (Unix ms), by
    /// Job ID (Null: asked, no answer); whether an asking is on its way; the newest version seen; whether Roblox
    /// blocks the game where the account is, or asked to pause (no more asking then until the list is loaded again);
    /// the country picked (None: all).
    facts: std::collections::HashMap<String, Value>,
    facts_asking: bool,
    newest: i64,
    facts_blocked: bool,
    facts_paused: bool,
    country: Option<String>,
    /// Private servers whose every account is shown (past the first few, "+N" opens them).
    priv_open: std::collections::HashSet<i64>,
}

struct Buy {
    account: String,
    place: i64,
    universe: i64,
    price: Option<Result<Option<i64>, String>>,
    armed: bool,
    busy: bool,
}

/// Opens Create Private Server for this account: the name box empty, the game's price asked.
fn start_buy(r: &mut Root, place: i64, account: String, window: &mut Window, cx: &mut Context<Root>) {
    r.ps_name.update(cx, |s, cx| s.set_value("", window, cx));
    begin_buy(r, place, account, cx);
}

fn begin_buy(r: &mut Root, place: i64, account: String, cx: &mut Context<Root>) {
    r.servers.create_private = false;
    r.servers.vip_error = None;
    r.servers.buy = Some(Buy { account, place, universe: 0, price: None, armed: false, busy: false });
    let rx = crate::roblox::run(move || crate::roblox::ps_price(place));
    crate::ask::reply(rx, cx, move |r, got, cx| {
        if let Some(b) = r.servers.buy.as_mut().filter(|b| b.place == place) {
            match got {
                Ok((universe, price)) => {
                    b.universe = universe;
                    b.price = Some(Ok(price));
                }
                Err(why) => b.price = Some(Err(why)),
            }
            cx.notify();
        }
    });
    cx.notify();
}

/// Buy: the first click arms it (the price on the button), the second buys with the account's own sign-in.
fn buy(r: &mut Root, cx: &mut Context<Root>) {
    let Some(link) = r.link.clone() else { return };
    let name = r.ps_name.read(cx).value().trim().to_string();
    let Some(b) = r.servers.buy.as_mut().filter(|b| !b.busy && b.universe > 0) else { return };
    let Some(Ok(Some(price))) = b.price.clone() else { return };
    if !b.armed {
        b.armed = true;
        cx.notify();
        return;
    }
    b.busy = true;
    let (account, universe, place) = (b.account.clone(), b.universe, b.place);
    cx.notify();
    // The account's own sign-in (the one Roblox charges), as the app holds it.
    let rx = link.call("robloxSignIn", json!({ "id": account }));
    answer(rx, cx, move |r, got, cx| {
        let cookie = got.ok().and_then(Result::ok).map(|v| s(&v, "cookie").to_string()).unwrap_or_default();
        if cookie.is_empty() {
            if let Some(b) = r.servers.buy.as_mut().filter(|b| b.place == place) {
                b.busy = false;
                b.armed = false;
            }
            r.toast = Some((r.tr("ps.noSign"), "bad".into()));
            cx.notify();
            return;
        }
        let name = if name.is_empty() { r.tr("ps.defaultName") } else { name };
        let rx = crate::roblox::run(move || crate::roblox::ps_create(&cookie, universe, &name, price));
        crate::ask::reply(rx, cx, move |r, got, cx| {
            match got {
                Ok(()) => {
                    r.servers.buy = None;
                    r.toast = Some((r.tr("ps.bought"), "ok".into()));
                    // The new one in the list at once.
                    r.servers.private_fresh = true;
                    if r.servers.place == place {
                        want(r, place, cx);
                    }
                    r.owned.forget();
                }
                Err(why) => {
                    if let Some(b) = r.servers.buy.as_mut().filter(|b| b.place == place) {
                        b.busy = false;
                        b.armed = false;
                    }
                    r.toast = Some((r.i18n.tv("ps.buyFailed", &[("why", crate::net::why_words(&r.i18n, &why).as_str())]), "bad".into()));
                }
            }
            cx.notify();
        });
    });
}

/// Roblox's Create Private Server box: for which account, a name, what it costs a month (it renews by itself until
/// cancelled), Buy (twice) and Cancel; buying on Roblox's own page stays one click away.
fn buy_box(root: &Root, b: &Buy, cx: &mut Context<Root>) -> Div {
    let who = root.accounts().iter().find(|a| s(a, "id") == b.account).map(crate::pages::accounts::label).unwrap_or_default();
    let price_line: AnyElement = match &b.price {
        None => div().text_color(c(MUTED)).child(root.tr("av.reading")).into_any_element(),
        Some(Err(why)) => div().text_color(c(BAD)).child(root.i18n.tv("ps.priceFailed", &[("why", crate::net::why_words(&root.i18n, why).as_str())])).into_any_element(),
        Some(Ok(None)) => div().text_color(c(MUTED)).child(root.tr("ps.notSold")).into_any_element(),
        Some(Ok(Some(0))) => div().text_color(c(OK)).font_weight(FontWeight::SEMIBOLD).child(root.tr("shop.free")).into_any_element(),
        Some(Ok(Some(p))) => div()
            .flex()
            .items_center()
            .gap(px(5.))
            .font_weight(FontWeight::SEMIBOLD)
            .font_features(tnum())
            .child(icon("robux", 14., c(TEXT)))
            .child(thousands(*p))
            .child(div().font_weight(FontWeight::NORMAL).text_color(c(MUTED)).child(root.tr("own.aMonth")))
            .into_any_element(),
    };
    let can_buy = matches!(b.price, Some(Ok(Some(_)))) && b.universe > 0;
    let buy_words = match (&b.price, b.armed, b.busy) {
        (_, _, true) => root.tr("ps.buying"),
        (Some(Ok(Some(p))), true, _) if *p > 0 => root.i18n.tv("ps.confirm", &[("n", &thousands(*p))]),
        (_, true, _) => root.tr("ps.confirmFree"),
        _ => root.tr("ps.buy"),
    };
    let (place, account) = (b.place, b.account.clone());
    let row = |label: String, body: AnyElement| {
        div().flex().items_center().gap(px(12.)).child(div().w(px(110.)).flex_none().text_size(px(T_M)).text_color(c(MUTED)).child(label)).child(div().flex_1().min_w_0().text_size(px(T_M)).child(body))
    };
    surface()
        .p(px(16.))
        .max_w(px(560.))
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(div().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).child(root.tr("ps.title")))
        .child(row(root.tr("ps.account"), div().font_weight(FontWeight::MEDIUM).child(who).into_any_element()))
        .child(row(root.tr("ps.name"), field(&root.ps_name, None).into_any_element()))
        .child(row(root.tr("ps.price"), price_line))
        .child(div().text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("ps.renewNote")))
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(8.))
                .child(Button::new("ps-buy", buy_words).primary().small().disabled(!can_buy || b.busy).on_click(cx.listener(|r, _, _, cx| buy(r, cx))))
                .child(Button::new("ps-cancel", root.tr("gp.cancel")).small().quiet().disabled(b.busy).on_click(cx.listener(|r, _, _, cx| {
                    r.servers.buy = None;
                    cx.notify();
                })))
                .child(div().flex_1())
                .child(Button::new("ps-on-roblox", root.tr("ps.onRoblox")).small().quiet().icon("external").on_click(cx.listener(move |r, _, _, cx| {
                    r.servers.buy = None;
                    open_vip_checkout(r, place, account.clone(), cx);
                }))),
        )
}

/// Ping, FPS and newest are sorted here, over the rows loaded.
fn local(sort: &str) -> bool {
    sort == "ping" || sort == "fps" || sort == "new"
}

/// Where each server shown runs, the game version it is on and when it started: asked of gamejoin as Roblox's own
/// client asks (the address gives the country; RoValra shows the same on Roblox's page), 40 at a time, the next 40
/// once these are in, the full ones never (gamejoin gives a full server no address).
fn want_facts(r: &mut Root, cx: &mut Context<Root>) {
    if r.servers.facts_asking || r.servers.facts_blocked || r.servers.facts_paused {
        return;
    }
    let Some(link) = r.link.clone() else { return };
    let jobs: Vec<String> = r
        .servers
        .rows
        .iter()
        .filter(|v| {
            let most = v.get("maxPlayers").and_then(Value::as_i64).unwrap_or(0);
            most == 0 || v.get("playing").and_then(Value::as_i64).unwrap_or(0) < most
        })
        .map(|v| s(v, "jobId").to_string())
        .filter(|j| !j.is_empty() && !r.servers.facts.contains_key(j))
        .take(40)
        .collect();
    if jobs.is_empty() {
        return;
    }
    let place = r.servers.place;
    let ids: Vec<String> = r.selected.iter().cloned().collect();
    r.servers.facts_asking = true;
    let rx = link.call("serverFacts", json!({ "placeId": place, "jobs": jobs, "ids": ids }));
    cx.spawn(async move |this, cx| {
        let answer = rx.await;
        let _ = this.update(cx, |r, cx| {
            if r.servers.place != place {
                return;
            }
            r.servers.facts_asking = false;
            match answer {
                Ok(Ok(v)) => {
                    for f in v.get("facts").and_then(Value::as_array).into_iter().flatten() {
                        r.servers.facts.insert(s(f, "jobId").to_string(), f.clone());
                    }
                    // (Asked once each: one that got no answer isn't asked again and again.)
                    for j in &jobs {
                        r.servers.facts.entry(j.clone()).or_insert(Value::Null);
                    }
                    r.servers.newest = r.servers.newest.max(v.get("newest").and_then(Value::as_i64).unwrap_or(0));
                    r.servers.facts_blocked = v.get("blocked").and_then(Value::as_bool).unwrap_or(false);
                    r.servers.facts_paused = v.get("stopped").and_then(Value::as_bool).unwrap_or(false);
                    want_facts(r, cx);
                }
                _ => r.servers.facts_paused = true,
            }
            cx.notify();
        });
    })
    .detach();
}

fn spare(r: &Root) -> bool {
    r.servers.left.is_none_or(|n| n < 0 || n >= 2)
}

/// A page of public servers: the first (again), or the next.
fn load_public(r: &mut Root, more: bool, cx: &mut Context<Root>) {
    let Some(link) = r.link.clone() else { return };
    if r.servers.more {
        return;
    }
    let cursor = if more { r.servers.next.clone() } else { None };
    if more && cursor.is_none() {
        return;
    }
    let (place, fewest) = (r.servers.place, r.servers.sort == "fewest");
    r.servers.asked = true;
    r.servers.more = true;
    r.servers.failed = false;
    if !more {
        r.servers.rows.clear();
        r.servers.next = None;
    }
    let rx = link.call("servers", json!({ "placeId": place, "fewest": fewest, "cursor": cursor }));
    cx.spawn(async move |this, cx| {
        let answer = rx.await;
        let _ = this.update(cx, |r, cx| {
            // (Another game, or the other order, was asked for meanwhile: this answer is not its.)
            if r.servers.place != place || (r.servers.sort == "fewest") != fewest {
                return;
            }
            r.servers.more = false;
            match answer {
                Ok(Ok(v)) if v.get("busy").and_then(Value::as_bool) == Some(true) => {
                    // (No wait given, or 0: 20 s, as Servers.tsx's `r.retryIn || 20`; 0 asked again every 400 ms.)
                    let secs = v.get("retryIn").and_then(Value::as_u64).filter(|n| *n > 0).unwrap_or(20);
                    let until = Instant::now() + Duration::from_secs(secs);
                    r.servers.wait_until = Some(until);
                    r.servers.wait_more = more;
                    // Asked again by itself once the wait is over (Servers.tsx), even with nothing else redrawing.
                    cx.spawn(async move |this, cx| {
                        cx.background_executor().timer(Duration::from_secs(secs) + Duration::from_millis(400)).await;
                        let _ = this.update(cx, |r, cx| {
                            if r.servers.place == place && r.servers.wait_until == Some(until) && !r.servers.more {
                                r.servers.wait_until = None;
                                load_public(r, more, cx);
                            }
                        });
                    })
                    .detach();
                }
                Ok(Ok(v)) => {
                    r.servers.wait_until = None;
                    r.servers.left = v.get("left").and_then(Value::as_i64);
                    let mut got = v.get("servers").and_then(Value::as_array).cloned().unwrap_or_default();
                    // (A page may repeat a server of the one before.)
                    got.retain(|n| !r.servers.rows.iter().any(|o| o.get("jobId") == n.get("jobId")));
                    r.servers.rows.extend(got);
                    r.servers.next = v.get("next").and_then(Value::as_str).filter(|n| !n.is_empty()).map(str::to_string);
                    if !more {
                        r.servers.facts_paused = false;
                    }
                    want_facts(r, cx);
                    // Ping and FPS: the first page and one more, so the order is over more than one page's servers.
                    if !more && local(r.servers.sort) && r.servers.next.is_some() && spare(r) {
                        load_public(r, true, cx);
                    }
                }
                Err(_) | Ok(Err(_)) if more => {}
                _ => r.servers.failed = true,
            }
            cx.notify();
        });
    })
    .detach();
}

fn set_sort(r: &mut Root, sort: &'static str, cx: &mut Context<Root>) {
    let was = r.servers.sort == "fewest";
    r.servers.sort = sort;
    // Ping and FPS are sorted here, from the fullest servers (the ones people play on); only emptiest-first asks again.
    // (No rows yet with the first page still on its way: that page is the answer; asking again would spend a list.)
    if was != (sort == "fewest") || (r.servers.rows.is_empty() && !r.servers.more) {
        r.servers.more = false;
        load_public(r, false, cx);
    } else if local(sort) && r.servers.rows.len() < 150 && r.servers.next.is_some() && spare(r) && !r.servers.more {
        // A next page for the order to be over more servers, while that leaves one for the next click.
        load_public(r, true, cx);
    }
    cx.notify();
}

pub fn want(r: &mut Root, place: i64, cx: &mut Context<Root>) {
    if r.servers.place != place {
        r.servers = Data { place, ..Default::default() };
    }
    // The public servers' first page; and once Roblox's wait is over, the page that was refused.
    if !r.servers.asked {
        load_public(r, false, cx);
    } else if !r.servers.more && r.servers.wait_until.is_some_and(|t| Instant::now() >= t) {
        r.servers.wait_until = None;
        if r.servers.wait_more && r.servers.next.is_some() {
            load_public(r, true, cx);
        } else if r.servers.rows.is_empty() {
            load_public(r, false, cx);
        }
    }
    want_friends(r, place, cx);
    // BLOXKEEP_SHOT_PS=1 with BLOXKEEP_SHOT_SERVERS: a hidden shot opens Create Private Server for the first account
    // (=armed: after Buy's first click; nothing is bought).
    #[cfg(feature = "shot")]
    if let (Ok(how), None, true) = (std::env::var("BLOXKEEP_SHOT_PS"), &r.servers.buy, std::env::var("BLOXKEEP_SHOT_SERVERS").is_ok()) {
        if let Some(first) = r.accounts().first().map(|a| s(a, "id").to_string()) {
            begin_buy(r, place, first, cx);
            if let Some(b) = r.servers.buy.as_mut() {
                b.armed = how == "armed";
            }
        }
    }
    if r.servers.loading {
        return;
    }
    // The accounts asked: the picked ones (Servers.tsx's ids), else the signed-in ones; the app's limit of them (six).
    let able = friends::able_ref(r);
    let picked: Vec<String> = able.iter().filter(|a| r.selected.contains(s(a, "id"))).take(6).map(|a| s(a, "id").to_string()).collect();
    let mut ids: Vec<String> = if picked.is_empty() { able.iter().take(6).map(|a| s(a, "id").to_string()).collect() } else { picked };
    if let Some(owner) = r.servers.vip_owner.as_ref().filter(|id| able.iter().any(|a| s(a, "id") == *id)) {
        ids.retain(|id| id != owner);
        ids.insert(0, owner.clone());
        ids.truncate(6);
    }
    if !r.servers.private_fresh && r.servers.private_ids == ids && r.servers.at.is_some_and(|t| t.elapsed() < FRESH) {
        return;
    }
    let Some(link) = r.link.clone() else { return };
    r.servers.loading = true;
    r.servers.private_ids = ids.clone();
    let fresh = std::mem::take(&mut r.servers.private_fresh);
    let rx = link.call("privateServers", json!({ "placeId": place, "ids": ids, "fresh": fresh }));
    cx.spawn(async move |this, cx| {
        let got = match rx.await {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err(e.to_string()),
            Err(_) => Err(String::new()),
        };
        let _ = this.update(cx, |r, cx| {
            if r.servers.place == place {
                r.servers.loading = false;
                r.servers.at = Some(Instant::now());
                // A later asking that fails leaves what is shown.
                if got.is_ok() || !matches!(r.servers.got, Some(Ok(_))) {
                    r.servers.got = Some(got);
                }
                cx.notify();
            }
        });
    })
    .detach();
}

/// The private servers' search box, made once the servers tab has a game; typing redraws the list.
pub fn settle(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    if r.servers.search.is_some() || r.servers.place <= 0 {
        return;
    }
    let e = cx.new(|cx| InputState::new(window, cx).placeholder(r.tr("srv.privateSearch")));
    cx.subscribe(&e, |_: &mut Root, _, ev: &InputEvent, cx| {
        if matches!(ev, InputEvent::Change) {
            cx.notify();
        }
    })
    .detach();
    r.servers.search = Some(e);
}

/// Where the accounts' friends are in this game (the app asks with three accounts' own sign-ins, the picked ones
/// first): their servers, to join.
fn want_friends(r: &mut Root, place: i64, cx: &mut Context<Root>) {
    if r.servers.friends_asking {
        return;
    }
    let ids: Vec<String> = r.accounts().iter().filter_map(|a| a.get("id").and_then(Value::as_str)).filter(|id| r.selected.contains(*id)).map(str::to_string).collect();
    if r.servers.friend_ids == ids && r.servers.friends_at.is_some_and(|t| t.elapsed() < FRESH) {
        return;
    }
    let Some(link) = r.link.clone() else { return };
    r.servers.friends_asking = true;
    r.servers.friend_ids = ids.clone();
    let rx = link.call("friendServers", json!({ "placeId": place, "ids": ids }));
    cx.spawn(async move |this, cx| {
        let got = match rx.await {
            Ok(Ok(v)) => Ok(v),
            _ => Err(()),
        };
        let _ = this.update(cx, |r, cx| {
            if r.servers.place == place {
                r.servers.friends_asking = false;
                r.servers.friends_at = Some(Instant::now());
                // A later asking that fails leaves what is shown.
                if got.is_ok() || !matches!(r.servers.friends, Some(Ok(_))) {
                    r.servers.friends = Some(got);
                }
                cx.notify();
            }
        });
    })
    .detach();
}

/// How many of the friends found are in this server.
fn friends_in(sv: &Data, job: &str) -> usize {
    match &sv.friends {
        Some(Ok(v)) => v.get("friends").and_then(Value::as_array).map_or(0, |l| l.iter().filter(|f| s(f, "jobId") == job).count()),
        _ => 0,
    }
}

/// A quiet line in place of a part's cards: what is being read, or why there is nothing.
fn said(text: String, looking: bool) -> Div {
    div()
        .w_full()
        .px(px(16.))
        .py(px(18.))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(10.))
        .rounded(px(R_L))
        .bg(c(PANEL))
        .border_1()
        .border_color(c(LINE))
        .text_size(px(T_M))
        .text_color(c(MUTED))
        .when(looking, |d| d.child(spinner(14., c(MUTED))))
        .child(text)
}

/// A friend's face at its exact size, a rounded square (a mark when there is no picture).
/// A friend's face, round as on the Friends page; while Roblox gives no picture, their first letter on a colour of their
/// own (a grey person icon on every card read as a missing picture, review 3b.3).
fn face_at(url: &str, name: &str, user: i64, size: f32, cx: &mut App) -> Div {
    let pic = Some(url).filter(|u| !u.is_empty()).and_then(|u| crate::images::pic_sq(u, size, cx));
    let first: String = name.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().collect()).unwrap_or_else(|| "?".into());
    let hue = (user.unsigned_abs() % 360) as f32 / 360.;
    div().size(px(size)).flex_none().rounded_full().overflow_hidden().bg(c(RAISED_2)).flex().items_center().justify_center().map(|d| match pic {
        Some(p) => d.child(img(p).size_full().object_fit(ObjectFit::Cover)),
        None => d.bg(hsla(hue, 0.38, 0.40, 1.)).text_color(c(WHITE)).text_size(px((size * 0.38).round())).font_weight(FontWeight::BOLD).child(first),
    })
}

/// Friends in this game: each a card with their face, name, whose friend they are, and Join (their server, in Play's
/// row), on the page's grid.
fn friends(root: &Root, width: f32, cx: &mut Context<Root>) -> (Option<usize>, AnyElement) {
    let sv = &root.servers;
    let answer = match &sv.friends {
        None => return (None, said(root.tr("srv.friendsLooking"), true).into_any_element()),
        Some(Err(())) => return (None, said(root.tr("srv.friendsError"), false).into_any_element()),
        Some(Ok(v)) => v,
    };
    let list: Vec<Value> = answer.get("friends").and_then(Value::as_array).cloned().unwrap_or_default();
    if list.is_empty() {
        let asked = answer.get("asked").and_then(Value::as_i64).unwrap_or(0) > 0;
        return (Some(0), said(root.tr(if asked { "srv.friendsNone" } else { "srv.friendsNoAccount" }), false).into_any_element());
    }
    let (_, card_w) = grid_fit(width, 300., GRID_GAP);
    let cards: Vec<AnyElement> = list
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let user = f.get("userId").and_then(Value::as_i64).unwrap_or(0);
            let name = [s(f, "displayName"), s(f, "name")].into_iter().find(|n| !n.is_empty()).map_or_else(|| format!("#{user}"), str::to_string);
            let handle = s(f, "name");
            // (@name and whose friend on lines of their own: on one line "friend of Main" was cut, review 3b.2.)
            let at = (!handle.is_empty()).then(|| format!("@{handle}"));
            let whose = root.i18n.tv("srv.friendOf", &[("acc", s(f, "via"))]);
            let job = s(f, "jobId").to_string();
            // (Their own place, as the web page's f.placeId || placeId: a friend can be in one of the game's sub-places.)
            let their_place = f.get("placeId").and_then(Value::as_i64).filter(|p| *p > 0);
            let tip: SharedString = root.i18n.tv("srv.useFriendTip", &[("name", &name)]).into();
            block()
                    .w(px(card_w))
                    .p(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child({
                        // (No picture in the answer: the face the Friends page already has of them, if any.)
                        let known = match &root.friends.list {
                            Some(Ok(l)) => l.iter().find(|x| x.id == user).map(|x| x.face.clone()).unwrap_or_default(),
                            _ => String::new(),
                        };
                        let url = Some(s(f, "avatarUrl")).filter(|u| !u.is_empty()).map(str::to_string).unwrap_or(known);
                        face_at(&url, &name, user, 44., cx)
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(div().truncate().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(name))
                            .children(at.map(|a| div().truncate().text_size(px(T_S)).text_color(c(MUTED)).child(a)))
                            .child(div().truncate().text_size(px(T_S)).text_color(c(TEXT_2)).child(whose)),
                    )
                    .child(copy_button(root, ("srv-friend-copy", i), &job, cx))
                    .child(
                        div().id(("srv-friend", i)).flex_none().tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()).child(
                            Button::new(("srv-friend-join", i), root.tr("srv.join")).icon("play").small().disabled(job.is_empty()).on_click(cx.listener(move |r, _, _, cx| {
                                crate::pages::games::pick_for_server(r, job.clone());
                                r.games.pick_place = their_place;
                                cx.notify();
                            })),
                        ),
                    )
            .into_any_element()
        })
        .collect();
    (Some(list.len()), grid(GRID_GAP).children(cards).into_any_element())
}

fn open_vip_checkout(root: &mut Root, place: i64, id: String, cx: &mut Context<Root>) {
    let Some(link) = root.link.clone() else { return };
    if root.servers.vip_opening {
        return;
    }
    let shown_place = root.servers.place;
    root.servers.create_private = false;
    root.servers.vip_opening = true;
    root.servers.vip_error = None;
    let rx = link.call("openGameServers", json!({ "placeId": place, "id": id }));
    answer(rx, cx, move |r, reply, cx| {
        if r.servers.place != shown_place {
            return;
        }
        r.servers.vip_opening = false;
        match reply {
            Ok(Ok(_)) => r.servers.vip_owner = Some(id),
            Ok(Err(e)) => r.servers.vip_error = Some(e.to_string()),
            Err(_) => r.servers.vip_error = Some(r.tr("gp.createPrivateError")),
        }
        cx.notify();
    });
    cx.notify();
}

/// A server's Job ID to the clipboard, said in a toast.
fn copy_job(r: &mut Root, job: &str, cx: &mut Context<Root>) {
    cx.write_to_clipboard(ClipboardItem::new_string(job.to_string()));
    r.toast = Some((r.tr("srv.jobCopied"), "ok".into()));
    cx.notify();
}

/// Copies a server's Job ID (a button beside its Join).
fn copy_button(root: &Root, id: impl Into<ElementId>, job: &str, cx: &mut Context<Root>) -> IconButton {
    let job = job.to_string();
    IconButton::new(id, "copy").size(30.).disabled(job.is_empty()).tip(root.tr("srv.copyJob")).on_click(cx.listener(move |r, _, _, cx| copy_job(r, &job, cx)))
}

/// The servers the accounts were in lately in this game, newest first (the one an account is in now on top, marked),
/// as cards like the friends' above: the account's face and name, when (or "In it now"), the Job ID, Copy and Join.
/// None when there is none.
fn recent(root: &Root, root_place: i64, width: f32, cx: &mut Context<Root>) -> Option<AnyElement> {
    /// The accounts' faces.
    const AV: f32 = 44.;
    let place = crate::pages::games::shown_place(root)?;
    let list = root.games.recent_servers.get(&place).filter(|l| !l.is_empty())?;
    let now = crate::time::now_secs();
    let (_, card_w) = grid_fit(width, 300., GRID_GAP);
    let cards: Vec<AnyElement> = list
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let job = s(v, "job").to_string();
            let at_place = v.get("place").and_then(Value::as_i64).filter(|p| *p > 0 && *p != root_place);
            let here = v.get("now").and_then(Value::as_bool).unwrap_or(false);
            let at = v.get("at").and_then(Value::as_i64).unwrap_or(0) / 1000;
            // Every one of the app's accounts that was in it: their faces overlapped (three at most), their names.
            let who: Vec<&Value> = v.get("accounts").and_then(Value::as_array).map(|l| l.iter().collect()).unwrap_or_default();
            let names: Vec<String> = who.iter().map(|w| s(w, "name").to_string()).collect();
            let name = match names.len() {
                0 => "–".to_string(),
                1 | 2 => names.join(", "),
                n => format!("{} +{}", names[0], n - 1),
            };
            let all_names: SharedString = names.join(", ").into();
            let faces: Vec<AnyElement> = who
                .iter()
                .take(3)
                .enumerate()
                .map(|(k, w)| {
                    let face = match root.accounts().iter().find(|a| s(a, "id") == s(w, "id")) {
                        Some(a) => crate::pages::accounts::face(a, AV, block_ground(), cx),
                        None => div().size(px(AV)).rounded_full().bg(ca(WHITE, 0.08)).flex().items_center().justify_center().child(icon("user", 18., c(MUTED))),
                    };
                    div().flex_none().when(k > 0, |d| d.ml(px(-AV * 0.4))).child(face).into_any_element()
                })
                .collect();
            let short: String = job.chars().take(8).collect();
            let full: SharedString = job.clone().into();
            let join = job.clone();
            let when = if here {
                div().flex_none().h(px(20.)).px(px(7.)).flex().items_center().gap(px(5.)).rounded(px(6.)).bg(ca(ONLINE, 0.16)).text_size(px(T_XS)).font_weight(FontWeight::SEMIBOLD).text_color(c(ONLINE)).child(icon("play", 10., c(ONLINE))).child(root.tr("srv.inNow"))
            } else {
                div().flex_none().text_size(px(T_S)).text_color(c(TEXT_2)).child(crate::time::ago_far(now - at, &root.i18n))
            };
            block()
                .w(px(card_w))
                .p(px(12.))
                .flex()
                .items_center()
                .gap(px(12.))
                .child(div().flex_none().flex().items_center().children(faces))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .child(div().id(("srv-recent-who", i)).truncate().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(name).when(names.len() > 1, |d| d.tooltip(move |_, cx| cx.new(|_| Tip(all_names.clone())).into())))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(when)
                                .child(
                                    div()
                                        .id(("srv-recent-job", i))
                                        .min_w_0()
                                        .flex()
                                        .items_center()
                                        .gap(px(4.))
                                        .text_size(px(T_S))
                                        .font_features(tnum())
                                        .text_color(c(MUTED))
                                        .child(icon("server", 12., c(DIM)))
                                        .child(div().truncate().child(short))
                                        .tooltip(move |_, cx| cx.new(|_| Tip(full.clone())).into()),
                                ),
                        ),
                )
                .child(copy_button(root, ("srv-recent-copy", i), &job, cx))
                .child(Button::new(("srv-recent-join", i), root.tr("srv.join")).icon("play").small().disabled(job.is_empty()).on_click(cx.listener(move |r, _, _, cx| {
                    crate::pages::games::pick_for_server(r, join.clone());
                    r.games.pick_place = at_place;
                    cx.notify();
                })))
                .into_any_element()
        })
        .collect();
    Some(
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(section_head(root.tr("srv.recentTitle"), Some(list.len())).child(
                Button::new("srv-recent-clear", root.tr("srv.recentClear")).small().quiet().icon("trash").on_click(cx.listener(move |r, _, _, cx| {
                    crate::pages::games::clear_recent_servers(r, place);
                    cx.notify();
                })),
            ))
            .child(grid(GRID_GAP).children(cards))
            .into_any_element(),
    )
}

pub fn view(root: &Root, root_place: i64, width: f32, cx: &mut Context<Root>) -> AnyElement {
    let (n_friends, friends_body) = friends(root, width, cx);
    let n_private = match &root.servers.got {
        Some(Ok(v)) => v.get("servers").and_then(Value::as_array).map(Vec::len),
        _ => None,
    };
    let place = if root_place > 0 { root_place } else { root.servers.place };
    let eligible = friends::able_ref(root);
    let only = (eligible.len() == 1).then(|| s(eligible[0], "id").to_string());
    let private_head = section_head(root.tr("srv.privateTitle"), n_private.filter(|n| *n > 0))
        .flex_wrap().gap(px(8.)).when(place > 0, |head| {
            let tip = root.tr(if eligible.is_empty() { "gp.createPrivateNoAccount" } else { "gp.createPrivateTip" });
            head.child(div().flex().items_center().gap(px(8.))
                .child(IconButton::new("srv-refresh-private", "refresh").size(30.)
                    .disabled(root.servers.loading || eligible.is_empty()).tip(root.tr("gp.refreshPrivate"))
                    .on_click(cx.listener(|r, _, _, cx| {
                        r.servers.private_fresh = true;
                        want(r, r.servers.place, cx);
                        cx.notify();
                    })))
                .child(Button::new("srv-create-private", root.tr("gp.createPrivate")).small().icon("plus")
                    .disabled(eligible.is_empty() || root.servers.vip_opening).tip(tip).on_click(cx.listener(move |r, _, window, cx| {
                        if let Some(id) = only.clone() {
                            start_buy(r, place, id, window, cx);
                        } else {
                            r.servers.create_private = !r.servers.create_private;
                            r.servers.vip_error = None;
                            cx.notify();
                        }
                    }))))
        });
    let picker = (root.servers.create_private && !eligible.is_empty()).then(|| {
        let accounts = eligible.into_iter().map(|account| {
            let id = s(account, "id").to_string();
            Button::new(SharedString::from(format!("srv-create-{id}")), crate::pages::accounts::label(account)).small()
                .on_click(cx.listener(move |r, _, window, cx| start_buy(r, place, id.clone(), window, cx)))
        });
        div().flex().flex_col().gap(px(8.)).p(px(12.)).rounded(px(R_M)).bg(c(PANEL))
            .child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).child(root.tr("gp.createPrivateAccount")))
            .child(div().flex().flex_wrap().gap(px(8.)).children(accounts))
    });
    let error = root.servers.vip_error.as_ref().map(|e| div().text_size(px(T_M)).text_color(c(BAD)).child(e.clone()));
    let buying = root.servers.buy.as_ref().filter(|b| b.place == place).map(|b| buy_box(root, b, cx));
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(SECTION_GAP))
        .child(div().flex().flex_col().gap(px(12.)).child(section_head(root.tr("srv.friendsTitle"), n_friends.filter(|n| *n > 0))).child(friends_body))
        .children(recent(root, root_place, width, cx))
        // Private before public: the public list runs long with more pages on demand, and the private servers
        // under it were out of sight.
        .child(div().flex().flex_col().gap(px(12.)).child(private_head)
            .when(n_private.is_some_and(|n| n > 8) || root.servers.search.as_ref().is_some_and(|e| !crate::kit::input_text(e, cx).is_empty()), |d| {
                d.children(root.servers.search.as_ref().map(|e| div().max_w(px(320.)).w_full().child(field(e, Some("search")).h(px(34.)))))
            })
            .children(picker).children(buying).children(error).child(private(root, width, cx)))
        .child(public(root, width, cx))
        .into_any_element()
}

fn public(root: &Root, width: f32, cx: &mut Context<Root>) -> Div {
    let sv = &root.servers;
    let sort = if sv.sort.is_empty() { "most" } else { sv.sort };
    let full = |v: &Value| {
        let (n, most) = (v.get("playing").and_then(Value::as_i64).unwrap_or(0), v.get("maxPlayers").and_then(Value::as_i64).unwrap_or(0));
        most > 0 && n >= most
    };
    // The accounts that would go in together: those picked that can play.
    let party = root.accounts().iter().filter(|a| crate::pages::tabs::play_selected(root, s(a, "id")) && can_join_game(a)).count().max(1) as i64;
    let room = party > 1 && sv.room.unwrap_or(true);
    let mut rows: Vec<&Value> = sv
        .rows
        .iter()
        .filter(|v| !room || v.get("maxPlayers").and_then(Value::as_i64).unwrap_or(0) - v.get("playing").and_then(Value::as_i64).unwrap_or(0) >= party)
        .collect();
    if sort == "ping" {
        rows.sort_by_key(|v| v.get("ping").and_then(Value::as_i64).filter(|p| *p > 0).unwrap_or(i64::MAX));
    }
    if sort == "fps" {
        rows.sort_by(|a, b| b.get("fps").and_then(Value::as_f64).unwrap_or(0.).total_cmp(&a.get("fps").and_then(Value::as_f64).unwrap_or(0.)));
    }
    let fact = |v: &Value| sv.facts.get(s(v, "jobId")).filter(|f| f.is_object());
    if sort == "new" {
        // (A server whose start Roblox didn't say goes after those it did.)
        rows.sort_by_key(|v| std::cmp::Reverse(fact(v).and_then(|f| f.get("started")).and_then(Value::as_i64).unwrap_or(0)));
    }
    // The country picked: only its servers.
    if let Some(cc) = &sv.country {
        rows.retain(|v| fact(v).is_some_and(|f| s(f, "cc") == cc));
    }
    // Full servers can't be joined: they go last (the sort keeps the rest in order).
    rows.sort_by_key(|v| full(v));
    let waiting = sv.wait_until.map(|t| t.saturating_duration_since(Instant::now()).as_secs().max(1));

    // The sorts as the page's filters: no track, the one used a yellow block.
    let mut sorts = div().flex().flex_wrap().items_center().gap(px(4.));
    for key in ["most", "fewest", "ping", "fps", "new"] {
        sorts = sorts.child(
            chip(SharedString::from(format!("srv-sort-{key}")), sort == key)
                .h(px(TOOL_H))
                .px(px(12.))
                .cursor_pointer()
                .child(root.tr(&format!("srv.sort.{key}")))
                .on_click(cx.listener(move |r, _, _, cx| set_sort(r, key, cx))),
        );
    }
    if party > 1 {
        sorts = sorts.child(
            chip("srv-room", room)
                .h(px(TOOL_H))
                .px(px(12.))
                .gap(px(6.))
                .cursor_pointer()
                .child(icon("users", 14., c(if room { INK } else { MUTED })))
                .child(root.i18n.tv("srv.room", &[("n", &party.to_string())]))
                .tooltip({
                    let tip: SharedString = root.i18n.tv("srv.roomTip", &[("n", &party.to_string())]).into();
                    move |_, cx| cx.new(|_| Tip(tip.clone())).into()
                })
                .on_click(cx.listener(move |r, _, _, cx| {
                    r.servers.room = Some(!room);
                    cx.notify();
                })),
        );
    }
    // The countries the servers loaded run in, most servers first: a chip each (its flag and name) to see only those.
    let mut lands: Vec<(String, usize)> = Vec::new();
    for f in sv.facts.values().filter(|f| f.is_object()) {
        let cc = s(f, "cc");
        if cc.is_empty() {
            continue;
        }
        match lands.iter_mut().find(|l| l.0 == cc) {
            Some(l) => l.1 += 1,
            None => lands.push((cc.to_string(), 1)),
        }
    }
    lands.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let countries = (lands.len() > 1 || sv.country.is_some()).then(|| {
        let mut row = div().flex().flex_wrap().items_center().gap(px(4.)).child(
            chip("srv-cc-any", sv.country.is_none())
                .h(px(TOOL_H))
                .px(px(12.))
                .cursor_pointer()
                .child(root.tr("srv.anyCountry"))
                .on_click(cx.listener(|r, _, _, cx| {
                    r.servers.country = None;
                    cx.notify();
                })),
        );
        for (i, (cc, n)) in lands.iter().enumerate() {
            let on = sv.country.as_deref() == Some(cc.as_str());
            let to = cc.clone();
            row = row.child(
                chip(("srv-cc", i), on)
                    .h(px(TOOL_H))
                    .px(px(10.))
                    .gap(px(6.))
                    .cursor_pointer()
                    .child(crate::pages::games::flag(cc, 18., cx))
                    .child(crate::pages::games::country(root, cc))
                    .child(div().font_features(tnum()).text_color(c(if on { INK } else { MUTED })).child(n.to_string()))
                    .on_click(cx.listener(move |r, _, _, cx| {
                        r.servers.country = if r.servers.country.as_deref() == Some(to.as_str()) { None } else { Some(to.clone()) };
                        cx.notify();
                    })),
            );
        }
        row
    });
    let open: Vec<String> = rows.iter().filter(|v| !full(v)).map(|v| s(v, "jobId").to_string()).filter(|j| !j.is_empty()).collect();
    let head = section_head(root.tr("srv.publicTitle"), (!rows.is_empty()).then_some(rows.len())).flex_wrap().gap(px(12.)).child(sorts).child(
        // A server with room, any of them.
        Button::new("srv-random", root.tr("srv.random")).icon("shuffle").small().disabled(open.is_empty()).on_click(cx.listener(move |r, _, _, cx| {
            if open.is_empty() {
                return;
            }
            let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos() as usize) % open.len();
            crate::pages::games::pick_for_server(r, open[at].clone());
            cx.notify();
        })),
    );
    let body = if rows.is_empty() {
        if let Some(n) = waiting {
            said(root.i18n.tv("srv.wait", &[("n", &n.to_string())]), false).into_any_element()
        } else if sv.failed {
            said(root.tr("srv.loadError"), false).child(Button::new("srv-retry", root.tr("srv.retry")).small().on_click(cx.listener(|r, _, _, cx| load_public(r, false, cx)))).into_any_element()
        } else if sv.more || !sv.asked {
            said(root.tr("av.reading"), true).into_any_element()
        } else if room && !sv.rows.is_empty() {
            // Servers there are, none with a seat for every account picked.
            said(root.i18n.tv("srv.noneRoom", &[("n", &party.to_string())]), false).into_any_element()
        } else {
            said(root.tr("srv.none"), false).into_any_element()
        }
    } else {
        // Each server a card: how full it is, large, with its bar; the ping and frame rate under it; friends there; Join.
        // (Only the lines near the view are made once there are many: each More adds a page of Roblox's.)
        let (_, card_w) = grid_fit(width, 232., GRID_GAP);
        let key = crate::near::sig((rows.len(), rows.first().map(|v| s(v, "jobId").to_string())));
        let mut card = |i: usize| {
                let v = &rows[i];
                let (n, most) = (v.get("playing").and_then(Value::as_i64).unwrap_or(0), v.get("maxPlayers").and_then(Value::as_i64).unwrap_or(0));
                let is_full = full(v);
                let ping = v.get("ping").and_then(Value::as_i64).unwrap_or(0);
                let fps = v.get("fps").and_then(Value::as_f64).unwrap_or(0.);
                let job = s(v, "jobId").to_string();
                let here = friends_in(sv, &job);
                let f = sv.facts.get(&job).filter(|f| f.is_object());
                let cc = f.map(|f| s(f, "cc").to_string()).filter(|c| !c.is_empty());
                let city = f.map(|f| s(f, "city").to_string()).unwrap_or_default();
                let started = f.and_then(|f| f.get("started")).and_then(Value::as_i64).filter(|t| *t > 0);
                let old = f.and_then(|f| f.get("version")).and_then(Value::as_i64).is_some_and(|n| n > 0 && sv.newest > n);
                let mut line: Vec<(String, Option<Hsla>)> = Vec::new();
                if let Some(t) = started {
                    let secs = (crate::time::now_secs() * 1000 - t) / 1000;
                    line.push((root.i18n.tv("srv.age", &[("t", &crate::time::span(secs))]), None));
                }
                if old {
                    line.push((root.tr("srv.old"), Some(c(WARN))));
                }
                if ping > 0 {
                    line.push((root.i18n.tv("srv.ping", &[("ms", &ping.to_string())]), None));
                }
                if fps > 0. {
                    line.push((root.i18n.tv("srv.fpsVal", &[("fps", &(fps.round() as i64).to_string())]), None));
                }
                let join = job.clone();
                block()
                        .w(px(card_w))
                        .p(px(14.))
                        .flex()
                        .flex_col()
                        .gap(px(10.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(icon("users", 16., c(if is_full { WARN } else { MUTED })))
                                .child(div().text_size(px(T_FIG)).line_height(px(28.)).font_weight(FontWeight::SEMIBOLD).font_features(tnum()).text_color(c(if is_full { WARN } else { TEXT })).child(n.to_string()))
                                .when(most > 0, |d| d.child(div().text_size(px(T_M)).font_features(tnum()).text_color(c(MUTED)).child(format!("/ {most}"))))
                                .child(div().flex_1())
                                // Where it runs: the flag and the city (the country when there is no city).
                                .when_some(cc.clone(), |d, cc| {
                                    let name = if city.is_empty() { crate::pages::games::country(root, &cc) } else { city.clone() };
                                    d.child(div().flex_none().max_w(px(120.)).flex().items_center().gap(px(6.)).child(crate::pages::games::flag(&cc, 18., cx)).child(div().min_w_0().truncate().text_size(px(T_S)).text_color(c(MUTED)).child(name)))
                                })
                                // Your friends in this server, how many.
                                .when(here > 0, |d| {
                                    let tip: SharedString = root.tr("srv.friendsHere").into();
                                    d.child(
                                        div()
                                            .id(("srv-friends-here", i))
                                            .flex_none()
                                            .h(px(22.))
                                            .px(px(8.))
                                            .flex()
                                            .items_center()
                                            .gap(px(5.))
                                            .rounded(px(7.))
                                            .bg(ca(ONLINE, 0.14))
                                            .text_size(px(T_S))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(c(ONLINE))
                                            .child(icon("users", 12., c(ONLINE)))
                                            .child(here.to_string())
                                            .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()),
                                    )
                                }),
                        )
                        .child(bar(("srv-fill", i), if most > 0 { n as f32 / most as f32 } else { 0. }, None))
                        // (Two lines high whether the facts take one or two: Join sits level across the cards.)
                        .child(
                            div()
                                .h(px(40.))
                                .flex()
                                .items_center()
                                .gap(px(10.))
                                .child(if line.is_empty() {
                                    div().flex_1().min_w_0().truncate().text_size(px(T_S)).text_color(c(DIM)).font_features(tnum()).child(job.chars().take(8).collect::<String>()).into_any_element()
                                } else {
                                    // What the ping and the frame rate are (Servers.tsx's titles): the players', not yours.
                                    let tip: SharedString = [(ping > 0).then(|| root.tr("srv.pingTip")), (fps > 0.).then(|| root.tr("srv.fpsTip"))].into_iter().flatten().collect::<Vec<_>>().join("\n").into();
                                    div().id(("srv-facts", i)).flex_1().min_w_0().child(facts(line).text_size(px(T_S))).tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()).into_any_element()
                                })
                                .child(copy_button(root, ("srv-copy", i), &job, cx))
                                .child(Button::new(("srv-join", i), root.tr("srv.join")).icon("play").small().disabled(is_full || job.is_empty()).tip(if is_full { root.tr("srv.full") } else { String::new() }).on_click(cx.listener(move |r, _, _, cx| {
                                    crate::pages::games::pick_for_server(r, join.clone());
                                    cx.notify();
                                }))),
                        )
                .into_any_element()
            };
        crate::near::tiles_in(Some(&root.games.page_scroll), "srv-cards", rows.len(), card_w, GRID_GAP, key, true, 128., &mut card).into_any_element()
    };
    // More of them: Roblox's next page (it may ask to wait first).
    let more = (!rows.is_empty() && sv.next.is_some()).then(|| {
        let label = match waiting {
            Some(n) => root.i18n.tv("srv.moreIn", &[("n", &n.to_string())]),
            None => root.tr("srv.more"),
        };
        div().pt(px(4.)).flex().justify_center().child(Button::new("srv-more", label).small().disabled(waiting.is_some() || sv.more).on_click(cx.listener(|r, _, _, cx| load_public(r, true, cx))))
    });
    // Roblox blocks the game where the account is: no server's country can be told (said once, under the head).
    let blocked = sv.facts_blocked.then(|| said(root.tr("rg.blocked"), false));
    div().flex().flex_col().gap(px(12.)).child(head).children(countries).children(blocked).child(body).children(more)
}

fn private(root: &Root, width: f32, cx: &mut Context<Root>) -> AnyElement {
    let sv = &root.servers;
    let answer = match &sv.got {
        None => return said(root.tr("av.reading"), true).into_any_element(),
        Some(Err(why)) => return said(root.i18n.tv("in.failed", &[("why", crate::net::why_words(&root.i18n, why).as_str())]), false).into_any_element(),
        Some(Ok(v)) => v,
    };
    let list = answer.get("servers").and_then(Value::as_array).cloned().unwrap_or_default();
    if list.is_empty() {
        let failed = answer.get("failed").and_then(Value::as_bool) == Some(true);
        return said(root.tr(if failed { "ps.failed" } else { "ps.none" }), false).into_any_element();
    }
    // The accounts in use first (see in_use below). Their own servers lead, then the servers
    // they can join, then the other accounts' own servers (Roblox's order within each); then only the ones the search
    // words are found in: the name, the owner or an account that can join.
    let mine: std::collections::HashSet<i64> = root.accounts().iter().filter_map(|a| a.get("userId").and_then(Value::as_i64)).filter(|u| *u > 0).collect();
    let own = |v: &Value| v.get("ownerId").and_then(Value::as_i64).is_some_and(|o| mine.contains(&o));
    // (The account whose Roblox tab is shown first: these servers are seen in it. Then the ones open in Roblox, then
    // the ones picked.)
    let tab_account = root.tabs.list.get(root.tabs.at).and_then(|t| t.account.clone());
    let in_tab: Vec<&Value> = root.accounts().iter().filter(|a| tab_account.as_deref() == Some(s(a, "id"))).collect();
    let open_now: Vec<&Value> = root.accounts().iter().filter(|a| crate::pages::accounts::is_local(a)).collect();
    let in_use: Vec<&Value> = if !in_tab.is_empty() {
        in_tab
    } else if !open_now.is_empty() {
        open_now
    } else {
        root.accounts().iter().filter(|a| root.selected.contains(s(a, "id"))).collect()
    };
    let use_ids: std::collections::HashSet<String> = in_use.iter().map(|a| s(a, "id").to_string()).collect();
    let use_users: std::collections::HashSet<i64> = in_use.iter().filter_map(|a| a.get("userId").and_then(Value::as_i64)).filter(|u| *u > 0).collect();
    let via_use = |v: &Value| v.get("viaIds").and_then(Value::as_array).is_some_and(|l| l.iter().filter_map(Value::as_str).any(|id| use_ids.contains(id)));
    let rank = |v: &Value| {
        if v.get("ownerId").and_then(Value::as_i64).is_some_and(|o| use_users.contains(&o)) {
            0
        } else if via_use(v) {
            1
        } else if own(v) {
            2
        } else {
            3
        }
    };
    let mut list = list;
    list.sort_by_key(|v| rank(v));
    // On each card the accounts in use come first among those that can join.
    for v in &mut list {
        let (Some(ids), Some(names)) = (v.get("viaIds").and_then(Value::as_array).cloned(), v.get("via").and_then(Value::as_array).cloned()) else { continue };
        let mut pairs: Vec<(Value, Value)> = ids.into_iter().zip(names).collect();
        pairs.sort_by_key(|(id, _)| !id.as_str().is_some_and(|id| use_ids.contains(id)));
        let (ids, names): (Vec<Value>, Vec<Value>) = pairs.into_iter().unzip();
        v["viaIds"] = Value::Array(ids);
        v["via"] = Value::Array(names);
    }
    let words = sv.search.as_ref().map(|e| crate::kit::input_text(e, cx).to_lowercase()).unwrap_or_default();
    if !words.is_empty() {
        list.retain(|v| {
            let via = v.get("via").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str);
            [s(v, "name"), s(v, "owner"), s(v, "ownerName")].into_iter().chain(via).any(|t| t.to_lowercase().contains(&words))
        });
        if list.is_empty() {
            return said(root.tr("srv.privateNoMatch"), false).into_any_element();
        }
    }
    let place = sv.place;
    let (_, card_w) = grid_fit(width, 300., GRID_GAP);
    // How many accounts a card shows before "+N".
    const JOINS_SHOWN: usize = 3;
    let cards: Vec<AnyElement> = list
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let playing = v.get("playing").and_then(Value::as_i64).unwrap_or(0);
            let most = v.get("maxPlayers").and_then(Value::as_i64).unwrap_or(0);
            let full = most > 0 && playing >= most;
            let running = v.get("running").and_then(Value::as_bool).unwrap_or(false);
            let server_id = v.get("id").and_then(Value::as_i64).unwrap_or(i as i64);
            let code = s(v, "accessCode").to_string();
            let owner_id = v.get("ownerId").and_then(Value::as_i64).unwrap_or(0);
            let is_own = own(v);
            // An own server's owner is one of the accounts: named as the app names it (hidden names stay hidden).
            let owner_account = if is_own { root.accounts().iter().find(|a| a.get("userId").and_then(Value::as_i64) == Some(owner_id)).cloned() } else { None };
            let owner = owner_account.as_ref().map_or_else(|| s(v, "owner").to_string(), |a| s(a, "label").to_string());
            let names: Vec<String> = v.get("via").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect();
            let ids: Vec<String> = v.get("viaIds").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect();
            let all = ids.len();
            let open = sv.priv_open.contains(&server_id) || all <= JOINS_SHOWN + 1;
            // A chip for each account that can join: its face and name, and it joins.
            let joins: Vec<AnyElement> = ids
                .into_iter()
                .zip(names)
                .take(if open { usize::MAX } else { JOINS_SHOWN })
                .enumerate()
                .map(|(n, (id, name))| {
                    let account = root.accounts().iter().find(|a| s(a, "id") == id).cloned();
                    let free = account.as_ref().is_some_and(can_join_game);
                    let ok = !full && free && !code.is_empty();
                    let input = format!("roblox://experiences/start?placeId={place}&accessCode={code}");
                    let (url, user) = account.as_ref().map_or((String::new(), 0), |a| (s(a, "avatarUrl").to_string(), a.get("userId").and_then(Value::as_i64).unwrap_or(0)));
                    let chip = hot(("ps-join", i * 64 + n))
                        .h(px(30.))
                        .max_w(px(170.))
                        .pl(px(4.))
                        .pr(px(10.))
                        .flex()
                        .items_center()
                        .gap(px(7.))
                        .rounded_full()
                        .bg(ca(WHITE, 0.08))
                        .text_size(px(T_S))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c(if ok { TEXT } else { MUTED }))
                        .child(face_at(&url, &name, user, 22., cx))
                        .child(div().min_w_0().truncate().child(name))
                        .child(icon("play", 11., c(if ok { TEXT } else { DIM })));
                    if ok {
                        chip.cursor_pointer()
                            .hover(|d| d.bg(ca(WHITE, 0.14)))
                            .on_click(cx.listener(move |r, _, _, cx| {
                                r.play_at(&[id.clone()], input.clone(), cx);
                            }))
                            .into_any_element()
                    } else {
                        chip.opacity(0.6).into_any_element()
                    }
                })
                .collect();
            let more = (!open).then(|| {
                hot(("ps-more", i))
                    .h(px(30.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .rounded_full()
                    .bg(ca(WHITE, 0.05))
                    .cursor_pointer()
                    .hover(|d| d.bg(ca(WHITE, 0.1)))
                    .text_size(px(T_S))
                    .text_color(c(TEXT_2))
                    .child(format!("+{}", all - JOINS_SHOWN))
                    .on_click(cx.listener(move |r, _, _, cx| {
                        r.servers.priv_open.insert(server_id);
                        cx.notify();
                    }))
            });
            // What it is now: full, someone in it, or nobody; the accounts' own: "Yours".
            let tag = |words: String, tone: u32, strong: bool| {
                div()
                    .flex_none()
                    .h(px(22.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .rounded(px(7.))
                    .bg(ca(tone, if strong { 0.18 } else { 0.08 }))
                    .text_size(px(T_XS))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(c(tone))
                    .child(words)
            };
            let state = if full {
                tag(root.tr("srv.privFull"), WARN, true)
            } else if running {
                tag(root.tr("srv.privRunning"), OK, true)
            } else {
                tag(root.tr("srv.privEmpty"), MUTED, false)
            };
            let face_url = owner_account.as_ref().map_or_else(|| s(v, "ownerPicture").to_string(), |a| s(a, "avatarUrl").to_string());
            let owner_face = div()
                .relative()
                .flex_none()
                .child(face_at(&face_url, &owner, owner_id, 44., cx))
                .child(div().absolute().right(px(-3.)).bottom(px(-3.)).size(px(20.)).rounded_full().bg(c(GROUND)).flex().items_center().justify_center().child(icon("key", 11., c(FACE))));
            let title = Some(s(v, "name")).filter(|n| !n.trim().is_empty()).map_or_else(|| root.tr("srv.privateUnnamed"), str::to_string);
            block()
                .w(px(card_w))
                .p(px(14.))
                .flex()
                .flex_col()
                .gap(px(12.))
                .when(is_own, |d| d.border_1().border_color(ca(FACE, 0.45)))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .child(owner_face)
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(div().truncate().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(title))
                                .child(div().truncate().text_size(px(T_S)).text_color(c(MUTED)).child(root.i18n.tv("srv.byOwner", &[("name", &owner)]))),
                        )
                        .when(is_own, |d| d.child(tag(root.tr("srv.privYours"), FACE, true))),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .child(div().text_size(px(T_FIG)).line_height(px(26.)).font_weight(FontWeight::SEMIBOLD).font_features(tnum()).text_color(c(if full { WARN } else { TEXT })).child(playing.to_string()))
                                .when(most > 0, |d| d.child(div().text_size(px(T_M)).font_features(tnum()).text_color(c(MUTED)).child(root.i18n.tv("srv.ofMax", &[("n", &most.to_string())]))))
                                .child(div().flex_1())
                                .child(state),
                        )
                        .child(bar(("ps-fill", i), if most > 0 { playing as f32 / most as f32 } else { 0. }, None)),
                )
                .child(
                    div()
                        .mt_auto()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(px(8.))
                        .child(div().text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("srv.joinWith")))
                        .children(joins)
                        .children(more),
                )
                .into_any_element()
        })
        .collect();
    // Each row as tall as its tallest card: stretched by the row, not h_full (a share of the whole list's height: with
    // many servers every card grew to it and the list ran over the public servers under it).
    grid(GRID_GAP).items_stretch().children(cards).into_any_element()
}
