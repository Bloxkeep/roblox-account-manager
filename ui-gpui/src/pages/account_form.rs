// An account's form (ui/src/AccountForm.tsx; app.js's accountModal): adding one by its .ROBLOSECURITY cookie, or editing
// one: its nickname, group, note, the Roblox password kept for reference (the saved one can be shown), and its
// authenticator key: the saved key's code counted down with Copy and Remove, or a box for a key, whose code is shown at
// once so it can be checked against the authenticator app. The app does the work ("addViaCookie", "updateAccount",
// "reveal", "totpCode", "totpCheck") and refuses in its own words, said under the boxes.
//
// Left out for now: the group's list of earlier groups, and the question before closing a changed form.
use std::time::{Duration, Instant};

use gpui_kit::component::WindowExt as _;
use gpui_kit::component::dialog::DialogEntrance;
use gpui_kit::component::input::{InputState, TextareaState};
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use crate::{kit::*, shell::Root, theme::*};
use crate::val::s;
use crate::val::flag;

#[derive(Default)]
pub struct Data {
    /// Which opening this is: a later one, or a closing, ends this one's clock.
    seq: u64,
    open: bool,
    /// The account edited (none: one is being added by its cookie).
    id: Option<String>,
    /// The saved password as the app showed it (the eye): the box holding just that isn't a change.
    pw_loaded: Option<String>,
    /// The saved key goes when the form is saved (Remove; Undo takes that back).
    remove_key: bool,
    /// The cookie, the password and the key shown as they are, not as dots.
    shown: [bool; 3],
    error: Option<String>,
    busy: bool,
    /// The saved key's code now and when it changes; and whether it is being asked for.
    code: Option<(String, Instant)>,
    asking: bool,
    /// After an asking that failed: not asked again before then.
    code_retry: Option<Instant>,
    /// Under the key box: a typed key's code now (true), or what is wrong with the key.
    key_msg: Option<(String, bool)>,
    key_seq: u64,
    /// The note's box: a few lines (app.css textarea.field), made with the form.
    note: Option<Entity<TextareaState>>,
    /// The nickname, group and note as the form opened with them: Cancel or × asks first once one has changed.
    start: [String; 3],
}

fn account(r: &Root) -> Option<Value> {
    let id = r.acc_form.id.as_deref()?;
    r.accounts().iter().find(|a| s(a, "id") == id).cloned()
}

/// "123456" as "123 456".
fn split(code: &str) -> String {
    if code.len() == 6 && code.is_ascii() { format!("{} {}", &code[..3], &code[3..]) } else { code.to_string() }
}

/// The saved key is the one in use: there is one, it isn't being removed, and no other key is typed.
fn saved_key(r: &Root, cx: &App) -> bool {
    account(r).is_some_and(|a| flag(&a, "hasAuthenticator")) && !r.acc_form.remove_key && r.af_key.read(cx).value().trim().is_empty()
}

/// Opens the form: for this account, or (none) to add one by its cookie.
pub fn open(r: &mut Root, id: Option<String>, window: &mut Window, cx: &mut Context<Root>) {
    let seq = r.acc_form.seq + 1;
    // (The note's box is kept from one opening to the next; only its words change.)
    let note_box = match r.acc_form.note.take() {
        Some(b) => b,
        None => cx.new(|cx| TextareaState::new(window, cx).rows(3)),
    };
    r.acc_form = Data { seq, open: true, id, note: Some(note_box.clone()), ..Default::default() };
    let a = account(r);
    let hide = flag(&r.state, "hideNames");
    let put = |e: &Entity<InputState>, v: String, window: &mut Window, cx: &mut Context<Root>| {
        e.update(cx, |st, cx| {
            st.set_value(v, window, cx);
            st.set_masked(true, window, cx);
        })
    };
    put(&r.af_cookie.clone(), String::new(), window, cx);
    put(&r.af_password.clone(), String::new(), window, cx);
    put(&r.af_key.clone(), String::new(), window, cx);
    let text = |k: &str| a.as_ref().map(|a| s(a, k).to_string()).unwrap_or_default();
    // (While names are hidden the nickname can't be shown, so it isn't edited either.)
    let (alias, group, note) = (if hide { String::new() } else { text("alias") }, text("group"), text("note"));
    r.acc_form.start = [alias.clone(), group.clone(), note.clone()];
    for (e, v) in [(r.af_alias.clone(), alias), (r.af_group.clone(), group)] {
        e.update(cx, |st, cx| st.set_value(v, window, cx));
    }
    note_box.update(cx, |st, cx| st.set_value(note, window, cx));
    let hint = r.tr(if a.as_ref().is_some_and(|a| flag(a, "hasPassword")) { "am.pwSaved" } else { "am.pwNone" });
    r.af_password.update(cx, |st, cx| st.set_placeholder(hint, window, cx));
    for (e, key) in [(r.af_alias.clone(), "am.aliasPh"), (r.af_group.clone(), "am.groupPh"), (r.af_key.clone(), "am.totpPh")] {
        let hint = r.tr(key);
        e.update(cx, |st, cx| st.set_placeholder(hint, window, cx));
    }
    r.help_open = true;
    if a.as_ref().is_some_and(|a| flag(a, "hasAuthenticator")) {
        ask_code(r, window, cx);
    }
    // The saved key's code counts down, and is asked for again as it changes.
    cx.spawn_in(window, async move |this, cx| {
        loop {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let going = this.update_in(cx, |r, window, cx| {
                if !r.acc_form.open || r.acc_form.seq != seq {
                    return false;
                }
                if saved_key(r, cx) {
                    let due = r.acc_form.code_retry.is_none_or(|t| Instant::now() >= t);
                    if due && r.acc_form.code.as_ref().is_none_or(|(_, until)| Instant::now() >= *until) {
                        ask_code(r, window, cx);
                    }
                    window.refresh();
                }
                true
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
        let acc = account(root.read(cx));
        let face = acc.as_ref().map(|a| div().p(px(2.)).rounded(px(18.)).border_1().border_color(ca(WHITE, 0.14)).child(crate::pages::accounts::avatar(a, 52., RAISED, cx)));
        let r = root.read(cx);
        let f = &r.acc_form;
        let is_new = f.id.is_none();
        let hide = flag(&r.state, "hideNames");
        let label = |key: &str, hint: Option<&str>| {
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(px(T_S))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(TEXT_2))
                .child(r.tr(key))
                .when_some(hint, |d, h| d.child(div().font_weight(FontWeight::NORMAL).text_color(c(MUTED)).child(r.tr(h))))
        };
        // A secret's box with its eye: shown as it is, or as dots again.
        let secret = |e: &Entity<InputState>, at: usize| {
            let (this, shown) = (this.clone(), f.shown[at]);
            field(e, None).w_full().child(
                IconButton::new(("af-eye", at), if shown { "eyeOff" } else { "eye" }).size(30.).tip(r.tr(if shown { "am.hide" } else { "common.show" })).on_click(move |_, w, cx| {
                    let _ = this.update(cx, |r, cx| reveal(r, at, w, cx));
                }),
            )
        };
        let boxed = |l: Div, ctl: AnyElement| div().flex().flex_col().gap(px(6.)).child(l).child(ctl);
        let mut body = div().flex().flex_col().gap(px(14.));
        if is_new {
            body = body
                .child(div().text_size(px(T_M)).line_height(px(20.)).text_color(c(TEXT_2)).child(crate::pages::lock::plain(r.tr("am.callout"))))
                .child(boxed(label("am.cookie", None), secret(&r.af_cookie, 0).into_any_element()));
        } else if let Some(a) = &acc {
            // Names hidden: no @username, and no user ID either (anyone can look an account up by it).
            let user = s(a, "username");
            let id = a.get("userId").and_then(Value::as_i64).unwrap_or(0);
            let under = [if user.is_empty() { String::new() } else { format!("@{user}") }, if hide || id == 0 { String::new() } else { format!("ID {id}") }].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" · ");
            body = body.child(
                div().flex().items_center().gap(px(14.)).pb(px(14.)).mb(px(2.)).border_b_1().border_color(c(LINE)).children(face).child(
                    div()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(div().truncate().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).child(crate::pages::accounts::label(a)))
                        .child(div().truncate().text_size(px(T_S)).text_color(c(MUTED)).child(under)),
                ),
            );
        }
        body = body.child(boxed(
            label("am.alias", None),
            if !is_new && hide { div().h(px(38.)).px(px(12.)).flex().items_center().rounded(px(R_M)).bg(c(INSET)).border_1().border_color(ca(TEXT, 0.12)).text_size(px(T_M)).text_color(c(DIM)).child(r.tr("am.aliasHidden")).into_any_element() } else { field(&r.af_alias, None).into_any_element() },
        ));
        // The groups the other accounts are in, to pick with a click (one name typed the same way groups them).
        let typed = r.af_group.read(cx).value().trim().to_string();
        let mut groups: Vec<String> = Vec::new();
        for a in r.accounts() {
            let g = s(a, "group").trim();
            if !g.is_empty() && !groups.iter().any(|x| x.eq_ignore_ascii_case(g)) && groups.len() < 6 {
                groups.push(g.to_string());
            }
        }
        let mut picks = crate::kit::slide("af-groups", div().flex().flex_wrap().gap(px(6.)));
        for (n, g) in groups.iter().enumerate() {
            let on = g.eq_ignore_ascii_case(&typed);
            let (this, name) = (this.clone(), g.clone());
            picks = picks.child(chip(("af-grp", n), on).h(px(28.)).text_size(px(T_S)).when(!on, |d| d.border_1().border_color(c(LINE))).child(icon("tag", 12., if on { c(INK) } else { c(MUTED) })).child(g.clone()).on_click(move |_, w, cx| {
                let name = if on { String::new() } else { name.clone() };
                let _ = this.update(cx, |r, cx| r.af_group.update(cx, |st, cx| st.set_value(name, w, cx)));
                w.refresh();
            }));
        }
        let group_box = div().flex().flex_col().gap(px(8.)).child(field(&r.af_group, None)).when(!groups.is_empty(), |d| d.child(picks));
        body = body.child(boxed(label("am.group", None), group_box.into_any_element()));
        body = body.child(boxed(label("am.pw", Some("am.pwHint")), secret(&r.af_password, 1).into_any_element()));
        if !is_new {
            let has_key = acc.as_ref().is_some_and(|a| flag(a, "hasAuthenticator"));
            let live = has_key && !f.remove_key;
            let mut totp = div().flex().flex_col().gap(px(6.)).child(label("am.totp", Some("am.totpHint")));
            if live {
                let left = f.code.as_ref().map(|(_, until)| until.saturating_duration_since(Instant::now()).as_secs().max(1));
                let id = f.id.clone().unwrap_or_default();
                let (copy, remove) = (this.clone(), this.clone());
                // (app.css .totp-live: the code of the moment in one boxed bar with its time and buttons.)
                totp = totp.child(
                    div()
                        .min_h(px(42.))
                        .pl(px(12.))
                        .pr(px(4.))
                        .py(px(4.))
                        .rounded(px(R_M))
                        .border_1()
                        .border_color(ca(WHITE, 0.1))
                        .bg(c(RAISED_2))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(div().flex_none().text_size(px(18.)).font_family("Consolas").font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(f.code.as_ref().map_or_else(|| "··· ···".to_string(), |(code, _)| split(code))))
                        .child(div().min_w_0().flex_1().text_size(px(T_S)).text_color(c(MUTED)).font_features(tnum()).when_some(left, |d, n| d.child(r.i18n.tv("am.totpLeft", &[("s", &n.to_string())]))))
                        .child(Button::new("af-totp-copy", r.tr("am.totpCopy")).icon("copy").small().on_click(move |_, _, cx| {
                            let id = id.clone();
                            let _ = copy.update(cx, |r, cx| crate::pages::accounts::copy(r, id, "totp", cx));
                        }))
                        // (Remove, and its Undo, only change the form: the key goes once the form is saved.)
                        .child(Button::new("af-totp-remove", r.tr("am.totpRemove")).icon("trash").quiet().small().on_click(move |_, w, cx| {
                            let _ = remove.update(cx, |r, _| r.acc_form.remove_key = true);
                            w.refresh();
                        })),
                );
            } else {
                totp = totp.child(secret(&r.af_key, 2));
                let typed = !r.af_key.read(cx).value().trim().is_empty();
                if has_key && f.remove_key && !typed {
                    let undo = this.clone();
                    totp = totp.child(
                        div().flex().items_center().gap(px(8.)).text_size(px(T_S)).text_color(c(MUTED)).child(r.tr("am.totpRemoved")).child(
                            focusable(div().id("af-totp-undo")).cursor_pointer().text_color(c(TEXT_2)).hover(|s| s.text_color(c(TEXT))).child(r.tr("am.totpUndo")).on_click(move |_, w, cx| {
                                let _ = undo.update(cx, |r, cx| {
                                    r.acc_form.remove_key = false;
                                    ask_code(r, w, cx);
                                });
                                w.refresh();
                            }),
                        ),
                    );
                } else if let Some((text, ok)) = f.key_msg.clone() {
                    totp = totp.child(div().text_size(px(T_S)).line_height(px(16.)).text_color(c(if ok { OK } else { BAD })).child(text));
                }
            }
            body = body.child(totp.child(div().text_size(px(T_S)).line_height(px(16.)).text_color(c(MUTED)).child(r.tr("am.totpNote"))));
        }
        if let Some(note) = &f.note {
            body = body.child(boxed(label("am.note", None), text_area(note).into_any_element()));
        }
        if is_new {
            // About the cookie: only while one is being pasted.
            body = body.child(
                div().flex().gap(px(8.)).text_size(px(T_S)).line_height(px(16.)).text_color(c(MUTED)).child(div().mt(px(1.)).flex_none().child(icon("shield", 13., c(MUTED)))).child(div().min_w_0().flex_1().child(r.tr("am.fine"))),
            );
        }
        if let Some(why) = f.error.clone() {
            body = body.child(div().text_size(px(T_M)).line_height(px(19.)).text_color(c(BAD)).child(why));
        }
        let (cancel, go, shut) = (this.clone(), this.clone(), this.clone());
        let foot = dialog_foot()
            .child(Button::new("af-cancel", r.tr("common.cancel")).on_click(move |_, w, cx| {
                let _ = cancel.update(cx, |r, cx| close_asking(r, w, cx));
            }))
            .child(Button::new("af-save", r.tr(if is_new { "am.add" } else { "am.save" })).primary().on_click(move |_, w, cx| {
                let _ = go.update(cx, |r, cx| save(r, w, cx));
            }));
        framed(dialog, r.tr(if is_new { "am.addTitle" } else { "am.editTitle" }), r.tr("common.close"), move |_, w, cx| {
            let _ = shut.update(cx, |r, cx| close_asking(r, w, cx));
        })
        .w(px(DIALOG_W))
        .centered()
        // (Only Cancel, Save and the × close it: a click beside it, or Esc, would throw away a pasted cookie.)
        .overlay_closable(false)
        .keyboard(false)
        .entrance(DialogEntrance::FadeSlide)
        .child(dialog_body(body))
        .footer(foot)
    });
}

/// Something typed that Save would keep: a cookie, a password other than the saved one shown, a key or its removal,
/// or the nickname, group or note changed.
fn changed(r: &Root, cx: &App) -> bool {
    let f = &r.acc_form;
    let val = |e: &Entity<InputState>| e.read(cx).value().trim().to_string();
    let pw = val(&r.af_password);
    let note = f.note.as_ref().map(|n| n.read(cx).value().to_string()).unwrap_or_default();
    !val(&r.af_cookie).is_empty()
        || (!pw.is_empty() && Some(&pw) != f.pw_loaded.as_ref())
        || !val(&r.af_key).is_empty()
        || f.remove_key
        || val(&r.af_alias) != f.start[0].trim()
        || val(&r.af_group) != f.start[1].trim()
        || note.trim() != f.start[2].trim()
}

/// Cancel or ×: closes, asking first when something would be lost.
fn close_asking(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    // A save on its way: its answer closes the form (or shows what's wrong). A "Discard changes?" box up meanwhile
    // was the dialog that answer closed, leaving the form open and dead.
    if r.acc_form.busy {
        return;
    }
    if changed(r, cx) {
        r.ask_discard(window, cx, close);
    } else {
        close(r, window, cx);
    }
}

fn close(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    r.acc_form.open = false;
    r.acc_form.seq += 1;
    // (Nothing secret stays in the boxes, nor the saved password read for the eye.)
    r.acc_form.pw_loaded = None;
    for e in [r.af_cookie.clone(), r.af_password.clone(), r.af_key.clone()] {
        e.update(cx, |st, cx| st.set_value("", window, cx));
    }
    window.close_dialog(cx);
}

/// Something was typed in a box (2: the key's): the old problem goes, and a key typed is checked at once.
pub fn typed(r: &mut Root, which: u8, window: &mut Window, cx: &mut Context<Root>) {
    if !r.acc_form.open {
        return;
    }
    r.acc_form.error = None;
    if which == 2 {
        check_key(r, window, cx);
    }
    window.refresh();
}

/// A key being typed: its code now, to compare with the authenticator app, or what is wrong with it.
fn check_key(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let key = r.af_key.read(cx).value().trim().to_string();
    r.acc_form.key_seq += 1;
    let (mine, seq) = (r.acc_form.key_seq, r.acc_form.seq);
    if key.is_empty() {
        r.acc_form.key_msg = None;
        return;
    }
    let Some(link) = r.link.clone() else { return };
    let rx = link.call("totpCheck", json!({ "key": key }));
    cx.spawn_in(window, async move |this, cx| {
        let answer = rx.await;
        let _ = this.update_in(cx, |r, window, _| {
            // (A reply for what was typed before doesn't count any more.)
            if r.acc_form.key_seq != mine || r.acc_form.seq != seq {
                return;
            }
            if let Ok(Ok(v)) = answer {
                r.acc_form.key_msg = Some(if flag(&v, "ok") { (r.i18n.tv("am.totpNow", &[("code", &split(s(&v, "code")))]), true) } else { (s(&v, "error").to_string(), false) });
                window.refresh();
            }
        });
    })
    .detach();
}

/// The saved key's code now, and for how long.
fn ask_code(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let (Some(id), Some(link)) = (r.acc_form.id.clone(), r.link.clone()) else { return };
    if r.acc_form.asking {
        return;
    }
    r.acc_form.asking = true;
    let seq = r.acc_form.seq;
    let rx = link.call("totpCode", json!({ "id": id }));
    cx.spawn_in(window, async move |this, cx| {
        let answer = rx.await;
        let _ = this.update_in(cx, |r, window, _| {
            if r.acc_form.seq != seq {
                return;
            }
            r.acc_form.asking = false;
            // (Failed: the clock asks again in 10 s, not every second.)
            r.acc_form.code_retry = Some(Instant::now() + Duration::from_secs(10));
            if let Ok(Ok(v)) = answer {
                if flag(&v, "ok") {
                    let secs = v.get("seconds").and_then(Value::as_u64).unwrap_or(30).max(1);
                    r.acc_form.code = Some((s(&v, "code").to_string(), Instant::now() + Duration::from_secs(secs)));
                    r.acc_form.code_retry = None;
                } else {
                    r.acc_form.code = None;
                    r.acc_form.error = Some(s(&v, "error").to_string()).filter(|e| !e.is_empty());
                }
                window.refresh();
            }
        });
    })
    .detach();
}

/// The eye: dots or the words. The saved password is asked from the app the first time (an authenticator key never
/// comes back from it).
fn reveal(r: &mut Root, at: usize, window: &mut Window, cx: &mut Context<Root>) {
    let now = !r.acc_form.shown[at];
    r.acc_form.shown[at] = now;
    let e = [r.af_cookie.clone(), r.af_password.clone(), r.af_key.clone()][at].clone();
    e.update(cx, |st, cx| st.set_masked(!now, window, cx));
    if now && at == 1 && r.acc_form.pw_loaded.is_none() && e.read(cx).value().is_empty() {
        if let (Some(id), Some(link)) = (r.acc_form.id.clone(), r.link.clone()) {
            let seq = r.acc_form.seq;
            let rx = link.call("reveal", json!({ "id": id, "field": "password" }));
            cx.spawn_in(window, async move |this, cx| {
                let answer = rx.await;
                let _ = this.update_in(cx, |r, window, cx| {
                    if r.acc_form.seq != seq {
                        return;
                    }
                    if let Ok(Ok(Value::String(pw))) = answer {
                        r.acc_form.pw_loaded = Some(pw.clone());
                        // Typed into meanwhile: what the person typed stays, the saved one isn't put over it.
                        if r.af_password.read(cx).value().is_empty() {
                            r.af_password.update(cx, |st, cx| st.set_value(pw, window, cx));
                        }
                    }
                });
            })
            .detach();
        }
    }
    window.refresh();
}

/// Add account, or Save: what the boxes hold goes to the app, which checks it.
pub fn save(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    if !r.acc_form.open || r.acc_form.busy {
        return;
    }
    let val = |e: &Entity<InputState>| e.read(cx).value().to_string();
    let note = r.acc_form.note.as_ref().map(|e| e.read(cx).value().to_string()).unwrap_or_default();
    let (cookie, alias, group, password, key) = (val(&r.af_cookie), val(&r.af_alias), val(&r.af_group), val(&r.af_password), val(&r.af_key).trim().to_string());
    let is_new = r.acc_form.id.is_none();
    if is_new && cookie.trim().is_empty() {
        r.acc_form.error = Some(r.tr("am.needCookie"));
        window.refresh();
        return;
    }
    let mut args = json!({ "group": group, "note": note });
    if is_new {
        args["alias"] = alias.into();
        args["cookie"] = cookie.into();
        args["password"] = password.into();
    } else {
        args["id"] = r.acc_form.id.clone().into();
        if !flag(&r.state, "hideNames") {
            args["alias"] = alias.into();
        }
        // The password as typed; the saved one only shown isn't a change.
        if !password.is_empty() && r.acc_form.pw_loaded.as_deref() != Some(password.as_str()) {
            args["password"] = password.into();
        }
        // A key typed replaces the saved one (the app checks it); Remove sends none; otherwise it stays.
        if !key.is_empty() {
            args["authenticator"] = key.into();
        } else if r.acc_form.remove_key {
            args["authenticator"] = "".into();
        }
    }
    let Some(link) = r.link.clone() else { return };
    r.acc_form.busy = true;
    r.acc_form.error = None;
    let seq = r.acc_form.seq;
    let rx = link.call(if is_new { "addViaCookie" } else { "updateAccount" }, args);
    cx.spawn_in(window, async move |this, cx| {
        let answer = rx.await;
        let _ = this.update_in(cx, |r, window, cx| {
            if r.acc_form.seq != seq {
                return;
            }
            r.acc_form.busy = false;
            match answer {
                Ok(Ok(v)) => {
                    close(r, window, cx);
                    if is_new && v.is_object() {
                        let key = if flag(&v, "merged") { "t.merged" } else { "t.added" };
                        r.toast = Some((r.i18n.tv(key, &[("name", s(&v, "label"))]), "ok".into()));
                        // The new account alone is picked, ready to play.
                        if !s(&v, "id").is_empty() {
                            r.selected.clear();
                            r.selected.insert(s(&v, "id").to_string());
                        }
                    }
                }
                Ok(Err(e)) => r.acc_form.error = Some(e.to_string()),
                Err(_) => {}
            }
            window.refresh();
            cx.notify();
        });
    })
    .detach();
}
