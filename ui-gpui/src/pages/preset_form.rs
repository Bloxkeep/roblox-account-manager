// The preset form (new or edit), shown in the Auto page in place of its list (as a game's page in Games): name, game
// (a link or Place ID pasted, a recent game, or a search by name), kind of server, wait between accounts, Job ID or the
// player to follow, the ticked accounts in launch order, options for its windows, its own Discord channel, the
// schedule. The rules are PresetForm.tsx's and app.js's presetModal: what Save sends (savePreset's args) and what it
// checks before. No browser pickers: choices are chips and switches, times are typed, a date typed or picked from
// Bloxkeep's own calendar (calendar.rs).
use std::collections::HashSet;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};
use crate::val::s;
use crate::time::{civil, day_short};
use crate::ask::answer;

use crate::{
    images,
    kit::*,
    pages::accounts::{avatar, label},
    shell::Root,
    theme::*,
};

const SERVERS_ALL: [&str; 7] = ["any", "most", "fewest", "ping", "random", "job", "follow"];
const SERVERS_EACH: [(&str, &str); 3] = [("eachFewest", "fewest"), ("eachMost", "most"), ("eachRandom", "random")];
const RESTART_HOURS: [i64; 5] = [0, 2, 4, 6, 12];
const DELAYS: [i64; 9] = [0, 5, 10, 15, 30, 45, 60, 90, 120];
const WEEK: [i64; 7] = [1, 2, 3, 4, 5, 6, 0];
const ACCOUNT_PAGE: usize = 40;

pub struct Form {
    id: String,
    is_new: bool,
    /// Something was typed or picked since it opened: leaving asks "Discard changes?" first (app.js's askClose).
    touched: bool,
    /// The saved Discord link, masked (the page never holds it).
    saved_discord: Option<String>,
    name: Entity<InputState>,
    place: Entity<InputState>,
    job: Entity<InputState>,
    follow: Entity<InputState>,
    time: Entity<InputState>,
    date: Entity<InputState>,
    stop_time: Entity<InputState>,
    hours: Entity<InputState>,
    dc_link: Entity<InputState>,
    /// The name filled in from the game picked: it follows the next game picked until one is typed.
    auto_name: String,
    server: String,
    delay: Option<i64>,
    /// Every account, the ticked first in their launch order.
    order: Vec<String>,
    ticked: HashSet<String>,
    account_query: Entity<InputState>,
    account_scope: &'static str,
    account_limit: usize,
    gone: usize,
    afk: Option<bool>,
    rejoin: Option<bool>,
    rejoin_with: String,
    profile: String,
    restart: i64,
    arrange: Option<bool>,
    dc_own: bool,
    sch_on: bool,
    once: bool,
    days: Vec<i64>,
    stop: String,
    /// The game the box names (resolveGame), and a search by name with its answer (None while it runs).
    game: Option<Value>,
    found: Option<(String, Option<Vec<Value>>)>,
    seq: u32,
    pub error: Option<String>,
    saving: bool,
    pub scroll: ScrollHandle,
    /// The list open under a choice box (Server, Wait), and the one a click outside just closed (that click's own
    /// button then only closes it).
    drop: Option<&'static str>,
    drop_closed: Option<(&'static str, std::time::Instant)>,
    /// The date box's calendar while it is open (the month on it), and when a click outside last closed it (its own
    /// button's click then only closes it).
    cal: Option<(i32, u32)>,
    cal_closed: Option<std::time::Instant>,
    /// A ticked account held by its grip: its row follows the mouse among the ticked.
    held: Option<String>,
}

/// A link to a game or a Place ID (the box looks it up rather than searching for it).
fn looks_like_target(v: &str) -> bool {
    let v = v.trim();
    v.contains("roblox.com/") || (v.len() >= 3 && v.chars().all(|c| c.is_ascii_digit()))
}

/// "20:00" from "20:00", "8:00 PM", "8pm", "7:30 p.m.", "2000", "830", "8.30", "20h30" (TimeField.tsx's parseTime);
/// Err for anything else, None for empty. Read char by char: no byte slicing, so no text can make it panic.
pub(crate) fn parse_time(v: &str) -> Result<Option<String>, ()> {
    let v: Vec<char> = v.trim().to_lowercase().chars().collect();
    if v.is_empty() {
        return Ok(None);
    }
    let digit = |i: usize| v.get(i).is_some_and(char::is_ascii_digit);
    let num = |from: usize, len: usize| v[from..from + len].iter().fold(0u32, |n, c| n * 10 + c.to_digit(10).unwrap_or(0));
    let ws = |mut i: usize| {
        while v.get(i).is_some_and(|c| c.is_whitespace()) {
            i += 1;
        }
        i
    };
    // The end: `\s*([ap])?\.?\s*(?:m\.?)?$`; Some(whether a/p and which) when it reaches the end.
    let tail = |i: usize| -> Option<Option<bool>> {
        let mut i = ws(i);
        let ap = match v.get(i) {
            Some('a') => Some(false),
            Some('p') => Some(true),
            _ => None,
        };
        if ap.is_some() {
            i += 1;
        }
        if v.get(i) == Some(&'.') {
            i += 1;
        }
        i = ws(i);
        if v.get(i) == Some(&'m') {
            i += 1;
            if v.get(i) == Some(&'.') {
                i += 1;
            }
        }
        (i == v.len()).then_some(ap)
    };
    // The hour's one or two digits (two first, as the pattern's greedy \d{1,2}), each with its minutes first, then without.
    let mut found = None;
    'hours: for hl in [2usize, 1] {
        if !(0..hl).all(digit) {
            continue;
        }
        // `(?:\s*[:.h]?\s*(\d{2}))?`
        let mut i = ws(hl);
        if matches!(v.get(i), Some(':' | '.' | 'h')) {
            i = ws(i + 1);
        }
        if digit(i) && digit(i + 1) {
            if let Some(ap) = tail(i + 2) {
                found = Some((num(0, hl), num(i, 2), ap));
                break 'hours;
            }
        }
        if let Some(ap) = tail(hl) {
            found = Some((num(0, hl), 0, ap));
            break 'hours;
        }
    }
    let Some((mut h, m, ap)) = found else { return Err(()) };
    if let Some(pm) = ap {
        if !(1..=12).contains(&h) {
            return Err(());
        }
        h = h % 12 + if pm { 12 } else { 0 };
    }
    if h <= 23 && m <= 59 { Ok(Some(format!("{h:02}:{m:02}"))) } else { Err(()) }
}

/// "2026-12-24" from that, or from "24/12/2026", "24-12-26" in the language's order (day first in Vietnamese,
/// month first in English: TimeField.tsx's parseDate); Err for anything else or a day that doesn't exist, None for empty.
fn parse_date(v: &str) -> Result<Option<String>, ()> {
    let v = v.trim();
    if v.is_empty() {
        return Ok(None);
    }
    if v.chars().any(|c| !(c.is_ascii_digit() || c.is_whitespace() || matches!(c, '.' | '/' | '-'))) {
        return Err(());
    }
    let parts: Vec<&str> = v.split(|c: char| !c.is_ascii_digit()).filter(|x| !x.is_empty()).collect();
    if parts.len() != 3 {
        return Err(());
    }
    // A number too long for any date is no date (and can't overflow).
    let n: Vec<u32> = parts.iter().map(|x| if x.len() > 6 { u32::MAX } else { x.parse().unwrap_or(u32::MAX) }).collect();
    let iso = v.len() > 4 && v.as_bytes()[..4].iter().all(u8::is_ascii_digit) && !v.as_bytes()[4].is_ascii_digit();
    let (y, m, d) = if iso {
        (n[0], n[1], n[2])
    } else if crate::time::english() {
        (n[2], n[0], n[1])
    } else {
        (n[2], n[1], n[0])
    };
    let y = if y < 100 { 2000 + y } else { y };
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let last = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return Err(()),
    };
    if (1000..=9999).contains(&y) && (1..=last).contains(&d) { Ok(Some(format!("{y:04}-{m:02}-{d:02}"))) } else { Err(()) }
}

fn tri_of(v: &Value, k: &str) -> Option<bool> {
    v.get(k).and_then(Value::as_bool)
}

/// Opens the form: a preset to edit, or a new one (the accounts picked on the Accounts page, the game in its box).
pub fn open(r: &mut Root, p: Option<Value>, window: &mut Window, cx: &mut Context<Root>) {
    let accounts: Vec<Value> = r.accounts().to_vec();
    if accounts.is_empty() {
        r.toast = Some((r.tr("rc.noAccounts"), "info".into()));
        cx.notify();
        return;
    }
    let is_new = p.is_none();
    let rank: Vec<String> = r.state.pointer("/settings/launchOrder").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    let pos = |id: &str| rank.iter().position(|x| x == id).unwrap_or(usize::MAX);
    let base = p.unwrap_or_else(|| {
        let mut ids: Vec<String> = r.selected.iter().cloned().collect();
        ids.sort_by_key(|id| pos(id));
        let place = r.place.read(cx).value().trim().to_string();
        let name = r.game.as_ref().filter(|g| g.get("ok").and_then(Value::as_bool) == Some(true)).map(|g| s(g, "name").to_string()).unwrap_or_default();
        json!({ "name": name, "accountIds": ids, "placeInput": place, "server": "any", "jobId": "", "followPlayer": "", "delaySeconds": null, "schedule": null })
    });
    let all_ids: HashSet<&str> = accounts.iter().map(|a| s(a, "id")).collect();
    let saved: Vec<String> = base.get("accountIds").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    let chosen: Vec<String> = saved.iter().filter(|id| all_ids.contains(id.as_str())).cloned().collect();
    let gone = saved.len() - chosen.len();
    let mut rest: Vec<String> = accounts.iter().map(|a| s(a, "id").to_string()).filter(|id| !chosen.contains(id)).collect();
    rest.sort_by_key(|id| pos(id));
    let ticked: HashSet<String> = chosen.iter().cloned().collect();
    let order: Vec<String> = chosen.into_iter().chain(rest).collect();

    let sc = base.get("schedule").filter(|v| v.is_object()).cloned().unwrap_or(json!({}));
    let mk = |hint: String, value: &str, window: &mut Window, cx: &mut Context<Root>| {
        let e = cx.new(|cx| InputState::new(window, cx).placeholder(hint));
        if !value.is_empty() {
            let v = value.to_string();
            e.update(cx, |st, cx| st.set_value(v, window, cx));
        }
        cx.subscribe(&e, |r: &mut Root, _, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                if let Some(f) = r.auto.form.as_mut() {
                    f.touched = true;
                }
                cx.notify();
            }
        })
        .detach();
        e
    };
    let today = {
        let t = crate::time::now_secs() + crate::time::utc_offset_secs();
        let days = t.div_euclid(86400);
        let (y, m, d) = civil(days);
        format!("{y:04}-{m:02}-{d:02}")
    };
    let name = mk(r.tr("ps.namePh"), s(&base, "name"), window, cx);
    let account_query = cx.new(|cx| InputState::new(window, cx).placeholder(r.tr("ps.acc.search")));
    cx.subscribe(&account_query, |r, _, event: &InputEvent, cx| {
        if matches!(event, InputEvent::Change) && let Some(form) = r.auto.form.as_mut() {
            form.account_limit = ACCOUNT_PAGE;
            cx.notify();
        }
    }).detach();
    let place = mk(r.tr("ps.gamePh"), s(&base, "placeInput"), window, cx);
    let job = mk("xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx".into(), s(&base, "jobId"), window, cx);
    let follow = mk(r.tr("jp.placeholder"), s(&base, "followPlayer"), window, cx);
    let time = mk("20:00".into(), s(&sc, "time"), window, cx);
    let date = mk(today.clone(), if s(&sc, "date").is_empty() { &today } else { s(&sc, "date") }, window, cx);
    let stop_time = mk("23:00".into(), s(&sc, "stopTime"), window, cx);
    let hours_v = sc.get("stopHours").and_then(Value::as_f64).filter(|h| *h > 0.).map_or("2".to_string(), |h| if h.fract() == 0. { format!("{h:.0}") } else { h.to_string() });
    let hours = mk("2".into(), &hours_v, window, cx);
    let saved_discord = Some(s(&base, "discord").to_string()).filter(|d| !d.is_empty());
    let dc_link = mk(saved_discord.clone().unwrap_or_else(|| "https://discord.com/api/webhooks/…".into()), "", window, cx);
    // The game box: a link looked up as it is pasted, a name searched on Enter.
    cx.subscribe(&place, |r, e, ev: &InputEvent, cx| {
        let v = e.read(cx).value().trim().to_string();
        match ev {
            InputEvent::Change => {
                if let Some(f) = r.auto.form.as_mut() {
                    f.found = None;
                    f.game = None;
                }
                if looks_like_target(&v) {
                    resolve(r, v, cx);
                } else if v.chars().count() >= 2 {
                    // A name: searched once typing pauses (PresetForm.tsx's 200 ms), as well as on Enter.
                    let boxed = e.clone();
                    cx.spawn(async move |this, cx| {
                        cx.background_executor().timer(std::time::Duration::from_millis(200)).await;
                        let _ = this.update(cx, |r, cx| {
                            if r.auto.form.is_some() && boxed.read(cx).value().trim() == v {
                                search(r, v, cx);
                            }
                        });
                    })
                    .detach();
                }
            }
            InputEvent::PressEnter { .. } if !v.is_empty() && !looks_like_target(&v) => search(r, v, cx),
            _ => {}
        }
    })
    .detach();

    let rejoin_with = s(&base, "rejoinWith").to_string();
    let form = Form {
        id: s(&base, "id").to_string(),
        is_new,
        saved_discord: saved_discord.clone(),
        auto_name: if is_new { s(&base, "name").to_string() } else { String::new() },
        server: Some(s(&base, "server")).filter(|x| !x.is_empty()).unwrap_or("any").to_string(),
        delay: base.get("delaySeconds").and_then(Value::as_i64),
        order,
        ticked,
        account_query,
        account_scope: "all",
        account_limit: ACCOUNT_PAGE,
        gone,
        afk: tri_of(&base, "antiAfk"),
        rejoin: tri_of(&base, "autoRejoin"),
        rejoin_with: rejoin_with.clone(),
        profile: s(&base, "profile").to_string(),
        restart: base.get("restartHours").and_then(Value::as_i64).unwrap_or(0),
        arrange: tri_of(&base, "arrange"),
        dc_own: saved_discord.is_some(),
        sch_on: sc.get("enabled").and_then(Value::as_bool).unwrap_or(false),
        once: !s(&sc, "date").is_empty(),
        days: sc.get("days").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_i64).collect()).unwrap_or_default(),
        stop: Some(s(&sc, "stop")).filter(|x| !x.is_empty()).unwrap_or("never").to_string(),
        game: None,
        found: None,
        seq: 0,
        error: None,
        saving: false,
        touched: false,
        scroll: ScrollHandle::new(),
        drop: None,
        drop_closed: None,
        cal: None,
        cal_closed: None,
        held: None,
        name,
        place,
        job,
        follow,
        time,
        date,
        stop_time,
        hours,
        dc_link,
    };
    let start = input_text(&form.place, cx);
    r.auto.form = Some(form);
    // BLOXKEEP_SHOT_CAL=1: a shot with the schedule on, set to a date, and the date's calendar open.
    #[cfg(feature = "shot")]
    if std::env::var("BLOXKEEP_SHOT_CAL").as_deref() == Ok("1") {
        if let Some(f) = r.auto.form.as_mut() {
            f.sch_on = true;
            f.once = true;
            f.cal = Some(crate::calendar::start_month(parse_date(&input_text(&f.date, cx)).ok().flatten().as_deref()));
        }
    }
    if looks_like_target(&start) {
        resolve(r, start, cx);
    }
    cx.notify();
}

/// Looks the box's link or Place ID up (resolveGame): its name and icon under the box.
fn resolve(r: &mut Root, value: String, cx: &mut Context<Root>) {
    let asked = r.resolve_game(value);
    let Some(f) = r.auto.form.as_mut() else { return };
    f.seq += 1;
    let mine = f.seq;
    cx.spawn(async move |this, cx| {
        let Some(g) = asked.await else { return };
        let _ = this.update(cx, |r, cx| {
            if let Some(f) = r.auto.form.as_mut().filter(|f| f.seq == mine) {
                f.game = (g.get("ok").and_then(Value::as_bool) == Some(true)).then_some(g);
                cx.notify();
            }
        });
    })
    .detach();
}

/// A hidden shot's lookup (BLOXKEEP_SHOT_PRESET_GAME).
#[cfg(feature = "shot")]
pub fn shot_resolve(r: &mut Root, value: String, cx: &mut Context<Root>) {
    resolve(r, value, cx);
}

/// A hidden shot's search (BLOXKEEP_SHOT_PRESET_FIND).
#[cfg(feature = "shot")]
pub fn shot_find(r: &mut Root, query: String, cx: &mut Context<Root>) {
    search(r, query, cx);
}

/// A search by name: its games under the box to pick from. Roblox is asked from here (roblox::search_rows; it was
/// the C# app's searchGames command until 2026-11-11).
fn search(r: &mut Root, query: String, cx: &mut Context<Root>) {
    let Some(f) = r.auto.form.as_mut() else { return };
    f.seq += 1;
    let mine = f.seq;
    f.found = Some((query.clone(), None));
    // (A new search starts its list at the top.)
    crate::scrolled::handle("ps-found-list").set_offset(point(px(0.), px(0.)));
    let ask = query.clone();
    let rx = crate::roblox::run(move || crate::roblox::search_rows(&ask));
    cx.notify();
    cx.spawn(async move |this, cx| {
        let list = rx.await.unwrap_or_default();
        let _ = this.update(cx, |r, cx| {
            if let Some(f) = r.auto.form.as_mut().filter(|f| f.seq == mine) {
                f.found = Some((query, Some(list)));
                cx.notify();
            }
        });
    })
    .detach();
}

/// A game picked from the recent ones or a search: its link in the box, its name as the preset's while none is typed.
fn choose(r: &mut Root, g: Value, window: &mut Window, cx: &mut Context<Root>) {
    let Some(f) = r.auto.form.as_mut() else { return };
    let place = g.get("placeId").and_then(Value::as_i64).unwrap_or(0);
    let name = s(&g, "name").to_string();
    let (place_box, name_box) = (f.place.clone(), f.name.clone());
    let typed = input_text(&name_box, cx);
    let follow_name = typed.is_empty() || typed == f.auto_name;
    if follow_name {
        f.auto_name = name.clone();
    }
    place_box.update(cx, |st, cx| st.set_value(format!("https://www.roblox.com/games/{place}"), window, cx));
    if follow_name {
        name_box.update(cx, |st, cx| st.set_value(name, window, cx));
    }
    if let Some(f) = r.auto.form.as_mut() {
        f.found = None;
        f.game = Some(g);
    }
    cx.notify();
}

/// What Save sends (savePreset's args), or why it can't.
fn collect(r: &Root, f: &Form, cx: &App) -> Result<Value, String> {
    // (A game looked up without its link in the box, as a shot's: its link.)
    let place = match (input_text(&f.place, cx), f.game.as_ref().and_then(|g| g.get("placeId")).and_then(Value::as_i64)) {
        (p, Some(id)) if p.is_empty() => format!("https://www.roblox.com/games/{id}"),
        (p, _) => p,
    };
    let server = if !place.is_empty() || f.server == "follow" { f.server.clone() } else { "any".into() };
    let example = |kind: &str| if kind == "time" { "20:00".to_string() } else if r.i18n.vietnamese { "24/12/2026".into() } else { "2026-12-24".into() };
    let bad = |kind: &str| r.i18n.tv(if kind == "time" { "tf.badTime" } else { "tf.badDate" }, &[("example", &example(kind))]);
    let tm = parse_time(&input_text(&f.time, cx)).map_err(|_| bad("time"));
    let dt = parse_date(&input_text(&f.date, cx)).map_err(|_| bad("date"));
    let st = parse_time(&input_text(&f.stop_time, cx)).map_err(|_| bad("time"));
    let tm = if f.sch_on { tm? } else { tm.unwrap_or(None) };
    let dt = if f.sch_on && f.once { dt? } else { dt.unwrap_or(None) };
    let st = if f.sch_on && f.stop == "at" { st? } else { st.unwrap_or(None) };
    let hours: f64 = input_text(&f.hours, cx).replace(',', ".").parse().unwrap_or(-1.);
    if f.sch_on && f.stop == "after" && !((0.5..=168.).contains(&hours) && (hours * 2.).fract() == 0.) {
        return Err(r.tr("ps.schHoursBad"));
    }
    // Its own Discord channel: none keeps the saved link, "" goes back to Settings' channel, a link replaces it.
    let link = input_text(&f.dc_link, cx);
    let discord = if !f.dc_own {
        f.saved_discord.as_ref().map(|_| json!(""))
    } else if !link.is_empty() {
        Some(json!(link))
    } else if f.saved_discord.is_none() {
        return Err(r.tr("ps.dc.needLink"));
    } else {
        None
    };
    let ids: Vec<&String> = f.order.iter().filter(|id| f.ticked.contains(*id)).collect();
    let with_on = !place.is_empty() && server != "follow" && f.rejoin != Some(false);
    let with = if with_on && (f.rejoin_with.is_empty() || f.rejoin_with == "any" || f.ticked.contains(&f.rejoin_with)) { f.rejoin_with.clone() } else { String::new() };
    let schedule = if !f.sch_on && tm.is_none() {
        Value::Null
    } else {
        json!({
            "enabled": f.sch_on,
            "time": tm.unwrap_or_default(),
            "days": if f.once { Vec::new() } else { WEEK.iter().copied().filter(|d| f.days.contains(d)).collect::<Vec<_>>() },
            "date": if f.once { dt.unwrap_or_default() } else { String::new() },
            "stop": f.stop,
            "stopHours": if hours > 0. { hours } else { 0. },
            "stopTime": st.unwrap_or_default(),
        })
    };
    let mut args = json!({
        "id": f.id, "name": input_text(&f.name, cx), "placeInput": place, "server": server, "jobId": input_text(&f.job, cx), "followPlayer": input_text(&f.follow, cx),
        "ids": ids, "delaySeconds": f.delay, "antiAfk": f.afk, "autoRejoin": f.rejoin, "rejoinWith": with, "profile": f.profile,
        "restartHours": f.restart, "arrange": f.arrange, "schedule": schedule,
    });
    if let Some(d) = discord {
        args["discordWebhook"] = d;
    }
    Ok(args)
}

fn save(r: &mut Root, cx: &mut Context<Root>) {
    let Some(f) = r.auto.form.as_ref() else { return };
    if f.saving {
        return;
    }
    let args = match collect(r, f, cx) {
        Ok(a) => a,
        Err(e) => {
            if let Some(f) = r.auto.form.as_mut() {
                f.error = Some(e);
            }
            cx.notify();
            return;
        }
    };
    let Some(link) = r.link.clone() else { return };
    if let Some(f) = r.auto.form.as_mut() {
        f.saving = true;
        f.error = None;
    }
    let rx = link.call("savePreset", args);
    cx.notify();
    answer(rx, cx, move |r, answer, cx| {
        match answer {
            Ok(Ok(_)) => r.auto.form = None,
            Ok(Err(e)) => {
                if let Some(f) = r.auto.form.as_mut() {
                    f.saving = false;
                    f.error = Some(e);
                }
            }
            Err(_) => {
                if let Some(f) = r.auto.form.as_mut() {
                    f.saving = false;
                }
            }
        }
        cx.notify();
    });
}

// ───────────────────────── drawing ─────────────────────────

/// A change to the form from a click.
/// Back or Cancel: the form goes, asking first when something in it would be lost.
pub fn leave(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    if r.auto.form.as_ref().is_some_and(|f| f.touched && !f.saving) {
        r.ask_discard(window, cx, |r, _, cx| {
            r.auto.form = None;
            cx.notify();
        });
    } else {
        r.auto.form = None;
        cx.notify();
    }
}

fn edit(cx: &mut Context<Root>, f: impl Fn(&mut Form) + 'static) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    cx.listener(move |r, _, _, cx| {
        if let Some(form) = r.auto.form.as_mut() {
            f(form);
            form.error = None;
            form.touched = true;
        }
        cx.notify();
    })
}

/// A row of choices as chips (a choice a game is needed for is off without one).
fn chips(cx: &mut Context<Root>, key: &'static str, items: Vec<(String, String, bool)>, selected: &str, f: impl Fn(&mut Form, &str) + 'static) -> Div {
    let f = std::rc::Rc::new(f);
    let mut row = div().flex().flex_wrap().gap(px(6.));
    for (i, (value, label, off)) in items.into_iter().enumerate() {
        let on = value == selected;
        let f = f.clone();
        row = row.child(
            pill((key, i), on, !off)
                .child(label)
                .when(!off, |d| d.on_click(edit(cx, move |form| f(form, &value)))),
        );
    }
    row
}

/// Follow Settings / On / Off.
fn tri(root: &Root, cx: &mut Context<Root>, key: &'static str, v: Option<bool>, follow: String, set: impl Fn(&mut Form, Option<bool>) + 'static) -> Div {
    let cur = match v {
        Some(true) => "on",
        Some(false) => "off",
        None => "",
    };
    chips(cx, key, vec![("".into(), follow, false), ("on".into(), root.tr("ps.opt.on"), false), ("off".into(), root.tr("ps.opt.off"), false)], cur, move |f, x| {
        set(f, match x {
            "on" => Some(true),
            "off" => Some(false),
            _ => None,
        })
    })
}

fn labelled(label: String, body: impl IntoElement) -> Div {
    div().flex().flex_col().gap(px(8.)).child(div().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).text_color(c(TEXT_2)).child(label)).child(body)
}

fn boxed(e: &Entity<InputState>) -> Field {
    field(e, None).h(px(36.)).px(px(12.))
}

/// A choice box (the web form's base-select, no list of the system's): its choice and a chevron; clicked, its choices
/// in a list under it, in groups with a heading each, the one chosen ticked, one that needs a game dimmed.
#[allow(clippy::type_complexity)]
fn select(
    root: &Root,
    cx: &mut Context<Root>,
    key: &'static str,
    groups: Vec<(Option<String>, Vec<(String, String, bool)>)>,
    selected: &str,
    set: impl Fn(&mut Form, &str) + 'static,
) -> Div {
    let Some(f) = root.auto.form.as_ref() else { return div() };
    let open = f.drop == Some(key);
    let mut shown = groups.iter().flat_map(|(_, l)| l.iter()).find(|(v, _, _)| v == selected).map(|(_, l, _)| l.clone()).unwrap_or_default();
    // A server picked for each account says so when closed ("Each account: Most players", PresetForm.tsx); its list's
    // heading says it when open.
    if key == "ps-srv" && selected.starts_with("each") && !shown.is_empty() {
        shown = format!("{}{shown}", root.tr("ps.srvEachIn"));
    }
    let button = focusable(div().id(key))
        .h(px(40.))
        .px(px(12.))
        .flex()
        .items_center()
        .gap(px(8.))
        .rounded(px(R_M))
        .bg(ca(WHITE, if open { 0.08 } else { 0.045 }))
        .border_color(if open { ca(TEXT, 0.35) } else { c(LINE) })
        .cursor_pointer()
        .hover(|d| d.bg(ca(WHITE, 0.08)))
        .child(div().flex_1().min_w_0().truncate().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).child(shown))
        .child(icon(if open { "chevronUp" } else { "chevronDown" }, 16., c(MUTED)))
        .on_click(cx.listener(move |r, _, _, cx| {
            if let Some(f) = r.auto.form.as_mut() {
                let just = f.drop_closed.is_some_and(|(k, at)| k == key && at.elapsed().as_millis() < 300);
                f.drop_closed = None;
                f.drop = if just || f.drop == Some(key) { None } else { Some(key) };
            }
            cx.notify();
        }));
    let wrap = div().relative().flex().flex_col().child(button);
    if !open {
        return wrap;
    }
    let set = std::rc::Rc::new(set);
    let mut menu = div()
        .id(SharedString::from(format!("{key}-menu")))
        .occlude()
        .min_w(px(280.))
        .max_h(px(340.))
        .overflow_y_scroll()
        .p(px(4.))
        .rounded(px(R_M))
        .bg(c(RAISED))
        .border_1()
        .border_color(c(LINE))
        .shadow_lg()
        .flex()
        .flex_col()
        .on_mouse_down_out(cx.listener(move |r, _, _, cx| {
            if let Some(f) = r.auto.form.as_mut().filter(|f| f.drop == Some(key)) {
                f.drop = None;
                f.drop_closed = Some((key, std::time::Instant::now()));
                cx.notify();
            }
        }));
    let mut k = 0_usize;
    for (g, (head, items)) in groups.into_iter().enumerate() {
        if g > 0 {
            menu = menu.child(div().my(px(4.)).mx(px(6.)).h(px(1.)).bg(c(LINE)));
        }
        if let Some(h) = head {
            menu = menu.child(div().px(px(10.)).pt(px(6.)).pb(px(4.)).text_size(px(T_XS)).font_weight(FontWeight::SEMIBOLD).text_color(c(MUTED)).child(h));
        }
        for (value, label, off) in items {
            k += 1;
            let on = value == selected;
            let row = menu_row((key, k))
                .text_color(if off { c(DIM) } else if on { c(TEXT) } else { c(TEXT_2) })
                .child(div().flex_1().min_w_0().whitespace_nowrap().child(label))
                .child(div().w(px(14.)).flex_none().when(on, |d| d.child(icon("check", 14., c(TEXT)))));
            menu = menu.child(if off {
                row.cursor_default().hover(|d| d.text_color(c(DIM))).into_any_element()
            } else {
                let set = set.clone();
                row.on_click(cx.listener(move |r, _, _, cx| {
                    if let Some(f) = r.auto.form.as_mut() {
                        set(f, &value);
                        f.drop = None;
                        f.error = None;
                    }
                    cx.notify();
                }))
                .into_any_element()
            });
        }
    }
    wrap.child(popup(PopAt::BelowLeft(44.), SharedString::from(format!("{key}-pop")), open, menu))
}

/// The one-off date: typed, or picked from Bloxkeep's own calendar under the box's calendar button (TimeField.tsx's
/// DateInput, past days off). A day picked goes into the box as the form opened it ("2026-12-24").
fn date_box(root: &Root, f: &Form, cx: &mut Context<Root>) -> Div {
    let button = IconButton::new("ps-cal-btn", "calendar").size(28.).on(f.cal.is_some()).tip(root.tr("tf.pickDate")).on_click(cx.listener(|r, _, _, cx| {
        if let Some(f) = r.auto.form.as_mut() {
            let just = f.cal_closed.is_some_and(|at| at.elapsed().as_millis() < 300);
            f.cal_closed = None;
            f.cal = if just || f.cal.is_some() { None } else { Some(crate::calendar::start_month(parse_date(&input_text(&f.date, cx)).ok().flatten().as_deref())) };
        }
        cx.notify();
    }));
    let wrap = div().relative().w(px(180.)).child(boxed(&f.date).child(button));
    let Some(shown) = f.cal else { return wrap };
    let chosen = parse_date(&input_text(&f.date, cx)).ok().flatten();
    let panel = crate::calendar::panel("ps-cal", &root.i18n, shown, chosen.as_deref(), true, cx, |r: &mut Root, act, window, cx| {
        let Some(f) = r.auto.form.as_mut() else { return };
        match act {
            crate::calendar::Act::Shift(n) => f.cal = f.cal.map(|m| crate::calendar::shift(m, n)),
            crate::calendar::Act::Close => {
                if f.cal.take().is_some() {
                    f.cal_closed = Some(std::time::Instant::now());
                }
            }
            crate::calendar::Act::Pick(day) => {
                f.cal = None;
                f.touched = true;
                f.error = None;
                let date = f.date.clone();
                date.update(cx, |st, cx| st.set_value(day, window, cx));
            }
        }
        cx.notify();
    });
    wrap.child(crate::calendar::popover("ps-cal", 40., panel))
}

/// The page's sides (the Accounts page's).
const SIDE: f32 = 24.;

/// A section of the form: a calm panel with its head (title, a muted line under it, its own control at the right).
fn section(title: String, sub: Option<String>, right: Option<AnyElement>, body: impl IntoElement) -> Div {
    let head = section_head(title, None).children(right);
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(16.))
        .p(px(20.))
        .rounded(px(R_L + 2.))
        .bg(ca(WHITE, 0.03))
        .border_1()
        .border_color(c(LINE))
        .child(div().flex().flex_col().gap(px(2.)).child(head).children(sub.map(|t| div().text_size(px(T_M)).text_color(c(MUTED)).child(t))))
        .child(body)
}

/// One setting of a section: its name at the left, its choice beside it (stacked when the column is narrow); rows a
/// hairline apart.
fn setting(label: String, stacked: bool, first: bool, body: impl IntoElement) -> Div {
    div()
        .w_full()
        .flex()
        .when(stacked, |d| d.flex_col().gap(px(8.)))
        .when(!stacked, |d| d.items_start().gap(px(16.)))
        .min_h(px(40.))
        .when(!first, |d| d.pt(px(12.)).border_t_1().border_color(ca(WHITE, 0.05)))
        .child(div().when(!stacked, |d| d.w(px(140.)).min_h(px(36.)).flex().items_center()).flex_none().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).text_color(c(TEXT_2)).child(label))
        .child(div().flex_1().min_w_0().child(body))
}

pub fn form(root: &Root, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let Some(f) = root.auto.form.as_ref() else { return div().into_any_element() };
    let inner = width - 2. * SIDE;
    // Three columns (the game and its schedule, its windows and Discord, its accounts), two, or one.
    let cols = if inner >= 1240. { 3 } else if inner >= 860. { 2 } else { 1 };
    let acc_w = if cols == 3 { 400. } else { (inner * 0.38).clamp(340., 420.) };
    let col_w = match cols {
        3 => (inner - acc_w - 2. * GRID_GAP) / 2.,
        2 => inner - acc_w - GRID_GAP,
        _ => inner,
    };
    // (A setting's name beside its choice where the column has room for both.)
    let stacked = col_w < 480.;
    let place = input_text(&f.place, cx);
    // (A game looked up counts as set, its link or not in the box.)
    let place_set = !place.is_empty() || f.game.is_some();
    let server = if place_set || f.server == "follow" { f.server.clone() } else { "any".into() };
    let accounts = root.accounts();
    let accounts_by_id: std::collections::HashMap<_, _> = accounts.iter().map(|a| (s(a, "id"), a)).collect();
    let by_id = |id: &str| accounts_by_id.get(id).copied();

    // Name and game.
    let game_line = f.game.as_ref().map(|g| {
        let pic = Some(s(g, "iconUrl")).filter(|u| !u.is_empty()).and_then(|u| images::pic_sq(u, 32., cx));
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .px(px(10.))
            .py(px(8.))
            .rounded(px(R_M))
            .bg(ca(WHITE, 0.04))
            .child(div().size(px(32.)).flex_none().rounded(px(8.)).overflow_hidden().bg(ca(WHITE, 0.06)).when_some(pic, |d, p| d.child(img(p).size_full().object_fit(ObjectFit::Cover))))
            .child(div().flex_1().min_w_0().truncate().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).child(crate::pages::home::clean(s(g, "name"))))
            .child(icon("check", 14., c(OK)))
    });
    // A search's games: a list, one a row, its picture, name and players (the web form's game results).
    let found_list = |list: Vec<Value>, cx: &mut Context<Root>| {
        let mut rows = div().py(px(4.)).flex().flex_col();
        for (i, g) in list.into_iter().enumerate() {
            let pic = Some(s(&g, "iconUrl")).filter(|u| !u.is_empty()).and_then(|u| images::pic_sq(u, 38., cx));
            let name = crate::pages::home::clean(s(&g, "name"));
            let playing = g.get("playing").and_then(Value::as_i64).filter(|n| *n > 0);
            rows = rows.child(
                hot(("ps-found", i))
                    .h(px(50.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .rounded(px(R_S))
                    .tones(ca(WHITE, 0.), ca(WHITE, 0.06))
                    .cursor_pointer()
                    .child(div().flex_none().size(px(38.)).rounded(px(8.)).overflow_hidden().bg(ca(WHITE, 0.06)).when_some(pic, |d, p| d.child(img(p).size_full().object_fit(ObjectFit::Cover))))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(div().truncate().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).child(name))
                            .when_some(playing, |d, n| d.child(div().text_size(px(T_S)).text_color(c(MUTED)).font_features(tnum()).child(root.i18n.tv("dock.playing", &[("n", &n.to_string())])))),
                    )
                    .on_click(cx.listener(move |r, _, window, cx| choose(r, g.clone(), window, cx))),
            );
        }
        // A framed box about four games high that scrolls, its bar showing: the whole list pushed Server and Wait down
        // to the form's foot.
        scroll_list("ps-found-list", &crate::scrolled::handle("ps-found-list"), 216., rows)
    };
    let recent: Vec<Value> = root.state.pointer("/settings/recentGames").and_then(Value::as_array).map(|l| l.iter().take(6).cloned().collect()).unwrap_or_default();
    let pick_row = |list: Vec<Value>, key: &'static str, cx: &mut Context<Root>| {
        let mut row = div().flex().flex_wrap().gap(px(6.));
        for (i, g) in list.into_iter().enumerate() {
            let pic = Some(s(&g, "iconUrl")).filter(|u| !u.is_empty()).and_then(|u| images::pic_sq(u, 26., cx));
            let name = crate::pages::home::clean(s(&g, "name"));
            row = row.child(
                hot((key, i))
                    .h(px(34.))
                    .pl(px(4.))
                    .pr(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .rounded(px(R_M))
                    .tones(ca(WHITE, 0.04), ca(WHITE, 0.08))
                    .cursor_pointer()
                    .child(div().size(px(26.)).flex_none().rounded(px(6.)).overflow_hidden().bg(ca(WHITE, 0.06)).when_some(pic, |d, p| d.child(img(p).size_full().object_fit(ObjectFit::Cover))))
                    .child(div().max_w(px(200.)).truncate().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).child(name))
                    .on_click(cx.listener(move |r, _, window, cx| choose(r, g.clone(), window, cx))),
            );
        }
        row
    };
    let game_extra = match &f.found {
        Some((_, None)) => small_note(root.tr("games.searching")).into_any_element(),
        Some((_, Some(list))) if list.is_empty() => small_note(root.tr("picker.noResults")).into_any_element(),
        Some((_, Some(list))) => found_list(list.iter().take(8).cloned().collect(), cx).into_any_element(),
        None if place.is_empty() && f.game.is_none() && !recent.is_empty() => {
            div().flex().flex_col().gap(px(8.)).child(small_note(root.tr("picker.recent"))).child(pick_row(recent, "ps-recent", cx)).into_any_element()
        }
        None => div().into_any_element(),
    };

    // Server and wait.
    let mut servers: Vec<(String, String, bool)> = SERVERS_ALL.iter().map(|k| (k.to_string(), root.tr(&format!("ps.srv.{k}")), !place_set && *k != "any" && *k != "follow")).collect();
    let each: Vec<(String, String, bool)> = SERVERS_EACH.iter().map(|(k, short)| (k.to_string(), root.tr(&format!("ps.srvEach.{short}")), !place_set)).collect();
    servers.truncate(7);
    let srv_note = if !place_set && server != "follow" {
        Some(root.tr("ps.srvNeedGame"))
    } else if server.starts_with("each") {
        Some(root.tr("ps.srvEachNote"))
    } else {
        None
    };
    let default_delay = root.state.pointer("/settings/launchDelaySeconds").and_then(Value::as_i64).unwrap_or(8);
    let follow_set_n = |n: i64| root.i18n.tv("ps.opt.followSet", &[("state", &root.i18n.tv("set.seconds", &[("n", &n.to_string())]))]);
    let follow_set = |on: bool| root.i18n.tv("ps.opt.followSet", &[("state", &root.tr(if on { "ps.opt.stOn" } else { "ps.opt.stOff" }))]);
    let mut delays: Vec<i64> = DELAYS.to_vec();
    if let Some(d) = f.delay.filter(|d| !DELAYS.contains(d)) {
        delays.push(d);
        delays.sort();
    }
    let mut delay_items = vec![("".to_string(), follow_set_n(default_delay), false)];
    delay_items.extend(delays.into_iter().map(|n| (n.to_string(), if n == 0 { root.tr("ps.delayNone") } else { root.i18n.tv("set.seconds", &[("n", &n.to_string())]) }, false)));
    let delay_sel = f.delay.map(|d| d.to_string()).unwrap_or_default();

    // Accounts: ticked first in launch order (their place a number); a ticked one's grip drags it among them, its
    // arrows move it a place.
    let ticked_n = f.ticked.len();
    let query = input_text(&f.account_query, cx);
    let query_folded = folded(&query);
    let matching: Vec<_> = f.order.iter().enumerate().filter_map(|(i, id)| {
        let a = by_id(id)?;
        let matches = match f.account_scope {
            "group" => !s(a, "group").is_empty() && folded(s(a, "group")).contains(&query_folded),
            "label" => !label(a).is_empty() && folded(&label(a)).contains(&query_folded),
            _ => crate::pages::accounts::found(a, &query),
        };
        matches.then_some((i, id, a))
    }).collect();
    let scope = ["all", "group", "label"].into_iter().fold(Segmented::new("ps-acc-scope", f.account_scope), |choices, scope| {
        choices.item(scope, root.tr(&format!("ps.acc.{scope}")))
    }).on_select({
        let this = cx.weak_entity();
        move |key, _, cx| {
            let _ = this.update(cx, |r, cx| {
                if let Some(f) = r.auto.form.as_mut() {
                    f.account_scope = match key.as_ref() { "group" => "group", "label" => "label", _ => "all" };
                    f.account_limit = ACCOUNT_PAGE;
                    cx.notify();
                }
            });
        }
    });
    let select_ids: Vec<_> = matching.iter().filter(|(_, id, _)| !f.ticked.contains(*id)).map(|(_, id, _)| (*id).clone()).collect();
    let count_to_select = select_ids.len();
    let filter = div().flex().flex_col().gap(px(8.))
        .child(scope)
        .child(field(&f.account_query, Some("search")))
        .when(!matching.is_empty(), |d| d.child(Button::new("ps-match-accounts",
            root.i18n.tv("ps.acc.pick", &[("n", &count_to_select.to_string())])).small().disabled(count_to_select == 0)
            .on_click(edit(cx, move |f| {
                f.ticked.extend(select_ids.iter().cloned());
                f.order.sort_by_key(|id| !f.ticked.contains(id));
            }))));
    let mut acc_list = div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .on_mouse_up(MouseButton::Left, cx.listener(|r, _, _, cx| drop_held(r, cx)))
        .on_mouse_up_out(MouseButton::Left, cx.listener(|r, _, _, cx| drop_held(r, cx)));
    let ticked_order: Vec<String> = f.order.iter().filter(|id| f.ticked.contains(*id)).cloned().collect();
    for &(i, id, a) in matching.iter().take(f.account_limit) {
        let on = f.ticked.contains(id);
        let at = on.then_some(i);
        let held = f.held.as_deref() == Some(id.as_str());
        let (up_id, down_id) = (id.clone(), id.clone());
        let up = edit(cx, move |f| move_in(f, &up_id, -1));
        let down = edit(cx, move |f| move_in(f, &down_id, 1));
        let user = s(a, "username");
        let group = s(a, "group");
        let handle = if on && ticked_n > 1 {
            let hid = id.clone();
            let tip: SharedString = root.tr("lo.drag").into();
            div()
                .id(("ps-grip", i))
                .w(px(18.))
                .h(px(40.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .cursor_grab()
                .child(icon("grip", 15., if held { c(TEXT) } else { c(DIM) }))
                .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |r, _, _, cx| {
                        if let Some(f) = r.auto.form.as_mut() {
                            f.held = Some(hid.clone());
                        }
                        cx.stop_propagation();
                        cx.notify();
                    }),
                )
                .on_click(|_, _, cx| cx.stop_propagation())
                .into_any_element()
        } else {
            div().w(px(18.)).flex_none().into_any_element()
        };
        let over = id.clone();
        // The line between the ticked and the rest.
        if !on && at.is_none() && i == ticked_n && ticked_n > 0 {
            acc_list = acc_list.child(div().h(px(1.)).mx(px(8.)).my(px(6.)).bg(ca(WHITE, 0.06)));
        }
        acc_list = acc_list.child(
            div()
                .id(("ps-acc", i))
                .h(px(52.))
                .pl(px(2.))
                .pr(px(6.))
                .flex()
                .items_center()
                .gap(px(10.))
                .rounded(px(R_M))
                .bg(if on { ca(WHITE, 0.045) } else { ca(WHITE, 0.) })
                .hover(|d| d.bg(ca(WHITE, 0.07)))
                .when(held, |d| d.opacity(0.6))
                .child(handle)
                // The whole row ticks it (the box has no click of its own).
                .child(Check::new(("ps-tick", i), on))
                .child(avatar(a, 32., RAISED, cx))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(1.))
                        .child(div().truncate().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).text_color(if on { c(TEXT) } else { c(TEXT_2) }).child(label(a)))
                        .when(!user.is_empty() || !group.is_empty(), |d| d.child(div().truncate().text_size(px(T_S)).text_color(c(MUTED))
                            .child(match (group.is_empty(), user.is_empty()) {
                                (false, false) => format!("{group} · @{user}"),
                                (false, true) => group.to_string(),
                                _ => format!("@{user}"),
                            }))),
                )
                .when_some(at, |d, k| {
                    // (No place numbers: owner 2026-11-14, no numbers on accounts.)
                    d
                        // An arrow that can't move it (the first one's up, the last one's down) leaves its place empty:
                        // greyed, the thin dim chevron read as another icon weight (review 4.5).
                        .child(if k == 0 { div().size(px(28.)).flex_none().into_any_element() } else { IconButton::new(("ps-up", i), "chevronUp").size(28.).tip(root.tr("lo.up")).on_click(up).into_any_element() })
                        .child(if k + 1 >= ticked_n { div().size(px(28.)).flex_none().into_any_element() } else { IconButton::new(("ps-down", i), "chevronDown").size(28.).tip(root.tr("lo.down")).on_click(down).into_any_element() })
                })
                .cursor_pointer()
                .on_mouse_move(cx.listener(move |r, ev: &MouseMoveEvent, _, cx| {
                    let Some(f) = r.auto.form.as_mut() else { return };
                    let Some(h) = f.held.clone() else { return };
                    if ev.pressed_button != Some(MouseButton::Left) {
                        f.held = None;
                        return cx.notify();
                    }
                    // Among the ticked only: it takes the place of the ticked row it passes over.
                    let (Some(from), Some(to)) = (f.order.iter().position(|x| *x == h), f.order.iter().position(|x| *x == over)) else { return };
                    if from != to && to < f.ticked.len() {
                        let moved = f.order.remove(from);
                        f.order.insert(to, moved);
                        cx.notify();
                    }
                }))
                .on_click({
                    let tid = id.clone();
                    edit(cx, move |f| {
                        let Some(from) = f.order.iter().position(|x| *x == tid) else { return };
                        let id = f.order.remove(from);
                        // Unticked: first of the unticked; ticked: last of the ticked.
                        if f.ticked.remove(&id) {
                            let n = f.ticked.len();
                            f.order.insert(n, id);
                        } else {
                            let n = f.ticked.len();
                            f.ticked.insert(id.clone());
                            f.order.insert(n, id);
                        }
                    })
                }),
        );
    }

    // Options for its windows.
    let with_on = place_set && server != "follow" && f.rejoin != Some(false);
    let mut with_items = vec![("".to_string(), root.tr("ps.opt.anyServer"), false), ("any".to_string(), root.tr("ps.opt.withAny"), false)];
    for id in &ticked_order {
        if let Some(a) = by_id(id) {
            with_items.push((id.clone(), root.i18n.tv("ps.opt.withAcc", &[("name", &label(a))]), false));
        }
    }
    let with_sel = if f.rejoin_with.is_empty() || f.rejoin_with == "any" || f.ticked.contains(&f.rejoin_with) { f.rejoin_with.clone() } else { String::new() };
    let tile_on = root.state.pointer("/multi/enabled").and_then(Value::as_bool).unwrap_or(false) && root.state.pointer("/multi/tile").and_then(Value::as_bool).unwrap_or(false);
    let afk_on = root.state.pointer("/settings/antiAfk").and_then(Value::as_bool).unwrap_or(false);
    let mut opt_notes = Vec::new();
    if with_on && !with_sel.is_empty() {
        opt_notes.push(root.tr("ps.opt.withNote"));
    }
    if f.profile == "afk" {
        opt_notes.push(root.tr("ps.opt.lightNote"));
    }
    if f.restart > 0 {
        opt_notes.push(root.tr(if place_set || server == "follow" { "ps.opt.restartNote" } else { "ps.opt.restartNoGame" }));
    }
    let with_box = select(root, cx, "ps-with", vec![(None, with_items)], &with_sel, |f, v| f.rejoin_with = v.to_string());
    let restart_box = select(
        root,
        cx,
        "ps-restart",
        vec![(None, RESTART_HOURS.iter().map(|h| (h.to_string(), if *h == 0 { root.tr("ps.opt.never") } else { root.i18n.tv("ps.opt.every", &[("n", &h.to_string())]) }, false)).collect())],
        &f.restart.to_string(),
        |f, v| f.restart = v.parse().unwrap_or(0),
    );
    let opts = div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(setting(root.tr("ps.opt.afk"), stacked, true, tri(root, cx, "ps-afk", f.afk, follow_set(afk_on), |f, v| f.afk = v)))
        .child(setting(root.tr("ps.opt.rejoin"), stacked, false, tri(root, cx, "ps-rejoin", f.rejoin, root.tr("ps.opt.followAcc"), |f, v| f.rejoin = v)))
        .when(with_on, |d| d.child(setting(root.tr("ps.opt.rejoinWith"), stacked, false, with_box)))
        .child(setting(
            root.tr("ps.opt.profile"),
            stacked,
            false,
            chips(cx, "ps-profile", vec![("".into(), root.tr("ps.opt.normal"), false), ("afk".into(), root.tr("ps.opt.light"), false)], &f.profile, |f, v| f.profile = v.to_string()),
        ))
        .child(setting(root.tr("ps.opt.restart"), stacked, false, restart_box))
        .child(setting(root.tr("ps.opt.arrange"), stacked, false, tri(root, cx, "ps-arrange", f.arrange, follow_set(tile_on), |f, v| f.arrange = v)))
        .when(!opt_notes.is_empty(), |d| d.child(div().pt(px(4.)).flex().flex_col().gap(px(6.)).children(opt_notes.into_iter().map(small_note))));

    // Discord.
    let dc_on = root.state.pointer("/discord/enabled").and_then(Value::as_bool).unwrap_or(false);
    let mut dc_note = Vec::new();
    if f.saved_discord.is_some() {
        dc_note.push(root.tr("ps.dc.saved"));
    }
    dc_note.push(root.tr("ps.dc.note"));
    let dc = div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(setting(
            root.tr("ps.dc.where"),
            true,
            true,
            chips(cx, "ps-dc", vec![("".into(), root.tr("ps.dc.main"), false), ("own".into(), root.tr("ps.dc.own"), false)], if f.dc_own { "own" } else { "" }, |f, v| f.dc_own = v == "own"),
        ))
        .when(f.dc_own, |d| {
            let pid = f.id.clone();
            d.child(div().flex().items_center().gap(px(8.)).child(div().flex_1().min_w_0().child(boxed(&f.dc_link))).child(Button::new("ps-dc-test", root.tr("dc.test")).on_click(
                cx.listener(move |r, _, _, cx| {
                    let url = r.auto.form.as_ref().map(|f| input_text(&f.dc_link, cx)).unwrap_or_default();
                    let Some(link) = r.link.clone() else { return };
                    let rx = link.call("presetDiscordTest", json!({ "id": pid, "url": url }));
                    answer(rx, cx, move |r, answer, cx| {
                        r.toast = Some(match answer {
                            Ok(Ok(_)) => (r.tr("dc.testOk"), "ok".into()),
                            Ok(Err(e)) => (e, "bad".into()),
                            Err(_) => return,
                        });
                        cx.notify();
                    });
                }),
            )))
        })
        .child(small_note(dc_note.join("\n")))
        .when(!dc_on, |d| d.child(div().flex().items_start().gap(px(8.)).text_size(px(T_S)).line_height(px(17.)).text_color(c(WARN)).child(icon("info", 14., c(WARN)).mt(px(1.))).child(root.tr("ps.dc.off"))));

    // Schedule.
    let sch_body = div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(setting(root.tr("ps.schTime"), stacked, true, div().w(px(140.)).child(boxed(&f.time))))
        .child(setting(
            root.tr("ps.schRepeat"),
            stacked,
            false,
            chips(cx, "ps-mode", vec![("days".into(), root.tr("ps.schDays"), false), ("once".into(), root.tr("ps.schOnce"), false)], if f.once { "once" } else { "days" }, |f, v| f.once = v == "once"),
        ))
        .child(if f.once {
            setting(root.tr("ps.schDate"), stacked, false, date_box(root, f, cx)).into_any_element()
        } else {
            // Days are many at once: a block toggles its own day.
            let mut row = div().flex().flex_wrap().gap(px(6.));
            for (i, d) in WEEK.iter().copied().enumerate() {
                let on = f.days.contains(&d);
                row = row.child(
                    segment(("ps-day", i), on)
                        .h(px(34.))
                        .min_w(px(42.))
                        .px(px(8.))
                        .justify_center()
                        .rounded(px(R_S))
                        .when(!on, |s| s.bg(ca(WHITE, 0.04)))
                        .child(day_short(d))
                        .on_click(edit(cx, move |f| {
                            if let Some(k) = f.days.iter().position(|x| *x == d) {
                                f.days.remove(k);
                            } else {
                                f.days.push(d);
                            }
                        })),
                );
            }
            setting(root.tr("ps.schDays"), stacked, false, row).into_any_element()
        })
        .child(setting(
            root.tr("ps.schStop"),
            stacked,
            false,
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(chips(cx, "ps-stop", ["never", "after", "at"].iter().map(|k| (k.to_string(), root.tr(&format!("ps.schStop.{k}")), false)).collect(), &f.stop, |f, v| f.stop = v.to_string()))
                .when(f.stop == "after", |d| d.child(div().flex().items_center().gap(px(10.)).child(div().w(px(100.)).child(boxed(&f.hours))).child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("ps.schHours")))))
                .when(f.stop == "at", |d| d.child(div().flex().items_center().gap(px(10.)).child(div().w(px(120.)).child(boxed(&f.stop_time))).child(div().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("ps.schStopTime"))))),
        ))
        .child(small_note(root.tr("ps.schNote")));
    let sch_switch = Switch::new("ps-sch-on", f.sch_on)
        .on_toggle({
            let this = cx.entity().downgrade();
            move |on, _, cx| {
                let on = *on;
                let _ = this.update(cx, |r, cx| {
                    if let Some(f) = r.auto.form.as_mut() {
                        f.sch_on = on;
                        f.error = None;
                    }
                    cx.notify();
                });
            }
        })
        .into_any_element();
    let sched = section(root.tr("ps.sch"), Some(root.tr("ps.schDesc")), Some(sch_switch), div().when(f.sch_on, |d| d.child(sch_body)));

    let gone = (f.gone > 0).then(|| {
        div()
            .flex()
            .items_start()
            .gap(px(8.))
            .px(px(12.))
            .py(px(8.))
            .rounded(px(R_M))
            .bg(tinted(PANEL, WARN, 0.07))
            .text_size(px(T_S))
            .line_height(px(17.))
            .text_color(c(WARN))
            .child(icon("info", 14., c(WARN)).mt(px(1.)))
            .child(div().flex_1().min_w_0().child(root.i18n.tn("ps.gone", f.gone as i64, &[])))
    });

    // Server and Wait side by side, each a choice box (the web form's two base-selects).
    let server_box = select(root, cx, "ps-srv", vec![(Some(root.tr("ps.grp.all")), servers), (Some(root.tr("ps.grp.each")), each)], &server, |f, v| f.server = v.to_string());
    let delay_box = select(root, cx, "ps-delay", vec![(None, delay_items)], &delay_sel, |f, v| f.delay = v.parse().ok());
    let choices = div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(
            div()
                .flex()
                .gap(px(14.))
                .child(div().flex_1().min_w_0().child(labelled(root.tr("ps.server"), server_box)))
                .child(div().flex_1().min_w_0().child(labelled(root.tr("ps.delay"), delay_box))),
        )
        .children(srv_note.map(|n| div().text_size(px(T_S)).text_color(c(if place_set { MUTED } else { WARN })).child(n)));

    let picked = section(
        root.tr("ps.accounts"),
        Some(root.tr("ps.order")),
        Some(div().text_size(px(T_SEC)).text_color(c(MUTED)).font_features(tnum()).child(format!("{ticked_n}/{}", accounts.len())).into_any_element()),
        div().flex().flex_col().gap(px(10.)).children(gone).child(filter)
            .when(matching.is_empty(), |d| d.child(small_note(root.tr("tabs.noMatches"))))
            .child(acc_list)
            .when(matching.len() > f.account_limit, |d| d.child(Button::new("ps-more-accounts", root.tr("ps.acc.more")).quiet()
                .on_click(cx.listener(|r, _, _, cx| {
                    if let Some(f) = r.auto.form.as_mut() {
                        f.account_limit += ACCOUNT_PAGE;
                        cx.notify();
                    }
                })))),
    );

    let general = section(
        root.tr("au.sec.game"),
        None,
        None,
        div()
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(labelled(root.tr("ps.name"), boxed(&f.name)))
            .child(labelled(root.tr("ps.game"), div().flex().flex_col().gap(px(10.)).child(boxed(&f.place)).children(game_line).child(game_extra)))
            .child(choices)
            .when(server == "job", |d| d.child(labelled(root.tr("ps.job"), boxed(&f.job))))
            .when(server == "follow", |d| d.child(labelled(root.tr("ps.follow"), div().flex().flex_col().gap(px(6.)).child(boxed(&f.follow)).child(small_note(root.tr("ps.followNote")))))),
    );
    let windows = section(root.tr("ps.opts"), Some(root.tr("ps.optsDesc")), None, opts);
    let discord = section("Discord".into(), None, None, dc);

    let column = |w: Option<f32>| div().flex().flex_col().gap(px(GRID_GAP)).map(|d| match w {
        Some(w) => d.w(px(w)).flex_none(),
        None => d.flex_1().min_w_0(),
    });
    let body = div().px(px(SIDE)).pt(px(4.)).pb(px(32.)).flex().items_start().gap(px(GRID_GAP)).map(|d| match cols {
        3 => d.child(column(None).child(general).child(sched)).child(column(None).child(windows).child(discord)).child(column(Some(acc_w)).child(picked)),
        2 => d.child(column(None).child(general).child(windows).child(sched).child(discord)).child(column(Some(acc_w)).child(picked)),
        _ => d.child(column(None).child(general).child(picked).child(windows).child(sched).child(discord)),
    });

    // The head: back to the list, the preset's name as it is typed, and under it what it does at a glance.
    let typed = input_text(&f.name, cx);
    let title = if !typed.is_empty() {
        typed
    } else {
        root.tr(if f.is_new { "ps.newTitle" } else { "ps.editTitle" })
    };
    let mut summary: Vec<(String, Option<Hsla>)> = Vec::new();
    summary.push((
        match (&f.game, server.as_str()) {
            (_, "follow") => root.i18n.tv("ps.following", &[("name", &input_text(&f.follow, cx))]),
            (Some(g), _) => crate::pages::home::clean(s(g, "name")),
            (None, _) if !place.is_empty() => place.clone(),
            _ => root.tr("ps.noGame"),
        },
        Some(c(TEXT_2)),
    ));
    summary.push((root.i18n.tn("au.accN", ticked_n as i64, &[]), if ticked_n == 0 { Some(c(WARN)) } else { None }));
    if place_set && server != "follow" {
        summary.push((root.tr(&format!("ps.srv.{server}")), None));
    }
    if f.sch_on {
        let days: Vec<i64> = WEEK.iter().copied().filter(|d| f.days.contains(d)).collect();
        let time = parse_time(&input_text(&f.time, cx)).ok().flatten().unwrap_or_default();
        let date = if f.once { parse_date(&input_text(&f.date, cx)).ok().flatten().unwrap_or_default() } else { String::new() };
        summary.push((crate::pages::auto::schedule_text(root, &json!({ "time": time, "days": days, "date": date })), Some(c(TEXT_2))));
    } else {
        summary.push((root.tr("au.noSchedule"), None));
    }
    let header = div()
        .flex_none()
        .px(px(SIDE))
        .pt(px(14.))
        .pb(px(18.))
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(
            div().flex().items_center().gap(px(4.)).child(
                tool("ps-back", Some("chevronLeft"), false).ml(px(-10.)).child(root.tr("nav.auto")).on_click(cx.listener(|r, _, window, cx| leave(r, window, cx))),
            )
            .child(div().text_size(px(T_M)).text_color(c(DIM)).child("/"))
            .child(div().px(px(6.)).text_size(px(T_M)).text_color(c(MUTED)).child(root.tr(if f.is_new { "ps.newTitle" } else { "ps.editTitle" }))),
        )
        .child(div().text_size(px(T_TITLE)).line_height(px(38.)).font_weight(FontWeight::BOLD).truncate().child(title))
        .child(facts(summary));

    let del = (!f.is_new).then(|| {
        let pid = f.id.clone();
        // The box asks first (app.js's confirmBox; "Don't ask again" is the app's to keep).
        Button::new("ps-form-del", root.tr("ps.delete")).icon("trash").danger().on_click(cx.listener(move |r, _, window, cx| {
            let name = r.auto.form.as_ref().map(|f| {
                let typed = f.name.read(cx).value().trim().to_string();
                if typed.is_empty() { f.auto_name.clone() } else { typed }
            });
            let pid = pid.clone();
            r.confirm(r.tr("ps.deleteTitle"), r.i18n.tv("ps.deleteQ", &[("name", &name.unwrap_or_default())]), r.tr("ps.delete"), true, Some("deletePreset"), window, cx, move |r, cx| {
                let Some(link) = r.link.clone() else { return };
                let rx = link.call("deletePreset", json!({ "id": pid }));
                answer(rx, cx, move |r, answer, cx| {
                    match answer {
                        Ok(Ok(_)) => {
                            r.auto.form = None;
                            r.toast = Some((r.tr("ps.deleted"), "ok".into()));
                        }
                        Ok(Err(e)) => r.toast = Some((e, "bad".into())),
                        Err(_) => {}
                    }
                    cx.notify();
                });
            });
        }))
    });
    // The foot stays put under the form: Delete at the left, what stops a save, Cancel, and the one yellow block.
    let foot = div()
        .flex_none()
        .px(px(SIDE))
        .py(px(14.))
        .border_t_1()
        .border_color(c(LINE))
        .bg(ca(SHADE, 0.12))
        .flex()
        .items_center()
        .gap(px(12.))
        .children(del)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .when_some(f.error.clone(), |d, e| d.flex().items_center().gap(px(8.)).text_size(px(T_M)).text_color(c(BAD)).child(icon("alert", 15., c(BAD))).child(div().min_w_0().child(e))),
        )
        .child(Button::new("ps-cancel", root.tr("common.cancel")).quiet().on_click(cx.listener(|r, _, window, cx| leave(r, window, cx))))
        .child(Button::new("ps-save", root.tr("ps.saveBtn")).icon("check").primary().disabled(f.saving).on_click(cx.listener(|r, _, _, cx| save(r, cx))));

    div()
        .id("ps-form")
        .size_full()
        .rounded(px(R_PANEL))
        .bg(c(PANEL))
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(header)
        // (The bar at its right edge, always there, says more is below: no fade.)
        .child(
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .child(div().id("ps-form-body").size_full().overflow_y_scroll().track_scroll(&f.scroll).child(body))
                .child(crate::pages::auto::scroll_bar("ps-form-bar", &f.scroll)),
        )
        .child(foot)
        .into_any_element()
}

impl Form {
    /// The schedule on (a shot of the whole form).
    #[cfg(feature = "shot")]
    pub fn open_folds(&mut self) {
        self.sch_on = true;
    }
}

/// A ticked account one place up or down among the ticked.
fn move_in(f: &mut Form, id: &str, dir: i32) {
    let Some(i) = f.order.iter().position(|x| x == id) else { return };
    let j = i as i32 + dir;
    if j >= 0 && (j as usize) < f.ticked.len() {
        f.order.swap(i, j as usize);
    }
}

/// The grip let go: the row stays where it was dragged to.
fn drop_held(r: &mut Root, cx: &mut Context<Root>) {
    if let Some(f) = r.auto.form.as_mut().filter(|f| f.held.is_some()) {
        f.held = None;
        cx.notify();
    }
}
