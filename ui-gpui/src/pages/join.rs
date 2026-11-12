// Join a player (ui/src/JoinPlayer.tsx; app.js's joinPlayerModal): the picked accounts open into the server someone is
// in. The name (or user ID) is looked up by the app with those accounts' sign-ins ("findPlayer": { status, placeId,
// jobId, userId, name, displayName, game }); found, the join is the app's "joinServer" through the usual launch, so its
// warnings ask first, and with "look them up again" ticked a rejoin goes wherever that player is by then. Names looked
// up before are offered again (the app's recentPlayers), each with a way to forget it.
use std::{cell::Cell, rc::Rc};

use gpui_kit::component::WindowExt as _;
use gpui_kit::component::dialog::DialogEntrance;
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use crate::{kit::*, shell::Root, theme::*};

#[derive(Default)]
pub struct Data {
    /// The accounts that will join.
    ids: Vec<String>,
    /// What the box says under the name: its words, how grave (0 a note, 1 a warning, 2 a problem), and whether it is
    /// still looking.
    msg: Option<(String, u8, bool)>,
    /// Rejoins look the player up again (the app remembers the tick once a join goes ahead with it).
    follow: bool,
    busy: bool,
    /// Which opening of the box this is, and whether it is still open: a reply for a box closed (Esc, a click beside
    /// it, Cancel) or opened again since does nothing (gpui's close_dialog shuts the newest box, not a given one).
    seq: u64,
    open: bool,
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or_default()
}

fn launching(r: &Root) -> bool {
    r.state.get("launching").and_then(Value::as_bool).unwrap_or(false)
}

/// The name is being typed again: what was said about the last one goes.
pub fn typed(r: &mut Root) {
    if !r.join.busy {
        r.join.msg = None;
    }
}

/// Find and join: Enter in the box, the button, or a name picked from the recent ones.
pub fn submit(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    if r.join.busy {
        return;
    }
    let input = r.jp_name.read(cx).value().trim().to_string();
    let refuse = if input.is_empty() {
        Some((r.tr("jp.needName"), 2))
    } else if launching(r) {
        // One launch at a time: while one runs, the join wouldn't start (and the lookup would be for nothing).
        Some((r.tr("jp.launching"), 1))
    } else {
        None
    };
    if let Some((text, tone)) = refuse {
        r.join.msg = Some((text, tone, false));
        window.refresh();
        return;
    }
    let Some(link) = r.link.clone() else { return };
    r.join.busy = true;
    r.join.msg = Some((r.i18n.tv("jp.looking", &[("name", &input)]), 0, true));
    window.refresh();
    let ids = r.join.ids.clone();
    let seq = r.join.seq;
    let rx = link.call("findPlayer", json!({ "input": input, "ids": ids }));
    cx.spawn_in(window, async move |this, cx| {
        let answer = rx.await;
        let _ = this.update_in(cx, |r, window, cx| {
            // The box was opened again since: this reply is the old box's, and that box is gone.
            if r.join.seq != seq {
                return;
            }
            r.join.busy = false;
            // The box was closed while looking: nothing is closed and nobody joins.
            if !r.join.open {
                return;
            }
            match answer {
                Ok(Ok(v)) if v.is_object() => {
                    let (name, shown, status) = (s(&v, "name"), s(&v, "displayName"), s(&v, "status"));
                    let who = if !shown.is_empty() && shown != name {
                        format!("{shown} (@{name})")
                    } else if name.is_empty() {
                        input.clone()
                    } else {
                        name.to_string()
                    };
                    if status != "found" {
                        // (Offline, on the website, in Studio, hidden by their privacy settings, or no such name.)
                        r.join.msg = Some((r.i18n.tv(&format!("jp.{status}"), &[("name", &who)]), if status == "notFound" { 2 } else { 1 }, false));
                    } else if launching(r) {
                        // (One started meanwhile: a scheduled preset.)
                        r.join.msg = Some((r.tr("jp.launching"), 1, false));
                    } else {
                        window.close_dialog(cx);
                        r.join.open = false;
                        r.join.msg = None;
                        let n = ids.len() as i64;
                        let place = v.get("placeId").and_then(Value::as_i64).unwrap_or(0);
                        let game = if s(&v, "game").is_empty() { r.i18n.tv("dock.placeN", &[("id", &place.to_string())]) } else { s(&v, "game").to_string() };
                        r.toast = Some((r.i18n.tn("jp.found", n, &[("name", &who), ("game", &game), ("n", &n.to_string())]), "info".into()));
                        let args = json!({
                            "ids": ids, "placeId": place, "jobId": s(&v, "jobId"), "follow": r.join.follow,
                            "followUserId": v.get("userId").cloned().unwrap_or(Value::Null), "followName": name,
                        });
                        r.launch("joinServer", args, false, cx);
                    }
                }
                Ok(Err(e)) => r.join.msg = Some((e.to_string(), 2, false)),
                _ => r.join.msg = None,
            }
            window.refresh();
            cx.notify();
        });
    })
    .detach();
}

/// Opens the box for these accounts.
pub fn open(r: &mut Root, ids: Vec<String>, window: &mut Window, cx: &mut Context<Root>) {
    if ids.is_empty() {
        r.toast = Some((r.tr("t.selectFirst"), "warn".into()));
        cx.notify();
        return;
    }
    let follow = r.state.pointer("/settings/followJoinedPlayer").and_then(Value::as_bool) != Some(false);
    let seq = r.join.seq.wrapping_add(1);
    r.join = Data { ids, msg: None, follow, busy: false, seq, open: true };
    r.jp_name.update(cx, |s, cx| s.set_value("", window, cx));
    // (The recent names are the app's: the box is drawn again as its state changes.)
    r.help_open = true;
    let this = cx.weak_entity();
    let first = Rc::new(Cell::new(true));
    window.open_dialog(cx, move |dialog, window, cx| {
        let Some(root) = this.upgrade() else { return dialog };
        // The name box has the keyboard as the box opens (once: later draws leave the keyboard where it is).
        if first.replace(false) {
            let h = root.read(cx).jp_name.read(cx).focus_handle(cx);
            window.focus(&h, cx);
        }
        let r = root.read(cx);
        let n = r.join.ids.len() as i64;
        let recent: Vec<String> = r.state.pointer("/settings/recentPlayers").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
        let mut body = div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(div().text_size(px(T_M)).line_height(px(20.)).text_color(c(TEXT_2)).child(r.i18n.tn("jp.note", n, &[("n", &n.to_string())])))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(div().text_size(px(T_S)).font_weight(FontWeight::MEDIUM).text_color(c(TEXT_2)).child(r.tr("jp.label")))
                    .child(field(&r.jp_name, Some("user"))),
            );
        if !recent.is_empty() {
            let mut chips = div().flex().flex_wrap().items_center().gap(px(6.)).child(div().mr(px(2.)).text_size(px(T_S)).text_color(c(MUTED)).child(r.tr("jp.recent")));
            for (i, name) in recent.into_iter().enumerate() {
                let (pick, forget) = (this.clone(), this.clone());
                let (picked, gone) = (name.clone(), name.clone());
                chips = chips.child(
                    div()
                        .h(px(26.))
                        .pl(px(10.))
                        .pr(px(2.))
                        .flex()
                        .items_center()
                        .gap(px(2.))
                        .rounded_full()
                        .bg(c(RAISED_2))
                        .text_size(px(T_S + 0.5))
                        // A name from the list is looked up at once.
                        .child(focusable(div().id(("jp-pick", i))).cursor_pointer().text_color(c(TEXT_2)).hover(|s| s.text_color(c(TEXT))).child(name).on_click(move |_, w, cx| {
                            let _ = pick.update(cx, |r, cx| {
                                r.join.msg = None;
                                r.jp_name.update(cx, |s, cx| s.set_value(picked.clone(), w, cx));
                                submit(r, w, cx);
                            });
                        }))
                        .child(IconButton::new(("jp-x", i), "x").size(22.).tip(r.tr("jp.forget")).on_click(move |_, _, cx| {
                            let _ = forget.update(cx, |r, _| r.send("forgetPlayer", json!({ "name": gone })));
                        })),
                );
            }
            body = body.child(chips);
        }
        {
            let this = this.clone();
            // (Its sentence wraps inside the dialog instead of running past its edge.)
            body = body.child(div().w_full().flex().child(Check::new("jp-follow", r.join.follow).label(r.tr("jp.follow")).on_toggle(move |on, w, cx| {
                let on = *on;
                let _ = this.update(cx, |r, _| r.join.follow = on);
                w.refresh();
            })));
        }
        if let Some((text, tone, looking)) = r.join.msg.clone() {
            let ink = match tone {
                2 => BAD,
                1 => WARN,
                _ => TEXT_2,
            };
            body = body.child(div().flex().items_center().gap(px(8.)).text_size(px(T_M)).line_height(px(19.)).text_color(c(ink)).when(looking, |d| d.child(spinner(14., c(ink)))).child(div().min_w_0().flex_1().child(text)));
        }
        body = body.child(
            div()
                .pt(px(4.))
                .flex()
                .gap(px(8.))
                .text_size(px(T_S))
                .line_height(px(16.))
                .text_color(c(MUTED))
                .child(div().mt(px(1.)).flex_none().child(icon("shield", 13., c(MUTED))))
                .child(div().min_w_0().flex_1().child(r.tr("jp.privacy"))),
        );
        let go = this.clone();
        let (cancel, closed, shut) = (this.clone(), this.clone(), this.clone());
        let foot = dialog_foot()
            .child(Button::new("jp-cancel", r.tr("common.cancel")).on_click(move |_, w, cx| {
                let _ = cancel.update(cx, |r, _| {
                    if r.join.seq == seq {
                        r.join.open = false;
                    }
                });
                w.close_dialog(cx);
            }))
            .child(Button::new("jp-go", r.tr("jp.go")).icon("userGo").primary().on_click(move |_, w, cx| {
                let _ = go.update(cx, |r, cx| submit(r, w, cx));
            }));
        framed(dialog, r.tr("jp.title"), r.tr("common.close"), move |_, w, cx| {
            let _ = shut.update(cx, |r, _| {
                if r.join.seq == seq {
                    r.join.open = false;
                }
            });
            w.close_dialog(cx);
        })
        .w(px(480.))
        .centered()
        .entrance(DialogEntrance::FadeSlide)
            // (Esc or a click beside it.)
            .on_close(move |_, _, cx| {
                let _ = closed.update(cx, |r, _| {
                    if r.join.seq == seq {
                        r.join.open = false;
                    }
                });
            })
        .child(dialog_body(body))
        .footer(foot)
    });
}
