// The selection bar's Presets menu (the web dock's, app.js presetMenu): each preset to run, then Save as preset… (a new
// preset with the picked accounts, in their launch order, and the game in the bar's box, named after that game) and
// Edit presets…. The form is Automation's (pages/preset_form.rs); the commands are the app's (runPreset, savePreset).
use std::time::Instant;

use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use crate::{
    images,
    kit::*,
    pages::{Page, auto::Tab},
    shell::Root,
    theme::*,
};

/// The app keeps this many presets at most (PresetsView's limit).
const MAX_PRESETS: usize = 50;

/// The menu: open, still built while it fades out, and closed by a click outside just now (that click on its own
/// button must not open it again).
#[derive(Default)]
pub struct Data {
    pub open: bool,
    was: bool,
    closed: Option<Instant>,
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or_default()
}

fn presets(root: &Root) -> Vec<Value> {
    root.state.pointer("/settings/presets").and_then(Value::as_array).cloned().unwrap_or_default()
}

/// How many of a preset's accounts are still there (one deleted since is left out).
pub fn accounts_of(root: &Root, p: &Value) -> usize {
    let ids: Vec<&str> = p.get("accountIds").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    root.accounts().iter().filter(|a| ids.contains(&s(a, "id"))).count()
}

pub fn acc_count(root: &Root, n: usize) -> String {
    if n == 1 { root.tr("ps.acc1") } else { root.i18n.tv("ps.accN", &[("n", &n.to_string())]) }
}

/// Save as preset…: the preset form, new, with the picked accounts and the bar's game (app.js newPreset).
pub fn save_as(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    r.preset_menu.open = false;
    if presets(r).len() >= MAX_PRESETS {
        r.toast = Some((r.i18n.tv("pv.full", &[("n", &MAX_PRESETS.to_string())]), "warn".into()));
        cx.notify();
        return;
    }
    r.auto.tab = Tab::Presets;
    r.auto.confirm = None;
    r.go(Page::Auto);
    crate::pages::preset_form::open(r, None, window, cx);
    cx.notify();
}

/// The bar's Presets button and its menu, opening upwards. `labelled`: its word beside its icon (where there is room).
pub fn button(root: &Root, labelled: bool, cx: &mut Context<Root>) -> Div {
    let open = root.preset_menu.open;
    let toggle = cx.listener(|r: &mut Root, _: &ClickEvent, _, cx| {
        let just = r.preset_menu.closed.take().is_some_and(|at| at.elapsed().as_millis() < 300);
        r.preset_menu.open = !just && !r.preset_menu.open;
        r.preset_menu.was = true;
        r.acc_menu = false;
        cx.notify();
    });
    let trigger = if labelled {
        Button::new("bar-presets", root.tr("ps.listTitle")).icon("layers").icon_end(if open { "chevronDown" } else { "chevronUp" }).quiet().on_click(toggle).into_any_element()
    } else {
        IconButton::new("bar-presets", "layers").size(32.).selected(open).tip(root.tr("ps.listTitle")).on_click(toggle).into_any_element()
    };
    let mut wrap = div().relative().flex_none().child(trigger);
    if open || root.preset_menu.was {
        wrap = wrap.child(div().absolute().bottom(px(38.)).right_0().child(deferred(anchored().anchor(Anchor::BottomRight).snap_to_window_with_margin(px(8.)).child(pop("bar-presets-pop", open, menu(root, cx)))).with_priority(1)));
    }
    wrap
}

fn menu(root: &Root, cx: &mut Context<Root>) -> Stateful<Div> {
    let list = presets(root);
    let launching = root.state.get("launching").and_then(Value::as_bool).unwrap_or(false);
    // (A long list scrolls while Save and Edit stay at its foot.)
    let mut rows = div().id("bar-presets-list").max_h(px(320.)).overflow_y_scroll().flex().flex_col();
    if list.is_empty() {
        rows = rows.child(menu_row("bp-none").h(px(36.)).opacity(0.38).cursor_default().child(icon("layers", 15., c(TEXT_2))).child(div().flex_1().child(root.tr("ps.none"))));
    }
    for (i, p) in list.iter().enumerate() {
        let n = accounts_of(root, p);
        let id = s(p, "id").to_string();
        let scheduled = p.pointer("/schedule/enabled").and_then(Value::as_bool).unwrap_or(false);
        let off = n == 0 || launching;
        // Its game's picture leads (a plain block without one); a clock says it has a schedule.
        let pic = Some(s(p, "iconUrl")).filter(|u| !u.is_empty()).and_then(|u| images::pic_sq(u, 26., cx));
        let lead = div()
            .size(px(26.))
            .flex_none()
            .rounded(px(7.))
            .overflow_hidden()
            .bg(c(RAISED_2))
            .flex()
            .items_center()
            .justify_center()
            .map(|d| match pic {
                Some(p) => d.child(img(p).size_full().object_fit(ObjectFit::Cover)),
                None => d.child(icon("play", 13., c(TEXT_2))),
            });
        let mut row = menu_row(("bp-run", i))
            .h(px(40.))
            .child(lead)
            .child(div().flex_1().min_w_0().truncate().text_color(c(TEXT)).child(s(p, "name").to_string()))
            .when(scheduled, |d| d.child(icon("clock", 13., c(MUTED))))
            .child(div().flex_none().text_size(px(T_S)).text_color(c(MUTED)).font_features(tnum()).child(acc_count(root, n)));
        row = if off {
            row.opacity(0.38).cursor_default()
        } else {
            row.on_click(cx.listener(move |r, _, _, cx| {
                r.preset_menu.open = false;
                r.launch("runPreset", json!({ "id": id }), false, cx);
                cx.notify();
            }))
        };
        rows = rows.child(row);
    }
    div()
        .id("bar-presets-menu")
        .occlude()
        .w(px(320.))
        .p(px(6.))
        .rounded(px(R_L))
        .bg(c(RAISED))
        .border_1()
        .border_color(ca(WHITE, 0.1))
        .flex()
        .flex_col()
        .on_mouse_down_out(cx.listener(|r, _, _, cx| {
            if r.preset_menu.open {
                r.preset_menu.open = false;
                r.preset_menu.closed = Some(Instant::now());
                cx.notify();
            }
        }))
        .child(div().px(px(10.)).pt(px(6.)).pb(px(6.)).flex().items_center().gap(px(6.)).text_size(px(T_S)).font_weight(FontWeight::SEMIBOLD).text_color(c(MUTED)).child(root.tr("ps.listTitle")).child(div().font_features(tnum()).text_color(c(DIM)).child(list.len().to_string())))
        .child(rows)
        .child(div().h(px(1.)).my(px(6.)).mx(px(4.)).bg(c(LINE)))
        .child(menu_row("bp-save").h(px(36.)).child(icon("plus", 15., c(TEXT_2))).child(div().flex_1().min_w_0().truncate().child(root.tr("ps.save"))).on_click(cx.listener(|r, _, window, cx| save_as(r, window, cx))))
        .child(menu_row("bp-manage").h(px(36.)).child(icon("edit", 15., c(TEXT_2))).child(div().flex_1().min_w_0().truncate().child(root.tr("ps.manage"))).on_click(cx.listener(|r, _, _, cx| {
            r.preset_menu.open = false;
            r.auto.tab = Tab::Presets;
            r.go(Page::Auto);
            cx.notify();
        })))
}

/// A shot opens Accounts with the Presets menu open (BLOXKEEP_SHOT_PICK picks the accounts).
pub fn shot(r: &mut Root, cx: &mut Context<Root>) {
    r.go(Page::Accounts);
    r.preset_menu.open = true;
    r.preset_menu.was = true;
    cx.notify();
}
