// The start for new users (the web page's StartSteps and AddOptions). With no accounts: the logo, the three steps,
// the ways to add one (the phone first: Roblox asks it for no password and no puzzle) and the other ways under them.
// With accounts none of which Bloxkeep has opened yet: the steps over the cards, each ticked when done, gone for good
// once Bloxkeep has opened Roblox for an account.
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use super::{is_local, launching};
use crate::{kit::*, shell::Root, theme::*};

/// The logo at the exact pixels it takes (crisp.rs needs the window, which only a drawing gets).
#[derive(IntoElement)]
struct Logo(f32);

impl RenderOnce for Logo {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        img(crate::crisp::logo(self.0, window)).size(px(self.0)).flex_none()
    }
}

/// Getting started: add an account, pick a game, select accounts then Play. None once any account has been opened
/// (its "last opened" is set) or is open. `boxed`: in its own box over the cards, with its title.
pub fn steps(root: &Root, boxed: bool) -> Option<Div> {
    let all = root.accounts();
    if all.iter().any(|a| a.get("lastUsed").is_some_and(|v| !v.is_null() && v.as_str() != Some("")) || is_local(a) || launching(a)) {
        return None;
    }
    // (The game box only counts once there are accounts: a game it holds from before isn't a step done yet.)
    let game = root.game.as_ref().is_some_and(|g| g.get("ok").and_then(Value::as_bool) == Some(true));
    let done = [!all.is_empty(), !all.is_empty() && game, false];
    let now = done.iter().position(|d| !d).unwrap_or(2);
    let mut line = div().flex().flex_wrap().items_center().gap(px(10.));
    for (i, key) in ["guide.add", "guide.game", "guide.play"].into_iter().enumerate() {
        if i > 0 {
            line = line.child(div().w(px(18.)).h(px(2.)).rounded_full().bg(if done[i - 1] { ca(OK, 0.5) } else { c(DIM) }));
        }
        let (is_done, is_now) = (done[i], i == now);
        // Each step a small block: green once done, solid yellow (on its darker base) for the one to do now.
        let mark = div()
            .size(px(24.))
            .flex_none()
            .rounded(px(7.))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(T_S))
            .font_weight(FontWeight::BOLD)
            .map(|d| {
                if is_done {
                    d.bg(c(OK)).child(icon("check", 14., c(INK)))
                } else if is_now {
                    d.bg(c(FACE)).text_color(c(INK)).child((i + 1).to_string())
                } else {
                    d.bg(c(INSET)).border_1().border_color(c(DIM)).text_color(c(MUTED)).child((i + 1).to_string())
                }
            });
        line = line.child(
            div()
                .flex()
                .items_center()
                .gap(px(9.))
                .child(div().flex_none().child(mark))
                .child(
                    div()
                        .text_size(px(T_M))
                        .map(|d| if is_done { d.text_color(c(MUTED)).line_through() } else if is_now { d.text_color(c(TEXT)).font_weight(FontWeight::SEMIBOLD) } else { d.text_color(c(MUTED)) })
                        .child(root.tr(key)),
                ),
        );
    }
    Some(if boxed {
        // A block of its own over the cards, on a dark base like theirs.
        div().relative().pb(px(4.)).child(div().absolute().left_0().right_0().top(px(5.)).bottom(px(0.)).rounded(px(R_L + 1.)).bg(base_dark())).child(
            div()
                .flex()
                .items_center()
                .gap(px(22.))
                .px(px(18.))
                .py(px(14.))
                .rounded(px(R_L))
                .border_1()
                .border_color(c(LINE))
                .bg(c(RAISED))
                .child(div().flex_none().text_size(px(T_H)).font_weight(FontWeight::BOLD).child(root.tr("guide.title")))
                .child(line),
        )
    } else {
        line
    })
}

/// No accounts yet: a title, the steps, the three ways to add the first one side by side, the other ways in a line.
/// Laid out to fit the smallest window (860×600) without scrolling, and centred in the page when there is room
/// (owner 2026-11-12: the stacked rows overflowed a 1280×800 window and showed a scroll bar).
pub fn empty(root: &Root, cx: &mut Context<Root>) -> impl IntoElement {
    let way = |key: &'static str, mark: &'static str, title: String, sub: String, rec: Option<String>, cx: &mut Context<Root>| {
        // A flat card: the way we'd pick first has the yellow ring and a yellow icon block.
        let first = rec.is_some();
        hot(key)
            .flex_1()
            .min_w_0()
            .min_h(px(150.))
            .p(px(18.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .rounded(px(R_L))
            .border_1()
            .cursor_pointer()
            .map(|d| if first { d.tones(tinted(RAISED, FACE, 0.07), tinted(RAISED_2, FACE, 0.10)).edge(ca(FACE, 0.75), c(FACE)) } else { d.tones(c(RAISED), c(RAISED_2)).edge(c(LINE), ca(WHITE, 0.17)) })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .size(px(40.))
                            .flex_none()
                            .rounded(px(R_M))
                            .flex()
                            .items_center()
                            .justify_center()
                            .map(|d| if first { d.bg(c(FACE)).child(icon(mark, 20., c(INK))) } else { d.bg(c(INSET)).child(icon(mark, 20., c(TEXT_2))) }),
                    )
                    .when_some(rec, |d, r| d.child(div().h(px(20.)).px(px(8.)).flex().items_center().rounded_full().bg(ca(OK, 0.16)).text_color(c(OK)).text_size(px(T_XS)).font_weight(FontWeight::SEMIBOLD).child(r))),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(div().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(title))
                    .child(div().text_size(px(T_S)).line_height(px(18.)).text_color(c(MUTED)).child(sub)),
            )
            .on_click(cx.listener(move |r, _, window, cx| {
                match key {
                    "start-phone" => crate::pages::quick::open(r, false, window, cx),
                    "start-pw" => crate::pages::password::open(r, window, cx),
                    _ => r.add_browser(cx),
                }
                cx.notify();
            }))
    };
    let other = |key: &'static str, text: String, cx: &mut Context<Root>| {
        Button::new(key, text).quiet().small().on_click(cx.listener(move |r, _, window, cx| {
            match key {
                "start-create" => r.add_browser_as(true, cx),
                "start-cookie" => crate::pages::account_form::open(r, None, window, cx),
                "start-vault" => crate::pages::import_vault::open(r, window, cx),
                _ => r.send("openWebsiteBlank", json!({})),
            }
            cx.notify();
        }))
    };
    div().flex_1().min_h_0().child(crate::scrolled::body(
        "acc-start",
        div()
            .w_full()
            .min_h(relative(1.))
            .px(px(PAD))
            .py(px(24.))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(760.))
                    .max_w_full()
                    .flex()
                    .flex_col()
                    .gap(px(22.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(16.))
                            .child(Logo(48.))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.))
                                    .child(div().text_size(px(T_FIG)).font_weight(FontWeight::BOLD).child(root.tr("empty.title")))
                                    .child(div().text_size(px(T_M)).text_color(c(TEXT_2)).child(root.tr("empty.text"))),
                            ),
                    )
                    .children(steps(root, false))
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .gap(px(12.))
                            .child(way("start-phone", "qr", root.tr("start.phone"), root.tr("start.phoneSub"), Some(root.tr("start.rec")), cx))
                            .child(way("start-pw", "key", root.tr("start.pw"), root.tr("start.pwSub"), None, cx))
                            .child(way("start-browser", "globe", root.tr("start.browser"), root.tr("start.browserSub"), None, cx)),
                    )
                    .child(
                        div()
                            .pt(px(14.))
                            .border_t_1()
                            .border_color(c(LINE))
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap(px(4.))
                            .child(div().mr(px(6.)).text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("start.more").trim_end_matches([':', '：']).to_string()))
                            .child(other("start-create", root.tr("pw.create"), cx))
                            .child(other("start-cookie", root.tr("start.cookie"), cx))
                            .child(other("start-vault", root.tr("start.vault"), cx))
                            .child(other("start-web", root.tr("start.web"), cx)),
                    ),
            ),
    ))
}
