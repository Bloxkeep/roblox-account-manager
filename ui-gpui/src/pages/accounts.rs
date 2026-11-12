// Accounts, with the Roblox windows (the old Game monitor) where the accounts are: what needs a look on top, then a
// lane for each game played here with a tile a window (its live picture laid there by the app; Roblox windows not
// opened by Bloxkeep too), then the accounts not open. Any tile or card is a tick box;
// the bar at the foot plays, rejoins or closes those picked. The commands are the app's (launch, closeAccounts,
// hideWindows, showWindows, monitorAct, cancelRejoin, resolveGame, openProfile, openAppeal), as the web page sends them.
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use crate::{images, kit::*, shell::Root, theme::*};

// The list layout, the account menu (its state and the page's other state Root doesn't hold), and the start for new
// users (no accounts yet, or none opened yet).
mod list;
mod menu;
mod start;

/// The filters over the list, as the web page's chips.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    All,
    Playing,
    Online,
    Offline,
    /// Signed out: they need signing in again.
    Invalid,
    Banned,
}

fn s<'a>(a: &'a Value, k: &str) -> &'a str {
    a.get(k).and_then(Value::as_str).unwrap_or_default()
}
fn conn(a: &Value) -> &str {
    a.pointer("/session/connection").and_then(Value::as_str).unwrap_or_default()
}
pub fn is_local(a: &Value) -> bool {
    s(a, "state") == "running"
}
fn launching(a: &Value) -> bool {
    s(a, "state") == "launching"
}
pub fn is_kicked(a: &Value) -> bool {
    is_local(a) && (conn(a) == "disconnected" || a.pointer("/session/bannedByGame").and_then(Value::as_bool).unwrap_or(false))
}
fn presence(a: &Value) -> &str {
    a.pointer("/presence/type").and_then(Value::as_str).unwrap_or_default()
}
fn in_game(a: &Value) -> bool {
    (is_local(a) && !matches!(conn(a), "disconnected" | "menu" | "signedout")) || presence(a) == "ingame"
}
fn is_online(a: &Value) -> bool {
    is_local(a) || matches!(presence(a), "online" | "ingame" | "studio")
}
fn signed_out(a: &Value) -> bool {
    a.get("cookieValid").and_then(Value::as_bool) == Some(false)
}
fn banned(a: &Value) -> bool {
    a.get("banned").and_then(Value::as_bool).unwrap_or(false)
}
pub fn label(a: &Value) -> String {
    ["label", "alias", "displayName", "username"].iter().map(|k| s(a, k)).find(|n| !n.is_empty()).unwrap_or_default().to_string()
}
/// Free to join a game: not open, signed in, not banned (as the Accounts bar's Play).
pub fn can_play(a: &Value) -> bool {
    !is_local(a) && !launching(a) && !signed_out(a) && !banned(a)
}
fn id_of(a: &Value) -> String {
    s(a, "id").to_string()
}

fn passes(a: &Value, f: Filter, query: &str) -> bool {
    let hit = query.is_empty() || ["alias", "displayName", "username", "group", "note", "label"].iter().any(|k| s(a, k).to_lowercase().contains(query));
    hit && match f {
        Filter::All => true,
        Filter::Playing => in_game(a),
        Filter::Online => is_online(a),
        Filter::Offline => !is_online(a),
        Filter::Invalid => signed_out(a),
        Filter::Banned => banned(a),
    }
}

/// The order picked (the app's "sort" setting, as the web page kept it): by name, by what they are doing, by when
/// they were last opened, or in the launch order; names break ties.
fn sort(root: &Root, list: &mut [Value]) {
    let how = root.state.pointer("/settings/sort").and_then(Value::as_str).unwrap_or("name").to_string();
    let order: Vec<String> = root.state.pointer("/settings/launchOrder").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    let place = |a: &Value| order.iter().position(|i| i == s(a, "id")).unwrap_or(usize::MAX);
    let rank = |a: &Value| if in_game(a) { 0 } else if launching(a) { 1 } else if is_online(a) { 2 } else { 3 };
    list.sort_by(|x, y| {
        let first = match how.as_str() {
            "status" => rank(x).cmp(&rank(y)),
            "recent" => s(y, "lastUsed").cmp(s(x, "lastUsed")),
            "order" => place(x).cmp(&place(y)),
            _ => std::cmp::Ordering::Equal,
        };
        first.then_with(|| folded(&label(x)).cmp(&folded(&label(y))))
    });
}

/// Why an account dropped, from Roblox's disconnect code (roblox_codes.rs: every code Roblox has).
fn kick_words(root: &Root, code: i64) -> String {
    match crate::roblox_codes::why(code as i32) {
        // Roblox's own box says no more than "an error" for these, and a number it doesn't have says nothing: the code.
        Some(c) if c.key != "error" => root.tr(&format!("kick.{}", c.key)),
        _ => root.tr("kick.code").replace("{code}", &code.to_string()),
    }
}

/// The colour of an account's dot: dropped or opening orange, playing green, online blue, else grey.
fn dot_of(a: &Value) -> u32 {
    if is_kicked(a) || launching(a) {
        WARN
    } else if in_game(a) {
        OK
    } else if is_online(a) {
        ONLINE
    } else {
        DIM
    }
}

fn game_name(a: &Value) -> Option<String> {
    a.pointer("/session/game/name")
        .and_then(Value::as_str)
        .filter(|g| !g.is_empty())
        .or_else(|| (presence(a) == "ingame").then(|| a.pointer("/presence/game").and_then(Value::as_str)).flatten())
        .map(crate::pages::home::clean)
}
fn game_place(a: &Value) -> Option<i64> {
    a.pointer("/session/game/placeId").and_then(Value::as_i64).or_else(|| a.pointer("/presence/placeId").and_then(Value::as_i64)).filter(|p| *p > 0)
}
fn game_icon(a: &Value) -> Option<String> {
    a.pointer("/session/game/iconUrl")
        .and_then(Value::as_str)
        .or_else(|| a.pointer("/presence/iconUrl").and_then(Value::as_str))
        .filter(|u| !u.is_empty())
        .map(str::to_string)
}

fn phase(a: &Value) -> &str {
    a.pointer("/session/phase").and_then(Value::as_str).unwrap_or_default()
}
/// Held by Roblox's bot check on the way in: the window stays open on its Security page.
fn held(a: &Value) -> bool {
    is_local(a) && phase(a) == "challenged"
}
/// Auto-rejoin waiting its turn (longer after several drops in a row), or a preset's restart about to open it again.
fn rejoin_wait(a: &Value) -> Option<&Value> {
    a.get("rejoin").filter(|r| r.is_object() && !is_local(a))
}
/// Auto-rejoin for the account's client: a preset that opened it may turn it on or off for that client.
fn rejoin_on(a: &Value) -> bool {
    a.pointer("/session/autoRejoin").and_then(Value::as_bool).or_else(|| a.get("autoRejoin").and_then(Value::as_bool)).unwrap_or(false)
}
/// A window on an older Roblox than the newest.
fn old_build(a: &Value) -> bool {
    is_local(a) && a.pointer("/session/oldBuild").and_then(Value::as_bool).unwrap_or(false)
}
/// The FPS limit Bloxkeep set when this window opened.
fn fps_limit(a: &Value) -> Option<i64> {
    a.pointer("/session/fpsLimit").and_then(Value::as_i64).filter(|f| *f > 0 && is_local(a))
}
fn pid_of(a: &Value) -> Option<i64> {
    a.pointer("/session/pid").and_then(Value::as_i64).filter(|p| *p > 0)
}

/// A countdown in whole minutes, rounded up, then "in a moment" for the last one (a rejoin waiting its turn).
fn until(root: &Root, iso: &str, tpl: &str, soon: &str) -> String {
    let left = crate::time::iso_secs(iso).map(|t| t - crate::time::now_secs()).unwrap_or(0);
    if left < 60 { root.tr(soon) } else { root.i18n.tv(tpl, &[("n", &((left + 59) / 60).to_string())]) }
}

/// What it does now, in a few words, its colour, and whether the game's name goes after it. A window here says how its
/// join goes (the web page's Activity): teleporting, waiting for a server, a full server and its place in line, held
/// by Roblox's bot check, didn't get in, dropped and why.
fn state_of(root: &Root, a: &Value) -> (String, u32, bool) {
    let now = crate::time::now_secs();
    if launching(a) {
        return (root.tr("act.opening"), WARN, false);
    }
    if let Some(rj) = rejoin_wait(a) {
        let mut text = if rj.get("restart").and_then(Value::as_bool).unwrap_or(false) { root.tr("rj.rowRestart") } else { until(root, s(rj, "at"), "rj.rowMin", "rj.rowSoon") };
        if let Some(g) = rj.pointer("/game/name").and_then(Value::as_str).filter(|g| !g.is_empty()) {
            text = format!("{text} · {}", crate::pages::home::clean(g));
        }
        return (text, WARN, false);
    }
    if is_local(a) {
        // The game's own words when it gave any ("Banned by the game: cheating").
        let said = |why: String| match a.pointer("/session/kickMessage").and_then(Value::as_str).filter(|m| !m.is_empty()) {
            Some(m) => format!("{why}: {m}"),
            None => why,
        };
        if a.pointer("/session/bannedByGame").and_then(Value::as_bool).unwrap_or(false) {
            return (said(root.tr("act.bannedByGame")), BAD, true);
        }
        if conn(a) == "disconnected" {
            let why = a.pointer("/session/reason").and_then(Value::as_i64).map(|code| kick_words(root, code)).unwrap_or_else(|| root.tr("row.kickedUnknown"));
            return (said(why), WARN, true);
        }
        return match (conn(a), phase(a)) {
            // Left before any server took it (a join error's Leave, or a wait given up).
            ("menu", "joinFailed") => (root.tr("act.joinFailed"), WARN, true),
            ("menu", _) => (root.tr("act.menuHere"), TEXT_2, false),
            ("signedout", _) => (root.tr("act.signedOutHere"), BAD, false),
            ("joining", "challenged") => (root.tr("act.botCheck"), WARN, true),
            ("joining", "full") => {
                let line = a.pointer("/session/queuePosition").and_then(Value::as_i64).filter(|n| *n > 0);
                let text = match line {
                    Some(n) => format!("{} · {}", root.tr("row.full"), root.i18n.tv("act.inLine", &[("n", &n.to_string())])),
                    None => root.tr("act.serverFull"),
                };
                (text, WARN, true)
            }
            (_, "teleporting") => (root.tr("act.teleporting"), OK, true),
            ("joining", "waiting") => (root.tr("act.waitServer"), OK, true),
            ("ingame", _) => (root.tr("act.playingHere"), OK, true),
            _ => (root.tr("act.joiningHere"), OK, true),
        };
    }
    if presence(a) == "ingame" {
        (root.tr("row.elsewhere"), TEXT_2, true)
    } else if presence(a) == "online" {
        (root.tr("act.online"), TEXT_2, false)
    } else if banned(a) {
        (root.tr("card.banned"), BAD, false)
    } else if signed_out(a) {
        (root.tr("card.expired"), BAD, false)
    } else {
        let last = a.get("lastUsed").and_then(Value::as_str).and_then(crate::time::iso_secs);
        (
            match last {
                Some(t) => root.i18n.tv("card.opened", &[("t", &crate::time::ago(now - t, &root.i18n))]),
                None => root.tr("card.never"),
            },
            MUTED,
            false,
        )
    }
}

/// What it does now with the game after it, as one line (a card's, a list row's); a window here whose game Roblox
/// hasn't named yet says it is loading it.
fn line_of(root: &Root, a: &Value) -> (String, u32) {
    let (state, tone, with_game) = state_of(root, a);
    let text = match (with_game, game_name(a)) {
        (true, Some(g)) => format!("{state} · {g}"),
        (true, None) if is_local(a) => format!("{state} · {}", root.tr("act.gameLoading")),
        _ => state,
    };
    (text, tone)
}

/// The Robux on an account ("…" while the first count is on its way).
fn robux_of(root: &Root, a: &Value) -> Option<String> {
    match a.get("robux").and_then(Value::as_i64) {
        Some(n) => Some(crate::pages::games::thousands(n, root.i18n.vietnamese)),
        None => a.get("robuxWait").and_then(Value::as_bool).filter(|w| *w).map(|_| "…".to_string()),
    }
}
fn robux_el(root: &Root, id: &str, text: String) -> Stateful<Div> {
    let tip: SharedString = root.tr("card.robuxTip").into();
    div()
        .id(SharedString::from(format!("robux-{id}")))
        .flex()
        .flex_none()
        .items_center()
        .gap(px(4.))
        .text_size(px(T_S))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c(TEXT_2))
        .font_features(tnum())
        .child(icon("robux", 13., c(MUTED)))
        .child(text)
        .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
}

/// A small word on a card or row (Main, VNG, 60 FPS, a group, Old build), in a colour.
fn tag(text: impl Into<SharedString>, col: u32) -> Div {
    div()
        .h(px(20.))
        .px(px(7.))
        .flex()
        .flex_none()
        .items_center()
        .rounded(px(6.))
        .bg(if col == TEXT_2 { ca(WHITE, 0.08) } else { ca(col, 0.14) })
        .text_color(c(col))
        .text_size(px(T_XS))
        .font_weight(FontWeight::SEMIBOLD)
        .whitespace_nowrap()
        .child(text.into())
}

/// "Old build" on a window on an older Roblox: a click goes to where Roblox is updated (Settings › Roblox).
fn old_tag(root: &Root, a: &Value, cx: &mut Context<Root>) -> Stateful<Div> {
    let tip: SharedString = root.i18n.tv("card.oldTip", &[("v", a.pointer("/session/robloxVersion").and_then(Value::as_str).unwrap_or_default())]).into();
    div()
        .id(SharedString::from(format!("old-{}", s(a, "id"))))
        .flex_none()
        .cursor_pointer()
        .child(tag(root.tr("card.old"), WARN))
        .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(|r, _, _, cx| {
            r.settings.part = crate::pages::settings::Part::Roblox;
            r.go(crate::pages::Page::Settings);
            cx.notify();
        }))
}

/// The process id, for tools that pick a Roblox window by it; a click copies it.
fn pid_el(root: &Root, a: &Value, cx: &mut Context<Root>) -> Option<Stateful<Div>> {
    let pid = pid_of(a)?;
    let id = id_of(a);
    let tip: SharedString = root.tr("card.pidTip").into();
    let copy_id = id.clone();
    Some(
        div()
            .id(SharedString::from(format!("pid-{id}")))
            .flex_none()
            .text_color(c(MUTED))
            .font_features(tnum())
            .when(!foreign(a), |d| {
                d.cursor_pointer()
                    .hover(|d| d.text_color(c(TEXT)))
                    .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |r, _, _, cx| copy(r, copy_id.clone(), "pid", cx)))
            })
            .child(format!("PID {pid}")),
    )
}

/// An account's main button (the web page's card and row button): Verify while Roblox holds it on a bot check,
/// Rejoin when dropped, Close while open, Cancel rejoin while one waits, Appeal when banned, Sign in when signed out,
/// else Play.
fn action(root: &Root, a: &Value, at: &str, cx: &mut Context<Root>) -> Option<Button> {
    if foreign(a) {
        return None;
    }
    let id = id_of(a);
    let key = SharedString::from(format!("{at}-act-{id}"));
    let b = if held(a) {
        Button::new(key, root.tr("card.botCheck")).icon("shield").tint().on_click(cx.listener(move |r, _, _, _| r.send("solveBotCheck", json!({ "id": id }))))
    } else if is_kicked(a) {
        let me = vec![a.clone()];
        Button::new(key, root.tr("card.rejoinNow")).icon("repeat").tint().on_click(cx.listener(move |r, _, _, cx| r.rejoin(&me, cx)))
    } else if is_local(a) || launching(a) {
        Button::new(key, root.tr("card.close")).icon("power").on_click(cx.listener(move |r, _, _, cx| close(r, vec![id.clone()], false, cx)))
    } else if rejoin_wait(a).is_some() {
        Button::new(key, root.tr("rj.cancel")).icon("x").on_click(cx.listener(move |r, _, _, _| r.send("cancelRejoin", json!({ "id": id }))))
    } else if banned(a) {
        // Play skips it, so its button is Roblox's appeal page, signed in as it.
        Button::new(key, root.tr("card.appeal")).icon("external").on_click(cx.listener(move |r, _, _, _| r.send("openAppeal", json!({ "id": id }))))
    } else if signed_out(a) {
        let vng = s(a, "realm") == "vng";
        Button::new(key, root.tr("card.relogin")).icon("logIn").tint().on_click(cx.listener(move |r, _, window, cx| crate::pages::quick::open(r, vng, window, cx)))
    } else {
        Button::new(key, root.tr("card.play")).icon("play").on_click(cx.listener(move |r, _, _, cx| r.play(std::slice::from_ref(&id), cx)))
    };
    Some(b.small())
}

/// Something pressed on a card that must not pick the card too.
fn no_pick(el: impl IntoElement) -> Div {
    div().flex_none().on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()).child(el)
}

/// An account's ⋮: its menu, under it.
fn more_button(root: &Root, a: &Value, at: &str, size: f32, cx: &mut Context<Root>) -> Div {
    let id = id_of(a);
    let open = menu::open_here(&id, true, cx);
    let key = SharedString::from(format!("{at}-more-{id}"));
    no_pick(IconButton::new(key, "more").size(size).on(open).tip(root.tr("card.moreTip")).on_click(cx.listener(move |_, _, _, cx| {
        menu::open_button(id.clone(), cx);
        cx.notify();
    })))
    .relative()
    .children(menu::place(root, a, true, cx))
}

/// A right click on a card, tile or row: its menu, where the mouse is.
fn on_right(a: &Value, cx: &mut Context<Root>) -> impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static {
    let id = id_of(a);
    cx.listener(move |_, e: &MouseDownEvent, _, cx| {
        menu::open_at(id.clone(), e.position, cx);
        cx.notify();
    })
}

fn up_of(a: &Value) -> Option<String> {
    (is_local(a) && !is_kicked(a))
        .then(|| a.pointer("/session/since").and_then(Value::as_str).and_then(crate::time::iso_secs).map(|t| crate::time::clock(crate::time::now_secs() - t)))
        .flatten()
}
/// The ring round an account's face (the web page's .avatar background): green playing, blue online, red signed out
/// or banned, amber cut off or opening, else a quiet line.
fn ring_of(a: &Value) -> Hsla {
    if is_kicked(a) || launching(a) {
        c(WARN)
    } else if in_game(a) {
        c(OK)
    } else if is_online(a) {
        c(ONLINE)
    } else if signed_out(a) || banned(a) {
        ca(BAD, 0.6)
    } else {
        ca(WHITE, 0.12)
    }
}

/// An account's face as the web page drew it (.avatar): the picture inside a coloured ring of its state, a thin edge
/// of the surface between them, and the state's dot at the corner; its first letter on a colour of its own while it
/// has no picture. `ring` is the surface it sits on.
pub fn avatar(a: &Value, size: f32, ring: u32, cx: &mut App) -> Div {
    let pad = if size >= 34. { 2. } else { 1.5 };
    let edge = if size >= 34. { 2. } else { 1.5 };
    let inner = size - 2. * pad - 2. * edge;
    let url = s(a, "avatarUrl");
    let face = images::pic_sq(url, inner, cx);
    let r = (size * 0.3).round();
    let d = (size * 0.26).clamp(9., 14.);
    let letter = (url.is_empty()).then(|| {
        let name = label(a);
        let first: String = name.trim().chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_else(|| "?".into());
        // A hue of its own (FNV-1a over the id), as the web page's --h.
        let h = s(a, "id").bytes().fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619));
        (first, hsla((h % 360) as f32 / 360., 0.38, 0.40, 1.))
    });
    div()
        .relative()
        .size(px(size))
        .flex_none()
        .rounded(px(r))
        .p(px(pad))
        .bg(ring_of(a))
        .child(
            div()
                .size_full()
                .rounded(px((r - pad).max(2.)))
                .border(px(edge))
                .border_color(c(ring))
                .bg(c(RAISED_2))
                .flex()
                .items_center()
                .justify_center()
                .map(|x| match (face, letter) {
                    (Some(f), _) => x.child(img(f).size_full().object_fit(ObjectFit::Cover).rounded(px((r - pad - edge).max(2.)))),
                    (None, Some((first, bg))) => {
                        x.bg(bg).text_color(c(WHITE)).text_size(px((inner * 0.45).round())).font_weight(FontWeight::BOLD).child(first)
                    }
                    (None, None) => x,
                }),
        )
        .child(div().absolute().right(px(-2.)).bottom(px(-2.)).size(px(d)).rounded(px(d * 0.35)).border_2().border_color(c(ring)).bg(c(dot_of(a))))
}

/// One grid for the whole page: window tiles and cards share its columns (each at least this wide, the row ending
/// where the page does), so every section lines up: four across at 1707 px, three at 1180.
const COL_MIN: f32 = 330.;
/// The page's sides (the header's and the body's), on the 8 px rhythm.
const SIDE_PAD: f32 = 24.;
/// How far a block's base shows under it.
const RISE: f32 = 4.;

/// A Roblox window not opened by Bloxkeep: shown with the accounts' windows, as the old Game monitor did.
fn foreign(a: &Value) -> bool {
    a.get("foreign").and_then(Value::as_bool).unwrap_or(false)
}

/// The window the app last sent for this account (by its process), or the foreign one itself.
fn window_of<'a>(root: &'a Root, a: &Value) -> Option<&'a Value> {
    if foreign(a) {
        let h = s(a, "hwnd");
        return root.windows.iter().find(|w| s(w, "id") == h);
    }
    let pid = a.pointer("/session/pid").and_then(Value::as_i64).filter(|p| *p > 0)?;
    root.windows.iter().find(|w| w.get("pid").and_then(Value::as_i64) == Some(pid))
}

/// The windows Bloxkeep didn't open, as accounts of their own (named as the app names them, "Roblox"), so they sit in
/// their game's lane; "No game" is the Roblox menu.
fn others(root: &Root) -> Vec<Value> {
    let no_game = root.tr("mon.noGame");
    root.windows
        .iter()
        .filter(|w| w.get("ours").and_then(Value::as_bool) == Some(false))
        .map(|w| {
            let game = s(w, "game");
            let status = if game == no_game || game.is_empty() { "menu" } else { s(w, "status") };
            json!({
                "id": format!("win-{}", s(w, "id")), "hwnd": s(w, "id"), "label": s(w, "name"), "state": "running", "foreign": true,
                "session": { "pid": w.get("pid"), "connection": status, "game": { "name": game, "iconUrl": w.get("iconUrl") } },
            })
        })
        .collect()
}

fn ram_mb(root: &Root, a: &Value) -> Option<f64> {
    if let Some(mb) = window_of(root, a).and_then(|w| w.get("ramMb")).and_then(Value::as_f64).filter(|m| *m > 0.) {
        return Some(mb);
    }
    let id = s(a, "id");
    is_local(a)
        .then(|| root.metrics.get("accounts").and_then(Value::as_array).and_then(|l| l.iter().find(|m| s(m, "id") == id)).and_then(|m| m.get("ramMb")?.as_f64()))
        .flatten()
}
/// RAM in MB or GB, with Vietnamese's decimal comma ("1,7 GB").
fn mb_text(mb: f64, vi: bool) -> String {
    let t = if mb >= 1024. { format!("{:.1} GB", mb / 1024.) } else { format!("{mb:.0} MB") };
    if vi { t.replace('.', ",") } else { t }
}
fn hidden(a: &Value) -> bool {
    a.pointer("/session/hidden").and_then(Value::as_bool).unwrap_or(false)
}
fn cover_of(root: &Root, place: Option<i64>) -> Option<String> {
    place.and_then(|p| root.looks.get(&p)).and_then(|l| l.get("cover")).and_then(Value::as_str).filter(|u| !u.is_empty()).map(str::to_string)
}

/// Picks or drops one account (a card or a tile is a tick box: the bar below acts on all those picked).
fn toggle(r: &mut Root, id: &str) {
    if !r.selected.remove(id) {
        r.selected.insert(id.to_string());
    }
}

/// The Roblox windows of one game: the old Game monitor's lane, now beside the accounts they belong to.
struct Lane {
    name: String,
    place: Option<i64>,
    icon: Option<String>,
    items: Vec<Value>,
    kicked: usize,
    ram: f64,
}

/// The open accounts by game: a game with one cut off first, then the most windows; the Roblox menu ("No game") last.
fn lanes(root: &Root, shown: &[Value]) -> Vec<Lane> {
    let no_game = root.tr("mon.noGame");
    let loading = root.tr("act.gameLoading");
    let mut out: Vec<Lane> = Vec::new();
    for a in shown.iter().filter(|a| is_local(a) || launching(a)) {
        let menu = is_local(a) && conn(a) == "menu";
        let place = if menu { None } else { game_place(a) };
        // (A window on its way into a game Roblox hasn't named yet: the name is on its way too.)
        let unnamed = if is_local(a) && !foreign(a) && !matches!(conn(a), "disconnected" | "signedout") { &loading } else { &no_game };
        let name = if menu { no_game.clone() } else { game_name(a).unwrap_or_else(|| unnamed.clone()) };
        // A window known only by its game's name (not opened by Bloxkeep) joins the lane of that name.
        let i = match out.iter().position(|l| l.name == name && (l.place == place || l.place.is_none() || place.is_none())) {
            Some(i) => {
                out[i].place = out[i].place.or(place);
                i
            }
            None => {
                out.push(Lane { name, place, icon: None, items: Vec::new(), kicked: 0, ram: 0. });
                out.len() - 1
            }
        };
        let l = &mut out[i];
        l.items.push(a.clone());
        l.kicked += is_kicked(a) as usize;
        l.ram += ram_mb(root, a).unwrap_or(0.);
        if l.icon.is_none() && !menu {
            l.icon = game_icon(a);
        }
    }
    out.sort_by(|a, b| {
        (b.kicked > 0)
            .cmp(&(a.kicked > 0))
            .then(a.place.is_none().cmp(&b.place.is_none()))
            .then(b.items.len().cmp(&a.items.len()))
            .then(a.name.cmp(&b.name))
    });
    out
}

/// One place for everything an account is doing. On top what needs a look (signed out, banned); then a lane for each
/// game the accounts play here, each window a tile (its picture, face, name, time, RAM); then the accounts not open.
/// A tile or a card is a tick box: the bar at the foot acts on those picked (play, rejoin, close).
/// The cards the page shows: the search, the filter and the group applied, sorted, other PCs' after.
fn shown_accounts(root: &Root, cx: &App) -> Vec<Value> {
    let query = root.acc_search.read(cx).value().trim().to_lowercase();
    let all = root.accounts();
    // (A group picked that no account is in any more counts as none.)
    let group = root.acc_group.clone().filter(|g| all.iter().any(|a| s(a, "group") == g));
    let mut shown: Vec<Value> = all.iter().filter(|a| passes(a, root.acc_filter, &query)).filter(|a| group.as_deref().is_none_or(|g| s(a, "group") == g)).cloned().collect();
    sort(root, &mut shown);
    shown.extend(others(root).into_iter().filter(|a| passes(a, root.acc_filter, &query)));
    shown
}

/// A key on the Accounts page (none typed into a box, no dialog open), as the old page had them.
#[derive(Clone, Copy)]
pub enum Key {
    /// Ctrl+A: every card shown picked.
    All,
    /// Delete: the picked ones deleted, asked first.
    Delete,
    /// F2: the one picked edited.
    Edit,
    /// Enter: the picked ones played.
    Play,
    /// Escape: none picked.
    Clear,
}

/// Does what the key says; false when it says nothing here (the key then goes on as usual).
pub fn key(r: &mut Root, k: Key, window: &mut Window, cx: &mut Context<Root>) -> bool {
    let picked: Vec<Value> = r.accounts().iter().filter(|a| r.selected.contains(s(a, "id"))).cloned().collect();
    let own: Vec<String> = picked.iter().filter(|a| !foreign(a)).map(id_of).collect();
    match k {
        Key::All => {
            let ids: Vec<String> = shown_accounts(r, cx).iter().map(id_of).collect();
            if ids.is_empty() {
                return false;
            }
            r.selected.extend(ids);
        }
        Key::Clear => {
            if menu::is_open(cx) {
                menu::shut(cx);
            } else if r.acc_menu {
                r.acc_menu = false;
            } else if !r.selected.is_empty() {
                r.selected.clear();
            } else {
                return false;
            }
        }
        Key::Delete => {
            if own.is_empty() {
                return false;
            }
            let names = picked.iter().filter(|a| !foreign(a)).map(label).collect();
            ask_delete(r, own, names, window, cx);
        }
        Key::Edit => {
            if own.len() != 1 || picked.len() != 1 {
                return false;
            }
            crate::pages::account_form::open(r, Some(own[0].clone()), window, cx);
        }
        Key::Play => {
            if own.is_empty() {
                return false;
            }
            r.play(&own, cx);
        }
    }
    cx.notify();
    true
}

/// Asks before deleting these accounts (their names, five at most, in the question), then deletes them.
fn ask_delete(r: &mut Root, ids: Vec<String>, names: Vec<String>, window: &mut Window, cx: &mut Context<Root>) {
    let n = ids.len() as i64;
    let list = format!("{}{}", names.iter().take(5).cloned().collect::<Vec<_>>().join(", "), if names.len() > 5 { ", …" } else { "" });
    let (title, text) = (r.i18n.tn("t.deleteTitle", n, &[("n", &n.to_string())]), r.i18n.tn("t.deleteQ", n, &[("n", &n.to_string()), ("list", &list)]));
    r.confirm(title, text, r.tr("t.deleteOk"), true, None, window, cx, move |r, cx| {
        let gone = ids.clone();
        act(r, "deleteAccounts", json!({ "ids": ids }), cx, move |r, v| {
            for id in &gone {
                r.selected.remove(id);
            }
            let n = v.as_i64().unwrap_or(0);
            r.toast = Some((r.i18n.tn("t.deleted", n, &[("n", &n.to_string())]), "ok".into()));
        });
    });
}

/// The layout shown: cards or a list, as picked here or as the app's settings keep it ("layout", as app.js sent it).
fn as_list(root: &Root, cx: &mut App) -> bool {
    let kept = root.state.pointer("/settings/layout").and_then(Value::as_str) == Some("list");
    let picked = cx.try_global::<menu::Ui>().and_then(|u| u.list);
    // (Once the settings say the same, they lead again: another vault brings its own.)
    if picked == Some(kept) {
        cx.default_global::<menu::Ui>().list = None;
    }
    picked.unwrap_or(kept)
}

pub fn accounts(root: &Root, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    #[cfg(feature = "shot")]
    menu::shot_hooks(root, cx);
    #[cfg(feature = "shot")]
    shot_place(cx);
    let all = root.accounts().to_vec();
    let shown = shown_accounts(root, cx);
    let picked: Vec<Value> = all.iter().filter(|a| root.selected.contains(s(a, "id"))).cloned().collect();
    let inner = width - 2. * SIDE_PAD;
    let rows = as_list(root, cx);

    let mut body = div().flex().flex_col().gap(px(SECTION_GAP)).px(px(SIDE_PAD)).pt(px(8.)).pb(px(32.));
    // Getting started (add an account, pick a game, select and Play), until Bloxkeep has opened Roblox for one.
    if let Some(steps) = start::steps(root, true) {
        body = body.child(steps);
    }
    if rows {
        // The Roblox windows' live pictures over the list too (the old Game monitor; the list had none, so the owner
        // couldn't find them, 2026-11-12), then one line an account.
        body = body.children(windows(root, &shown, inner, cx));
        let own: Vec<&Value> = shown.iter().filter(|a| !foreign(a)).collect();
        if !own.is_empty() {
            body = body.child(list::rows(root, &own, width < 1300., cx));
        }
    } else {
        body = cards(root, body, &shown, inner, cx);
    }
    if shown.is_empty() && !all.is_empty() {
        body = body.child(div().pt(px(24.)).text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("nores.title")));
    }

    div()
        .size_full()
        .rounded(px(R_PANEL))
        .bg(c(PANEL))
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(header(root, &all, width, rows, cx))
        .child(if all.is_empty() {
            start::empty(root, cx).into_any_element()
        } else {
            // The live pictures show only inside the scrolling part (never over the header or the bar).
            div()
                .flex_1()
                .min_h_0()
                .relative()
                .child(root.live.area())
                .child(crate::scrolled::body("acc-body", body))
                .into_any_element()
        })
        .when(!picked.is_empty(), |d| d.child(bar(root, &picked, width, cx)))
}

/// A shot's bar with a game in its box (BLOXKEEP_SHOT_PLACE=<place id>) and, with BLOXKEEP_SHOT_JOB=1, the Job ID box
/// open with a server in it: set once, just after this frame (a box needs the window).
#[cfg(feature = "shot")]
fn shot_place(cx: &mut Context<Root>) {
    let Ok(place) = std::env::var("BLOXKEEP_SHOT_PLACE") else { return };
    if cx.try_global::<menu::Ui>().is_some_and(|u| u.shot_place_done) {
        return;
    }
    let Some(h) = cx.windows().first().copied() else { return };
    cx.default_global::<menu::Ui>().shot_place_done = true;
    let this = cx.weak_entity();
    cx.spawn(async move |_, cx| {
        let _ = cx.update_window(h, |_, window, cx| {
            let _ = this.update(cx, |r, cx| {
                r.place.update(cx, |st, cx| st.set_value(place.clone(), window, cx));
                r.resolve_place(cx);
                if std::env::var("BLOXKEEP_SHOT_JOB").is_ok() {
                    let hint = r.tr("dock.job");
                    let e = cx.new(|cx| gpui_kit::component::input::InputState::new(window, cx).placeholder(hint));
                    e.update(cx, |st, cx| st.set_value("1f0a3c2e-77d1-4b8e-9c5a-2d6e8f9a0b1c", window, cx));
                    let ui = cx.default_global::<menu::Ui>();
                    ui.job = Some(e);
                    ui.job_open = true;
                }
                cx.notify();
            });
        });
    })
    .detach();
}

/// The Roblox windows open now, a block per game with their live pictures (the old Game monitor), for the cards and the
/// list alike: the games flow on the page's grid, each taking as many columns as it has windows (all of them at most,
/// wrapping inside its own block past that), its head over its own tiles. None while no window is open.
fn windows(root: &Root, shown: &[Value], inner: f32, cx: &mut Context<Root>) -> Option<Div> {
    let lanes = lanes(root, shown);
    if lanes.is_empty() {
        return None;
    }
    let (cols, col_w) = grid_fit(inner, COL_MIN, GRID_GAP);
    // (Every tile as tall as the fullest one: tiles side by side end on one line, review 2.3.)
    let tile_n = lanes.iter().flat_map(|l| l.items.iter()).map(|a| tile_lines(root, a)).max().unwrap_or(0);
    Some(
        div().w_full().flex().flex_wrap().items_start().gap_x(px(GRID_GAP)).gap_y(px(SECTION_GAP)).children(
            lanes
                .into_iter()
                .enumerate()
                .map(|(i, l)| {
                    let span = l.items.len().clamp(1, cols);
                    lane(root, i, l, col_w, span, tile_n, cx).into_any_element()
                })
                .collect::<Vec<_>>(),
        ),
    )
}

/// The cards layout: what needs a look on top, a lane for each game played here with a tile a window, then the
/// accounts not open.
fn cards(root: &Root, mut body: Div, shown: &[Value], inner: f32, cx: &mut Context<Root>) -> Div {
    let not_open = |a: &&Value| !is_local(a) && !launching(a);
    let check: Vec<&Value> = shown.iter().filter(not_open).filter(|a| signed_out(a) || banned(a)).collect();
    let idle: Vec<&Value> = shown.iter().filter(not_open).filter(|a| !signed_out(a) && !banned(a)).collect();
    let col_w = grid_fit(inner, COL_MIN, GRID_GAP).1;
    // What needs a hand: one calm panel, a full-width row each with its fix lined up at the right.
    if !check.is_empty() {
        let n = check.len();
        let mut panel = alert_panel(WARN, root.tr("ui.grp.check"), n);
        for a in check {
            panel = panel.child(alert_line(WARN)).child(look_row(root, a, cx));
        }
        body = body.child(panel);
    }
    body = body.children(windows(root, shown, inner, cx));
    if !idle.is_empty() {
        let n = idle.len();
        body = body.child(
            div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(section_head(root.tr("acc.grp.notHere"), Some(n)))
                // (Every card as tall as the fullest one: the grid's rows stay even.)
                .child({
                    let lines = idle.iter().map(|a| card_lines(root, a)).max().unwrap_or(0);
                    grid(GRID_GAP).pb(px(RISE)).children(idle.into_iter().map(|a| card(root, a, col_w, lines, cx).into_any_element()).collect::<Vec<_>>())
                }),
        );
    }
    body
}

/// What is wrong with an account that needs a look, in plain words after its state: a sign-in to do again, a ban.
fn look_hint(root: &Root, a: &Value) -> Option<String> {
    if rejoin_wait(a).is_some() {
        None
    } else if banned(a) {
        Some(root.tr("card.bannedTip"))
    } else if signed_out(a) {
        Some(root.tr("act.signInAgain"))
    } else {
        None
    }
}

/// A row of the panel of what needs a look: the face, the name, what is wrong in plain words, then its fix (Sign in,
/// Appeal, Cancel rejoin) in one column at the right, and the ⋮. A click picks it, as a card's.
fn look_row(root: &Root, a: &Value, cx: &mut Context<Root>) -> impl IntoElement {
    let id = id_of(a);
    let on = root.selected.contains(&id);
    let (line, tone) = line_of(root, a);
    let hint = look_hint(root, a);
    let fix = action(root, a, "look", cx);
    let user = s(a, "username").to_string();
    let pick = id.clone();
    // Its group, when Bloxkeep last opened it, its note: what the card had before it needed a look (review 2.1).
    let tags = tags_of(root, a);
    let opened = opened_of(root, a);
    let note = note_of(a);
    // (Three lines: taller than an alert row's two, or the third ran into the next row.)
    alert_row(SharedString::from(format!("look-{id}")))
        .h(px(88.))
        .relative()
        .map(|d| if on { d.tones(tinted(PANEL, FACE, 0.10), tinted(PANEL, FACE, 0.13)) } else { d })
        .on_click(cx.listener(move |r, _, _, cx| {
            toggle(r, &pick);
            cx.notify();
        }))
        .on_mouse_down(MouseButton::Right, on_right(a, cx))
        .children(menu::place(root, a, false, cx))
        .child(avatar(a, 40., PANEL, cx))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(3.))
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .gap(px(8.))
                        // (The name keeps its room, the @username gives way first: a long one cut the name to a letter.)
                        .child(div().flex_shrink_0().max_w(relative(1.)).truncate().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).child(label(a)))
                        .when(!user.is_empty(), |d| d.child(div().min_w_0().flex_shrink(1.).truncate().text_size(px(T_S)).text_color(c(MUTED)).child(format!("@{user}")))),
                )
                .child(
                    div()
                        .flex()
                        .min_w_0()
                        .items_center()
                        .gap(px(8.))
                        .text_size(px(T_M))
                        .child(div().size(px(7.)).flex_none().rounded_full().bg(c(tone)))
                        .child(div().flex_none().text_color(c(tone)).font_weight(FontWeight::MEDIUM).child(line))
                        .when_some(hint, |d, h| d.child(div().min_w_0().truncate().text_color(c(MUTED)).child(h))),
                )
                .child(
                    div()
                        .pt(px(2.))
                        .flex()
                        .min_w_0()
                        .items_center()
                        .gap(px(10.))
                        .children(tags)
                        .child(small_line("history", opened).flex_none())
                        .when_some(note, |d, n| d.child(small_line("message", n).min_w_0())),
                ),
        )
        .when(on, |d| d.child(div().size(px(22.)).flex_none().rounded(px(7.)).bg(c(FACE)).flex().items_center().justify_center().child(icon("check", 14., c(INK)))))
        .child(alert_fix(div().children(fix.map(no_pick))))
        .child(edit_button(root, a, "look", 28., cx))
        .child(more_button(root, a, "look", 28., cx))
        // Picked: the yellow ring every picked card has (the tick alone was easy to miss on a long row).
        .when(on, |d| d.child(div().absolute().inset_0().rounded(px(R_M)).border_2().border_color(c(FACE))))
}

/// The title and count with a quiet summary under them (who plays, their RAM, what needs a look), the search and Add on
/// that line; the filters under them, and at their right one row of quiet tools, all of one height.
fn header(root: &Root, all: &[Value], width: f32, rows: bool, cx: &mut Context<Root>) -> impl IntoElement {
    // Short of room (a window about 1180 wide): the tools keep the value chosen and drop their names.
    let compact = width < 1300.;
    // Cards or a list (the web page's layout switch): kept by the app with the sort, so it is the same next time.
    let layout = view_switch(
        [("cards", "grid", false), ("list", "rows", true)]
            .into_iter()
            .map(|(key, mark, list)| {
                let tip: SharedString = root.tr(&format!("layout.{key}")).into();
                view_choice(SharedString::from(format!("acc-layout-{key}")), mark, rows == list).tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()).on_click(cx.listener(move |r, _, _, cx| {
                    cx.default_global::<menu::Ui>().list = Some(list);
                    r.send("setSettings", json!({ "layout": key }));
                    cx.notify();
                }))
            })
            .collect(),
    );
    let playing = all.iter().filter(|a| in_game(a)).count();
    let online = all.iter().filter(|a| is_online(a)).count();
    let mut chips = vec![
        (Filter::All, root.tr("chip.all"), all.len()),
        (Filter::Playing, root.tr("chip.ingame"), playing),
        (Filter::Online, root.tr("chip.online"), online),
        (Filter::Offline, root.tr("chip.offline"), all.len() - online),
    ];
    // (Shown only while there are some: needing a sign-in, and banned.)
    for (f, key, n) in [(Filter::Invalid, "chip.invalid", all.iter().filter(|a| signed_out(a)).count()), (Filter::Banned, "chip.banned", all.iter().filter(|a| banned(a)).count())] {
        if n > 0 || root.acc_filter == f {
            chips.push((f, root.tr(key), n));
        }
    }
    // The page at a glance, in muted words: who plays (or that Roblox isn't open), their windows' RAM, who dropped,
    // how many need a look.
    let strays = others(root);
    let ram: f64 = all.iter().chain(strays.iter()).filter(|a| is_local(a)).filter_map(|a| ram_mb(root, a)).sum();
    let dropped = all.iter().filter(|a| is_kicked(a)).count();
    let look = all.iter().filter(|a| !is_local(a) && !launching(a) && (signed_out(a) || banned(a))).count();
    let mut summary = vec![(if playing > 0 { root.i18n.tn("acc.sum.playing", playing as i64, &[]) } else { root.tr("acc.sum.idle") }, None)];
    if ram > 0. {
        summary.push((mb_text(ram, root.i18n.vietnamese), None));
    }
    if dropped > 0 {
        summary.push((root.i18n.tv("ui.droppedN", &[("n", &dropped.to_string())]), None));
    }
    if look > 0 {
        summary.push((root.i18n.tn("acc.sum.look", look as i64, &[]), None));
    }
    // The accounts' groups, when they have any: one of them alone.
    let mut names: Vec<String> = all.iter().map(|a| s(a, "group").to_string()).filter(|g| !g.is_empty()).collect();
    names.sort_by_key(|g| folded(g));
    names.dedup();
    let now = root.acc_group.clone().filter(|g| names.contains(g));
    let groups = (!names.is_empty()).then(|| {
        let shown = now.clone().unwrap_or_else(|| root.tr("chip.all"));
        let items: Vec<DropItem> = std::iter::once((None, root.tr("chip.all")))
            .chain(names.iter().map(|g| (Some(g.clone()), g.clone())))
            .map(|(key, text): (Option<String>, String)| {
                let on = key == now;
                let f: DropAct = std::rc::Rc::new(move |r, _, _| r.acc_group = key.clone());
                (text, on, f)
            })
            .collect();
        drop_list(root, "acc-group", "tag", Some(root.tr("grp.label")), shown, items, cx)
    });
    // The order: kept by the app, so it is the same next time; under it, the order Play opens the picked accounts in.
    let how = root.state.pointer("/settings/sort").and_then(Value::as_str).unwrap_or("name").to_string();
    let mut items: Vec<DropItem> = ["name", "status", "recent", "order"]
        .into_iter()
        .map(|key| {
            let f: DropAct = std::rc::Rc::new(move |r, _, _| {
                r.send("setSettings", json!({ "sort": key }));
            });
            (root.tr(&format!("sort.{key}")), how == key, f)
        })
        .collect();
    items.push((LINE_ITEM.to_string(), false, std::rc::Rc::new(|_, _, _| {})));
    items.push((root.tr("lo.edit"), false, std::rc::Rc::new(|r, window, cx| open_order(r, window, cx))));
    let sorts = drop_list(root, "acc-sort", "sort", Some(root.tr("sort.label")), root.tr(&format!("sort.{how}")), items, cx);
    // Select all: the ones shown; the main accounts (the ones played on) are left out, unless they are all there is.
    let query = root.acc_search.read(cx).value().trim().to_lowercase();
    let seen: Vec<&Value> = all.iter().filter(|a| passes(a, root.acc_filter, &query)).filter(|a| now.as_deref().is_none_or(|g| s(a, "group") == g)).collect();
    let farm: Vec<String> = seen.iter().filter(|a| !a.get("isMain").and_then(Value::as_bool).unwrap_or(false)).map(|a| id_of(a)).collect();
    let pickable: Vec<String> = if farm.is_empty() { seen.iter().map(|a| id_of(a)).collect() } else { farm };
    let none = pickable.is_empty();
    // (Named at every width: short of room the tools take a line of their own under the filters, review 2.5.)
    let all_words: SharedString = crate::pages::lock::plain(root.tr("acc.selectAll")).into();
    let pick_all = tool("acc-pick-all", Some("check"), false).child(all_words).map(|d| {
        if none {
            d.opacity(0.45).cursor_default()
        } else {
            d.on_click(cx.listener(move |r, _, _, cx| {
                r.selected = pickable.iter().cloned().collect();
                cx.notify();
            }))
        }
    });
    let tools = div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(4.))
        .child(layout)
        .child(tool_rule())
        .children(groups)
        .child(sorts)
        // Windows: Open Roblox only, hide or show them all, and Arrange's layouts as pictures (pages/snap.rs).
        .child(crate::pages::snap::button(root, false, cx))
        .child(tool_rule())
        .child(pick_all);
    div()
        .flex_none()
        .px(px(SIDE_PAD))
        .pt(px(22.))
        .pb(px(16.))
        .flex()
        .flex_col()
        .gap(px(16.))
        .child(
            div()
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
                        .child(
                            div()
                                .flex()
                                .items_baseline()
                                .gap(px(10.))
                                .child(div().text_size(px(T_TITLE)).line_height(px(38.)).font_weight(FontWeight::BOLD).child(root.tr("acc.title")))
                                .child(div().text_size(px(T_TITLE)).line_height(px(38.)).text_color(c(DIM)).font_features(tnum()).child(all.len().to_string())),
                        )
                        .when(!all.is_empty(), |d| d.child(facts(summary))),
                )
                .child(
                    div()
                        .w(px(if width < 900. { 220. } else { 280. }))
                        .min_w(px(140.))
                        .flex_shrink(1.)
                        .child(field(&root.acc_search, Some("search"))),
                )
                // Online status asked again now (the app's refreshPresence; its icon turns until it answers).
                .child({
                    let busy = cx.try_global::<menu::Ui>().is_some_and(|u| u.refreshing);
                    IconButton::new("acc-refresh", "refresh").size(38.).busy(busy).tip(root.tr("acc.refreshTip")).on_click(cx.listener(|r, _, _, cx| refresh(r, cx)))
                })
                .child(tools_menu(root, all, cx))
                // Add account, and the other ways to add one beside it.
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .gap(px(2.))
                        .child(Button::new("acc-add", root.tr("acc.add")).icon("plus").primary().on_click(cx.listener(|r, _, _, cx| r.add_account(cx))))
                        .child(add_menu(root, cx)),
                ),
        )
        // No Roblox on this PC: nothing can open until it is installed (the web page's sidebar card); Get Roblox opens
        // Roblox's download page.
        .when(root.state.get("robloxInstalled").and_then(Value::as_bool) == Some(false), |d| {
            d.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(14.))
                    .py(px(8.))
                    .rounded(px(R_M))
                    .bg(tinted(PANEL, BAD, 0.07))
                    .border_1()
                    .border_color(ca(BAD, 0.3))
                    .child(icon("alert", 16., c(BAD)))
                    .child(div().flex_1().min_w_0().text_size(px(T_M)).text_color(c(TEXT_2)).child(root.tr("side.noRoblox")))
                    .child(Button::new("acc-get-roblox", root.tr("acc.getRoblox")).icon("download").small().on_click(|_, _, cx| cx.open_url("https://www.roblox.com/download"))),
            )
        })
        // (No accounts yet: nothing to filter, sort or lay out; the start below says what to do.)
        .when(!all.is_empty(), |d| {
            d.child(
                // One row: the filters take what the tools leave; short of room (about 1180 wide), the tools on a line
                // of their own under them, with their names (icons alone read as "All", "Name" and a picture).
                div()
                    .flex()
                    .when(compact, |d| d.flex_col().items_start().gap(px(10.)))
                    .when(!compact, |d| d.items_center().gap(px(12.)))
                    .child(
                        div().when(compact, |d| d.w_full()).when(!compact, |d| d.flex_1()).min_w_0().flex().flex_wrap().items_center().gap(px(if compact { 2. } else { 4. })).children(chips.into_iter().map(|(f, text, count)| {
                            let on = root.acc_filter == f;
                            let dot = match f {
                                Filter::All => None,
                                Filter::Playing => Some(OK),
                                Filter::Online => Some(ONLINE),
                                Filter::Offline => Some(MUTED),
                                Filter::Invalid | Filter::Banned => Some(BAD),
                            };
                            chip(SharedString::from(format!("chip-{text}")), on)
                                .h(px(TOOL_H))
                                .when(compact, |d| d.px(px(8.)))
                                .when_some(dot, |d, col| d.child(div().size(px(7.)).flex_none().rounded_full().bg(c(col))))
                                .child(text)
                                .child(chip_count(count, on))
                                .on_click(cx.listener(move |r, _, _, cx| {
                                    r.acc_filter = f;
                                    cx.notify();
                                }))
                        })),
                    )
                    .child(tools),
            )
        })
        // Multi-Roblox off: opening one more account closes the window before it (the web page said so in its
        // sidebar card, "Off: one Roblox window at a time", and in Multi-Roblox's box); said here, where Play is.
        .when(root.state.pointer("/multi/enabled").and_then(Value::as_bool) == Some(false) && all.len() > 1, |d| {
            d.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(14.))
                    .py(px(8.))
                    .rounded(px(R_M))
                    .bg(tinted(PANEL, WARN, 0.07))
                    .border_1()
                    .border_color(ca(WARN, 0.26))
                    .child(icon("alert", 16., c(WARN)))
                    .child(div().flex_1().min_w_0().text_size(px(T_M)).text_color(c(TEXT_2)).child(root.tr("mh.off")))
                    .child(Button::new("acc-multi-on", root.tr("mh.enable")).icon("layers").small().on_click(cx.listener(|r, _, window, cx| r.multi_help(window, cx)))),
            )
        })
}

/// Refresh: the accounts' online status asked again (app.js's refresh-presence); only a click that did nothing is said.
fn refresh(r: &mut Root, cx: &mut Context<Root>) {
    if cx.try_global::<menu::Ui>().is_some_and(|u| u.refreshing) {
        return;
    }
    let Some(link) = r.link.clone() else { return };
    cx.default_global::<menu::Ui>().refreshing = true;
    let rx = link.call("refreshPresence", json!({}));
    cx.spawn(async move |this, cx| {
        let answer = rx.await;
        let _ = this.update(cx, |r, cx| {
            cx.default_global::<menu::Ui>().refreshing = false;
            match answer {
                Ok(Ok(v)) if v.get("skipped").and_then(Value::as_bool) == Some(true) => r.toast = Some((r.tr("t.refreshBusy"), "info".into())),
                Ok(Err(e)) => r.toast = Some((e.to_string(), "error".into())),
                _ => {}
            }
            cx.notify();
        });
    })
    .detach();
    cx.notify();
}

/// A line of an icon menu: its icon, words, whether it is a danger (red), and what picking it does; None: a hairline.
type MenuLine = Option<(&'static str, String, bool, DropAct)>;

/// An icon button with a short menu under it (the header's ⋮, Add's other ways), on the same open/closed state as the
/// toolbar's lists, so one of them is open at a time.
fn icon_menu(root: &Root, key: &'static str, button: IconButton, width: f32, lines: Vec<MenuLine>, cx: &mut Context<Root>) -> Div {
    let open = root.acc_drop == Some(key);
    let mut wrap = div().relative().flex_none().child(button.selected(open).on_click(cx.listener(move |r, _, _, cx| {
        let just = r.acc_drop_closed.is_some_and(|(k, at)| k == key && at.elapsed().as_millis() < 300);
        r.acc_drop_closed = None;
        r.acc_drop = if just || r.acc_drop == Some(key) { None } else { Some(key) };
        r.acc_drop_was = Some(key);
        cx.notify();
    })));
    if open || root.acc_drop_was == Some(key) {
        let mut menu = div()
            .id(SharedString::from(format!("{key}-menu")))
            .occlude()
            .w(px(width))
            .p(px(4.))
            .rounded(px(R_M))
            .bg(c(RAISED))
            .border_1()
            .border_color(c(LINE))
            .shadow_lg()
            .flex()
            .flex_col()
            .on_mouse_down_out(cx.listener(move |r, _, _, cx| {
                if r.acc_drop == Some(key) {
                    r.acc_drop = None;
                    r.acc_drop_closed = Some((key, std::time::Instant::now()));
                    cx.notify();
                }
            }));
        for (i, l) in lines.into_iter().enumerate() {
            let Some((mark, text, danger, f)) = l else {
                menu = menu.child(div().h(px(1.)).my(px(4.)).mx(px(6.)).bg(c(LINE)));
                continue;
            };
            let tone = if danger { BAD } else { TEXT };
            menu = menu.child(
                menu_row((key, 2000 + i))
                    .text_color(c(tone))
                    .child(icon(mark, 14., c(if danger { BAD } else { TEXT_2 })))
                    .child(div().min_w_0().flex_1().truncate().child(text))
                    .on_click(cx.listener(move |r, _, window, cx| {
                        r.acc_drop = None;
                        f(r, window, cx);
                        cx.notify();
                    })),
            );
        }
        wrap = wrap.child(div().absolute().top(px(42.)).right_0().child(deferred(anchored().anchor(Anchor::TopRight).snap_to_window_with_margin(px(8.)).child(pop(SharedString::from(format!("{key}-pop")), open, menu))).with_priority(1)));
    }
    wrap
}

/// The header's ⋮ (app.js's tools-menu): check every sign-in, the launch order, close or end every Roblox, the guide.
fn tools_menu(root: &Root, all: &[Value], cx: &mut Context<Root>) -> Div {
    let some = !all.is_empty();
    let act = |f: fn(&mut Root, &mut Window, &mut Context<Root>)| -> DropAct { std::rc::Rc::new(f) };
    let mut lines: Vec<MenuLine> = Vec::new();
    if some {
        lines.push(Some(("refresh", root.tr("m.checkAll"), false, act(|r, _, cx| check_accounts(r, Vec::new(), cx)))));
    }
    if all.len() > 1 {
        lines.push(Some(("sort", root.tr("m.launchOrder"), false, act(|r, window, cx| open_order(r, window, cx)))));
    }
    if !lines.is_empty() {
        lines.push(None);
    }
    lines.push(Some(("power", root.tr("m.closeAll"), true, act(|r, window, cx| r.close_all(false, window, cx)))));
    lines.push(Some(("x", root.tr("m.killAll"), true, act(|r, window, cx| r.close_all(true, window, cx)))));
    lines.push(None);
    lines.push(Some(("book", root.tr("help.title"), false, act(|r, _, _| r.send("openGuide", json!({}))))));
    icon_menu(root, "acc-tools", IconButton::new("acc-tools-btn", "more").size(38.).tip(root.tr("acc.moreTip")), 280., lines, cx)
}

/// Beside Add account (app.js's add-menu): the other ways to add one, from another Bloxkeep file, and roblox.com.
fn add_menu(root: &Root, cx: &mut Context<Root>) -> Div {
    let act = |f: fn(&mut Root, &mut Window, &mut Context<Root>)| -> DropAct { std::rc::Rc::new(f) };
    let lines: Vec<MenuLine> = vec![
        Some(("qr", root.tr("m.addCode"), false, act(|r, window, cx| crate::pages::quick::open(r, false, window, cx)))),
        Some(("globe", root.tr("m.addBrowser"), false, act(|r, _, cx| r.add_browser(cx)))),
        Some(("cookie", root.tr("m.addCookie"), false, act(|r, window, cx| crate::pages::account_form::open(r, None, window, cx)))),
        None,
        Some(("folder", root.tr("m.importVault"), false, act(|r, window, cx| crate::pages::import_vault::open(r, window, cx)))),
        None,
        Some(("external", root.tr("m.openWeb"), false, act(|r, _, _| r.send("openWebsiteBlank", json!({}))))),
    ];
    icon_menu(root, "acc-addmore", IconButton::new("acc-addmore-btn", "chevronDown").size(38.).filled().tip(root.tr("acc.addMoreTip")), 320., lines, cx)
}

/// What a short list's row does when picked.
type DropAct = std::rc::Rc<dyn Fn(&mut Root, &mut Window, &mut Context<Root>)>;
/// A row of a short list: its words, whether it is the one chosen, what picking it does. Words of `LINE_ITEM` are a
/// hairline between groups of rows.
type DropItem = (String, bool, DropAct);
const LINE_ITEM: &str = "-";

/// A quiet tool that opens a short list under it ("Group  All ▾", "Sort  Name ▾"): `name` before the value chosen
/// (none when room is short). The row keeps one bright thing, the chosen filter.
fn drop_list(root: &Root, key: &'static str, mark: &'static str, name: Option<String>, value: String, items: Vec<DropItem>, cx: &mut Context<Root>) -> Div {
    let open = root.acc_drop == Some(key);
    let mut wrap = div().relative().flex_none().child(tool(key, Some(mark), open).child(tool_words(name, value)).child(tool_chev()).on_click(cx.listener(move |r, _, _, cx| {
        // A click outside the open list closed it just now: this click on its own button only closes it.
        let just = r.acc_drop_closed.is_some_and(|(k, at)| k == key && at.elapsed().as_millis() < 300);
        r.acc_drop_closed = None;
        r.acc_drop = if just || r.acc_drop == Some(key) { None } else { Some(key) };
        r.acc_drop_was = Some(key);
        cx.notify();
    })));
    if open || root.acc_drop_was == Some(key) {
        let mut menu = div()
            .id(SharedString::from(format!("{key}-menu")))
            .occlude()
            .min_w(px(200.))
            .p(px(4.))
            .rounded(px(R_M))
            .bg(c(RAISED))
            .border_1()
            .border_color(c(LINE))
            .shadow_lg()
            .flex()
            .flex_col()
            .on_mouse_down_out(cx.listener(move |r, _, _, cx| {
                if r.acc_drop == Some(key) {
                    r.acc_drop = None;
                    r.acc_drop_closed = Some((key, std::time::Instant::now()));
                    cx.notify();
                }
            }));
        for (i, (text, on, f)) in items.into_iter().enumerate() {
            if text == LINE_ITEM {
                menu = menu.child(div().h(px(1.)).my(px(4.)).mx(px(6.)).bg(c(LINE)));
                continue;
            }
            menu = menu.child(
                menu_row((key, 1000 + i))
                    .text_color(if on { c(TEXT) } else { c(TEXT_2) })
                    .child(div().min_w_0().flex_1().whitespace_nowrap().child(text))
                    .child(div().w(px(14.)).flex_none().when(on, |d| d.child(icon("check", 14., c(TEXT)))))
                    .on_click(cx.listener(move |r, _, window, cx| {
                        r.acc_drop = None;
                        f(r, window, cx);
                        cx.notify();
                    })),
            );
        }
        wrap = wrap.child(div().absolute().top(px(TOOL_H + 4.)).right_0().child(deferred(anchored().anchor(Anchor::TopRight).snap_to_window_with_margin(px(8.)).child(pop(SharedString::from(format!("{key}-pop")), open, menu))).with_priority(1)));
    }
    wrap
}

/// A window in trouble: dropped, held by Roblox's bot check, signed out in its window. It stands on a red base, its
/// state in words on it.
fn trouble(a: &Value) -> bool {
    !foreign(a) && (is_kicked(a) || held(a) || (is_local(a) && conn(a) == "signedout"))
}

/// One game's windows as a block on the page's grid, `span` columns wide: its head over its own tiles (the game's icon
/// on the game's own colour, its name, how many windows and their RAM, and the buttons for all of them at its right
/// edge), then its windows, each tile standing on the game's colour.
fn lane(root: &Root, i: usize, l: Lane, col_w: f32, span: usize, lines: usize, cx: &mut Context<Root>) -> impl IntoElement {
    // As wide as its columns; one column wide, its buttons are icons (their words the tips).
    let w = span as f32 * col_w + (span as f32 - 1.) * GRID_GAP;
    let tight = span < 2;
    let n = l.items.len();
    // Bloxkeep's accounts by id; the other windows by their handle (closed one by one, as the Game monitor did).
    let ids: Vec<String> = l.items.iter().filter(|a| !foreign(a)).map(id_of).collect();
    let strays: Vec<String> = l.items.iter().filter(|a| foreign(a)).map(|a| s(a, "hwnd").to_string()).collect();
    let dropped: Vec<Value> = l.items.iter().filter(|a| is_kicked(a) && !foreign(a)).cloned().collect();
    let any_hidden = l.items.iter().any(hidden);
    let gicon = l.icon.as_deref().and_then(|u| images::pic_sq(u, GROUP_PIC, cx));
    // The game's own colour, from its icon: the base its icon and its windows' tiles stand on.
    let game_base = l.icon.as_deref().and_then(|u| images::tone(u, cx)).map(|t| base_of(c(t))).unwrap_or_else(base_dark);
    let mut meta = vec![(root.i18n.tn("mon.windows", n as i64, &[]), None)];
    if l.ram > 0. {
        meta.push((mb_text(l.ram, root.i18n.vietnamese), None));
    }
    if l.kicked > 0 {
        meta.push((root.i18n.tn("mon.nBad", l.kicked as i64, &[]), Some(c(WARN))));
    }
    let close_ids = ids.clone();
    let win_ids = ids.clone();
    let win_btn = (!win_ids.is_empty()).then(|| {
        let label = root.tr(if any_hidden { "card.showWindow" } else { "ui.hideWin" });
        let mark = if any_hidden { "eye" } else { "eyeOff" };
        let act = cx.listener(move |r: &mut Root, _: &ClickEvent, _, _| r.send(if any_hidden { "showWindows" } else { "hideWindows" }, json!({ "ids": win_ids })));
        // (An icon on every lane, its words the tip: lanes side by side look the same whatever their width.)
        IconButton::new(("lane-win", i), mark).size(30.).tip(label).on_click(act).into_any_element()
    });
    let close_btn = {
        let label = format!("{} · {n}", root.tr("card.close"));
        let act = cx.listener(move |r: &mut Root, _: &ClickEvent, _, _| {
            if !close_ids.is_empty() {
                r.send("closeAccounts", json!({ "ids": close_ids, "force": false }));
            }
            for h in &strays {
                r.send("monitorAct", json!({ "id": h, "what": "close" }));
            }
        });
        // (Labelled: it is the lane's main button. A one-column lane short of room keeps its icon, its words the tip,
        // so the game's name isn't cut to "Brookhaven…", review 2.5.)
        if tight && col_w < 400. {
            IconButton::new(("lane-close", i), "power").size(30.).tip(label).on_click(act).into_any_element()
        } else {
            Button::new(("lane-close", i), label).icon("power").small().on_click(act).into_any_element()
        }
    };
    let picture = on_base(
        game_base,
        3.,
        R_M + 1.,
        div().size(px(GROUP_PIC)).rounded(px(R_M)).overflow_hidden().bg(c(RAISED_2)).flex().items_center().justify_center().map(|d| match gicon {
            Some(p) => d.child(img(p).size_full().object_fit(ObjectFit::Cover)),
            None => d.child(icon("gamepad", 20., c(MUTED))),
        }),
    );
    let head = group_head(picture, l.name.clone(), facts(meta)).child(
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(if tight { 2. } else { 8. }))
            // One cut off has its own Rejoin on its tile; this is for several at once.
            .when(dropped.len() > 1, |d| {
                let label = root.i18n.tv("ui.rejoinN", &[("n", &dropped.len().to_string())]);
                d.child(Button::new(("lane-rejoin", i), label).icon("refresh").tint().small().on_click(cx.listener(move |r, _, _, cx| r.rejoin(&dropped, cx))))
            })
            .children(win_btn)
            .child(close_btn),
    );
    let tiles: Vec<AnyElement> = l
        .items
        .iter()
        .map(|a| {
            let base = if root.selected.contains(&id_of(a)) {
                c(FACE_BASE)
            } else if trouble(a) {
                bad_base()
            } else {
                game_base
            };
            on_base(base, RISE, R_L + 1., tile(root, a, col_w, lines, cx)).w(px(col_w)).into_any_element()
        })
        .collect();
    div().w(px(w)).flex_none().flex().flex_col().gap(px(16.)).child(head).child(grid(GRID_GAP).pb(px(RISE)).children(tiles))
}

/// One Roblox window. Its picture first, across the top at 16:9: the live one the app lays there (Windows draws it
/// over this window, so nothing of ours goes on it), the game's own under it while there's none (minimised, hidden,
/// opening, or not sent yet). Under it the face, name and time on one line, then a line of facts: the state's dot and
/// words at the left, RAM and PID at the right; its chips and note, today's strip, then its main button (Close, or the
/// way back in: Rejoin, Verify), Edit and its menu at the foot. A click brings the window to the
/// front, as the old Game monitor did; the round tick (or Ctrl+click) picks the account for the bar.
/// The lines of facts a tile has under its state: its chips and Robux, its note.
fn tile_lines(root: &Root, a: &Value) -> usize {
    if foreign(a) {
        return 0;
    }
    let chips = !tags_of(root, a).is_empty() || old_build(a) || fps_limit(a).is_some() || robux_of(root, a).is_some();
    chips as usize + note_of(a).is_some() as usize
}

/// `lines`: the most lines of facts a tile on the page has (`tile_lines`), so all of them are one height.
fn tile(root: &Root, a: &Value, w: f32, lines: usize, cx: &mut Context<Root>) -> impl IntoElement {
    const INSET_PAD: f32 = 6.;
    want_strips(root, cx);
    let id = id_of(a);
    let other = foreign(a);
    let on = root.selected.contains(&id);
    let bad = trouble(a);
    let (state, tone, _) = state_of(root, a);
    let win = window_of(root, a);
    let hwnd = win.map(|w| s(w, "id").to_string()).filter(|h| !h.is_empty());
    let minimized = win.and_then(|w| w.get("minimized")).and_then(Value::as_bool).unwrap_or(false);
    let live = hwnd.clone().filter(|_| !minimized && !hidden(a));
    // (The tile's hairline is inside its width.)
    let pic_w = w - 2. * INSET_PAD - 2.;
    let h = (pic_w * 9. / 16.).floor();
    let cover = cover_of(root, game_place(a)).and_then(|u| images::pic_at(&u, pic_w, h, cx));
    // Its main button at the foot, as a not-open card has Play there: Close, or the way back in (Rejoin, Verify) when
    // dropped or held on Roblox's bot check; then Edit and its menu (review 2.2: a playing tile had no Edit).
    let main = action(root, a, "tile", cx);
    let ram = ram_mb(root, a).map(|m| mb_text(m, root.i18n.vietnamese));
    let pid = pid_el(root, a, cx);
    let both = ram.is_some() && pid.is_some();
    let old = old_build(a).then(|| old_tag(root, a, cx));
    let fps = fps_limit(a);
    // Who it is and what it holds (a window not opened by Bloxkeep is no account: none of these).
    let under = (!other).then(|| handle_of(a)).flatten();
    let tags = if other { Vec::new() } else { tags_of(root, a) };
    let robux = (!other).then(|| robux_of(root, a).map(|r| robux_el(root, &id, r))).flatten();
    let note = (!other).then(|| note_of(a)).flatten();
    // Over the game's picture while there is no live one: why not.
    let why = if hidden(a) {
        Some(("eyeOff", root.tr("ui.winHidden")))
    } else if minimized {
        Some(("screens", root.tr("mon.minimized")))
    } else {
        None
    };
    let pick = id.clone();
    let tick_id = id.clone();
    let front = hwnd.clone();
    let group = SharedString::from(format!("tile-{id}"));
    hot(group.clone())
        .lift(3.)
        .group(group.clone())
        .w(px(w))
        .p(px(INSET_PAD))
        .flex()
        .flex_col()
        .rounded(px(R_L))
        .cursor_pointer()
        .map(|d| {
            if on {
                // (Solid grounds: the tile stands on a base that must not show through it.)
                d.tones(tinted(RAISED, FACE, 0.10), tinted(RAISED_2, FACE, 0.12)).edge(ca(FACE, 0.7), ca(FACE, 0.85))
            } else if bad {
                d.tones(tinted(RAISED, BAD, 0.09), tinted(RAISED_2, BAD, 0.11)).edge(ca(BAD, 0.38), ca(BAD, 0.52))
            } else {
                d.tones(c(RAISED), c(RAISED_2)).edge(c(LINE), ca(WHITE, 0.17))
            }
        })
        .on_click(cx.listener(move |r, e: &ClickEvent, _, cx| {
            if e.modifiers().control && !other {
                toggle(r, &pick);
                cx.notify();
            } else if let Some(h) = &front {
                r.send("monitorAct", json!({ "id": h, "what": "show" }));
            } else if !other {
                toggle(r, &pick);
                cx.notify();
            }
        }))
        .when(!other, |d| d.on_mouse_down(MouseButton::Right, on_right(a, cx)).children(menu::place(root, a, false, cx)))
        .child(
            div()
                .relative()
                .w_full()
                .h(px(h))
                .rounded(px(R_L - INSET_PAD + 1.))
                .overflow_hidden()
                .bg(ca(WHITE, 0.05))
                .when_some(cover, |d, p| d.child(div().absolute().inset_0().child(img(p).size_full().object_fit(ObjectFit::Cover))))
                .when(why.is_some() || launching(a) || conn(a) == "menu", |d| d.child(div().absolute().inset_0().bg(ca(GROUND, 0.6))))
                .when_some(why, |d, (glyph, text)| {
                    d.child(
                        div()
                            .absolute()
                            .inset_0()
                            .px(px(12.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(px(6.))
                            .text_size(px(T_M))
                            .text_color(c(TEXT_2))
                            .child(icon(glyph, 15., c(TEXT_2)))
                            .child(div().truncate().child(text)),
                    )
                })
                .when_some(live, |d, h| d.child(root.live.picture(h))),
        )
        .child(
            div()
                .px(px(6.))
                .pt(px(12.))
                .pb(px(4.))
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .child(avatar(a, 36., RAISED, cx))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(div().min_w_0().text_size(px(T_L)).line_height(px(20.)).font_weight(FontWeight::SEMIBOLD).truncate().child(label(a)))
                                .when_some(under, |d, u| d.child(div().min_w_0().truncate().text_size(px(T_S)).line_height(px(16.)).text_color(c(MUTED)).child(u))),
                        )
                        .when_some(up_of(a), |d, t| d.child(div().flex_none().text_size(px(T_M)).text_color(c(MUTED)).font_features(tnum()).child(t)))
                        // The round tick: picks the account for the bar below (a window not opened by Bloxkeep has none).
                        .when(!other, |d| {
                            d.child(
                                div()
                                    .id(SharedString::from(format!("tick-{id}")))
                                    .size(px(20.))
                                    .flex_none()
                                    .rounded_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .border_1()
                                    .map(|d| {
                                        if on {
                                            d.bg(c(FACE)).border_color(c(FACE)).child(icon("check", 13., c(INK)))
                                        } else {
                                            d.border_color(ca(WHITE, 0.18)).group_hover(group.clone(), |s| s.border_color(ca(WHITE, 0.45)))
                                        }
                                    })
                                    .tooltip({
                                        let t: SharedString = root.tr("ui.pickOne").into();
                                        move |_, cx| cx.new(|_| Tip(t.clone())).into()
                                    })
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                    .on_click(cx.listener(move |r, _, _, cx| {
                                        toggle(r, &tick_id);
                                        cx.notify();
                                    })),
                            )
                        }),
                )
                // The facts: the state at the left, RAM and PID at the right in tabular figures.
                .child(
                    div()
                        .h(px(28.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .text_size(px(T_M))
                        .child(div().size(px(7.)).flex_none().rounded_full().bg(c(tone)))
                        .child(div().min_w_0().truncate().font_weight(FontWeight::MEDIUM).text_color(if tone == OK || tone == TEXT_2 { c(TEXT_2) } else { c(tone) }).child(state))
                        .child(div().flex_1())
                        .child(
                            div()
                                .flex()
                                .flex_none()
                                .items_center()
                                .gap(px(6.))
                                .text_color(c(MUTED))
                                .font_features(tnum())
                                .when_some(ram, |d, r| d.child(r))
                                .when(both, |d| d.child(sep()))
                                .children(pid),
                        ),
                )
                // Main and its group, an older Roblox than the newest, the FPS limit it was opened with, and its Robux
                // at the right: a line of their own, so the state and the RAM keep their room.
                // (Held at the height of the fullest tile's lines, so tiles in a row end together.)
                .when(lines > 0, |d| {
                    let tip: SharedString = root.tr("card.fpsTip").into();
                    let chips = !tags.is_empty() || old.is_some() || fps.is_some() || robux.is_some();
                    d.child(
                        div()
                            .h(px(lines as f32 * 20. + (lines as f32 - 1.) * 10.))
                            .flex()
                            .flex_col()
                            .gap(px(10.))
                            .when(chips, |d| {
                                d.child(
                                    div()
                                        .h(px(20.))
                                        .flex()
                                        .items_center()
                                        .gap(px(6.))
                                        .children(tags)
                                        .children(old)
                                        .when_some(fps, |d, f| {
                                            d.child(div().id(SharedString::from(format!("fps-{id}"))).flex_none().child(tag(format!("{f} FPS"), TEXT_2)).tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()))
                                        })
                                        .child(div().flex_1())
                                        .children(robux),
                                )
                            })
                            .when_some(note, |d, n| d.child(small_line("message", n).h(px(20.)))),
                    )
                })
                // Today, as a strip (a window not opened by Bloxkeep: its room, empty).
                .child(if other { div().h(px(12.)).into_any_element() } else { day_strip(root, &id).into_any_element() })
                // Its main button, Edit, its menu (also a right click); a window not opened by Bloxkeep keeps the room.
                .child(
                    div()
                        .h(px(30.))
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .children(main.map(no_pick))
                        .child(div().flex_1())
                        .when(!other, |d| d.child(edit_button(root, a, "tile", 30., cx)).child(more_button(root, a, "tile", 30., cx))),
                ),
        )
}

/// A command for the picked accounts; the app's refusal is said as a notice.
fn act(r: &mut Root, cmd: &'static str, args: Value, cx: &mut Context<Root>, done: impl FnOnce(&mut Root, Value) + 'static) {
    let Some(link) = r.link.clone() else { return };
    let rx = link.call(cmd, args);
    cx.spawn(async move |this, cx| {
        let answer = rx.await;
        let _ = this.update(cx, |r, cx| {
            match answer {
                Ok(Ok(v)) => done(r, v),
                Ok(Err(e)) => r.toast = Some((e.to_string(), "error".into())),
                Err(_) => {}
            }
            cx.notify();
        });
    })
    .detach();
}

/// Launch order (app.js's launchOrderModal): every account, moved up or down into the order Play opens them in; Done
/// hands the order to the app ("setLaunchOrder"). The web page's list could also be dragged.
pub fn open_order(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    use gpui_kit::component::WindowExt as _;
    use gpui_kit::component::dialog::DialogEntrance;
    let order: Vec<String> = r.state.pointer("/settings/launchOrder").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    let mut ids: Vec<String> = r.accounts().iter().map(id_of).collect();
    if ids.is_empty() {
        r.toast = Some((r.tr("rc.noAccounts"), "info".into()));
        cx.notify();
        return;
    }
    // (Accounts the order doesn't name yet go last, as they are.)
    ids.sort_by_key(|id| order.iter().position(|o| o == id).unwrap_or(usize::MAX));
    r.order_edit = ids;
    let this = cx.weak_entity();
    window.open_dialog(cx, move |dialog, _, cx| {
        let Some(root) = this.upgrade() else { return dialog };
        // Names and pictures first: a picture's load needs the app, which the read of Root holds.
        let (people, tips, words): (Vec<(String, String)>, [SharedString; 4], [SharedString; 4]) = {
            let r = root.read(cx);
            let people = r
                .order_edit
                .iter()
                .map(|id| r.accounts().iter().find(|a| s(a, "id") == id).map(|a| (label(a), s(a, "avatarUrl").to_string())).unwrap_or_default())
                .collect();
            (
                people,
                [r.tr("lo.up").into(), r.tr("lo.down").into(), r.tr("common.cancel").into(), r.tr("common.done").into()],
                [r.tr("lo.title").into(), r.tr("lo.text").into(), r.tr("common.close").into(), SharedString::default()],
            )
        };
        let count = people.len();
        let mut rows = div().flex().flex_col();
        for (i, (name, url)) in people.iter().enumerate() {
            let step = |to: usize, key: &'static str, icon_name: &'static str, off: bool| {
                let this = this.clone();
                IconButton::new((key, i), icon_name).size(26.).disabled(off).tip(if to < i { tips[0].clone() } else { tips[1].clone() }).on_click(move |_, w, cx| {
                    let _ = this.update(cx, |r, _| {
                        if i < r.order_edit.len() && to < r.order_edit.len() {
                            r.order_edit.swap(i, to);
                        }
                    });
                    w.refresh();
                })
            };
            let pic = crate::images::pic_sq(url, 28., cx);
            let face = match pic {
                Some(src) => div().size(px(28.)).flex_none().rounded(px(8.)).overflow_hidden().bg(c(RAISED_2)).child(img(src).size_full()),
                None => crate::kit::face(name, None, 28.),
            };
            rows = rows.child(
                div()
                    .h(px(46.))
                    .px(px(10.))
                    .when(i > 0, |d| d.border_t_1().border_color(c(LINE)))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(crate::kit::grip())
                    .child(div().w(px(18.)).flex_none().text_size(px(T_S)).text_color(c(MUTED)).font_features(tnum()).child((i + 1).to_string()))
                    .child(face)
                    .child(div().min_w_0().flex_1().truncate().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).child(name.clone()))
                    .child(step(i.saturating_sub(1), "lo-up", "chevronUp", i == 0))
                    .child(step((i + 1).min(count - 1), "lo-down", "chevronDown", i + 1 >= count)),
            );
        }
        let done = this.clone();
        let list = crate::kit::scroll_list("lo-list", &crate::scrolled::handle("lo-list"), 420., rows);
        crate::kit::framed(dialog, words[0].clone(), words[2].clone(), |_, w, cx| w.close_dialog(cx))
            .w(px(crate::kit::DIALOG_W))
            .centered()
            .overlay_closable(false)
            .entrance(DialogEntrance::FadeSlide)
            .child(crate::kit::dialog_body(div().flex().flex_col().gap(px(12.)).child(div().text_size(px(T_M)).line_height(px(20.)).text_color(c(TEXT_2)).child(words[1].clone())).child(list)))
            .footer(
                crate::kit::dialog_foot()
                    .child(Button::new("lo-cancel", tips[2].clone()).on_click(|_, w, cx| w.close_dialog(cx)))
                    .child(Button::new("lo-done", tips[3].clone()).primary().on_click(move |_, w, cx| {
                        w.close_dialog(cx);
                        let _ = done.update(cx, |r, cx| {
                            let ids = r.order_edit.clone();
                            act(r, "setLaunchOrder", json!({ "ids": ids }), cx, |_, _| {});
                        });
                    })),
            )
    });
}

/// Every Roblox window laid out on the main screen, as Settings › Launching says, and how many were (app.js's arrange).
pub fn arrange(r: &mut Root, cx: &mut Context<Root>) {
    let monitor = r.state.get("monitors").and_then(Value::as_array).and_then(|l| l.iter().find(|m| m.get("primary").and_then(Value::as_bool) == Some(true))).and_then(|m| m.get("index")).and_then(Value::as_i64).unwrap_or(0);
    act(r, "arrangeWindows", json!({ "monitor": monitor }), cx, |r, v| {
        let n = v.as_i64().unwrap_or(0);
        r.toast = Some(if n > 0 { (r.i18n.tn("t.arranged", n, &[("n", &n.to_string())]), "ok".into()) } else { (r.tr("t.noWindows"), "info".into()) });
    });
}

/// Close these accounts' windows (or end them, forced), and say how many (app.js's closeIds).
fn close(r: &mut Root, ids: Vec<String>, force: bool, cx: &mut Context<Root>) {
    act(r, "closeAccounts", json!({ "ids": ids, "force": force }), cx, move |r, v| {
        let n = v.as_i64().unwrap_or(0);
        r.toast = Some(if n > 0 { (r.i18n.tn(if force { "t.killing" } else { "t.closing" }, n, &[("n", &n.to_string())]), "ok".into()) } else { (r.tr("t.noneRunning"), "info".into()) });
    });
}

/// Copies one thing of an account's (the app puts it on the clipboard) and says what; a 2FA code says how long it
/// still works (app.js's copy).
pub fn copy(r: &mut Root, id: String, field: &'static str, cx: &mut Context<Root>) {
    act(r, "copy", json!({ "id": id, "field": field }), cx, move |r, v| {
        if v.is_null() {
            return;
        }
        let text = if field == "totp" {
            r.i18n.tv("t.copiedCode", &[("s", &v.get("seconds").and_then(Value::as_i64).unwrap_or(0).to_string())])
        } else if v.get("secret").and_then(Value::as_bool).unwrap_or(false) {
            r.tr("t.copiedSecret")
        } else {
            r.tr(match field {
                "username" => "t.copiedUser",
                "pid" => "t.copiedPid",
                _ => "t.copied",
            })
        };
        r.toast = Some((text, "ok".into()));
    });
}

/// Asks Roblox whether these accounts are still signed in, and says how many are, how many need signing in again, how
/// many couldn't be asked; the ones Roblox holds back (a warning to accept) are said apart (app.js's checkAccounts).
fn check_accounts(r: &mut Root, ids: Vec<String>, cx: &mut Context<Root>) {
    act(r, "checkAccounts", json!({ "ids": ids }), cx, |r, v| {
        if !v.is_object() {
            return;
        }
        let n = |k: &str| v.get(k).and_then(Value::as_i64).unwrap_or(0);
        let (ok, bad, failed, refused) = (n("ok"), n("bad"), n("failed"), n("refused"));
        let mut text = r.i18n.tn(if failed > 0 { "t.checkedFail" } else { "t.checked" }, bad, &[("ok", &ok.to_string()), ("bad", &bad.to_string()), ("f", &failed.to_string())]);
        if refused > 0 {
            let key = if n("moderated") == refused { "t.checkedModerated" } else { "t.checkedRefused" };
            text = format!("{text} {}", r.i18n.tn(key, refused, &[("n", &refused.to_string())]));
        }
        r.toast = Some((text, if bad > 0 || refused > 0 { "warn" } else { "ok" }.into()));
    });
}

/// More for the picked accounts, as the web page's account menu: check their sign-ins, main account, hide or show
/// their windows, another server, roblox.com and Copy for one account, force close, and Delete (after a question).
fn more(root: &Root, picked: &[Value], cx: &mut Context<Root>) -> Div {
    let open = root.acc_menu;
    let ids: Vec<String> = picked.iter().filter(|a| !foreign(a)).map(id_of).collect();
    let local: Vec<String> = picked.iter().filter(|a| !foreign(a) && is_local(a)).map(id_of).collect();
    let one = (ids.len() == 1).then(|| picked.iter().find(|a| !foreign(a)).cloned()).flatten();
    let flag = |a: &Value, k: &str| a.get(k).and_then(Value::as_bool).unwrap_or(false);
    let all_main = !ids.is_empty() && picked.iter().filter(|a| !foreign(a)).all(|a| flag(a, "isMain"));
    let any_hidden = picked.iter().any(hidden);
    let row = |key: &'static str, icon_name: &'static str, label: String, danger: bool| {
        menu_row(key)
            .h(px(32.))
            .gap(px(10.))
            .text_color(c(if danger { BAD } else { TEXT }))
            .child(icon(icon_name, 14., c(if danger { BAD } else { TEXT_2 })))
            .child(div().flex_1().min_w_0().truncate().child(label))
    };
    let line = || div().h(px(1.)).my(px(4.)).mx(px(6.)).bg(c(LINE));
    let mut menu = div()
        .id("acc-more-menu")
        .occlude()
        .w(px(250.))
        .p(px(4.))
        .rounded(px(R_M))
        .bg(c(RAISED_2))
        .border_1()
        .border_color(ca(WHITE, 0.1))
        .shadow_lg()
        .flex()
        .flex_col()
        .on_mouse_down_out(cx.listener(|r, _, _, cx| {
            if r.acc_menu {
                r.acc_menu = false;
                cx.notify();
            }
        }));
    {
        let ids = ids.clone();
        menu = menu.child(row("am-check", "refresh", root.tr("m.check"), false).on_click(cx.listener(move |r, _, _, cx| {
            r.acc_menu = false;
            check_accounts(r, ids.clone(), cx);
            cx.notify();
        })));
    }
    // Who sees them online and who may join them: their own Roblox privacy settings.
    {
        let ids = ids.clone();
        menu = menu.child(row("am-vis", "eye", root.tr("vis.menu"), false).on_click(cx.listener(move |r, _, window, cx| {
            r.acc_menu = false;
            crate::pages::visibility::open(r, ids.clone(), window, cx);
        })));
    }
    {
        let ids = ids.clone();
        menu = menu.child(
            row("am-main", if all_main { "starFill" } else { "star" }, root.tr("m.main"), false)
                .when(all_main, |d| d.child(icon("check", 14., c(TEXT))))
                .on_click(cx.listener(move |r, _, _, cx| {
                    r.acc_menu = false;
                    r.send("setMain", json!({ "ids": ids, "on": !all_main }));
                    cx.notify();
                })),
        );
    }
    {
        // Roblox's own app for these accounts, without a game.
        let ids = ids.clone();
        menu = menu.child(row("am-app", "monitor", root.tr("dock.openAppTip"), false).on_click(cx.listener(move |r, _, _, cx| {
            r.acc_menu = false;
            r.launch("launch", json!({ "ids": ids, "appOnly": true, "placeInput": "", "jobId": "" }), false, cx);
            cx.notify();
        })));
    }
    // Every Roblox window of this PC hidden (their games run on) or shown again, whichever can apply; and all of
    // them laid out on the main screen as Settings › Launching says.
    {
        let all = root.accounts();
        let can_hide = all.iter().any(|a| is_local(a) && !hidden(a));
        let can_show = all.iter().any(hidden);
        menu = menu.child(line());
        if can_hide {
            menu = menu.child(row("am-hide-all", "eyeOff", root.tr("m.hideAll"), false).on_click(cx.listener(|r, _, _, cx| {
                r.acc_menu = false;
                r.send("hideWindows", json!({}));
                cx.notify();
            })));
        }
        if can_show {
            menu = menu.child(row("am-show-all", "eye", root.tr("m.showAll"), false).on_click(cx.listener(|r, _, _, cx| {
                r.acc_menu = false;
                r.send("showWindows", json!({}));
                cx.notify();
            })));
        }
        let monitor = root.state.get("monitors").and_then(Value::as_array).and_then(|l| l.iter().find(|m| m.get("primary").and_then(Value::as_bool) == Some(true))).and_then(|m| m.get("index")).and_then(Value::as_i64).unwrap_or(0);
        menu = menu.child(row("am-arrange", "grid", root.tr("m.arrange"), false).on_click(cx.listener(move |r, _, _, cx| {
            r.acc_menu = false;
            act(r, "arrangeWindows", json!({ "monitor": monitor }), cx, |r, v| {
                let n = v.as_i64().unwrap_or(0);
                r.toast = Some(if n > 0 { (r.i18n.tn("t.arranged", n, &[("n", &n.to_string())]), "ok".into()) } else { (r.tr("t.noWindows"), "info".into()) });
            });
            cx.notify();
        })));
    }
    if !local.is_empty() {
        let (ids, ids2) = (local.clone(), local.clone());
        menu = menu
            .child(line())
            .child(row("am-hide", if any_hidden { "eye" } else { "eyeOff" }, root.tr(if any_hidden { "m.showWindow" } else { "m.hideWindow" }), false).on_click(cx.listener(move |r, _, _, cx| {
                r.acc_menu = false;
                r.send(if any_hidden { "showWindows" } else { "hideWindows" }, json!({ "ids": ids }));
                cx.notify();
            })))
            .child(row("am-switch", "shuffle", root.tr("m.switchServer"), false).on_click(cx.listener(move |r, _, _, cx| {
                r.acc_menu = false;
                r.send("switchServer", json!({ "ids": ids2 }));
                cx.notify();
            })));
    }
    if let Some(a) = one {
        let id = id_of(&a);
        let vng = s(&a, "realm") == "vng";
        let (has_pw, has_code) = (flag(&a, "hasPassword"), flag(&a, "hasAuthenticator"));
        menu = menu.child(line());
        {
            let id = id.clone();
            menu = menu.child(row("am-edit", "edit", root.tr("m.edit"), false).on_click(cx.listener(move |r, _, window, cx| {
                r.acc_menu = false;
                crate::pages::account_form::open(r, Some(id.clone()), window, cx);
                cx.notify();
            })));
        }
        if !vng {
            let id = id.clone();
            // roblox.com signed in as this account, in a window of Bloxkeep's own.
            menu = menu.child(row("am-web", "globe", root.tr("m.website"), false).on_click(cx.listener(move |r, _, _, cx| {
                r.acc_menu = false;
                r.send("openWebsite", json!({ "id": id }));
                cx.notify();
            })));
        }
        for (key, label, field, has, none) in [("am-copy-user", "m.copyUser", "username", true, ""), ("am-copy-pw", "m.copyPw", "password", has_pw, "m.noPw"), ("am-copy-code", "m.copyCode", "totp", has_code, "m.noCode")] {
            let id = id.clone();
            menu = menu.child(row(key, "copy", root.tr(label), false).on_click(cx.listener(move |r, _, _, cx| {
                r.acc_menu = false;
                // Nothing saved to copy: picking it says why (a sign-in never keeps the password).
                if has {
                    copy(r, id.clone(), field, cx);
                } else {
                    r.toast = Some((r.tr(none), "info".into()));
                }
                cx.notify();
            })));
        }
    }
    menu = menu.child(line());
    if !local.is_empty() {
        let ids = local.clone();
        menu = menu.child(row("am-kill", "x", root.tr("m.kill"), true).on_click(cx.listener(move |r, _, _, cx| {
            r.acc_menu = false;
            close(r, ids.clone(), true, cx);
            cx.notify();
        })));
    }
    {
        let n = ids.len() as i64;
        let names: Vec<String> = picked.iter().filter(|a| !foreign(a)).map(label).collect();
        let label = if n > 1 { root.i18n.tv("m.deleteN", &[("n", &n.to_string())]) } else { root.tr("m.delete") };
        menu = menu.child(row("am-delete", "trash", label, true).on_click(cx.listener(move |r, _, window, cx| {
            r.acc_menu = false;
            ask_delete(r, ids.clone(), names.clone(), window, cx);
            cx.notify();
        })));
    }
    div()
        .relative()
        .flex_none()
        .child(IconButton::new("bar-more", "more").size(32.).selected(open).tip(root.tr("dock.moreTip")).on_click(cx.listener(|r, _, _, cx| {
            r.acc_menu = !r.acc_menu;
            cx.notify();
        })))
        .child(div().absolute().bottom(px(38.)).right_0().child(deferred(anchored().anchor(Anchor::BottomRight).snap_to_window_with_margin(px(8.)).child(pop("acc-more-pop", open, menu))).with_priority(1)))
}

/// Today's stays of every account (todayStrips: since local midnight), asked once a minute while cards show, and when.
/// (Shared: each card reads it, and a clone of the handle costs nothing next to one of the whole answer.)
static STRIPS: std::sync::Mutex<Option<(std::sync::Arc<Value>, std::time::Instant)>> = std::sync::Mutex::new(None);
static STRIPS_ASKED: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);

fn want_strips(root: &Root, cx: &mut Context<Root>) {
    let Some(link) = root.link.clone() else { return };
    let Ok(mut asked) = STRIPS_ASKED.lock() else { return };
    if asked.is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(60)) {
        return;
    }
    *asked = Some(std::time::Instant::now());
    let rx = link.call("todayStrips", json!({}));
    cx.spawn(async move |_, cx| {
        if let Some(d) = rx.await.ok().and_then(Result::ok)
            && let Ok(mut s) = STRIPS.lock()
        {
            *s = Some((std::sync::Arc::new(d), std::time::Instant::now()));
            let _ = cx.update(|cx| cx.refresh_windows());
        }
    })
    .detach();
}

/// Today on an account's card, in words (the old DayStrip's thin bar read as noise): how long it played since
/// midnight and how often it dropped, only when it played at all ("Played 2 h 10 min today · 1 drop").
fn day_line(root: &Root, id: &str) -> Option<String> {
    let (d, _) = STRIPS.lock().ok()?.clone()?;
    let stays: Vec<(f64, bool)> = d
        .pointer(&format!("/rows/{id}"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|s| {
            let s = s.as_array()?;
            Some(((s.get(1)?.as_f64()? - s.first()?.as_f64()?).max(0.), s.get(2).and_then(Value::as_i64).unwrap_or(0) == 1))
        })
        .collect();
    let played: f64 = stays.iter().map(|s| s.0).sum();
    if played < 60. {
        return None;
    }
    let drops = stays.iter().filter(|s| s.1).count() as i64;
    let mut t = root.i18n.tv("acc.today", &[("t", &crate::pages::stats::played(root, played as i64))]);
    if drops > 0 {
        t = format!("{t} · {}", root.i18n.tn("acc.todayDrops", drops, &[]));
    }
    Some(t)
}

/// Today on a card or tile as a strip from midnight to midnight (the web page's DayStrip, review 2.1): filled green
/// where the account played, a red tick where a stay ended cut off, a light mark at now. Flat: one hairline track, no
/// glow. Its words ("Today: played 2 h · cut off once") are its tip. Every card and tile has one, so they line up.
fn day_strip(root: &Root, id: &str) -> Stateful<Div> {
    const DAY: f32 = 86400.;
    let data = STRIPS.lock().ok().and_then(|s| s.clone()).map(|(d, _)| d);
    let mut stays: Vec<(f32, f32, bool)> = Vec::new();
    let mut now = None;
    if let Some(d) = data.as_ref() {
        stays = d
            .pointer(&format!("/rows/{id}"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|s| {
                let s = s.as_array()?;
                let (from, to) = (s.first()?.as_f64()? as f32, s.get(1)?.as_f64()? as f32);
                Some((from.clamp(0., DAY), to.clamp(0., DAY), s.get(2).and_then(Value::as_i64).unwrap_or(0) == 1))
            })
            .filter(|(a, b, _)| b > a)
            .collect();
        let at = |k: &str| d.get(k).and_then(Value::as_str).and_then(crate::time::iso_secs);
        now = at("now").zip(at("since")).map(|(n, s)| ((n - s) as f32).clamp(0., DAY));
    }
    let tip: SharedString = day_line(root, id).unwrap_or_else(|| root.tr("card.dayNone")).into();
    let frac = |s: f32| s / DAY;
    div()
        .id(SharedString::from(format!("day-{id}")))
        .relative()
        .w_full()
        .h(px(12.))
        .flex_none()
        // The track, 4 px, in the middle of a 12 px band (room for the ticks above and below it).
        .child(div().absolute().left_0().right_0().top(px(4.)).h(px(4.)).rounded_full().bg(ca(WHITE, 0.08)))
        .children(stays.into_iter().map(|(from, to, cut)| {
            div()
                .absolute()
                .top(px(4.))
                .h(px(4.))
                .left(relative(frac(from)))
                .w(relative(frac(to - from)))
                .min_w(px(2.))
                .rounded_full()
                .bg(c(OK))
                .when(cut, |d| d.child(div().absolute().right(px(-1.)).top(px(-2.)).w(px(2.)).h(px(8.)).rounded(px(1.)).bg(c(BAD))))
        }))
        .when_some(now, |d, n| d.child(div().absolute().top(px(1.)).left(relative(frac(n))).ml(px(-1.)).w(px(2.)).h(px(10.)).rounded(px(1.)).bg(c(TEXT_2))))
        .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
}

/// "Opened 9 days ago" (from Bloxkeep, the last time), or "Never opened".
fn opened_of(root: &Root, a: &Value) -> String {
    match a.get("lastUsed").and_then(Value::as_str).and_then(crate::time::iso_secs) {
        Some(t) => root.i18n.tv("card.opened", &[("t", &crate::time::ago(crate::time::now_secs() - t, &root.i18n))]),
        None => root.tr("card.never"),
    }
}

/// "@username · Display name" under a name (the display name only when the name shown isn't it already).
fn handle_of(a: &Value) -> Option<String> {
    let (user, display, shown) = (s(a, "username"), s(a, "displayName"), label(a));
    let mut parts = Vec::new();
    if !user.is_empty() {
        parts.push(format!("@{user}"));
    }
    if !display.is_empty() && display != shown && display != user {
        parts.push(display.to_string());
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// The note the owner wrote on an account (Edit), when there is one.
fn note_of(a: &Value) -> Option<String> {
    Some(s(a, "note").trim()).filter(|n| !n.is_empty()).map(str::to_string)
}

/// A quiet line of facts under a card's state: an icon, then the words, cut at the card's edge.
fn small_line(mark: &'static str, text: String) -> Div {
    div()
        .h(px(18.))
        .flex()
        .items_center()
        .gap(px(6.))
        .text_size(px(T_S))
        .text_color(c(MUTED))
        .font_features(tnum())
        .child(icon(mark, 12., c(MUTED)))
        .child(div().min_w_0().truncate().child(text))
}

/// The small words a card or tile carries after the name: Main, and the group it is in.
fn tags_of(root: &Root, a: &Value) -> Vec<Div> {
    let mut out = Vec::new();
    if a.get("isMain").and_then(Value::as_bool).unwrap_or(false) {
        out.push(tag(root.tr("card.main"), TEXT_2));
    }
    if let Some(g) = Some(s(a, "group").trim()).filter(|g| !g.is_empty()) {
        out.push(tag(g.to_string(), TEXT_2));
    }
    // Auto-rejoin on: its mark beside the chips, as the web page's card has (review 2.1).
    if rejoin_on(a) {
        let tip: SharedString = root.tr("card.rejoinTip").into();
        out.push(div().flex_none().child(
            div()
                .id(SharedString::from(format!("rejoin-on-{}", id_of(a))))
                .size(px(20.))
                .flex()
                .items_center()
                .justify_center()
                .child(icon("repeat", 13., c(ONLINE)))
                .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()),
        ));
    }
    out
}

/// Edit (the pencil): the account's form, as F2 and its menu's Edit open it.
fn edit_button(root: &Root, a: &Value, at: &str, size: f32, cx: &mut Context<Root>) -> Div {
    let id = id_of(a);
    let key = SharedString::from(format!("{at}-edit-{id}"));
    no_pick(IconButton::new(key, "edit").size(size).tip(root.tr("card.editTip")).on_click(cx.listener(move |r, _, window, cx| crate::pages::account_form::open(r, Some(id.clone()), window, cx))))
}

/// What a card says under its name: its state, when Bloxkeep last opened it (when the state doesn't say so already:
/// online, playing elsewhere), and its note. Today is the strip at its foot.
fn card_facts(root: &Root, a: &Value) -> (String, u32, Option<String>, Option<String>) {
    let (line, tone) = line_of(root, a);
    let opened = opened_of(root, a);
    let opened = (line != opened && line != root.tr("card.never")).then_some(opened);
    (line, tone, opened, note_of(a))
}

/// The lines a card has under its state: when it was opened, and its note. Every card in the grid takes as many as the
/// fullest one, so the rows stay one height.
fn card_lines(root: &Root, a: &Value) -> usize {
    let (_, _, opened, note) = card_facts(root, a);
    opened.is_some() as usize + note.is_some() as usize
}

/// An account not open here: its face, name, @username and Robux; Main and its group; what it does (online, playing
/// elsewhere, last opened…), how long it played today and its note; then its main button (Play, Sign in, Appeal, Cancel
/// rejoin), Edit and its ⋮ at the foot. A click picks it, a double click plays it; picked, warm with a yellow ring.
/// `lines`: the most lines of facts a card in its grid has, so all of them are one height.
fn card(root: &Root, a: &Value, w: f32, lines: usize, cx: &mut Context<Root>) -> impl IntoElement {
    want_strips(root, cx);
    let id = id_of(a);
    let on = root.selected.contains(&id);
    let (line, tone, opened, note) = card_facts(root, a);
    let robux = robux_of(root, a).map(|r| robux_el(root, &id, r));
    let tags = tags_of(root, a);
    let under = handle_of(a);
    let main = action(root, a, "card", cx);
    let pick = id.clone();
    // (Its padding, the face's row, the state's line and the extra lines, today's strip, the foot.)
    let h = 16. + 44. + 12. + 20. + lines as f32 * 20. + 12. + 12. + 14. + 30. + 14.;
    let body = hot(SharedString::from(format!("card-{id}")))
        .lift(3.)
        .w(px(w))
        .h(px(h))
        .pt(px(16.))
        .pb(px(14.))
        .px(px(16.))
        .flex()
        .flex_col()
        .rounded(px(R_L))
        .cursor_pointer()
        .map(|d| {
            if on {
                d.tones(tinted(RAISED, FACE, 0.10), tinted(RAISED_2, FACE, 0.12)).edge(ca(FACE, 0.7), ca(FACE, 0.85))
            } else {
                d.tones(c(RAISED), c(RAISED_2)).edge(c(LINE), ca(WHITE, 0.17))
            }
        })
        .on_click(cx.listener(move |r, e: &ClickEvent, _, cx| {
            // A double click plays this one alone (the first click already picked it).
            if e.click_count() >= 2 {
                r.selected.clear();
                r.selected.insert(pick.clone());
                r.play(std::slice::from_ref(&pick), cx);
            } else {
                toggle(r, &pick);
            }
            cx.notify();
        }))
        .on_mouse_down(MouseButton::Right, on_right(a, cx))
        // The face, the name with its tags and Robux, @username under it.
        .child(
            div()
                .h(px(44.))
                .flex()
                .items_center()
                .gap(px(12.))
                .child(avatar(a, 44., RAISED, cx))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .child(div().min_w_0().flex_shrink(1.).text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).truncate().child(label(a)))
                                .children(tags)
                                .child(div().flex_1())
                                .children(robux),
                        )
                        .when_some(under, |d, u| d.child(div().min_w_0().truncate().text_size(px(T_S)).text_color(c(MUTED)).child(u))),
                )
                .when(on, |d| d.child(div().size(px(22.)).flex_none().rounded(px(7.)).bg(c(FACE)).flex().items_center().justify_center().child(icon("check", 14., c(INK))))),
        )
        // What it does, then today and the note.
        .child(
            div()
                .mt(px(12.))
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .h(px(20.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .text_size(px(T_M))
                        .child(div().size(px(7.)).flex_none().rounded_full().bg(c(dot_of(a))))
                        .child(div().min_w_0().truncate().text_color(c(if tone == TEXT_2 || tone == OK || tone == MUTED { TEXT_2 } else { tone })).child(line)),
                )
                .when_some(opened, |d, t| d.child(small_line("history", t)))
                .when_some(note, |d, n| d.child(small_line("message", n)))
                // Today, at the foot of the facts (the room a card with fewer lines keeps is above it, not a gap).
                .child(div().flex_1())
                .child(day_strip(root, &id)),
        )
        // Its main button, Edit, its ⋮.
        .child(
            div()
                .mt(px(14.))
                .h(px(30.))
                .flex()
                .items_center()
                .gap(px(4.))
                .children(main.map(no_pick))
                .child(div().flex_1())
                .child(edit_button(root, a, "card", 30., cx))
                .child(more_button(root, a, "card", 30., cx)),
        )
        .children(menu::place(root, a, false, cx))
        .relative();
    on_base(if on { c(FACE_BASE) } else { base_dark() }, RISE, R_L + 1., body).w(px(w))
}


/// Play for the picked accounts that are free: into the server whose Job ID is in the bar's box, when one is there and
/// a game is too; else as Play does (the game in the box, or Roblox's home screen).
fn play_picked(r: &mut Root, ids: &[String], cx: &mut Context<Root>) {
    let job = cx.try_global::<menu::Ui>().filter(|u| u.job_open).and_then(|u| u.job.clone()).map(|e| e.read(cx).value().trim().to_string()).unwrap_or_default();
    let place = r.place.read(cx).value().trim().to_string();
    if job.is_empty() || place.is_empty() || ids.is_empty() {
        r.play(ids, cx);
        return;
    }
    r.launch("launch", json!({ "ids": ids, "appOnly": false, "placeInput": place, "jobId": job }), false, cx);
}

/// The server to join, by its Job ID (the web page's Server box): a button until it is wanted (a box needs the window
/// to be made), then the box with an x that puts it away (and empties it: any server again).
fn job_box(root: &Root, compact: bool, cx: &mut Context<Root>) -> AnyElement {
    let ui = cx.try_global::<menu::Ui>();
    let shown = ui.filter(|u| u.job_open).and_then(|u| u.job.clone());
    match shown {
        Some(e) => div()
            .id("bar-job")
            .w(px(if compact { 170. } else { 230. }))
            .flex_none()
            .child(field(&e, Some("server")).child(IconButton::new("bar-job-x", "x").size(26.).tip(root.tr("common.close")).on_click(cx.listener(|_, _, window, cx| {
                let ui = cx.default_global::<menu::Ui>();
                ui.job_open = false;
                if let Some(e) = ui.job.clone() {
                    e.update(cx, |st, cx| st.set_value("", window, cx));
                }
                cx.notify();
            }))))
            .tooltip({
                let tip: SharedString = root.tr("dock.jobTip").into();
                move |_, cx| cx.new(|_| Tip(tip.clone())).into()
            })
            .into_any_element(),
        None => {
            let open = cx.listener(|r: &mut Root, _: &ClickEvent, window: &mut Window, cx: &mut Context<Root>| {
                let hint = r.tr("dock.job");
                let e = match cx.try_global::<menu::Ui>().and_then(|u| u.job.clone()) {
                    Some(e) => e,
                    None => {
                        let e = cx.new(|cx| gpui_kit::component::input::InputState::new(window, cx).placeholder(hint));
                        cx.default_global::<menu::Ui>().job = Some(e.clone());
                        e
                    }
                };
                cx.default_global::<menu::Ui>().job_open = true;
                e.update(cx, |st, cx| st.focus(window, cx));
                cx.notify();
            });
            if compact {
                IconButton::new("bar-job-open", "server").size(34.).tip(root.tr("dock.jobTip")).on_click(open).into_any_element()
            } else {
                div().id("bar-job-wrap").flex_none().child(Button::new("bar-job-open", root.tr("dock.job")).icon("server").quiet().on_click(open)).tooltip({
                    let tip: SharedString = root.tr("dock.jobTip").into();
                    move |_, cx| cx.new(|_| Tip(tip.clone())).into()
                }).into_any_element()
            }
        }
    }
}

/// Browse servers: the game in the box, on its page's Servers tab (the friends' servers, the public ones and the
/// private ones); a server picked there joins with these accounts. There once the box holds a game Roblox knows.
/// Always in the bar (a button that came and went with the box read as missing, review 2.4): greyed, with why, until
/// the box holds a game.
fn browse_servers(root: &Root, compact: bool, cx: &mut Context<Root>) -> Option<AnyElement> {
    let known = root
        .game
        .as_ref()
        .filter(|g| g.get("ok").and_then(Value::as_bool) == Some(true))
        .and_then(|g| g.get("placeId").and_then(Value::as_i64).filter(|p| *p > 0).map(|p| (p, s(g, "name").to_string())));
    let off = known.is_none();
    let go = cx.listener(move |r: &mut Root, _: &ClickEvent, _: &mut Window, cx: &mut Context<Root>| {
        let Some((place, name)) = known.as_ref() else { return };
        crate::pages::games::open_place(r, *place, name);
        r.games.view = crate::pages::games::View::Games;
        r.games.tab = crate::pages::games::Tab::Servers;
        cx.notify();
    });
    let tip: SharedString = root.tr(if off { "dock.serversFirst" } else { "dock.servers" }).into();
    Some(if compact {
        IconButton::new("bar-servers", "screens").size(34.).disabled(off).tip(tip).on_click(go).into_any_element()
    } else {
        div()
            .id("bar-servers-wrap")
            .flex_none()
            .child(Button::new("bar-servers", root.tr("dock.servers")).icon("screens").quiet().disabled(off).on_click(go))
            .when(off, |d| d.tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()))
            .into_any_element()
    })
}

/// At the foot while some are picked: their faces and count, then what can be done for them: play (the game box,
/// games played lately, Play), rejoin those cut off, close those open; the profile when one is picked.
fn bar(root: &Root, picked: &[Value], width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    // Short of room (a window about 1180 wide): the bar's tools keep their icons, their words the tips.
    let compact = width < 1300.;
    let n = picked.len();
    let playable: Vec<String> = picked.iter().filter(|a| can_play(a)).map(id_of).collect();
    let dropped: Vec<Value> = picked.iter().filter(|a| is_kicked(a)).cloned().collect();
    let open: Vec<String> = picked.iter().filter(|a| is_local(a) || launching(a)).map(id_of).collect();
    let profile = (n == 1).then(|| picked[0].get("userId").and_then(Value::as_i64)).flatten().filter(|u| *u > 0);
    let game = root.game.as_ref().filter(|g| g.get("ok").and_then(Value::as_bool) == Some(true));
    let gpic = game.and_then(|g| g.get("iconUrl").and_then(Value::as_str)).filter(|u| !u.is_empty()).and_then(|u| images::pic_sq(u, 34., cx));
    let has_game = !root.place.read(cx).value().trim().is_empty();
    let current = game.and_then(|g| g.get("placeId").and_then(Value::as_i64));
    let recents: Vec<Value> = root
        .state
        .pointer("/settings/recentGames")
        .and_then(Value::as_array)
        .map(|l| l.iter().filter(|r| r.get("placeId").and_then(Value::as_i64) != current).take(3).cloned().collect())
        .unwrap_or_default();
    let verb = root.tr(if has_game { "dock.play" } else { "dock.openHome" });
    let k = playable.len();

    let who = div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(12.))
        .child(div().flex().children(picked.iter().take(4).enumerate().map(|(i, a)| {
            div().when(i > 0, |d| d.ml(px(-10.))).rounded_full().p(px(2.)).bg(c(RAISED)).child(avatar(a, 28., RAISED, cx))
        })))
        .child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).child(root.i18n.tv("ui.nPicked", &[("n", &n.to_string())])))
        .when_some(profile, |d, user_id| {
            d.child(IconButton::new("bar-profile", "external").size(32.).tip(root.tr("ui.profile")).on_click(cx.listener(move |r, _, _, _| {
                r.send("openProfile", json!({ "userId": user_id }))
            })))
        });

    let play = (k > 0).then(|| {
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(
                div()
                    .size(px(34.))
                    .flex_none()
                    .rounded(px(R_M))
                    .overflow_hidden()
                    .bg(ca(WHITE, 0.06))
                    .flex()
                    .items_center()
                    .justify_center()
                    .map(|d| match gpic {
                        Some(i) => d.child(img(i).size_full().object_fit(ObjectFit::Cover)),
                        None => d.child(icon("gamepad", 16., c(MUTED))),
                    }),
            )
            // Choose a game: the ones played lately, the pinned, the saved servers, favourites, popular.
            .child(crate::pages::picker::button(root, cx))
            .children(crate::pages::picker::save_button(root, cx))
            .child(div().flex_1().min_w(px(200.)).child(field(&root.place, None)))
            .child(job_box(root, compact, cx))
            .children(browse_servers(root, compact, cx))
            .children(recents.into_iter().enumerate().map(|(i, r)| {
                let place = r.get("placeId").and_then(Value::as_i64).unwrap_or(0);
                let pic = r.get("iconUrl").and_then(Value::as_str).filter(|u| !u.is_empty()).and_then(|u| images::pic_sq(u, 34., cx));
                let name: SharedString = crate::pages::home::clean(s(&r, "name")).into();
                focusable(div().id(("recent", i)))
                    .size(px(34.))
                    .flex_none()
                    .rounded(px(R_M))
                    .overflow_hidden()
                    .bg(ca(WHITE, 0.06))
                    .cursor_pointer()
                    .hover(|d| d.opacity(0.85))
                    .when_some(pic, |d, p| d.child(img(p).size_full().object_fit(ObjectFit::Cover)))
                    .tooltip(move |_, cx| cx.new(|_| Tip(name.clone())).into())
                    .on_click(cx.listener(move |r, _, window, cx| {
                        let url = format!("https://www.roblox.com/games/{place}");
                        r.place.update(cx, |st, cx| st.set_value(url, window, cx));
                        r.resolve_place(cx);
                    }))
            }))
            // While accounts are being opened one after another: Stop, in Play's place.
            .child(div().flex_none().child(if root.state.get("launching").and_then(Value::as_bool).unwrap_or(false) {
                Button::new("bar-stop", root.tr("dock.stop")).icon("stop").danger().on_click(cx.listener(|r, _, _, _| r.send("cancelLaunch", json!({}))))
            } else {
                Button::new("bar-play", if k > 1 { format!("{verb} · {k}") } else { verb })
                    .icon(if has_game { "play" } else { "screens" })
                    .primary()
                    .on_click(cx.listener(move |r, _, _, cx| play_picked(r, &playable, cx)))
            }))
    });

    let acts = div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.))
        .when(!dropped.is_empty(), |d| {
            let label = root.i18n.tv("ui.rejoinN", &[("n", &dropped.len().to_string())]);
            d.child(Button::new("bar-rejoin", label).icon("refresh").tint().on_click(cx.listener(move |r, _, _, cx| r.rejoin(&dropped, cx))))
        })
        .when(!open.is_empty(), |d| {
            let label = format!("{} · {}", root.tr("card.close"), open.len());
            d.child(Button::new("bar-close", label).icon("power").on_click(cx.listener(move |r, _, _, cx| close(r, open.clone(), false, cx))))
        })
        // Join a player: these accounts into the server someone is in.
        .child({
            let ids: Vec<String> = picked.iter().filter(|a| !foreign(a)).map(id_of).collect();
            let go = cx.listener(move |r: &mut Root, _: &ClickEvent, window: &mut Window, cx: &mut Context<Root>| crate::pages::join::open(r, ids.clone(), window, cx));
            if compact {
                IconButton::new("bar-join-player", "userGo").size(32.).tip(root.tr("jp.buttonTip")).on_click(go).into_any_element()
            } else {
                div().id("bar-join-wrap").flex_none().child(Button::new("bar-join-player", root.tr("jp.button")).icon("userGo").quiet().on_click(go)).tooltip({
                    let tip: SharedString = root.tr("jp.buttonTip").into();
                    move |_, cx| cx.new(|_| Tip(tip.clone())).into()
                }).into_any_element()
            }
        })
        // The presets to run, and Save as preset… with these accounts (pages/preset_menu.rs).
        .child(crate::pages::preset_menu::button(root, !compact, cx))
        .child(more(root, picked, cx))
        .child(IconButton::new("bar-clear", "x").size(32.).tip(root.tr("ui.clearPick")).on_click(cx.listener(|r, _, _, cx| {
            r.selected.clear();
            cx.notify();
        })));

    // Two lines at any width: who and what to do with them, then where to play (one line ran out of room).
    div()
        .flex_none()
        .mx(px(PAD))
        // (Clear of the cards: the scrolling part ends a gap above it, with a hairline round the bar, so a card cut
        // at that edge reads as "more above", not as the bar lying over it.)
        .mt(px(8.))
        .mb(px(PAD))
        .p(px(10.))
        .pl(px(14.))
        .rounded(px(R_L))
        .bg(c(RAISED))
        .border_1()
        .border_color(c(LINE))
        .flex()
        .flex_col()
        .gap(px(10.))
        .child(div().flex().items_center().gap(px(12.)).child(who).child(div().flex_1()).child(acts))
        .children(play)
}
