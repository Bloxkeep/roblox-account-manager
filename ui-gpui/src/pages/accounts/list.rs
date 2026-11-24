// The accounts as a list (the web page's list layout): a line each, in one box. The tick, the face, the name and
// @username, what it does (a red pill for a ban or a lost sign-in, Old build, the time played, Main, VNG), then the
// columns the list has (Robux, group, auto-rejoin, PID, RAM), each as wide in every line, then its button and ⋮.
// A line is picked like a card; a double click plays it, a right click opens its menu.
use gpui_kit::{prelude::*, *};
use serde_json::Value;

use super::{
    day_line, day_strip, dot_of, edit_button, game_icon, game_name, handle_of, last_line, note_of, opened_of, small_line, tags_of, want_strips,
    action, avatar, banned, foreign, id_of, is_kicked, is_local, label, launching, line_of, menu, more_button, no_pick, old_build, old_tag, on_right, pid_el, ram_mb, mb_text, rejoin_on, robux_el,
    robux_of, s, signed_out, tag, toggle, up_of,
};
use crate::{kit::*, shell::Root, theme::*};

/// The columns after the status: shown when any line has something for them, as wide as they need.
struct Cols {
    robux: bool,
    /// A window's frame-rate limit, a column of its own: before the status it pushed the dot and words of a line with
    /// one off those of a line without (the main account has none), so the lines didn't stand in line (owner, 2026-11-24).
    fps: bool,
    group: f32,
    rejoin: bool,
    pid: bool,
    ram: bool,
    /// The name's column, and the button's.
    name: f32,
    button: f32,
    gap: f32,
    /// The list is cut into a part a group: a line says no group (its part's head does).
    sectioned: bool,
}

/// Keep the status and row actions visible as the pane narrows. Secondary details are still available per account.
fn cols_of(shown: &[&Value], width: f32, sectioned: bool) -> Cols {
    let longest = shown.iter().map(|a| s(a, "group").chars().count()).max().unwrap_or(0);
    let (name, button, gap) = ((width * 0.24).clamp(112., 230.), (width * 0.14).clamp(88., 132.), if width < 880. { 10. } else { 14. });
    // (About a letter's width each, the tag's sides, at most a short word's room: a longer one is cut with "…".)
    let group = if longest > 0 && !sectioned { (longest as f32 * 7.2 + 16.).min(130.) } else { 0. };
    // The columns beside the name, the status and the button, least needed first: the FPS limit, the PID, the rejoin mark, the
    // memory, the group, the Robux. Each is given up, in that order, until the status has room for its words: a width
    // of its own decided each before, and at about 900 px they all stood while the status read "Pla…" (the shots of
    // every size, 2026-11-18).
    const STATUS: f32 = 220.;
    let mut on = [
        shown.iter().any(|a| super::fps_limit(a).is_some()),
        shown.iter().any(|a| is_local(a) && super::pid_of(a).is_some()),
        shown.iter().any(|a| rejoin_on(a)),
        shown.iter().any(|a| is_local(a)),
        group > 0.,
        shown.iter().any(|a| robux_of(a).is_some()),
    ];
    let takes = [FPS_W, 84., 26., 78., group, 92.];
    // (The line's sides, the tick, the face, the name, the button and ⋮, with the gaps between them.)
    let fixed = 20. + 30. + 46. + name + button + 40. + gap * 5.;
    let left = |on: &[bool; 6]| width - fixed - on.iter().zip(takes).filter(|(o, _)| **o).map(|(_, w)| w + gap).sum::<f32>();
    for i in 0..on.len() {
        if left(&on) >= STATUS {
            break;
        }
        on[i] = false;
    }
    Cols { fps: on[0], pid: on[1], rejoin: on[2], ram: on[3], group: if on[4] { group } else { 0. }, robux: on[5], name, button, gap, sectioned }
}

/// The accounts as a list. With groups among them, a part a group (the owner, 2026-11-18: "chia ngăn"): its head
/// (folds it; its name, how many; select all of it), then its own lines; the accounts in no group last. `shown` comes
/// ordered by group (accounts::shown_accounts). Every part has the same columns.
pub fn rows(root: &Root, shown: &[&Value], width: f32, cx: &mut Context<Root>) -> Div {
    let sectioned = shown.iter().any(|a| !s(a, "group").is_empty());
    let cols = cols_of(shown, width, sectioned);
    if !sectioned {
        return block(root, "acc-rows", shown, &cols, cx);
    }
    let mut col = div().w_full().flex().flex_col().gap(px(14.));
    let mut from = 0;
    while from < shown.len() {
        let group = s(shown[from], "group");
        let to = from + shown[from..].iter().take_while(|a| s(a, "group") == group).count();
        col = col.child(part(root, group, &shown[from..to], &cols, cx));
        from = to;
    }
    col
}

/// One group's part of the list.
fn part(root: &Root, group: &str, members: &[&Value], cols: &Cols, cx: &mut Context<Root>) -> Div {
    let shut = root.acc_fold.contains(group);
    let ids: Vec<String> = members.iter().map(|a| id_of(a)).collect();
    let all_on = ids.iter().all(|i| root.selected.contains(i));
    let name = if group.is_empty() { root.tr("grp.none") } else { group.to_string() };
    let key = group.to_string();
    let words: SharedString = crate::pages::lock::plain(root.tr(if all_on { "acc.selectNone" } else { "acc.selectAll" })).into();
    let head = div()
        .w_full()
        .h(px(32.))
        .flex()
        .items_center()
        .gap(px(8.))
        .child(
            div()
                .id(SharedString::from(format!("grp-head-{group}")))
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .items_center()
                .gap(px(8.))
                .cursor_pointer()
                .child(icon(if shut { "chevronRight" } else { "chevronDown" }, 14., c(MUTED)))
                .child(div().min_w_0().truncate().text_size(px(T_SEC)).font_weight(FontWeight::SEMIBOLD).text_color(c(if group.is_empty() { TEXT_2 } else { TEXT })).child(name))
                .child(div().flex_none().text_size(px(T_SEC)).text_color(c(MUTED)).font_features(tnum()).child(members.len().to_string()))
                .on_click(cx.listener(move |r, _, _, cx| {
                    super::groups::fold(r, &key);
                    cx.notify();
                })),
        )
        .child(tool(SharedString::from(format!("grp-pick-{group}")), Some(if all_on { "x" } else { "check" }), false).child(words).on_click(cx.listener(move |r, _, _, cx| {
            if all_on {
                for id in &ids {
                    r.selected.remove(id);
                }
            } else {
                r.selected.extend(ids.iter().cloned());
            }
            cx.notify();
        })));
    let col = div().w_full().flex().flex_col().gap(px(6.)).child(head);
    if shut {
        return col;
    }
    // (A long part draws only its lines near the view, under an id of its own.)
    let long = members.len() >= crate::near::FROM;
    col.child(block(root, if long { crate::near::named("acc-rows:", group) } else { "acc-rows" }, members, cols, cx))
}

/// The lines of `shown` in one box.
fn block(root: &Root, id: &'static str, shown: &[&Value], cols: &Cols, cx: &mut Context<Root>) -> Div {
    // One surface, a line an account with a hairline between (a card a line read as a stack of buttons), each line
    // as tall, its columns as wide in every line.
    // A long list: only the lines near the view are made (crate::near), each line and its hairline 57 px.
    let n = shown.len();
    let must = super::revealed_in(root, shown);
    let near = crate::near::Near::new(n >= crate::near::FROM, id, n, ROW_H, 1., must);
    let mut list = crate::kit::block_of(R_L + 2., c(RAISED)).w_full().p(px(4.)).flex().flex_col();
    list = list.children(near.mark());
    for part in near.parts {
        match part {
            crate::near::Part::Space(h) => list = list.child(crate::near::space(h)),
            crate::near::Part::Rows(r) => {
                for i in r.clone() {
                    list = list.child(row(root, shown[i], cols, cx));
                    // (A hairline between lines; none under a part's last: the room after it counts it.)
                    if i + 1 < r.end {
                        list = list.child(div().h(px(1.)).mx(px(12.)).flex_none().bg(c(LINE)));
                    }
                }
            }
        }
    }
    list
}

/// A line's height.
const ROW_H: f32 = 56.;
/// The frame-rate column's width ("144 FPS" fits).
const FPS_W: f32 = 64.;

fn row(root: &Root, a: &Value, cols: &Cols, cx: &mut Context<Root>) -> impl IntoElement {
    let id = id_of(a);
    let on = root.selected.contains(&id);
    let local = is_local(a);
    let (line, tone) = line_of(root, a);
    // A ban or a lost sign-in is the red pill, in the line's place (its words would only say it again).
    let pill = !local && !launching(a) && (banned(a) || signed_out(a)) && tone == BAD;
    let up = (local && tone == OK).then(|| up_of(a)).flatten();
    let robux = robux_of(a).map(|r| robux_el(root, &id, r));
    let group = s(a, "group").to_string();
    let compact_group = !cols.sectioned && cols.group == 0. && !group.is_empty();
    let ram = local.then(|| ram_mb(root, a)).flatten().map(|m| mb_text(m));
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
        .child(super::spot(root, &id, R_M))
        .h(px(ROW_H))
        .pl(px(14.))
        .pr(px(6.))
        .flex()
        .items_center()
        .gap(px(cols.gap))
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
                toggle(r, &pick, e.modifiers().shift, cx);
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
                .on_click(cx.listener(move |r, e: &ClickEvent, _, cx| {
                    toggle(r, &tick_id, e.modifiers().shift, cx);
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
                .when(compact_group, |d| d.flex_col().gap(px(2.)))
                .when(!compact_group, |d| d.items_baseline().gap(px(8.)))
                // The name keeps its room and the @username gives way first: both shrinking, a long username cut the
                // name to one letter ("C…" on the owner's real list, 2026-11-12).
                .child(div().flex_shrink_0().max_w(px(cols.name)).truncate().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).child(label(a)))
                .when(!compact_group && !s(a, "username").is_empty(), |d| d.child(div().min_w_0().flex_shrink(1.).truncate().text_size(px(T_S)).text_color(c(MUTED)).child(format!("@{}", s(a, "username")))))
                .when(compact_group, |d| d.child(div().flex().min_w_0().items_center().gap(px(5.))
                    .child(div().flex_none().child(icon("users", 11., c(MUTED))))
                    .child(div().min_w_0().truncate().text_size(px(T_S)).text_color(c(TEXT_2)).child(group.clone())))),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .gap(px(8.))
                .text_size(px(T_M))
                // (Every line's dot and words start at the same place: nothing stands before them but the red pill
                // that takes their place; the tags come after the time.)
                .when(pill, |d| d.child(badge(line.clone(), Tone::Bad)))
                // The state's dot and words (words in the state's colour only when it isn't fine), then the time.
                .when(!pill, |d| {
                    d.child(div().size(px(7.)).flex_none().rounded_full().bg(c(if tone == MUTED { DIM } else { tone })))
                        .child(div().min_w_0().flex_shrink(1.).truncate().text_color(c(if tone == OK || tone == ONLINE { TEXT_2 } else { tone })).child(line))
                })
                .when_some(up, |d, t| d.child(div().flex_none().text_color(c(MUTED)).font_features(tnum()).child(t)))
                .when(old_build(a), |d| d.child(old_tag(root, a, cx)))
                .when(a.get("isMain").and_then(Value::as_bool).unwrap_or(false), |d| d.child(tag(root.tr("card.main"), TEXT_2)))
                .when(s(a, "realm") == "vng", |d| d.child(tag("VNG", ONLINE))),
        )
        // (The Robux mark at the column's left in every line, the number after it: right-aligned, the mark moved
        // with the number's length.)
        .when(cols.robux, |d| d.child(div().w(px(92.)).flex_none().flex().children(robux)))
        .when(cols.group > 0., |d| {
            d.child(div().w(px(cols.group)).flex_none().flex().min_w_0().when(!group.is_empty(), |d| d.child(tag("", TEXT_2).flex_shrink(1.).min_w_0().max_w_full().child(div().min_w_0().truncate().child(group.clone())))))
        })
        .when(cols.rejoin, |d| {
            let tip: SharedString = root.tr("card.rejoinTip").into();
            d.child(div().id(SharedString::from(format!("row-rj-{id}"))).w(px(16.)).flex_none().when(rejoin_on(a), |d| {
                d.child(icon("repeat", 14., c(ONLINE))).tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
            }))
        })
        .when(cols.fps, |d| {
            let fps = local.then(|| super::fps_limit(a)).flatten();
            let tip: SharedString = root.tr("card.fpsTip").into();
            d.child(div().id(SharedString::from(format!("row-fps-{id}"))).w(px(FPS_W)).flex_none().flex().justify_end().when_some(fps, |d, f| {
                d.child(tag(format!("{f} FPS"), TEXT_2)).tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
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

/// One account in full beside the list (its detail, the line clicked last): the face and name large, its number and
/// tags, what it does, the game it is in, its facts a line each (play time, RAM, PID, last opened, today, note),
/// today's strip, then its main button wide and Edit. Flat, one surface like the list's.
pub fn detail(root: &Root, a: &Value, cx: &mut Context<Root>) -> Div {
    want_strips(root, cx);
    let id = id_of(a);
    let local = is_local(a);
    let (line, tone) = line_of(root, a);
    let line_said = line.clone();
    let robux = robux_of(a).map(|r| robux_el(root, &id, r));
    let game = game_name(a);
    let pic = game_icon(a).and_then(|u| crate::images::pic_sq(&u, 40., cx));
    let up = up_of(a);
    let ram = local.then(|| ram_mb(root, a)).flatten().map(|m| mb_text(m));
    let pid = local.then(|| pid_el(root, a, cx)).flatten();
    let opened = opened_of(root, a);
    let today = day_line(root, &id);
    let note = note_of(a);
    let main = action(root, a, "detail", cx);
    let fact = |mark: &'static str, text: String| small_line(mark, text).h(px(22.)).text_size(px(T_M)).text_color(c(TEXT_2));
    let hair = || div().h(px(1.)).flex_none().bg(c(LINE));
    crate::kit::block_of(R_L + 2., c(RAISED))
        .w(px(340.))
        .flex_none()
        .p(px(18.))
        .flex()
        .flex_col()
        .gap(px(14.))
        // Who: the face large, the number and name, @username, Robux.
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(14.))
                .child(avatar(a, 56., RAISED, cx))
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
                                .items_center()
                                .gap(px(8.))
                                .child(div().min_w_0().truncate().text_size(px(T_SEC)).font_weight(FontWeight::SEMIBOLD).child(label(a))),
                        )
                        .children(handle_of(a).map(|h| div().min_w_0().truncate().text_size(px(T_S)).text_color(c(MUTED)).child(h))),
                ),
        )
        .child(div().flex().flex_wrap().items_center().gap(px(6.)).children(tags_of(root, a)).child(div().flex_1()).children(robux))
        // What it does now, and the game it is in.
        .child(hair())
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .text_size(px(T_M))
                .child(div().size(px(8.)).flex_none().rounded_full().bg(c(dot_of(a))))
                .child(div().min_w_0().flex_1().text_color(c(if tone == OK || tone == ONLINE || tone == MUTED { TEXT } else { tone })).child(line)),
        )
        .when_some(game, |d, g| {
            d.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(match pic {
                        Some(p) => img(p).size(px(40.)).flex_none().rounded(px(R_S)).into_any_element(),
                        None => div().size(px(40.)).flex_none().rounded(px(R_S)).bg(c(INSET)).flex().items_center().justify_center().child(icon("gamepad", 18., c(MUTED))).into_any_element(),
                    })
                    .child(div().min_w_0().flex_1().truncate().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).child(g)),
            )
        })
        // Its facts, a line each.
        .child(hair())
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .when_some(up, |d, t| d.child(fact("clock", t)))
                .when_some(ram, |d, r| d.child(fact("chip", r)))
                .when_some(pid, |d, p| d.child(div().h(px(22.)).flex().items_center().gap(px(6.)).text_size(px(T_M)).child(icon("screens", 12., c(MUTED))).child(p)))
                // (Not when the state above says it already.)
                .when(opened != line_said && opened != root.tr("card.never"), |d| d.child(last_line(root, a, opened)))
                .when_some(today, |d, t| d.child(fact("calendar", t)))
                .when_some(note, |d, n| d.child(fact("message", n))),
        )
        .child(day_strip(root, &id))
        // Its main button wide, then Edit.
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(div().flex_1().min_w_0().children(main.filter(|_| !foreign(a)).map(no_pick)))
                .child(edit_button(root, a, "detail", 34., cx)),
        )
}
