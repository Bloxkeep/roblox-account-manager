// Bloxkeep 1.1's look (layout C, 2026-11-09; surfaces and type reworked 2026-11-11 after the owner found the neutral
// greys flat and small next to the old page): blue-slate surfaces in clear layers (the ground darkest, a panel a step
// up, a card a step above that, each step one you can see), the old page's type scale, the face yellow on the main
// button of a place and on whatever is chosen.
use gpui_kit::{Hsla, Rgba, rgb};

pub const GROUND: u32 = 0x0c0e13;
pub const PANEL: u32 = 0x151922;
pub const RAISED: u32 = 0x1c212c;
pub const RAISED_2: u32 = 0x262c39;
pub const TEXT: u32 = 0xe9ecf3;
pub const TEXT_2: u32 = 0xb6bccc;
/// Meta text (times, "played 12 h"). As light as the old page's (#969cad there, neutral here): the first pick
/// (#86868d) left most of a screen's words dim.
pub const MUTED: u32 = 0x969cad;
pub const DIM: u32 = 0x4b5263;
pub const FACE: u32 = 0xf5c933;
pub const FACE_HI: u32 = 0xffd84d;
/// The base a yellow block stands on (the main button's, pressed down onto it).
pub const FACE_BASE: u32 = 0xa67d10;
pub const INK: u32 = 0x15181e;
pub const OK: u32 = 0x3ccf6e;
pub const WARN: u32 = 0xf2a93b;
/// Online on Roblox (its site or app), not playing: the dot on an account's face.
pub const ONLINE: u32 = 0x5b9cf0;
pub const BAD: u32 = 0xef5b4c;
pub const WHITE: u32 = 0xffffff;
/// The rail's own ground, a shade off the window's, and the hairline around panels and between parts.
pub const RAIL_BG: u32 = 0x0f1218;
pub const LINE: u32 = 0x2a303d;
/// A text box's ground: a shade below what it sits on, so it reads as a place to type.
pub const INSET: u32 = 0x0e1117;
/// The face yellow pressed.
pub const FACE_DOWN: u32 = 0xe0b82c;
/// A switch's knob while it is off (on, it is the ink on the yellow).
pub const KNOB: u32 = 0xc9c9cf;
/// Black, the same on every theme: what a block's base is darkened toward (kit::base_of), never a ground of its own.
pub const SHADE: u32 = 0x000000;

pub const FONT: &str = "Be Vietnam Pro";

/// The frame: the icon rail, the gap between panels, the accounts column, a panel's inner padding.
pub const RAIL: f32 = 68.;
/// The top bar's height (it is the window's title bar too).
pub const TOP: f32 = 52.;
pub const GAP: f32 = 12.;
pub const SIDE: f32 = 300.;
pub const PAD: f32 = 16.;
/// Below this width the accounts column folds away; a strip on top of the place then says who dropped.
pub const NARROW: f32 = 1080.;

/// Type sizes, px: the old page's scale (app.css --fs-2..5: 12 meta, 13 body, 14 a row's main words, 15 sections
/// and big buttons), then the figures and titles. The first scale here (11 / 12.5 / 14 / 22) read small and thin
/// next to the old page at the owner's window size.
pub const T_XS: f32 = 11.;
pub const T_S: f32 = 12.;
pub const T_M: f32 = 13.;
pub const T_L: f32 = 14.;
pub const T_H: f32 = 15.;
/// A section's title on a page ("Needs a look", "Not open"), and a group's own name over its tiles (a game's).
pub const T_SEC: f32 = 16.;
pub const T_GROUP: f32 = 18.;
pub const T_FIG: f32 = 22.;
pub const T_TITLE: f32 = 30.;
pub const T_STAGE: f32 = 28.;

/// Corners, px.
pub const R_S: f32 = 8.;
pub const R_M: f32 = 10.;
pub const R_L: f32 = 12.;
pub const R_PANEL: f32 = 16.;

// ───────────────────────── the theme in use ─────────────────────────
//
// The colours above are Dark's, Bloxkeep 1.1's own look. Every other theme is the web page's (app.css, resolved into
// theme_table.rs by tools/themes.mjs): `c(GROUND)` and the rest look up the theme in use, keyed by the constant's value
// (no two constants share one). A picture theme (a scene behind the window) has see-through panels over it; the
// picture itself is scene.rs's.

/// The slots a theme fills, in this order (WHITE is not one: `ca(WHITE, a)` below a half is the theme's tint).
const SLOTS: [u32; 21] = [GROUND, PANEL, RAISED, RAISED_2, TEXT, TEXT_2, MUTED, DIM, FACE, FACE_HI, FACE_BASE, INK, OK, WARN, ONLINE, BAD, RAIL_BG, LINE, INSET, FACE_DOWN, KNOB];

fn slot(hex: u32) -> Option<usize> {
    Some(match hex {
        GROUND => 0,
        PANEL => 1,
        RAISED => 2,
        RAISED_2 => 3,
        TEXT => 4,
        TEXT_2 => 5,
        MUTED => 6,
        DIM => 7,
        FACE => 8,
        FACE_HI => 9,
        FACE_BASE => 10,
        INK => 11,
        OK => 12,
        WARN => 13,
        ONLINE => 14,
        BAD => 15,
        RAIL_BG => 16,
        LINE => 17,
        INSET => 18,
        FACE_DOWN => 19,
        KNOB => 20,
        _ => return None,
    })
}

/// The web tokens theme_table.rs keeps, by place (tools/themes.mjs KEYS).
mod web {
    pub const BG: usize = 0;
    pub const BG_2: usize = 1;
    pub const SURFACE: usize = 2;
    pub const SURFACE_2: usize = 3;
    pub const SURFACE_3: usize = 4;
    pub const RAISED: usize = 5;
    pub const FIELD: usize = 6;
    pub const LINE_2: usize = 8;
    pub const TEXT: usize = 9;
    pub const TEXT_2: usize = 10;
    pub const MUTED: usize = 11;
    pub const DIM: usize = 12;
    pub const ACCENT: usize = 13;
    pub const ACCENT_HOVER: usize = 14;
    pub const ACCENT_INK: usize = 15;
    pub const OK: usize = 16;
    pub const WARN: usize = 17;
    pub const ONLINE: usize = 18;
    pub const BAD: usize = 19;
    pub const KNOB: usize = 20;
    pub const N: usize = 21;
}

/// What is behind the window on a picture theme: the file (a scene's name, or the custom theme's "user/…" picture; empty
/// for a video, which can't play here), the colour under it, a veil over it, how much it is blurred (logical px), the
/// theme's wash over all of it, and what the panels show of it (the web's backdrop-filter: blur in logical px and
/// saturate, 1 = as is; None: the picture as it is).
#[derive(Clone, Debug, PartialEq)]
pub struct Backdrop {
    pub file: String,
    pub under: Hsla,
    pub veil: Option<Hsla>,
    pub blur: f32,
    pub wash: Hsla,
    pub frost: Option<(f32, f32)>,
}

#[derive(Clone)]
pub struct Palette {
    slots: [Hsla; 21],
    tint: Hsla,
    pub dark: bool,
    pub backdrop: Option<Backdrop>,
}

impl Palette {
    /// Dark: the constants as they are.
    fn own() -> Palette {
        Palette { slots: SLOTS.map(|h| rgb(h).into()), tint: rgb(WHITE).into(), dark: true, backdrop: None }
    }

    /// A colour of this palette (for a theme's preview tile, drawn in its own colours).
    pub fn get(&self, hex: u32) -> Hsla {
        slot(hex).map(|i| self.slots[i]).unwrap_or_else(|| rgb(hex).into())
    }
}

thread_local! {
    static PAL: std::cell::RefCell<Palette> = std::cell::RefCell::new(Palette::own());
    /// The theme shown (its id, and the custom theme's look when it is that one): a state that says the same changes nothing.
    static SHOWN: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

pub fn c(hex: u32) -> Hsla {
    match slot(hex) {
        Some(i) => PAL.with(|p| p.borrow().slots[i]),
        None => rgb(hex).into(),
    }
}

/// A colour at `a`. White under a half is a faint ground or hairline: the theme's tint (dark on a light theme).
pub fn ca(hex: u32, a: f32) -> Hsla {
    if hex == WHITE && a < 0.5 {
        let mut t = PAL.with(|p| p.borrow().tint);
        t.a = a;
        return t;
    }
    let mut h = c(hex);
    h.a *= a;
    h
}

/// The theme in use is a light one.
pub fn light() -> bool {
    PAL.with(|p| !p.borrow().dark)
}

/// The picture behind the window, when the theme has one.
pub fn backdrop() -> Option<Backdrop> {
    PAL.with(|p| p.borrow().backdrop.clone())
}

fn hx((rgb_, a): (u32, f32)) -> Hsla {
    let mut h: Hsla = rgb(rgb_).into();
    h.a = a;
    h
}

/// `top` laid over `under` (straight alpha).
pub fn over(top: Hsla, under: Hsla) -> Hsla {
    let (t, u) = (top.to_rgb(), under.to_rgb());
    let a = t.a + u.a * (1. - t.a);
    if a <= 0. {
        return Hsla::transparent_black();
    }
    let ch = |x: f32, y: f32| (x * t.a + y * u.a * (1. - t.a)) / a;
    Rgba { r: ch(t.r, u.r), g: ch(t.g, u.g), b: ch(t.b, u.b), a }.into()
}

fn darker(h: Hsla, by: f32) -> Hsla {
    over(hx((0x000000, by)), h)
}

/// A theme's slots from its web tokens: see-through panels over a picture (frosted as the web page's were, scene.rs
/// draws that; at least 72 % thick where the web left the picture sharp), solid ones laid over the ground otherwise.
fn build(dark: bool, w: &[(u32, f32); web::N], tint: u32, picture: Option<(String, Hsla, Option<Hsla>, f32, Option<(f32, f32)>)>) -> Palette {
    let v = |i: usize| hx(w[i]);
    let mut s = Palette::own().slots;
    let (ground, panel, rail, raised, raised_2);
    let backdrop = match picture {
        Some((file, under, veil, blur, frost)) => {
            let under = Hsla { a: 1., ..under };
            let base = over(v(web::BG), over(veil.unwrap_or(Hsla::transparent_black()), under));
            let floor = if frost.is_some() { 0. } else { 0.72 };
            let thick = |h: Hsla| Hsla { a: h.a.max(floor), ..h };
            ground = base;
            panel = thick(v(web::BG_2));
            rail = thick(v(web::BG_2));
            raised = over(v(web::RAISED), base);
            raised_2 = over(v(web::SURFACE_3), raised);
            Some(Backdrop { file, under, veil, blur, wash: v(web::BG), frost })
        }
        None => {
            ground = Hsla { a: 1., ..v(web::BG) };
            panel = over(v(web::SURFACE), ground);
            rail = over(v(web::BG_2), ground);
            raised = over(v(web::SURFACE_2), panel);
            raised_2 = over(v(web::SURFACE_3), panel);
            None
        }
    };
    let face = v(web::ACCENT);
    for (hex, value) in [
        (GROUND, ground),
        (PANEL, panel),
        (RAISED, raised),
        (RAISED_2, raised_2),
        (RAIL_BG, rail),
        (TEXT, v(web::TEXT)),
        (TEXT_2, v(web::TEXT_2)),
        (MUTED, v(web::MUTED)),
        (DIM, v(web::DIM)),
        (FACE, face),
        (FACE_HI, v(web::ACCENT_HOVER)),
        (FACE_DOWN, darker(face, 0.085)),
        (FACE_BASE, darker(face, 0.35)),
        (INK, v(web::ACCENT_INK)),
        (OK, v(web::OK)),
        (WARN, v(web::WARN)),
        (ONLINE, v(web::ONLINE)),
        (BAD, v(web::BAD)),
        (LINE, v(web::LINE_2)),
        (INSET, v(web::FIELD)),
        (KNOB, v(web::KNOB)),
    ] {
        if let Some(i) = slot(hex) {
            s[i] = value;
        }
    }
    Palette { slots: s, tint: rgb(tint).into(), dark, backdrop }
}

/// A web theme by its id; Dark (and anything unknown) is Bloxkeep's own.
pub fn palette_of(id: &str) -> Palette {
    let Some(l) = crate::theme_table::LOOKS.iter().find(|l| l.id == id && id != "dark") else {
        return Palette::own();
    };
    let picture = l.scene.map(|f| (f.to_string(), hx(l.under), l.veil.map(hx), 0., l.frost));
    build(l.dark, &l.web, l.tint, picture)
}

/// The custom theme (Settings › Appearance › Custom) from its two colours, as shared.js's customVars works it out: dark or
/// light by the background, surfaces a step from it, words in its own hue, controls in the accent; with a picture the
/// page goes behind glass.
pub fn custom_palette(ct: &serde_json::Value) -> Palette {
    let hex_of = |k: &str, or: u32| {
        ct.get(k).and_then(|v| v.as_str()).and_then(|s| s.strip_prefix('#')).filter(|s| s.len() == 6).and_then(|s| u32::from_str_radix(s, 16).ok()).unwrap_or(or)
    };
    let (bg, ac) = (hex_of("background", 0x1b2a3a), hex_of("accent", 0xf59e0b));
    let ch = |h: u32| [((h >> 16) & 255) as f32, ((h >> 8) & 255) as f32, (h & 255) as f32];
    let pack = |c: [f32; 3]| c.iter().fold(0u32, |n, v| (n << 8) | v.round().clamp(0., 255.) as u32);
    let mix = |a: [f32; 3], b: [f32; 3], t: f32| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
    let lum = |c: [f32; 3]| {
        let l = |v: f32| {
            let s = v / 255.;
            if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * l(c[0]) + 0.7152 * l(c[1]) + 0.0722 * l(c[2])
    };
    const W: [f32; 3] = [255., 255., 255.];
    const K: [f32; 3] = [0., 0., 0.];
    let (b, a) = (ch(bg), ch(ac));
    let dark = lum(b) < 0.179;
    let words = if dark { W } else { K };
    let surface = if dark { mix(b, W, 0.05) } else { mix(b, W, 0.75) };
    let raised = if dark { mix(b, W, 0.07) } else { mix(b, W, 0.9) };
    let accent_hover = mix(a, if dark { W } else { K }, 0.14);
    let accent_ink = if lum(a) < 0.179 { 0xffffff } else { 0x0b0d12 };
    // What the custom theme doesn't set (the status colours, the knob, the hairlines) is Dark's or Light's (app.css's
    // [data-theme="custom"][data-tone="light"] shares Light's block).
    let rest = crate::theme_table::LOOKS.iter().find(|l| l.id == if dark { "dark" } else { "light" }).map(|l| l.web);
    let mut w = rest.unwrap_or([(0, 1.); web::N]);
    let o = |c: [f32; 3]| (pack(c), 1.);
    w[web::BG] = (bg, 1.);
    w[web::BG_2] = o(if dark { mix(b, W, 0.02) } else { mix(b, K, 0.03) });
    w[web::SURFACE] = o(surface);
    w[web::SURFACE_2] = o(if dark { mix(b, W, 0.08) } else { mix(b, W, 0.5) });
    w[web::SURFACE_3] = o(if dark { mix(b, W, 0.13) } else { mix(b, K, 0.07) });
    w[web::RAISED] = o(raised);
    w[web::FIELD] = o(if dark { mix(b, K, 0.3) } else { mix(b, W, 0.9) });
    w[web::TEXT] = o(mix(words, b, 0.08));
    w[web::TEXT_2] = o(mix(words, b, 0.28));
    w[web::MUTED] = o(mix(words, b, 0.4));
    w[web::DIM] = o(mix(words, b, 0.55));
    w[web::ACCENT] = (ac, 1.);
    w[web::ACCENT_HOVER] = o(accent_hover);
    w[web::ACCENT_INK] = (accent_ink, 1.);
    let video = ct.get("video").and_then(|v| v.as_bool()).unwrap_or(false);
    let num = |k: &str, or: f64| ct.get(k).and_then(|v| v.as_f64()).unwrap_or(or);
    let picture = ct.get("picture").and_then(|v| v.as_str()).filter(|p| !p.is_empty()).map(|file| {
        let fade = (num("fade", 45.).clamp(0., 90.) / 100.) as f32;
        w[web::BG] = (bg, fade);
        w[web::BG_2] = (bg, (fade + 0.25).min(0.95));
        w[web::SURFACE_3] = (pack(words), 0.15);
        w[web::RAISED] = (pack(raised), 0.94);
        w[web::FIELD] = if dark { (0x000000, 0.3) } else { (0xffffff, 0.6) };
        (if video { String::new() } else { file.to_string() }, hx((bg, 1.)), None, num("blur", 0.).clamp(0., 30.) as f32, if video { None } else { Some((18., 1.4)) })
    });
    // The custom glass is as see-through as the fade the user chose: its panels are the background at that fade + 25 %
    // (shared.js), frosted. A video can't play here (only its colour shows), so its panels keep at least half.
    let video_glass = picture.is_some() && video;
    let mut p = build(dark, &w, pack(words), picture);
    if video_glass {
        for hex in [PANEL, RAIL_BG] {
            if let Some(i) = slot(hex) {
                p.slots[i] = Hsla { a: w[web::BG_2].1.max(0.5), ..p.slots[i] };
            }
        }
    }
    p
}

/// Shows a theme: its colours on every slot, GPUI Component's parts dressed in them, every window drawn again.
pub fn show(id: &str, custom: Option<&serde_json::Value>, cx: &mut gpui_kit::App) {
    let key = format!("{id}|{}", if id == "custom" { custom.map(|c| c.to_string()).unwrap_or_default() } else { String::new() });
    if SHOWN.with(|s| *s.borrow() == key) {
        return;
    }
    SHOWN.with(|s| *s.borrow_mut() = key);
    let p = match (id, custom) {
        ("custom", Some(ct)) => custom_palette(ct),
        _ => palette_of(id),
    };
    PAL.with(|cur| *cur.borrow_mut() = p);
    dress(cx);
    cx.refresh_windows();
}

thread_local! {
    /// A theme shown before the app has said so (a tile clicked, a colour being dragged): until then, a state that
    /// still has the old one doesn't put it back.
    static HELD: std::cell::Cell<Option<std::time::Instant>> = const { std::cell::Cell::new(None) };
}

/// Shows a theme at once, ahead of the app's answer (Settings › Appearance).
pub fn preview(id: &str, custom: Option<&serde_json::Value>, cx: &mut gpui_kit::App) {
    HELD.with(|h| h.set(Some(std::time::Instant::now() + std::time::Duration::from_millis(1500))));
    show(id, custom, cx);
}

/// The theme the app's state says is in use (Follow Windows already worked out, as themeApplied).
pub fn follow(state: &serde_json::Value, cx: &mut gpui_kit::App) {
    if HELD.with(|h| h.get()).is_some_and(|t| std::time::Instant::now() < t) {
        return;
    }
    let id = state.get("themeApplied").or_else(|| state.get("theme")).and_then(|v| v.as_str()).unwrap_or("dark").to_string();
    show(&id, state.get("customTheme").filter(|c| c.is_object()), cx);
}

/// Figures that change while you look (times, percents) keep their width: tabular digits.
pub fn tnum() -> gpui_kit::FontFeatures {
    gpui_kit::FontFeatures(std::sync::Arc::new(vec![("tnum".into(), 1)]))
}

/// GPUI Component's own parts (its dialog, chart, carousel, text box) in the theme's colours: its surfaces, lines,
/// accent and font, dark or light as the theme is. Run again when the theme changes (`show`).
pub fn dress(cx: &mut gpui_kit::App) {
    use gpui_kit::component::{Theme, ThemeMode};
    use gpui_kit::px;
    Theme::change(if light() { ThemeMode::Light } else { ThemeMode::Dark }, None, cx);
    Theme::update(cx, |t| {
        t.font_family = FONT.into();
        t.radius = px(R_M);
        t.radius_lg = px(R_PANEL);
        let k = &mut t.colors;
        k.background = c(RAISED);
        k.foreground = c(TEXT);
        k.popover = c(RAISED_2);
        k.popover_foreground = c(TEXT);
        k.border = ca(WHITE, 0.1);
        k.muted = c(RAISED_2);
        k.muted_foreground = c(MUTED);
        k.primary = c(FACE);
        k.primary_hover = c(FACE_HI);
        k.primary_active = c(FACE_DOWN);
        k.primary_foreground = c(INK);
        k.ring = ca(FACE, 0.6);
        k.overlay = ca(0x000000, 0.6);
        k.danger = c(BAD);
    });
}

/// Words laid on a game's picture under the theme's own veil (Home's stage, a game's page): white on a dark theme,
/// the theme's words on a light one, whose veil is light.
pub fn on_picture(a: f32) -> Hsla {
    if light() { ca(TEXT, a) } else { ca(WHITE, a) }
}
