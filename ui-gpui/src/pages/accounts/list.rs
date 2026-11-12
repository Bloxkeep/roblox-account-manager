// The accounts as a list (the web page's list layout): a line each, in one box. The tick, the face, the name and
// @username, what it does (a red pill for a ban or a lost sign-in, Old build, the time played, Main, VNG), then the
// columns the list has (Robux, group, auto-rejoin, PID, RAM), each as wide in every line, then its button and ⋮.
// A line is picked like a card; a double click plays it, a right click opens its menu.
use gpui_kit::{prelude::*, *};
use serde_json::Value;

use super::{
    action, avatar, banned, foreign, id_of, is_kicked, is_local, label, launching, line_of, menu, more_button, no_pick, old_build, old_tag, on_right, pid_el, ram_mb, mb_text, rejoin_on, robux_el,
    robux_of, s, signed_out, tag, toggle, up_of,
};
use crate::{kit::*, shell::Root, theme::*};

/// The columns after the status: shown when any line has something for them, as wide as they need.
struct Cols {
    robux: bool,
    group: f32,
    rejoin: bool,
    pid: bool,
    ram: bool,
    /// The name's column, and the button's.
    name: f32,
    button: f32,
}

/// `narrow`: short of room (a window about 1180 wide), the status keeps its room: the name's and the button's columns
/// give some, and the PID (on the window's tile and in the menu too) goes.
pub fn rows(root: &Root, shown: &[&Value], narrow: bool, cx: &mut Context<Root>) -> Div {
    let longest = shown.iter().map(|a| s(a, "group").chars().count()).max().unwrap_or(0);
    let cols = Cols {
        robux: shown.iter().any(|a| robux_of(root, a).is_some()),
        // (About a letter's width each, the tag's sides, at most a short word's room: a longer one is cut with "…".)
        group: if longest > 0 { (longest as f32 * 7.2 + 16.).min(130.) } else { 0. },
        rejoin: shown.iter().any(|a| rejoin_on(a)),
        pid: !narrow && shown.iter().any(|a| is_local(a) && super::pid_of(a).is_some()),
        ram: shown.iter().any(|a| is_local(a)),
        name: if narrow { 190. } else { 230. },
        button: if narrow { 112. } else { 132. },
    };
    // One surface, a line an account with a hairline between (a card a line read as a stack of buttons), each line
    // as tall, its columns as wide in every line.
    let n = shown.len();
    let mut list = div().w_full().p(px(4.)).flex().flex_col().rounded(px(R_L + 2.)).bg(c(RAISED)).border_1().border_color(c(LINE));
    for (i, a) in shown.iter().enumerate() {
        list = list.child(row(root, a, &cols, cx));
        if i + 1 < n {
            list = list.child(div().h(px(1.)).mx(px(12.)).flex_none().bg(c(LINE)));
        }
    }
    list
}

fn row(root: &Root, a: &Value, cols: &Cols, cx: &mut Context<Root>) -> impl IntoElement {
    let id = id_of(a);
    let on = root.selected.contains(&id);
    let local = is_local(a);
    let (line, tone) = line_of(root, a);
    // A ban or a lost sign-in is the red pill, in the line's place (its words would only say it again).
    let pill = !local && !launching(a) && (banned(a) || signed_out(a)) && tone == BAD;
    let up = (local && tone == OK).then(|| up_of(a)).flatten();
    let robux = robux_of(root, a).map(|r| robux_el(root, &id, r));
    let group = s(a, "group").to_string();
    let ram = local.then(|| ram_mb(root, a)).flatten().map(|m| mb_text(m, root.i18n.vietnamese));
    let pid = local.then(|| pid_el(root, a, cx)).flatten();
    let pick = id.clone();
    let tick_id = id.clone();
    let main = action(root, a, "row", cx);
    let key = SharedString::from(format!("row-{id}"));
    let stuck = is_kicked(a) || super::held(a);
    // (A line inside the surface doesn't rise: it would leave its hairlines behind. Its ground eases in instead.)
    hot(key.clone())
        .group(key.clone())
        .relative()
        .h(px(56.))
        .pl(px(14.))
        .pr(px(6.))
        .flex()
        .items_center()
        .gap(px(14.))
        .rounded(px(R_M))
        .cursor_pointer()
        .map(|d| {
            if on {
                d.tones(tinted(RAISED, FACE, 0.10), tinted(RAISED, FACE, 0.14))
            } else if stuck {
                d.tones(tinted(RAISED, BAD, 0.08), tinted(RAISED, BAD, 0.12))
            } else {
                d.tones(ca(WHITE, 0.), ca(WHITE, 0.04))
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
        .children(menu::place(root, a, false, cx))
        // Playing here: a green rail at its edge (4 px, as DESIGN.md's status rails); dropped, a red one.
        .when(local && !stuck, |d| d.child(div().absolute().left_0().top(px(14.)).bottom(px(14.)).w(px(4.)).rounded_r(px(2.)).bg(c(OK))))
        .when(stuck, |d| d.child(div().absolute().left_0().top(px(14.)).bottom(px(14.)).w(px(4.)).rounded_r(px(2.)).bg(c(BAD))))
        // The tick: a small block, solid yellow once picked.
        .child(
            div()
                .id(SharedString::from(format!("row-tick-{id}")))
                .size(px(22.))
                .flex_none()
                .rounded(px(7.))
                .flex()
                .items_center()
                .justify_center()
                .border_1()
                .map(|d| {
                    if on {
                        d.bg(c(FACE)).border_color(c(FACE)).child(icon("check", 14., c(INK)))
                    } else {
                        d.bg(c(INSET)).border_color(ca(WHITE, 0.14)).group_hover(key.clone(), |s| s.border_color(ca(WHITE, 0.4)))
                    }
                })
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |r, _, _, cx| {
                    toggle(r, &tick_id);
                    cx.notify();
                })),
        )
        .child(avatar(a, 36., RAISED, cx))
        .child(
            div()
                .w(px(cols.name))
                .flex_none()
                .min_w_0()
                .flex()
                .items_baseline()
                .gap(px(8.))
                // The name keeps its room and the @username gives way first: both shrinking, a long username cut the
                // name to one letter ("C…" on the owner's real list, 2026-11-12).
                .child(div().flex_shrink_0().max_w(px(cols.name)).truncate().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).child(label(a)))
                .when(!s(a, "username").is_empty(), |d| d.child(div().min_w_0().flex_shrink(1.).truncate().text_size(px(T_S)).text_color(c(MUTED)).child(format!("@{}", s(a, "username"))))),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .gap(px(8.))
                .text_size(px(T_M))
                .when(pill, |d| d.child(badge(line.clone(), Tone::Bad)))
                .when(old_build(a), |d| d.child(old_tag(root, a, cx)))
                .when_some(local.then(|| super::fps_limit(a)).flatten(), |d, f| {
                    let tip: SharedString = root.tr("card.fpsTip").into();
                    d.child(div().id(SharedString::from(format!("row-fps-{id}"))).flex_none().child(tag(format!("{f} FPS"), TEXT_2)).tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()))
                })
                // The state's dot and words (words in the state's colour only when it isn't fine), then the time.
                .when(!pill, |d| {
                    d.child(div().size(px(7.)).flex_none().rounded_full().bg(c(if tone == MUTED { DIM } else { tone })))
                        .child(div().min_w_0().flex_shrink(1.).truncate().text_color(c(if tone == OK { TEXT_2 } else { tone })).child(line))
                })
                .when_some(up, |d, t| d.child(div().flex_none().text_color(c(MUTED)).font_features(tnum()).child(t)))
                .when(a.get("isMain").and_then(Value::as_bool).unwrap_or(false), |d| d.child(tag(root.tr("card.main"), TEXT_2)))
                .when(s(a, "realm") == "vng", |d| d.child(tag("VNG", ONLINE))),
        )
        .when(cols.robux, |d| d.child(div().w(px(92.)).flex_none().flex().justify_end().children(robux)))
        .when(cols.group > 0., |d| {
            d.child(div().w(px(cols.group)).flex_none().flex().min_w_0().when(!group.is_empty(), |d| d.child(tag("", TEXT_2).flex_shrink(1.).min_w_0().max_w_full().child(div().min_w_0().truncate().child(group.clone())))))
        })
        .when(cols.rejoin, |d| {
            let tip: SharedString = root.tr("card.rejoinTip").into();
            d.child(div().id(SharedString::from(format!("row-rj-{id}"))).w(px(16.)).flex_none().when(rejoin_on(a), |d| {
                d.child(icon("repeat", 14., c(ONLINE))).tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
            }))
        })
        .when(cols.pid, |d| d.child(div().w(px(84.)).flex_none().flex().justify_end().text_size(px(T_S)).children(pid)))
        .when(cols.ram, |d| {
            d.child(div().w(px(72.)).flex_none().flex().justify_end().text_size(px(T_S)).text_color(c(TEXT_2)).font_features(tnum()).children(ram))
        })
        // Its button, every line's as wide; then the menu.
        .child(div().w(px(cols.button)).flex_none().flex().justify_end().children(main.filter(|_| !foreign(a)).map(no_pick)))
        .child(more_button(root, a, "row", 28., cx))
}
