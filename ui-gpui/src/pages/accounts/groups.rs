//! Accounts' groups. The Group tool, on the toolbar and on the bar of the selected accounts, opens one box: which
//! group the page shows, and the group the selected accounts go in (one there is, a new one named there, or none).
//! A group lives on its accounts: it is made by naming it for the ones selected, and is gone with its last account.
//! (The group's box stood in ⋮ › Edit… only.)
use super::*;

/// The tool on the toolbar, and the one on the selected accounts' bar (each its own box: `Root::acc_drop`).
pub const TOOL: &str = "acc-group";
pub const BAR: &str = "bar-group";
/// A group's name at most, as a friend's label.
const NAME_MAX: usize = 24;

/// The groups the accounts are in, in the order they are listed everywhere.
/// [`names`] of the state's accounts, made again only with a new state (the button reads them on every frame the
/// page is drawn, a scroll's too).
fn names_kept(root: &Root, all: &[Value]) -> std::rc::Rc<Vec<String>> {
    thread_local! {
        static KEPT: std::cell::RefCell<Option<(u64, std::rc::Rc<Vec<String>>)>> = const { std::cell::RefCell::new(None) };
    }
    if let Some(n) = KEPT.with(|k| k.borrow().as_ref().filter(|(at, _)| *at == root.state_gen).map(|(_, n)| n.clone())) {
        return n;
    }
    let n = std::rc::Rc::new(names(all));
    KEPT.with(|k| *k.borrow_mut() = Some((root.state_gen, n.clone())));
    n
}

pub fn names(all: &[Value]) -> Vec<String> {
    let mut names: Vec<String> = all.iter().map(|a| s(a, "group").to_string()).filter(|g| !g.is_empty()).collect();
    names.sort_by_cached_key(|g| (Natural(folded(g)), g.clone()));
    names.dedup();
    names
}

/// The accounts selected.
fn picked(r: &Root) -> Vec<String> {
    r.accounts().iter().filter(|a| r.selected.contains(s(a, "id"))).map(id_of).collect()
}

/// These accounts into `group` ("": out of theirs). The group alone is written (the edit form's command writes the
/// note and the name with it).
pub fn put(r: &mut Root, ids: Vec<String>, group: &str) {
    if !ids.is_empty() {
        r.send("setGroup", json!({ "ids": ids, "group": group }));
    }
}

/// The name typed in the box, given to the selected accounts (Enter, or Add). One there is already, whatever its
/// capitals, is that one, not a second beside it.
pub fn add(r: &mut Root, cx: &mut Context<Root>) {
    let typed: String = r.group_box.read(cx).value().trim().chars().take(NAME_MAX).collect();
    let ids = picked(r);
    if typed.is_empty() || ids.is_empty() {
        return;
    }
    let name = names(r.accounts()).into_iter().find(|g| folded(g) == folded(&typed)).unwrap_or(typed);
    put(r, ids, &name);
    r.acc_drop = None;
    empty_field(&r.group_box, cx);
    cx.notify();
}

/// A part of the list folded or opened again; kept for the next start.
pub fn fold(r: &mut Root, group: &str) {
    if !r.acc_fold.remove(group) {
        r.acc_fold.insert(group.to_string());
    }
    crate::prefs::set("accFold", json!(r.acc_fold.iter().collect::<Vec<_>>()));
}

/// A click on one of the two tools: its box opened or shut (a click that just shut it from outside only shuts it),
/// the name's box ready to type in when accounts are selected.
fn flip(key: &'static str, cx: &mut Context<Root>) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    cx.listener(move |r, _, window, cx| {
        let just = r.acc_drop_closed.is_some_and(|(k, at)| k == key && at.elapsed().as_millis() < 300);
        r.acc_drop_closed = None;
        r.acc_drop = if just || r.acc_drop == Some(key) { None } else { Some(key) };
        r.acc_menu = false;
        if r.acc_drop == Some(key) && !picked(r).is_empty() {
            let h = r.group_box.read(cx).focus_handle(cx);
            window.focus(&h, cx);
        }
        cx.notify();
    })
}

/// The toolbar's tool, always there: "Group" while there is none yet, else the one shown ("Group: All").
pub fn button(root: &Root, all: &[Value], cx: &mut Context<Root>) -> Div {
    let names = names_kept(root, all);
    let now = root.acc_group.clone().filter(|g| names.contains(g));
    let open = root.acc_drop == Some(TOOL);
    let words = match now {
        // (A long name is cut: the toolbar is one line.)
        Some(g) if g.chars().count() > 16 => tool_words(Some(root.tr("grp.label")), format!("{}…", g.chars().take(15).collect::<String>())),
        Some(g) => tool_words(Some(root.tr("grp.label")), g),
        None if names.is_empty() => tool_words(None, root.tr("grp.label")),
        None => tool_words(Some(root.tr("grp.label")), root.tr("chip.all")),
    };
    let mut wrap = div().relative().flex_none().child(tool(TOOL, Some("folder"), open).child(words).child(tool_chev()).on_click(flip(TOOL, cx)));
    if pop_alive(TOOL, open) {
        wrap = wrap.child(popup(PopAt::BelowRight(TOOL_H + 4.), "acc-group-pop", open, panel(root, TOOL, true, cx)));
    }
    wrap
}

/// On the selected accounts' bar: the same box above it, without the part that says which group shows.
pub fn bar_button(root: &Root, compact: bool, cx: &mut Context<Root>) -> Div {
    let open = root.acc_drop == Some(BAR);
    let trigger = if compact {
        IconButton::new("bar-group-btn", "folder").size(32.).selected(open).tip(root.tr("grp.tip")).on_click(flip(BAR, cx)).into_any_element()
    } else {
        Button::new("bar-group-btn", root.tr("grp.label")).icon("folder").quiet().on_click(flip(BAR, cx)).into_any_element()
    };
    let mut wrap = div().relative().flex_none().child(trigger);
    if pop_alive(BAR, open) {
        wrap = wrap.child(popup(PopAt::AboveRight(38.), "bar-group-pop", open, panel(root, BAR, false, cx)));
    }
    wrap
}

/// The box: which group shows (`filter`: the toolbar's), then where the selected accounts go, a chip a group there is
/// (lit when they are all in it) and a new one's name; with none selected, how a group is made.
fn panel(root: &Root, key: &'static str, filter: bool, cx: &mut Context<Root>) -> Stateful<Div> {
    let all = root.accounts();
    let names = names(all);
    let ids = picked(root);
    let n = ids.len();
    let now = root.acc_group.clone().filter(|g| names.contains(g));
    let typed = !root.group_box.read(cx).value().trim().is_empty();
    let head = |text: String| div().px(px(6.)).pt(px(4.)).pb(px(4.)).text_size(px(T_XS)).font_weight(FontWeight::SEMIBOLD).text_color(c(MUTED)).child(text);
    let mut b = div()
        .id(SharedString::from(format!("{key}-box")))
        .occlude()
        .w(px(300.))
        .p(px(8.))
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
    if filter && !names.is_empty() {
        b = b.child(head(root.tr("grp.show")));
        // (Many groups: the list scrolls, the part under it stays.)
        let mut rows = div().id(SharedString::from(format!("{key}-list"))).max_h(px(232.)).overflow_y_scroll().flex().flex_col();
        for (i, g) in std::iter::once(None).chain(names.iter().cloned().map(Some)).enumerate() {
            let on = g == now;
            let count = match &g {
                None => all.len(),
                Some(g) => all.iter().filter(|a| s(a, "group") == g).count(),
            };
            let text = g.clone().unwrap_or_else(|| root.tr("chip.all"));
            rows = rows.child(
                menu_row((key, i))
                    .flex_none()
                    .text_color(if on { c(TEXT) } else { c(TEXT_2) })
                    .child(div().min_w_0().flex_1().truncate().child(text))
                    .child(div().flex_none().text_size(px(T_S)).text_color(c(MUTED)).font_features(tnum()).child(count.to_string()))
                    .child(div().w(px(14.)).flex_none().when(on, |d| d.child(icon("check", 14., c(TEXT)))))
                    .on_click(cx.listener(move |r, _, _, cx| {
                        r.acc_group = g.clone();
                        r.acc_drop = None;
                        cx.notify();
                    })),
            );
        }
        b = b.child(rows).child(div().h(px(1.)).flex_none().my(px(6.)).mx(px(6.)).bg(c(LINE)));
    }
    if n == 0 {
        return b.child(head(root.tr("grp.new"))).child(
            div()
                .px(px(6.))
                .pb(px(6.))
                .flex()
                .items_start()
                .gap(px(8.))
                .child(div().pt(px(3.)).flex_none().child(icon("check", 13., c(MUTED))))
                .child(div().min_w_0().flex_1().text_size(px(T_S)).line_height(px(18.)).text_color(c(TEXT_2)).child(root.tr("grp.hint"))),
        );
    }
    b = b.child(head(root.i18n.tv("grp.move", &[("n", &n.to_string())])));
    if !names.is_empty() {
        let of: Vec<&str> = all.iter().filter(|a| ids.contains(&id_of(a))).map(|a| s(a, "group")).collect();
        let mut chips = div().px(px(6.)).pb(px(8.)).flex().flex_wrap().gap(px(6.));
        for (i, g) in names.iter().enumerate() {
            let on = of.iter().all(|x| x == g);
            let (ids, name) = (ids.clone(), g.clone());
            chips = chips.child(chip((key, 1000 + i), on).child(div().max_w(px(200.)).truncate().child(g.clone())).on_click(cx.listener(move |r, _, _, cx| {
                put(r, ids.clone(), &name);
                r.acc_drop = None;
                cx.notify();
            })));
        }
        // Out of the group they are in.
        if of.iter().any(|g| !g.is_empty()) {
            let ids = ids.clone();
            chips = chips.child(chip((key, 999usize), false).child(icon("x", 12., c(MUTED))).child(root.tr("grp.out")).on_click(cx.listener(move |r, _, _, cx| {
                put(r, ids.clone(), "");
                r.acc_drop = None;
                cx.notify();
            })));
        }
        b = b.child(chips);
    }
    b.child(
        div()
            .px(px(6.))
            .pb(px(6.))
            .flex()
            .items_center()
            .gap(px(8.))
            .child(div().flex_1().min_w_0().child(field(&root.group_box, Some("folder")).h(px(36.))))
            .child(Button::new(if key == TOOL { "acc-group-add" } else { "bar-group-add" }, root.tr("tag.add")).small().primary().disabled(!typed).on_click(cx.listener(|r, _, _, cx| add(r, cx)))),
    )
}
