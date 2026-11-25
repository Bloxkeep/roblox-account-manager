// Two boxes of the frame's, as the web page had them.
//
// Send feedback (ui/src/Feedback.tsx; app.js's feedbackModal): what it is (something that doesn't work, or an idea),
// the message, how to reach you (optional), whether Bloxkeep's error log goes along, and "See exactly what will be
// sent" (the app's "feedbackPreview"); Send is the app's "sendFeedback".
//
// What's new (ui/src/WhatsNew.tsx; app.js's showWhatsNew): the newest release first, its headline features (each may
// lead to a place), then what is new, improved, fixed and removed; the releases skipped since fold below. Shown by
// itself after an update (the app's state carries whatsNew once), and from Settings.
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::dialog::DialogEntrance;
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};
use crate::val::s;
use crate::ask::answer;

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
                    .text_size(px(T_M))
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
    if !(1..=12).contains(&m) {
        return String::new();
    }
    if vi { format!("{d} tháng {m}, {y}") } else { format!("{} {d}, {y}", crate::time::month_name(m as i64)) }
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

/// A kind's notes: a bullet each, its short title (when it has one) before its words. `quiet`: in the second colour
/// (what was fixed reads after what is new).
fn notes(items: Vec<String>, quiet: bool) -> Div {
    let mut list = div().flex().flex_col().gap(px(6.));
    for note in items {
        let (title, text) = split_note(&note);
        list = list.child(
            div().flex().gap(px(10.)).text_size(px(T_M)).line_height(px(20.))
                .child(div().mt(px(8.)).size(px(4.)).flex_none().rounded_full().bg(c(if quiet { MUTED } else { TEXT_2 })))
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .when_some(title, |d, t| d.child(div().font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(t)))
                        .child(div().text_color(c(if quiet { TEXT_2 } else { TEXT })).child(text)),
                ),
        );
    }
    list
}

/// A release's lists: its own lines, then New, Improved, Fixed and Removed, each under its small tag.
fn release(root: &Root, rel: &Value, also: bool) -> Vec<Div> {
    let vi = root.i18n.vietnamese;
    let section = |text: Option<String>, tint: Option<u32>, items| {
        let tag = |title: String| {
            let (ground, words) = match tint {
                Some(t) => (ca(t, 0.14), c(t)),
                None => (ca(WHITE, 0.07), c(TEXT_2)),
            };
            div().flex().child(div().h(px(22.)).px(px(10.)).flex().items_center().rounded(px(R_S)).bg(ground).text_size(px(T_S)).font_weight(FontWeight::SEMIBOLD).text_color(words).child(title))
        };
        div().flex().flex_col().gap(px(8.)).when_some(text, |d, title| d.child(tag(title))).child(notes(items, tint.is_none()))
    };
    let mut out = Vec::new();
    let own = pick(Some(rel), vi);
    if !own.is_empty() {
        out.push(section(also.then(|| root.tr("wn.also")), Some(FACE), own));
    }
    for (k, tint) in [("new", Some(OK)), ("improved", Some(ONLINE)), ("fixed", None), ("removed", None)] {
        let items = pick(rel.get(k), vi);
        if !items.is_empty() {
            out.push(section(Some(root.tr(&format!("wn.kind.{k}"))), tint, items));
        }
    }
    out
}

/// Settings' What's new: the app's notes for the version running, or that there are none.
pub fn ask_whats_new(r: &mut Root, cx: &mut Context<Root>) {
    let Some(link) = r.link.clone() else { return };
    let rx = link.call("whatsNew", json!({}));
    answer(rx, cx, move |r, answer, cx| {
        match answer {
            Ok(Ok(Value::Array(list))) if !list.is_empty() => r.show_whats_new(list),
            Ok(Ok(_)) => r.toast = Some((r.tr("wn.none"), "info".into())),
            Ok(Err(e)) => r.toast = Some((e.to_string(), "error".into())),
            Err(_) => {}
        }
        cx.notify();
    });
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

/// A feature's demo as a clip: the MP4 beside its picture ("….webp" → "….mp4"), played by the video player. GPUI
/// draws an animated WebP as its first frame only (it decodes every frame of a GIF alone), and all of a demo's frames
/// held at once would be ~370 MB (184 at 900×562); the player keeps a few frames ahead and stops when not drawn.
fn wn_clip(rel: &str, vi: bool) -> Option<String> {
    rel.strip_suffix(".webp").and_then(|stem| wn_picture(&format!("{stem}.mp4"), vi))
}

pub fn whats_new(r: &mut Root, releases: Vec<Value>, window: &mut Window, cx: &mut Context<Root>) {
    if releases.is_empty() {
        return;
    }
    r.extras.older = false;
    // The pictures' files, found once as the box opens (in both languages), not on every drawing of it.
    let files: Vec<[Option<String>; 2]> = releases[0]
        .get("highlights")
        .and_then(Value::as_array)
        .map(|l| l.iter().map(|h| [wn_picture(s(h, "image"), false), wn_picture(s(h, "image"), true)]).collect())
        .unwrap_or_default();
    // Each demo's clip in both languages, made once as the box opens; none plays until drawn, and they go with the box.
    // One that can't play on this PC (no decoder) leaves its picture in its place.
    let failed: std::rc::Rc<std::cell::RefCell<std::collections::HashSet<String>>> = Default::default();
    let clips: Vec<[Option<(String, Entity<crate::video::Video>)>; 2]> = releases[0]
        .get("highlights")
        .and_then(Value::as_array)
        .map(|l| {
            l.iter()
                .map(|h| {
                    [false, true].map(|vi| {
                        let path = wn_clip(s(h, "image"), vi)?;
                        let video = cx.new(|cx| crate::video::Video::quiet(std::path::PathBuf::from(&path), cx));
                        let (gone, key) = (failed.clone(), path.clone());
                        cx.subscribe(&video, move |_, _, _: &crate::video::Failed, cx| {
                            gone.borrow_mut().insert(key.clone());
                            cx.notify();
                        })
                        .detach();
                        Some((path, video))
                    })
                })
                .collect()
        })
        .unwrap_or_default();
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
            .enumerate()
            .map(|(i, _)| {
                let path = files.get(i).and_then(|f| f[usize::from(vi)].clone())?;
                let p = crate::images::pic(&path, cx)?;
                let shape = crate::images::dims(&path, cx).map(|(w, h)| w as f32 / h as f32).filter(|a| a.is_finite() && *a > 0.2 && *a < 6.).unwrap_or(16. / 10.);
                Some((p, shape))
            })
            .collect();
        let has_pictures = files.iter().any(|f| f[usize::from(vi)].is_some());
        // (A reading column, as the mock-up: 560 of words, wider only for its pictures.)
        let wide = (f32::from(window.viewport_size().width) - 40.).clamp(320., if has_pictures { 680. } else { 560. });
        let r = root.read(cx);
        let older = &releases[1..];
        let when = day(s(latest, "date"), vi);
        let mut body = div().flex().flex_col().gap(px(16.));
        for (i, h) in highlights.iter().enumerate() {
            let words = pick(Some(h), vi);
            let (title, text, button) = (words.first().cloned().unwrap_or_default(), words.get(1).cloned().unwrap_or_default(), words.get(2).cloned().unwrap_or_default());
            let target = s(h, "go").to_string();
            let go = this.clone();
            let picture = pictures.get(i).cloned().flatten();
            let clip = clips
                .get(i)
                .and_then(|c| c[usize::from(vi)].clone())
                .filter(|(path, _)| !failed.borrow().contains(path))
                .map(|(_, v)| (v, picture.as_ref().map(|(_, shape)| *shape).unwrap_or(16. / 10.)));
            // Its name and words, then its picture or clip whole at the card's width, then its button. (The picture used to
            // lead; after a feature without one it read as the one above's.)
            let words = div().min_w_0().flex().flex_col().items_start().gap(px(6.))
                .child(div().text_size(px(T_GROUP)).font_weight(FontWeight::SEMIBOLD).child(title))
                .child(div().text_size(px(T_M)).line_height(px(20.)).text_color(c(TEXT_2)).child(text));
            body = body.child(
                div().flex().flex_col().items_start().gap(px(12.))
                    .child(words)
                    .when_some(clip.clone(), |d, (video, shape)| {
                        d.child(div().w_full().aspect_ratio(shape).rounded(px(R_M)).border_1().border_color(c(LINE)).bg(c(GROUND)).overflow_hidden().child(video))
                    })
                    .when_some(picture.filter(|_| clip.is_none()), |d, (p, shape)| {
                        d.child(div().w_full().aspect_ratio(shape).rounded(px(R_M)).border_1().border_color(c(LINE)).bg(c(GROUND)).overflow_hidden()
                            .child(img(p).size_full().object_fit(ObjectFit::Contain)))
                    })
                    .when(!target.is_empty() && !button.is_empty(), |d| {
                        d.child(Button::new(("wn-go", i), button).icon_end("chevronRight").small().on_click(move |_, w, cx| {
                            w.close_dialog(cx);
                            let _ = go.update(cx, |r, cx| {
                                go_to(r, &target);
                                cx.notify();
                            });
                        }))
                    }),
            );
        }
        for part in release(r, latest, !highlights.is_empty()) {
            body = body.child(part);
        }
        // (Older versions fold from the foot, beside Got it.)
        let open = r.extras.older;
        let fold = this.clone();
        let n = older.len() as i64;
        let older_fold = (!older.is_empty()).then(|| {
            focusable(div().id("wn-older"))
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(px(T_M))
                .text_color(c(TEXT_2))
                .cursor_pointer()
                .hover(|st| st.text_color(c(TEXT)))
                .child(icon(if open { "chevronUp" } else { "chevronDown" }, 14., c(MUTED)))
                .child(r.i18n.tv("wn.olderN", &[("n", &n.to_string())]))
                .on_click(move |_, w, cx| {
                    let _ = fold.update(cx, |r, _| r.extras.older = !r.extras.older);
                    w.refresh();
                })
        });
        if !older.is_empty() {
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
        // The head: what this is, its version and day, the × at its end.
        let version = if when.is_empty() { r.i18n.tv("wn.version", &[("v", s(latest, "version"))]) } else { format!("{} · {when}", r.i18n.tv("wn.version", &[("v", s(latest, "version"))])) };
        let head = div().w_full().px(px(22.)).pt(px(20.)).pb(px(14.)).flex().items_center().gap(px(12.))
            .child(div().size(px(40.)).flex_none().rounded(px(R_M)).bg(ca(FACE, 0.14)).flex().items_center().justify_center().child(icon("news", 20., c(FACE))))
            .child(div().min_w_0().flex_1().flex().flex_col().gap(px(2.))
                .child(div().text_size(px(T_GROUP)).line_height(px(24.)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(r.tr("wn.title")))
                .child(div().text_size(px(T_M)).font_weight(FontWeight::NORMAL).text_color(c(TEXT_2)).child(version)))
            .child(dialog_x(r.tr("common.close"), |_, w, cx| w.close_dialog(cx)));
        dialog.close_button(false).p_0().gap_0().title(head)
            .w(px(wide))
            .centered()
            .entrance(DialogEntrance::FadeSlide)
            .child(dialog_body(body))
            .footer(dialog_foot()
                .children(older_fold)
                .child(div().flex_1())
                .child(Button::new("wn-star", r.tr("star.open")).icon("star").quiet().small().on_click(move |_, _, cx| {
                    let _ = star.update(cx, |r, _| r.send("openGitHub", json!({})));
                }))
                .child(Button::new("wn-ok", r.tr("wn.ok")).primary().on_click(|_, w, cx| w.close_dialog(cx))))
    });
}
