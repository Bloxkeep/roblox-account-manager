// The lock screen, as the web page's (ui/src/Lock.tsx, Vaults.tsx; app.js renderLock): the master password, or the first
// time (or for a vault being added) a new one typed twice with its strength; "Restore from a backup" and "Create a new
// vault"; with several vaults, which one this is and Switch vault, or the picker; an update found before the unlock
// installing; and, with no vault yet, Multi-Roblox alone. The app decides which (vaults.pick, vaults.adding,
// vaultExists) and answers "unlock" / "createVault" with the opened vault's state, or with its sentence for why not.
// The picker and the new-vault form also come up over an open vault (Switch vault): Back returns to it.
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use gpui_kit::component::input::{Input, InputState};

use crate::{kit::*, shell::Root, theme::*};

#[derive(Default)]
pub struct Data {
    /// An answer is being waited for, and what the last try came to (the app's words, or this screen's own).
    busy: bool,
    said: Option<String>,
    /// The box the problem is about (1, 2): its edge is red until something is typed again.
    wrong: u8,
    /// The password box has been given the keyboard since the screen came up.
    pub focused: bool,
    /// What is typed shows as it is, not as dots (the eye).
    shown: bool,
    /// How many tries were turned down (each one shakes the form), and how many of those by the app (a wrong
    /// password): from the second, the way on is pointed out.
    noes: u32,
    refused: u32,
    /// An update found before the unlock: taken (the screen shows it installing) or left for later. None until decided.
    updating: Option<bool>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Create,
    Unlock,
    Pick,
}

static NONE: Value = Value::Null;

fn vaults(r: &Root) -> &Value {
    r.state.get("vaults").unwrap_or(&NONE)
}

fn flag(v: &Value, k: &str) -> bool {
    v.get(k).and_then(Value::as_bool).unwrap_or(false)
}

/// A vault is being added: the form asks its name too.
pub fn adding(r: &Root) -> bool {
    flag(vaults(r), "adding")
}

pub fn mode(r: &Root) -> Mode {
    if flag(vaults(r), "pick") {
        Mode::Pick
    } else if adding(r) || r.state.get("vaultExists").and_then(Value::as_bool) == Some(false) {
        Mode::Create
    } else {
        Mode::Unlock
    }
}

/// This screen is up: the vault is closed, or the picker / the new-vault form is over the open one.
pub fn up(r: &Root) -> bool {
    r.locked() || flag(vaults(r), "pick") || adding(r)
}

/// Something was typed in a password box: the old problem goes.
pub fn typed(r: &mut Root) {
    r.lock.said = None;
    r.lock.wrong = 0;
}

/// An update found before the unlock takes the screen while it installs, unless something was already typed (they're
/// on their way in); decided once per update offered, and the app keeps the answer (Lock.tsx).
pub fn settle(r: &mut Root, cx: &mut Context<Root>) {
    let offer = mode(r) != Mode::Pick && r.state.get("lockUpdate").is_some_and(Value::is_object);
    if !offer {
        r.lock.updating = None;
        return;
    }
    if r.lock.updating.is_some() {
        return;
    }
    let go = r.state.pointer("/lockUpdate/go").and_then(Value::as_bool);
    let typed = !r.lock_pw.read(cx).value().is_empty() || !r.lock_pw2.read(cx).value().is_empty();
    let take = go.unwrap_or(!typed);
    r.lock.updating = Some(take);
    if go.is_none() {
        r.send("updateAtLock", json!({ "go": take }));
    }
}

/// A command of this screen's; the app's refusal is said as a notice.
fn run(r: &mut Root, cmd: &'static str, args: Value, cx: &mut Context<Root>) {
    let Some(link) = r.link.clone() else { return };
    let rx = link.call(cmd, args);
    cx.spawn(async move |this, cx| {
        if let Ok(Err(why)) = rx.await {
            let _ = this.update(cx, |r, cx| {
                r.toast = Some((why.to_string(), "bad".into()));
                cx.notify();
            });
        }
    })
    .detach();
}

/// Enter in a box, or the button.
pub fn submit(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let mode = mode(r);
    if r.lock.busy || mode == Mode::Pick {
        return;
    }
    let Some(link) = r.link.clone() else { return };
    let create = mode == Mode::Create;
    let pw = r.lock_pw.read(cx).value().to_string();
    let refuse = |r: &mut Root, key: &str, wrong: u8, cx: &mut Context<Root>| {
        r.lock.said = Some(r.tr(key));
        r.lock.wrong = wrong;
        r.lock.noes += 1;
        cx.notify();
    };
    if create {
        if pw.chars().count() < 6 {
            return refuse(r, "lock.short", 1, cx);
        }
        if pw.trim().is_empty() {
            return refuse(r, "lock.blank", 1, cx);
        }
        if pw != r.lock_pw2.read(cx).value().as_ref() {
            return refuse(r, "lock.mismatch", 2, cx);
        }
    } else if pw.is_empty() {
        return refuse(r, "lock.empty", 1, cx);
    }
    let name = if adding(r) { r.vault_name.read(cx).value().to_string() } else { String::new() };
    r.lock.busy = true;
    typed(r);
    cx.notify();
    let rx = link.call(if create { "createVault" } else { "unlock" }, json!({ "password": pw, "name": name }));
    cx.spawn_in(window, async move |this, cx| {
        let answer = rx.await;
        let _ = this.update_in(cx, |r, window, cx| {
            r.lock.busy = false;
            match answer {
                // The app's state, with the vault open (it sends it again as it goes on loading).
                Ok(Ok(state)) if state.is_object() => r.take_state(state, window, cx),
                Ok(Ok(_)) => {}
                Ok(Err(why)) => {
                    r.lock.said = Some(why);
                    r.lock.wrong = 1;
                    r.lock.noes += 1;
                    r.lock.refused += 1;
                }
                Err(_) => r.lock.said = Some(r.tr("common.noBridge")),
            }
            cx.notify();
        });
    })
    .detach();
}

/// Words with the page's marks taken out (the web page's text has <b>…</b> in it).
pub fn plain(text: String) -> String {
    let mut out = String::with_capacity(text.len());
    let mut inside = false;
    for ch in text.chars() {
        match ch {
            '<' => inside = true,
            '>' if inside => inside = false,
            _ if !inside => out.push(ch),
            _ => {}
        }
    }
    out
}

/// A master password's strength (app.js strengthScore / strengthLevel): only a guide, any 6+ characters do.
pub fn strength(v: &str) -> (usize, &'static str) {
    let n = v.chars().count();
    let score = [
        n >= 8,
        n >= 12,
        v.chars().any(|c| c.is_ascii_lowercase()) && v.chars().any(|c| c.is_ascii_uppercase()),
        v.chars().any(|c| c.is_ascii_digit()),
        v.chars().any(|c| !c.is_ascii_alphanumeric()),
    ]
    .iter()
    .filter(|x| **x)
    .count();
    (score, if score <= 1 { "weak" } else if score <= 3 { "ok" } else { "strong" })
}

/// "5 minutes", "1 hour", "2 hours" (app.js autoLockText).
fn auto_text(root: &Root, minutes: i64) -> String {
    if minutes < 60 {
        root.i18n.tv("lock.autoMin", &[("n", &minutes.to_string())])
    } else if minutes == 60 {
        root.tr("lock.autoHour1")
    } else {
        root.i18n.tv("lock.autoHours", &[("n", &(minutes / 60).to_string())])
    }
}

/// A vault's face: its picture, or the first letter of its name on a colour of its own (the same one every time:
/// Vaults.tsx's FNV-1a over its id).
pub fn face(id: &str, name: &str, picture: Option<&str>, size: f32, radius: f32, letter: f32) -> Div {
    const COLORS: [u32; 8] = [0x2563eb, 0x15803d, 0xb45309, 0xdc2626, 0x7c3aed, 0x0e7490, 0xbe185d, 0x0f766e];
    let mut h: u32 = 2166136261;
    for u in id.encode_utf16() {
        h = (h ^ u as u32).wrapping_mul(16777619);
    }
    h ^= h >> 16;
    h = h.wrapping_mul(0x45d9f3b);
    h ^= h >> 16;
    let base = div().size(px(size)).flex_none().rounded(px(radius)).overflow_hidden().flex().items_center().justify_center();
    match picture.and_then(crate::images::user) {
        Some(file) => base.child(img(file).size_full().object_fit(ObjectFit::Cover)),
        None => {
            let first: String = name.trim().chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_else(|| "?".into());
            base.bg(c(COLORS[(h % 8) as usize])).text_color(c(WHITE)).text_size(px(letter)).font_weight(FontWeight::BOLD).child(first)
        }
    }
}

/// The faint dots behind the card: one every 26 px, fading out from the middle (the web page's .dots and its mask).
/// Dots per side of one painted block of the grid.
const DOT_BLOCK: usize = 4;
/// Faintness steps a block can take.
const DOT_LEVELS: usize = 16;

thread_local! {
    /// A block of dots at each faintness, drawn at the window's pixels (its side in pixels, the pictures).
    static DOT_TILES: std::cell::RefCell<Option<(u32, Vec<std::sync::Arc<RenderImage>>)>> = const { std::cell::RefCell::new(None) };
}

/// A block of DOT_BLOCK × DOT_BLOCK dots `side` pixels square, `scale` pixels a point, at faintness `a`.
fn dot_tile(side: u32, scale: f32, a: f32) -> Option<RenderImage> {
    let mut px_buf = vec![0u8; (side * side * 4) as usize];
    let alpha = (a * 255.).round().clamp(0., 255.) as u8;
    for i in 0..DOT_BLOCK {
        for j in 0..DOT_BLOCK {
            // Each dot 2 points square around 13 + 26·n, as the grid had them.
            let (x0, y0) = (((12. + 26. * i as f32) * scale).round() as u32, ((12. + 26. * j as f32) * scale).round() as u32);
            let (x1, y1) = (((14. + 26. * i as f32) * scale).round() as u32, ((14. + 26. * j as f32) * scale).round() as u32);
            for y in y0..y1.min(side) {
                for x in x0..x1.min(side) {
                    let at = ((y * side + x) * 4) as usize;
                    px_buf[at..at + 4].copy_from_slice(&[255, 255, 255, alpha]);
                }
            }
        }
    }
    let buffer = image::RgbaImage::from_raw(side, side, px_buf)?;
    Some(RenderImage::new(vec![image::Frame::new(buffer)]))
}

/// A faint grid of dots, brightest in the middle. Drawn as blocks of dots made once per window scale (about 350 pictures
/// on a big screen), not one shape a dot (about 5000 every frame).
fn dots() -> impl IntoElement {
    canvas(
        |_, _, _| (),
        |bounds, _, window, _| {
            let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
            let reach = 0.7 * (w * w + h * h).sqrt() / 2.;
            let scale = window.scale_factor();
            let block = 26. * DOT_BLOCK as f32;
            let side = (block * scale).round().max(1.) as u32;
            let tiles = DOT_TILES.with(|t| {
                let mut t = t.borrow_mut();
                if t.as_ref().is_none_or(|(s, _)| *s != side) {
                    if let Some((_, old)) = t.take() {
                        for o in old {
                            let _ = window.drop_image(o);
                        }
                    }
                    let made: Option<Vec<_>> = (0..DOT_LEVELS).map(|l| dot_tile(side, scale, 0.11 * (l as f32 + 1.) / DOT_LEVELS as f32).map(std::sync::Arc::new)).collect();
                    *t = Some((side, made.unwrap_or_default()));
                }
                t.as_ref().map(|(_, v)| v.clone()).unwrap_or_default()
            });
            if tiles.len() != DOT_LEVELS {
                return;
            }
            let snap = |v: f32| (v * scale).round() / scale;
            let shown = side as f32 / scale;
            let mut by = 0.;
            while by < h {
                let mut bx = 0.;
                while bx < w {
                    // The faintness at the block's middle, in steps.
                    let (cx_, cy_) = (bx + block / 2., by + block / 2.);
                    let far = ((cx_ - w / 2.).powi(2) + (cy_ - h / 2.).powi(2)).sqrt();
                    let a = 0.11 * (1. - far / reach);
                    if a > 0.004 {
                        let level = ((a / 0.11 * DOT_LEVELS as f32).round() as usize).clamp(1, DOT_LEVELS) - 1;
                        let x = snap(f32::from(bounds.origin.x) + bx);
                        let y = snap(f32::from(bounds.origin.y) + by);
                        let at = Bounds::new(point(px(x), px(y)), size(px(shown), px(shown)));
                        let _ = window.paint_image(at, at, Corners::all(px(0.)), tiles[level].clone(), 0, false);
                    }
                    bx += block;
                }
                by += block;
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// EN | VI, top right: the language, before anything is unlocked.
fn lang_switch(root: &Root, cx: &mut Context<Root>) -> Div {
    let vi = root.i18n.vietnamese;
    let mut row = div().absolute().top(px(18.)).right(px(22.)).flex().gap(px(2.)).p(px(3.)).rounded(px(R_M)).bg(c(RAISED));
    for (lang, label, on) in [("en", "EN", !vi), ("vi", "VI", vi)] {
        row = row.child(
            segment(SharedString::from(format!("lock-lang-{lang}")), on)
                .h(px(26.))
                .px(px(10.))
                .text_size(px(T_S))
                .font_weight(FontWeight::SEMIBOLD)
                .cursor_pointer()
                .child(label)
                .on_click(cx.listener(move |r, _, _, cx| {
                    if r.i18n.vietnamese != (lang == "vi") {
                        r.send("setLanguage", json!({ "language": lang }));
                    }
                    cx.notify();
                })),
        );
    }
    row
}

/// A link in the card's last lines.
fn link(id: &'static str, label: String) -> Stateful<Div> {
    focusable(div().id(id)).text_size(px(T_M)).text_color(c(TEXT_2)).cursor_pointer().hover(|s| s.text_color(c(TEXT))).child(label)
}

fn card() -> Div {
    div()
        .w(px(456.))
        .pt(px(40.))
        .px(px(40.))
        .pb(px(32.))
        .flex()
        .flex_col()
        .items_center()
        .rounded(px(24.))
        .bg(c(PANEL))
        .border_1()
        .border_color(ca(WHITE, 0.07))
        .shadow(menu_shadow(0.8))
}

/// Under the card, near the foot of the window: whose screen this is, quietly (Bloxkeep, what it is, its version).
fn signature(root: &Root) -> Div {
    let version = root.state.pointer("/update/current").and_then(Value::as_str).unwrap_or_default();
    let mut parts = vec![("Bloxkeep".to_string(), None), (root.tr("brand.sub"), None)];
    if !version.is_empty() {
        parts.push((format!("v{version}"), None));
    }
    div().absolute().bottom(px(26.)).left_0().right_0().flex().justify_center().child(facts(parts).text_size(px(T_M)).text_color(c(MUTED)))
}

/// The main button of the card, full width and flat (the raised base under it was taken out, 2026-11-12).
fn go_block(el: impl IntoElement) -> Div {
    div().w_full().child(el)
}

/// Back, top left: to the open vault (the picker is over it), or out of the new vault's form.
fn back(root: &Root, cmd: &'static str, cx: &mut Context<Root>) -> Div {
    div().absolute().top(px(18.)).left(px(18.)).child(
        Button::new("vp-back", root.tr("vault.back")).icon("chevronLeft").on_click(cx.listener(move |r, _, _, cx| run(r, cmd, json!({}), cx))),
    )
}

pub fn lock(root: &Root, window: &mut Window, cx: &mut Context<Root>) -> AnyElement {
    let mode = mode(root);
    let layer = div().size_full().relative().child(dots()).child(lang_switch(root, cx));
    let v = vaults(root);
    let current = v.get("current").and_then(Value::as_str).unwrap_or_default();
    if mode == Mode::Pick {
        return layer
            .when(!current.is_empty(), |d| d.child(back(root, "closeVaultPicker", cx)))
            .child(div().size_full().flex().items_center().justify_center().child(picker(root, window, cx)))
            .into_any_element();
    }
    let adding = adding(root);
    let layer = layer.when(adding, |d| d.child(back(root, "cancelAddVault", cx)));
    if root.lock.updating == Some(true) {
        return layer.child(div().size_full().flex().items_center().justify_center().child(update(root, window, cx))).child(signature(root)).into_any_element();
    }

    let create = mode == Mode::Create;
    let busy = root.lock.busy;
    let screen_lock = root.state.get("screenLock").is_some_and(Value::is_object);
    let listed = v.get("list").and_then(Value::as_array).map_or(0, |l| l.len());
    let boxed = |state: &Entity<InputState>, bad: bool| {
        div()
            .w_full()
            .h(px(46.))
            .pl(px(14.))
            .pr(px(6.))
            .flex()
            .items_center()
            .gap(px(4.))
            .rounded(px(R_M))
            .bg(c(INSET))
            .border_1()
            // As the kit's fields: a border that can be seen at rest, stronger under the pointer.
            .border_color(if bad { c(BAD) } else { ca(TEXT, 0.26) })
            .when(!bad, |d| d.hover(|s| s.border_color(ca(TEXT, 0.5))))
            .text_size(px(T_L))
            .text_color(c(TEXT))
            .child(div().flex_1().min_w_0().child(Input::new(state).appearance(false)))
    };
    // The eye, in the first box: both boxes show what is typed, or hide it again.
    let eye = IconButton::new("lock-eye", if root.lock.shown { "eyeOff" } else { "eye" })
        .size(32.)
        .tip(root.tr(if root.lock.shown { "lock.hidePw" } else { "lock.showPw" }))
        .on_click(cx.listener(|r, _, window, cx| {
            r.lock.shown = !r.lock.shown;
            let masked = !r.lock.shown;
            r.lock_pw.update(cx, |s, cx| s.set_masked(masked, window, cx));
            r.lock_pw2.update(cx, |s, cx| s.set_masked(masked, window, cx));
            cx.notify();
        }));
    // Locked by itself after the PC sat unused, or while it was in use: what kept running.
    let notice = if create {
        None
    } else if let Some(m) = root.state.pointer("/screenLock/minutes").and_then(Value::as_i64) {
        Some(root.i18n.tv("lock.auto", &[("t", &auto_text(root, m))]))
    } else if root.seen_open && root.locked() {
        Some(root.tr("lock.paused"))
    } else {
        None
    };
    let typed = root.lock_pw.read(cx).value().to_string();
    let (score, level) = strength(&typed);
    let caps = window.capslock().on;

    let mut lines: Vec<AnyElement> = Vec::new();
    // Which vault this is, once there are several (or one is being added), and the way to another.
    if !current.is_empty() && (listed > 1 || adding) {
        let name = v.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
        lines.push(
            div()
                .mt(px(18.))
                .pl(px(5.))
                .pr(px(14.))
                .py(px(5.))
                .flex()
                .items_center()
                .justify_center()
                .gap(px(10.))
                .rounded(px(R_M + 2.))
                .bg(ca(WHITE, 0.045))
                .border_1()
                .border_color(c(LINE))
                .child(face(current, &name, v.get("picture").and_then(Value::as_str), 30., R_S, 14.))
                .child(div().max_w(px(180.)).truncate().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).child(name))
                .when(!adding && !screen_lock, |d| {
                    d.child(div().w(px(1.)).h(px(14.)).bg(c(LINE))).child(link("lock-switch", root.tr("vault.switch")).on_click(cx.listener(|r, _, _, cx| run(r, "pickVault", json!({}), cx))))
                })
                .into_any_element(),
        );
    }
    lines.push(
        div()
            .mt(px(22.))
            .text_size(px(T_STAGE))
            .line_height(px(32.))
            .font_weight(FontWeight::BOLD)
            .text_center()
            .child(root.tr(if adding { "vault.newTitle" } else if create { "lock.createTitle" } else { "lock.unlockTitle" }))
            .into_any_element(),
    );
    lines.push(
        div()
            .mt(px(8.))
            .mb(px(if notice.is_some() { 14. } else { 26. }))
            .text_size(px(T_L))
            .line_height(px(21.))
            .text_color(c(TEXT_2))
            .text_center()
            .child(plain(root.tr(if adding { "vault.newText" } else if create { "lock.createText" } else { "lock.unlockText" })))
            .into_any_element(),
    );
    if let Some(n) = notice {
        lines.push(div().mb(px(24.)).px(px(12.)).py(px(9.)).rounded(px(R_M)).bg(ca(WHITE, 0.04)).text_size(px(T_S)).line_height(px(17.)).text_color(c(MUTED)).text_center().child(n).into_any_element());
    }
    // (A try turned down shakes the boxes.)
    let mut stack = div().w_full().flex().flex_col().gap(px(10.));
    if adding {
        stack = stack.child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(div().text_size(px(T_S)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT_2)).child(root.tr("vault.name")))
                .child(boxed(&root.vault_name, false)),
        );
    }
    stack = stack.child(boxed(&root.lock_pw, root.lock.wrong == 1).child(eye));
    if create {
        let part = if typed.is_empty() { 0. } else { (score as f32 * 0.2).max(0.12) };
        let tone = match level {
            "weak" => BAD,
            "ok" => WARN,
            _ => OK,
        };
        stack = stack.child(boxed(&root.lock_pw2, root.lock.wrong == 2)).child(
            div()
                .h(px(16.))
                .flex()
                .items_center()
                .gap(px(10.))
                .child(div().flex_1().h(px(4.)).rounded_full().bg(c(RAISED_2)).child(div().h_full().rounded_full().bg(c(tone)).with_spring("pw-meter", SpringAnimation::new(EASE).to(part), |d, w| d.w(relative(w.clamp(0., 1.))))))
                // Before anything is typed the bar says what it is for (an empty bar with no word, review 8.2).
                .child(div().w(px(64.)).flex_none().text_size(px(T_S)).text_color(if typed.is_empty() { c(MUTED) } else { c(tone) }).child(root.tr(if typed.is_empty() { "lock.strength.none".to_string() } else { format!("lock.strength.{level}") }.as_str()))),
        );
    }
    lines.push(div().w_full().child(shake(root.lock.noes, stack)).into_any_element());
    if caps {
        lines.push(div().mt(px(10.)).flex().items_center().justify_center().gap(px(6.)).text_size(px(T_M)).text_color(c(WARN)).child(icon("alert", 14., c(WARN))).child(root.tr("lock.caps")).into_any_element());
    }
    lines.push(
        div()
            .w_full()
            .min_h(px(18.))
            .mt(px(6.))
            .mb(px(4.))
            .text_size(px(T_M))
            .line_height(px(18.))
            .text_color(c(BAD))
            .text_center()
            .child(root.lock.said.clone().unwrap_or_default())
            .into_any_element(),
    );
    // From the second refused password: the way on (the two links below; not when it locked by itself).
    if root.lock.refused >= 2 && !create && !screen_lock {
        lines.push(div().mb(px(10.)).text_size(px(T_M)).line_height(px(17.)).text_color(c(MUTED)).text_center().child(root.tr("lock.forgot")).into_any_element());
    }
    lines.push(go_block(
        focusable(div().id("lock-go"))
            .h(px(46.))
            .w_full()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(R_M))
            .bg(c(FACE))
            .text_color(c(INK))
            .text_size(px(T_L))
            .font_weight(FontWeight::SEMIBOLD)
            .gap(px(8.))
            .map(|d| if busy { d.opacity(0.75).child(spinner(15., c(INK))) } else { d.cursor_pointer().hover(|d| d.bg(c(FACE_HI))).active(|d| d.bg(c(FACE_DOWN))) })
            .on_click(cx.listener(|r, _, window, cx| submit(r, window, cx)))
            .child(root.tr(if create { "lock.create" } else { "lock.unlock" })),
    )
    .into_any_element());
    // Set up: a backup instead. Unlock: a backup (a forgotten master password can't be recovered) or another vault.
    if !screen_lock && !adding {
        lines.push(
            div()
                .mt(px(22.))
                .flex()
                .items_center()
                .justify_center()
                .gap(px(10.))
                .child(link("lock-restore", root.tr("lock.restore")).on_click(cx.listener(|r, _, _, cx| run(r, "restoreBackup", json!({}), cx))))
                .when(!create, |d| {
                    d.child(div().text_size(px(T_M)).text_color(c(DIM)).child("·"))
                        .child(link("lock-new-vault", root.tr("vault.new")).on_click(cx.listener(|r, _, _, cx| run(r, "addVault", json!({}), cx))))
                })
                .into_any_element(),
        );
    }
    // For someone who signs in to Roblox only in their own browser: Multi-Roblox without a vault or accounts.
    if create && !adding {
        let on = flag(&root.state, "multiAtStart");
        let line = div().mt(px(16.)).flex().flex_col().items_center().gap(px(8.));
        lines.push(if on {
            line.child(div().text_size(px(T_M)).line_height(px(17.)).text_color(c(TEXT_2)).text_center().child(root.tr("lock.multiOnlyOn")))
                .child(link("lock-multi-off", root.tr("lock.multiOnlyOff")).on_click(cx.listener(|r, _, _, cx| run(r, "multiOnly", json!({ "on": false }), cx))))
                .when(flag(&root.state, "closeToTray"), |d| {
                    d.child(Button::new("lock-close", root.tr("lock.closeWin")).on_click(cx.listener(|r, _, _, _| r.send("window", json!({ "action": "close" })))))
                })
                .into_any_element()
        } else {
            line.child(link("lock-multi", root.tr("lock.multiOnly")).on_click(cx.listener(|r, _, window, cx| {
                r.confirm(r.tr("set.multiConfirmTitle"), r.tr("set.multiConfirm"), r.tr("set.multiOk"), false, None, window, cx, |r, cx| {
                    let Some(link) = r.link.clone() else { return };
                    let rx = link.call("multiOnly", json!({ "on": true }));
                    cx.spawn(async move |this, cx| {
                        let answer = rx.await;
                        let _ = this.update(cx, |r, cx| {
                            r.toast = Some(match answer {
                                Ok(Ok(_)) => (r.tr("lock.multiOnlyDone"), "ok".into()),
                                Ok(Err(why)) => (why.to_string(), "bad".into()),
                                Err(_) => return,
                            });
                            cx.notify();
                        });
                    })
                    .detach();
                });
            })))
            .into_any_element()
        });
    }

    // The logo pops out, and the lines under it come in one after another (the web page's lock-intro).
    let mut box_ = card().child(pop_out("lock-logo", 72., |s| img("logo.svg").size(px(s)).into_any_element()));
    for (i, line) in lines.into_iter().enumerate() {
        box_ = box_.child(line_in("lock-line", i.min(6), div().w_full().flex().flex_col().items_center().child(line)));
    }
    layer.child(div().size_full().flex().items_center().justify_center().child(box_)).child(signature(root)).into_any_element()
}

/// An update found before the unlock, installing: its bar, and the way to unlock now instead (Lock.tsx's lock-update).
fn update(root: &Root, window: &Window, cx: &mut Context<Root>) -> Div {
    let u = root.state.get("update").unwrap_or(&NONE);
    let ready = u.get("status").and_then(Value::as_str) == Some("ready");
    let percent = if ready { 100. } else { u.get("progress").and_then(Value::as_f64).unwrap_or(0.).round().clamp(0., 100.) };
    let version = u.get("version").and_then(Value::as_str).unwrap_or_default();
    // A quiet fix shows no number: it is the version it fixes, as the app shows it.
    let title = if flag(u, "quiet") || version.is_empty() { root.tr("lock.upd.title") } else { root.i18n.tv("lock.upd.titleV", &[("v", version)]) };
    card()
        .child(img(crate::crisp::logo(72., window)).size(px(72.)))
        .child(div().mt(px(22.)).text_size(px(T_STAGE)).line_height(px(32.)).font_weight(FontWeight::BOLD).text_center().child(title))
        .child(div().mt(px(8.)).mb(px(26.)).text_size(px(T_L)).line_height(px(21.)).text_color(c(TEXT_2)).text_center().child(root.tr("lock.upd.note")))
        .child(
            div()
                .w_full()
                .p(px(16.))
                .rounded(px(R_L))
                .bg(c(INSET))
                .border_1()
                .border_color(c(LINE))
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .text_size(px(T_M))
                        .child(if ready { spinner(14., c(FACE)).into_any_element() } else { icon("download", 15., c(MUTED)).into_any_element() })
                        .child(div().flex_1().min_w_0().truncate().text_color(c(TEXT_2)).font_features(tnum()).child(if ready { root.tr("lock.upd.install") } else { root.i18n.tv("lock.upd.get", &[("p", &(percent as i64).to_string())]) })),
                )
                .child(div().w_full().h(px(6.)).rounded_full().bg(c(RAISED_2)).child(div().h_full().rounded_full().bg(c(FACE)).with_spring("lock-upd", SpringAnimation::new(EASE).to(percent as f32 / 100.), |d, w| d.w(relative(w.clamp(0., 1.)))))),
        )
        .when(!ready, |d| {
            d.child(div().mt(px(20.)).child(Button::new("lock-update-later", root.tr("lock.upd.later")).quiet().tip(root.tr("lock.upd.laterTip")).on_click(cx.listener(|r, _, _, cx| {
                r.lock.updating = Some(false);
                r.lock.focused = false;
                r.send("updateAtLock", json!({ "go": false }));
                cx.notify();
            }))))
        })
}

/// "Choose a vault": a tile each (the one used last marked), + to add one, x to take one off this PC, and whether to
/// ask each time Bloxkeep starts (Vaults.tsx's VaultPicker).
fn picker(root: &Root, window: &Window, cx: &mut Context<Root>) -> Div {
    let v = vaults(root);
    let current = v.get("current").and_then(Value::as_str).unwrap_or_default();
    let open = v.get("open").and_then(Value::as_str).unwrap_or_default();
    let list: Vec<Value> = v.get("list").and_then(Value::as_array).cloned().unwrap_or_default();
    let name_under = |name: String| div().w(px(132.)).truncate().text_center().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT_2)).child(name);
    let mut tiles = div().flex().flex_wrap().justify_center().gap(px(22.));
    for (i, one) in list.iter().enumerate() {
        let id = one.get("id").and_then(Value::as_str).unwrap_or_default().to_string();
        let name = one.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
        let last = id == current;
        let pick = id.clone();
        // A picture of the app's own scenes (not a file of the user's) is drawn as the theme tiles draw it.
        let picture = one.get("picture").and_then(Value::as_str);
        let px_size = (128. * window.scale_factor()).round() as u32;
        let scene_pic = picture.filter(|p| p.starts_with("scene-") && crate::images::user(p).is_none()).and_then(|p| crate::scene::thumb(p, c(RAISED_2), None, px_size, px_size, cx));
        let tile = hot(("vp-tile", i))
            .size(px(132.))
            .rounded(px(R_L))
            .overflow_hidden()
            .border_2()
            .tones(c(PANEL), c(PANEL))
            .edge(if last { c(FACE) } else { ca(WHITE, 0.14) }, if last { c(FACE_HI) } else { c(TEXT) })
            .lift(2.)
            .cursor_pointer()
            .child(match scene_pic {
                Some(p) => div().size(px(128.)).bg(c(RAISED_2)).child(img(ImageSource::Render(p)).size_full().object_fit(ObjectFit::Cover)),
                None => face(&id, &name, picture, 128., 0., 56.),
            })
            .on_click(cx.listener(move |r, _, _, cx| run(r, "selectVault", json!({ "id": pick }), cx)));
        // Not the vault open behind the picker: switch away from it first.
        let remove = (id != open).then(|| {
            let (id, name) = (id.clone(), name.clone());
            div().absolute().top(px(-10.)).right(px(-10.)).opacity(0.85).group_hover("vp-item", |s| s.opacity(1.)).child(
                IconButton::new(("vp-remove", i), "x").size(30.).filled().round().tip(root.tr("vault.remove")).on_click(cx.listener(move |r, _, window, cx| {
                    let id = id.clone();
                    r.confirm(r.i18n.tv("vault.removeTitle", &[("name", &name)]), r.tr("vault.removeText"), r.tr("vault.removeOk"), true, None, window, cx, move |r, cx| {
                        run(r, "removeVault", json!({ "id": id }), cx)
                    });
                })),
            )
        });
        tiles = tiles.child(div().group("vp-item").relative().w(px(132.)).flex().flex_col().items_center().gap(px(10.)).child(tile).children(remove).child(name_under(name)));
    }
    tiles = tiles.child(
        div().w(px(132.)).flex().flex_col().items_center().gap(px(10.)).child(
            hot("vp-add")
                .size(px(132.))
                .rounded(px(R_L))
                .border_2()
                .border_dashed()
                .tones(ca(WHITE, 0.), ca(WHITE, 0.04))
                .edge(ca(WHITE, 0.14), c(TEXT))
                .lift(2.)
                .cursor_pointer()
                .flex()
                .items_center()
                .justify_center()
                .child(icon("plus", 44., c(TEXT_2)))
                .on_click(cx.listener(|r, _, _, cx| run(r, "addVault", json!({}), cx))),
        )
        .child(name_under(root.tr("vault.add"))),
    );
    div()
        .max_w(px(920.))
        .p(px(24.))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(22.))
        .child(img(crate::crisp::logo(56., window)).size(px(56.)))
        .child(div().text_size(px(T_STAGE)).line_height(px(32.)).font_weight(FontWeight::BOLD).text_center().child(root.tr("vault.who")))
        .child(tiles)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .text_size(px(T_M))
                .text_color(c(TEXT_2))
                .child(root.tr("vault.ask"))
                .child(Switch::new("vault-ask", flag(v, "ask")).on_toggle(cx.listener(|r, on: &bool, _, cx| run(r, "setAskVault", json!({ "enabled": *on }), cx)))),
        )
}
