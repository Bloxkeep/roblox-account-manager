// Two boxes of the frame's, as the web page had them.
//
// Send feedback (ui/src/Feedback.tsx; app.js's feedbackModal): what it is (something that doesn't work, or an idea),
// the message, how to reach you (optional), whether Bloxkeep's error log goes along, and "See exactly what will be
// sent" (the app's "feedbackPreview"); Send is the app's "sendFeedback".
//
// What's new (ui/src/WhatsNew.tsx; app.js's showWhatsNew): the newest release first, its headline features (each may
// lead to a place), then what is new, improved, fixed and removed; the releases skipped since fold below. Shown by
// itself after an update (the app's state carries whatsNew once), and from Settings. The features' pictures aren't
// drawn here yet.
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::dialog::DialogEntrance;
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use crate::{
    kit::*,
    pages::Page,
    shell::Root,
    theme::*,
};

#[derive(Default)]
pub struct Data {
    /// Feedback: open; an idea (not something broken); the error log goes along; what would be sent, when asked to
    /// see it; what is wrong; and whether it is being sent.
    fb_open: bool,
    idea: bool,
    log: bool,
    preview: Option<String>,
    error: Option<String>,
    busy: bool,
    /// What's new: the releases before the newest are unfolded.
    older: bool,
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or_default()
}

// ───────────────────────── feedback ─────────────────────────

/// What is typed changed: an open preview is shut again rather than left out of date, and the old problem goes.
pub fn edited(r: &mut Root) {
    r.extras.preview = None;
    r.extras.error = None;
}

fn values(r: &Root, cx: &App) -> Value {
    json!({
        "kind": if r.extras.idea { "idea" } else { "bug" },
        "text": r.fb_text.read(cx).value().to_string(),
        "contact": r.fb_contact.read(cx).value().to_string(),
        "log": r.extras.log,
    })
}

fn send(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    if r.extras.busy {
        return;
    }
    let v = values(r, cx);
    if s(&v, "text").trim().chars().count() < 3 {
        r.extras.error = Some(r.tr("fb.needText"));
        window.refresh();
        return;
    }
    let Some(link) = r.link.clone() else { return };
    r.extras.busy = true;
    r.extras.error = None;
    window.refresh();
    let rx = link.call("sendFeedback", v);
    cx.spawn_in(window, async move |this, cx| {
        let answer = rx.await;
        let _ = this.update_in(cx, |r, window, cx| {
            // Still this box's sending: open, and not a box opened again since (which starts idle). Else the box it was
            // sent from is gone, and whatever is on top now isn't closed.
            let mine = r.extras.fb_open && r.extras.busy;
            r.extras.busy = false;
            match answer {
                Ok(Ok(_)) => {
                    if mine {
                        r.extras.fb_open = false;
                        window.close_dialog(cx);
                    }
                    r.toast = Some((r.tr("fb.sent"), "ok".into()));
                }
                Ok(Err(e)) => r.extras.error = Some(e.to_string()),
                Err(_) => {}
            }
            window.refresh();
            cx.notify();
        });
    })
    .detach();
}

/// "See exactly what will be sent": asked from the app, or shut again.
fn look(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    if r.extras.preview.take().is_some() {
        window.refresh();
        return;
    }
    let Some(link) = r.link.clone() else { return };
    let rx = link.call("feedbackPreview", values(r, cx));
    cx.spawn_in(window, async move |this, cx| {
        if let Ok(Ok(v)) = rx.await {
            let _ = this.update_in(cx, |r, window, _| {
                if r.extras.fb_open {
                    r.extras.preview = Some(s(&v, "preview").to_string());
                    window.refresh();
                }
            });
        }
    })
    .detach();
}

pub fn open_feedback(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    r.extras = Data { fb_open: true, log: true, older: r.extras.older, ..Default::default() };
    let hint = r.tr("fb.textPh");
    r.fb_text.update(cx, |st, cx| {
        st.set_value("", window, cx);
        st.set_placeholder(hint, window, cx);
    });
    let hint = r.tr("fb.contactPh");
    r.fb_contact.update(cx, |st, cx| {
        st.set_value("", window, cx);
        st.set_placeholder(hint, window, cx);
    });
    let this = cx.weak_entity();
    let first = std::rc::Rc::new(std::cell::Cell::new(true));
    window.open_dialog(cx, move |dialog, window, cx| {
        let Some(root) = this.upgrade() else { return dialog };
        if first.replace(false) {
            let h = root.read(cx).fb_text.read(cx).focus_handle(cx);
            window.focus(&h, cx);
        }
        let r = root.read(cx);
        let f = &r.extras;
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
        // Two answers: a pair to pick from, not a list to open.
        let mut kind = div().flex().gap(px(2.)).p(px(3.)).rounded(px(R_M)).bg(c(INSET));
        for (key, idea) in [("fb.bug", false), ("fb.idea", true)] {
            let this = this.clone();
            kind = kind.child(
                segment(key, f.idea == idea)
                    .flex_1()
                    .h(px(28.))
                    .px(px(10.))
                    .justify_center()
                    .text_size(px(T_S + 0.5))
                    .cursor_pointer()
                    .child(r.tr(key))
                    .on_click(move |_, w, cx| {
                        let _ = this.update(cx, |r, _| {
                            if r.extras.idea != idea {
                                edited(r);
                                r.extras.idea = idea;
                                // An idea rarely needs the log; a report of something broken usually does.
                                r.extras.log = !idea;
                            }
                        });
                        w.refresh();
                    }),
            );
        }
        let (log, see, cancel, go) = (this.clone(), this.clone(), this.clone(), this.clone());
        let mut body = div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(div().flex().flex_col().gap(px(6.)).child(label("fb.kind", None)).child(kind))
            .child(
                div().flex().flex_col().gap(px(6.)).child(label("fb.text", None)).child(text_area(&r.fb_text)),
            )
            .child(div().flex().flex_col().gap(px(6.)).child(label("fb.contact", Some("fb.optional"))).child(field(&r.fb_contact, None)))
            // (Its sentence wraps inside the dialog instead of running past its edge.)
            .child(div().w_full().flex().child(Check::new("fb-log", f.log).label(r.tr("fb.log")).on_toggle(move |on, w, cx| {
                let on = *on;
                let _ = log.update(cx, |r, _| {
                    edited(r);
                    r.extras.log = on;
                });
                w.refresh();
            })))
            .child(div().flex().child(Button::new("fb-look", r.tr("fb.preview")).icon("eye").quiet().small().on_click(move |_, w, cx| {
                let _ = see.update(cx, |r, cx| look(r, w, cx));
            })));
        if let Some(text) = &f.preview {
            let mut lines = div().id("fb-preview").max_h(px(180.)).overflow_y_scroll().p(px(10.)).rounded(px(R_M)).bg(c(INSET)).flex().flex_col().text_size(px(T_S)).line_height(px(16.)).text_color(c(TEXT_2));
            for line in text.lines() {
                lines = lines.child(if line.is_empty() { div().h(px(8.)) } else { div().child(line.to_string()) });
            }
            body = body.child(lines);
        }
        body = body.child(
            div().flex().gap(px(8.)).text_size(px(T_S)).line_height(px(16.)).text_color(c(MUTED)).child(div().mt(px(1.)).flex_none().child(icon("shield", 13., c(MUTED)))).child(div().min_w_0().flex_1().child(r.tr("fb.fine"))),
        );
        if let Some(why) = f.error.clone() {
            body = body.child(form_error(why));
        }
        let shut = this.clone();
        let foot = dialog_foot()
            .child(Button::new("fb-cancel", r.tr("common.cancel")).on_click(move |_, w, cx| {
                let _ = cancel.update(cx, |r, _| r.extras.fb_open = false);
                w.close_dialog(cx);
            }))
            .child(Button::new("fb-send", r.tr("fb.send")).primary().on_click(move |_, w, cx| {
                let _ = go.update(cx, |r, cx| send(r, w, cx));
            }));
        framed(dialog, r.tr("fb.title"), r.tr("common.close"), move |_, w, cx| {
            let _ = shut.update(cx, |r, _| r.extras.fb_open = false);
            w.close_dialog(cx);
        })
        .w(px(DIALOG_W))
        .centered()
        // (Only Cancel, Send and the × close it: a click beside it, or Esc, would throw away what was written.)
        .overlay_closable(false)
        .keyboard(false)
        .entrance(DialogEntrance::FadeSlide)
        .child(dialog_body(body))
        .footer(foot)
    });
}

// ───────────────────────── what's new ─────────────────────────

/// Where a headline's button leads: a place by its name, a part of Settings ("settings:perf"), or Bloxkeep on GitHub.
fn go_to(r: &mut Root, target: &str) {
    if let Some(part) = target.strip_prefix("settings:") {
        if let Some(p) = crate::pages::settings::Part::ALL.into_iter().find(|p| p.key() == part) {
            r.settings.part = p;
        }
        r.go(Page::Settings);
    } else if target == "github" {
        r.send("openGitHub", json!({}));
    } else if let Some(p) = Page::from_key(target) {
        r.go(p);
    }
}

/// "2026-11-08" as "November 8, 2026" or "8 tháng 11, 2026".
fn day(date: &str, vi: bool) -> String {
    let mut parts = date.split('-').filter_map(|p| p.parse::<usize>().ok());
    let (Some(y), Some(m), Some(d)) = (parts.next(), parts.next(), parts.next()) else { return String::new() };
    const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    if !(1..=12).contains(&m) {
        return String::new();
    }
    if vi { format!("{d} tháng {m}, {y}") } else { format!("{} {d}, {y}", MONTHS[m - 1]) }
}

/// A release's lines in the language shown (English where there is no translation).
fn pick(v: Option<&Value>, vi: bool) -> Vec<String> {
    let list = |k: &str| v.and_then(|v| v.get(k)).and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect::<Vec<_>>()).unwrap_or_default();
    let own = if vi { list("vi") } else { Vec::new() };
    if own.is_empty() { list("en") } else { own }
}

/// "Title: what it does" as its title and its words; a note without a short title before its first ": " is all words.
fn split_note(note: &str) -> (Option<String>, String) {
    match note.find(": ") {
        Some(i) if i > 0 && note[..i].chars().count() <= 60 && !note[..i].contains('.') => {
            let text = &note[i + 2..];
            // (Under its own title it starts a sentence: "choose it…" reads "Choose it…".)
            let mut chars = text.chars();
            let text = match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            };
            (Some(note[..i].to_string()), text)
        }
        _ => (None, note.to_string()),
    }
}

fn notes(items: Vec<String>) -> Div {
    let mut list = div().flex().flex_col().gap(px(8.));
    for note in items {
        let (title, text) = split_note(&note);
        list = list.child(
            div().flex().gap(px(9.)).text_size(px(T_M)).line_height(px(19.)).child(div().mt(px(7.)).size(px(5.)).flex_none().rounded_full().bg(c(DIM))).child(
                div()
                    .min_w_0()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .when_some(title, |d, t| d.child(div().font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(t)))
                    .child(div().text_color(c(TEXT_2)).child(text)),
            ),
        );
    }
    list
}

/// A release's lists: its own lines, then New, Improved, Fixed and Removed.
fn release(root: &Root, rel: &Value, also: bool) -> Vec<Div> {
    let vi = root.i18n.vietnamese;
    let kind = |text: String| div().pt(px(4.)).text_size(px(T_S)).font_weight(FontWeight::SEMIBOLD).text_color(c(MUTED)).child(text);
    let mut out = Vec::new();
    let own = pick(Some(rel), vi);
    if !own.is_empty() {
        if also {
            out.push(kind(root.tr("wn.also")));
        }
        out.push(notes(own));
    }
    for k in ["new", "improved", "fixed", "removed"] {
        let items = pick(rel.get(k), vi);
        if !items.is_empty() {
            out.push(kind(root.tr(&format!("wn.kind.{k}"))));
            out.push(notes(items));
        }
    }
    out
}

/// Settings' What's new: the app's notes for the version running, or that there are none.
pub fn ask_whats_new(r: &mut Root, cx: &mut Context<Root>) {
    let Some(link) = r.link.clone() else { return };
    let rx = link.call("whatsNew", json!({}));
    cx.spawn(async move |this, cx| {
        let answer = rx.await;
        let _ = this.update(cx, |r, cx| {
            match answer {
                Ok(Ok(Value::Array(list))) if !list.is_empty() => r.show_whats_new(list),
                Ok(Ok(_)) => r.toast = Some((r.tr("wn.none"), "info".into())),
                Ok(Err(e)) => r.toast = Some((e.to_string(), "error".into())),
                Err(_) => {}
            }
            cx.notify();
        });
    })
    .detach();
}

/// A feature's picture ("whatsnew/1.0.16-macros.{lang}.webp") as a file: in the "wwwroot" folder beside the program
/// (BLOXKEEP_WWWROOT: another), else the source tree's for a run from it. Only a plain name inside that folder.
fn wn_picture(rel: &str, vi: bool) -> Option<String> {
    let rel = rel.replace("{lang}", if vi { "vi" } else { "en" });
    if rel.is_empty() || rel.contains("..") || rel.contains(':') || rel.contains('\\') || rel.starts_with('/') {
        return None;
    }
    let exe = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf()));
    let mut roots: Vec<std::path::PathBuf> = Vec::new();
    if let Some(d) = std::env::var_os("BLOXKEEP_WWWROOT").filter(|d| !d.is_empty()) {
        roots.push(d.into());
    }
    roots.extend(exe.iter().map(|e| e.join("wwwroot")));
    if let Some(a) = std::env::var_os("BLOXKEEP_UI_ASSETS") {
        roots.push(std::path::PathBuf::from(a).join("..").join("..").join("RobloxManager").join("wwwroot"));
    }
    roots.into_iter().map(|r| r.join(&rel)).find(|f| f.is_file()).map(|f| f.to_string_lossy().into_owned())
}

pub fn whats_new(r: &mut Root, releases: Vec<Value>, window: &mut Window, cx: &mut Context<Root>) {
    if releases.is_empty() {
        return;
    }
    r.extras.older = false;
    let this = cx.weak_entity();
    window.open_dialog(cx, move |dialog, window, cx| {
        let Some(root) = this.upgrade() else { return dialog };
        let vi = root.read(cx).i18n.vietnamese;
        let latest = &releases[0];
        let highlights = latest.get("highlights").and_then(Value::as_array).cloned().unwrap_or_default();
        // Each feature's picture or demo, in the app's language (loaded as the box shows; the box draws again then),
        // with its shape: drawn whole at the card's width, never cut to a strip (review 9.1).
        let pictures: Vec<Option<(std::sync::Arc<Image>, f32)>> = highlights
            .iter()
            .map(|h| {
                let path = wn_picture(s(h, "image"), vi)?;
                let p = crate::images::pic(&path, cx)?;
                let shape = crate::images::dims(&path, cx).map(|(w, h)| w as f32 / h as f32).filter(|a| a.is_finite() && *a > 0.2 && *a < 6.).unwrap_or(16. / 10.);
                Some((p, shape))
            })
            .collect();
        // With pictures the box is as wide as the window lets it, up to the pictures' own width (900 px): at 620 their
        // words were 5 px and nobody could read them.
        let wide = if pictures.iter().any(Option::is_some) { (f32::from(window.viewport_size().width) - 96.).clamp(620., 980.) } else { 620. };
        let r = root.read(cx);
        let older = &releases[1..];
        // (The dialog's body scrolls by itself between its head and its foot, as the web page's .modal-body does.)
        let mut body = div().flex().flex_col().gap(px(20.));
        for (i, h) in highlights.iter().enumerate() {
            let words = pick(Some(h), vi);
            let (title, text, button) = (words.first().cloned().unwrap_or_default(), words.get(1).cloned().unwrap_or_default(), words.get(2).cloned().unwrap_or_default());
            let target = s(h, "go").to_string();
            let go = this.clone();
            body = body.child(
                div()
                    .p(px(16.))
                    .rounded(px(R_L))
                    .bg(ca(WHITE, 0.045))
                    .border_1()
                    .border_color(c(LINE))
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap(px(6.))
                    // The feature's name and words first, then its picture: a card cut at the body's foot shows the
                    // next feature's name, so it reads as the next one in the list and not as a picture spilling out.
                    .child(div().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).child(title))
                    .child(div().text_size(px(T_M)).line_height(px(19.)).text_color(c(TEXT_2)).child(text))
                    .when_some(pictures.get(i).cloned().flatten(), |d, (p, shape)| {
                        d.child(
                            div()
                                .w_full()
                                .aspect_ratio(shape)
                                .mt(px(6.))
                                .rounded(px(R_M))
                                .border_1()
                                .border_color(c(LINE))
                                .bg(c(GROUND))
                                // (The whole picture at the card's width and its own shape: nothing cut, no bars. Its
                                // own corners, not a clip of the box: a box that clips, scrolled wholly out of the
                                // body, was drawn below it over the foot.)
                                .child(img(p).size_full().rounded(px(R_M - 1.)).object_fit(ObjectFit::Contain)),
                        )
                    })
                    .when(!target.is_empty() && !button.is_empty(), |d| {
                        d.child(div().pt(px(4.)).child(Button::new(("wn-go", i), button).icon_end("chevronRight").small().on_click(move |_, w, cx| {
                            w.close_dialog(cx);
                            let _ = go.update(cx, |r, cx| {
                                go_to(r, &target);
                                cx.notify();
                            });
                        })))
                    }),
            );
        }
        for part in release(r, latest, !highlights.is_empty()) {
            body = body.child(part);
        }
        if !older.is_empty() {
            let open = r.extras.older;
            let fold = this.clone();
            let n = older.len() as i64;
            body = body.child(div().pt(px(6.)).flex().child(
                focusable(div().id("wn-older"))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_size(px(T_M))
                    .text_color(c(TEXT_2))
                    .cursor_pointer()
                    .hover(|st| st.text_color(c(TEXT)))
                    .child(icon(if open { "chevronDown" } else { "chevronRight" }, 14., c(MUTED)))
                    .child(r.i18n.tn("wn.older", n, &[("n", &n.to_string())]))
                    .on_click(move |_, w, cx| {
                        let _ = fold.update(cx, |r, _| r.extras.older = !r.extras.older);
                        w.refresh();
                    }),
            ));
            if open {
                for rel in older {
                    body = body.child(
                        div()
                            .pt(px(8.))
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).child(s(rel, "version").to_string()))
                            .child(div().text_size(px(T_S)).text_color(c(MUTED)).child(day(s(rel, "date"), vi))),
                    );
                    for part in release(r, rel, false) {
                        body = body.child(part);
                    }
                }
            }
        }
        let star = this.clone();
        body = body.child(
            div()
                .mt(px(6.))
                .pt(px(12.))
                .border_t_1()
                .border_color(c(LINE))
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(10.))
                .child(div().flex_1().min_w(px(200.)).text_size(px(T_S + 0.5)).text_color(c(MUTED)).child(r.tr("wn.star")))
                .child(Button::new("wn-star", r.tr("star.open")).icon("star").small().on_click(move |_, _, cx| {
                    let _ = star.update(cx, |r, _| r.send("openGitHub", json!({})));
                })),
        );
        let when = day(s(latest, "date"), vi);
        let title = if when.is_empty() { format!("Bloxkeep {}", s(latest, "version")) } else { format!("Bloxkeep {} · {when}", s(latest, "version")) };
        framed(dialog, title, r.tr("common.close"), |_, w, cx| w.close_dialog(cx))
            .w(px(wide))
            .centered()
            .entrance(DialogEntrance::FadeSlide)
            .child(dialog_body(body))
            .footer(dialog_foot().child(Button::new("wn-ok", r.tr("wn.ok")).primary().on_click(|_, w, cx| w.close_dialog(cx))))
    });
}
