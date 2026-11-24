// Game collection: an account's own games laid out on its profile, as Discord's profile Board has them (its widgets,
// studied 2026-11-24 on Discord's blog "How to Customize Your Discord Profile" and its Profile Widgets FAQ): a section
// for each kind, added once at most, each with Add game at its right, the games' pictures in it, a game's own tags and a
// note on the favourite; a game under the mouse can be taken out; a section's ⋮ moves it up or down or takes it off
// (asked first). Add game opens a box: Roblox's search by name and, before anything is typed, games suggested from the
// account's own (played from Bloxkeep lately, pinned, its Roblox favourites). A game pressed opens its page here.
//
// Bloxkeep's own parts and words: Roblox's square game icons (Roblox has no box art), Bloxkeep's blocks and pills, its
// own tag list. Kept in Bloxkeep only, per Roblox account (prefs "gameCollection:<user>"): Roblox has no such thing to
// put it in, so the section says it is seen here only.
use std::collections::HashMap;
use std::time::Duration;

use gpui_kit::component::WindowExt as _;
use gpui_kit::component::dialog::DialogEntrance;
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use crate::ask::answer;
use crate::val::{int, s};
use crate::{images, kit::*, shell::Root, theme::*};

/// The kinds of section, in the order a new board offers them, and how many games each holds (Discord's own: one
/// favourite, five being played, twenty wanted, twenty played).
pub const KINDS: [(&str, usize); 4] = [("favorite", 1), ("playing", 5), ("want", 20), ("played", 20)];
/// The tags a game can carry: how well they play it (one of these at most)…
const SKILL: [&str; 5] = ["new", "learning", "casual", "good", "expert"];
/// …and how they feel about it or what they look for (any of these).
const FEEL: [&str; 7] = ["love", "like", "grind", "lfg", "open", "tips", "teach"];
const TYPE_DELAY: Duration = Duration::from_millis(330);

#[derive(Clone, Default)]
pub struct Game {
    pub place: i64,
    pub universe: i64,
    pub name: String,
    pub icon: String,
    pub note: String,
    pub tags: Vec<String>,
}

/// One account's board: its sections in the order shown, each with its games.
#[derive(Clone, Default)]
pub struct Board {
    pub parts: Vec<(String, Vec<Game>)>,
}

fn cap(kind: &str) -> usize {
    KINDS.iter().find(|k| k.0 == kind).map_or(20, |k| k.1)
}

fn game_of(v: &Value) -> Option<Game> {
    let place = int(v, "place");
    (place > 0).then(|| Game {
        place,
        universe: int(v, "universe"),
        name: s(v, "name").to_string(),
        icon: s(v, "icon").to_string(),
        note: s(v, "note").to_string(),
        tags: v.get("tags").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).filter(|t| SKILL.contains(t) || FEEL.contains(t)).map(str::to_string).collect(),
    })
}

pub fn board(user: i64) -> Board {
    #[cfg(feature = "shot")]
    if std::env::var_os("BLOXKEEP_SHOT_GC").is_some() {
        return shot_board();
    }
    let Some(v) = crate::prefs::get(&format!("gameCollection:{user}")) else { return Board::default() };
    let mut parts: Vec<(String, Vec<Game>)> = Vec::new();
    for p in v.get("parts").and_then(Value::as_array).into_iter().flatten() {
        let kind = s(p, "kind");
        if !KINDS.iter().any(|k| k.0 == kind) || parts.iter().any(|x| x.0 == kind) {
            continue;
        }
        let mut games: Vec<Game> = p.get("games").and_then(Value::as_array).into_iter().flatten().filter_map(game_of).collect();
        games.truncate(cap(kind));
        parts.push((kind.to_string(), games));
    }
    Board { parts }
}

fn keep(user: i64, b: &Board) {
    let parts: Vec<Value> = b
        .parts
        .iter()
        .map(|(kind, games)| {
            json!({ "kind": kind, "games": games.iter().map(|g| json!({ "place": g.place, "universe": g.universe, "name": g.name, "icon": g.icon, "note": g.note, "tags": g.tags })).collect::<Vec<_>>() })
        })
        .collect();
    crate::prefs::set(&format!("gameCollection:{user}"), json!({ "parts": parts }));
}

fn change(user: i64, f: impl FnOnce(&mut Board)) {
    let mut b = board(user);
    f(&mut b);
    keep(user, &b);
}

/// The games' icons as Roblox has them now (asked once a run for the ones kept: an icon's address can change when a
/// game's maker swaps it).
#[derive(Default)]
pub struct Data {
    /// Add game's box: whose board and which section.
    adding: Option<(i64, String)>,
    /// Its search: what was asked and the games Roblox gave (None while asked; Err when it failed); the latest asking.
    found: Option<(String, Option<Result<Vec<Value>, ()>>)>,
    seq: u64,
    /// The games suggested before anything is typed (the app's lists), once read.
    suggested: Option<Vec<Value>>,
    /// The ⋮ of a section open, a game's tag list open (section, game).
    menu: Option<String>,
    tag_menu: Option<(String, usize)>,
    /// The note being written (whose board, section, game).
    noting: Option<(i64, String, usize)>,
    /// The add-a-section list open.
    more_open: bool,
    icons: HashMap<i64, String>,
    icons_asked: bool,
    #[cfg(feature = "shot")]
    shot_done: bool,
}

#[cfg(feature = "shot")]
fn shot_board() -> Board {
    let g = |place: i64, universe: i64, name: &str, tags: &[&str], note: &str| Game { place, universe, name: name.into(), icon: String::new(), note: note.into(), tags: tags.iter().map(|t| t.to_string()).collect() };
    match std::env::var("BLOXKEEP_SHOT_GC").as_deref() {
        Ok("empty") => Board::default(),
        _ => Board {
            parts: vec![
                ("favorite".into(), vec![g(2753915549, 994732206, "Blox Fruits", &["expert", "grind"], "Sea 3 every night, still no Dragon")]),
                ("playing".into(), vec![g(920587237, 383310974, "Adopt Me!", &["casual", "love"], ""), g(4924922222, 1686885941, "Brookhaven RP", &["open"], ""), g(142823291, 66654135, "Murder Mystery 2", &[], "")]),
                ("want".into(), vec![g(606849621, 245662005, "Jailbreak", &[], ""), g(6516141723, 2440500124, "DOORS", &[], ""), g(735030788, 280306617, "Royale High", &[], ""), g(2788229376, 1008451066, "Da Hood", &[], "")]),
            ],
        },
    }
}

/// The icon to draw for a game: Roblox's newest, else the one kept with it.
fn icon_url(root: &Root, g: &Game) -> String {
    root.collection.icons.get(&g.universe).cloned().unwrap_or_else(|| g.icon.clone())
}

/// Asks the app for the board's icons once a run (and the ones kept with no icon).
fn want_icons(r: &mut Root, user: i64, cx: &mut Context<Root>) {
    if r.collection.icons_asked {
        return;
    }
    let Some(link) = r.link.clone() else { return };
    r.collection.icons_asked = true;
    let ids: Vec<i64> = board(user).parts.iter().flat_map(|p| p.1.iter().map(|g| g.universe)).filter(|u| *u > 0).collect();
    if ids.is_empty() {
        return;
    }
    let rx = link.call("gameIcons", json!({ "universeIds": ids }));
    answer(rx, cx, |r, got, cx| {
        if let Ok(Ok(Value::Object(m))) = got {
            for (k, v) in m {
                if let (Ok(id), Some(u)) = (k.parse::<i64>(), v.as_str()) {
                    r.collection.icons.insert(id, u.to_string());
                }
            }
            cx.notify();
        }
    });
}

fn kind_word(root: &Root, kind: &str) -> String {
    root.tr(&format!("gcol.k.{kind}"))
}

fn open_game(r: &mut Root, place: i64, name: &str) {
    r.games.view = crate::pages::games::View::Games;
    crate::pages::games::open_place(r, place, name);
}

/// A game's picture, square, `size` px, rounded; its name's letter on a colour while none is had.
fn art(root: &Root, g: &Game, size: f32, cx: &mut Context<Root>) -> Div {
    let url = icon_url(root, g);
    let pic = (!url.is_empty()).then(|| images::pic_pair(&url, size, size, cx)).flatten();
    let r = if size >= 72. { R_L } else { R_M };
    div()
        .size(px(size))
        .flex_none()
        .rounded(px(r))
        .overflow_hidden()
        .bg(c(RAISED_2))
        .border_1()
        .border_color(c(LINE))
        .map(|d| match pic {
            Some((src, key)) => d.child(fade_in(img(src).size_full().rounded(px(r)).object_fit(ObjectFit::Cover), &key)),
            None => d.flex().items_center().justify_center().child(icon("gamepad", (size * 0.3).min(22.), c(MUTED))),
        })
}

/// The board on an account's own profile: its sections, or the four to start from.
pub fn section(root: &Root, user: i64, inner: f32, cx: &mut Context<Root>) -> AnyElement {
    if !root.collection.icons_asked {
        cx.spawn(async move |this, cx| {
            let _ = this.update(cx, |r, cx| want_icons(r, user, cx));
        })
        .detach();
    }
    let b = board(user);
    let missing: Vec<&str> = KINDS.iter().map(|k| k.0).filter(|k| !b.parts.iter().any(|p| p.0 == *k)).collect();
    let head = section_head(root.tr("gcol.title"), None)
        .child(div().flex_none().flex().items_center().gap(px(6.)).text_size(px(T_S)).text_color(c(MUTED)).child(icon("eye", 13., c(MUTED))).child(root.tr("gcol.only")))
        .when(!missing.is_empty() && !b.parts.is_empty(), |d| d.child(more_button(root, user, &missing, cx)));
    let body: AnyElement = if b.parts.is_empty() {
        starters(root, user, inner, cx)
    } else {
        // The favourite and the ones being played (a few lines each) side by side, as tall as each other, where there
        // is room and they come one after the other; the shelves of pictures (up to 20) across the whole width, so
        // a short block never leaves a hole beside a tall one.
        let half = ((inner - GRID_GAP) / 2.).floor();
        let short = |k: &str| matches!(k, "favorite" | "playing");
        let n = b.parts.len();
        let mut widths = vec![inner; n];
        let mut i = 0;
        while i + 1 < n {
            if inner >= 880. && short(&b.parts[i].0) && short(&b.parts[i + 1].0) {
                widths[i] = half;
                widths[i + 1] = half;
                i += 2;
            } else {
                i += 1;
            }
        }
        div()
            .flex()
            .flex_wrap()
            .gap(px(GRID_GAP))
            .children(b.parts.iter().enumerate().map(|(i, (kind, games))| part(root, user, kind, games, i, n, widths[i], cx)))
            .into_any_element()
    };
    div().flex().flex_col().gap(px(12.)).child(head).child(body).into_any_element()
}

/// The empty board: one tile for each kind (Discord's "Customize your profile with widgets"), a press opens Add game
/// for it.
fn starters(root: &Root, user: i64, inner: f32, cx: &mut Context<Root>) -> AnyElement {
    let cols = if inner >= 880. { 4. } else if inner >= 440. { 2. } else { 1. };
    let w = ((inner - GRID_GAP * (cols - 1.)) / cols).floor();
    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .child(div().text_size(px(T_M)).text_color(c(TEXT_2)).child(root.tr("gcol.start")))
        .child(div().flex().flex_wrap().gap(px(GRID_GAP)).children(KINDS.iter().map(|(kind, n)| {
            let kind = kind.to_string();
            let cap_words = root.i18n.tn("gcol.capUpTo", *n as i64, &[]);
            block_card(SharedString::from(format!("gc-start-{kind}")))
                .w(px(w))
                .h(px(76.))
                .px(px(14.))
                .flex()
                .items_center()
                .gap(px(12.))
                .cursor_pointer()
                .child(div().size(px(40.)).flex_none().rounded(px(R_M)).bg(c(RAISED_2)).flex().items_center().justify_center().child(icon(glyph(&kind), 18., c(FACE))))
                .child(div().flex_1().min_w_0().flex().flex_col().gap(px(2.)).child(div().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).truncate().child(kind_word(root, &kind))).child(div().text_size(px(T_S)).text_color(c(MUTED)).truncate().child(cap_words)))
                .child(icon("plus", 16., c(TEXT_2)))
                .on_click(cx.listener(move |r, _, window, cx| {
                    let k = kind.clone();
                    change(user, |b| b.parts.push((k, Vec::new())));
                    open_add(r, user, kind.clone(), window, cx);
                }))
                .into_any_element()
        })))
        .into_any_element()
}

/// Add a section (the kinds not on the board yet).
fn more_button(root: &Root, user: i64, missing: &[&str], cx: &mut Context<Root>) -> Div {
    let open = root.collection.more_open;
    let menu = div()
        .id("gc-more-menu")
        .occlude()
        .w(px(220.))
        .p(px(4.))
        .rounded(px(R_M))
        .bg(c(RAISED))
        .border_1()
        .border_color(c(LINE))
        .flex()
        .flex_col()
        .on_mouse_down_out(cx.listener(|r, _, _, cx| {
            r.collection.more_open = false;
            cx.notify();
        }))
        .children(missing.iter().map(|k| {
            let kind = k.to_string();
            menu_row(SharedString::from(format!("gc-more-{kind}"))).child(icon("plus", 14., c(MUTED))).child(kind_word(root, &kind)).on_click(cx.listener(move |r, _, window, cx| {
                r.collection.more_open = false;
                let k = kind.clone();
                change(user, |b| b.parts.push((k, Vec::new())));
                open_add(r, user, kind.clone(), window, cx);
            }))
        }));
    div()
        .relative()
        .flex_none()
        .child(Button::new("gc-more", root.tr("gcol.addPart")).icon("plus").small().quiet().on_click(cx.listener(|r, _, _, cx| {
            r.collection.more_open = !r.collection.more_open;
            cx.notify();
        })))
        .child(popup(PopAt::BelowRight(36.), "gc-more-pop", open, menu))
}

/// A section's mark: the one its starter card has.
fn glyph(kind: &str) -> &'static str {
    match kind {
        "favorite" => "star",
        "playing" => "gamepad",
        "want" => "bookmark",
        _ => "clock",
    }
}

/// One section: its head (name, how many it holds, Add game, ⋮) and its games.
#[allow(clippy::too_many_arguments)]
fn part(root: &Root, user: i64, kind: &str, games: &[Game], at: usize, of: usize, w: f32, cx: &mut Context<Root>) -> AnyElement {
    let n = cap(kind);
    let full = games.len() >= n;
    let cap_words = root.i18n.tn("gcol.capUpTo", n as i64, &[]);
    let k = kind.to_string();
    let head = div()
        .flex()
        .items_center()
        .gap(px(8.))
        // (Its mark in the starter card's tile, the name at a group's size and how many it holds beside it, as the
        // Home page's heads count theirs: "hơi xoàng" at 14 px, the owner, 2026-11-25.)
        .child(div().size(px(40.)).flex_none().mr(px(4.)).rounded(px(R_M)).bg(c(RAISED_2)).flex().items_center().justify_center().child(icon(glyph(kind), 18., c(FACE))))
        .child(div().flex_1().min_w_0().flex().flex_col().gap(px(2.))
            .child(div().flex().items_baseline().gap(px(8.)).min_w_0()
                .child(div().min_w_0().truncate().text_size(px(T_GROUP)).font_weight(FontWeight::SEMIBOLD).child(kind_word(root, kind)))
                .when(n > 1, |d| d.child(div().flex_none().text_size(px(T_SEC)).text_color(c(MUTED)).font_features(tnum()).child(format!("{}/{n}", games.len())))))
            .child(div().text_size(px(T_S)).text_color(c(MUTED)).truncate().child(cap_words)))
        .when(!full, |d| {
            let k = k.clone();
            d.child(Button::new(SharedString::from(format!("gc-add-{kind}")), root.tr("gcol.add")).icon("plus").small().on_click(cx.listener(move |r, _, window, cx| open_add(r, user, k.clone(), window, cx))))
        })
        .child(part_menu(root, user, kind, at, of, cx));
    let inner = w - 2. * 16.;
    let body: AnyElement = if games.is_empty() {
        div().h(px(64.)).flex().items_center().text_size(px(T_M)).text_color(c(MUTED)).child(root.tr("gcol.noneYet")).into_any_element()
    } else if kind == "favorite" {
        favourite(root, user, &games[0], inner, cx)
    } else if kind == "playing" {
        div().flex().flex_col().gap(px(10.)).children(games.iter().enumerate().map(|(i, g)| playing_row(root, user, kind, i, g, cx))).into_any_element()
    } else {
        shelf(root, user, kind, games, inner, cx)
    };
    block().w(px(w)).flex_none().p(px(16.)).flex().flex_col().gap(px(14.)).child(head).child(body).into_any_element()
}

/// A section's ⋮: move it up or down, take it off (asked first, with its games).
fn part_menu(root: &Root, user: i64, kind: &str, at: usize, of: usize, cx: &mut Context<Root>) -> Div {
    let open = root.collection.menu.as_deref() == Some(kind);
    let k = kind.to_string();
    let row = |id: String| menu_row(SharedString::from(id));
    let (k1, k2, k3) = (k.clone(), k.clone(), k.clone());
    let menu = div()
        .id(SharedString::from(format!("gc-menu-{kind}")))
        .occlude()
        .w(px(200.))
        .p(px(4.))
        .rounded(px(R_M))
        .bg(c(RAISED))
        .border_1()
        .border_color(c(LINE))
        .flex()
        .flex_col()
        .on_mouse_down_out(cx.listener(|r, _, _, cx| {
            r.collection.menu = None;
            cx.notify();
        }))
        .when(at > 0, |d| {
            d.child(row(format!("gc-up-{kind}")).child(icon("chevronUp", 14., c(MUTED))).child(root.tr("gcol.up")).on_click(cx.listener(move |r, _, _, cx| {
                r.collection.menu = None;
                change(user, |b| {
                    if let Some(i) = b.parts.iter().position(|p| p.0 == k1) {
                        if i > 0 {
                            b.parts.swap(i, i - 1);
                        }
                    }
                });
                cx.notify();
            })))
        })
        .when(at + 1 < of, |d| {
            d.child(row(format!("gc-down-{kind}")).child(icon("chevronDown", 14., c(MUTED))).child(root.tr("gcol.down")).on_click(cx.listener(move |r, _, _, cx| {
                r.collection.menu = None;
                change(user, |b| {
                    if let Some(i) = b.parts.iter().position(|p| p.0 == k2) {
                        if i + 1 < b.parts.len() {
                            b.parts.swap(i, i + 1);
                        }
                    }
                });
                cx.notify();
            })))
        })
        .child(row(format!("gc-drop-{kind}")).child(icon("trash", 14., c(MUTED))).child(root.tr("gcol.removePart")).on_click(cx.listener(move |r, _, window, cx| {
            r.collection.menu = None;
            let k = k3.clone();
            let name = kind_word(r, &k);
            let empty = board(user).parts.iter().find(|p| p.0 == k).is_none_or(|p| p.1.is_empty());
            let drop = move |_: &mut Root, cx: &mut Context<Root>| {
                change(user, |b| b.parts.retain(|p| p.0 != k));
                cx.notify();
            };
            if empty {
                drop(r, cx);
            } else {
                r.confirm(r.tr("gcol.removePart"), r.i18n.tv("gcol.removeAsk", &[("name", &name)]), r.tr("gcol.remove"), true, None, window, cx, drop);
            }
        })));
    div()
        .relative()
        .flex_none()
        .child({
            let b = IconButton::new(SharedString::from(format!("gc-more-{kind}-btn")), "more").size(30.);
            if open { b } else { b.tip(root.tr("gcol.partMore")) }
        }
        .on_click(cx.listener(move |r, _, _, cx| {
            r.collection.menu = if r.collection.menu.as_deref() == Some(k.as_str()) { None } else { Some(k.clone()) };
            cx.notify();
        })))
        .child(popup(PopAt::BelowRight(34.), SharedString::from(format!("gc-menu-pop-{kind}")), open, menu))
}

/// The × over a game under the mouse: takes it out of its section. Inside the picture's corner, not hanging off it:
/// half outside, the mouse left the picture on its way to it and it faded away under it (the owner, 2026-11-25: "nút
/// remove khó bấm vl").
fn take_out(root: &Root, user: i64, kind: &str, i: usize, group: SharedString, cx: &mut Context<Root>) -> Div {
    let k = kind.to_string();
    div().absolute().top(px(6.)).right(px(6.)).opacity(0.).group_hover(group, |s| s.opacity(1.)).child(
        IconButton::new(SharedString::from(format!("gc-x-{kind}-{i}")), "x").size(28.).round().filled().tip(root.tr("gcol.remove")).on_click(cx.listener(move |r, _, _, cx| {
            let k = k.clone();
            change(user, |b| {
                if let Some(p) = b.parts.iter_mut().find(|p| p.0 == k) {
                    if i < p.1.len() {
                        p.1.remove(i);
                    }
                }
            });
            // (A tag list or note of a game that moved up is not that game's any more.)
            r.collection.tag_menu = None;
            r.collection.noting = None;
            cx.notify();
        })),
    )
}

/// The favourite: its picture large, its name, the note (Discord's ✎ line) and its tags.
fn favourite(root: &Root, user: i64, g: &Game, inner: f32, cx: &mut Context<Root>) -> AnyElement {
    let size = if inner >= 360. { 112. } else { 84. };
    let group: SharedString = "gc-fav".into();
    let (place, name) = (g.place, g.name.clone());
    div()
        .flex()
        .items_start()
        .gap(px(16.))
        .child(
            div()
                .relative()
                .flex_none()
                .group(group.clone())
                .child(div().id("gc-fav-art").cursor_pointer().hover(|d| d.opacity(0.88)).on_click(cx.listener(move |r, _, _, cx| {
                    open_game(r, place, &name);
                    cx.notify();
                })).child(art(root, g, size, cx)))
                .child(take_out(root, user, "favorite", 0, group, cx)),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .pt(px(4.))
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(div().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).line_height(px(24.)).truncate().child(g.name.clone()))
                .child(note_line(root, user, g, cx))
                .child(tags_row(root, user, "favorite", 0, g, cx)),
        )
        .into_any_element()
}

/// The favourite's note: its words after a pen, a press to write them; "Add a note" while there are none.
fn note_line(root: &Root, user: i64, g: &Game, cx: &mut Context<Root>) -> AnyElement {
    let writing = root.collection.noting.as_ref().is_some_and(|n| n.0 == user && n.1 == "favorite" && n.2 == 0);
    if writing {
        return div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(div().flex_1().min_w_0().child(field(&root.gc_note, None)))
            .child(Button::new("gc-note-save", root.tr("common.done")).small().primary().on_click(cx.listener(|r, _, _, cx| save_note(r, cx))))
            .into_any_element();
    }
    let empty = g.note.trim().is_empty();
    let note = g.note.clone();
    div()
        .id("gc-note")
        .flex()
        .items_center()
        .gap(px(7.))
        .max_w_full()
        .cursor_pointer()
        .text_size(px(T_M))
        .text_color(c(if empty { MUTED } else { TEXT_2 }))
        .hover(|d| d.text_color(c(TEXT)))
        .child(icon("edit", 13., c(MUTED)))
        .child(div().min_w_0().truncate().child(if empty { root.tr("gcol.note") } else { note.clone() }))
        .on_click(cx.listener(move |r, _, window, cx| {
            r.collection.noting = Some((user, "favorite".into(), 0));
            let v = note.clone();
            r.gc_note.update(cx, |st, cx| st.set_value(v, window, cx));
            r.gc_note.update(cx, |st, cx| st.focus(window, cx));
            cx.notify();
        }))
        .into_any_element()
}

/// Keeps the note being written (Done or Enter), 120 letters at most.
pub fn save_note(r: &mut Root, cx: &mut Context<Root>) {
    let Some((user, kind, i)) = r.collection.noting.take() else { return };
    let text: String = input_text(&r.gc_note, cx).chars().take(120).collect();
    change(user, |b| {
        if let Some(g) = b.parts.iter_mut().find(|p| p.0 == kind).and_then(|p| p.1.get_mut(i)) {
            g.note = text;
        }
    });
    cx.notify();
}

/// A game's tags, each with its ×, and "+ Tags" with the list to pick from.
fn tags_row(root: &Root, user: i64, kind: &str, i: usize, g: &Game, cx: &mut Context<Root>) -> AnyElement {
    let open = root.collection.tag_menu.as_ref().is_some_and(|t| t.0 == kind && t.1 == i);
    let k = kind.to_string();
    let chip = |t: &str| {
        let (k, t2) = (k.clone(), t.to_string());
        // (Its × only under the mouse, as a chip's on Discord: a row of crosses read as a list to delete.)
        let gid: SharedString = format!("gc-chip-{kind}-{i}-{t}").into();
        div()
            .group(gid.clone())
            .h(px(26.))
            .pl(px(10.))
            .pr(px(4.))
            .flex()
            .flex_none()
            .items_center()
            .gap(px(4.))
            .rounded_full()
            .bg(ca(WHITE, 0.06))
            .border_1()
            .border_color(c(LINE))
            .text_size(px(T_S))
            .font_weight(FontWeight::MEDIUM)
            .text_color(c(TEXT_2))
            .child(root.tr(&format!("gcol.t.{t}")))
            .child(
                div()
                    .id(SharedString::from(format!("gc-tx-{kind}-{i}-{t}")))
                    .size(px(18.))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .opacity(0.)
                    .group_hover(gid, |d| d.opacity(1.))
                    .hover(|d| d.bg(ca(WHITE, 0.1)))
                    .child(icon("x", 10., c(MUTED)))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        let (k, t) = (k.clone(), t2.clone());
                        change(user, |b| {
                            if let Some(g) = b.parts.iter_mut().find(|p| p.0 == k).and_then(|p| p.1.get_mut(i)) {
                                g.tags.retain(|x| *x != t);
                            }
                        });
                        cx.notify();
                    })),
            )
    };
    let pick = |t: &'static str, on: bool| {
        let k = k.clone();
        menu_row(SharedString::from(format!("gc-tp-{kind}-{i}-{t}")))
            .child(div().size(px(14.)).flex_none().when(on, |d| d.child(icon("check", 14., c(FACE)))))
            .child(root.tr(&format!("gcol.t.{t}")))
            .on_click(cx.listener(move |_, _, _, cx| {
                let k = k.clone();
                change(user, |b| {
                    if let Some(g) = b.parts.iter_mut().find(|p| p.0 == k).and_then(|p| p.1.get_mut(i)) {
                        if g.tags.iter().any(|x| x == t) {
                            g.tags.retain(|x| x != t);
                        } else {
                            // (One of how well they play at most: another replaces it.)
                            if SKILL.contains(&t) {
                                g.tags.retain(|x| !SKILL.contains(&x.as_str()));
                            }
                            g.tags.push(t.to_string());
                        }
                    }
                });
                cx.notify();
            }))
    };
    let has = |t: &str| g.tags.iter().any(|x| x == t);
    let label = |key: &str| div().px(px(10.)).pt(px(6.)).pb(px(2.)).text_size(px(T_XS)).font_weight(FontWeight::SEMIBOLD).text_color(c(MUTED)).child(root.tr(key));
    let menu = div()
        .id(SharedString::from(format!("gc-tags-{kind}-{i}")))
        .occlude()
        .w(px(230.))
        .p(px(4.))
        .rounded(px(R_M))
        .bg(c(RAISED))
        .border_1()
        .border_color(c(LINE))
        .flex()
        .flex_col()
        .on_mouse_down_out(cx.listener(|r, _, _, cx| {
            r.collection.tag_menu = None;
            cx.notify();
        }))
        .child(label("gcol.tagsSkill"))
        .children(SKILL.iter().map(|t| pick(t, has(t))))
        .child(div().my(px(4.)).h(px(1.)).bg(c(LINE)))
        .child(label("gcol.tagsFeel"))
        .children(FEEL.iter().map(|t| pick(t, has(t))));
    let k2 = kind.to_string();
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(6.))
        .children(g.tags.iter().map(|t| chip(t)))
        .child(
            div()
                .relative()
                .flex_none()
                .child(
                    div()
                        .id(SharedString::from(format!("gc-tag-add-{kind}-{i}")))
                        .h(px(26.))
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .gap(px(5.))
                        .rounded_full()
                        .border_1()
                        .border_color(c(LINE))
                        .cursor_pointer()
                        .text_size(px(T_S))
                        .text_color(c(MUTED))
                        .hover(|d| d.text_color(c(TEXT)).bg(ca(WHITE, 0.05)))
                        .child(icon("plus", 11., c(MUTED)))
                        .child(root.tr("gcol.tags"))
                        .on_click(cx.listener(move |r, _, _, cx| {
                            let me = (k2.clone(), i);
                            r.collection.tag_menu = if r.collection.tag_menu.as_ref() == Some(&me) { None } else { Some(me) };
                            cx.notify();
                        })),
                )
                .child(popup(PopAt::BelowLeft(30.), SharedString::from(format!("gc-tags-pop-{kind}-{i}")), open, menu)),
        )
        .into_any_element()
}

/// One of the games being played: its picture, its name and its tags.
fn playing_row(root: &Root, user: i64, kind: &str, i: usize, g: &Game, cx: &mut Context<Root>) -> AnyElement {
    let group: SharedString = format!("gc-row-{kind}-{i}").into();
    let (place, name) = (g.place, g.name.clone());
    div()
        .flex()
        .items_center()
        .gap(px(14.))
        .child(
            div()
                .relative()
                .flex_none()
                .group(group.clone())
                .child(div().id(SharedString::from(format!("gc-art-{kind}-{i}"))).cursor_pointer().hover(|d| d.opacity(0.88)).on_click(cx.listener(move |r, _, _, cx| {
                    open_game(r, place, &name);
                    cx.notify();
                })).child(art(root, g, 64., cx)))
                .child(take_out(root, user, kind, i, group, cx)),
        )
        .child(div().flex_1().min_w_0().flex().flex_col().gap(px(7.)).child(div().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).truncate().child(g.name.clone())).child(tags_row(root, user, kind, i, g, cx)))
        .into_any_element()
}

/// Wanted or played: the pictures side by side, each with its name under it.
fn shelf(root: &Root, user: i64, kind: &str, games: &[Game], inner: f32, cx: &mut Context<Root>) -> AnyElement {
    const GAP: f32 = 12.;
    let cols = ((inner + GAP) / (84. + GAP)).floor().max(2.);
    let size = ((inner - GAP * (cols - 1.)) / cols).floor().min(108.);
    div()
        .flex()
        .flex_wrap()
        .gap(px(GAP))
        .children(games.iter().enumerate().map(|(i, g)| {
            let group: SharedString = format!("gc-tile-{kind}-{i}").into();
            let (place, name) = (g.place, g.name.clone());
            div()
                .relative()
                .w(px(size))
                .flex_none()
                .group(group.clone())
                .child(
                    div()
                        .id(SharedString::from(format!("gc-art-{kind}-{i}")))
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .cursor_pointer()
                        .hover(|d| d.opacity(0.88))
                        .on_click(cx.listener(move |r, _, _, cx| {
                            open_game(r, place, &name);
                            cx.notify();
                        }))
                        .child(art(root, g, size, cx))
                        .child(div().h(px(32.)).overflow_hidden().text_size(px(T_S)).line_height(px(16.)).text_color(c(TEXT_2)).child(g.name.clone())),
                )
                .child(take_out(root, user, kind, i, group, cx))
                .into_any_element()
        }))
        .into_any_element()
}

// ───────────────────────── Add game ─────────────────────────

/// Add game's box for a section: Roblox's search and the account's own games to start from.
fn open_add(r: &mut Root, user: i64, kind: String, window: &mut Window, cx: &mut Context<Root>) {
    r.collection.adding = Some((user, kind));
    r.collection.found = None;
    r.collection.seq += 1;
    r.gc_q.update(cx, |st, cx| st.set_value("", window, cx));
    if r.collection.suggested.is_none() {
        if let Some(link) = r.link.clone() {
            let rx = link.call("gameLists", json!({}));
            answer(rx, cx, |r, got, cx| {
                if let Ok(Ok(v)) = got {
                    let mut out: Vec<Value> = Vec::new();
                    for list in ["recent", "pinned", "favorites"] {
                        for g in v.get(list).and_then(Value::as_array).into_iter().flatten() {
                            if int(g, "placeId") > 0 && !out.iter().any(|x| int(x, "placeId") == int(g, "placeId")) {
                                out.push(g.clone());
                            }
                        }
                    }
                    r.collection.suggested = Some(out);
                }
                cx.notify();
            });
        }
    }
    let this = cx.weak_entity();
    let shut = this.clone();
    window.open_dialog(cx, move |dialog, window, cx| {
        let Some(root) = this.upgrade() else { return dialog };
        let (user, kind, title, close, done) = {
            let r = root.read(cx);
            let Some((user, kind)) = r.collection.adding.clone() else { return dialog };
            let title = r.i18n.tv("gcol.addTo", &[("part", &kind_word(r, &kind))]);
            (user, kind, title, r.tr("common.close"), r.tr("common.done"))
        };
        let body = add_body(&root, user, &kind, &this, window, cx);
        let (s1, s2) = (shut.clone(), shut.clone());
        framed(dialog, title, close, move |_, w, cx| {
            let _ = s1.update(cx, |r, _| r.collection.adding = None);
            w.close_dialog(cx);
        })
        .w(px(560.))
        .centered()
        .entrance(DialogEntrance::FadeSlide)
        .on_close(move |_, _, cx| {
            let _ = s2.update(cx, |r, _| r.collection.adding = None);
        })
        .child(dialog_body(body))
        .footer(dialog_foot().child(Button::new("gc-add-done", done).primary().on_click(|_, w, cx| w.close_dialog(cx))))
    });
}

/// What Add game's box holds: the search box, then the games found (or suggested), each with a tick once on the
/// section; a press adds it (or takes it back off).
fn add_body(root: &Entity<Root>, user: i64, kind: &str, this: &WeakEntity<Root>, _window: &mut Window, cx: &mut App) -> Div {
    let have: Vec<i64> = board(user).parts.iter().find(|p| p.0 == kind).map(|p| p.1.iter().map(|g| g.place).collect()).unwrap_or_default();
    let full = have.len() >= cap(kind);
    // (What is read of the app first: the pictures below need the app's context free.)
    let (q_box, _query, head, rows, empty_words, full_words, playing_words) = {
        let r = root.read(cx);
        let query = r.gc_q.read(cx).value().trim().to_string();
        let (head, rows): (String, Option<Vec<Value>>) = if query.is_empty() {
            (r.tr("gcol.suggested"), r.collection.suggested.clone())
        } else {
            match &r.collection.found {
                Some((q, Some(Ok(v)))) if *q == query => (r.tr("gcol.results"), Some(v.clone())),
                Some((q, Some(Err(())))) if *q == query => (r.tr("gcol.results"), Some(Vec::new())),
                _ => (r.tr("gcol.results"), None),
            }
        };
        let playing: Vec<String> = rows.iter().flatten().take(40).map(|g| { let n = int(g, "playing"); if n > 0 { r.i18n.tv("gcol.playing", &[("n", &thousands(n))]) } else { String::new() } }).collect();
        (r.gc_q.clone(), query.clone(), head, rows, r.tr(if query.is_empty() { "gcol.noSuggest" } else { "gcol.none" }), r.tr("gcol.full"), playing)
    };
    let list: AnyElement = match rows {
        None => div().h(px(120.)).flex().items_center().justify_center().child(spinner(18., c(MUTED))).into_any_element(),
        Some(v) if v.is_empty() => div().h(px(120.)).flex().items_center().justify_center().text_size(px(T_M)).text_color(c(MUTED)).child(empty_words).into_any_element(),
        Some(v) => div()
            .id("gc-add-list")
            .max_h(px(340.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(2.))
            .children(v.iter().take(40).enumerate().map(|(n, g)| {
                let place = int(g, "placeId");
                let on = have.contains(&place);
                let item = Game { place, universe: int(g, "universeId"), name: s(g, "name").to_string(), icon: s(g, "iconUrl").to_string(), note: String::new(), tags: Vec::new() };
                let pic = (!item.icon.is_empty()).then(|| images::pic_pair(&item.icon, 40., 40., cx)).flatten();
                let playing = playing_words.get(n).cloned().unwrap_or_default();
                let this = this.clone();
                let kind = kind.to_string();
                let off = full && !on;
                hot(("gc-pick", n))
                    .h(px(52.))
                    .px(px(8.))
                    .rounded(px(R_M))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .when(!off, |d| d.cursor_pointer())
                    .when(off, |d| d.opacity(0.45))
                    .child(div().size(px(40.)).flex_none().rounded(px(R_M)).overflow_hidden().bg(c(RAISED_2)).map(|d| match pic {
                        Some((src, key)) => d.child(fade_in(img(src).size_full().rounded(px(R_M)).object_fit(ObjectFit::Cover), &key)),
                        None => d.flex().items_center().justify_center().child(icon("gamepad", 16., c(MUTED))),
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(1.))
                            .child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).truncate().child(item.name.clone()))
                            .when(!playing.is_empty(), |d| d.child(div().text_size(px(T_S)).text_color(c(MUTED)).font_features(tnum()).child(playing))),
                    )
                    .child(div().size(px(26.)).flex_none().rounded_full().flex().items_center().justify_center().map(|d| if on { d.bg(c(FACE)).child(icon("check", 14., crate::theme::on_text())) } else { d.border_1().border_color(c(LINE)).child(icon("plus", 13., c(MUTED))) }))
                    .on_click(move |_, window, cx| {
                        if off {
                            return;
                        }
                        let (kind, item) = (kind.clone(), item.clone());
                        let _ = this.update(cx, |_, cx| {
                            change(user, |b| {
                                let Some(p) = b.parts.iter_mut().find(|p| p.0 == kind) else { return };
                                if let Some(at) = p.1.iter().position(|g| g.place == item.place) {
                                    p.1.remove(at);
                                } else if p.1.len() < cap(&kind) {
                                    p.1.push(item);
                                }
                            });
                            cx.notify();
                        });
                        // (The favourite holds one: picked, the box has done its work.)
                        if cap(&kind) == 1 && !on {
                            window.close_dialog(cx);
                        }
                        window.refresh();
                    })
            }))
            .into_any_element(),
    };
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(field(&q_box, Some("search")))
        .child(div().flex().items_center().text_size(px(T_S)).font_weight(FontWeight::SEMIBOLD).text_color(c(MUTED)).child(head).child(div().flex_1()).when(cap(kind) > 1, |d| d.font_features(tnum()).child(format!("{}/{}", have.len(), cap(kind)))))
        .when(full && cap(kind) > 1, |d| d.child(small_note(full_words)))
        .child(list)
}

/// Add game's search box changed (a pause while typing asks Roblox; Enter asks at once).
pub fn typed(r: &mut Root, immediate: bool, window: &mut Window, cx: &mut Context<Root>) {
    if r.collection.adding.is_none() {
        return;
    }
    r.collection.seq += 1;
    let query = input_text(&r.gc_q, cx);
    window.refresh();
    if query.is_empty() {
        r.collection.found = None;
        return;
    }
    let mine = r.collection.seq;
    r.collection.found = Some((query.clone(), None));
    cx.spawn_in(window, async move |this, cx| {
        if !immediate {
            cx.background_executor().timer(TYPE_DELAY).await;
        }
        let Ok(Some(link)) = this.update(cx, |r, _| (r.collection.adding.is_some() && r.collection.seq == mine).then(|| r.link.clone()).flatten()) else { return };
        let got = link.call("searchGames", json!({ "query": query })).await;
        let rows = match got {
            Ok(Ok(v)) => Ok(v.as_array().cloned().unwrap_or_default()),
            _ => Err(()),
        };
        let _ = this.update_in(cx, |r, window, cx| {
            if r.collection.seq == mine {
                r.collection.found = Some((query, Some(rows)));
                window.refresh();
                cx.notify();
            }
        });
    })
    .detach();
}

// ───────────────────────── Elsewhere ─────────────────────────

/// Discord's row on a small profile: "Game collection" and the first few pictures, `+N` for the rest; nothing while
/// the board holds no game. `go` opens the profile.
pub fn row(root: &Root, user: i64, cx: &mut Context<Root>, go: impl Fn(&mut Root, &mut Context<Root>) + 'static) -> Option<AnyElement> {
    let b = board(user);
    let all: Vec<&Game> = b.parts.iter().flat_map(|p| p.1.iter()).collect();
    if all.is_empty() {
        return None;
    }
    let mut seen: Vec<i64> = Vec::new();
    let mut shown: Vec<&Game> = Vec::new();
    for g in &all {
        if !seen.contains(&g.place) {
            seen.push(g.place);
            shown.push(g);
        }
    }
    let more = shown.len().saturating_sub(4);
    let go = std::rc::Rc::new(go);
    Some(
        hot("gc-row")
            .h(px(52.))
            .px(px(12.))
            .rounded(px(R_L))
            .bg(c(RAISED))
            .border_1()
            .border_color(c(LINE))
            .flex()
            .items_center()
            .gap(px(10.))
            .cursor_pointer()
            .child(div().flex_1().min_w_0().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).truncate().child(root.tr("gcol.title")))
            .children(shown.iter().take(4).map(|g| art(root, g, 34., cx).into_any_element()))
            .when(more > 0, |d| d.child(div().h(px(34.)).min_w(px(34.)).px(px(6.)).rounded(px(R_M)).bg(c(RAISED_2)).flex().items_center().justify_center().text_size(px(T_S)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT_2)).font_features(tnum()).child(format!("+{more}"))))
            .on_click(cx.listener(move |r, _, _, cx| go(r, cx)))
            .into_any_element(),
    )
}

/// A hidden shot's Add game box (BLOXKEEP_SHOT_GC_ADD=want): opened over the profile once it is drawn.
#[cfg(feature = "shot")]
pub fn shot_add(r: &mut Root, user: i64, window: &mut Window, cx: &mut Context<Root>) {
    if let Ok(kind) = std::env::var("BLOXKEEP_SHOT_GC_ADD") {
        if !r.collection.shot_done {
            r.collection.shot_done = true;
            open_add(r, user, kind, window, cx);
        }
    }
}
