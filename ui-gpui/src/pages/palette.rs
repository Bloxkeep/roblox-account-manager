// Quick search (Ctrl+K), the web page's ui/src/Palette.tsx and app.js's paletteEntries: one box over the app's actions,
// its places, the presets, the macros' panels, the pinned and recent games, the accounts and Settings' parts, ranked by
// a small fuzzy match and grouped by kind (the group with the best match first). ↑ ↓ (and Page Up/Down) move, Enter
// runs the highlighted entry, Esc closes; an account's other actions (Play or Close, Edit) are on Ctrl+Enter and
// Shift+Enter, and on its highlighted row. The entries are built as it opens, as the web page's were.
use std::rc::Rc;

use gpui_kit::component::WindowExt as _;
use gpui_kit::component::dialog::DialogEntrance;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};
use crate::val::s;

use crate::{
    kit::*,
    pages::{self, Page, settings::Part},
    shell::Root,
    theme::*,
};

actions!(palette, [PalUp, PalDown, PalPageUp, PalPageDown, PalEnter, PalAltEnter, PalShiftEnter, PalTab]);

const CONTEXT: &str = "Palette";
/// Only the best ones are listed: the rest are counted, so a long list doesn't read as all there is.
const MAX: usize = 60;
const ALT_KEYS: [&str; 2] = ["Ctrl+Enter", "Shift+Enter"];

/// The palette's keys, inside its box (the box's own Enter passes on; a one-line box takes no arrows).
pub fn bind(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", PalUp, Some(CONTEXT)),
        KeyBinding::new("down", PalDown, Some(CONTEXT)),
        KeyBinding::new("pageup", PalPageUp, Some(CONTEXT)),
        KeyBinding::new("pagedown", PalPageDown, Some(CONTEXT)),
        KeyBinding::new("enter", PalEnter, Some(CONTEXT)),
        KeyBinding::new("ctrl-enter", PalAltEnter, Some(CONTEXT)),
        KeyBinding::new("shift-enter", PalShiftEnter, Some(CONTEXT)),
        // (The box keeps the keyboard: the arrows walk the list.)
        KeyBinding::new("tab", PalTab, Some(CONTEXT)),
        KeyBinding::new("shift-tab", PalTab, Some(CONTEXT)),
    ]);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Action,
    Tab,
    Preset,
    Panel,
    Game,
    Account,
    Setting,
}

impl Kind {
    const ALL: [Kind; 7] = [Kind::Action, Kind::Tab, Kind::Preset, Kind::Panel, Kind::Game, Kind::Account, Kind::Setting];
    fn heading(self) -> &'static str {
        match self {
            Kind::Action => "pal.cat.action",
            Kind::Tab => "pal.cat.tab",
            Kind::Preset => "pal.cat.preset",
            Kind::Panel => "pal.cat.panel",
            Kind::Game => "pal.cat.game",
            Kind::Account => "pal.cat.account",
            Kind::Setting => "pal.cat.setting",
        }
    }

    /// The small mark before a group's heading.
    fn icon(self) -> &'static str {
        match self {
            Kind::Action => "keyboard",
            Kind::Tab => "grid",
            Kind::Preset => "layers",
            Kind::Panel => "repeat",
            Kind::Game => "gamepad",
            Kind::Account => "users",
            Kind::Setting => "sliders",
        }
    }
}

/// A row's picture: an icon, a game's icon (the gamepad while it doesn't load), or an account's face.
enum Lead {
    Icon(&'static str),
    Pic(String),
    Face(Value),
}

/// What an entry does.
#[derive(Clone)]
enum Run {
    Join,
    Add,
    CheckAll,
    Refresh,
    Arrange,
    CloseAll,
    Send(&'static str),
    SelectAll,
    Order,
    SavePreset,
    NewMacro,
    StopMacros,
    Backup(bool),
    Updates,
    Feedback,
    Export,
    Lang(&'static str),
    Lock,
    Go(Page),
    Auto(pages::auto::Tab),
    Activity,
    Panel(String, bool),
    Preset(String),
    Game(i64),
    Reveal(String),
    ShowWindow(String),
    Close(String),
    Play(String),
    Edit(String),
    Settings(Part),
}

struct Entry {
    kind: Kind,
    text: String,
    hint: String,
    lead: Lead,
    run: Run,
    alts: Vec<(String, Run)>,
    /// The text, the hint and the other words it is found by, as a search compares them (kit::folded).
    text_f: String,
    hint_f: String,
    keys_f: Vec<String>,
}

fn entry(kind: Kind, lead: Lead, text: String, hint: String, keys: &[&str], run: Run) -> Entry {
    Entry { kind, text_f: folded(&text), hint_f: folded(&hint), keys_f: keys.iter().filter(|k| !k.is_empty()).map(|k| folded(k)).collect(), text, hint, lead, run, alts: Vec::new() }
}

#[derive(Default)]
pub struct Data {
    pub open: bool,
    seq: u32,
    _input: Option<Entity<InputState>>,
    _typed: Option<Subscription>,
    entries: Rc<Vec<Entry>>,
    /// The entries listed now (best first, by kind when several kinds match), how many more matched, whether several
    /// kinds are listed (each under its heading), the highlighted row, and where the mouse was last.
    rows: Rc<Vec<usize>>,
    more: usize,
    mixed: bool,
    index: usize,
    pointer: Option<Point<Pixels>>,
    scroll: ScrollHandle,
}

// ───────────────────────── matching ─────────────────────────

const WORD: [char; 11] = [' ', '@', '_', '-', '.', '·', '(', '/', ':', '[', ']'];

fn word_start(s: &[char], j: usize) -> bool {
    j == 0 || WORD.contains(&s[j - 1]) || s[j - 1].is_whitespace()
}

fn find(s: &[char], q: &[char]) -> Option<usize> {
    if q.is_empty() || q.len() > s.len() {
        return None;
    }
    (0..=s.len() - q.len()).find(|&i| s[i..i + q.len()] == *q)
}

/// Match quality, both folded: the text's start beats a word's start, then a word's middle, then every word somewhere,
/// then the letters in order (fewer gaps first). -1 for no match.
fn score(q: &str, qc: &[char], s: &str) -> i32 {
    if s.is_empty() {
        return -1;
    }
    let sc: Vec<char> = s.chars().collect();
    if let Some(at) = find(&sc, qc) {
        if at == 0 {
            return 1000 - sc.len().min(200) as i32;
        }
        return if word_start(&sc, at) { 800 } else { 600 } - at.min(100) as i32;
    }
    let words: Vec<&str> = q.split_whitespace().collect();
    if words.len() > 1 && words.iter().all(|w| s.contains(w)) {
        return 500;
    }
    // Letters in order, the first one starting a word: "clall" finds "Close all", "anti" doesn't find "sang tieng".
    let letters: Vec<char> = qc.iter().copied().filter(|c| !c.is_whitespace()).collect();
    let Some(&l0) = letters.first() else { return -1 };
    let next = |from: usize, ch: char| sc.get(from..).and_then(|rest| rest.iter().position(|&c| c == ch)).map(|p| p + from);
    let mut first = next(0, l0);
    while let Some(f) = first {
        if word_start(&sc, f) {
            break;
        }
        first = next(f + 1, l0);
    }
    let Some(first) = first else { return -1 };
    let (mut score, mut from, mut last) = (306i32, first + 1, first);
    for &ch in &letters[1..] {
        let Some(j) = next(from, ch) else { return -1 };
        if j != last + 1 {
            score -= 5 + (j - from).min(15) as i32;
        }
        if word_start(&sc, j) {
            score += 6;
        }
        last = j;
        from = j + 1;
    }
    score.max(1)
}

/// The entries for a query, best first (ties keep the registry's order); the text counts most, the other words only
/// when they match well. With several kinds, grouped (the group with the best match first, the best first within each).
fn requery(p: &mut Data, query: &str) {
    let q = folded(query.trim());
    let (ranked, more): (Vec<usize>, usize) = if q.is_empty() {
        ((0..p.entries.len().min(MAX)).collect(), p.entries.len().saturating_sub(MAX))
    } else {
        let qc: Vec<char> = q.chars().collect();
        let mut hits: Vec<(i32, usize)> = p
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let by_text = score(&q, &qc, &e.text_f);
                let by_other = std::iter::once(&e.hint_f)
                    .chain(e.keys_f.iter())
                    .map(|k| {
                        let s = score(&q, &qc, k);
                        if s >= 500 { s - 300 } else { -1 }
                    })
                    .max()
                    .unwrap_or(-1);
                (by_text.max(by_other), i)
            })
            .filter(|(s, _)| *s > 0)
            .collect();
        hits.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        let more = hits.len().saturating_sub(MAX);
        (hits.into_iter().take(MAX).map(|(_, i)| i).collect(), more)
    };
    let mut kinds: Vec<Kind> = Vec::new();
    for &i in &ranked {
        if !kinds.contains(&p.entries[i].kind) {
            kinds.push(p.entries[i].kind);
        }
    }
    p.mixed = kinds.len() > 1;
    let rows: Vec<usize> = if p.mixed { kinds.iter().flat_map(|k| ranked.iter().copied().filter(|&i| p.entries[i].kind == *k)).collect() } else { ranked };
    p.rows = Rc::new(rows);
    p.more = more;
    p.index = 0;
    p.scroll.set_offset(point(px(0.), px(0.)));
}

/// The list's child that holds a row (the headings are children too).
fn child_of(p: &Data, index: usize) -> usize {
    if !p.mixed {
        return index;
    }
    let mut heads = 0;
    let mut was = None;
    for &i in p.rows.iter().take(index + 1) {
        let k = p.entries[i].kind;
        if was != Some(k) {
            heads += 1;
            was = Some(k);
        }
    }
    index + heads
}

// ───────────────────────── the entries ─────────────────────────

fn build(r: &Root, set_words: &[(Part, Vec<String>)], cx: &App) -> Vec<Entry> {
    use crate::pages::accounts::{is_local, label};
    let accounts = r.accounts();
    let st = r.state.get("settings").cloned().unwrap_or(Value::Null);
    let b = |p: &str| r.state.pointer(p).and_then(Value::as_bool).unwrap_or(false);
    let robloxes = r.metrics(cx).get("robloxCount").and_then(Value::as_i64).unwrap_or(0);
    let live_macros = r.state.get("macroRuns").and_then(Value::as_array).is_some_and(|l| l.iter().any(|m| matches!(s(m, "status"), "waiting" | "running" | "paused")))
        || !r.state.get("macroRecording").is_none_or(Value::is_null);
    let a = |icon: &'static str, key: &str, keys: &[&str], run: Run| entry(Kind::Action, Lead::Icon(icon), r.tr(key), String::new(), keys, run);
    // The app's own actions, as its menus and buttons have them; "off" ones (nothing to act on) aren't offered.
    let mut out = vec![a("userGo", "jp.button", &["follow", "player", "username", "server"], Run::Join), a("userPlus", "acc.add", &["sign in", "login", "cookie"], Run::Add)];
    if !accounts.is_empty() && !b("/checking") {
        out.push(a("shieldCheck", "m.checkAll", &[], Run::CheckAll));
    }
    out.push(a("refresh", "acc.refreshTip", &["presence", "status"], Run::Refresh));
    out.push(a("screens", "m.arrange", &["tile", "windows"], Run::Arrange));
    if accounts.iter().any(is_local) || robloxes > 0 {
        out.push(a("power", "m.closeAll", &["quit", "exit"], Run::CloseAll));
    }
    out.push(a("monitor", if b("/miniOpen") { "sys.miniHide" } else { "sys.mini" }, &[], Run::Send("toggleMini")));
    if !accounts.is_empty() {
        out.push(a("check", "m.selectAll", &[], Run::SelectAll));
    }
    if accounts.len() >= 2 {
        out.push(a("sort", "m.launchOrder", &[], Run::Order));
    }
    out.push(a("layers", "ps.save", &[], Run::SavePreset));
    out.push(a("keyboard", "macro.newPal", &["macro", "record", "auto", "click", "ghi"], Run::NewMacro));
    if live_macros {
        out.push(a("stop", "macro.stopAllPal", &["macro", "Ctrl+Alt+0"], Run::StopMacros));
    }
    let has_folder = !s(r.state.get("backup").unwrap_or(&Value::Null), "folder").is_empty();
    out.push(a("cloud", "bk.now", &["backup"], Run::Backup(has_folder)));
    out.push(a("download", "pal.checkUpdates", &[], Run::Updates));
    out.push(a("message", "fb.title", &["bug", "report", "idea", "feedback", "góp ý", "báo lỗi"], Run::Feedback));
    out.push(a("book", "help.title", &["help", "guide", "manual", "how", "hướng dẫn", "trợ giúp"], Run::Send("openGuide")));
    out.push(a("star", "star.title", &["github", "star", "sao", "open source", "mã nguồn"], Run::Send("openGitHub")));
    out.push(a("discord", "dc.title", &["discord", "community", "server", "chat", "cộng đồng", "hỏi đáp"], Run::Send("openDiscord")));
    out.push(a("history", "pal.exportActivity", &["csv", "log"], Run::Export));
    // Each other language by its own name ("Español"), found by the word "language" in English and its own.
    for l in crate::lang::ALL.iter().filter(|l| l.code != r.i18n.lang) {
        out.push(entry(Kind::Action, Lead::Icon("languages"), l.name.to_string(), r.tr("set.language"), &["language", "ngôn ngữ", l.code, l.word], Run::Lang(l.code)));
    }
    out.push(a("lock", "side.lock", &["password"], Run::Lock));

    // The places, as the rail has them (Automation's two parts each its own), and Statistics' Activity.
    let t = |icon: &'static str, key: &str, keys: &[&str], run: Run| entry(Kind::Tab, Lead::Icon(icon), r.tr(key), String::new(), keys, run);
    let games_word = r.tr("nav.news");
    out.extend([
        t("home", "nav.home", &[], Run::Go(Page::Home)),
        t("users", "nav.accounts", &[], Run::Go(Page::Accounts)),
        t("layers", "nav.presets", &[], Run::Auto(pages::auto::Tab::Presets)),
        t("roblox", Page::Games.label_key(), &[games_word.as_str()], Run::Go(Page::Games)),
        t("keyboard", "nav.macros", &[], Run::Auto(pages::auto::Tab::Macros)),
        t("clock", "nav.stats", &[], Run::Go(Page::Stats)),
        t("sliders", "nav.settings", &[], Run::Go(Page::Settings)),
        t("history", "alog.title", &["log", "history", "events"], Run::Activity),
    ]);

    // The presets that have accounts to open.
    let presets: Vec<Value> = st.get("presets").and_then(Value::as_array).cloned().unwrap_or_default();
    for p in &presets {
        let n = pages::preset_menu::accounts_of(r, p);
        if n == 0 {
            continue;
        }
        let server = s(p, "server");
        let place = s(p, "placeInput");
        let what = if server == "follow" {
            r.i18n.tv("ps.following", &[("name", s(p, "followPlayer"))])
        } else {
            Some(s(p, "gameName")).filter(|g| !g.is_empty()).or(Some(place).filter(|g| !g.is_empty())).map_or_else(|| r.tr("ps.noGame"), str::to_string)
        };
        let mut hint = vec![what, pages::preset_menu::acc_count(r, n)];
        if !matches!(server, "any" | "follow" | "") && !place.is_empty() {
            hint.push(r.tr(&format!("ps.srv.{server}")));
        }
        if let Some(t) = p.pointer("/schedule/enabled").and_then(Value::as_bool).filter(|on| *on).and_then(|_| pages::auto::next_text(r, p)) {
            hint.push(t);
        }
        out.push(entry(Kind::Preset, Lead::Icon("layers"), s(p, "name").to_string(), hint.join(" · "), &[s(p, "gameName")], Run::Preset(s(p, "id").to_string())));
    }

    // Each macro's panel: Show it (or Hide it, when it is up).
    let shown: Vec<&str> = r.state.get("panelsShown").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    for m in st.get("macros").and_then(Value::as_array).into_iter().flatten() {
        let (id, name) = (s(m, "id"), s(m, "name"));
        let up = shown.contains(&id);
        let text = r.i18n.tv(if up { "macro.panelHidePal" } else { "macro.panelShowPal" }, &[("name", name)]);
        out.push(entry(Kind::Panel, Lead::Icon("screens"), text, String::new(), &[name, "macro", "panel", "bảng"], Run::Panel(id.to_string(), up)));
    }

    // The pinned games, then the recent ones (each once).
    let mut seen = std::collections::HashSet::new();
    for (key, hint) in [("pinnedGames", "pal.pinned"), ("recentGames", "pal.recent")] {
        for g in st.get(key).and_then(Value::as_array).into_iter().flatten() {
            let place = g.get("placeId").and_then(Value::as_i64).unwrap_or(0);
            if place <= 0 || !seen.insert(place) {
                continue;
            }
            let pic = s(g, "iconUrl").to_string();
            let lead = if pic.is_empty() { Lead::Icon("gamepad") } else { Lead::Pic(pic) };
            out.push(entry(Kind::Game, lead, crate::pages::home::clean(s(g, "name")), r.tr(hint), &[&place.to_string()], Run::Game(place)));
        }
    }

    // The accounts: found by their real names too while names are hidden, as the search box does. Play for one not
    // open here; one open here is closed, or its hidden window shown.
    for acc in accounts {
        let id = s(acc, "id").to_string();
        let name = label(acc);
        let user = s(acc, "username");
        let hint: Vec<String> = [if !user.is_empty() && user != name { format!("@{user}") } else { String::new() }, s(acc, "group").to_string()].into_iter().filter(|x| !x.is_empty()).collect();
        let real = |k: &str| acc.pointer(&format!("/real/{k}")).and_then(Value::as_str).unwrap_or_default();
        let uid = acc.get("userId").and_then(Value::as_i64).filter(|u| *u > 0).map(|u| u.to_string()).unwrap_or_default();
        let keys = [real("label"), real("username"), real("displayName"), user, s(acc, "displayName"), s(acc, "group"), uid.as_str()];
        let mut e = entry(Kind::Account, Lead::Face(acc.clone()), name, hint.join(" · "), &keys, Run::Reveal(id.clone()));
        let first = if acc.pointer("/session/hidden").and_then(Value::as_bool) == Some(true) {
            (r.tr("m.showWindow"), Run::ShowWindow(id.clone()))
        } else if is_local(acc) || s(acc, "state") == "launching" {
            (r.tr("m.close"), Run::Close(id.clone()))
        } else {
            (r.tr("m.play"), Run::Play(id.clone()))
        };
        e.alts = vec![first, (r.tr("pal.edit"), Run::Edit(id))];
        out.push(e);
    }

    // Settings' parts.
    for part in Part::ALL {
        let keys: Vec<&str> = set_words.iter().find(|(p, _)| *p == part).map(|(_, w)| w.iter().map(String::as_str).collect()).unwrap_or_default();
        out.push(entry(Kind::Setting, Lead::Icon(part.icon()), r.tr(part.title()), String::new(), &keys, Run::Settings(part)));
    }
    out
}

// ───────────────────────── running ─────────────────────────

/// An account brought into view on Accounts: a filter, a group or a search that hides it is put aside, and the search
/// then names it (the cards don't scroll to one by themselves here).
fn reveal(r: &mut Root, id: &str, window: &mut Window, cx: &mut Context<Root>) {
    crate::pages::accounts::reveal(r, id, window, cx);
}

fn perform(r: &mut Root, run: Run, window: &mut Window, cx: &mut Context<Root>) {
    match run {
        Run::Join => {
            let ids: Vec<String> = r.accounts().iter().map(|a| s(a, "id").to_string()).filter(|id| r.selected.contains(id)).collect();
            if ids.is_empty() {
                r.toast = Some((r.tr("t.selectFirst"), "warn".into()));
                r.go(Page::Accounts);
                cx.notify();
                return;
            }
            pages::join::open(r, ids, window, cx);
        }
        Run::Add => r.add_account(cx),
        // How many are signed in, how many need signing in again, how many couldn't be asked (app.js's checkAccounts).
        Run::CheckAll => r.ask("checkAccounts", json!({ "ids": [] }), cx, |r, v| {
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
        }),
        // Done shows on the cards themselves; only a click that did nothing is said.
        Run::Refresh => r.ask("refreshPresence", json!({}), cx, |r, v| {
            if v.get("skipped").and_then(Value::as_bool) == Some(true) {
                r.toast = Some((r.tr("t.refreshBusy"), "info".into()));
            }
        }),
        Run::Arrange => pages::snap::arrange_main(r, cx),
        Run::CloseAll => r.close_all(false, window, cx),
        Run::Send(cmd) => r.send(cmd, json!({})),
        Run::SelectAll => {
            r.go(Page::Accounts);
            pages::accounts::key(r, pages::accounts::Key::All, window, cx);
        }
        Run::Order => pages::accounts::open_order(r, window, cx),
        Run::SavePreset => pages::preset_menu::save_as(r, window, cx),
        Run::NewMacro => {
            r.auto.tab = pages::auto::Tab::Macros;
            r.auto.confirm = None;
            r.go(Page::Auto);
            pages::macro_form::open(r, None, window, cx);
        }
        Run::StopMacros => r.ask("stopMacros", json!({}), cx, |r, _| r.toast = Some((r.tr("macro.stoppedAll"), "ok".into()))),
        Run::Backup(true) => r.ask("backupNow", json!({}), cx, |r, _| r.toast = Some((r.tr("bk.done"), "ok".into()))),
        Run::Backup(false) => perform(r, Run::Settings(Part::Backup), window, cx),
        Run::Updates => {
            perform(r, Run::Settings(Part::Updates), window, cx);
            r.ask("checkUpdates", json!({}), cx, |_, _| {});
        }
        Run::Feedback => pages::extras::open_feedback(r, window, cx),
        // (The list may not be loaded yet: the app knows the events.)
        Run::Export => {
            r.stats.tab = pages::stats::Tab::Activity;
            r.go(Page::Stats);
            pages::stats::export(r, cx);
        }
        Run::Lang(l) => r.send("setLanguage", json!({ "language": l })),
        Run::Lock => r.lock_app(window, cx),
        Run::Go(p) => r.go(p),
        Run::Auto(tab) => {
            r.auto.tab = tab;
            r.go(Page::Auto);
        }
        Run::Activity => {
            r.stats.tab = pages::stats::Tab::Activity;
            r.go(Page::Stats);
        }
        Run::Panel(id, up) => r.send(if up { "hidePanel" } else { "showPanel" }, json!({ "id": id })),
        Run::Preset(id) => r.launch("runPreset", json!({ "id": id }), false, cx),
        // A game put in the bar's game box (no server), as the recent games do.
        Run::Game(place) => {
            r.go(Page::Accounts);
            pages::accounts::clear_job(window, cx);
            r.place.update(cx, |st, cx| st.set_value(format!("https://www.roblox.com/games/{place}"), window, cx));
            r.resolve_place(cx);
        }
        Run::Reveal(id) => reveal(r, &id, window, cx),
        Run::ShowWindow(id) => r.send("showWindows", json!({ "ids": [id] })),
        Run::Close(id) => r.ask("closeAccounts", json!({ "ids": [id], "force": false }), cx, |r, v| {
            let n = v.as_i64().unwrap_or(0);
            r.toast = Some(if n > 0 { (r.i18n.tn("t.closing", n, &[("n", &n.to_string())]), "ok".into()) } else { (r.tr("t.noneRunning"), "info".into()) });
        }),
        Run::Play(id) => {
            reveal(r, &id, window, cx);
            r.play(&[id], cx);
        }
        Run::Edit(id) => pages::account_form::open(r, Some(id), window, cx),
        Run::Settings(part) => {
            r.settings.part = part;
            r.set_q.update(cx, |st, cx| st.set_value("", window, cx));
            r.go(Page::Settings);
        }
    }
    cx.notify();
}

/// The highlighted entry (or its other action `alt`) run: first the palette closes, then the entry may open a dialog of
/// its own.
fn run_at(this: &WeakEntity<Root>, row: Option<usize>, alt: Option<usize>, window: &mut Window, cx: &mut App) {
    let Some(root) = this.upgrade() else { return };
    let p = &root.read(cx).palette;
    let Some(&i) = p.rows.get(row.unwrap_or(p.index)) else { return };
    let e = &p.entries[i];
    let run = match alt {
        Some(k) => match e.alts.get(k) {
            Some((_, run)) => run.clone(),
            None => return,
        },
        None => e.run.clone(),
    };
    window.close_dialog(cx);
    let _ = this.update(cx, |r, cx| {
        r.palette.open = false;
        perform(r, run, window, cx);
    });
}

fn step(r: &mut Root, by: i32, page: bool) {
    let p = &mut r.palette;
    let n = p.rows.len() as i32;
    if n == 0 {
        return;
    }
    let at = p.index as i32 + by;
    p.index = if page { at.clamp(0, n - 1) } else { at.rem_euclid(n) } as usize;
    let child = child_of(p, p.index);
    p.scroll.scroll_to_item(child);
}

// ───────────────────────── the box ─────────────────────────

/// Opens it (Ctrl+K). Not over another dialog, and not while locked.
pub fn open(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    // Ctrl+K again closes it (app.js).
    if r.palette.open {
        window.close_dialog(cx);
        r.palette.open = false;
        cx.notify();
        return;
    }
    if r.locked() || r.state.is_null() || window.has_active_dialog(cx) {
        return;
    }
    let hint = r.tr("pal.placeholder");
    let input = cx.new(|cx| InputState::new(window, cx).placeholder(hint));
    let typed = cx.subscribe(&input, |r: &mut Root, e, ev: &InputEvent, cx| {
        if matches!(ev, InputEvent::Change) {
            let q = e.read(cx).value().to_string();
            requery(&mut r.palette, &q);
            cx.notify();
        }
    });
    let seq = r.palette.seq.wrapping_add(1);
    let set_words = crate::pages::settings::part_words(r, cx);
    let entries = Rc::new(build(r, &set_words, cx));
    r.palette = Data { open: true, seq, _input: Some(input.clone()), _typed: Some(typed), entries, ..Default::default() };
    requery(&mut r.palette, "");
    // BLOXKEEP_SHOT_PAL="farm": a shot's palette with those words typed.
    #[cfg(feature = "shot")]
    if let Ok(q) = std::env::var("BLOXKEEP_SHOT_PAL") {
        // (set_value sends no Change, so the list is asked again here.)
        requery(&mut r.palette, &q);
        input.update(cx, |st, cx| st.set_value(q, window, cx));
    }
    let this = cx.weak_entity();
    // The box has the keyboard as the palette opens (given here, once; the box's draws leave it where it is).
    let keys = input.read(cx).focus_handle(cx);
    window.open_dialog(cx, move |dialog, window, cx| {
        let Some(root) = this.upgrade() else { return dialog };
        let top = window.viewport_size().height * 0.12;
        let r = root.read(cx);
        let p = &r.palette;
        let (entries, rows, mixed, more, index) = (p.entries.clone(), p.rows.clone(), p.mixed, p.more, p.index);
        let scroll = p.scroll.clone();
        let heads: Vec<String> = Kind::ALL.iter().map(|k| r.tr(k.heading())).collect();
        let query = input.read(cx).value().trim().to_string();
        let none = r.i18n.tv("pal.none", &[("q", &query)]);
        let more_text = r.i18n.tv("pal.more", &[("n", &more.to_string())]);
        let foot_words = [r.tr("pal.move"), r.tr("pal.open"), r.tr("pal.close")];

        let mut list: Vec<AnyElement> = Vec::new();
        let mut was: Option<Kind> = None;
        for (at, &i) in rows.iter().enumerate() {
            let e = &entries[i];
            if mixed && was != Some(e.kind) {
                let k = Kind::ALL.iter().position(|k| *k == e.kind).unwrap_or(0);
                list.push(
                    div()
                        .px(px(12.))
                        .pt(px(if at == 0 { 6. } else { 16. }))
                        .pb(px(6.))
                        .flex()
                        .items_center()
                        .gap(px(7.))
                        .text_size(px(T_S))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(MUTED))
                        .child(icon(e.kind.icon(), 13., c(MUTED)))
                        .child(heads[k].clone())
                        .into_any_element(),
                );
            }
            was = Some(e.kind);
            // The highlighted row is the one solid yellow block in the list (1.1's "chosen" look).
            let on = at == index;
            let (ink, soft) = if on { (c(INK), ca(INK, 0.66)) } else { (c(TEXT), c(MUTED)) };
            let lead = match &e.lead {
                Lead::Face(a) => crate::pages::accounts::avatar(a, 36., RAISED, cx),
                Lead::Pic(url) => {
                    let pic = crate::images::pic_sq(url, 36., cx);
                    lead_box(on).map(|d| match pic {
                        Some(p) => d.child(img(p).size_full().object_fit(ObjectFit::Cover)),
                        None => d.child(icon("gamepad", 17., if on { c(INK) } else { c(DIM) })),
                    })
                }
                Lead::Icon(name) => lead_box(on).child(icon(name, 17., if on { c(INK) } else { c(TEXT_2) })),
            };
            let mut row = div()
                .id(("pal-row", at))
                .min_h(px(52.))
                .px(px(10.))
                .py(px(7.))
                .flex()
                .items_center()
                .gap(px(14.))
                .rounded(px(R_L))
                .cursor_pointer()
                .when(on, |d| d.bg(c(FACE)))
                .child(lead)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(1.))
                        .child(div().truncate().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).text_color(ink).child(e.text.clone()))
                        .when(!e.hint.is_empty(), |d| d.child(div().truncate().text_size(px(T_S)).text_color(soft).child(e.hint.clone()))),
                );
            // The highlighted account's other actions, with their keys; a plain row shows the Enter key.
            if on {
                for (k, (text, _)) in e.alts.iter().enumerate() {
                    let this = this.clone();
                    row = row.child(
                        div()
                            .id(("pal-alt", k))
                            .flex_none()
                            .h(px(30.))
                            .px(px(10.))
                            .flex()
                            .items_center()
                            .gap(px(7.))
                            .rounded(px(R_S))
                            .bg(ca(INK, 0.1))
                            .hover(|d| d.bg(ca(INK, 0.18)))
                            .text_size(px(T_S))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c(INK))
                            .child(text.clone())
                            .child(div().text_size(px(T_XS)).font_weight(FontWeight::NORMAL).text_color(ca(INK, 0.6)).child(ALT_KEYS[k.min(1)]))
                            .on_click(move |_, w, cx| {
                                cx.stop_propagation();
                                run_at(&this, Some(at), Some(k), w, cx);
                            }),
                    );
                }
                if e.alts.is_empty() {
                    row = row.child(div().flex_none().pr(px(6.)).text_size(px(T_S)).font_weight(FontWeight::SEMIBOLD).text_color(ca(INK, 0.6)).child("Enter ↵"));
                }
            }
            let (moved, clicked) = (this.clone(), this.clone());
            row = row
                // Only a real move takes the highlight: rows scrolled under a resting pointer (the arrow keys) don't.
                .on_mouse_move(move |ev, _, cx| {
                    let _ = moved.update(cx, |r, cx| {
                        let p = &mut r.palette;
                        if p.pointer == Some(ev.position) {
                            return;
                        }
                        p.pointer = Some(ev.position);
                        if p.index != at {
                            p.index = at;
                            cx.notify();
                        }
                    });
                })
                .on_click(move |_, w, cx| run_at(&clicked, Some(at), None, w, cx));
            list.push(row.into_any_element());
        }
        if more > 0 {
            list.push(div().px(px(12.)).pt(px(10.)).pb(px(6.)).text_center().text_size(px(T_M)).text_color(c(MUTED)).child(more_text).into_any_element());
        }
        let body = if rows.is_empty() {
            div().py(px(28.)).px(px(10.)).text_center().text_size(px(T_M)).text_color(c(MUTED)).child(none).into_any_element()
        } else {
            div().id("pal-list").max_h(px(460.)).overflow_y_scroll().track_scroll(&scroll).px(px(10.)).pb(px(8.)).flex().flex_col().gap(px(2.)).children(list).into_any_element()
        };
        // A key is a small block standing on its darker base, like the kit's buttons.
        let kbd = |t: &str| {
            div()
                .min_w(px(24.))
                .h(px(22.))
                .px(px(6.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(6.))
                .bg(c(RAISED_2))
                .border_b_2()
                .border_color(c(GROUND))
                .text_size(px(T_XS))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(TEXT_2))
                .child(t.to_string())
        };
        let [w_move, w_open, w_close] = foot_words;
        let foot = div()
            .flex()
            .flex_wrap()
            .gap(px(20.))
            .px(px(18.))
            .pt(px(12.))
            .pb(px(14.))
            .border_t_1()
            .border_color(c(LINE))
            .text_size(px(T_S))
            .text_color(c(MUTED))
            .child(div().flex().items_center().gap(px(5.)).child(kbd("↑")).child(kbd("↓")).child(div().pl(px(2.)).child(w_move)))
            .child(div().flex().items_center().gap(px(5.)).child(kbd("Enter")).child(div().pl(px(2.)).child(w_open)))
            .child(div().flex().items_center().gap(px(5.)).child(kbd("Esc")).child(div().pl(px(2.)).child(w_close)));

        let moves = |by: i32, page: bool| {
            let this = this.clone();
            move |cx: &mut App| {
                let _ = this.update(cx, |r, cx| {
                    step(r, by, page);
                    cx.notify();
                });
            }
        };
        let (up, down, pgup, pgdn) = (moves(-1, false), moves(1, false), moves(-8, true), moves(8, true));
        let (enter, alt0, alt1) = (this.clone(), this.clone(), this.clone());
        let content = div()
            .key_context(CONTEXT)
            .on_action(move |_: &PalUp, _, cx| up(cx))
            .on_action(move |_: &PalDown, _, cx| down(cx))
            .on_action(move |_: &PalPageUp, _, cx| pgup(cx))
            .on_action(move |_: &PalPageDown, _, cx| pgdn(cx))
            .on_action(move |_: &PalEnter, w, cx| run_at(&enter, None, None, w, cx))
            .on_action(move |_: &PalAltEnter, w, cx| run_at(&alt0, None, Some(0), w, cx))
            .on_action(move |_: &PalShiftEnter, w, cx| run_at(&alt1, None, Some(1), w, cx))
            .on_action(|_: &PalTab, _, _| {})
            .w_full()
            .flex()
            .flex_col()
            .child(div().p(px(14.)).border_b_1().border_color(c(LINE)).child(field(&input, Some("search")).h(px(56.)).px(px(16.)).text_size(px(17.)).rounded(px(R_L))))
            .child(div().pt(px(6.)).child(body))
            .child(foot);
        let closed = this.clone();
        dialog
            .close_button(false)
            .p_0()
            .gap_0()
            .w(px(680.))
            .margin_top(top)
            .entrance(DialogEntrance::FadeSlide)
            // (Esc, or a click beside it.)
            .on_close(move |_, _, cx| {
                let _ = closed.update(cx, |r, cx| {
                    if r.palette.seq == seq {
                        r.palette.open = false;
                    }
                    // (Drawn again at once: the windows' pictures come back as soon as it is gone.)
                    cx.notify();
                });
            })
            .child(content)
    });
    window.focus(&keys, cx);
    cx.notify();
}

/// The square an icon or a game's picture sits in.
fn lead_box(on: bool) -> Div {
    div().size(px(36.)).flex_none().flex().items_center().justify_center().rounded(px(R_M)).overflow_hidden().bg(if on { ca(INK, 0.12) } else { c(RAISED_2) })
}
