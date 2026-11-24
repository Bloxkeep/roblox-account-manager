// Add account: the sign-in screen (ui/src/PasswordLogin.tsx; app.js's passwordLogin). Username and password, with the
// other ways around it: the phone's code first (no password, no puzzle), Roblox's own browser sign-in, creating an
// account, a pasted cookie, a Bloxkeep file. The values go to the app, which signs in on Roblox's (or Roblox VNG's)
// own host ("loginPassword": { status, error, seconds, methods, media, autofill, added, closed }): it may want a 2-step
// code (authenticator, email, text message; "loginTwoStep", "loginTwoStepSend", and "loginTwoStepCode" for a key saved
// here), a puzzle in a window of its own ("loginCaptcha"), or only another try at loading the account ("loginFinish").
// Back, and closing, end the attempt in the app too ("loginCancel": the password goes).
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use gpui_kit::component::WindowExt as _;
use gpui_kit::component::dialog::DialogEntrance;
use gpui_kit::component::input::InputState;
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use crate::{kit::*, shell::Root, theme::*};
use crate::val::s;

#[derive(Clone, Copy, PartialEq, Default)]
enum Step {
    #[default]
    Login,
    TwoStep,
    Captcha,
    Finish,
}

/// How long until "Resend" works again.
const RESEND: Duration = Duration::from_secs(30);

#[derive(Default)]
pub struct Data {
    open: bool,
    /// Which opening this is: an answer or a countdown of a box closed and opened again is dropped.
    seq: u64,
    vng: bool,
    step: Step,
    err: Option<String>,
    /// The password shown as it is.
    shown: bool,
    /// The name being signed in (the code screen says it).
    user: String,
    /// The 2-step ways this account has that take a typed code, the one shown, the ways a code went out by and when
    /// each can be sent again, and when Verify works again after too many tries.
    methods: Vec<String>,
    method: String,
    sent: HashSet<String>,
    resend_at: HashMap<String, Instant>,
    verify_at: Option<Instant>,
    /// A key saved here fills the authenticator's code in (never sent by itself); what it filled, and which filling
    /// is the latest.
    autofill: bool,
    filled: String,
    fill_n: u64,
    /// A request on its way: nothing that would send another works, so one click never sends the password twice.
    pending: bool,
    sending: bool,
    added: Vec<(String, String)>,
}

fn left(at: Option<Instant>) -> u64 {
    at.map(|t| t.saturating_duration_since(Instant::now())).filter(|d| !d.is_zero()).map_or(0, |d| d.as_secs().max(1))
}

fn label_of(method: &str) -> &'static str {
    match method {
        "email" => "pw.email",
        "sms" => "pw.sms",
        _ => "pw.auth",
    }
}

/// Leaving a code screen: what was sent, and the waits, belong to that sign-in, not to the next one.
fn reset_two(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let p = &mut r.pwl;
    p.sent.clear();
    p.resend_at.clear();
    p.verify_at = None;
    p.autofill = false;
    p.filled.clear();
    p.fill_n += 1;
    r.pl_code.update(cx, |st, cx| st.set_value("", window, cx));
}

/// A screen drawn anew: its words, and its first box with the keyboard. Back on the first screen the password goes
/// with the attempt that used it (the username stays).
fn draw(r: &mut Root, step: Step, err: String, window: &mut Window, cx: &mut Context<Root>) {
    r.pwl.step = step;
    r.pwl.err = Some(err).filter(|e| !e.is_empty());
    if step == Step::Login {
        r.pl_pass.update(cx, |st, cx| st.set_value("", window, cx));
    }
    let first = match step {
        Step::Login if r.pl_user.read(cx).value().trim().is_empty() => Some(r.pl_user.clone()),
        Step::Login => Some(r.pl_pass.clone()),
        Step::TwoStep => Some(r.pl_code.clone()),
        _ => None,
    };
    if let Some(e) = first {
        let h = e.read(cx).focus_handle(cx);
        window.focus(&h, cx);
    }
    window.refresh();
}

/// One request at a time; the app's refusal is said under the boxes.
fn ask(r: &mut Root, cmd: &'static str, args: Value, window: &mut Window, cx: &mut Context<Root>, done: impl FnOnce(&mut Root, Value, &mut Window, &mut Context<Root>) + 'static) {
    if r.pwl.pending {
        return;
    }
    let Some(link) = r.link.clone() else { return };
    r.pwl.pending = true;
    r.pwl.err = None;
    window.refresh();
    let rx = link.call(cmd, args);
    let seq = r.pwl.seq;
    cx.spawn_in(window, async move |this, cx| {
        let answer = rx.await;
        let _ = this.update_in(cx, |r, window, cx| {
            if r.pwl.seq != seq {
                return;
            }
            r.pwl.pending = false;
            if r.pwl.open {
                match answer {
                    Ok(Ok(v)) => done(r, v, window, cx),
                    Ok(Err(e)) => r.pwl.err = Some(e),
                    Err(_) => {}
                }
            }
            window.refresh();
            cx.notify();
        });
    })
    .detach();
}

/// What the app said about the sign-in.
fn handle(r: &mut Root, v: Value, window: &mut Window, cx: &mut Context<Root>) {
    match s(&v, "status") {
        // The attempt is over (wrong password, account locked, too many tries): back to the first step with the reason.
        "login" => {
            reset_two(r, window, cx);
            draw(r, Step::Login, s(&v, "error").to_string(), window, cx);
        }
        // Signed in, but the account couldn't be looked up: "Try again" only does that.
        "finish" => {
            reset_two(r, window, cx);
            draw(r, Step::Finish, s(&v, "error").to_string(), window, cx);
        }
        // Too many codes tried: the code screen stays, Verify waits.
        "wait" => {
            r.pwl.verify_at = Some(Instant::now() + Duration::from_secs(v.get("seconds").and_then(Value::as_u64).unwrap_or(60)));
            r.pwl.err = None;
        }
        // (Solved, and Roblox wants another one: said, or it looks like nothing happened.)
        "captcha" => {
            let again = if r.pwl.step == Step::Captcha { r.tr("pw.captchaAgain") } else { String::new() };
            draw(r, Step::Captcha, again, window, cx);
        }
        "twostep" => {
            reset_two(r, window, cx);
            let known: Vec<String> = v.get("methods").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).filter(|m| ["authenticator", "email", "sms"].contains(m)).map(str::to_string).collect()).unwrap_or_default();
            let list = if known.is_empty() { vec!["authenticator".to_string(), "email".to_string()] } else { known };
            // Roblox may have sent the code a given way already: that tab first.
            let first = if list.iter().any(|m| m == s(&v, "media")) { s(&v, "media").to_string() } else { list[0].clone() };
            if first != "authenticator" {
                r.pwl.sent.insert(first.clone());
                r.pwl.resend_at.insert(first.clone(), Instant::now() + RESEND);
            }
            r.pwl.autofill = v.get("autofill").and_then(Value::as_bool).unwrap_or(false);
            r.pwl.methods = list;
            r.pwl.method = first;
            draw(r, Step::TwoStep, String::new(), window, cx);
            fill_code(r, window, cx);
        }
        "added" => {
            let a = v.get("added").cloned().unwrap_or(Value::Null);
            let name = s(&a, "label").to_string();
            let key = if a.get("merged").and_then(Value::as_bool).unwrap_or(false) { "t.merged" } else { "t.added" };
            r.toast = Some((r.i18n.tv(key, &[("name", &name)]), "ok".into()));
            r.pwl.added.push((s(&a, "id").to_string(), name));
            reset_two(r, window, cx);
            r.pl_user.update(cx, |st, cx| st.set_value("", window, cx));
            draw(r, Step::Login, String::new(), window, cx);
        }
        _ => {}
    }
}

/// The saved key's code in the box, following it as it changes while the box is untouched.
fn fill_code(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    r.pwl.fill_n += 1;
    let (mine, seq) = (r.pwl.fill_n, r.pwl.seq);
    let untouched = |r: &Root, cx: &App| {
        let now = r.pl_code.read(cx).value().to_string();
        now.is_empty() || now == r.pwl.filled
    };
    if !r.pwl.autofill || r.pwl.method != "authenticator" || !untouched(r, cx) {
        return;
    }
    let Some(link) = r.link.clone() else { return };
    let rx = link.call("loginTwoStepCode", json!({}));
    cx.spawn_in(window, async move |this, cx| {
        let Ok(Ok(v)) = rx.await else { return };
        let code = s(&v, "code").to_string();
        let secs = v.get("seconds").and_then(Value::as_u64).unwrap_or(30).max(1);
        let took = this.update_in(cx, |r, window, cx| {
            if !r.pwl.open || r.pwl.seq != seq || r.pwl.fill_n != mine || code.is_empty() || r.pwl.method != "authenticator" || !untouched(r, cx) {
                return false;
            }
            r.pwl.filled = code.clone();
            r.pl_code.update(cx, |st, cx| st.set_value(code.clone(), window, cx));
            window.refresh();
            true
        });
        if !matches!(took, Ok(true)) {
            return;
        }
        cx.background_executor().timer(Duration::from_millis(secs * 1000 + 300)).await;
        let _ = this.update_in(cx, |r, window, cx| {
            if r.pwl.open && r.pwl.seq == seq && r.pwl.fill_n == mine {
                fill_code(r, window, cx);
            }
        });
    })
    .detach();
}

/// Something was typed (2: the code box, which keeps digits only, six at most): the old problem goes.
pub fn typed(r: &mut Root, which: u8, window: &mut Window, cx: &mut Context<Root>) {
    if !r.pwl.open {
        return;
    }
    r.pwl.err = None;
    if which == 2 {
        let now = r.pl_code.read(cx).value().to_string();
        let clean: String = now.chars().filter(char::is_ascii_digit).take(6).collect();
        if clean != now {
            r.pl_code.update(cx, |st, cx| st.set_value(clean, window, cx));
        }
    }
    window.refresh();
}

/// Enter in a box: Sign in, or Verify.
pub fn submit(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    if !r.pwl.open {
        return;
    }
    match r.pwl.step {
        Step::Login => {
            let (name, pass) = (r.pl_user.read(cx).value().trim().to_string(), r.pl_pass.read(cx).value().to_string());
            if name.is_empty() || pass.is_empty() {
                r.pwl.err = Some(r.tr("pw.need"));
                window.refresh();
                return;
            }
            r.pwl.user = name.clone();
            let realm = if r.pwl.vng { "vng" } else { "" };
            ask(r, "loginPassword", json!({ "username": name, "password": pass, "realm": realm }), window, cx, handle);
        }
        Step::TwoStep => {
            let code = r.pl_code.read(cx).value().trim().to_string();
            if code.chars().count() < 6 {
                r.pwl.err = Some(r.tr(if code.is_empty() { "pw.needCode" } else { "pw.needCode6" }));
                window.refresh();
                return;
            }
            if left(r.pwl.verify_at) > 0 {
                return;
            }
            let method = r.pwl.method.clone();
            ask(r, "loginTwoStep", json!({ "method": method, "code": code }), window, cx, handle);
        }
        _ => {}
    }
}

/// A code by email or text message: asked for, then "Resend" waits half a minute.
fn send_code(r: &mut Root, via: String, window: &mut Window, cx: &mut Context<Root>) {
    let p = &r.pwl;
    if p.sending || left(p.verify_at) > 0 || (p.sent.contains(&via) && left(p.resend_at.get(&via).copied()) > 0) {
        return;
    }
    let Some(link) = r.link.clone() else { return };
    r.pwl.sending = true;
    r.pwl.err = None;
    window.refresh();
    let rx = link.call("loginTwoStepSend", json!({ "method": via }));
    let seq = r.pwl.seq;
    cx.spawn_in(window, async move |this, cx| {
        let answer = rx.await;
        let _ = this.update_in(cx, |r, window, cx| {
            if r.pwl.seq != seq {
                return;
            }
            r.pwl.sending = false;
            if r.pwl.open {
                match answer {
                    Ok(Ok(v)) if s(&v, "status") == "sent" => {
                        r.pwl.sent.insert(via.clone());
                        r.pwl.resend_at.insert(via.clone(), Instant::now() + RESEND);
                    }
                    Ok(Ok(v)) => handle(r, v, window, cx),
                    Ok(Err(e)) => r.pwl.err = Some(e),
                    Err(_) => {}
                }
            }
            window.refresh();
        });
    })
    .detach();
}

/// Closing: the app drops the half-done sign-in and its password; what was added is picked.
fn close(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    if r.pwl.open {
        r.pwl.open = false;
        r.pwl.fill_n += 1;
        r.send("loginCancel", json!({}));
    }
    for e in [r.pl_pass.clone(), r.pl_code.clone()] {
        e.update(cx, |st, cx| st.set_value("", window, cx));
    }
    if !r.pwl.added.is_empty() {
        r.selected = r.pwl.added.iter().map(|(id, _)| id.clone()).filter(|id| !id.is_empty()).collect();
    }
    window.close_dialog(cx);
    cx.notify();
}

/// Another way picked: this closes, and that way opens.
fn way(r: &mut Root, name: &'static str, window: &mut Window, cx: &mut Context<Root>) {
    let vng = r.pwl.vng;
    close(r, window, cx);
    match name {
        "browser" => r.add_browser(cx),
        "create" => r.add_browser_as(true, cx),
        "phone" => crate::pages::quick::open(r, vng, window, cx),
        "cookie" => crate::pages::account_form::open(r, None, window, cx),
        "vault" => crate::pages::import_vault::open(r, window, cx),
        _ => {}
    }
}

/// The wide yellow button, flat like every main button (kit Button primary).
fn block(el: impl IntoElement) -> Div {
    div().w_full().child(el)
}

/// The wide yellow button of a screen; with a request on its way it waits (a spinner) and takes no click.
fn main_button(id: &'static str, label: String, off: bool, busy: bool) -> Stateful<Div> {
    focusable(div().id(id))
        .h(px(40.))
        .w_full()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(8.))
        .rounded(px(R_M + 1.))
        .bg(c(FACE))
        .text_color(c(INK))
        .text_size(px(T_L))
        .font_weight(FontWeight::SEMIBOLD)
        .map(|d| if busy { d.opacity(0.75).child(spinner(14., c(INK))) } else if off { d.opacity(0.5) } else { d.cursor_pointer().hover(|d| d.bg(c(FACE_HI))).active(|d| d.bg(c(FACE_DOWN))) })
        .child(label)
}

pub fn open(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let seq = r.pwl.seq.wrapping_add(1);
    r.pwl = Data { open: true, seq, methods: vec!["authenticator".into(), "email".into()], method: "authenticator".into(), ..Default::default() };
    r.help_open = true;
    // Each box says what goes in it, in the language of the moment (made once at the start, before it was known; with
    // only an icon the boxes read as empty, review 9.2).
    let hints = [r.tr("pw.userHint"), r.tr("pw.passHint"), "000000".to_string()];
    for ((e, masked), hint) in [(r.pl_user.clone(), false), (r.pl_pass.clone(), true), (r.pl_code.clone(), false)].into_iter().zip(hints) {
        e.update(cx, |st, cx| {
            st.set_value("", window, cx);
            st.set_masked(masked, window, cx);
            st.set_placeholder(hint, window, cx);
        });
    }
    let h = r.pl_user.read(cx).focus_handle(cx);
    window.focus(&h, cx);
    // The countdowns ("Resend in 25 s", "Verify in 40 s") are drawn again each second while one runs.
    cx.spawn_in(window, async move |this, cx| {
        loop {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let going = this.update_in(cx, |r, window, _| {
                let p = &r.pwl;
                if p.open && p.seq == seq && p.step == Step::TwoStep && (left(p.verify_at) > 0 || left(p.resend_at.get(&p.method).copied()) > 0) {
                    window.refresh();
                }
                // (Closed and opened again: the new box runs its own loop.)
                p.open && p.seq == seq
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
        let r = root.read(cx);
        let p = &r.pwl;
        let verify_wait = left(p.verify_at);
        let said = if p.step == Step::TwoStep && verify_wait > 0 { Some(r.i18n.tv("pw.waitMsg", &[("s", &verify_wait.to_string())])) } else { p.err.clone() };
        // (The app's refusal in a red box, as the web page's .form-error: a security key it can't do reads as a stop,
        // not as one more line of the form.)
        let error = said.map(form_error);
        let label = |key: &str| div().text_size(px(T_S)).font_weight(FontWeight::MEDIUM).text_color(c(TEXT_2)).child(r.tr(key));
        let quiet_link = |id: &'static str, text: String| focusable(div().id(id)).text_size(px(T_M)).text_color(c(TEXT_2)).cursor_pointer().hover(|st| st.text_color(c(TEXT))).child(text);
        // Back ends this attempt in the app too: the password goes, and a reply still on its way is dropped.
        let back = {
            let this = this.clone();
            Button::new("pw-back", r.tr("pw.back")).quiet().on_click(move |_, w, cx| {
                let _ = this.update(cx, |r, cx| {
                    r.send("loginCancel", json!({}));
                    r.pwl.pending = false;
                    reset_two(r, w, cx);
                    draw(r, Step::Login, String::new(), w, cx);
                });
            })
        };
        let badge = |icon_name: &'static str, title: String, text: String| {
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(8.))
                .child(icon(icon_name, 26., c(FACE)))
                .child(div().text_size(px(T_SEC)).font_weight(FontWeight::SEMIBOLD).child(title))
                .child(div().text_size(px(T_M)).line_height(px(20.)).text_color(c(TEXT_2)).text_center().child(text))
        };
        let mut body = div().flex().flex_col().gap(px(14.));
        match p.step {
            Step::Captcha | Step::Finish => {
                let captcha = p.step == Step::Captcha;
                let go = this.clone();
                body = body
                    .child(badge(if captcha { "shield" } else { "shieldCheck" }, r.tr(if captcha { "pw.captchaTitle" } else { "pw.finishTitle" }), r.tr(if captcha { "pw.captcha" } else { "pw.finish" })))
                    .children(error)
                    .child(block(main_button("pw-solve", r.tr(if captcha { "pw.solve" } else { "pw.retry" }), false, p.pending).on_click(move |_, w, cx| {
                        let _ = go.update(cx, |r, cx| {
                            ask(r, if captcha { "loginCaptcha" } else { "loginFinish" }, json!({}), w, cx, |r, v, w, cx| {
                                // The puzzle window was closed: "Solve" asks for a fresh one.
                                if s(&v, "status") == "captcha" && v.get("closed").and_then(Value::as_bool) == Some(true) {
                                    r.pwl.err = Some(r.tr("pw.captchaClosed"));
                                } else {
                                    handle(r, v, w, cx);
                                }
                            })
                        });
                    })))
                    .child(div().flex().justify_center().child(back));
            }
            Step::TwoStep => {
                body = body.child(badge("shieldCheck", r.tr("pw.twoTitle"), r.i18n.tv("pw.twoLead", &[("name", &p.user)])));
                if p.methods.len() > 1 {
                    let mut tabs = crate::kit::slide("pw-method", div().flex().gap(px(2.)).p(px(3.)).rounded(px(R_M)).bg(c(INSET)));
                    for m in p.methods.clone() {
                        let this = this.clone();
                        let on = p.method == m;
                        tabs = tabs.child(segment(SharedString::from(format!("pw-m-{m}")), on).flex_1().h(px(28.)).justify_center().text_size(px(T_M)).cursor_pointer().child(r.tr(label_of(&m))).on_click(move |_, w, cx| {
                            let m = m.clone();
                            let _ = this.update(cx, |r, cx| {
                                r.pwl.err = None;
                                r.pwl.method = m.clone();
                                // Email and text-message codes go out on their own: another way gets one sent when it's picked.
                                if m != "authenticator" && !r.pwl.sent.contains(&m) {
                                    send_code(r, m.clone(), w, cx);
                                }
                                // A filled-in authenticator code isn't an email or text-message code: it goes with another way.
                                if m != "authenticator" && !r.pwl.filled.is_empty() && r.pl_code.read(cx).value().as_ref() == r.pwl.filled {
                                    r.pl_code.update(cx, |st, cx| st.set_value("", w, cx));
                                }
                                fill_code(r, w, cx);
                                let h = r.pl_code.read(cx).focus_handle(cx);
                                w.focus(&h, cx);
                            });
                            w.refresh();
                        }));
                    }
                    body = body.child(tabs);
                }
                body = body.child(div().flex().flex_col().gap(px(6.)).child(label("pw.code")).child(field(&r.pl_code, None)));
                let typed = r.pl_code.read(cx).value().to_string();
                if !p.filled.is_empty() && typed == p.filled {
                    body = body.child(div().text_size(px(T_S)).text_color(c(MUTED)).child(r.tr("pw.filled")));
                }
                if p.method != "authenticator" {
                    let was_sent = p.sent.contains(&p.method);
                    let resend = left(p.resend_at.get(&p.method).copied());
                    let (send, via) = (this.clone(), p.method.clone());
                    let words = if !was_sent { r.tr("pw.send") } else if resend > 0 { r.i18n.tv("pw.resendIn", &[("s", &resend.to_string())]) } else { r.tr("pw.resend") };
                    body = body.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .child(div().min_w_0().flex_1().text_size(px(T_M)).text_color(c(MUTED)).child(if was_sent { r.tr(if p.method == "sms" { "pw.sentSms" } else { "pw.sentEmail" }) } else { String::new() }))
                            .child(quiet_link("pw-send", words).on_click(move |_, w, cx| {
                                let via = via.clone();
                                let _ = send.update(cx, |r, cx| send_code(r, via, w, cx));
                            })),
                    );
                }
                let go = this.clone();
                let words = if verify_wait > 0 { r.i18n.tv("pw.verifyIn", &[("s", &verify_wait.to_string())]) } else { r.tr("pw.verify") };
                body = body
                    .children(error)
                    .child(block(main_button("pw-verify", words, verify_wait > 0, p.pending).on_click(move |_, w, cx| {
                        let _ = go.update(cx, |r, cx| submit(r, w, cx));
                    })))
                    .child(div().flex().justify_center().child(back));
            }
            Step::Login => {
                let mut realm = crate::kit::slide("pw-realm", div().flex().gap(px(2.)).p(px(3.)).rounded(px(R_M)).bg(c(INSET)));
                for (words, vng) in [("Roblox", false), ("Roblox VNG", true)] {
                    let this = this.clone();
                    realm = realm.child(segment(words, p.vng == vng).flex_1().h(px(28.)).justify_center().text_size(px(T_M)).cursor_pointer().child(words).on_click(move |_, w, cx| {
                        let _ = this.update(cx, |r, cx| {
                            // (The answer on its way is for the realm it was sent to.)
                            if !r.pwl.pending && r.pwl.vng != vng {
                                r.pwl.err = None;
                                r.pwl.vng = vng;
                                r.pwl.shown = false;
                                r.pl_pass.update(cx, |st, cx| st.set_masked(true, w, cx));
                            }
                        });
                        w.refresh();
                    }));
                }
                body = body.child(realm);
                if !p.added.is_empty() {
                    let done = this.clone();
                    let names = p.added.iter().map(|(_, n)| n.clone()).collect::<Vec<_>>().join(", ");
                    body = body.child(
                        div()
                            .px(px(12.))
                            .py(px(9.))
                            .rounded(px(R_M))
                            .bg(ca(OK, 0.12))
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .child(icon("check", 15., c(OK)))
                            .child(div().min_w_0().flex_1().text_size(px(T_M)).text_color(c(TEXT)).child(r.i18n.tv("pw.addedList", &[("names", &names)])))
                            .child(Button::new("pw-done", r.tr("common.done")).primary().small().on_click(move |_, w, cx| {
                                let _ = done.update(cx, |r, cx| close(r, w, cx));
                            })),
                    );
                }
                // The phone first: no password, no puzzle.
                let phone = this.clone();
                body = body.child(
                    // (app.css .start-opt.rec: the recommended way has the stronger edge and the tag.)
                    card("pw-phone")
                        .edge(ca(TEXT, 0.34), ca(TEXT, 0.5))
                        .px(px(14.))
                        .py(px(12.))
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .cursor_pointer()
                        .child(div().size(px(36.)).flex_none().rounded(px(R_M)).border_1().border_color(ca(WHITE, 0.14)).flex().items_center().justify_center().child(icon("qr", 20., c(TEXT))))
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(div().flex().items_center().gap(px(8.)).child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).child(r.tr("start.phone"))).child(div().h(px(18.)).px(px(8.)).rounded_full().bg(c(OK)).flex().items_center().text_size(px(T_XS)).font_weight(FontWeight::SEMIBOLD).text_color(c(INK)).child(r.tr("start.rec"))))
                                .child(div().text_size(px(T_S)).text_color(c(MUTED)).child(r.tr("start.phoneSub"))),
                        )
                        .child(icon("chevronRight", 15., c(MUTED)))
                        .on_click(move |_, w, cx| {
                            let _ = phone.update(cx, |r, cx| way(r, "phone", w, cx));
                        }),
                );
                body = body.child(div().flex().items_center().gap(px(10.)).child(div().flex_1().h(px(1.)).bg(c(LINE))).child(div().text_size(px(T_S)).text_color(c(MUTED)).child(r.tr("pw.orPw"))).child(div().flex_1().h(px(1.)).bg(c(LINE))));
                let (eye, go) = (this.clone(), this.clone());
                body = body
                    .child(div().flex().flex_col().gap(px(6.)).child(label("pw.user")).child(field(&r.pl_user, Some("user"))))
                    .child(
                        div().flex().flex_col().gap(px(6.)).child(label("pw.pass")).child(
                            field(&r.pl_pass, Some("lock")).w_full().when(p.err.is_some(), |d| d.border_color(ca(BAD, 0.55))).child(
                                IconButton::new("pw-eye", if p.shown { "eyeOff" } else { "eye" }).size(30.).tip(r.tr(if p.shown { "lock.hidePw" } else { "lock.showPw" })).on_click(move |_, w, cx| {
                                    let _ = eye.update(cx, |r, cx| {
                                        r.pwl.shown = !r.pwl.shown;
                                        let masked = !r.pwl.shown;
                                        r.pl_pass.update(cx, |st, cx| st.set_masked(masked, w, cx));
                                    });
                                    w.refresh();
                                }),
                            ),
                        ),
                    )
                    .children(error)
                    .child(block(main_button("pw-go", r.tr("pw.go"), false, p.pending).on_click(move |_, w, cx| {
                        let _ = go.update(cx, |r, cx| submit(r, w, cx));
                    })))
                    .child(div().text_size(px(T_S)).text_color(c(MUTED)).text_center().child(r.tr("pw.extra")));
                // The other ways. (Roblox's browser sign-in and sign-up are roblox.com's: not for Roblox VNG.)
                // (app.css .ways: "Other ways:" on a line of its own, the ways under it as small outlined buttons, in the
                // middle.)
                // (Two even columns of flat blocks, each with its icon, so the four ways line up instead of ragged pills.)
                let mut ways = div().w_full().flex().flex_col().gap(px(8.));
                let offered: Vec<(&'static str, &'static str, &'static str)> = if p.vng {
                    vec![("cookie", "pw.cookie", "cookie"), ("vault", "pw.vault", "folder")]
                } else {
                    vec![("browser", "pw.withBrowser", "globe"), ("create", "pw.create", "userPlus"), ("cookie", "pw.cookie", "cookie"), ("vault", "pw.vault", "folder")]
                };
                for pair in offered.chunks(2) {
                    let mut line = div().w_full().flex().gap(px(8.));
                    for (name, key, ic) in pair.iter().copied() {
                        let this = this.clone();
                        line = line.child(
                            hot(name)
                                .flex_1()
                                .min_w_0()
                                .h(px(40.))
                                .px(px(12.))
                                .rounded(px(R_M))
                                .border_1()
                                .tones(ca(WHITE, 0.03), ca(WHITE, 0.07))
                                .edge(c(LINE), ca(WHITE, 0.17))
                                .flex()
                                .items_center()
                                .gap(px(9.))
                                .cursor_pointer()
                                .child(icon(ic, 15., c(TEXT_2)))
                                .child(div().flex_1().min_w_0().truncate().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).text_color(c(TEXT)).child(r.tr(key)))
                                .on_click(move |_, w, cx| {
                                    let _ = this.update(cx, |r, cx| way(r, name, w, cx));
                                }),
                        );
                    }
                    ways = ways.child(line);
                }
                body = body.child(div().flex().flex_col().items_center().gap(px(8.)).child(div().text_size(px(T_S)).text_color(c(MUTED)).child(r.tr("start.more"))).child(ways)).child(
                    // (.auth-note: across the whole dialog, under a hairline.)
                    div()
                        .mx(px(-28.))
                        .mt(px(2.))
                        .px(px(28.))
                        .pt(px(14.))
                        .flex()
                        .gap(px(9.))
                        .border_t_1()
                        .border_color(c(LINE))
                        .text_size(px(T_S))
                        .line_height(px(18.))
                        .text_color(c(MUTED))
                        .child(div().mt(px(2.)).flex_none().child(icon("lock", 13., c(MUTED))))
                        .child(div().min_w_0().flex_1().child(r.tr("pw.note"))),
                );
            }
        }
        let shut = this.clone();
        framed_center(dialog, r.tr("pw.title"), r.tr("common.close"), move |_, w, cx| {
            // (Asks first when it would throw away what was typed, or a sign-in half way: PasswordLogin.tsx's
            // asksToClose.)
            let _ = shut.update(cx, |r, cx| {
                // A step on its way: its answer may close or redraw the dialog, which would shut a "Discard?" box up
                // over it instead. Closed once it has answered.
                if r.pwl.pending {
                    return;
                }
                let typed = !r.pl_user.read(cx).value().trim().is_empty() || !r.pl_pass.read(cx).value().is_empty();
                if !matches!(r.pwl.step, Step::Login | Step::Finish) || typed {
                    r.ask_discard(w, cx, close);
                } else {
                    close(r, w, cx);
                }
            });
        })
        .w(px(DIALOG_AUTH_W))
        .centered()
        // (A stray click beside it mustn't end a sign-in half way: only its buttons and the × close it.)
        .overlay_closable(false)
        .keyboard(false)
        .entrance(DialogEntrance::FadeSlide)
        .child(div().w_full().px(px(28.)).pt(px(10.)).pb(px(22.)).child(body))
    });
}

/// The boxes the sign-in screen uses (the shell makes them once): the username, the password, the 2-step code.
pub fn boxes(window: &mut Window, cx: &mut Context<Root>) -> (Entity<InputState>, Entity<InputState>, Entity<InputState>) {
    let mut made = Vec::new();
    for (which, masked, hint) in [(0u8, false, ""), (1u8, true, ""), (2u8, false, "000000")] {
        let e = cx.new(|cx| InputState::new(window, cx).masked(masked).placeholder(hint));
        cx.subscribe_in(&e, window, move |r: &mut Root, _, ev: &gpui_kit::component::input::InputEvent, window, cx| match ev {
            gpui_kit::component::input::InputEvent::Change => typed(r, which, window, cx),
            gpui_kit::component::input::InputEvent::PressEnter { .. } => submit(r, window, cx),
            _ => {}
        })
        .detach();
        made.push(e);
    }
    let code = made.pop().unwrap();
    let pass = made.pop().unwrap();
    let user = made.pop().unwrap();
    (user, pass, code)
}
