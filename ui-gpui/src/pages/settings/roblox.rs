// Settings › Roblox version (ui/src/settings/RobloxPage.tsx): Roblox closing windows, the build installed and the newest,
// updating Roblox, what the Multi-Roblox check found, the builds seen here. And the settings Bloxkeep gives Roblox before
// it opens (RobloxConfig), shown at the top of Staying in game. A question the old page asked in a box (update with
// windows open) is asked in place under its row; the accounts picker opens under its row too.
use super::*;

/// Roblox's id for a build ("version-cec3ad5889b447cf"), its first 8 characters.
fn short_build(b: &str) -> String {
    b.trim_start_matches("version-").chars().take(8).collect()
}

/// "13/10/2026, 20:25" in Vietnamese; "Oct 13, 2026, 8:25 PM" in English.
pub(super) fn when_text(root: &Root, iso: &str) -> String {
    let Some(at) = crate::time::iso_secs(iso) else { return String::new() };
    let local = at + crate::time::utc_offset_secs();
    let (y, m, d) = civil(local.div_euclid(86400));
    let (h, min) = (local.rem_euclid(86400) / 3600, local.rem_euclid(3600) / 60);
    if root.i18n.vietnamese {
        format!("{d:02}/{m:02}/{y}, {h:02}:{min:02}")
    } else {
        let month = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"][(m - 1).clamp(0, 11) as usize];
        let (h12, am) = (if h % 12 == 0 { 12 } else { h % 12 }, if h < 12 { "AM" } else { "PM" });
        format!("{month} {d}, {y}, {h12}:{min:02} {am}")
    }
}

/// The watch's word on a build: (tone, words); two windows having played together on it here outweigh it; None if unchecked.
fn watch_word(root: &Root, check: &str, worked: bool, at: &str) -> Option<(Tone, String)> {
    let key = if worked {
        "worked"
    } else if check == "same" || check == "first" {
        "same"
    } else if ["changed", "renamed", "gone"].contains(&check) {
        check
    } else {
        return None;
    };
    let tone = if key == "worked" || key == "same" { Tone::Ok } else { Tone::Warn };
    Some((tone, root.tr(&format!("{at}.{key}"))))
}

/// A link in a line of text: words that act.
pub(super) fn link_text(id: impl Into<ElementId>, text: String, cx: &mut Context<Root>, f: impl Fn(&mut Root, &mut Window, &mut Context<Root>) + 'static) -> Stateful<Div> {
    focusable(div().id(id))
        .text_size(px(T_S))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(TEXT))
        .cursor_pointer()
        .hover(|d| d.underline())
        .child(text)
        .on_click(cx.listener(move |r, _, w, cx| f(r, w, cx)))
}

/// Check for a Bloxkeep update, from the Multi-Roblox check: the answer is said in a toast.
fn check_bloxkeep(r: &mut Root, cx: &mut Context<Root>) {
    ask(r, "checkUpdates", Value::Null, cx, |r, _| {
        let u = r.state.get("update").cloned().unwrap_or(Value::Null);
        let v = s(&u, "version").to_string();
        let p = u.get("progress").map(|p| p.to_string()).unwrap_or_default();
        let (key, tone) = if !b(&u, "supported") {
            ("upd.unsupported", "info")
        } else {
            match s(&u, "status") {
                "upToDate" => ("upd.latest", "info"),
                "ready" if b(&u, "quiet") => ("upd.readyQuiet", "info"),
                "ready" if b(&u, "auto") => ("upd.readyAuto", "info"),
                "ready" => ("upd.ready", "info"),
                "downloading" => ("upd.downloading", "info"),
                "error" => ("upd.error", "warn"),
                _ => ("upd.checking", "info"),
            }
        };
        let text = r.i18n.tv(key, &[("v", &v), ("p", &p)]);
        r.toast = Some((text, tone.into()));
    });
}

fn update_roblox(r: &mut Root, cx: &mut Context<Root>) {
    let l = r.state.pointer("/robloxVersion/updater").and_then(Value::as_str).unwrap_or_default().to_string();
    ask(r, "updateRoblox", Value::Null, cx, move |r, _| {
        let text = r.i18n.tv("rbx.updateStarted", &[("l", &l)]);
        r.toast = Some((text, "info".into()));
    });
}

/// A fact in the status card: its name small, its value under it.
fn fact(name: String, value: impl IntoElement) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(div().text_size(px(T_S)).text_color(c(MUTED)).child(name))
        .child(div().flex().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).child(value))
}

pub(super) fn roblox(root: &Root, cx: &mut Context<Root>) -> Vec<Div> {
    let st = st(root);
    let ru = root.state.get("robloxUpdates").unwrap_or(&Value::Null);
    let rv = root.state.get("robloxVersion").unwrap_or(&Value::Null);
    let closed = ru.get("closed").filter(|v| v.is_object());
    let installed = s(rv, "installed");
    let latest = s(rv, "latest");
    let up_to_date = rv.get("upToDate").and_then(Value::as_bool);
    let updater = s(rv, "updater");
    let updating = b(rv, "updating");
    let can_update = up_to_date == Some(false) && !updater.is_empty();
    let n_open = root.metrics.get("robloxCount").and_then(Value::as_i64).unwrap_or(0);
    let rv_text = if updating && !updater.is_empty() {
        root.i18n.tv("rbx.updateStarted", &[("l", updater)])
    } else if installed.is_empty() {
        root.tr("rbx.notInstalled")
    } else if can_update {
        root.i18n.tv("rbx.outdatedUpdate", &[("v", latest), ("l", updater)])
    } else if up_to_date == Some(false) {
        let k = if b(rv, "store") { "rbx.outdatedStore" } else if b(rv, "direct") { "rbx.outdatedDirect" } else { "rbx.outdated" };
        root.i18n.tv(k, &[("v", latest)])
    } else if up_to_date == Some(true) {
        root.tr("rbx.latest")
    } else {
        root.tr("rbx.unknown")
    };
    let inst = ru.get("installed").unwrap_or(&Value::Null);
    let source = s(inst, "source");
    // Roblox closed windows: the check's word is moot meanwhile, so it is greyed.
    let grey = |t: Tone| if closed.is_some() { Tone::Muted } else { t };
    let asking = root.settings.asking;
    let mut out = Vec::new();

    if let Some(cl) = closed {
        let n = cl.get("count").and_then(Value::as_i64).unwrap_or(0);
        let mut text = root.i18n.tn("ru.closed", n, &[("t", &when_text(root, s(cl, "at")))]);
        if !s(cl, "updatedAt").is_empty() {
            text = format!("{text} {}", root.i18n.tv("ru.closedUpdate", &[("t", &when_text(root, s(cl, "updatedAt")))]));
        }
        text = format!("{text} {}", root.tr("rw.todoClosed"));
        out.push(
            div()
                .px(px(ROW_X))
                .py(px(14.))
                .rounded(px(R_L))
                .bg(ca(WARN, 0.07))
                .border_1()
                .border_color(ca(WARN, 0.22))
                .flex()
                .items_start()
                .gap(px(12.))
                .child(div().pt(px(1.)).child(icon("alert", 16., c(WARN))))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(10.))
                        .child(div().text_size(px(T_M)).text_color(c(TEXT_2)).child(text))
                        .child(
                            div()
                                .flex()
                                .gap(px(8.))
                                .child(Button::new("rw-check", root.tr("rw.checkBloxkeep")).small().on_click(cx.listener(|r, _, _, cx| check_bloxkeep(r, cx))))
                                .child(Button::new("ru-hide", root.tr("ru.hide")).small().quiet().on_click(cx.listener(|r, _, _, _| r.send("hideRobloxClosed", Value::Null)))),
                        ),
                ),
        );
    }

    // ── the status card: what is installed and whether it is the newest; the update; the Multi-Roblox check
    let pill_tone = match up_to_date {
        Some(false) => Tone::Warn,
        Some(true) => Tone::Ok,
        None => Tone::Muted,
    };
    let build_tip = (!s(inst, "build").is_empty()).then(|| root.i18n.tv("ru.build", &[("b", &short_build(s(inst, "build")))]));
    let w = ru.get("watch").unwrap_or(&Value::Null);
    let word = watch_word(root, s(w, "check"), b(w, "worked"), "rw")
        .unwrap_or_else(|| (Tone::Muted, root.tr(if s(w, "readAt").is_empty() { "rbx.unknown" } else { "rw.unchecked" })));
    let head = div()
        .p(px(20.))
        .flex()
        .items_start()
        .gap(px(16.))
        .child(div().flex_none().child(icon("gamepad", 28., c(TEXT))))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .child(tip(
                            div().id("ru-build").text_size(px(T_SEC)).font_weight(FontWeight::BOLD).child(if installed.is_empty() { "Roblox".to_string() } else { format!("Roblox {installed}") }),
                            build_tip,
                        ))
                        .child(badge(
                            match up_to_date {
                                Some(true) => root.tr("rbx.latest"),
                                Some(false) => root.i18n.tv("set.rbx.newer", &[("v", latest)]),
                                None => root.tr(if installed.is_empty() { "rbx.notInstalled" } else { "rbx.unknown" }),
                            },
                            pill_tone,
                        )),
                )
                // (Up to date: the badge says it already, so the line under it says what that was checked against.)
                .child(div().text_size(px(T_M)).text_color(c(TEXT_2)).child(if up_to_date == Some(true) && !updating {
                    root.tr("set.rbx.same")
                } else {
                    rv_text
                })),
        )
        .when(can_update, |d| {
            d.child(
                Button::new("rbx-update", root.tr(if updating { "rbx.updating" } else { "rbx.update" })).icon("download").disabled(updating).on_click(cx.listener(move |r, _, _, cx| {
                    if n_open > 0 {
                        r.settings.asking = Some("rbx-update");
                        cx.notify();
                    } else {
                        update_roblox(r, cx);
                    }
                })),
            )
        });
    let facts = div()
        .px(px(ROW_X))
        .py(px(16.))
        .flex()
        .gap(px(20.))
        .child(fact(root.tr("ru.newest"), if latest.is_empty() { root.tr("rbx.unknown") } else { latest.to_string() }))
        .child(fact(root.tr("rw.title"), badge(word.1.clone(), grey(word.0))))
        .child(fact(root.tr("set.rbx.source"), if source.is_empty() { "—".to_string() } else { source.to_string() }));
    let mut card: Vec<AnyElement> = vec![head.into_any_element(), facts.into_any_element()];
    // Roblox's own installer can't leave windows open: they close and open again.
    let reopens = rv.get("reopens").and_then(Value::as_i64).unwrap_or(0);
    let reopen_q = if reopens > 0 {
        root.i18n.tn("rbx.reopenQ", n_open, &[("m", &reopens.to_string())])
    } else {
        root.i18n.tn("rbx.reopenNone", n_open, &[])
    };
    if asking == Some("rbx-update") {
        let q = if updater == "Roblox" { reopen_q.clone() } else { root.i18n.tn("rbx.updateQ", n_open, &[("l", updater)]) };
        card.push(question(root, "rbx-update-q", root.tr("rbx.updateTitle"), q, root.tr("rbx.updateOk"), false, cx, |r, cx| update_roblox(r, cx)));
    }
    let old = rv.get("oldWindows").and_then(Value::as_i64).unwrap_or(0);
    if old > 0 {
        card.push(row(
            root.i18n.tn("rbx.oldWindows", old, &[]),
            Some(root.tr("rbx.oldWindowsDesc")),
            Button::new("rbx-reopen", root.tr("rbx.reopen")).small().disabled(updating).on_click(cx.listener(|r, _, _, cx| {
                r.settings.asking = Some("rbx-reopen");
                cx.notify();
            })),
        ));
        if asking == Some("rbx-reopen") {
            card.push(question(root, "rbx-reopen-q", root.tr("rbx.reopenTitle"), reopen_q, root.tr("rbx.reopenOk"), false, cx, |r, cx| {
                ask(r, "updateAndReopen", Value::Null, cx, |_, _| {})
            }));
        }
    }
    out.push(group(None, card));

    // ── updating by itself
    let auto = st.get("autoUpdateRoblox").and_then(Value::as_bool) != Some(false);
    // When it goes in: once no window is open (accounts opened before then wait, or play on the build running), or at
    // once after closing them all (like Close all).
    let when = match s(st, "robloxUpdateWhen") {
        "now" => "now",
        "keep" => "keep",
        _ => "idle",
    };
    let mut upd = vec![row(
        root.tr("rbx.auto"),
        Some(root.tr(match when {
            "now" => "rbx.autoDescNow",
            "keep" => "rbx.autoDescKeep",
            _ => "rbx.autoDesc",
        })),
        switch("s-rbx-auto", auto, cx, |r, on, cx| set(r, json!({ "autoUpdateRoblox": on }), cx)),
    )];
    if auto {
        let items = ["idle", "keep", "now"].into_iter().map(|k| (k, root.tr(&format!("rbx.when.{k}")), false)).collect();
        upd.push(row(root.tr("rbx.when"), None, chips(root, cx, "s-rbx-when", items, &when, |r, v, cx| set(r, json!({ "robloxUpdateWhen": v }), cx))));
    }
    out.push(group(None, upd));

    // ── the Multi-Roblox check: what it is, the next build, what to do when it warns
    let mut watch_desc = Vec::new();
    if b(w, "updatesFirst") && !s(w, "version").is_empty() {
        watch_desc.push(root.i18n.tv("rw.updatesTo", &[("v", s(w, "version"))]));
    }
    watch_desc.push(root.tr("rw.desc"));
    let held: Vec<&str> = w.get("held").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    if !held.is_empty() {
        watch_desc.push(root.i18n.tv("rw.held", &[("n", &held.join(", "))]));
    }
    let mut watch: Vec<AnyElement> = vec![row(
        root.tr("rw.title"),
        Some(watch_desc.join(" ")),
        Button::new("rw-how", root.tr("rw.how")).small().quiet().icon_end("external").on_click(cx.listener(|r, _, _, _| r.send("openRobloxWatch", Value::Null))),
    )];
    let next = w.get("next").filter(|v| v.is_object());
    if let Some((tone, words)) = next.and_then(|n| watch_word(root, s(n, "check"), false, "rw")) {
        let v = next.map(|n| s(n, "version")).unwrap_or_default();
        watch.push(row(root.tr("rw.next"), Some(root.i18n.tv("rw.nextDesc", &[("v", v)])), badge(words, grey(tone))));
    }
    let warns = ["changed", "renamed", "gone"].contains(&s(w, "check")) && !b(w, "worked");
    if warns && closed.is_none() {
        watch.push(row_el(
            root.tr("rw.todo"),
            div().text_size(px(T_M)).font_weight(FontWeight::NORMAL).text_color(c(TEXT_2)).child(root.tr("rw.todo")),
            None,
            Button::new("rw-check2", root.tr("rw.checkBloxkeep")).small().on_click(cx.listener(|r, _, _, cx| check_bloxkeep(r, cx))),
        ));
    }
    out.push(group(None, watch));

    // ── builds seen here, newest first
    let hist: Vec<Value> = ru.get("history").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut list: Vec<AnyElement> = Vec::new();
    for (i, h) in hist.iter().enumerate() {
        let older = hist.get(i + 1);
        let note_word = if b(h, "first") {
            root.tr("ru.first")
        } else if older.is_some_and(|o| s(o, "version") == s(h, "version")) {
            root.tr("ru.sameVersion")
        } else {
            String::new()
        };
        let check = watch_word(root, s(h, "check"), b(h, "worked"), "rw.h");
        let mut sub = when_text(root, s(h, "at"));
        if !s(h, "source").is_empty() {
            sub = format!("{sub} · {}", s(h, "source"));
        }
        if !note_word.is_empty() {
            sub = format!("{sub} · {note_word}");
        }
        let version = s(h, "version").to_string();
        list.push(
            div()
                .px(px(ROW_X))
                .py(px(12.))
                .flex()
                .items_center()
                .gap(px(14.))
                .child(div().size(px(8.)).flex_none().rounded_full().bg(if i == 0 { c(TEXT) } else { c(DIM) }))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(3.))
                        .child(div().flex().items_center().gap(px(10.)).child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).child(version.clone())).when_some(check, |d, (tone, words)| d.child(badge(words, tone))))
                        .child(tip(div().id(("ru-sub", i)).text_size(px(T_S)).text_color(c(MUTED)).child(sub), Some(root.i18n.tv("ru.build", &[("b", &short_build(s(h, "build")))])))),
                )
                .child(
                    Button::new(("ru-notes", i), root.tr("ru.notesShort"))
                        .small()
                        .quiet()
                        // (Roblox writes them for game creators, RobloxPage.tsx says.)
                        .tip(format!("{} · {}", root.tr("ru.notes"), root.tr("ru.notesHint")))
                        .icon_end("external")
                        .on_click(cx.listener(move |r, _, _, _| r.send("openReleaseNotes", json!({ "version": version.clone() })))),
                )
                .into_any_element(),
        );
    }
    if list.is_empty() {
        list.push(note("info", root.tr("ru.none"), MUTED));
    }
    out.push(group(Some(root.tr("ru.history")), list));
    out
}

// ───────────────────────── RobloxConfig ─────────────────────────

struct Level {
    id: &'static str,
    mode: &'static str,
    graphics: i64,
    fps: i64,
    volume: i64,
    ram: Option<i64>,
    trim: bool,
    cores: bool,
}

/// What each mode sets, all of it said on its card. "Play and farm" leaves the RAM limit as the person set it.
const LEVELS: [Level; 3] = [
    Level { id: "light", mode: "all", graphics: 1, fps: 20, volume: 0, ram: Some(-1), trim: true, cores: true },
    Level { id: "medium", mode: "all", graphics: 3, fps: 45, volume: -1, ram: None, trim: true, cores: true },
    Level { id: "off", mode: "off", graphics: 0, fps: 0, volume: -1, ram: Some(0), trim: false, cores: false },
];

fn int(v: &Value, k: &str, d: i64) -> i64 {
    v.get(k).and_then(Value::as_i64).unwrap_or(d)
}

/// The Roblox settings group, first in Staying in game.
pub(super) fn config(root: &Root, cx: &mut Context<Root>) -> Div {
    let st = st(root);
    let rc = st.get("robloxConfig").cloned().unwrap_or(json!({ "mode": "off", "graphics": 0, "fps": 0, "volume": -1 }));
    let mode = { let m = s(&rc, "mode"); if m.is_empty() { "off" } else { m } }.to_string();
    let (graphics, fps, volume) = (int(&rc, "graphics", 0), int(&rc, "fps", 0), int(&rc, "volume", -1));
    let accounts = root.accounts();
    let chosen = accounts.iter().filter(|a| b(a, "useRobloxConfig")).count() as i64;
    let own_fps = accounts.iter().filter(|a| int(a, "fpsCap", 0) > 0).count() as i64;
    let level = LEVELS
        .iter()
        .find(|l| {
            let rc_same = if l.mode == "off" { mode == "off" && !b(st, "fastFlags") } else { mode == l.mode && graphics == l.graphics && fps == l.fps && volume == l.volume };
            rc_same && l.ram.is_none_or(|r| int(st, "ramLimitMb", 0) == r) && b(st, "trimBackground") == l.trim && b(st, "splitCores") == l.cores
        })
        .map(|l| l.id);
    // Custom is shown on request, or by itself once the settings match no mode.
    let shown = if root.settings.rc_custom || level.is_none() { "own" } else { level.unwrap_or("own") };

    let mut cards = div().px(px(ROW_X)).pt(px(18.)).pb(px(12.)).flex().gap(px(10.));
    for (i, id) in ["light", "medium", "off", "own"].into_iter().enumerate() {
        let on = id == shown;
        let says = root.tr(&format!("rc.level.{id}.says"));
        cards = cards.child(
            focusable(div().id(("rc-level", i)))
                .flex_1()
                .min_w(px(0.))
                .p(px(14.))
                .rounded(px(R_M))
                .flex()
                .flex_col()
                .gap(px(8.))
                .border_1()
                .border_color(if on { ca(WHITE, 0.5) } else { c(LINE) })
                .bg(if on { ca(WHITE, 0.06) } else { c(INSET) })
                .when(!on, |d| d.cursor_pointer().hover(|d| d.border_color(ca(WHITE, 0.18))))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(div().flex_1().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).child(root.tr(&format!("rc.level.{id}"))))
                        .child(
                            div()
                                .size(px(16.))
                                .flex_none()
                                .rounded_full()
                                .border_1()
                                .border_color(if on { c(TEXT) } else { ca(WHITE, 0.25) })
                                .bg(if on { c(TEXT) } else { ca(WHITE, 0.) })
                                .flex()
                                .items_center()
                                .justify_center()
                                .when(on, |d| d.child(icon("check", 11., c(INK)))),
                        ),
                )
                .child(div().text_size(px(T_S)).text_color(c(MUTED)).child(says))
                .on_click(cx.listener(move |r, _, _, cx| {
                    if id == "own" {
                        r.settings.rc_custom = true;
                        cx.notify();
                        return;
                    }
                    r.settings.rc_custom = false;
                    if Some(id) == level {
                        cx.notify();
                        return;
                    }
                    let l = LEVELS.iter().find(|l| l.id == id).expect("a level");
                    let mut v = json!({
                        "robloxConfig": { "mode": l.mode, "graphics": l.graphics, "fps": l.fps, "volume": l.volume },
                        "trimBackground": l.trim, "splitCores": l.cores,
                    });
                    if let Some(ram) = l.ram {
                        v["ramLimitMb"] = json!(ram);
                    }
                    if l.id == "off" {
                        v["fastFlags"] = json!(false);
                    }
                    set(r, v, cx);
                })),
        );
    }
    let mut notes = div().px(px(ROW_X)).pb(px(16.)).flex().flex_col().gap(px(3.)).text_size(px(T_S)).text_color(c(MUTED)).child(root.tr("rc.reopen"));
    if own_fps > 0 {
        notes = notes.child(root.i18n.tn("rc.ownFps", own_fps, &[]));
    }
    let mut rows: Vec<AnyElement> = vec![div().flex().flex_col().child(cards).child(notes).into_any_element()];

    // Fast Flags go to the same accounts as the rest; a Bloxstrap-style launcher writes its own over Bloxkeep's: said here.
    let updater = root.state.pointer("/robloxVersion/updater").and_then(Value::as_str).unwrap_or_default();
    let overridden = !updater.is_empty() && updater != "Roblox" && s(st, "launchVia") != "direct";
    let mut flags_desc = root.tr("rc.flagsDesc");
    if overridden {
        flags_desc = format!("{flags_desc} {}", root.i18n.tv("rc.flagsOver", &[("l", updater)]));
    }
    let flags = |cx: &mut Context<Root>| row(root.tr("rc.flags"), Some(flags_desc.clone()), switch("s-rc-flags", b(st, "fastFlags"), cx, |r, on, cx| set(r, json!({ "fastFlags": on }), cx)));
    if shown != "own" {
        if mode != "off" {
            rows.push(flags(cx));
        }
    } else {
        let modes = ["off", "all", "marked"].into_iter().map(|m| (m.to_string(), root.tr(&format!("rc.mode.{m}")), false)).collect();
        let hint = match mode.as_str() {
            "off" => None,
            "all" => Some(root.tr("rc.hint.all")),
            _ => Some(root.i18n.tn("rc.hint.chosen", chosen, &[])),
        };
        let picking = root.settings.rc_pick.is_some();
        let pick_link = (mode == "marked" && !picking).then(|| {
            link_text("rc-pick", root.tr("rc.pick"), cx, |r, _, cx| {
                if r.accounts().is_empty() {
                    r.toast = Some((r.tr("rc.noAccounts"), "info".into()));
                } else {
                    r.settings.rc_pick = Some(r.accounts().iter().filter(|a| b(a, "useRobloxConfig")).map(|a| s(a, "id").to_string()).collect());
                }
                cx.notify();
            })
        });
        let desc = div()
            .flex()
            .items_center()
            .gap(px(10.))
            .when_some(hint, |d, h| d.child(div().text_size(px(T_S)).font_weight(FontWeight::NORMAL).text_color(c(MUTED)).child(h)))
            .children(pick_link);
        rows.push(row_el(
            root.tr("rc.mode"),
            div().flex().flex_col().gap(px(3.)).child(root.tr("rc.mode")).child(desc),
            None,
            chips(root, cx, "s-rc-mode", modes, &mode, |r, v, cx| set(r, json!({ "robloxConfig": { "mode": v } }), cx)),
        ));
        if picking {
            rows.push(rc_picker(root, cx));
        }
        if mode != "off" {
            rows.push(flags(cx));
            let keep = root.tr("rc.keep");
            let pick = |list: &[i64], v: i64| if list.contains(&v) { v } else { list[0] };
            let gl: Vec<i64> = std::iter::once(0).chain(1..=10).collect();
            let fl: Vec<i64> = vec![0, 15, 20, 30, 45, 60, 120, 144, 240];
            let vl: Vec<i64> = vec![-1, 0, 25, 50, 75, 100];
            let g_items = gl.iter().map(|&n| (n, if n == 0 { keep.clone() } else { n.to_string() }, false)).collect();
            let f_items = fl.iter().map(|&n| (n, if n == 0 { keep.clone() } else { format!("{n} FPS") }, false)).collect();
            let v_items = vl
                .iter()
                .map(|&n| (n, if n == -1 { keep.clone() } else if n == 0 { root.tr("rc.mute") } else { format!("{n}%") }, false))
                .collect();
            rows.push(row_below(root.tr("rc.graphics"), None, chips(root, cx, "s-rc-graphics", g_items, &pick(&gl, graphics), |r, v, cx| set(r, json!({ "robloxConfig": { "graphics": v } }), cx))));
            rows.push(row_below(root.tr("rc.fps"), None, chips(root, cx, "s-rc-fps", f_items, &pick(&fl, fps), |r, v, cx| set(r, json!({ "robloxConfig": { "fps": v } }), cx))));
            rows.push(row_below(root.tr("rc.volume"), None, chips(root, cx, "s-rc-volume", v_items, &pick(&vl, volume), |r, v, cx| set(r, json!({ "robloxConfig": { "volume": v } }), cx))));
        }
    }
    group(Some(root.tr("rc.title")), rows)
}

/// The accounts that get the settings, ticked in place under the row; Done saves them.
fn rc_picker(root: &Root, cx: &mut Context<Root>) -> AnyElement {
    let ticked = root.settings.rc_pick.clone().unwrap_or_default();
    let mut list: Vec<Value> = root.accounts().to_vec();
    list.sort_by_key(|a| crate::pages::accounts::label(a).to_lowercase());
    let all: Vec<String> = list.iter().map(|a| s(a, "id").to_string()).collect();
    picker(
        root,
        "rc-pick",
        root.tr("rc.pickText"),
        list.iter().map(|a| (s(a, "id").to_string(), crate::pages::accounts::label(a), at_name(a))).collect(),
        &ticked,
        cx,
        |r, id, on| {
            if let Some(set) = r.settings.rc_pick.as_mut() {
                if on { set.insert(id) } else { set.remove(&id) };
            }
        },
        vec![],
        move |r, cx| {
            let on: Vec<String> = r.settings.rc_pick.clone().unwrap_or_default().into_iter().collect();
            let off: Vec<String> = all.iter().filter(|id| !on.contains(id)).cloned().collect();
            let n = on.len() as i64;
            let Some(link) = r.link.clone() else { return };
            let calls = [(on, true), (off, false)];
            cx.spawn(async move |this, cx| {
                for (ids, enabled) in calls {
                    if ids.is_empty() {
                        continue;
                    }
                    match link.call("setUseRobloxConfig", json!({ "ids": ids, "enabled": enabled })).await {
                        Ok(Ok(_)) => {}
                        Ok(Err(e)) => {
                            let _ = this.update(cx, |r, cx| {
                                r.toast = Some((e, "bad".into()));
                                cx.notify();
                            });
                            return;
                        }
                        // No answer (the app's pipe dropped): the picker stays open, nothing said picked.
                        Err(_) => return,
                    }
                }
                let _ = this.update(cx, |r, cx| {
                    r.settings.rc_pick = None;
                    let text = r.i18n.tn("rc.picked", n, &[]);
                    ok(r, text);
                    cx.notify();
                });
            })
            .detach();
        },
        |r| r.settings.rc_pick = None,
    )
}

/// "@name" under an account's label in a picker.
pub(super) fn at_name(a: &Value) -> String {
    let u = s(a, "username");
    if u.is_empty() { String::new() } else { format!("@{u}") }
}
