// Accounts from another Bloxkeep file (ui/src/ImportVault.tsx; app.js's importVault): pick the file (a backup, or
// vault.rmv from another PC), type that file's own master password, tick what to bring over. The app keeps the opened
// file ("vaultImportPick": { name, wait }, "vaultImportOpen": { status, token, accounts, presets, error, wait },
// "vaultImportApply": { added, updated, presets, presetsSkipped, ids }); this window sees only names and states, never
// cookies or passwords. Closing without importing makes the app forget the file ("vaultImportCancel"). After three
// wrong passwords the app makes each try wait, and the button counts down meanwhile.
//
// The same box brings accounts from Roblox Account Manager's AccountData.json ("vaultImportPick" { from: "ram" }: its
// password asked only when the file has one) and from a pasted list of cookies ("listImportOpen" { text }: one a line,
// or username:password:cookie; the app checks each with Roblox and answers as "vaultImportOpen" does, with how many
// lines it left out).
use std::collections::HashSet;
use std::time::{Duration, Instant};

use gpui_kit::component::WindowExt as _;
use gpui_kit::component::dialog::DialogEntrance;
use gpui_kit::component::input::TextareaState;
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use crate::{kit::*, shell::Root, theme::*};
use crate::val::s;
use crate::val::flag;

/// Where the accounts come from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    Vault,
    /// Roblox Account Manager's AccountData.json.
    Ram,
    /// A pasted list of cookies.
    List,
}

#[derive(Default)]
pub struct Data {
    open: bool,
    kind: Kind,
    /// 0 pick the file (or paste the list), 1 its password, 2 what to bring over.
    step: u8,
    /// The picked file needs a password (a Roblox Account Manager file saved without one doesn't).
    locked: bool,
    /// The pasted list's box.
    list: Option<Entity<TextareaState>>,
    /// Lines of the list left out of the preview.
    skipped: i64,
    file: String,
    preview: Option<Value>,
    ticked: HashSet<i64>,
    presets: bool,
    /// The password shown as it is.
    shown: bool,
    error: Option<String>,
    wait_until: Option<Instant>,
    busy: bool,
    /// Where the list of the file's accounts is scrolled to.
    list_scroll: ScrollHandle,
    /// Which opening this is: the countdown loop of one closed and opened again at once stops (two ran side by side).
    opened: u64,
}

fn accounts(r: &Root) -> Vec<Value> {
    r.import.preview.as_ref().and_then(|p| p.get("accounts")).and_then(Value::as_array).cloned().unwrap_or_default()
}

fn key_of(a: &Value) -> i64 {
    a.get("key").and_then(Value::as_i64).unwrap_or(-1)
}

/// Seconds still to wait before the next try, when the app asked for a wait.
fn wait_left(r: &Root) -> Option<u64> {
    r.import.wait_until.map(|t| t.saturating_duration_since(Instant::now())).filter(|d| !d.is_zero()).map(|d| d.as_secs().max(1))
}

/// Names hidden (Settings): the file's accounts too, by their first letter as the page's are (no numbers).
fn shown_name(root: &Root, a: &Value) -> String {
    let name = s(a, "label");
    if flag(&root.state, "hideNames") { crate::val::first_only(name) } else { name.to_string() }
}

/// A long file name with its middle left out, so its start and its ".rmv" both show.
fn short_name(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    if chars.len() <= 44 {
        return name.to_string();
    }
    let head: String = chars[..26].iter().collect();
    let tail: String = chars[chars.len() - 15..].iter().collect();
    format!("{head}…{tail}")
}

/// The accounts the search shows (every one while its box is empty).
fn listed(root: &Root, cx: &App) -> Vec<Value> {
    let q = folded(root.iv_q.read(cx).value().trim());
    let hide = flag(&root.state, "hideNames");
    accounts(root)
        .into_iter()
        .filter(|a| q.is_empty() || folded(&shown_name(root, a)).contains(&q) || (!hide && folded(s(a, "username")).contains(&q)) || folded(s(a, "group")).contains(&q))
        .collect()
}

fn call(r: &mut Root, cmd: &'static str, args: Value, window: &mut Window, cx: &mut Context<Root>, done: impl FnOnce(&mut Root, Value, &mut Window, &mut Context<Root>) + 'static) {
    let Some(link) = r.link.clone() else { return };
    r.import.busy = true;
    r.import.error = None;
    window.refresh();
    let rx = link.call(cmd, args);
    // (An answer for a box closed and opened again since is the old box's: dropped.)
    let opened = r.import.opened;
    cx.spawn_in(window, async move |this, cx| {
        let answer = rx.await;
        let _ = this.update_in(cx, |r, window, cx| {
            if r.import.opened != opened {
                return;
            }
            r.import.busy = false;
            if r.import.open {
                match answer {
                    Ok(Ok(v)) => done(r, v, window, cx),
                    Ok(Err(e)) => r.import.error = Some(e),
                    Err(_) => {}
                }
            }
            window.refresh();
            cx.notify();
        });
    })
    .detach();
}

/// Choose file…: the app's file dialog; a file picked goes on to its password.
fn pick(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let from = if r.import.kind == Kind::Ram { "ram" } else { "vault" };
    call(r, "vaultImportPick", json!({ "from": from }), window, cx, |r, v, window, cx| {
        // (The file dialog was cancelled: things stay as they were.)
        if !v.is_object() {
            return;
        }
        r.import.file = s(&v, "name").to_string();
        r.import.preview = None;
        r.import.shown = false;
        r.import.locked = v.get("locked").and_then(Value::as_bool).unwrap_or(true);
        // A Roblox Account Manager file without a password opens at once.
        if !r.import.locked {
            r.import.step = 0;
            return open_file(r, String::new(), window, cx);
        }
        r.import.step = 1;
        r.import.wait_until = v.get("wait").and_then(Value::as_u64).filter(|w| *w > 0).map(|w| Instant::now() + Duration::from_secs(w));
        for e in [r.iv_pw.clone(), r.iv_q.clone()] {
            e.update(cx, |st, cx| st.set_value("", window, cx));
        }
        r.iv_pw.update(cx, |st, cx| st.set_masked(true, window, cx));
        let h = r.iv_pw.read(cx).focus_handle(cx);
        window.focus(&h, cx);
    });
}

/// The main button: Choose file…, Open, or Import, by the step. Enter in the password box is Open.
pub fn next(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    if !r.import.open || r.import.busy {
        return;
    }
    match r.import.step {
        0 if r.import.kind == Kind::List => {
            let text = r.import.list.as_ref().map(|e| e.read(cx).value().to_string()).unwrap_or_default();
            if text.trim().is_empty() {
                r.import.error = Some(r.tr("il.need"));
                window.refresh();
                return;
            }
            call(r, "listImportOpen", json!({ "text": text }), window, cx, opened);
        }
        0 => pick(r, window, cx),
        1 => {
            if wait_left(r).is_some() {
                return;
            }
            let password = r.iv_pw.read(cx).value().to_string();
            if password.is_empty() {
                r.import.error = Some(r.tr(if r.import.kind == Kind::Ram { "ir.needPw" } else { "iv.needPw" }));
                window.refresh();
                return;
            }
            open_file(r, password, window, cx);
        }
        _ => {
            let Some(p) = r.import.preview.clone() else { return };
            let keys: Vec<i64> = accounts(r).iter().map(key_of).filter(|k| r.import.ticked.contains(k)).collect();
            if keys.is_empty() && !r.import.presets {
                return;
            }
            let presets = r.import.presets;
            call(r, "vaultImportApply", json!({ "token": s(&p, "token"), "keys": keys, "presets": presets }), window, cx, |r, v, window, cx| {
                // (Done: the app has let go of the file itself.)
                r.import.open = false;
                window.close_dialog(cx);
                let n = |k: &str| v.get(k).and_then(Value::as_i64).unwrap_or(0);
                let (added, updated, presets, skipped) = (n("added"), n("updated"), n("presets"), n("presetsSkipped"));
                let count = |key: &str, n: i64| r.i18n.tn(key, n, &[("n", &n.to_string())]);
                let parts = if added > 0 && updated > 0 {
                    r.i18n.tv("iv.doneBoth", &[("n", &added.to_string()), ("u", &updated.to_string())])
                } else if added > 0 {
                    count("iv.done", added)
                } else if updated > 0 {
                    count("iv.doneUpd", updated)
                } else {
                    String::new()
                };
                // Presets past the limit are left out: said, so the count doesn't read as all of them.
                let about = [if presets > 0 { count("iv.donePresets", presets) } else { String::new() }, if skipped > 0 { count("iv.presetsSkipped", skipped) } else { String::new() }]
                    .into_iter()
                    .filter(|x| !x.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                let text = [parts.clone(), about.clone()].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" ");
                r.toast = Some(if text.is_empty() { (r.tr("iv.doneNone"), "info".into()) } else { (text, if skipped > 0 { "warn" } else { "ok" }.into()) });
                // What came in is picked, ready to play.
                let ids: HashSet<String> = v.get("ids").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
                if !ids.is_empty() {
                    r.selected = ids;
                }
            });
        }
    }
}

/// The pasted list's words, as typed (the hidden window's shots).
pub fn set_list_text(r: &mut Root, text: String, window: &mut Window, cx: &mut Context<Root>) {
    if let Some(b) = r.import.list.clone() {
        b.update(cx, |st, cx| st.set_value(text, window, cx));
    }
}

/// Opens the picked file (with its password, when it has one).
fn open_file(r: &mut Root, password: String, window: &mut Window, cx: &mut Context<Root>) {
    call(r, "vaultImportOpen", json!({ "password": password }), window, cx, opened);
}

/// The app's answer to opening the file or checking the list: its accounts to tick, or why not.
fn opened(r: &mut Root, v: Value, window: &mut Window, cx: &mut Context<Root>) {
    // (Another file picked meanwhile: the app dropped this one.)
    if !v.is_object() {
        return;
    }
    if s(&v, "status") != "ok" {
        // While each try waits the window says so itself, counting down, in place of the app's words.
        let wait = v.get("wait").and_then(Value::as_u64).filter(|w| *w > 0);
        r.import.wait_until = wait.map(|w| Instant::now() + Duration::from_secs(w));
        r.import.error = if wait.is_some() { None } else { Some(s(&v, "error").to_string()).filter(|e| !e.is_empty()) };
        if r.import.step == 0 && r.import.kind == Kind::Ram {
            r.import.step = 1; // (a file that turned out to need its password after all)
        }
        return;
    }
    // What isn't here yet starts ticked.
    r.import.ticked = v.get("accounts").and_then(Value::as_array).map(|l| l.iter().filter(|a| !flag(a, "exists")).map(key_of).collect()).unwrap_or_default();
    r.import.skipped = v.get("skipped").and_then(Value::as_i64).unwrap_or(0);
    if r.import.kind == Kind::List {
        r.import.file = s(&v, "name").to_string();
    }
    r.import.preview = Some(v);
    r.import.presets = false;
    r.import.step = 2;
    r.iv_pw.update(cx, |st, cx| st.set_value("", window, cx));
}

/// "Step n/total": a file without a password, and the pasted list, have two steps.
fn step_words(r: &Root) -> String {
    let f = &r.import;
    let short = f.kind == Kind::List || (f.kind == Kind::Ram && !f.locked && f.step != 1);
    let (n, total) = if short { (if f.step == 0 { 1 } else { 2 }, 2) } else { (f.step as usize + 1, 3) };
    r.i18n.tv("iv.stepOf", &[("n", &n.to_string()), ("total", &total.to_string())])
}

fn close(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    if r.import.open {
        r.import.open = false;
        r.send("vaultImportCancel", json!({}));
    }
    r.iv_pw.update(cx, |st, cx| st.set_value("", window, cx));
    window.close_dialog(cx);
}

pub fn open(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    open_kind(r, Kind::Vault, window, cx);
}

/// Accounts from Roblox Account Manager's AccountData.json.
pub fn open_ram(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    open_kind(r, Kind::Ram, window, cx);
}

/// Accounts from a pasted list of cookies.
pub fn open_list(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    open_kind(r, Kind::List, window, cx);
}

fn open_kind(r: &mut Root, kind: Kind, window: &mut Window, cx: &mut Context<Root>) {
    let opened = r.import.opened.wrapping_add(1);
    // (The list's box is kept from one opening to the next, emptied.)
    let list = match (kind, r.import.list.take()) {
        (Kind::List, Some(b)) => Some(b),
        (Kind::List, None) => Some(cx.new(|cx| TextareaState::new(window, cx).rows(8))),
        (_, kept) => kept,
    };
    if let (Kind::List, Some(b)) = (kind, &list) {
        b.update(cx, |st, cx| st.set_value("", window, cx));
        // The list's box takes the keys once the box is up (a box made just now isn't drawn yet).
        let b = b.clone();
        cx.defer_in(window, move |_, window, cx| {
            let h = b.read(cx).focus_handle(cx);
            window.focus(&h, cx);
        });
    }
    r.import = Data { open: true, opened, kind, list, ..Default::default() };
    r.help_open = true;
    // While a try waits, the button counts down.
    cx.spawn_in(window, async move |this, cx| {
        loop {
            cx.background_executor().timer(Duration::from_millis(500)).await;
            let going = this.update_in(cx, |r, window, _| {
                if r.import.open && r.import.wait_until.is_some() {
                    if wait_left(r).is_none() {
                        r.import.wait_until = None;
                    }
                    window.refresh();
                }
                r.import.open && r.import.opened == opened
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
        // The faces of the file's accounts, where the file has their pictures (else their initials).
        let urls: Vec<(i64, String)> = accounts(root.read(cx)).iter().map(|a| (key_of(a), s(a, "avatarUrl").to_string())).filter(|(_, u)| !u.is_empty()).collect();
        let pics: std::collections::HashMap<i64, std::sync::Arc<Image>> = urls.into_iter().filter_map(|(k, u)| crate::images::pic(&u, cx).map(|p| (k, p))).collect();
        let r = root.read(cx);
        let f = &r.import;
        let waiting = wait_left(r);
        let fine = |icon_name: &'static str, key: &str| {
            div().flex().gap(px(8.)).text_size(px(T_S)).line_height(px(16.)).text_color(c(MUTED)).child(div().mt(px(1.)).flex_none().child(icon(icon_name, 13., c(MUTED)))).child(div().min_w_0().flex_1().child(r.tr(key)))
        };
        let mut body = div().flex().flex_col().gap(px(12.)).child(div().text_size(px(T_S)).font_weight(FontWeight::SEMIBOLD).text_color(c(MUTED)).child(step_words(r)));
        // The file picked, and the way to another.
        if f.step > 0 {
            let other = this.clone();
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(icon(if f.kind == Kind::List { "rows" } else { "folder" }, 15., c(TEXT_2)))
                    .child(div().min_w_0().flex_1().truncate().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).child(short_name(&f.file)))
                    .child(
                        focusable(div().id("iv-other"))
                            .flex_none()
                            .text_size(px(T_M))
                            .text_color(c(TEXT_2))
                            .underline()
                            .cursor_pointer()
                            .hover(|st| st.text_color(c(TEXT)))
                            .child(r.tr(if f.kind == Kind::List { "il.edit" } else { "iv.otherFile" }))
                            .on_click(move |_, w, cx| {
                                let _ = other.update(cx, |r, cx| {
                                    if r.import.kind == Kind::List {
                                        // Back to the list as it was pasted; the checked one is let go.
                                        r.send("vaultImportCancel", json!({}));
                                        r.import.preview = None;
                                        r.import.error = None;
                                        r.import.step = 0;
                                        if let Some(b) = r.import.list.clone() {
                                            let h = b.read(cx).focus_handle(cx);
                                            w.focus(&h, cx);
                                        }
                                    } else {
                                        pick(r, w, cx);
                                    }
                                });
                                w.refresh();
                            }),
                    ),
            );
        }
        match f.step {
            0 => {
                let intro = match f.kind {
                    Kind::Vault => "iv.intro",
                    Kind::Ram => "ir.intro",
                    Kind::List => "il.intro",
                };
                body = body.child(div().text_size(px(T_M)).line_height(px(20.)).text_color(c(TEXT_2)).child(crate::pages::lock::plain(r.tr(intro))));
                if let (Kind::List, Some(list)) = (f.kind, &f.list) {
                    body = body.child(text_area(list).when(f.error.is_some(), |d| d.border_color(ca(BAD, 0.55))));
                }
                body = body.child(fine("shield", if f.kind == Kind::List { "il.safe" } else { "iv.safe" }));
            }
            1 => {
                let eye = this.clone();
                body = body
                    .child(
                        div().flex().flex_col().gap(px(6.)).child(div().text_size(px(T_S)).font_weight(FontWeight::MEDIUM).text_color(c(TEXT_2)).child(r.tr(if f.kind == Kind::Ram { "ir.pw" } else { "iv.pw" }))).child(
                            field(&r.iv_pw, Some("lock")).w_full().when(f.error.is_some(), |d| d.border_color(ca(BAD, 0.55))).child(
                                IconButton::new("iv-eye", if f.shown { "eyeOff" } else { "eye" }).size(30.).tip(r.tr(if f.shown { "lock.hidePw" } else { "lock.showPw" })).on_click(move |_, w, cx| {
                                    let _ = eye.update(cx, |r, cx| {
                                        r.import.shown = !r.import.shown;
                                        let masked = !r.import.shown;
                                        r.iv_pw.update(cx, |st, cx| st.set_masked(masked, w, cx));
                                    });
                                    w.refresh();
                                }),
                            ),
                        ),
                    )
                    .child(fine("shield", "iv.safe"));
            }
            _ => {
                let all = accounts(r);
                if all.is_empty() {
                    body = body.child(div().text_size(px(T_M)).text_color(c(TEXT_2)).child(r.tr("iv.empty")));
                } else {
                    let list = listed(r, cx);
                    let n = all.iter().filter(|a| f.ticked.contains(&key_of(a))).count();
                    // (A long list gets a search box.)
                    if all.len() > 20 {
                        body = body.child(field(&r.iv_q, Some("search")));
                    }
                    let (every, none) = (this.clone(), this.clone());
                    let new_keys: Vec<i64> = list.iter().filter(|a| !flag(a, "exists")).map(key_of).collect();
                    let seen_keys: Vec<i64> = list.iter().map(key_of).collect();
                    let word = |id: &'static str, key: &str| focusable(div().id(id)).text_size(px(T_M)).text_color(c(TEXT_2)).underline().cursor_pointer().hover(|st| st.text_color(c(TEXT))).child(r.tr(key));
                    body = body.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(14.))
                            .child(div().flex_1().text_size(px(T_M)).text_color(c(MUTED)).child(r.i18n.tv("iv.count", &[("n", &n.to_string()), ("total", &all.len().to_string())])))
                            .child(word("iv-all", "iv.all").on_click(move |_, w, cx| {
                                let _ = every.update(cx, |r, _| r.import.ticked.extend(new_keys.iter().copied()));
                                w.refresh();
                            }))
                            .child(word("iv-none", "iv.none").on_click(move |_, w, cx| {
                                let _ = none.update(cx, |r, _| r.import.ticked.retain(|k| !seen_keys.contains(k)));
                                w.refresh();
                            })),
                    );
                    let mut rows = div().flex().flex_col();
                    for (i, a) in list.iter().enumerate() {
                        let key = key_of(a);
                        let exists = flag(a, "exists");
                        let signed = a.get("cookieValid").and_then(Value::as_bool);
                        let hide = flag(&r.state, "hideNames");
                        // The facts under the name, each in its own colour: who and where quiet, whether it is signed in
                        // green or pink (needs a sign-in), a saved password dimmer still (app.css .iv-row).
                        let mut facts: Vec<(String, Hsla)> = Vec::new();
                        if !hide && !s(a, "username").is_empty() {
                            facts.push((format!("@{}", s(a, "username")), c(MUTED)));
                        }
                        if !s(a, "group").is_empty() {
                            facts.push((s(a, "group").to_string(), c(MUTED)));
                        }
                        facts.push(match signed {
                            Some(true) => (r.tr("iv.ckOk"), c(OK)),
                            Some(false) => (r.tr("iv.ckBad"), mix(c(BAD), c(WHITE), 0.3)),
                            None => (r.tr("iv.ckUnknown"), c(MUTED)),
                        });
                        if flag(a, "hasPassword") {
                            facts.push((r.tr("iv.pwSaved"), ca(MUTED, 0.7)));
                        }
                        // One line of text with a colour per part, so a line too long for the row ends in "…" instead
                        // of being cut mid-word.
                        let (mut line, mut marks) = (String::new(), Vec::new());
                        for (n, (text, col)) in facts.into_iter().enumerate() {
                            if n > 0 {
                                let at = line.len();
                                line.push_str("  ·  ");
                                marks.push((at..line.len(), HighlightStyle { color: Some(c(DIM)), ..Default::default() }));
                            }
                            let at = line.len();
                            line.push_str(&text);
                            marks.push((at..line.len(), HighlightStyle { color: Some(col), ..Default::default() }));
                        }
                        let under = div().min_w_0().text_size(px(T_S)).line_height(px(17.)).child(StyledText::new(line).with_highlights(marks));
                        let name = shown_name(r, a);
                        let tick = this.clone();
                        rows = rows.child(
                            div()
                                .min_h(px(52.))
                                .px(px(4.))
                                .py(px(7.))
                                .when(i > 0, |d| d.border_t_1().border_color(ca(WHITE, 0.05)))
                                .flex()
                                .items_center()
                                .gap(px(10.))
                                .child(Check::new(("iv-tick", i), f.ticked.contains(&key)).on_toggle(move |on, w, cx| {
                                    let on = *on;
                                    let _ = tick.update(cx, |r, _| {
                                        if on {
                                            r.import.ticked.insert(key);
                                        } else {
                                            r.import.ticked.remove(&key);
                                        }
                                    });
                                    w.refresh();
                                }))
                                .child(face(&name, if hide { None } else { pics.get(&key).cloned() }, 34.))
                                .child(
                                    div()
                                        .min_w_0()
                                        .flex_1()
                                        .flex()
                                        .flex_col()
                                        .gap(px(2.))
                                        .child(div().truncate().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).child(name))
                                        .child(under),
                                )
                                // (Roblox VNG's: the amber tag, as the web page's .tag.vng.)
                                .when(s(a, "realm") == "vng", |d| d.child(div().flex_none().text_size(px(T_XS)).font_weight(FontWeight::SEMIBOLD).text_color(c(WARN)).child("VNG")))
                                .child(div().flex_none().text_size(px(T_S)).font_weight(FontWeight::MEDIUM).text_color(c(if exists { MUTED } else { OK })).child(r.tr(if exists { "iv.exists" } else { "iv.new" }))),
                        );
                    }
                    // (Taller than its box: it scrolls inside it, its scrollbar there all along.)
                    body = body.child(scroll_list("iv-list", &f.list_scroll, 300., rows));
                }
                let count = f.preview.as_ref().and_then(|p| p.get("presets")).and_then(Value::as_i64).unwrap_or(0);
                if count > 0 {
                    let also = this.clone();
                    body = body.child(
                        div()
                            .p(px(12.))
                            .rounded(px(R_M))
                            .border_1()
                            .border_color(c(LINE))
                            .flex()
                            .items_start()
                            .gap(px(10.))
                            .child(Check::new("iv-presets", f.presets).on_toggle(move |on, w, cx| {
                                let on = *on;
                                let _ = also.update(cx, |r, _| r.import.presets = on);
                                w.refresh();
                            }))
                            .child(
                                div()
                                    .min_w_0()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.))
                                    .child(div().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).child(r.i18n.tv("iv.presets", &[("n", &count.to_string())])))
                                    .child(div().text_size(px(T_S)).line_height(px(16.)).text_color(c(MUTED)).child(r.tr("iv.presetsSub"))),
                            ),
                    );
                }
                if all.iter().any(|a| flag(a, "exists")) {
                    body = body.child(fine("info", "iv.existsNote"));
                }
                // The list's lines left out: said, so the count doesn't read as all of them.
                if f.skipped > 0 {
                    body = body.child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .text_size(px(T_S))
                            .line_height(px(16.))
                            .text_color(c(WARN))
                            .child(div().mt(px(1.)).flex_none().child(icon("alert", 13., c(WARN))))
                            .child(div().min_w_0().flex_1().child(r.i18n.tn("il.skipped", f.skipped, &[]))),
                    );
                }
            }
        }
        let said = match waiting {
            Some(n) if f.step == 1 => Some(r.i18n.tv("iv.tooMany", &[("n", &n.to_string())])),
            _ => f.error.clone(),
        };
        if let Some(why) = said {
            body = body.child(form_error(why));
        }
        let ticked = accounts(r).iter().filter(|a| f.ticked.contains(&key_of(a))).count();
        let (label, off) = match f.step {
            0 if f.kind == Kind::List => (r.tr(if f.busy { "il.checking" } else { "il.check" }), false),
            0 => (r.tr("iv.choose"), false),
            1 => match waiting {
                Some(n) => (r.i18n.tv("iv.wait", &[("n", &n.to_string())]), true),
                None => (r.tr("iv.open"), false),
            },
            _ => (r.tr("iv.importBtn"), ticked == 0 && !f.presets),
        };
        let (cancel, go, shut) = (this.clone(), this.clone(), this.clone());
        let title = match f.kind {
            Kind::Vault => "iv.title",
            Kind::Ram => "ir.title",
            Kind::List => "il.title",
        };
        framed(dialog, r.tr(title), r.tr("common.close"), move |_, w, cx| {
            let _ = shut.update(cx, |r, cx| close(r, w, cx));
        })
        .w(px(DIALOG_W + 80.))
        .centered()
        // (Only its buttons and the × close it, so the app always hears when the file is let go.)
        .overlay_closable(false)
        .keyboard(false)
        .entrance(DialogEntrance::FadeSlide)
        .child(dialog_body(body))
        .footer(
                dialog_foot()
                    .child(Button::new("iv-cancel", r.tr("common.cancel")).on_click(move |_, w, cx| {
                        let _ = cancel.update(cx, |r, cx| close(r, w, cx));
                    }))
                    .child(Button::new("iv-next", label).primary().disabled(off || f.busy).on_click(move |_, w, cx| {
                        let _ = go.update(cx, |r, cx| next(r, w, cx));
                    })),
            )
    });
}
