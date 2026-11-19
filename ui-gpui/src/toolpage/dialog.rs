// The app's message box: the question, its buttons side by side (the main one right) or stacked as choices with a line
// under each, "Don't ask again" when offered, and a footnote (DialogView.tsx, app.css .modal.dlg, part for part). Enter
// presses the default button, Esc and ✕ cancel; the core keeps the answer. Its window is sized to it.
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use super::{Body, ToolPage};
use crate::{kit::*, theme::*};

const WIDTH: f32 = 460.;

pub struct Dialog {
    data: Value,
    remember: bool,
}

impl Dialog {
    pub fn new(data: Value) -> Dialog {
        Dialog { data, remember: false }
    }
    fn choices(&self) -> Vec<Value> {
        self.data.get("choices").and_then(Value::as_array).cloned().unwrap_or_default()
    }
    fn def(&self) -> usize {
        let n = self.choices().len().max(1);
        (self.data.get("def").and_then(Value::as_i64).unwrap_or(0).max(0) as usize).min(n - 1)
    }
    fn text(&self, k: &str) -> Option<String> {
        self.data.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string)
    }
}

fn pick(t: &ToolPage, index: usize) {
    if let Body::Dialog(d) = &t.body {
        t.link.act("pick", json!({ "index": index, "remember": d.remember }));
    }
}

/// Enter: the default button; Esc: cancel.
pub fn key(t: &mut ToolPage, key: &str, _window: &mut Window, _cx: &mut Context<ToolPage>) -> bool {
    let Body::Dialog(d) = &t.body else { return false };
    match key {
        "escape" => {
            t.link.act("cancel", json!({}));
            true
        }
        "enter" => {
            pick(t, d.def());
            true
        }
        _ => false,
    }
}

pub fn render(t: &ToolPage, _window: &mut Window, cx: &mut Context<ToolPage>) -> AnyElement {
    let Body::Dialog(d) = &t.body else { return div().into_any_element() };
    let link = t.link;
    let choices = d.choices();
    let listed = choices.iter().any(|c| c.get("detail").and_then(Value::as_str).is_some_and(|s| !s.is_empty()));
    let any_danger = choices.iter().any(|c| c.get("danger").and_then(Value::as_bool).unwrap_or(false));
    let heading = d.text("heading");
    let footnote = d.text("footnote");
    let (mark, tone) = match d.data.get("kind").and_then(Value::as_str).unwrap_or("info") {
        "warning" => ("alert", WARN),
        "error" => ("x", BAD),
        _ => ("info", ONLINE),
    };

    // The head: dragged, the window moves (not from its ✕).
    let title = match &heading {
        Some(h) => div().min_w_0().flex_1().text_size(px(T_GROUP)).line_height(px(24.)).font_weight(FontWeight::SEMIBOLD).child(h.clone()),
        None => div()
            .min_w_0()
            .flex_1()
            .flex()
            .items_center()
            .gap(px(12.))
            .text_size(px(T_H))
            .font_weight(FontWeight::SEMIBOLD)
            .child(div().size(px(22.)).flex_none().rounded(px(5.)).overflow_hidden().child(crate::crisp::blocks(22.)))
            .child("Bloxkeep"),
    };
    let head = div()
        .id("dlg-head")
        .w_full()
        .flex()
        .items_center()
        .gap(px(12.))
        .pl(px(24.))
        .pr(px(14.))
        .pt(px(14.))
        .pb(px(11.))
        .border_b_1()
        .border_color(c(LINE))
        .on_mouse_down(MouseButton::Left, move |_, _, _| link.drag())
        .child(title)
        .child(div().on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()).child(dialog_x(t.t("common.close"), move |_, _, _| link.act("cancel", json!({})))));

    let text = d.text("text").unwrap_or_default();
    let mut body = div()
        .w_full()
        .px(px(24.))
        .pt(px(18.))
        .pb(px(if listed { 22. } else { 16. }))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_start()
                .gap(px(14.))
                .child(div().size(px(32.)).flex_none().rounded_full().bg(ca(tone, 0.16)).flex().items_center().justify_center().child(icon(mark, 18., c(tone))))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .mt(px(6.))
                        .text_size(px(T_M))
                        .line_height(px(19.5))
                        .text_color(if heading.is_some() { c(TEXT_2) } else { c(TEXT) })
                        .child(text),
                ),
        );
    if let Some(r) = d.text("remember") {
        body = body.child(div().mt(px(14.)).ml(px(46.)).flex().child(Check::new("dlg-remember", d.remember).label(r).on_toggle(cx.listener(|t, on: &bool, _, cx| {
            if let Body::Dialog(d) = &mut t.body {
                d.remember = *on;
            }
            cx.notify();
        }))));
    }
    if listed {
        let mut list = div().mt(px(16.)).ml(px(46.)).flex().flex_col().gap(px(8.));
        for (i, ch) in choices.iter().enumerate() {
            let danger = ch.get("danger").and_then(Value::as_bool).unwrap_or(false);
            let words = ch.get("text").and_then(Value::as_str).unwrap_or_default().to_string();
            let detail = ch.get("detail").and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string);
            list = list.child(
                div()
                    .id(("dlg-choice", i))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .px(px(14.))
                    .py(px(10.))
                    .rounded(px(R_M))
                    .border_1()
                    .cursor_pointer()
                    .map(|d| if danger { d.border_color(ca(BAD, 0.4)).bg(ca(BAD, 0.12)).hover(|d| d.bg(ca(BAD, 0.2))) } else { d.border_color(c(LINE)).bg(c(RAISED)).hover(|d| d.bg(c(RAISED_2))) })
                    .child(div().text_size(px(T_L)).line_height(px(19.)).font_weight(FontWeight::SEMIBOLD).text_color(if danger { mix(c(BAD), c(WHITE), 0.3) } else { c(TEXT) }).child(words))
                    .children(detail.map(|s| div().text_size(px(T_S)).line_height(px(17.)).text_color(c(TEXT_2)).child(s)))
                    .on_click(cx.listener(move |t, _, _, _| pick(t, i))),
            );
        }
        body = body.child(list);
        if let Some(n) = &footnote {
            body = body.child(div().mt(px(14.)).ml(px(46.)).text_size(px(T_S)).line_height(px(17.)).text_color(c(MUTED)).child(n.clone()));
        }
    }

    let foot = (!listed).then(|| {
        let mut f = dialog_foot();
        if let Some(n) = &footnote {
            f = f.child(div().flex_1().min_w_0().text_size(px(T_S)).line_height(px(17.)).text_color(c(MUTED)).child(n.clone()));
        }
        // The main button right, as in the app's own dialogs: the first choice last.
        for (i, ch) in choices.iter().enumerate().rev() {
            let danger = ch.get("danger").and_then(Value::as_bool).unwrap_or(false);
            let filled = if any_danger { danger } else { i == 0 };
            let b = Button::new(("dlg-btn", i), ch.get("text").and_then(Value::as_str).unwrap_or_default().to_string());
            let b = if filled && danger { b.danger() } else if filled { b.primary() } else { b };
            f = f.child(b.on_click(cx.listener(move |t, _, _, _| pick(t, i))));
        }
        f
    });

    div().w(px(WIDTH)).flex().flex_col().child(head).child(body).children(foot).into_any_element()
}
