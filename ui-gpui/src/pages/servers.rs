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

use gpui_kit::{prelude::*, *};
use serde_json::{json, Value};

use crate::{
    kit::*,
    pages::{accounts::can_play, friends},
    shell::Root,
    theme::*,
};

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or_default()
}

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
    friend_ids: Vec<String>,
    /// "Room for all": only servers with a seat for every account picked (Servers.tsx's room); None: on when more
    /// than one is picked.
    room: Option<bool>,
    /// The page Roblox said to wait for was a next one (asked again by itself then, the rows so far kept).
    wait_more: bool,
    /// How many more lists Roblox gives this minute (None: not known): a page for ping or FPS only while one is left
    /// for the next click (Servers.tsx's spare).
    left: Option<i64>,
}

/// Ping and FPS are sorted here, over the rows loaded.
fn local(sort: &str) -> bool {
    sort == "ping" || sort == "fps"
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
    if r.servers.loading {
        return;
    }
    // The accounts asked: the picked ones (Servers.tsx's ids), else the signed-in ones; the app's limit of them (six).
    let able = friends::able_ref(r);
    let picked: Vec<String> = able.iter().filter(|a| r.selected.contains(s(a, "id"))).take(6).map(|a| s(a, "id").to_string()).collect();
    let ids: Vec<String> = if picked.is_empty() { able.iter().take(6).map(|a| s(a, "id").to_string()).collect() } else { picked };
    if r.servers.private_ids == ids && r.servers.at.is_some_and(|t| t.elapsed() < FRESH) {
        return;
    }
    let Some(link) = r.link.clone() else { return };
    r.servers.loading = true;
    r.servers.private_ids = ids.clone();
    let rx = link.call("privateServers", json!({ "placeId": place, "ids": ids }));
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

pub fn view(root: &Root, width: f32, cx: &mut Context<Root>) -> AnyElement {
    let (n_friends, friends_body) = friends(root, width, cx);
    let n_private = match &root.servers.got {
        Some(Ok(v)) => v.get("servers").and_then(Value::as_array).map(Vec::len),
        _ => None,
    };
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(SECTION_GAP))
        .child(div().flex().flex_col().gap(px(12.)).child(section_head(root.tr("srv.friendsTitle"), n_friends.filter(|n| *n > 0))).child(friends_body))
        // Private before public (owner, 2026-11-13): the public list runs long with more pages on demand, and the
        // private servers under it were out of sight.
        .child(div().flex().flex_col().gap(px(12.)).child(section_head(root.tr("srv.privateTitle"), n_private.filter(|n| *n > 0))).child(private(root, width, cx)))
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
    let party = root.accounts().iter().filter(|a| root.selected.contains(s(a, "id")) && crate::pages::accounts::can_play(a)).count().max(1) as i64;
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
    // Full servers can't be joined: they go last (the sort keeps the rest in order).
    rows.sort_by_key(|v| full(v));
    let waiting = sv.wait_until.map(|t| t.saturating_duration_since(Instant::now()).as_secs().max(1));

    // The sorts as the page's filters: no track, the one used a yellow block.
    let mut sorts = div().flex().flex_wrap().items_center().gap(px(4.));
    for key in ["most", "fewest", "ping", "fps"] {
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
                let mut line: Vec<(String, Option<Hsla>)> = Vec::new();
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
                        .child(
                            div()
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
                                .child(Button::new(("srv-join", i), root.tr("srv.join")).icon("play").small().disabled(is_full || job.is_empty()).tip(if is_full { root.tr("srv.full") } else { String::new() }).on_click(cx.listener(move |r, _, _, cx| {
                                    crate::pages::games::pick_for_server(r, join.clone());
                                    cx.notify();
                                }))),
                        )
                .into_any_element()
            };
        crate::near::tiles("srv-cards", rows.len(), card_w, GRID_GAP, key, true, 128., &mut card).into_any_element()
    };
    // More of them: Roblox's next page (it may ask to wait first).
    let more = (!rows.is_empty() && sv.next.is_some()).then(|| {
        let label = match waiting {
            Some(n) => root.i18n.tv("srv.moreIn", &[("n", &n.to_string())]),
            None => root.tr("srv.more"),
        };
        div().pt(px(4.)).flex().justify_center().child(Button::new("srv-more", label).small().disabled(waiting.is_some() || sv.more).on_click(cx.listener(|r, _, _, cx| load_public(r, true, cx))))
    });
    div().flex().flex_col().gap(px(12.)).child(head).child(body).children(more)
}

fn private(root: &Root, width: f32, cx: &mut Context<Root>) -> AnyElement {
    let sv = &root.servers;
    let answer = match &sv.got {
        None => return said(root.tr("av.reading"), true).into_any_element(),
        Some(Err(why)) => return said(root.i18n.tv("in.failed", &[("why", if why.is_empty() { "Roblox" } else { why })]), false).into_any_element(),
        Some(Ok(v)) => v,
    };
    let list = answer.get("servers").and_then(Value::as_array).cloned().unwrap_or_default();
    if list.is_empty() {
        let failed = answer.get("failed").and_then(Value::as_bool) == Some(true);
        return said(root.tr(if failed { "ps.failed" } else { "ps.none" }), false).into_any_element();
    }
    let place = sv.place;
    let (_, card_w) = grid_fit(width, 340., GRID_GAP);
    let cards: Vec<AnyElement> = list
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let playing = v.get("playing").and_then(Value::as_i64).unwrap_or(0);
            let most = v.get("maxPlayers").and_then(Value::as_i64).unwrap_or(0);
            let full = most > 0 && playing >= most;
            let code = s(v, "accessCode").to_string();
            let names: Vec<String> = v.get("via").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect();
            let ids: Vec<String> = v.get("viaIds").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect();
            // A button for each account that can join: its name, and it joins.
            let joins = ids.into_iter().zip(names).enumerate().map(|(n, (id, name))| {
                // (An account already playing, or with a sign-in that ran out, can't be started.)
                let free = root.accounts().iter().find(|a| s(a, "id") == id).is_some_and(can_play);
                let input = format!("roblox://experiences/start?placeId={place}&accessCode={code}");
                Button::new(("ps-join", i * 16 + n), name).small().icon("play").disabled(full || !free || code.is_empty()).on_click(cx.listener(move |r, _, _, cx| {
                    r.play_at(&[id.clone()], input.clone(), cx);
                }))
            });
            let count = if most > 0 { format!("{playing}/{most}") } else { playing.to_string() };
            let tone = if full { WARN } else { TEXT_2 };
            block()
                    .w(px(card_w))
                    .h_full()
                    .p(px(14.))
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .child(div().flex_none().child(icon("key", 20., c(TEXT_2))))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.))
                                    .child(div().truncate().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(Some(s(v, "name")).filter(|n| !n.trim().is_empty()).map_or_else(|| root.tr("srv.privateUnnamed"), str::to_string)))
                                    .child(div().truncate().text_size(px(T_S)).text_color(c(MUTED)).child(s(v, "owner").to_string())),
                            )
                            .child(div().flex_none().flex().items_center().gap(px(6.)).text_size(px(T_M)).font_features(tnum()).text_color(c(tone)).child(icon("users", 14., c(tone))).child(count)),
                    )
                    .child(div().flex().flex_wrap().gap(px(8.)).children(joins))
            .into_any_element()
        })
        .collect();
    grid(GRID_GAP).children(cards).into_any_element()
}
