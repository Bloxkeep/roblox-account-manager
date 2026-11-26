// The start for new users (the web page's StartSteps and AddOptions). With no accounts: the logo, the three steps,
// the ways to add one (the phone first: Roblox asks it for no password and no puzzle) and the other ways under them.
// With accounts none of which Bloxkeep has opened yet: the steps over the cards, each ticked when done, gone for good
// once Bloxkeep has opened Roblox for an account.
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use super::{is_local, launching};
use crate::{kit::*, shell::Root, theme::*};

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
                    // (No step numbers, no numbers anywhere.)
                    d.bg(c(FACE)).child(div().size(px(8.)).rounded_full().bg(c(INK)))
                } else {
                    d.bg(c(INSET)).border_1().border_color(c(DIM))
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

/// No accounts yet, in Home's welcome look (home::empty_stage): a game's picture with the title, a line and the way we
/// would pick first; the three ways to add the first account as blocks under it, each with its button; the other ways
/// in a line under them.
pub fn empty(root: &Root, width: f32, cx: &mut Context<Root>) -> impl IntoElement {
    let go = |key: &'static str, label: String, main: bool, cx: &mut Context<Root>| {
        Button::new(key, label)
            .map(|b| if main { b.primary().icon("qr") } else { b.small() })
            .on_click(cx.listener(move |r, _, window, cx| {
                match key {
                    "start-phone" | "start-phone-b" => crate::pages::quick::open(r, false, window, cx),
                    "start-pw" => crate::pages::password::open(r, window, cx),
                    _ => r.add_browser(cx),
                }
                cx.notify();
            }))
            .into_any_element()
    };
    let wide = |b: AnyElement| div().flex().flex_col().child(b).into_any_element();
    let blocks = vec![
        crate::pages::home::stage_block(
            root.tr("start.rec"),
            crate::pages::home::block_text("qr", Some(root.tr("start.phone")), root.tr("start.phoneSub"), None),
            Some(wide(go("start-phone-b", root.tr("acc.w.use"), false, cx))),
            false,
        ),
        crate::pages::home::stage_block(
            root.tr("acc.w.pwHead"),
            crate::pages::home::block_text("key", Some(root.tr("start.pw")), root.tr("start.pwSub"), None),
            Some(wide(go("start-pw", root.tr("acc.w.use"), false, cx))),
            false,
        ),
        crate::pages::home::stage_block(
            root.tr("acc.w.webHead"),
            crate::pages::home::block_text("globe", Some(root.tr("start.browser")), root.tr("start.browserSub"), None),
            Some(wide(go("start-browser", root.tr("acc.w.use"), false, cx))),
            false,
        ),
    ];
    let r_tip = (root.tr("pw.createTip"), root.tr("start.vaultTip"), root.tr("start.webTip"));
    let other = |key: &'static str, text: String, cx: &mut Context<Root>| {
        // What each other way does, under the mouse (Accounts.tsx's start screen).
        let tip = match key {
            "start-create" => r_tip.0.clone(),
            "start-vault" => r_tip.1.clone(),
            "start-web" => r_tip.2.clone(),
            _ => String::new(),
        };
        Button::new(key, text).quiet().small().tip(tip).on_click(cx.listener(move |r, _, window, cx| {
            match key {
                "start-create" => r.add_browser_as(true, cx),
                "start-cookie" => crate::pages::account_form::open(r, None, window, cx),
                "start-vault" => crate::pages::import_vault::open(r, window, cx),
                "start-ram" => crate::pages::import_vault::open_ram(r, window, cx),
                _ => r.send("openWebsiteBlank", json!({})),
            }
            cx.notify();
        }))
    };
    let others = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(4.))
        .child(div().mr(px(6.)).text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("start.more").trim_end_matches([':', '：']).to_string()))
        .child(other("start-create", root.tr("pw.create"), cx))
        .child(other("start-cookie", root.tr("start.cookie"), cx))
        .child(other("start-vault", root.tr("start.vault"), cx))
        .child(other("start-ram", root.tr("start.ram"), cx))
        .child(other("start-web", root.tr("start.web"), cx));
    let main = go("start-phone", root.tr("start.phone"), true, cx);
    let pad = PAGE_X;
    div().flex_1().min_h_0().child(crate::scrolled::body(
        "acc-start",
        div().w_full().px(px(pad)).pt(px(8.)).pb(px(24.)).child(crate::pages::home::empty_stage(
            root,
            1,
            None,
            root.tr("empty.title"),
            root.tr("empty.text"),
            Some(main),
            blocks,
            Some(others.into_any_element()),
            width - 2. * pad,
            cx,
        )),
    ))
}
