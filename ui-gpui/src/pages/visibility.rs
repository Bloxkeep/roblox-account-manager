// Visibility on Roblox for the picked accounts: who sees them online and who may join them in a game. These are each
// account's own Roblox privacy settings (Settings › Privacy › Visibility), read and written from here (roblox.rs's
// visibility and set_visibility) with the sign-in the app hands over for each account ("robloxSignIn"). "No one" for
// online is how an account appears offline to everybody. Nothing is kept by Bloxkeep.
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::dialog::DialogEntrance;
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use crate::{kit::*, roblox, shell::Root, theme::*};
use crate::val::s;

const VALUES: [&str; 3] = ["AllUsers", "Friends", "NoOne"];

#[derive(Default)]
pub struct Data {
    ids: Vec<String>,
    /// What Roblox says each account has now (None while it is read).
    rows: Option<Vec<Value>>,
    /// What Apply will set: None leaves that setting as it is.
    online: Option<&'static str>,
    join: Option<&'static str>,
    busy: bool,
    /// What the last Apply did: its words, and whether something failed.
    said: Option<(String, bool)>,
    /// Which opening of the box this is, and whether it is still open: a reply for a box closed or opened again since
    /// does nothing (gpui's close_dialog shuts the newest box, not a given one). And which reading of the settings is
    /// the latest: an older one's answer is dropped.
    seq: u64,
    open: bool,
    asked: u64,
}

/// Why an account couldn't be read or set, in words.
fn why(r: &Root, row: &Value) -> String {
    match s(row, "why") {
        "" => s(row, "error").to_string(),
        w => r.tr(&format!("vis.skip.{w}")),
    }
}

/// Each account's sign-in, handed over by the app one by one ("robloxSignIn"); None for one that has none that
/// works, or that is a Roblox VNG account.
async fn signs(link: crate::link::Link, ids: Vec<String>) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    for id in ids {
        let got = link.call("robloxSignIn", json!({ "id": id })).await.ok().and_then(Result::ok);
        let cookie = got.filter(|v| s(v, "id") == id).map(|v| s(&v, "cookie").to_string()).filter(|c| !c.is_empty());
        out.push((id, cookie));
    }
    out
}

fn bad_row(id: &str, error: String) -> Value {
    if error.is_empty() { json!({ "id": id, "ok": false, "why": "signin", "error": "" }) } else { json!({ "id": id, "ok": false, "why": "", "error": error }) }
}

fn load(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let Some(link) = r.link.clone() else { return };
    let ids = r.vis.ids.clone();
    r.vis.asked = r.vis.asked.wrapping_add(1);
    let asked = r.vis.asked;
    // (A shot has no Roblox to ask: made-up settings, the third account without a sign-in.)
    #[cfg(feature = "shot")]
    if std::env::var("BLOXKEEP_SHOT_ASK").is_ok() {
        r.vis.rows = Some(ids.iter().enumerate().map(|(i, id)| if i == 2 { bad_row(id, String::new()) } else { json!({ "id": id, "ok": true, "online": "Friends", "join": if i == 0 { "AllUsers" } else { "Friends" } }) }).collect());
        return;
    }
    let all = ids.clone();
    cx.spawn_in(window, async move |this, cx| {
        let signs = signs(link, ids).await;
        let rows = roblox::run(move || {
            signs
                .into_iter()
                .map(|(id, cookie)| match cookie.as_deref().map(roblox::visibility) {
                    Some(Ok((online, join))) => json!({ "id": id, "ok": true, "online": online.map(|c| c.now).unwrap_or_default(), "join": join.map(|c| c.now).unwrap_or_default() }),
                    Some(Err(e)) => bad_row(&id, e),
                    None => bad_row(&id, String::new()),
                })
                .collect::<Vec<Value>>()
        })
        .await;
        let _ = this.update_in(cx, |r, window, cx| {
            // (A later reading, or the box opened again for other accounts, has the say.)
            if r.vis.asked != asked {
                return;
            }
            // (The reading itself failed: each account says so, not an empty list.)
            let rows = rows.unwrap_or_else(|_| all.iter().map(|id| bad_row(id, r.tr("ps.failed"))).collect());
            r.vis.rows = Some(rows);
            window.refresh();
            cx.notify();
        });
    })
    .detach();
}

fn apply(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    if r.vis.busy || (r.vis.online.is_none() && r.vis.join.is_none()) {
        return;
    }
    let Some(link) = r.link.clone() else { return };
    r.vis.busy = true;
    r.vis.said = None;
    window.refresh();
    let (ids, online, join) = (r.vis.ids.clone(), r.vis.online, r.vis.join);
    let seq = r.vis.seq;
    cx.spawn_in(window, async move |this, cx| {
        let signs = signs(link, ids).await;
        // One account after another, a short pause between them, so Roblox doesn't rate-limit a dozen.
        let failed = roblox::run(move || {
            let mut failed: Vec<Value> = Vec::new();
            let mut done = 0i64;
            for (id, cookie) in signs {
                let Some(cookie) = cookie else {
                    failed.push(bad_row(&id, String::new()));
                    continue;
                };
                if done > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(400));
                }
                match roblox::set_visibility(&cookie, online, join) {
                    Ok(()) => done += 1,
                    Err(e) => failed.push(bad_row(&id, e)),
                }
            }
            (done, failed)
        })
        .await;
        let _ = this.update_in(cx, |r, window, cx| {
            // The box was opened again since: this reply is the old box's, and that box is gone.
            if r.vis.seq != seq {
                return;
            }
            r.vis.busy = false;
            // The box was closed meanwhile: nothing is closed (what was set stays set).
            if !r.vis.open {
                return;
            }
            if let Ok((done, failed)) = failed {
                let mut text = r.i18n.tv("vis.done", &[("n", &done.to_string())]);
                if failed.is_empty() {
                    window.close_dialog(cx);
                    r.vis.open = false;
                    r.toast = Some((text, "ok".into()));
                } else {
                    let names: Vec<String> = failed.iter().map(|f| format!("{} ({})", crate::pages::accounts::label_by_id(r, s(f, "id")).unwrap_or_default(), why(r, f))).collect();
                    text = format!("{text} {}", r.i18n.tv("vis.failed", &[("n", &failed.len().to_string()), ("names", &names.join(", "))]));
                    r.vis.said = Some((text, true));
                    // (What the ones that went through have now.)
                    r.vis.rows = None;
                    load(r, window, cx);
                }
            } else {
                r.vis.said = Some((r.tr("ps.failed"), true));
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
    // (The counts go on from the last box's, so its replies can't pass for this one's.)
    let seq = r.vis.seq.wrapping_add(1);
    r.vis = Data { ids, seq, open: true, asked: r.vis.asked, ..Default::default() };
    r.help_open = true;
    load(r, window, cx);
    let this = cx.weak_entity();
    window.open_dialog(cx, move |dialog, _, cx| {
        let Some(root) = this.upgrade() else { return dialog };
        let r = root.read(cx);
        let d = &r.vis;
        let n = d.ids.len();
        let note = if n == 1 { r.i18n.tv("vis.note1", &[("name", &crate::pages::accounts::label_by_id(r, &d.ids[0]).unwrap_or_default())]) } else { r.i18n.tv("vis.noteN", &[("n", &n.to_string())]) };
        let mut body = div().flex().flex_col().gap(px(16.)).child(div().text_size(px(T_M)).line_height(px(20.)).text_color(c(TEXT_2)).child(note));

        // What the accounts have now, said under each setting: one value when they agree.
        let read: Vec<&Value> = d.rows.iter().flatten().filter(|row| row.get("ok").and_then(Value::as_bool) == Some(true)).collect();
        let now = |key: &str| -> Option<String> {
            let mut seen: Vec<&str> = read.iter().map(|row| s(row, key)).filter(|v| !v.is_empty()).collect();
            seen.sort_unstable();
            seen.dedup();
            match seen.as_slice() {
                [] => None,
                [one] => Some(r.i18n.tv("vis.now", &[("v", &crate::pages::rsettings::vis_word(r, one))])),
                _ => Some(r.tr("vis.mixed")),
            }
        };
        let setting = |key: &'static str, title: String, picked: Option<&'static str>, set: fn(&mut Data, Option<&'static str>)| {
            let mut row = div().flex().flex_wrap().gap(px(6.));
            let choices = std::iter::once((None, r.tr("vis.keep"))).chain(VALUES.iter().map(|v| (Some(*v), r.tr(&format!("vis.{v}")))));
            for (i, (value, label)) in choices.enumerate() {
                let this = this.clone();
                row = row.child(pill((key, i), picked == value, true).child(label).on_click(move |_, w, cx| {
                    let _ = this.update(cx, |r, _| {
                        set(&mut r.vis, value);
                        r.vis.said = None;
                    });
                    w.refresh();
                }));
            }
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .gap(px(10.))
                        .child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(title))
                        .children(now(if key == "vis-online" { "online" } else { "join" }).map(|t| div().text_size(px(T_S)).text_color(c(MUTED)).child(t))),
                )
                .child(row)
        };
        body = body.child(setting("vis-online", r.tr("vis.online"), d.online, |d, v| d.online = v)).child(setting("vis-join", r.tr("vis.join"), d.join, |d, v| d.join = v));
        body = body.child(div().text_size(px(T_S)).line_height(px(17.)).text_color(c(MUTED)).child(r.tr("vis.offlineHint")));

        if d.rows.is_none() {
            body = body.child(div().flex().items_center().gap(px(8.)).text_size(px(T_S)).text_color(c(MUTED)).child(spinner(13., c(MUTED))).child(r.tr("vis.reading")));
        }
        // The accounts Roblox couldn't be asked for.
        let skipped: Vec<String> = d.rows.iter().flatten().filter(|row| row.get("ok").and_then(Value::as_bool) != Some(true)).map(|row| format!("{} ({})", crate::pages::accounts::label_by_id(r, s(row, "id")).unwrap_or_default(), why(r, row))).collect();
        if !skipped.is_empty() && d.said.is_none() {
            body = body.child(div().flex().gap(px(8.)).text_size(px(T_S)).line_height(px(17.)).text_color(c(WARN)).child(div().mt(px(1.)).flex_none().child(icon("alert", 13., c(WARN)))).child(div().min_w_0().flex_1().child(skipped.join(", "))));
        }
        if let Some((text, bad)) = d.said.clone() {
            body = body.child(if bad { form_error(text) } else { div().text_size(px(T_M)).line_height(px(19.)).text_color(c(OK)).child(text) });
        }

        let go = this.clone();
        let (cancel, closed, shut) = (this.clone(), this.clone(), this.clone());
        let idle = d.online.is_none() && d.join.is_none();
        let foot = dialog_foot()
            .child(Button::new("vis-cancel", r.tr("common.cancel")).on_click(move |_, w, cx| {
                let _ = cancel.update(cx, |r, _| {
                    if r.vis.seq == seq {
                        r.vis.open = false;
                    }
                });
                w.close_dialog(cx);
            }))
            .child(Button::new("vis-apply", r.tr("vis.apply")).icon("check").primary().disabled(idle || d.busy).on_click(move |_, w, cx| {
                let _ = go.update(cx, |r, cx| apply(r, w, cx));
            }));
        framed(dialog, r.tr("vis.title"), r.tr("common.close"), move |_, w, cx| {
            let _ = shut.update(cx, |r, _| {
                if r.vis.seq == seq {
                    r.vis.open = false;
                }
            });
            w.close_dialog(cx);
        })
        .w(px(DIALOG_W))
        .centered()
        .entrance(DialogEntrance::FadeSlide)
            // (Esc or a click beside it.)
            .on_close(move |_, _, cx| {
                let _ = closed.update(cx, |r, _| {
                    if r.vis.seq == seq {
                        r.vis.open = false;
                    }
                });
            })
        .child(dialog_body(body))
        .footer(foot)
    });
}
