// Bloxkeep 1.1's look (layout C, 2026-11-09; surfaces and type reworked 2026-11-11 after the owner found the neutral
// greys flat and small next to the old page): blue-slate surfaces in clear layers (the ground darkest, a panel a step
// up, a card a step above that, each step one you can see), the old page's type scale, the face yellow on the main
// button of a place and on whatever is chosen.
// Colours reworked 2026-11-13 (owner, on the real app: "màu không sang"): the surfaces keep their clear steps but lose
// their blue (charcoal, not slate) and their hairlines go fainter; the words are ivory, not blue-white; the face is a
// soft gold, not the logo's full yellow (that stays the logo's own), kept for a place's one main action and a switch
// that is on: what is merely chosen (the rail's place, a filter, a segment) is a light pill, not a yellow one; and the
// state colours are a shade quieter.
// The same evening (owner, on that build: "trông app nhợt nhạt"): the soft gold, ivory words and quiet states together
// washed the page out. Kept: charcoal with no blue, the face only on a place's one main action, the chosen thing a
// neutral pill. Back to full strength: the ground deeper and the steps wider, the words near white, the face a full
// gold, the state colours vivid.
use gpui_kit::{Hsla, Rgba, rgb};

// Dark's grounds, a step lighter each: the window, a panel on it, a block on the panel, a block on that. They were
// near black (#08080a, #121317, #1b1c21, #27282f) and the page read as one dark mass with no blocks to tell apart
// (the owner, 2026-11-15: "tối mù, bí bách", beside a charcoal page whose cards and buttons stand out). Black stays
// a theme of its own for those who want it.
pub const GROUND: u32 = 0x111215;
pub const PANEL: u32 = 0x1a1b20;
pub const RAISED: u32 = 0x25262c;
pub const RAISED_2: u32 = 0x31323a;
pub const TEXT: u32 = 0xf7f6f2;
pub const TEXT_2: u32 = 0xc9c7c0;
/// Meta text (times, "played 12 h"). As light as the old page's (#969cad there, neutral here): the first pick
/// (#86868d) left most of a screen's words dim.
pub const MUTED: u32 = 0x9a9890;
pub const DIM: u32 = 0x6b6963;
pub const FACE: u32 = 0xf6c63a;
pub const FACE_HI: u32 = 0xffd75e;
/// The base a yellow block stands on (the main button's, pressed down onto it).
pub const FACE_BASE: u32 = 0x9a7a12;
pub const INK: u32 = 0x171200;
pub const OK: u32 = 0x3ddc84;
pub const WARN: u32 = 0xffab3d;
/// Online on Roblox (its site or app), not playing: the dot on an account's face.
pub const ONLINE: u32 = 0x5aa2ff;
pub const BAD: u32 = 0xff6257;
pub const WHITE: u32 = 0xffffff;
/// The rail's own ground, a shade off the window's, and the hairline around panels and between parts.
pub const RAIL_BG: u32 = 0x15161a;
pub const LINE: u32 = 0x393a43;
/// A text box's ground: a shade below what it sits on, so it reads as a place to type.
pub const INSET: u32 = 0x131418;
/// The Roblox tab's view capsule: true black on every theme, as a phone's camera island is.
pub const ISLAND: u32 = 0x000000;
/// The face yellow pressed.
pub const FACE_DOWN: u32 = 0xdcae22;
/// A switch's knob while it is off (on, it is the ink on the yellow).
pub const KNOB: u32 = 0xd4d2cc;
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
/// for a video, which plays in its place: `video`), the colour under it, a veil over it, how much it is blurred (logical px), the
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
    /// A video instead of a picture (the custom theme's "user/…" .mp4 or .webm): played behind the page (scene.rs).
    pub video: Option<String>,
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
        #[allow(unused_mut)]
        let mut slots: [Hsla; 21] = SLOTS.map(|h| rgb(h).into());
        // BLOXKEEP_SHOT_TONES="ground=0c0d10,panel=121317,…": a hidden shot of Dark with other grounds and words, to
        // lay candidates side by side before any is taken.
        #[cfg(feature = "shot")]
        if let Ok(list) = std::env::var("BLOXKEEP_SHOT_TONES") {
            for part in list.split(',') {
                let Some((k, v)) = part.split_once('=') else { continue };
                let i = match k.trim() {
                    "ground" => 0,
                    "panel" => 1,
                    "raised" => 2,
                    "raised2" => 3,
                    "text" => 4,
                    "text2" => 5,
                    "muted" => 6,
                    "dim" => 7,
                    "rail" => 16,
                    "line" => 17,
                    "inset" => 18,
                    _ => continue,
                };
                if let Ok(h) = u32::from_str_radix(v.trim(), 16) {
                    slots[i] = rgb(h).into();
                }
            }
        }
        Palette { slots, tint: rgb(WHITE).into(), dark: true, backdrop: None }
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

/// How light a colour looks (CIE L*, 0 to 100).
pub fn lightness(h: Hsla) -> f32 {
    let c = h.to_rgb();
    let lin = |v: f32| if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    let y = 0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b);
    if y > 0.008856 { 116. * y.cbrt() - 16. } else { 903.3 * y }
}

/// `block` at least `step` L* away from what it stands on: left as it is when it already is, else made lighter
/// (`lighter`) or darker a little at a time, keeping its hue. (The other way when it runs out of room.)
fn apart(block: Hsla, on: Hsla, step: f32, lighter: bool) -> Hsla {
    let base = lightness(Hsla { a: 1., ..on });
    let far = |h: Hsla| (lightness(Hsla { a: 1., ..h }) - base).abs() >= step;
    if far(block) {
        return block;
    }
    for way in [lighter, !lighter] {
        let mut h = block;
        for _ in 0..24 {
            h.l = (h.l + if way { 0.006 } else { -0.006 }).clamp(0., 1.);
            if far(h) {
                return h;
            }
        }
    }
    block
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
            Some(Backdrop { file, under, veil, blur, wash: v(web::BG), frost, video: None })
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
    // Blocks must read as blocks on every theme: a third of the web page's themes had a block within 2.5 L* of the
    // panel it stands on, and a fifth a panel within 2 of the window's ground (measured over all 98, 2026-11-15), so
    // a card or a button melted into its page. Each is moved just far enough from what is under it, the way it
    // already leans (lighter on a dark theme, darker on a light one).
    let under_panel = over(panel, ground);
    let raised = apart(raised, under_panel, 3., dark);
    let raised_2 = apart(raised_2, raised, 2.5, dark);
    let ground = if backdrop.is_none() { apart(ground, panel, 2., false) } else { ground };
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

/// Bloxkeep's own picture theme, not one of the web page's (so not in the generated table): the keep at dusk. A
/// rendered scene (three.js, assets/photo-keep.jpg): you stand on the walk between the keep's two walls, built of
/// rounded blocks, lanterns along them; the walk runs to the gatehouse on the horizon; the logo's keeper rises behind
/// it; sky lanterns drift up under the stars. The page lies thin over it, as launchers lay theirs over their own
/// banner: the cards are the plates.
pub const KEEP: crate::theme_table::Look = crate::theme_table::Look {
    id: "keep",
    dark: true,
    scene: Some("photo-keep.jpg"),
    under: (0x0b1030, 1.),
    veil: None,
    frost: None,
    tint: 0xdfe4ff,
    web: [
        (0x070a14, 0.),
        (0x070a14, 0.5),
        (0x0c1230, 0.9),
        (0xffffff, 0.08),
        (0xffffff, 0.15),
        (0x0b1024, 0.95),
        (0x000000, 0.3),
        (0x0a0f22, 0.9),
        (0xdfe4ff, 0.16),
        (0xf1f4fa, 1.),
        (0xd0d6ea, 1.),
        (0xb4bccf, 1.),
        (0x7c86a3, 1.),
        (0xe2c06a, 1.),
        (0xecd089, 1.),
        (0x17140c, 1.),
        (0x34d399, 1.),
        (0xfbbf24, 1.),
        (0x8fb4ff, 1.),
        (0xfb7185, 1.),
        (0xcbd2e1, 1.),
    ],
};
/// How thick the page and the rail lie over the keep's picture (thinner than the 72 % of the web's sharp pictures:
/// its sky is dark where the words are, and the cards carry the rest; thick enough that Settings' rows, which stand
/// on the page itself, read over the lanterns low in the picture).
const KEEP_PAGE: f32 = 0.64;
const KEEP_RAIL: f32 = 0.84;
/// How soft the keep's picture is drawn (logical px): a little, so it sits behind the page instead of beside it.
const KEEP_BLUR: f32 = 1.;

/// A picture's file, built into the program: Bloxkeep's own, else one of the web page's.
pub fn scene_bytes(name: &str) -> Option<&'static [u8]> {
    match name {
        "photo-keep.jpg" => Some(include_bytes!("../assets/photo-keep.jpg")),
        _ => crate::theme_table::scene_bytes(name),
    }
}

/// A web theme by its id; Dark (and anything unknown) is Bloxkeep's own.
pub fn palette_of(id: &str) -> Palette {
    if id == KEEP.id {
        let picture = KEEP.scene.map(|f| (f.to_string(), hx(KEEP.under), None, KEEP_BLUR, None));
        let mut p = build(true, &KEEP.web, KEEP.tint, picture);
        for (hex, a) in [(PANEL, KEEP_PAGE), (RAIL_BG, KEEP_RAIL)] {
            if let Some(i) = slot(hex) {
                p.slots[i] = Hsla { a, ..p.slots[i] };
            }
        }
        return p;
    }
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
    if video_glass && let (Some(b), Some(name)) = (p.backdrop.as_mut(), ct.get("picture").and_then(|v| v.as_str())) {
        b.video = Some(name.to_string());
    }
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

thread_local! {
    /// A theme being looked at before it is picked (the mouse resting on its tile), until then: the app's state doesn't
    /// put the chosen one back meanwhile. It runs out by itself, so a tile left without its "left" can't hold it.
    static PEEK: std::cell::Cell<Option<std::time::Instant>> = const { std::cell::Cell::new(None) };
}

/// Shows a theme while the mouse rests on its tile, without choosing it (as VS Code shows one while its list is walked).
pub fn peek(id: &str, cx: &mut gpui_kit::App) {
    PEEK.with(|p| p.set(Some(std::time::Instant::now() + std::time::Duration::from_secs(20))));
    show(id, None, cx);
}

/// The look is over: true when one was on (the chosen theme is then shown again by the caller).
pub fn unpeek() -> bool {
    PEEK.with(|p| p.replace(None)).is_some()
}

/// Shows a theme at once, ahead of the app's answer (Settings › Appearance).
pub fn preview(id: &str, custom: Option<&serde_json::Value>, cx: &mut gpui_kit::App) {
    HELD.with(|h| h.set(Some(std::time::Instant::now() + std::time::Duration::from_millis(1500))));
    show(id, custom, cx);
}

/// How far two colours stand apart (WCAG contrast, 1 to 21), see-through ones taken as solid.
pub fn contrast(a: Hsla, b: Hsla) -> f32 {
    let y = |h: Hsla| {
        let c = Hsla { a: 1., ..h }.to_rgb();
        let lin = |v: f32| if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
        0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
    };
    let (la, lb) = (y(a), y(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// What is written or drawn on a chosen pill, whose ground is the theme's text colour: the page's own ground, solid;
/// white or black instead on the rare theme where that wouldn't read.
pub fn on_text() -> Hsla {
    let text = c(TEXT);
    let page = Hsla { a: 1., ..over(c(PANEL), Hsla { a: 1., ..c(GROUND) }) };
    if contrast(page, text) >= 4.5 {
        return page;
    }
    let (white, black): (Hsla, Hsla) = (rgb(0xffffff).into(), rgb(0x000000).into());
    if contrast(white, text) >= contrast(black, text) { white } else { black }
}

/// The ground the top bar and the status bar stand on when the window's own can't carry their words (Retro's teal
/// under grey words): the rail's, which every theme draws its icons on. None when the window's ground does.
pub fn bars_ground() -> Option<Hsla> {
    let ground = Hsla { a: 1., ..c(GROUND) };
    if backdrop().is_some() || contrast(c(TEXT_2), ground) >= 4.5 {
        return None;
    }
    Some(Hsla { a: 1., ..over(c(RAIL_BG), ground) })
}

/// The status bar's own ground: as `bars_ground`, and on a picture theme the rail's see-through ground, as the foot of
/// a picture is its busiest part (Miami night's street lights ran under the CPU and RAM figures).
pub fn status_ground() -> Option<Hsla> {
    bars_ground().or_else(|| backdrop().map(|_| c(RAIL_BG)))
}

/// Lets the app's next state through at once (Sync with Windows: only the app knows which of light and dark it is, so
/// nothing is shown ahead; a theme clicked a moment before must not hold its answer back, which left that theme's
/// colours on while Sync with Windows was the one ticked).
pub fn release() {
    HELD.with(|h| h.set(None));
}

/// The theme the app's state says is in use (Follow Windows already worked out, as themeApplied).
pub fn follow(state: &serde_json::Value, cx: &mut gpui_kit::App) {
    if HELD.with(|h| h.get()).is_some_and(|t| std::time::Instant::now() < t) || PEEK.with(|p| p.get()).is_some_and(|t| std::time::Instant::now() < t) {
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
