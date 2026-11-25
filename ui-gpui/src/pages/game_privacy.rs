// Privacy for one game (owner's concept, 2026-11-17): who can join the account and who sees it online while it plays
// this game, picked on the game page beside Play. Set just before a launch from Bloxkeep to that game, each account's
// own settings written down first (prefs "gamePrivacyOn"), and put back once the account has left the game, unless the
// person changed them on Roblox meanwhile (their change stays). Left over after Bloxkeep or the PC was closed: put back
// at the next start, once the vault is open. Uses roblox.rs's visibility/set_visibility with each account's sign-in
// ("robloxSignIn"), as the Visibility box does.
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use crate::{kit::*, roblox, shell::Root, theme::*};
use crate::val::s;

/// The picks, by place: { "<placeId>": { on, online, join } }, in online status' words (AllUsers, Friends…).
const PICKS: &str = "gamePrivacy";
/// The accounts set now: [{ id, place, online: [was, set] | null, join: [was, set] | null, at, seen, tries }].
const ON: &str = "gamePrivacyOn";
/// The choices shown, in Roblox's order; Trusted only when Roblox offers it to the account read.
const WORDS: [&str; 4] = ["AllUsers", "Friends", "TrustedFriends", "NoOne"];
/// A launch that never showed a window: put back after this long.
const NEVER_SEEN_SECS: i64 = 180;

#[derive(Default)]
pub struct Data {
    pub menu: bool,
    pub closed: Option<std::time::Instant>,
    /// The first account's settings now, read as the menu opens: its place, then (name, online, join) or why not.
    read: Option<(i64, Option<Result<(String, Option<roblox::Choice>, Option<roblox::Choice>), String>>)>,
    /// A put-back running (one at a time).
    restoring: bool,
}

#[derive(Clone, Default, PartialEq)]
pub struct Pick {
    pub on: bool,
    pub online: Option<String>,
    pub join: Option<String>,
}

/// Joining says "All" where online status says "AllUsers" (roblox.rs): one word for both here.
fn norm(v: &str) -> &str {
    if v == "All" { "AllUsers" } else { v }
}

pub fn pick(place: i64) -> Pick {
    let v = crate::prefs::get(PICKS).and_then(|m| m.get(place.to_string()).cloned()).unwrap_or(Value::Null);
    let word = |k: &str| v.get(k).and_then(Value::as_str).filter(|w| !w.is_empty()).map(str::to_string);
    Pick { on: v.get("on").and_then(Value::as_bool).unwrap_or(false), online: word("online"), join: word("join") }
}

fn save_pick(place: i64, p: &Pick) {
    let mut all = crate::prefs::get(PICKS).filter(Value::is_object).unwrap_or_else(|| json!({}));
    if p.online.is_none() && p.join.is_none() && !p.on {
        if let Some(m) = all.as_object_mut() {
            m.remove(&place.to_string());
        }
    } else {
        all[place.to_string()] = json!({ "on": p.on, "online": p.online, "join": p.join });
    }
    crate::prefs::set(PICKS, all);
}

fn active() -> Vec<Value> {
    crate::prefs::get(ON).and_then(|v| v.as_array().cloned()).unwrap_or_default()
}

/// How long an account set for a game may stay out of the open vault (another vault open) before it is let go.
const AWAY_KEPT_SECS: i64 = 7 * 24 * 3600;

/// The account's name as it was when its privacy was set (the app's current one first), else its id.
fn name_in(e: &Value) -> String {
    let name = s(e, "name");
    if name.is_empty() { s(e, "id").to_string() } else { name.to_string() }
}

fn save_active(list: &[Value]) {
    crate::prefs::set(ON, Value::Array(list.to_vec()));
}

/// The place a launch goes to: a bare number or a roblox.com game link.
fn place_of(input: &str) -> i64 {
    let t = input.trim();
    if let Ok(n) = t.parse::<i64>() {
        return n;
    }
    t.split_once("/games/").map(|(_, rest)| rest.chars().take_while(char::is_ascii_digit).collect::<String>()).and_then(|d| d.parse().ok()).unwrap_or(0)
}

/// A launch to a game with a privacy picked and on: its accounts, the place and the pick (Root::launch sets them first).
pub fn before_launch(args: &Value) -> Option<(Vec<String>, i64, Pick)> {
    if args.get("appOnly").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let place = place_of(s(args, "placeInput"));
    let p = pick(place);
    if place <= 0 || !p.on || (p.online.is_none() && p.join.is_none()) {
        return None;
    }
    let ids: Vec<String> = args.get("ids").and_then(Value::as_array).into_iter().flatten().filter_map(|v| v.as_str().map(str::to_string)).collect();
    (!ids.is_empty()).then_some((ids, place, p))
}

/// Each account's sign-in, handed over by the app one by one; None for one that has none that works.
async fn signs(link: crate::link::Link, ids: Vec<String>) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    for id in ids {
        let got = link.call("robloxSignIn", json!({ "id": id })).await.ok().and_then(Result::ok);
        let cookie = got.filter(|v| s(v, "id") == id).map(|v| s(&v, "cookie").to_string()).filter(|c| !c.is_empty());
        out.push((id, cookie));
    }
    out
}

/// Sets the pick on each account, its settings read first: { id, place, online, join } or { id, error }.
pub async fn apply(link: crate::link::Link, ids: Vec<String>, place: i64, p: Pick) -> Vec<Value> {
    let signs = signs(link, ids).await;
    roblox::run(move || {
        let mut out = Vec::new();
        for (i, (id, cookie)) in signs.into_iter().enumerate() {
            let Some(cookie) = cookie else {
                out.push(json!({ "id": id, "error": "no working sign-in" }));
                continue;
            };
            if i > 0 {
                std::thread::sleep(std::time::Duration::from_millis(300));
            }
            let (online_now, join_now) = match roblox::visibility(&cookie) {
                Ok((o, j)) => (o.map(|c| c.now), j.map(|c| c.now)),
                Err(e) => {
                    out.push(json!({ "id": id, "error": e }));
                    continue;
                }
            };
            // Only what differs is set, and only what was set is put back.
            let online = p.online.clone().zip(online_now).filter(|(want, now)| norm(want) != norm(now));
            let join = p.join.clone().zip(join_now).filter(|(want, now)| norm(want) != norm(now));
            if online.is_none() && join.is_none() {
                continue;
            }
            match roblox::set_visibility(&cookie, online.as_ref().map(|o| o.0.as_str()), join.as_ref().map(|j| j.0.as_str())) {
                Ok(()) => out.push(json!({
                    "id": id,
                    "place": place,
                    "online": online.map(|(set, was)| json!([was, set])),
                    "join": join.map(|(set, was)| json!([was, set])),
                })),
                Err(e) => out.push(json!({ "id": id, "error": e })),
            }
        }
        out
    })
    .await
    .unwrap_or_default()
}

fn log(line: &str) {
    #[cfg(feature = "core")]
    crate::core::log::write(&line);
    #[cfg(not(feature = "core"))]
    let _ = line;
}

/// What apply did: the set ones written down (an account already set keeps its first "was", its real normal), the
/// failed ones said and in error.log.
pub fn applied(r: &mut Root, got: Vec<Value>) {
    let mut list = active();
    let mut failed = Vec::new();
    for g in got {
        let id = s(&g, "id").to_string();
        if let Some(e) = g.get("error").and_then(Value::as_str) {
            let name = crate::pages::accounts::label_by_id(r, &id).unwrap_or_else(|| id.clone());
            log(&format!("Game privacy: couldn't set it for {name} ({id}) before its launch: {e}"));
            failed.push(format!("{name} ({e})"));
            continue;
        }
        let name = crate::pages::accounts::label_by_id(r, &id).unwrap_or_default();
        log(&format!(
            "Game privacy: set for {name} ({id}) for place {}: online {}, join {}.",
            g.get("place").and_then(Value::as_i64).unwrap_or(0),
            g.get("online").filter(|v| !v.is_null()).map_or("kept".to_string(), Value::to_string),
            g.get("join").filter(|v| !v.is_null()).map_or("kept".to_string(), Value::to_string),
        ));
        let mut entry = json!({ "id": id, "name": name, "place": g.get("place"), "online": g.get("online"), "join": g.get("join"), "at": crate::time::now_secs(), "seen": false, "tries": 0 });
        if let Some(old) = list.iter().find(|e| s(e, "id") == id) {
            for k in ["online", "join"] {
                if let (Some(was), Some(new)) = (old.get(k).and_then(|v| v.get(0)).cloned(), entry.get_mut(k).filter(|v| v.is_array())) {
                    new[0] = was;
                } else if entry.get(k).is_none_or(Value::is_null) {
                    entry[k] = old.get(k).cloned().unwrap_or(Value::Null);
                }
            }
        }
        list.retain(|e| s(e, "id") != id);
        list.push(entry);
    }
    save_active(&list);
    if !failed.is_empty() {
        r.toast = Some((r.i18n.tv("gpv.failed", &[("names", &failed.join(", "))]), "warn".into()));
    }
}

/// Every few seconds: the accounts set for a game that have left it get their settings back.
pub fn tick(r: &mut Root, cx: &mut Context<Root>) {
    if r.gpv.restoring || r.locked() {
        return;
    }
    let mut list = active();
    if list.is_empty() {
        return;
    }
    let now = crate::time::now_secs();
    let mut due = Vec::new();
    let before = list.len();
    list.retain(|e| {
        let id = s(e, "id");
        let here = r.accounts().iter().any(|a| s(a, "id") == id);
        let at = e.get("at").and_then(Value::as_i64).unwrap_or(0);
        let gone = !here && now - at > AWAY_KEPT_SECS;
        if gone {
            log(&format!("Game privacy: {} ({id}) has been out of the open vault for a week; its privacy is no longer waited on (check it in Roblox's settings).", name_in(e)));
        }
        !gone
    });
    let mut changed = list.len() != before;
    for e in list.iter_mut() {
        let id = s(e, "id").to_string();
        if !r.accounts().iter().any(|a| s(a, "id") == id) {
            if !e.get("away").and_then(Value::as_bool).unwrap_or(false) {
                log(&format!("Game privacy: {} ({id}) isn't in the open vault; its privacy is put back when it is.", name_in(e)));
                e["away"] = json!(true);
                changed = true;
            }
            continue;
        }
        let running = r.accounts().iter().any(|a| s(a, "id") == id && s(a, "state") == "running");
        let seen = e.get("seen").and_then(Value::as_bool).unwrap_or(false);
        if running && !seen {
            e["seen"] = json!(true);
            changed = true;
        }
        let at = e.get("at").and_then(Value::as_i64).unwrap_or(0);
        if !running && (seen || now - at > NEVER_SEEN_SECS) {
            due.push(e.clone());
        }
    }
    if changed {
        save_active(&list);
    }
    if due.is_empty() {
        return;
    }
    let Some(link) = r.link.clone() else { return };
    r.gpv.restoring = true;
    cx.spawn(async move |this, cx| {
        let signs = signs(link, due.iter().map(|e| s(e, "id").to_string()).collect()).await;
        let done = roblox::run(move || {
            let mut out = Vec::new();
            for ((id, cookie), e) in signs.into_iter().zip(due) {
                let Some(cookie) = cookie else {
                    out.push(json!({ "id": id, "error": "no working sign-in" }));
                    continue;
                };
                let (online_now, join_now) = match roblox::visibility(&cookie) {
                    Ok((o, j)) => (o.map(|c| c.now).unwrap_or_default(), j.map(|c| c.now).unwrap_or_default()),
                    Err(err) => {
                        out.push(json!({ "id": id, "error": err }));
                        continue;
                    }
                };
                // Put back only what is still as Bloxkeep set it: a change made meanwhile is the person's.
                let back = |k: &str, now: &str| -> Option<String> {
                    let pair = e.get(k)?.as_array()?;
                    let (was, set) = (pair.first()?.as_str()?, pair.get(1)?.as_str()?);
                    (norm(set) == norm(now) && norm(was) != norm(now)).then(|| was.to_string())
                };
                let (online, join) = (back("online", &online_now), back("join", &join_now));
                if online.is_none() && join.is_none() {
                    out.push(json!({ "id": id, "kept": true }));
                    continue;
                }
                match roblox::set_visibility(&cookie, online.as_deref(), join.as_deref()) {
                    Ok(()) => out.push(json!({ "id": id })),
                    Err(err) => out.push(json!({ "id": id, "error": err })),
                }
            }
            out
        })
        .await
        .unwrap_or_default();
        let _ = this.update(cx, |r, cx| {
            r.gpv.restoring = false;
            let mut list = active();
            let mut back = Vec::new();
            let mut gave_up = Vec::new();
            for d in done {
                let id = s(&d, "id").to_string();
                match d.get("error").and_then(Value::as_str) {
                    None => {
                        list.retain(|e| s(e, "id") != id);
                        if d.get("kept").is_none() {
                            back.push(crate::pages::accounts::label_by_id(r, &id).unwrap_or_else(|| "?".into()));
                        }
                    }
                    Some(err) => {
                        let name = list.iter().find(|e| s(e, "id") == id).map(name_in).or_else(|| crate::pages::accounts::label_by_id(r, &id)).unwrap_or_else(|| id.clone());
                        if let Some(e) = list.iter_mut().find(|e| s(e, "id") == id) {
                            let tries = e.get("tries").and_then(Value::as_i64).unwrap_or(0) + 1;
                            e["tries"] = json!(tries);
                            log(&format!("Game privacy: couldn't put {name}'s privacy back ({id}, try {tries} of 5): {err}"));
                            if tries >= 5 {
                                gave_up.push(name);
                            }
                        }
                        list.retain(|e| e.get("tries").and_then(Value::as_i64).unwrap_or(0) < 5);
                    }
                }
            }
            save_active(&list);
            if !gave_up.is_empty() {
                r.toast = Some((r.i18n.tv("gpv.backFailed", &[("names", &gave_up.join(", "))]), "warn".into()));
            } else if !back.is_empty() {
                r.toast = Some((r.i18n.tv("gpv.back", &[("names", &back.join(", "))]), "ok".into()));
            }
            cx.notify();
        });
    })
    .detach();
}

/// The status bar's mark while accounts are set for a game: who, back to normal when they leave it.
/// (Drawn in the status bar: the names come from the accounts read there, so that it is drawn again when they change.)
pub fn chip(r: &Root, cx: &App) -> Option<AnyElement> {
    let list = active();
    if list.is_empty() {
        return None;
    }
    let accounts = r.accounts_in(cx);
    let names: Vec<String> = list.iter().map(|e| accounts.iter().find(|a| s(a, "id") == s(e, "id")).map(crate::pages::accounts::label).unwrap_or_else(|| "?".into())).collect();
    let tip: SharedString = r.i18n.tv("gpv.chipTip", &[("names", &names.join(", "))]).into();
    Some(
        div()
            .id("gpv-chip")
            .flex_none()
            .flex()
            .items_center()
            .gap(px(6.))
            .text_color(c(TEXT_2))
            .child(icon("shieldCheck", 14., c(TEXT_2)))
            .child(r.tr("gpv.chip"))
            .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
            .into_any_element(),
    )
}

/// Reads the first account's settings for the menu (its "Normally" lines and whether Roblox offers Trusted).
fn read(r: &mut Root, place: i64, first: Option<String>, cx: &mut Context<Root>) {
    r.gpv.read = Some((place, None));
    #[cfg(feature = "shot")]
    if std::env::var("BLOXKEEP_SHOT_PRIVACY").is_ok() {
        let c = |now: &str, o: &[&str]| Some(roblox::Choice { now: now.into(), options: o.iter().map(|s| s.to_string()).collect() });
        let name = first.as_deref().map(|id| crate::pages::accounts::label_by_id(r, id).unwrap_or_else(|| "?".into())).unwrap_or_default();
        r.gpv.read = Some((place, Some(Ok((name, c("AllUsers", &["AllUsers", "Friends", "TrustedFriends", "NoOne"]), c("All", &["All", "Friends", "TrustedFriends", "NoOne"]))))));
        return;
    }
    let (Some(link), Some(id)) = (r.link.clone(), first) else {
        r.gpv.read = Some((place, Some(Err(r.tr("gpv.noAccount")))));
        return;
    };
    let name = crate::pages::accounts::label_by_id(r, &id).unwrap_or_else(|| "?".into());
    cx.spawn(async move |this, cx| {
        let signs = signs(link, vec![id]).await;
        let cookie = signs.into_iter().next().and_then(|x| x.1);
        let got = match cookie {
            Some(cookie) => roblox::run(move || roblox::visibility(&cookie)).await.unwrap_or_else(|_| Err(String::new())),
            None => Err("signin".into()),
        };
        let _ = this.update(cx, |r, cx| {
            if r.gpv.read.as_ref().is_some_and(|x| x.0 == place) {
                let got = got.map(|(o, j)| (name.clone(), o, j)).map_err(|e| if e == "signin" { r.tr("vis.skip.signin") } else { r.tr("ps.failed") });
                r.gpv.read = Some((place, Some(got)));
            }
            cx.notify();
        });
    })
    .detach();
}

/// The Privacy key beside Play and its menu: on for this game, who can join, who sees the account online.
pub fn button(root: &Root, place: i64, first: Option<String>, cx: &mut Context<Root>) -> AnyElement {
    let p = pick(place);
    let open = root.gpv.menu;
    let tip: SharedString = root.tr("gpv.tip").into();
    let key = div()
        .id("gm-privacy")
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
        .child(icon(if p.on { "shieldCheck" } else { "shield" }, 16., c(if p.on { TEXT } else { MUTED })))
        .child(root.tr("gpv.key"))
        .when(p.on, |d| d.child(div().size(px(6.)).rounded_full().bg(c(OK))))
        .child(icon(if open { "chevronUp" } else { "chevronDown" }, 14., c(MUTED)))
        .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
        .on_click(cx.listener(move |r, _, _, cx| {
            if r.gpv.closed.is_some_and(|t| t.elapsed().as_millis() < 250) {
                return;
            }
            r.gpv.menu = !r.gpv.menu;
            if r.gpv.menu && !r.gpv.read.as_ref().is_some_and(|x| x.0 == place) {
                read(r, place, first.clone(), cx);
            }
            cx.notify();
        }));

    let read_now = root.gpv.read.as_ref().filter(|x| x.0 == place).and_then(|x| x.1.as_ref());
    let ok = read_now.and_then(|x| x.as_ref().ok());
    let offered = |choice: Option<&roblox::Choice>| -> Vec<&'static str> {
        let opts: Vec<&str> = choice.map(|c| c.options.iter().map(|o| norm(o)).collect()).unwrap_or_default();
        WORDS.iter().copied().filter(|w| if opts.is_empty() { *w != "TrustedFriends" } else { opts.contains(w) }).collect()
    };
    let section = |root: &Root, title: &str, key: &'static str, choice: Option<&roblox::Choice>, chosen: Option<String>, cx: &mut Context<Root>| -> Div {
        let mut d = div().flex().flex_col();
        d = d.child(div().px(px(10.)).pt(px(10.)).pb(px(4.)).text_size(px(T_XS)).font_weight(FontWeight::SEMIBOLD).text_color(c(MUTED)).child(root.tr(title).to_uppercase()));
        for (i, w) in offered(choice).into_iter().enumerate() {
            let on = chosen.as_deref().map(norm) == Some(w);
            d = d.child(
                menu_row((if key == "online" { "gpv-online" } else { "gpv-join" }, i))
                    .child(div().flex_1().min_w_0().truncate().when(on, |d| d.text_color(c(TEXT))).child(root.tr(&format!("vis.{w}"))))
                    .when(on, |d| d.child(tick_mark(16.)))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        let mut p = pick(place);
                        let v = Some(w.to_string());
                        if key == "online" { p.online = v } else { p.join = v }
                        p.on = true;
                        save_pick(place, &p);
                        cx.notify();
                    })),
            );
        }
        if let Some(now) = choice.map(|c| c.now.clone()).filter(|n| !n.is_empty()) {
            let word = root.tr(&format!("vis.{}", norm(&now)));
            d = d.child(div().px(px(10.)).pt(px(2.)).text_size(px(T_S)).text_color(c(MUTED)).child(root.i18n.tv("gpv.normally", &[("v", &word)])));
        }
        d
    };

    let this = cx.weak_entity();
    let mut rows = div().flex().flex_col();
    rows = rows.child(
        div()
            .px(px(10.))
            .py(px(8.))
            .flex()
            .items_center()
            .gap(px(10.))
            .child(div().flex_1().min_w_0().text_color(c(TEXT)).font_weight(FontWeight::SEMIBOLD).child(root.tr("gpv.use")))
            .child(Switch::new("gpv-on", p.on).on_toggle(move |on, _, cx| {
                let on = *on;
                let _ = this.update(cx, |_, cx| {
                    let mut p = pick(place);
                    p.on = on;
                    // (Turned on with nothing picked yet: friends only, the concept's first pick.)
                    if on && p.online.is_none() && p.join.is_none() {
                        p.join = Some("Friends".into());
                    }
                    save_pick(place, &p);
                    cx.notify();
                });
            })),
    );
    match read_now {
        None => rows = rows.child(div().px(px(10.)).pb(px(6.)).text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("vis.reading"))),
        Some(Err(e)) => rows = rows.child(div().px(px(10.)).pb(px(6.)).text_size(px(T_S)).text_color(c(WARN)).child(e.clone())),
        Some(Ok(_)) => {}
    }
    let join_choice = ok.and_then(|x| x.2.as_ref());
    let online_choice = ok.and_then(|x| x.1.as_ref());
    rows = rows.child(section(root, "gpv.join", "join", join_choice, p.join.clone(), cx));
    rows = rows.child(section(root, "gpv.online", "online", online_choice, p.online.clone(), cx));
    rows = rows.child(div().mt(px(8.)).px(px(10.)).py(px(8.)).border_t_1().border_color(c(LINE)).text_size(px(T_S)).line_height(px(17.)).text_color(c(MUTED)).child(root.tr("gpv.note")));

    let menu = div()
        .id("gm-privacy-menu")
        .occlude()
        .w(px(300.))
        .p(px(4.))
        .rounded(px(R_M))
        .bg(c(RAISED))
        .border_1()
        .border_color(c(LINE))
        .on_mouse_down_out(cx.listener(|r, _, _, cx| {
            if r.gpv.menu {
                r.gpv.menu = false;
                r.gpv.closed = Some(std::time::Instant::now());
                cx.notify();
            }
        }))
        .child(rows);
    div()
        .relative()
        .child(key)
        .child(popup(PopAt::BelowLeft(42.), "gm-privacy-pop", open, menu))
        .into_any_element()
}

/// BLOXKEEP_SHOT_PRIVACY: a hidden shot's game with a privacy picked, its menu open and the status bar's mark on.
#[cfg(feature = "shot")]
pub fn shot_open(r: &mut Root, place: i64, cx: &mut Context<Root>) {
    save_pick(place, &Pick { on: true, online: Some("NoOne".into()), join: Some("Friends".into()) });
    let first = r.accounts().iter().map(|a| s(a, "id").to_string()).next();
    if let Some(id) = &first {
        save_active(&[json!({ "id": id, "place": place, "online": ["AllUsers", "NoOne"], "join": ["All", "Friends"], "at": crate::time::now_secs(), "seen": true, "tries": 0 })]);
    }
    r.gpv.menu = true;
    read(r, place, first, cx);
}
