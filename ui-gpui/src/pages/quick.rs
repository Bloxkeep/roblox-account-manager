// Sign in with your phone (ui/src/QuickLogin.tsx; app.js's quickLogin): Roblox's own way of adding an account from a
// signed-in phone. Bloxkeep shows Roblox's code with its QR ("quickLoginStart": { code, qr, seconds }), the phone
// confirms, the account comes in ("quickLoginPoll": waiting, linked, adding, added, cancelled, expired), then a new code
// for the next account. Roblox VNG accounts confirm in the Roblox VNG app. Closing forgets the code
// ("quickLoginCancel") and leaves what was added picked.
use std::time::{Duration, Instant};

use gpui_kit::component::WindowExt as _;
use gpui_kit::component::dialog::DialogEntrance;
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use crate::{images, kit::*, link::Reply, shell::Root, theme::*};
use crate::val::s;

#[derive(Clone, Copy, PartialEq, Default)]
enum Phase {
    #[default]
    Getting,
    Waiting,
    Linked,
    Adding,
    Cancelled,
    Expired,
    Failed,
}

#[derive(Default)]
pub struct Data {
    open: bool,
    /// Which code this is: a newer one, or a closing, ends the older one's asking.
    code_n: u64,
    vng: bool,
    code: String,
    qr: String,
    phase: Phase,
    error: String,
    /// Who entered the code on the phone, and their picture when Roblox gave one.
    linked: String,
    linked_pic: String,
    until: Option<Instant>,
    /// The accounts added so far (their ids and names).
    added: Vec<(String, String)>,
    /// Codes that ran out one after another: renewed quietly a few times, then it stops asking Roblox.
    expired_row: u32,
    /// Signing a signed-out account in again: its name, at the top.
    for_name: String,
    /// Which opening this is: the countdown loop of one closed and opened again at once stops (two ran side by side).
    opened: u64,
}

/// A code that is gone can't be used any more: it goes, so nobody types it in.
fn clear(r: &mut Root) {
    r.quick.code.clear();
    r.quick.qr.clear();
    r.quick.until = None;
}

/// A new code (and its QR), then the asking whether the phone has confirmed.
fn start(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let Some(link) = r.link.clone() else { return };
    r.quick.code_n += 1;
    let mine = r.quick.code_n;
    clear(r);
    r.quick.phase = Phase::Getting;
    window.refresh();
    let realm = if r.quick.vng { "vng" } else { "" };
    let rx = link.call("quickLoginStart", json!({ "realm": realm }));
    cx.spawn_in(window, async move |this, cx| {
        let answer = rx.await;
        let go = this.update_in(cx, |r, window, _| {
            if !r.quick.open || r.quick.code_n != mine {
                return false;
            }
            let ok = match answer {
                Ok(Ok(v)) if v.is_object() => {
                    r.quick.code = s(&v, "code").to_string();
                    r.quick.qr = s(&v, "qr").to_string();
                    r.quick.until = Some(Instant::now() + Duration::from_secs(v.get("seconds").and_then(Value::as_u64).unwrap_or(120)));
                    r.quick.phase = Phase::Waiting;
                    true
                }
                Ok(Err(e)) => {
                    r.quick.error = e.to_string();
                    r.quick.phase = Phase::Failed;
                    false
                }
                _ => {
                    r.quick.phase = Phase::Failed;
                    false
                }
            };
            window.refresh();
            ok
        });
        if !matches!(go, Ok(true)) {
            return;
        }
        // Has the phone confirmed? Asked every two and a half seconds while this code is the one shown.
        loop {
            cx.background_executor().timer(Duration::from_millis(2500)).await;
            if !matches!(this.update_in(cx, |r, _, _| r.quick.open && r.quick.code_n == mine), Ok(true)) {
                break;
            }
            let answer = link.call("quickLoginPoll", json!({})).await;
            if !matches!(this.update_in(cx, |r, window, cx| heard(r, mine, answer, window, cx)), Ok(true)) {
                break;
            }
        }
    })
    .detach();
}

/// What Roblox said about the code; whether to go on asking about it.
fn heard(r: &mut Root, mine: u64, answer: Result<Reply, futures::channel::oneshot::Canceled>, window: &mut Window, cx: &mut Context<Root>) -> bool {
    if !r.quick.open || r.quick.code_n != mine {
        return false;
    }
    let v = match answer {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => {
            // That code is spent: it goes with the error, as it does when it is declined or runs out.
            clear(r);
            r.quick.error = e.to_string();
            r.quick.phase = Phase::Failed;
            window.refresh();
            return false;
        }
        // The line to the app went: said, not left waiting on a code that can't be heard about.
        Err(_) => {
            clear(r);
            r.quick.error = r.tr("ui.lost");
            r.quick.phase = Phase::Failed;
            window.refresh();
            return false;
        }
    };
    let go_on = match s(&v, "status") {
        "added" => {
            let a = v.get("added").cloned().unwrap_or(Value::Null);
            let (id, name) = (s(&a, "id").to_string(), s(&a, "label").to_string());
            let key = if a.get("merged").and_then(Value::as_bool).unwrap_or(false) { "t.merged" } else { "t.added" };
            r.toast = Some((r.i18n.tv(key, &[("name", &name)]), "ok".into()));
            r.quick.added.push((id, name));
            r.quick.expired_row = 0;
            // (The next account's code.)
            start(r, window, cx);
            false
        }
        "linked" => {
            r.quick.expired_row = 0;
            r.quick.linked = s(&v, "name").to_string();
            r.quick.linked_pic = s(&v, "picture").to_string();
            r.quick.phase = Phase::Linked;
            true
        }
        "adding" => {
            r.quick.phase = Phase::Adding;
            true
        }
        "cancelled" => {
            clear(r);
            r.quick.phase = Phase::Cancelled;
            false
        }
        "expired" | "none" => {
            // Renewed quietly a few times; after that it stops asking Roblox until the person comes back.
            r.quick.expired_row += 1;
            if r.quick.expired_row <= 3 {
                start(r, window, cx);
            } else {
                clear(r);
                r.quick.phase = Phase::Expired;
            }
            false
        }
        _ => {
            if let Some(secs) = v.get("seconds").and_then(Value::as_u64) {
                r.quick.until = Some(Instant::now() + Duration::from_secs(secs));
            }
            r.quick.phase = Phase::Waiting;
            true
        }
    };
    window.refresh();
    cx.notify();
    go_on
}

fn close(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    r.quick.open = false;
    r.quick.code_n += 1;
    r.send("quickLoginCancel", json!({}));
    // What was added is picked, ready to play.
    if !r.quick.added.is_empty() {
        r.selected = r.quick.added.iter().map(|(id, _)| id.clone()).filter(|id| !id.is_empty()).collect();
    }
    window.close_dialog(cx);
    cx.notify();
}

pub fn open(r: &mut Root, vng: bool, window: &mut Window, cx: &mut Context<Root>) {
    open_for(r, vng, String::new(), window, cx)
}

/// "Sign in again" on a signed-out account (app.js's quickLogin(realm, label)): its realm, and its name at the top.
pub fn open_for(r: &mut Root, vng: bool, for_name: String, window: &mut Window, cx: &mut Context<Root>) {
    let opened = r.quick.opened.wrapping_add(1);
    r.quick = Data { open: true, vng, code_n: r.quick.code_n, for_name, opened, ..Default::default() };
    r.help_open = true;
    start(r, window, cx);
    // The time left on the code counts down.
    cx.spawn_in(window, async move |this, cx| {
        loop {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let going = this.update_in(cx, |r, window, _| {
                if r.quick.open && r.quick.phase == Phase::Waiting {
                    window.refresh();
                }
                r.quick.open && r.quick.opened == opened
            });
            if !matches!(going, Ok(true)) {
                break;
            }
        }
    })
    .detach();

    let this = cx.weak_entity();
    window.open_dialog(cx, move |dialog, _, cx| {
        let Some(root) = this.upgrade() else { return dialog };
        let qr = Some(root.read(cx).quick.qr.clone()).filter(|q| !q.is_empty()).and_then(|q| images::pic(&q, cx));
        let linked_pic = Some(root.read(cx).quick.linked_pic.clone()).filter(|q| !q.is_empty()).and_then(|q| images::pic(&q, cx));
        let r = root.read(cx);
        let q = &r.quick;
        let dead = matches!(q.phase, Phase::Cancelled | Phase::Expired | Phase::Failed);
        // Roblox's accounts, or Roblox VNG's (each confirms in its own app).
        let mut realm = crate::kit::slide("quick-realm", div().flex().gap(px(2.)).p(px(3.)).rounded(px(R_M)).bg(c(INSET)));
        for (label, vng) in [("Roblox", false), ("Roblox VNG", true)] {
            let this = this.clone();
            realm = realm.child(segment(label, q.vng == vng).flex_1().h(px(28.)).justify_center().text_size(px(T_M)).cursor_pointer().child(label).on_click(move |_, w, cx| {
                let _ = this.update(cx, |r, cx| {
                    if r.quick.vng != vng {
                        r.quick.vng = vng;
                        r.quick.expired_row = 0;
                        start(r, w, cx);
                    }
                });
            }));
        }
        let step = |n: usize, key: &str| {
            div()
                .flex()
                .gap(px(8.))
                .text_size(px(T_M))
                .line_height(px(19.))
                .text_color(c(TEXT_2))
                .child(div().w(px(16.)).flex_none().text_color(c(MUTED)).font_features(tnum()).child(format!("{n}.")))
                .child(div().min_w_0().flex_1().child(r.tr(key)))
        };
        // (app.css .ql-code: Roblox's code in a fixed-width face, easy to read off and type.)
        let code = if q.code.is_empty() { "······".to_string() } else { q.code.chars().map(|ch| ch.to_string()).collect::<Vec<_>>().join(" ") };
        // The QR keeps its white margin so phones can read it; with no QR it is a dim square, not a white block.
        let has_qr = qr.is_some() && !dead;
        let main = div()
            .flex()
            .items_center()
            .gap(px(22.))
            .child(
                div()
                    .size(px(172.))
                    .flex_none()
                    .rounded(px(R_L))
                    .bg(if has_qr { c(WHITE) } else { c(RAISED_2) })
                    .flex()
                    .items_center()
                    .justify_center()
                    // (A code that is gone: its picture and letters go dim and its steps go, so nobody scans or types it.)
                    .when(dead, |d| d.child(icon("qr", 56., ca(MUTED, 0.45))))
                    .when(!dead, |d| match qr {
                        Some(pic) => d.child(img(pic).size(px(148.))),
                        // (No QR yet: a quiet "Getting a code…" in the square, not an empty dark box.)
                        None => d.flex_col().gap(px(10.)).child(spinner(20., c(MUTED))).child(div().px(px(12.)).text_center().text_size(px(T_S)).text_color(c(MUTED)).child(r.tr("ql.getting"))),
                    }),
            )
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .child(div().when(dead, |d| d.opacity(0.3)).text_size(px(28.)).font_family("Consolas").font_weight(FontWeight::BOLD).text_color(c(TEXT)).child(code))
                    .when(!dead, |d| d.child(div().flex().flex_col().gap(px(5.)).child(step(1, if q.vng { "ql.step1Vng" } else { "ql.step1" })).child(step(2, "ql.step2")).child(step(3, "ql.step3")))),
            );
        let again = {
            let this = this.clone();
            Button::new("ql-new", r.tr("ql.newCode")).small().on_click(move |_, w, cx| {
                let _ = this.update(cx, |r, cx| {
                    r.quick.expired_row = 0;
                    start(r, w, cx);
                });
            })
        };
        // Where it stands, in a strip of its own (app.css .ql-status): a turning mark while it waits, the person who
        // entered the code once they have, and each way it can end with its own mark and colour.
        let line = |tone: Option<u32>| {
            div()
                .min_h(px(44.))
                .pl(px(12.))
                .pr(px(6.))
                .py(px(6.))
                .rounded(px(R_M))
                .border_1()
                .flex()
                .items_center()
                .gap(px(10.))
                .text_size(px(T_M))
                .line_height(px(19.))
                .map(|d| match tone {
                    Some(t) => d.bg(ca(t, 0.1)).border_color(ca(t, 0.32)).text_color(mix(c(t), c(WHITE), 0.3)),
                    None => d.bg(ca(WHITE, 0.025)).border_color(c(LINE)).text_color(c(TEXT_2)),
                })
        };
        let wait = |text: String| line(None).child(spinner(15., c(TEXT_2))).child(div().min_w_0().flex_1().child(text));
        let ended = |tone: Option<u32>, mark: &'static str, text: String, again: Button| {
            line(tone).child(icon(mark, 16., tone.map_or(c(MUTED), c))).child(div().min_w_0().flex_1().child(text)).child(again)
        };
        let hide = r.state.get("hideNames").and_then(Value::as_bool).unwrap_or(false);
        let status = match q.phase {
            Phase::Getting => wait(r.tr("ql.getting")),
            Phase::Waiting => {
                let left = q.until.map_or(0, |t| t.saturating_duration_since(Instant::now()).as_secs());
                wait(r.i18n.tv("ql.waiting", &[("time", &format!("{}:{:02}", left / 60, left % 60))]))
            }
            // (Names hidden in Settings: the phone's account isn't named, or shown, either.)
            Phase::Linked => line(Some(OK))
                .child(match linked_pic.filter(|_| !hide) {
                    Some(pic) => img(pic).size(px(24.)).rounded_full().flex_none().into_any_element(),
                    None => div().size(px(24.)).flex_none().rounded_full().bg(ca(OK, 0.18)).flex().items_center().justify_center().child(icon("user", 14., c(OK))).into_any_element(),
                })
                .child(div().min_w_0().flex_1().child(if hide { r.tr("ql.linkedHidden") } else { r.i18n.tv("ql.linked", &[("name", &q.linked)]) }))
                .child(spinner(15., c(OK))),
            Phase::Adding => wait(r.tr("ql.adding")),
            Phase::Cancelled => ended(Some(BAD), "x", r.tr("ql.cancelled"), again),
            Phase::Expired => ended(None, "clock", r.tr("ql.expired"), again),
            Phase::Failed => ended(Some(BAD), "alert", q.error.clone(), again),
        };
        let mut body = div().flex().flex_col().gap(px(16.));
        if !q.for_name.is_empty() {
            // "Signing in again to <b>name</b>.": the name in bold, where the words put it.
            let words = r.i18n.tv("ql.for", &[("name", "\u{0}")]);
            let mut parts = words.split('\u{0}');
            let before = parts.next().unwrap_or_default().to_string();
            let after = parts.next().unwrap_or_default().to_string();
            body = body.child(
                div()
                    .flex()
                    .flex_wrap()
                    .text_size(px(T_M))
                    .text_color(c(TEXT_2))
                    .child(before)
                    .child(div().font_weight(FontWeight::BOLD).text_color(c(TEXT)).child(q.for_name.clone()))
                    .child(after),
            );
        }
        body = body.child(realm).child(main).child(status);
        if !q.added.is_empty() {
            let names = q.added.iter().map(|(_, n)| n.clone()).collect::<Vec<_>>().join(", ");
            body = body.child(div().text_size(px(T_M)).line_height(px(17.)).text_color(c(OK)).child(r.i18n.tv("ql.addedList", &[("names", &names)])));
        }
        let (browser, done, shut) = (this.clone(), this.clone(), this.clone());
        let foot = dialog_foot()
            // (Roblox VNG has no roblox.com sign-in.)
            .when(!q.vng, |d| {
                d.child(Button::new("ql-browser", r.tr("ql.browser")).on_click(move |_, w, cx| {
                    let _ = browser.update(cx, |r, cx| {
                        close(r, w, cx);
                        r.add_browser(cx);
                    });
                }))
            })
            .child(Button::new("ql-done", r.tr("common.done")).primary().on_click(move |_, w, cx| {
                let _ = done.update(cx, |r, cx| close(r, w, cx));
            }));
        framed(dialog, r.tr("ql.title"), r.tr("common.close"), move |_, w, cx| {
            let _ = shut.update(cx, |r, cx| close(r, w, cx));
        })
        .w(px(DIALOG_W))
        .centered()
        // (Only its buttons and the × close it, so the code is always forgotten on the way out.)
        .overlay_closable(false)
        .keyboard(false)
        .entrance(DialogEntrance::FadeSlide)
        .child(dialog_body(body))
        .footer(foot)
    });
}
