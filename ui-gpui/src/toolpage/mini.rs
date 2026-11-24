// The mini window: the accounts running now, each with its state, game, RAM, Show and Close; Close all (asked first,
// as the main page asks) and Open Bloxkeep at the foot (MiniView.tsx, app.css .pn.mini, part for part). The core sends
// the list every two seconds ("mini"); rows keep their account.
use std::{
    cell::Cell,
    collections::HashMap,
    rc::Rc,
    time::{Duration, Instant},
};

use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use super::{Body, ToolPage, dot, drag_row, mb, title_icon};
use crate::{kit::*, theme::*};

/// No click this soon after a row appeared or moved: it was meant for what was there before.
const SETTLE: Duration = Duration::from_millis(500);
/// After Close, its buttons stay off this long; Roblox normally ends well within it, and the row with it.
const CLOSE_WAIT: Duration = Duration::from_secs(10);

pub struct Mini {
    data: Value,
    asking: bool,
    remember: bool,
    closing: HashMap<String, Instant>,
    /// Where each row was last drawn, and since when.
    places: HashMap<String, (usize, Instant)>,
    from: Rc<Cell<Option<Point<Pixels>>>>,
    scroll: ScrollHandle,
}

impl Mini {
    pub fn new(data: Value) -> Mini {
        let mut m = Mini { data: Value::Null, asking: false, remember: false, closing: HashMap::new(), places: HashMap::new(), from: Rc::new(Cell::new(None)), scroll: ScrollHandle::new() };
        m.set(data);
        m
    }

    pub fn set(&mut self, data: Value) {
        self.data = data;
        let now = Instant::now();
        let ids: Vec<String> = self.clients().iter().map(|c| s(c, "id")).collect();
        for (i, id) in ids.iter().enumerate() {
            if self.places.get(id).is_none_or(|(at, _)| *at != i) {
                self.places.insert(id.clone(), (i, now));
            }
        }
        self.places.retain(|id, _| ids.contains(id));
        self.closing.retain(|id, at| at.elapsed() < CLOSE_WAIT && ids.contains(id));
    }

    fn clients(&self) -> Vec<Value> {
        self.data.get("clients").and_then(Value::as_array).cloned().unwrap_or_default()
    }
    fn flag(&self, k: &str) -> bool {
        self.data.get(k).and_then(Value::as_bool).unwrap_or(false)
    }
    fn settled(&self, id: &str) -> bool {
        self.places.get(id).is_none_or(|(_, since)| since.elapsed() >= SETTLE)
    }
    fn can_close_all(&self) -> bool {
        self.flag("unlocked") && (!self.clients().is_empty() || self.data.get("robloxCount").and_then(Value::as_i64).unwrap_or(0) > 0)
    }
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn mini(t: &mut ToolPage) -> Option<&mut Mini> {
    match &mut t.body {
        Body::Mini(m) => Some(m),
        _ => None,
    }
}

/// Esc hides the window, or first takes back the question.
pub fn key(t: &mut ToolPage, key: &str, cx: &mut Context<ToolPage>) -> bool {
    if key != "escape" {
        return false;
    }
    let link = t.link;
    if let Some(m) = mini(t) {
        if m.asking {
            m.asking = false;
            cx.notify();
        } else {
            link.act("hide", json!({}));
        }
    }
    true
}

fn close_all(t: &mut ToolPage, cx: &mut Context<ToolPage>) {
    let link = t.link;
    let Some(m) = mini(t) else { return };
    if !m.can_close_all() {
        return;
    }
    if m.flag("askCloseAll") {
        m.remember = false;
        m.asking = true;
        cx.notify();
    } else {
        link.act("closeAll", json!({}));
    }
}

pub fn render(t: &ToolPage, _window: &mut Window, cx: &mut Context<ToolPage>) -> AnyElement {
    let Body::Mini(m) = &t.body else { return div().into_any_element() };
    let unlocked = m.flag("unlocked");
    let clients = m.clients();
    let n = clients.len() as i64;
    let link = t.link;

    let title = drag_row(div().id("mini-title"), link, m.from.clone())
        .h(px(36.))
        .flex_none()
        .flex()
        .items_center()
        .gap(px(8.))
        .pl(px(12.))
        .pr(px(4.))
        .border_b_1()
        .border_color(c(LINE))
        .child(div().size(px(18.)).flex_none().child(crate::crisp::mark(18.)))
        .child(div().flex_1().min_w_0().truncate().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).child("Bloxkeep"))
        .when(unlocked && n > 0, |d| {
            d.child(
                div()
                    .min_w(px(20.))
                    .h(px(20.))
                    .px(px(6.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(c(RAISED_2))
                    .text_size(px(T_XS))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(c(TEXT_2))
                    .child(n.to_string()),
            )
        })
        .child(title_icon("mini-x", "x", t.t("mini.hide")).on_click(move |_, _, _| link.act("hide", json!({}))));

    let empty = |icon_name: &'static str, words: String, more: Option<String>| {
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(6.))
            .px(px(20.))
            .py(px(32.))
            .text_center()
            .text_color(c(TEXT_2))
            .child(icon(icon_name, 22., c(MUTED)))
            .child(div().text_size(px(T_M)).child(words))
            .children(more.map(|m| div().text_size(px(T_S)).text_color(c(MUTED)).child(m)))
    };
    let mut list = div().id("mini-list").flex_1().min_h_0().overflow_y_scroll().track_scroll(&m.scroll).py(px(4.)).flex().flex_col();
    if !unlocked {
        list = list.child(empty("lock", t.t("mini.locked"), None));
    } else if n == 0 {
        let count = m.data.get("robloxCount").and_then(Value::as_i64).unwrap_or(0);
        list = list.child(empty("screens", t.t("mini.none"), (count > 0).then(|| t.tn("tray.openCount", count))));
    } else {
        for (i, cl) in clients.iter().enumerate() {
            let id = s(cl, "id");
            let state = s(cl, "state");
            let off = m.closing.contains_key(&id);
            let (tone, words) = match state.as_str() {
                "game" => (c(OK), "mini.inGame"),
                "menu" => (c(ONLINE), "mini.inMenu"),
                _ => (c(BAD), "mini.dropped"),
            };
            let ram_mb = cl.get("ramMb").and_then(Value::as_i64).unwrap_or(0);
            let (show_id, close_id) = (id.clone(), id.clone());
            let tip: SharedString = format!("PID {} · {}", cl.get("pid").and_then(Value::as_i64).unwrap_or(0), t.t(words)).into();
            list = list.child(
                div()
                    .id(("mini-row", i))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .min_h(px(52.))
                    .pl(px(12.))
                    .pr(px(10.))
                    .py(px(6.))
                    .when(i > 0, |d| d.border_t_1().border_color(c(LINE)))
                    .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
                    .child(div().self_start().mt(px(6.)).child(dot(tone)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(div().truncate().font_weight(FontWeight::SEMIBOLD).child(s(cl, "name")))
                            .child(
                                div()
                                    .flex()
                                    .min_w_0()
                                    .text_size(px(T_S))
                                    .text_color(c(TEXT_2))
                                    .child(div().min_w_0().truncate().child(s(cl, "game")))
                                    .when(ram_mb > 0, |d| d.child(div().flex_none().px(px(4.)).text_color(c(MUTED)).child("·")).child(div().flex_none().child(mb(ram_mb)))),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .gap(px(4.))
                            .child(Button::new(("mini-show", i), t.t("tray.show")).small().disabled(off).on_click(cx.listener(move |t, _, _, _| {
                                let link = t.link;
                                if mini(t).is_some_and(|m| m.settled(&show_id)) {
                                    link.act("show", json!({ "id": show_id }));
                                }
                            })))
                            .child(Button::new(("mini-close", i), t.t(if off { "mini.closing" } else { "tray.close" })).small().quiet().disabled(off).on_click(cx.listener(move |t, _, _, cx| {
                                let link = t.link;
                                let Some(m) = mini(t) else { return };
                                if !m.settled(&close_id) || m.closing.contains_key(&close_id) {
                                    return;
                                }
                                m.closing.insert(close_id.clone(), Instant::now());
                                link.act("close", json!({ "id": close_id }));
                                cx.notify();
                                // A close that didn't take (Roblox asked something, or hung): its buttons come back.
                                cx.spawn(async move |this, cx| {
                                    cx.background_executor().timer(CLOSE_WAIT).await;
                                    let _ = this.update(cx, |t, cx| {
                                        if let Some(m) = mini(t) {
                                            m.closing.retain(|_, at| at.elapsed() < CLOSE_WAIT);
                                        }
                                        cx.notify();
                                    });
                                })
                                .detach();
                            }))),
                    ),
            );
        }
    }

    let can = m.can_close_all();
    let foot = div()
        .flex()
        .flex_none()
        .items_center()
        .justify_between()
        .gap(px(8.))
        .px(px(12.))
        .pt(px(8.))
        .pb(px(10.))
        .border_t_1()
        .border_color(c(LINE))
        .child(
            Button::new("mini-close-all", t.t("sys.closeAll"))
                .small()
                .quiet()
                .danger()
                .tip(if can { String::new() } else { t.t(if unlocked { "mini.none" } else { "mini.locked" }) })
                .on_click(cx.listener(|t, _, _, cx| close_all(t, cx))),
        )
        .child(Button::new("mini-open", t.t("tray.open")).small().on_click(move |_, _, _| link.act("open", json!({}))));

    let grip = div()
        .id("mini-grip")
        .absolute()
        .left_0()
        .right_0()
        .bottom_0()
        .h(px(6.))
        .flex()
        .items_center()
        .justify_center()
        .cursor(CursorStyle::ResizeUpDown)
        .group("mini-grip")
        .child(div().w(px(28.)).h(px(2.)).rounded(px(1.)).bg(c(LINE)).group_hover("mini-grip", |d| d.bg(c(TEXT_2))))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            link.resize();
        });

    let ask = m.asking.then(|| {
        let remember = m.remember;
        div()
            .id("mini-ask")
            .absolute()
            .left(px(8.))
            .right(px(8.))
            .bottom(px(8.))
            .occlude()
            .flex()
            .flex_col()
            .gap(px(12.))
            .p(px(14.))
            .rounded(px(R_L))
            .border_1()
            .border_color(c(LINE))
            .bg(c(RAISED))
            .shadow_lg()
            .child(div().text_size(px(T_M)).line_height(px(19.)).child(t.t("t.closeAllQ")))
            .child(div().flex().child(Check::new("mini-remember", remember).label(t.t("common.dontAsk")).on_toggle(cx.listener(|t, on: &bool, _, cx| {
                if let Some(m) = mini(t) {
                    m.remember = *on;
                }
                cx.notify();
            }))))
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(Button::new("mini-ask-no", t.t("common.cancel")).small().quiet().on_click(cx.listener(|t, _, _, cx| {
                        if let Some(m) = mini(t) {
                            m.asking = false;
                        }
                        cx.notify();
                    })))
                    .child(Button::new("mini-ask-yes", t.t("t.closeAllOk")).small().danger().on_click(cx.listener(|t, _, _, cx| {
                        let link = t.link;
                        if let Some(m) = mini(t) {
                            m.asking = false;
                            link.act("closeAll", json!({ "remember": m.remember }));
                        }
                        cx.notify();
                    }))),
            )
    });

    div().size_full().relative().flex().flex_col().child(title).child(list).child(foot).child(grip).children(ask).into_any_element()
}
