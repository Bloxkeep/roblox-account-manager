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
use crate::val::flag;
use crate::ask::answer;

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
    /// This screen played the opening (below): its card stands as it is, with no second entrance.
    opened: std::cell::Cell<bool>,
}

// ───────────────────────── the opening ─────────────────────────
// Opening Bloxkeep, once a run, on the first password screen: as the web page did it (app.js lockIntro, app.css
// .lock-intro; the owner, 2026-11-08: "nảy ở giữa xong bung ra", and 2026-11-15: GPUI's "không giống bản C#", which
// only popped the logo out on the card). The logo stands alone in the middle of the window and bounces there for a
// second: a squash, a hop that grows it half again, a landing, a smaller hop. Then the card opens out around it from a
// logo-sized tile while the logo flies up to its place on the card, shrinking back; the dots come up, and the lines
// step in under the logo one after another. The same times and curves as the page's.

/// The bounce, then the rest.
const BOUNCE_MS: f32 = 1000.;
const OPENING_MS: f32 = BOUNCE_MS + 1500.;
/// The logo's size during the bounce's second half and until it flies (the page's BIG).
const BIG: f32 = 1.5;

static OPENING_OVER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static OPENING_FROM: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
thread_local! {
    /// The card's height as last laid out (the logo's way from the middle to its place depends on it).
    static CARD_H: std::cell::Cell<f32> = const { std::cell::Cell::new(0.) };
}

/// How far into the opening this draw is, in ms; None when it is over (or was never to play).
fn opening_ms(root: &Root, window: &Window) -> Option<f32> {
    use std::sync::atomic::Ordering::Relaxed;
    if OPENING_OVER.load(Relaxed) {
        return None;
    }
    // BLOXKEEP_SHOT_INTRO=1300: a hidden shot of the opening at that moment; without it a shot has none.
    #[cfg(feature = "shot")]
    {
        let at = std::env::var("BLOXKEEP_SHOT_INTRO").ok().and_then(|v| v.parse::<f32>().ok());
        if at.is_none() {
            OPENING_OVER.store(true, Relaxed);
        } else if CARD_H.with(|h| h.get()) == 0. {
            window.request_animation_frame();
        }
        return at;
    }
    #[cfg(not(feature = "shot"))]
    {
        let ms = OPENING_FROM.get_or_init(std::time::Instant::now).elapsed().as_secs_f32() * 1000.;
        if ms >= OPENING_MS {
            OPENING_OVER.store(true, Relaxed);
            root.lock.opened.set(true);
            return None;
        }
        window.request_animation_frame();
        Some(ms)
    }
}

/// The bouncing logo at `t` of its second: how wide and tall (1 = its own size) and how far off the ground.
fn bounce(t: f32) -> (f32, f32, f32) {
    // (when, wide, tall, lift, and the curve to the next: 0 the first press, 1 up, 2 down)
    const K: [(f32, f32, f32, f32, u8); 7] = [
        (0., 1., 1., 0., 0),
        (0.12, 1.12, 0.86, 0., 1),
        (0.4, BIG * 0.94, BIG * 1.06, 54., 2),
        (0.62, BIG * 1.16, BIG * 0.82, 0., 1),
        (0.78, BIG * 0.98, BIG * 1.03, 16., 2),
        (0.9, BIG * 1.05, BIG * 0.95, 0., 1),
        (1., BIG, BIG, 0., 1),
    ];
    let t = t.clamp(0., 1.);
    let i = K.iter().rposition(|k| k.0 <= t).unwrap_or(0).min(K.len() - 2);
    let (a, b) = (K[i], K[i + 1]);
    let x = ((t - a.0) / (b.0 - a.0)).clamp(0., 1.);
    let e = match a.4 {
        0 => bezier(0.3, 0., 0.6, 1., x),
        1 => bezier(0.2, 0.7, 0.35, 1., x),
        _ => bezier(0.6, 0., 0.85, 0.45, x),
    };
    let mix = |p: f32, q: f32| p + (q - p) * e;
    (mix(a.1, b.1), mix(a.2, b.2), mix(a.3, b.3))
}

/// The lock screen at `ms` of the opening: `lines` are the card's lines under the logo.
fn opening(layer: Div, lines: Vec<AnyElement>, sign: Div, ms: f32, window: &Window) -> AnyElement {
    const W: f32 = 456.;
    const TILE: f32 = 112.;
    let h = CARD_H.with(|c| c.get());
    let after = ms - BOUNCE_MS;
    // The card: out from a logo-sized tile round the middle (.7 s on the soft spring, never past its size).
    let open = spring_soft((after / 700.).clamp(0., 1.)).clamp(0., 1.);
    // The logo: its way up to its place and back to its size (.8 s on the spring).
    let fly = if after <= 0. { 0. } else { spring((after / 800.).clamp(0., 1.)) };
    let (wide, tall, lift) = if after < 0. { bounce(ms / BOUNCE_MS) } else { (BIG + (1. - BIG) * fly, BIG + (1. - BIG) * fly, 0.) };
    // (Its slot's middle is 76 px under the card's top; the card's middle is the window's.)
    let away = (h / 2. - 76.) * (1. - fly) - lift;
    let (lw, lh) = (72. * wide, 72. * tall);
    let logo = div().size(px(72.)).flex_none().relative().when(h > 0., |d| {
        d.child(img(crate::crisp::logo(72. * BIG * 1.2, window)).absolute().left(px((72. - lw) / 2.)).top(px((72. - lh) / 2. + away)).w(px(lw)).h(px(lh)).object_fit(ObjectFit::Fill))
    });
    let (cw, ch) = (TILE + (W - TILE) * open, TILE + (h - TILE).max(0.) * open);
    let chrome = (after >= 0. && h > 0.).then(|| {
        div()
            .absolute()
            .left(px((W - cw) / 2.))
            .top(px((h - ch) / 2.))
            .w(px(cw))
            .h(px(ch))
            .rounded(px(24.))
            .map(card_ground)
            .border_1()
            .border_color(ca(WHITE, 0.07))
            .shadow(menu_shadow(0.8 * open))
    });
    let mut card = div()
        .relative()
        .w(px(W))
        .pt(px(40.))
        .px(px(40.))
        .pb(px(32.))
        .flex()
        .flex_col()
        .items_center()
        .child(canvas(|b, _, _| CARD_H.with(|c| c.set(f32::from(b.size.height))), |_, _, _, _| {}).absolute().inset_0())
        .children(chrome)
        .child(logo);
    for (i, line) in lines.into_iter().enumerate() {
        // Each .6 s on the soft spring, the first .22 s after the card starts, the next every .05 s (six of them).
        let e = spring_soft(((after - 220. - 50. * i.min(5) as f32) / 600.).clamp(0., 1.));
        card = card.child(div().w_full().flex().flex_col().items_center().relative().top(px((1. - e) * 12.)).opacity(e.clamp(0., 1.)).child(line));
    }
    let late = ((after - 450.) / 500.).clamp(0., 1.);
    layer.child(middle(card)).child(sign.opacity(late)).into_any_element()
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

/// Words people here build passwords from that the estimator's English lists don't hold: Vietnamese favourites and
/// the app's and Roblox's own names.
const OWN_WORDS: &[&str] = &[
    "bloxkeep", "roblox", "blox", "robux", "matkhau", "anhyeuem", "emyeuanh", "yeuem", "yeuanh", "iloveyou", "vietnam", "saigon", "hanoi", "conmeo",
    "concho", "deptrai", "xinhgai", "minhanh", "hoang", "nguyen", "tran", "pham", "huynh", "phan", "trang", "linh", "huong", "tuan", "dung", "khanh",
];

/// A master password's strength: 0 to 4 (zxcvbn's score: how many guesses it would take, from common passwords,
/// dictionary words, keyboard runs, dates and repeats, not from which kinds of characters it has) and its word. Only a
/// guide: any 6+ characters do. The old count of character kinds called "Password1!" strong and a long phrase of plain
/// words weak (the owner, 2026-11-19: "detect mật khẩu mạnh hay yếu chính xác hơn").
pub fn strength(v: &str) -> (usize, &'static str) {
    if v.is_empty() {
        return (0, "weak");
    }
    // (The estimator is slow past a hundred characters and sure by then.)
    let head: String = v.chars().take(100).collect();
    let score = u8::from(zxcvbn::zxcvbn(&head, OWN_WORDS).score()) as usize;
    // (Strong only at the top: a 3 still fell to names and years, "minhhoang123", "trangxinhgai99".)
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

/// What the lock screen shows, in the middle of its room. Where the window is too low for it (the smallest, 500 px
/// high: the new vault's card is taller than its room), it starts at the top and the rest is a scroll away: laid in
/// the middle whatever its height, its top and its foot were cut with no way to reach them (the shots of every size,
/// 2026-11-18).
fn middle(el: impl IntoElement) -> Stateful<Div> {
    div().id("lock-middle").size_full().overflow_y_scroll().flex().flex_col().items_center().child(div().flex_none().my_auto().py(px(12.)).child(el))
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
        .relative()
        .map(card_ground)
        // (How tall it stands, for what is laid round it: `signature`.)
        .child(canvas(|b, _, _| CARD_H.with(|c| c.set(f32::from(b.size.height))), |_, _, _, _| {}).absolute().inset_0())
        .border_1()
        .border_color(ca(WHITE, 0.07))
        .shadow(menu_shadow(0.8))
}

/// The card's ground: the panel's fill, and on a theme that frosts its picture the frosted picture under it with what
/// the card's words need of the theme's ground (scene::frost_lazy). The fill alone, thin on a custom theme at a low
/// fade, let the sharp picture through behind the words (the owner, 2026-11-18, over a light wallpaper).
fn card_ground(card: Div) -> Div {
    if !crate::theme::frosted() {
        return card.bg(c(PANEL));
    }
    // (Its place is the caller's: the card as it opens is laid by hand, absolute, and a `relative` here put it in the
    // card's own flow instead, where it pushed the card's lines out and grew with the card it measures: the opening
    // showed a tall empty pane for its few seconds, alpha.22, the owner's screenshot 2026-11-18.)
    card.child(crate::scene::frost_lazy(Corners::all(px(23.)), |_| c(PANEL)))
}

/// Under the card, near the foot of the window: whose screen this is, quietly (Bloxkeep, what it is, its version).
fn signature(root: &Root, cx: &App) -> Div {
    let version = root.state.pointer("/update/current").and_then(Value::as_str).unwrap_or_default();
    let mut parts = vec![("Bloxkeep".to_string(), None), (root.tr("brand.sub"), None)];
    if !version.is_empty() {
        parts.push((format!("v{version}"), None));
    }
    // No room for it under the card (the smallest window, 500 px high): left out, it lay over the card's last line.
    let room = (root.window_h.get() - CARD_H.with(|c| c.get())) / 2.;
    if room < 64. {
        return div();
    }
    // (On a picture too light along its foot for these words: on the ground the language switch has, as a pill.)
    let line = facts(parts).text_size(px(T_M)).text_color(c(MUTED));
    let line = if crate::scene::foot_reads(cx) { line } else { div().h(px(30.)).px(px(14.)).flex().items_center().rounded_full().bg(c(RAISED)).child(line.text_color(c(TEXT_2))) };
    div().absolute().bottom(px(if crate::scene::foot_reads(cx) { 26. } else { 20. })).left_0().right_0().flex().justify_center().child(line)
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
    // The opening plays on the first password screen only (not over the vault picker, nor an update installing).
    let intro = if mode == Mode::Pick || root.lock.updating == Some(true) { None } else { opening_ms(root, window) };
    let layer = match intro {
        // (The dots come up .1 s after the card starts, over .6 s; the language switch with the last lines.)
        Some(ms) => div()
            .size_full()
            .relative()
            .child(div().absolute().top_0().left_0().size_full().opacity(((ms - BOUNCE_MS - 100.) / 600.).clamp(0., 1.)).child(dots()))
            .child(lang_switch(root, cx).opacity(((ms - BOUNCE_MS - 450.) / 500.).clamp(0., 1.))),
        None => div().size_full().relative().child(dots()).child(lang_switch(root, cx)),
    };
    let v = vaults(root);
    let current = v.get("current").and_then(Value::as_str).unwrap_or_default();
    if mode == Mode::Pick {
        return layer
            .when(!current.is_empty(), |d| d.child(back(root, "closeVaultPicker", cx)))
            .child(middle(picker(root, window, cx)))
            .into_any_element();
    }
    let adding = adding(root);
    let layer = layer.when(adding, |d| d.child(back(root, "cancelAddVault", cx)));
    if root.lock.updating == Some(true) {
        return layer.child(middle(update(root, window, cx))).child(signature(root, cx)).into_any_element();
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
    // Shots never show the PC's own Caps Lock.
    let caps = window.capslock().on && !(cfg!(feature = "shot") && std::env::var("BLOXKEEP_KEPT").is_ok());

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
        let part = if typed.is_empty() { 0. } else { (score as f32 + 1.) * 0.2 };
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
        let armed = root.state.pointer("/multi/armed").and_then(Value::as_bool).unwrap_or(false);
        lines.push(if on {
            line.when(armed, |d| d.child(div().text_size(px(T_M)).line_height(px(17.)).text_color(c(TEXT_2)).text_center().child(root.tr("lock.multiOnlyOn"))))
                // Roblox was open when it was turned on: it starts once every Roblox is closed.
                .when(!armed, |d| d.child(root.multi_blocked_note(cx).max_w(px(460.)).px(px(14.)).py(px(10.)).rounded(px(R_M)).bg(ca(WARN, 0.1))))
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
                    answer(rx, cx, move |r, answer, cx| {
                        r.toast = Some(match answer {
                            Ok(Ok(_)) => (r.tr("lock.multiOnlyDone"), "ok".into()),
                            Ok(Err(why)) => (why.to_string(), "bad".into()),
                            Err(_) => return,
                        });
                        cx.notify();
                    });
                });
            })))
            .into_any_element()
        });
    }

    if let Some(ms) = intro {
        return opening(layer, lines, signature(root, cx), ms, window);
    }
    // The screen that played the opening stands as it is (the page's .opened: no second entrance once it is over).
    // Any later one (after a lock): the logo pops out, and the lines under it come in one after another.
    let mut box_ = if root.lock.opened.get() {
        card().child(img(crate::crisp::logo(72., window)).size(px(72.)))
    } else {
        card().child(pop_out("lock-logo", 72., |s| img("logo.svg").size(px(s)).into_any_element()))
    };
    for (i, line) in lines.into_iter().enumerate() {
        let line = div().w_full().flex().flex_col().items_center().child(line);
        box_ = if root.lock.opened.get() { box_.child(line) } else { box_.child(line_in("lock-line", i.min(6), line)) };
    }
    layer.child(middle(box_)).child(signature(root, cx)).into_any_element()
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
