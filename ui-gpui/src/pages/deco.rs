// The profile's decorations (own profile, kept in Bloxkeep only): a theme playing over the banner (snow and a sleigh at
// Christmas, bats and fog at Halloween, fireworks and lanterns at Tết…) and a ring round the picture, picked in a small
// panel from the banner's buttons, or following the season by itself. The owner's approved mock-up (2026-11-19,
// "trang-tri.html"), drawn here with GPUI's own pieces: round boxes, lines, soft shadows for the glows, and the
// pictures (Fluent Emoji 3D, MIT; the sleigh's 12 frames, Lottie Simple License; assets/deco/NOTICE.txt).
//
// GPUI turns no picture, so each one that spins is made once in 24 turns (one picture of 24 frames, frame_index);
// nothing is made per frame. The pictures are let go 45 s after the last frame that drew any (`sweep`). It moves at
// 30 frames a second at most, only while the window is in front (kit::looping), else it stands still.
use std::{cell::RefCell, collections::HashMap, f32::consts::{PI, TAU}, sync::Arc, time::Instant};

use gpui_kit::{prelude::*, *};
use serde_json::json;

/// The themes: their name, and the ring a pick of the theme brings with it.
pub const THEMES: [(&str, &str); 14] = [
    ("none", "none"),
    ("xmas", "lights"),
    ("halloween", "bats"),
    ("tet", "mai"),
    ("valentine", "hearts"),
    ("space", "orbit"),
    ("sakura", "petals"),
    ("autumn", "leaves"),
    ("summer", "sun"),
    ("aurora", "aurora"),
    ("neon", "neon"),
    ("storm", "electric"),
    ("fireflies", "fireflies"),
    ("embers", "fire"),
];
pub const RINGS: [&str; 18] = [
    "none", "classic", "lights", "frost", "bats", "pumpkin", "mai", "hearts", "orbit", "petals", "leaves", "sun", "aurora", "neon", "electric", "fireflies", "fire", "glitch",
];

/// How strongly the profile moves with its song: off, or light to strong (kept per account; medium until set).
pub const PULSES: [&str; 4] = ["off", "light", "medium", "strong"];

pub fn pulse(user: i64) -> &'static str {
    #[cfg(feature = "shot")]
    if let Ok(v) = std::env::var("BLOXKEEP_SHOT_PULSE") {
        return known(&PULSES, Some(&v)).unwrap_or("medium");
    }
    known(&PULSES, crate::prefs::get(&format!("profilePulse:{user}")).as_ref().and_then(|v| v.as_str())).unwrap_or("medium")
}

pub fn keep_pulse(user: i64, v: &str) {
    crate::prefs::set(&format!("profilePulse:{user}"), json!(v));
}

/// The pulse's strength, 0 when off.
pub fn power(pulse: &str) -> f32 {
    match pulse {
        "light" => 0.45,
        "medium" => 0.75,
        "strong" => 1.,
        _ => 0.,
    }
}

/// What the account picked: the theme, the ring, and whether the season picks for it on its days.
#[derive(Clone, Copy, PartialEq)]
pub struct Pick {
    pub theme: &'static str,
    pub ring: &'static str,
    pub season: bool,
}

fn known(list: &[&'static str], v: Option<&str>) -> Option<&'static str> {
    v.and_then(|v| list.iter().copied().find(|t| *t == v))
}

pub fn pick(user: i64) -> Pick {
    #[cfg(feature = "shot")]
    if let Ok(v) = std::env::var("BLOXKEEP_SHOT_DECO") {
        let mut it = v.split('|');
        let theme = known(&THEMES.map(|t| t.0), it.next()).unwrap_or("none");
        let ring = known(&RINGS, it.next()).unwrap_or("none");
        return Pick { theme, ring, season: false };
    }
    let kept = crate::prefs::get(&format!("profileDeco:{user}"));
    // (Before the panel there was one ring, on or off: on is the classic one.)
    let old_ring = crate::prefs::get(&format!("profileRing:{user}")).and_then(|v| v.as_bool()).unwrap_or(false);
    let Some(kept) = kept else { return Pick { theme: "none", ring: if old_ring { "classic" } else { "none" }, season: false } };
    Pick {
        theme: known(&THEMES.map(|t| t.0), kept.get("theme").and_then(|v| v.as_str())).unwrap_or("none"),
        ring: known(&RINGS, kept.get("ring").and_then(|v| v.as_str())).unwrap_or("none"),
        season: kept.get("season").and_then(|v| v.as_bool()).unwrap_or(false),
    }
}

pub fn keep(user: i64, p: Pick) {
    crate::prefs::set(&format!("profileDeco:{user}"), json!({ "theme": p.theme, "ring": p.ring, "season": p.season }));
}

/// The theme and ring shown: the season's on its days when they follow it, else theirs.
pub fn shown(user: i64) -> (&'static str, &'static str) {
    let p = pick(user);
    if p.season {
        if let Some(t) = season_now() {
            let ring = THEMES.iter().find(|x| x.0 == t).map(|x| x.1).unwrap_or("none");
            return (t, ring);
        }
    }
    (p.theme, p.ring)
}

/// Tết's first day (the lunar new year), the years ahead.
const TET: [(i32, u32, u32); 8] = [(2026, 2, 17), (2027, 2, 6), (2028, 1, 26), (2029, 2, 13), (2030, 2, 3), (2031, 1, 23), (2032, 2, 11), (2033, 1, 31)];

/// The season's theme today, by the PC's own date: Christmas through December, Halloween from 20 October to 1 November,
/// Tết from ten days before its first day to a week after, Valentine from 10 to 14 February.
pub fn season_now() -> Option<&'static str> {
    let secs = crate::time::now_secs() + crate::time::utc_offset_secs();
    let days = secs.div_euclid(86_400);
    let (y, m, d) = civil(days);
    if let Some(&(_, tm, td)) = TET.iter().find(|t| t.0 == y as i32) {
        let first = days_from_civil(y, tm, td);
        if (first - 10..=first + 7).contains(&days) {
            return Some("tet");
        }
    }
    match (m, d) {
        (12, _) => Some("xmas"),
        (10, 20..) | (11, 1) => Some("halloween"),
        (2, 10..=14) => Some("valentine"),
        _ => None,
    }
}

/// Days since 1970-01-01 to a date (Howard Hinnant's).
fn civil(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (i64::from(m) + 9) % 12;
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

// ───────────────────────── Pictures ─────────────────────────

/// The pictures that turn: made in TURNS frames, each turned a step more.
const SPINS: [&str; 10] = ["snowflake", "wrapped_gift", "mai", "red_envelope", "cherry_blossom", "maple_leaf", "fallen_leaf", "star", "sun", "rocket"];
const TURNS: usize = 24;
/// A turning picture's side, and the part its drawing takes (room to turn without its corners cut).
const SPIN_SIDE: u32 = 96;
const SPIN_FILL: f32 = 0.70;
const STILL_SIDE: u32 = 128;
/// The sleigh's frames, and their size.
const SLEIGH_FRAMES: usize = 12;

struct Pics {
    map: HashMap<&'static str, Option<Arc<RenderImage>>>,
    last: Option<Instant>,
}

thread_local! {
    static PICS: RefCell<Pics> = RefCell::new(Pics { map: HashMap::new(), last: None });
    static START: Instant = Instant::now();
    static SURFS: RefCell<HashMap<String, Surf>> = RefCell::new(HashMap::new());
    static SEED: std::cell::Cell<u64> = const { std::cell::Cell::new(0x9E37_79B9_7F4A_7C15) };
}

/// Lets go of the pictures (from every window's atlas) and the moving things 45 s after anything was last drawn.
/// Called from the app's own timer (images.rs).
pub fn sweep(cx: &mut App) {
    let gone = PICS.with(|p| {
        let mut p = p.borrow_mut();
        if p.last.is_some_and(|t| t.elapsed().as_secs() >= 45) {
            p.last = None;
            p.map.drain().filter_map(|(_, v)| v).collect::<Vec<_>>()
        } else {
            Vec::new()
        }
    });
    if !gone.is_empty() {
        SURFS.with(|s| s.borrow_mut().clear());
        for pic in gone {
            cx.drop_image(pic, None);
        }
    }
}

fn pic(name: &'static str, cx: &App) -> Option<Arc<RenderImage>> {
    PICS.with(|p| {
        let mut p = p.borrow_mut();
        p.last = Some(Instant::now());
        // (One whose frames were wanted where they weren't is made again: images::made.)
        if p.map.get(name).is_some_and(|v| v.as_ref().is_some_and(crate::images::made_lost)) {
            p.map.remove(name);
        }
        p.map.entry(name).or_insert_with(|| make(name, cx).map(|r| crate::images::made("decoration", r))).clone()
    })
}

fn make(name: &str, cx: &App) -> Option<RenderImage> {
    let file = if name == "sleigh" { "deco/sleigh.webp".to_string() } else { format!("deco/{name}.png") };
    let bytes = cx.asset_source().load(&file).ok().flatten()?;
    let src = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let frames: Vec<image::RgbaImage> = if name == "sleigh" {
        let w = src.width() / SLEIGH_FRAMES as u32;
        let one: Vec<image::RgbaImage> = (0..SLEIGH_FRAMES).map(|i| image::imageops::crop_imm(&src, i as u32 * w, 0, w, src.height()).to_image()).collect();
        // (Then the same frames facing the other way.)
        let back: Vec<image::RgbaImage> = one.iter().map(image::imageops::flip_horizontal).collect();
        one.into_iter().chain(back).collect()
    } else if SPINS.contains(&name) {
        let side = (SPIN_SIDE as f32 * SPIN_FILL).round() as u32;
        let small = premul(image::imageops::resize(&src, side, side, image::imageops::FilterType::CatmullRom));
        (0..TURNS).map(|i| turned(&small, SPIN_SIDE, i as f32 * TAU / TURNS as f32)).collect()
    } else {
        let small = image::imageops::resize(&src, STILL_SIDE, STILL_SIDE, image::imageops::FilterType::CatmullRom);
        vec![premul(small)]
    };
    let frames = frames.into_iter().map(|f| image::Frame::new(bgra(f))).collect::<Vec<_>>();
    Some(RenderImage::new(frames))
}

fn premul(mut p: image::RgbaImage) -> image::RgbaImage {
    for px in p.pixels_mut() {
        let a = px[3] as u32;
        for c in &mut px.0[..3] {
            *c = ((*c as u32 * a + 127) / 255) as u8;
        }
    }
    p
}

/// From colours weighed by alpha back to plain ones, red and blue swapped (what GPUI's pictures are).
fn bgra(mut p: image::RgbaImage) -> image::RgbaImage {
    for px in p.pixels_mut() {
        let a = px[3];
        for c in &mut px.0[..3] {
            let v = (*c).min(a);
            *c = if a == 0 { 0 } else { ((v as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8 };
        }
        px.0.swap(0, 2);
    }
    p
}

/// `src` (alpha-weighed) turned by `angle` in the middle of a `side`-square, each pixel read between its four nearest.
fn turned(src: &image::RgbaImage, side: u32, angle: f32) -> image::RgbaImage {
    let mut out = image::RgbaImage::new(side, side);
    let (s, c) = angle.sin_cos();
    let (sw, sh) = (src.width() as f32, src.height() as f32);
    let half = side as f32 / 2.;
    for y in 0..side {
        for x in 0..side {
            let (dx, dy) = (x as f32 + 0.5 - half, y as f32 + 0.5 - half);
            // (Back from the turned picture to the plain one.)
            let u = c * dx + s * dy + sw / 2. - 0.5;
            let v = -s * dx + c * dy + sh / 2. - 0.5;
            if u < -1. || v < -1. || u > sw || v > sh {
                continue;
            }
            let (x0, y0) = (u.floor(), v.floor());
            let (fx, fy) = (u - x0, v - y0);
            let mut acc = [0f32; 4];
            for (ox, oy, wgt) in [(0., 0., (1. - fx) * (1. - fy)), (1., 0., fx * (1. - fy)), (0., 1., (1. - fx) * fy), (1., 1., fx * fy)] {
                let (px_, py_) = (x0 + ox, y0 + oy);
                if px_ < 0. || py_ < 0. || px_ >= sw || py_ >= sh {
                    continue;
                }
                let p = src.get_pixel(px_ as u32, py_ as u32);
                for i in 0..4 {
                    acc[i] += p[i] as f32 * wgt;
                }
            }
            out.put_pixel(x, y, image::Rgba(acc.map(|v| v.round().clamp(0., 255.) as u8)));
        }
    }
    out
}

// ───────────────────────── Drawing ─────────────────────────

pub(crate) fn col(hex: u32, a: f32) -> Hsla {
    let mut h: Hsla = rgb(hex).into();
    h.a = a.clamp(0., 1.);
    h
}

fn rnd(a: f32, b: f32) -> f32 {
    SEED.with(|s| {
        let mut x = s.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.set(x);
        a + (x >> 40) as f32 / (1u64 << 24) as f32 * (b - a)
    })
}

fn now() -> f32 {
    START.with(|s| s.elapsed().as_secs_f32())
}

/// Draws into a box: its top-left corner in the window, and the window.
pub(crate) struct Pen<'a> {
    pub(crate) w: &'a mut Window,
    pub(crate) cx: &'a mut App,
    pub(crate) o: Point<Pixels>,
}

impl Pen<'_> {
    pub(crate) fn at(&self, x: f32, y: f32) -> Point<Pixels> {
        point(self.o.x + px(x), self.o.y + px(y))
    }
    pub(crate) fn bx(&self, x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
        Bounds::new(self.at(x, y), size(px(w.max(0.)), px(h.max(0.))))
    }
    /// A round dot.
    pub(crate) fn dot(&mut self, x: f32, y: f32, r: f32, hex: u32, a: f32) {
        if a > 0.01 && r > 0. {
            let b = self.bx(x - r, y - r, r * 2., r * 2.);
            self.w.paint_quad(fill(b, col(hex, a)).corner_radii(px(r)));
        }
    }
    /// A box with round ends (a bulb, a petal), centred.
    pub(crate) fn pill(&mut self, x: f32, y: f32, w: f32, h: f32, hex: u32, a: f32) {
        if a > 0.01 {
            let b = self.bx(x - w / 2., y - h / 2., w, h);
            self.w.paint_quad(fill(b, col(hex, a)).corner_radii(px(w.min(h) / 2.)));
        }
    }
    pub(crate) fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, hex: u32, a: f32) {
        if a > 0.01 {
            let b = self.bx(x, y, w, h);
            self.w.paint_quad(fill(b, col(hex, a)));
        }
    }
    /// A circle's line.
    pub(crate) fn circle(&mut self, x: f32, y: f32, r: f32, width: f32, hex: u32, a: f32) {
        if a > 0.01 {
            let b = self.bx(x - r - width / 2., y - r - width / 2., r * 2. + width, r * 2. + width);
            self.w.paint_quad(quad(b, px(r + width / 2.), transparent_black(), px(width), col(hex, a), BorderStyle::Solid));
        }
    }
    /// A soft light round a spot (a blurred shadow of a small round box; nothing drawn of the box itself).
    pub(crate) fn glow(&mut self, x: f32, y: f32, r: f32, hex: u32, a: f32) {
        if a > 0.01 && r > 0. {
            let s = r * 0.5;
            let b = self.bx(x - s / 2., y - s / 2., s, s);
            let shadow = BoxShadow { color: col(hex, a), offset: point(px(0.), px(0.)), blur_radius: px(r * 0.9), spread_radius: px(0.), inset: false };
            self.w.paint_drop_shadows(b, Corners::all(px(s / 2.)), &[shadow]);
        }
    }
    /// A line through points.
    pub(crate) fn line(&mut self, pts: &[(f32, f32)], width: f32, hex: u32, a: f32) {
        if pts.len() < 2 || a <= 0.01 {
            return;
        }
        let mut p = PathBuilder::stroke(px(width));
        p.move_to(self.at(pts[0].0, pts[0].1));
        for &(x, y) in &pts[1..] {
            p.line_to(self.at(x, y));
        }
        if let Ok(path) = p.build() {
            self.w.paint_path(path, col(hex, a));
        }
    }
    /// A filled shape.
    pub(crate) fn shape(&mut self, pts: &[(f32, f32)], hex: u32, a: f32) {
        if pts.len() < 3 || a <= 0.01 {
            return;
        }
        let mut p = PathBuilder::fill();
        p.move_to(self.at(pts[0].0, pts[0].1));
        for &(x, y) in &pts[1..] {
            p.line_to(self.at(x, y));
        }
        p.close();
        if let Ok(path) = p.build() {
            self.w.paint_path(path, col(hex, a));
        }
    }
    /// A picture, centred on x, y, `s` across, turned by `rot` (the pictures that turn), seen at `a`, squashed to
    /// `sx`, `sy` of itself (a wing's beat, a leaf's turn).
    pub(crate) fn spr(&mut self, name: &'static str, x: f32, y: f32, s: f32, rot: f32, a: f32, sx: f32, sy: f32) {
        if a <= 0.01 || s <= 0. {
            return;
        }
        let Some(data) = pic(name, self.cx) else { return };
        let spins = SPINS.contains(&name);
        let (frame, full) = if spins {
            let step = TAU / TURNS as f32;
            (((rot.rem_euclid(TAU) / step).round() as usize) % TURNS, s / SPIN_FILL)
        } else {
            (0, s)
        };
        let (w, h) = (full * sx.abs().max(0.06), full * sy.abs().max(0.06));
        let b = self.bx(x - w / 2., y - h / 2., w, h);
        self.w.with_element_opacity(Some(a.min(1.)), |w| {
            let _ = w.paint_image(b, b, Corners::default(), data, frame, false);
        });
    }
    /// A box shaded from one colour to another (`angle` as CSS has it: 180 runs top to bottom).
    pub(crate) fn grad(&mut self, x: f32, y: f32, w: f32, h: f32, angle: f32, from: (u32, f32), to: (u32, f32)) {
        if from.1 <= 0.01 && to.1 <= 0.01 {
            return;
        }
        let b = self.bx(x, y, w, h);
        self.w.paint_quad(fill(b, linear_gradient(angle, linear_color_stop(col(from.0, from.1), 0.), linear_color_stop(col(to.0, to.1), 1.))));
    }
    /// A wavy ground across the box (a drift, a hill, the sea), from its top line down to `bottom`.
    pub(crate) fn hills(&mut self, w: f32, y: f32, amp: f32, waves: f32, phase: f32, bottom: f32, hex: u32, a: f32) {
        let mut pts: Vec<(f32, f32)> = (0..=48).map(|i| {
            let x = i as f32 / 48. * w;
            let u = x / w.max(1.) * TAU * waves + phase;
            (x, y + u.sin() * amp + (u * 2.3 + phase * 0.7).sin() * amp * 0.35)
        }).collect();
        pts.push((w, bottom));
        pts.push((0., bottom));
        self.shape(&pts, hex, a);
    }
    /// A soft beam from a point, `len` long at `angle`, `spread` wide (a radian or so at most).
    pub(crate) fn ray(&mut self, x: f32, y: f32, angle: f32, len: f32, spread: f32, hex: u32, a: f32) {
        let (a0, a1) = (angle - spread / 2., angle + spread / 2.);
        self.shape(&[(x, y), (x + a0.cos() * len, y + a0.sin() * len), (x + a1.cos() * len, y + a1.sin() * len)], hex, a);
    }
    /// A small four-pointed twinkle.
    pub(crate) fn twinkle(&mut self, x: f32, y: f32, r: f32, hex: u32, a: f32) {
        let q = r * 0.22;
        self.shape(&[(x, y - r), (x + q, y - q), (x + r, y), (x + q, y + q), (x, y + r), (x - q, y + q), (x - r, y), (x - q, y - q)], hex, a);
    }
}

/// A colour mixed toward another, `u` of the way.
fn mix(a: u32, b: u32, u: f32) -> u32 {
    let u = u.clamp(0., 1.);
    let ch = |s: u32| {
        let (x, y) = (((a >> s) & 0xff) as f32, ((b >> s) & 0xff) as f32);
        ((x + (y - x) * u).round() as u32) << s
    };
    ch(16) | ch(8) | ch(0)
}

/// A number from 0 to 1 that stays the same for the same seed (the lightning's path, a glitch's slices).
fn hash(n: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9E37_79B1) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xffff) as f32 / 65_535.
}

// ───────────────────────── The themes ─────────────────────────

#[derive(Clone, Copy, Default)]
struct Pt {
    x: f32,
    y: f32,
    v: f32,
    s: f32,
    r: f32,
    vr: f32,
    ph: f32,
    l: f32,
    vx: f32,
    vy: f32,
    n: u8,
    land: f32,
}

/// The sleigh's pass: which way, its curve, size and speed.
#[derive(Clone, Copy)]
struct Run {
    t0: f32,
    dir: f32,
    s: f32,
    sw: f32,
    sh: f32,
    ys: f32,
    ye: f32,
    c1: f32,
    c2: f32,
    bob: f32,
    ph: f32,
    d: f32,
    drop: f32,
    fr: f32,
}

/// A box's moving things.
#[derive(Default)]
struct Sim {
    a: Vec<Pt>,
    b: Vec<Pt>,
    c: Vec<Pt>,
    d: Vec<Pt>,
    next: f32,
    next2: f32,
    run: Option<Run>,
    last_run: Option<(f32, f32)>,
}

struct Surf {
    theme: &'static str,
    w: f32,
    h: f32,
    last: f32,
    sim: Sim,
}

fn many(n: usize, mut f: impl FnMut(usize) -> Pt) -> Vec<Pt> {
    (0..n).map(&mut f).collect()
}

fn init(theme: &str, w: f32, h: f32, k: f32, t: f32) -> Sim {
    let mut s = Sim::default();
    let area = w * h / 354_000.;
    match theme {
        "xmas" => {
            let n = ((110. * area).round() as usize).max(18);
            s.a = many(n, |_| Pt { x: rnd(0., w), y: rnd(0., h), r: rnd(0.7, 2.5) * k.max(0.6), v: rnd(16., 42.) * k, ph: rnd(0., TAU), s: rnd(0.5, 1.4), ..Default::default() });
            s.b = many((n / 14).max(2), |_| Pt { x: rnd(0., w), y: rnd(0., h), s: rnd(12., 20.) * k, v: rnd(20., 32.) * k, r: rnd(0., 6.), vr: rnd(-0.8, 0.8), ph: rnd(0., 6.), ..Default::default() });
            s.next = t + 0.6;
        }
        "halloween" => {
            s.a = many(7, |_| Pt { x: rnd(0., w), y: h - rnd(0., 40.) * k, r: rnd(140., 240.) * k, v: rnd(8., 22.) * k * if rnd(0., 1.) < 0.5 { -1. } else { 1. }, ..Default::default() });
            s.b = many(((5. * w / 1220.).round() as usize).max(2), |_| Pt { x: rnd(0., w), y: rnd(0.12, 0.55) * h, v: rnd(60., 110.) * k, s: rnd(28., 46.) * k, ph: rnd(0., TAU), ..Default::default() });
            s.c = many(2, |i| Pt { x: rnd(0.35, 0.8) * w, y: rnd(0.35, 0.6) * h, ph: i as f32 * 2., v: rnd(8., 14.) * k, ..Default::default() });
        }
        "tet" => {
            let n = ((24. * w / 1220.).round() as usize).max(6);
            s.a = many(n, |_| Pt { x: rnd(0., w), y: rnd(0., h), s: rnd(13., 24.) * k, v: rnd(18., 34.) * k, r: rnd(0., 6.), vr: rnd(-1.5, 1.5), ph: rnd(0., 6.), ..Default::default() });
            s.next = t + 0.3;
            s.next2 = t + 2.;
        }
        "valentine" => {
            let n = ((18. * w / 1220.).round() as usize).max(6);
            s.a = many(n, |_| Pt { x: rnd(0., w), y: rnd(0., h), s: rnd(14., 30.) * k, v: rnd(18., 34.) * k, ph: rnd(0., 6.), n: u8::from(rnd(0., 1.) < 0.3), ..Default::default() });
            s.b = many((30. * w / 1220.).round() as usize + 4, |_| Pt { x: rnd(0., w), y: rnd(0., h), ph: rnd(0., 6.), ..Default::default() });
        }
        "space" => {
            s.a = many((150. * area).round() as usize + 10, |_| Pt { x: rnd(0., w), y: rnd(0., h), r: rnd(0.5, 1.6) * k.max(0.6), ph: rnd(0., 6.), s: rnd(1., 3.), ..Default::default() });
            s.next = t + 0.5;
            s.next2 = t + 2.;
        }
        "sakura" => {
            let n = ((46. * area).round() as usize).max(8);
            s.a = many(n, |_| Pt { x: rnd(0., w), y: rnd(0., h), s: rnd(5., 9.) * k, v: rnd(18., 36.) * k, r: rnd(0., 6.), vr: rnd(-1.6, 1.6), ph: rnd(0., 6.), n: u8::from(rnd(0., 1.) < 0.2), ..Default::default() });
        }
        "autumn" => {
            let n = ((26. * w / 1220.).round() as usize).max(6);
            s.a = many(n, |_| Pt { x: rnd(0., w), y: rnd(0., h), s: rnd(18., 30.) * k, v: rnd(22., 40.) * k, r: rnd(0., 6.), vr: rnd(-1.4, 1.4), ph: rnd(0., 6.), n: u8::from(rnd(0., 1.) < 0.55), ..Default::default() });
        }
        "summer" => {
            s.a = many((26. * w / 1220.).round() as usize + 4, |_| Pt { x: rnd(0., w), y: rnd(0., h), r: rnd(3., 8.) * k, v: rnd(16., 34.) * k, ph: rnd(0., 6.), ..Default::default() });
            s.b = many(3, |i| Pt { x: rnd(0., w), y: h * (0.55 + i as f32 * 0.13), v: rnd(30., 55.) * k, s: rnd(30., 42.) * k, ..Default::default() });
        }
        "aurora" | "neon" => {
            s.a = many((90. * area).round() as usize + 8, |_| Pt { x: rnd(0., w), y: rnd(0., h * 0.55), r: rnd(0.5, 1.5) * k.max(0.6), ph: rnd(0., 6.), s: rnd(0.8, 2.5), ..Default::default() });
        }
        "storm" => {
            s.a = many(((150. * area).round() as usize).max(30), |_| Pt { x: rnd(-0.2 * w, w), y: rnd(-h, h), v: rnd(520., 760.) * k, l: rnd(10., 22.) * k, s: rnd(0.18, 0.4), ..Default::default() });
            s.b = many(6, |i| Pt { x: w * (i as f32 / 5.), y: rnd(-0.05, 0.12) * h, r: rnd(110., 190.) * k, v: rnd(6., 14.) * k, ..Default::default() });
            s.next = t + rnd(1.5, 3.);
        }
        "fireflies" => {
            s.a = many(((34. * area).round() as usize).max(10), |_| Pt { x: rnd(0., w), y: rnd(0.15, 0.9) * h, vx: rnd(-14., 14.) * k, vy: rnd(-8., 8.) * k, ph: rnd(0., TAU), s: rnd(0.6, 1.6), r: rnd(1.6, 3.) * k.max(0.6), ..Default::default() });
            s.b = many((w / (7. * k.max(0.4))).round() as usize, |i| Pt { x: i as f32 * 7. * k.max(0.4) + rnd(-3., 3.), l: rnd(12., 34.) * k, ph: rnd(0., TAU), ..Default::default() });
        }
        "embers" => {
            s.a = many(((110. * area).round() as usize).max(24), |_| Pt { x: rnd(0., w), y: rnd(0., h), v: rnd(24., 70.) * k, r: rnd(1.2, 3.2) * k.max(0.6), ph: rnd(0., TAU), s: rnd(0.6, 1.6), n: rnd(0., 2.99) as u8, ..Default::default() });
        }
        _ => {}
    }
    s
}

const BULBS: [u32; 5] = [0xff4d5e, 0xffd166, 0x3ddc84, 0x4db8ff, 0xff8ad8];

/// A string of bulbs along the top edge, sagging between hooks, chasing.
fn lights(p: &mut Pen, t: f32, w: f32, k: f32) {
    let span = 150. * k;
    let sag = 20. * k;
    let mut i = 0usize;
    let mut x0 = -20. * k;
    while x0 < w {
        let x1 = x0 + span;
        let wire: Vec<(f32, f32)> = (0..=12).map(|j| {
            let u = j as f32 / 12.;
            ((1. - u) * (1. - u) * x0 + 2. * u * (1. - u) * (x0 + x1) / 2. + u * u * x1, -2. + 2. * u * (1. - u) * (sag * 2.))
        }).collect();
        p.line(&wire, (2. * k).max(1.), 0x1d3b27, 1.);
        for j in 1..=4 {
            let u = j as f32 / 5.;
            let x = (1. - u) * (1. - u) * x0 + 2. * u * (1. - u) * (x0 + x1) / 2. + u * u * x1;
            let y = -2. + 2. * u * (1. - u) * (sag * 2.);
            let hex = BULBS[i % 5];
            let b = 0.35 + 0.65 * (t * 3.2 - i as f32 * 0.9).sin().max(0.);
            i += 1;
            p.rect(x - 2.2 * k, y, 4.4 * k, 4. * k, 0x3a3a3a, 1.);
            p.glow(x, y + 10. * k, 14. * k, hex, 0.8 * b);
            p.pill(x, y + 10. * k, 8.8 * k, 14. * k, hex, 0.45 + 0.55 * b);
        }
        x0 = x1;
    }
}

/// The top of `Pen::hills`' ground at `x`.
fn hill_y(x: f32, w: f32, y: f32, amp: f32, waves: f32, phase: f32) -> f32 {
    let u = x / w.max(1.) * TAU * waves + phase;
    y + u.sin() * amp + (u * 2.3 + phase * 0.7).sin() * amp * 0.35
}

/// A colour along a run of colours, `u` from 0 to 1 and round again.
fn cycle(u: f32, run: &[u32]) -> u32 {
    let n = run.len() as f32;
    let at = u.rem_euclid(1.) * n;
    let i = at.floor() as usize % run.len();
    mix(run[i], run[(i + 1) % run.len()], at.fract())
}

/// What lies under a theme's moving things: its sky, its light and its ground (shades, glows and shapes only). The
/// left foot of the banner is under the profile's picture, so what stands on the ground stands right of it.
fn back(theme: &str, p: &mut Pen, t: f32, w: f32, h: f32, k: f32) {
    match theme {
        "xmas" => {
            p.grad(0., 0., w, h * 0.75, 180., (0x0b1d45, 0.5), (0x0b1d45, 0.));
            // Warm lights out of focus, drifting.
            for i in 0..7 {
                let f = i as f32;
                let x = w * (0.08 + 0.14 * f) + (t * 0.13 + f * 1.7).sin() * 30. * k;
                let y = h * (0.4 + 0.22 * (t * 0.09 + f * 2.3).sin());
                p.glow(x, y, (44. + 16. * (f * 1.3).sin()) * k, BULBS[i % 5], 0.14 + 0.06 * (t * 0.8 + f).sin());
            }
            p.hills(w, h - 24. * k, 6. * k, 1.6, 0.4, h, 0xc9dcf2, 0.6);
            p.hills(w, h - 13. * k, 5. * k, 2.3, 2.1, h, 0xffffff, 0.94);
        }
        "halloween" => {
            p.grad(0., h * 0.25, w, h * 0.75, 180., (0x3a1460, 0.), (0x3a1460, 0.6));
            p.glow(w * 0.6, h + 20. * k, w * 0.35, 0xff7a1f, 0.22);
            // A graveyard on its hill: a bare tree and stones.
            let (gy, ga, gw, gp) = (h - 26. * k, 8. * k, 0.8, 1.1);
            p.hills(w, gy, ga, gw, gp, h, 0x140a1f, 0.96);
            let tx = w * 0.36;
            let ty = hill_y(tx, w, gy, ga, gw, gp) + 2. * k;
            let ink = 0x140a1f;
            p.line(&[(tx, ty), (tx + 4. * k, ty - 40. * k), (tx - 2. * k, ty - 72. * k)], 6. * k, ink, 0.96);
            p.line(&[(tx + 3. * k, ty - 44. * k), (tx + 26. * k, ty - 62. * k), (tx + 38. * k, ty - 60. * k)], 3. * k, ink, 0.96);
            p.line(&[(tx, ty - 58. * k), (tx - 22. * k, ty - 80. * k), (tx - 30. * k, ty - 78. * k)], 2.5 * k, ink, 0.96);
            p.line(&[(tx - 1. * k, ty - 70. * k), (tx + 10. * k, ty - 90. * k)], 2. * k, ink, 0.96);
            for (u, s) in [(0.45f32, 1.), (0.51, 0.75), (0.58, 1.1), (0.66, 0.85)] {
                let x = u * w;
                let y = hill_y(x, w, gy, ga, gw, gp) + 4. * k;
                p.rect(x - 9. * k * s, y - 22. * k * s, 18. * k * s, 22. * k * s, 0x1f1530, 1.);
                p.dot(x, y - 22. * k * s, 9. * k * s, 0x1f1530, 1.);
            }
        }
        "tet" => {
            p.grad(0., 0., w, h * 0.85, 180., (0x8a0f14, 0.5), (0x8a0f14, 0.));
            p.glow(w * 0.72, h + 10. * k, w * 0.32, 0xffb52e, 0.22);
        }
        "valentine" => {
            p.grad(0., h * 0.15, w, h * 0.85, 180., (0xff4d8d, 0.), (0xff4d8d, 0.32));
            for i in 0..8 {
                let f = i as f32;
                let y = h + 40. * k - ((t * 10. * k + f * 97.) % (h + 80. * k));
                p.glow(w * (0.08 + 0.12 * f) + (t * 0.3 + f).sin() * 20. * k, y, (34. + 12. * (f * 1.9).sin()) * k, if i % 2 == 0 { 0xff7aa8 } else { 0xffb3c8 }, 0.18);
            }
        }
        "space" => {
            p.grad(0., 0., w, h, 180., (0x070b24, 0.55), (0x070b24, 0.25));
            p.glow(w * 0.3 + (t * 0.05).sin() * 40. * k, h * 0.45, w * 0.26, 0x7b3fe4, 0.24);
            p.glow(w * 0.56, h * 0.72 + (t * 0.07).cos() * 20. * k, w * 0.2, 0x1fb6c9, 0.17);
            p.glow(w * 0.78, h * 0.3, w * 0.18, 0xd63ad6, 0.15);
        }
        "sakura" => {
            p.grad(0., 0., w, h, 160., (0xffc1d6, 0.3), (0xffc1d6, 0.));
            // A blossoming branch from the top right corner, swaying a little.
            let sway = |u: f32| (t * 0.8).sin() * 3. * k * u;
            let main = [(w + 10. * k, 6. * k), (w - 80. * k, 26. * k), (w - 170. * k, 22. * k), (w - 250. * k, 42. * k), (w - 310. * k, 38. * k)];
            let wood = 0x4a2c2a;
            for (i, pair) in main.windows(2).enumerate() {
                let u = i as f32 / 4.;
                p.line(&[(pair[0].0, pair[0].1 + sway(u)), (pair[1].0, pair[1].1 + sway(u + 0.25))], (7. - 1.4 * i as f32) * k, wood, 0.95);
            }
            for (from, to) in [(1usize, (-30f32, 40f32)), (2, (-20., -14.)), (3, (-26., 30.)), (2, (10., 34.))] {
                let (x, y) = main[from];
                let u = from as f32 / 4.;
                p.line(&[(x, y + sway(u)), (x + to.0 * k, y + to.1 * k + sway(u + 0.2))], 2.4 * k, wood, 0.95);
            }
            for i in 0..11 {
                let j = (i % 4) + 1;
                let (x, y) = main[j];
                let (dx, dy) = ((hash(i as u32) - 0.5) * 60. * k, (hash(i as u32 + 40) - 0.3) * 34. * k);
                p.spr("cherry_blossom", x + dx, y + dy + sway(j as f32 / 4. + 0.2), (17. + hash(i as u32 + 80) * 10.) * k, hash(i as u32 + 120) * TAU, 1., 1., 1.);
            }
        }
        "autumn" => {
            p.grad(0., 0., w, h, 180., (0xffb347, 0.1), (0xc4501b, 0.4));
            for i in 0..5 {
                let f = i as f32;
                p.ray(-20. * k, -30. * k, 0.42 + f * 0.13 + (t * 0.2 + f).sin() * 0.02, w * 0.95, 0.06, 0xffd58a, 0.06 + 0.03 * (t * 0.6 + f * 1.4).sin());
            }
            p.hills(w, h - 10. * k, 4. * k, 3., 0.3, h, 0x8a3a12, 0.85);
        }
        "summer" => {
            p.grad(0., 0., w, h * 0.65, 180., (0x3fb0ff, 0.32), (0x3fb0ff, 0.));
            for i in 0..12 {
                p.ray(w - 90. * k, 60. * k, t * 0.08 + i as f32 * TAU / 12., 280. * k, 0.1, 0xfff2a8, 0.1);
            }
            p.hills(w, h - 30. * k, 4. * k, 2., t * 0.8, h, 0x1e88e5, 0.45);
            p.hills(w, h - 18. * k, 3.5 * k, 3., 1. - t * 1.1, h, 0x4fc3f7, 0.45);
        }
        "aurora" => p.grad(0., 0., w, h, 180., (0x031225, 0.78), (0x0a2a3a, 0.4)),
        "neon" => p.grad(0., 0., w, h * 0.62, 180., (0x1a0533, 0.8), (0x6a1279, 0.6)),
        "storm" => p.grad(0., 0., w, h, 180., (0x0d1119, 0.72), (0x1b2333, 0.5)),
        "fireflies" => p.grad(0., 0., w, h, 180., (0x0a1530, 0.6), (0x0c2a1a, 0.7)),
        "embers" => {
            p.grad(0., 0., w, h, 180., (0x1a0a05, 0.35), (0xc2310f, 0.5));
            p.glow(w * 0.55, h + 30. * k, w * 0.45, 0xff7a1f, 0.32);
        }
        _ => {}
    }
}

fn draw(theme: &str, s: &mut Sim, p: &mut Pen, t: f32, dt: f32, w: f32, h: f32, k: f32) {
    back(theme, p, t, w, h, k);
    match theme {
        "xmas" => {
            lights(p, t, w, k);
            for f in &mut s.a {
                f.y += f.v * dt;
                f.x += (t * f.s + f.ph).sin() * 14. * k * dt;
                if f.y > h + 4. {
                    f.y = -4.;
                    f.x = rnd(0., w);
                }
                p.dot(f.x, f.y, f.r, 0xffffff, 0.5 + 0.4 * f.r / 2.5);
            }
            for f in &mut s.b {
                f.y += f.v * dt;
                f.r += f.vr * dt;
                f.x += (t + f.ph).sin() * 10. * k * dt;
                if f.y > h + 20. {
                    f.y = -20.;
                    f.x = rnd(0., w);
                }
                p.spr("snowflake", f.x, f.y, f.s, f.r, 0.85, 1., 1.);
            }
            // Each pass is new: which way, how high in and out, the swoop, how far (smaller, slower, a little faint)
            // and when; never the same way at about the same height twice running.
            if s.run.is_none() && t > s.next {
                let (mut dir, mut ys) = (1., 0.);
                for _ in 0..20 {
                    dir = if rnd(0., 1.) < 0.5 { -1. } else { 1. };
                    ys = rnd(-0.08, 0.5) * h;
                    match s.last_run {
                        Some((ld, ly)) if dir == ld && (ys - ly).abs() <= 0.22 * h => continue,
                        _ => break,
                    }
                }
                let sc = if rnd(0., 1.) < 0.3 { rnd(0.55, 0.75) } else { rnd(0.8, 1.12) };
                let sw = 300. * k * sc;
                let ye = (ys + rnd(-0.35, 0.35) * h).clamp(-0.08 * h, 0.5 * h);
                s.run = Some(Run { t0: t, dir, s: sc, sw, sh: sw * 297. / 600., ys, ye, c1: rnd(-0.15, 0.6) * h, c2: rnd(-0.15, 0.6) * h, bob: rnd(0.6, 1.8), ph: rnd(0., TAU), d: (w + sw * 2.) / (rnd(150., 250.) * k * sc), drop: 0., fr: rnd(13., 18.) });
                s.last_run = Some((dir, ys));
            }
            if let Some(mut r) = s.run {
                let q = ((t - r.t0) / r.d).clamp(0., 1.);
                let v = 1. - q;
                let (x0, x1) = if r.dir < 0. { (w + 20. * k, -r.sw - 20. * k) } else { (-r.sw - 20. * k, w + 20. * k) };
                let x = x0 + (x1 - x0) * q;
                let y = v * v * v * r.ys + 3. * v * v * q * r.c1 + 3. * v * q * q * r.c2 + q * q * q * r.ye + (q * TAU * r.bob + r.ph).sin() * 7. * k * r.s;
                let back = if r.dir < 0. { x + r.sw * 0.97 } else { x + r.sw * 0.03 };
                let body = if r.dir < 0. { x + r.sw * 0.78 } else { x + r.sw * 0.22 };
                for _ in 0..2 {
                    if rnd(0., 1.) < 0.8 {
                        s.c.push(Pt { x: back + rnd(-3., 3.) * k, y: y + r.sh * 0.62 + rnd(-5., 5.) * k, l: r.s, vx: -r.dir, ..Default::default() });
                    }
                }
                let fi = ((t - r.t0) * r.fr) as usize % SLEIGH_FRAMES + if r.dir > 0. { SLEIGH_FRAMES } else { 0 };
                if let Some(data) = pic("sleigh", p.cx) {
                    let b = p.bx(x, y, r.sw, r.sh);
                    p.w.with_element_opacity(Some(if r.s < 0.8 { 0.85 } else { 1. }), |w| {
                        let _ = w.paint_image(b, b, Corners::default(), data, fi, false);
                    });
                }
                if t - r.drop > rnd(0.6, 1.1) && body > 20. * k && body < w - 20. * k && y + r.sh < h * 0.9 {
                    r.drop = t;
                    s.d.push(Pt { x: body, y: y + r.sh * 0.6, vx: r.dir * 30. * k * r.s, vy: -14. * k, vr: rnd(-3., 3.), s: rnd(20., 28.) * k * r.s, ..Default::default() });
                }
                s.run = if (t - r.t0) / r.d > 1. {
                    s.next = t + rnd(3., 9.);
                    None
                } else {
                    Some(r)
                };
            }
            for sp in &mut s.c {
                sp.l -= dt * 1.3;
                sp.y += 8. * k * dt;
                sp.x += 10. * k * dt * sp.vx;
                p.rect(sp.x, sp.y, 2. * k, 2. * k, 0xffe28c, sp.l.max(0.));
            }
            s.c.retain(|sp| sp.l > 0.);
            for g in &mut s.d {
                if g.land == 0. {
                    g.vy += 280. * k * dt;
                    g.x += g.vx * dt;
                    g.y += g.vy * dt;
                    g.r += g.vr * dt;
                    if g.y > h - g.s / 2. - 2. {
                        g.y = h - g.s / 2. - 2.;
                        if g.vy.abs() < 60. * k {
                            g.land = t;
                            g.vr = 0.;
                        }
                        g.vy *= -0.32;
                        g.vx *= 0.5;
                        g.vr *= 0.4;
                    }
                }
                let a = if g.land > 0. { (1. - (t - g.land) / 1.4).max(0.) } else { 1. };
                p.spr("wrapped_gift", g.x, g.y, g.s, g.r, a, 1., 1.);
            }
            s.d.retain(|g| g.land == 0. || t - g.land < 1.4);
        }
        "halloween" => {
            p.glow(w * 0.42, 40. * k, 90. * k, 0xffd68c, 0.22);
            p.spr("crescent_moon", w * 0.42, 40. * k, 54. * k, 0., 1., 1., 1.);
            for f in &mut s.a {
                f.x += f.v * dt;
                if f.x < -f.r {
                    f.x = w + f.r;
                }
                if f.x > w + f.r {
                    f.x = -f.r;
                }
                p.glow(f.x, f.y, f.r, 0x966ed2, 0.26);
            }
            p.spr("spider_web", w - 58. * k, 50. * k, 118. * k, 0., 0.85, 1., 1.);
            let len = (64. + 22. * (t * 0.9).sin()) * k;
            p.line(&[(w - 92. * k, 0.), (w - 92. * k, len)], 1., 0xe6e6f0, 0.55);
            p.spr("spider", w - 92. * k, len + 12. * k, 32. * k, 0., 1., 1., 1.);
            for g in &mut s.c {
                g.x -= g.v * dt;
                if g.x < -40. * k {
                    g.x = w + 40. * k;
                }
                p.spr("ghost", g.x, g.y + (t * 1.1 + g.ph).sin() * 10. * k, 44. * k, 0., 0.55 + 0.3 * (t * 1.4 + g.ph).sin(), 1., 1.);
            }
            for b in &mut s.b {
                b.x -= b.v * dt;
                if b.x < -50. * k {
                    b.x = w + 50. * k;
                    b.y = rnd(0.12, 0.55) * h;
                }
                p.spr("bat", b.x, b.y + (t * 1.8 + b.ph).sin() * 16. * k, b.s, 0., 1., 1., 0.5 + 0.5 * (t * 9. + b.ph).sin().abs());
            }
            let fl = 0.75 + 0.25 * (t * 13.).sin() * (t * 7.3).sin();
            for (x, sz) in [(w - 230. * k, 46. * k), (w - 180. * k, 30. * k)] {
                p.glow(x, h - sz / 2. - 4. * k, sz * 1.3, 0xff8c28, 0.35 * fl);
                p.spr("jack_o_lantern", x, h - sz / 2. - 4. * k, sz, 0., 1., 1., 1.);
            }
        }
        "tet" => {
            if t > s.next {
                s.next = t + rnd(0.9, 1.6);
                let (x, y) = (rnd(0.3, 0.95) * w, rnd(0.12, 0.42) * h);
                let n = rnd(0., 3.99) as u8;
                // Two rings of sparks (the inner one another colour) and a flash where it bursts.
                for i in 0..56 {
                    let a = i as f32 / 56. * TAU + rnd(-0.03, 0.03);
                    let v = if i % 2 == 0 { rnd(95., 135.) } else { rnd(45., 75.) } * k;
                    s.b.push(Pt { x, y, vx: a.cos() * v, vy: a.sin() * v, l: 1., n: if i % 2 == 0 { n } else { n + 1 }, ..Default::default() });
                }
                s.d.push(Pt { x, y, l: 1., n, ..Default::default() });
            }
            const FIRE: [u32; 4] = [0xffd166, 0xff5a5f, 0xff9de2, 0xfff3b0];
            for f in &mut s.d {
                f.l -= dt * 3.;
                p.glow(f.x, f.y, 70. * k, FIRE[f.n as usize % 4], 0.45 * f.l.max(0.));
            }
            s.d.retain(|f| f.l > 0.);
            for f in &mut s.b {
                f.vy += 45. * k * dt;
                f.vx *= 0.985;
                f.x += f.vx * dt;
                f.y += f.vy * dt;
                f.l -= dt * 0.8;
                let hex = FIRE[f.n as usize % 4];
                let a = f.l.max(0.);
                // (A short tail behind each, and the last of them twinkle.)
                p.line(&[(f.x - f.vx * 0.06, f.y - f.vy * 0.06), (f.x, f.y)], 1.4 * k, hex, a * 0.6);
                let tw = if a < 0.4 { (t * 30. + f.x).sin().abs() } else { 1. };
                p.dot(f.x, f.y, 1.7 * k, hex, a * tw);
            }
            s.b.retain(|f| f.l > 0.);
            for (i, u) in [0.56f32, 0.69, 0.81, 0.93].into_iter().enumerate() {
                let x = u * w;
                let len = (26. + (i % 2) as f32 * 26.) * k;
                let a = (t * 1.3 + i as f32).sin() * 0.12;
                let at = |d: f32| (x - a.sin() * d, a.cos() * d);
                let end = at(len);
                p.line(&[(x, 0.), end], 1.2, 0xc9a24a, 1.);
                let (lx, ly) = at(len + 22. * k);
                p.glow(lx, ly, 36. * k, 0xff5a3c, 0.25);
                p.spr("red_paper_lantern", lx, ly, 46. * k, 0., 1., 1., 1.);
            }
            for m in &mut s.a {
                m.y += m.v * dt;
                m.r += m.vr * dt;
                m.x += (t * 0.9 + m.ph).sin() * 16. * k * dt;
                if m.y > h + 20. {
                    m.y = -20.;
                    m.x = rnd(0., w);
                }
                p.spr("mai", m.x, m.y, m.s, m.r, 0.95, (t * 2. + m.ph).cos(), 1.);
            }
            if t > s.next2 {
                s.next2 = t + rnd(3., 5.);
                s.c.push(Pt { x: rnd(0.3, 0.9) * w, y: -30. * k, r: rnd(-1., 1.), ..Default::default() });
            }
            for e in &mut s.c {
                e.y += 46. * k * dt;
                e.r += 0.8 * dt;
                p.spr("red_envelope", e.x + (t + e.r).sin() * 20. * k, e.y, 30. * k, e.r, 1., 1., 1.);
            }
            s.c.retain(|e| e.y < h + 40.);
        }
        "valentine" => {
            for sp in &s.b {
                let a = (t * 2. + sp.ph).sin().max(0.);
                let r = 4. * k * a + 1.;
                let q = r * 0.25;
                p.shape(&[(sp.x, sp.y - r), (sp.x + q, sp.y - q), (sp.x + r, sp.y), (sp.x + q, sp.y + q), (sp.x, sp.y + r), (sp.x - q, sp.y + q), (sp.x - r, sp.y), (sp.x - q, sp.y - q)], 0xffd1e1, a);
            }
            for hp in &mut s.a {
                hp.y -= hp.v * dt;
                if hp.y < -30. {
                    hp.y = h + 30.;
                    hp.x = rnd(0., w);
                }
                let top = (hp.y / (h * 0.35)).min(1.);
                let name = if hp.n == 1 { "sparkling_heart" } else { "red_heart" };
                p.spr(name, hp.x + (t * 1.2 + hp.ph).sin() * 14. * k, hp.y, hp.s * (1. + 0.06 * (t * 6. + hp.ph).sin()), 0., top, 1., 1.);
            }
        }
        "space" => {
            for st in &s.a {
                let a = 0.35 + 0.6 * (t * st.s + st.ph).sin().abs();
                p.dot(st.x, st.y, st.r, 0xffffff, a);
                if st.r > 1.25 * k.max(0.6) {
                    p.twinkle(st.x, st.y, st.r * 4., 0xdfe8ff, a * 0.7);
                }
            }
            if t > s.next {
                s.next = t + rnd(1.6, 3.2);
                s.b.push(Pt { x: rnd(0.4, 1.1) * w, y: rnd(-0.1, 0.3) * h, l: 1., ..Default::default() });
            }
            for sh in &mut s.b {
                sh.x -= 420. * k * dt;
                sh.y += 170. * k * dt;
                sh.l -= dt * 1.2;
                // (The tail fades out behind it, in a few steps.)
                for j in 0..5 {
                    let (u0, u1) = (j as f32 / 5., (j + 1) as f32 / 5.);
                    p.line(&[(sh.x + 90. * k * u0, sh.y - 36. * k * u0), (sh.x + 90. * k * u1, sh.y - 36. * k * u1)], 2. * k, 0xffffff, sh.l.max(0.) * (1. - u0));
                }
            }
            s.b.retain(|sh| sh.l > 0.);
            p.glow(w - 120. * k, 74. * k, 70. * k, 0xffbe78, 0.18);
            p.spr("ringed_planet", w - 120. * k, 74. * k + (t * 0.7).sin() * 5. * k, 86. * k, 0., 1., 1., 1.);
            p.spr("star", w * 0.62, 48. * k, 18. * k, t * 0.5, 0.9, 1., 1.);
            if s.run.is_none() && t > s.next2 {
                s.run = Some(Run { t0: t, dir: 1., s: 1., sw: 0., sh: 0., ys: 0., ye: 0., c1: 0., c2: 0., bob: 0., ph: 0., d: 7., drop: 0., fr: 0. });
            }
            if let Some(u) = s.run {
                let q = (t - u.t0) / 7.;
                let x = -60. * k + q * (w + 120. * k);
                let y = h * 0.42 + (q * TAU * 2.).sin() * 14. * k;
                if (q * TAU * 3.).sin() > 0.3 {
                    p.shape(&[(x - 8. * k, y + 8. * k), (x + 8. * k, y + 8. * k), (x + 30. * k, h), (x - 30. * k, h)], 0xaaffc8, 0.13);
                }
                p.spr("flying_saucer", x, y, 52. * k, 0., 1., 1., 1.);
                if q > 1. {
                    s.run = None;
                    s.next2 = t + rnd(4., 8.);
                }
            }
        }
        "sakura" => {
            for pt in &mut s.a {
                pt.y += pt.v * dt;
                pt.r += pt.vr * dt;
                pt.x += ((t * 0.8 + pt.ph).sin() * 18. + 10.) * k * dt;
                if pt.y > h + 12. {
                    pt.y = -12.;
                    pt.x = rnd(-40., w);
                }
                if pt.n == 1 {
                    p.spr("cherry_blossom", pt.x, pt.y, pt.s * 3., pt.r, 0.95, 1., 1.);
                } else {
                    let turn = (t * 2.4 + pt.ph).cos().abs().max(0.15);
                    p.pill(pt.x, pt.y, pt.s * 2. * turn, pt.s * 1.2, 0xffc1d6, 0.9);
                    p.pill(pt.x + pt.s * 0.4 * turn, pt.y, pt.s * 0.7 * turn, pt.s * 0.4, 0xff9ec0, 0.9);
                }
            }
            for i in 0..2 {
                let ph = i as f32 * 3.;
                let x = w * (0.55 + 0.3 * (t * 0.23 + ph).sin());
                let y = h * (0.35 + 0.2 * (t * 0.51 + ph * 1.7).sin());
                p.spr("butterfly", x, y, 30. * k, 0., 1., 0.35 + 0.65 * (t * 8. + ph).sin().abs(), 1.);
            }
        }
        "autumn" => {
            for l in &mut s.a {
                l.y += l.v * dt;
                l.r += l.vr * dt;
                l.x += ((t * 0.9 + l.ph).sin() * 26. + 8.) * k * dt;
                if l.y > h + 24. {
                    l.y = -24.;
                    l.x = rnd(-30., w);
                }
                p.spr(if l.n == 1 { "maple_leaf" } else { "fallen_leaf" }, l.x, l.y, l.s, l.r, 0.95, (t * 1.8 + l.ph).cos(), 1.);
            }
        }
        "summer" => {
            p.glow(w - 90. * k, 60. * k, 120. * k, 0xffd65a, 0.28);
            p.spr("sun", w - 90. * k, 60. * k, 92. * k, t * 0.15, 1., 1., 1.);
            p.spr("palm_tree", w - 26. * k, h - 62. * k, 130. * k, 0., 1., 1., 1.);
            for f in &mut s.b {
                f.x -= f.v * dt;
                if f.x < -40. * k {
                    f.x = w + 40. * k;
                }
                p.spr("tropical_fish", f.x, f.y + (t * 2. + f.x * 0.02).sin() * 6. * k, f.s, 0., 1., 1., 1.);
            }
            for b in &mut s.a {
                b.y -= b.v * dt;
                b.x += (t * 1.5 + b.ph).sin() * 10. * k * dt;
                if b.y < -10. {
                    b.y = h + 10.;
                    b.x = rnd(0., w);
                }
                p.circle(b.x, b.y, b.r, 1.2, 0xdcf5ff, 0.7);
                p.dot(b.x - b.r * 0.35, b.y - b.r * 0.35, b.r * 0.22, 0xffffff, 0.8);
            }
        }
        "aurora" => {
            for st in &s.a {
                p.dot(st.x, st.y, st.r, 0xffffff, 0.3 + 0.5 * (t * st.s + st.ph).sin().abs());
            }
            // Three curtains of light: a soft wide band, a brighter core, a bright lower edge and rays rising from it.
            for (i, (hex, base, sp)) in [(0xa45cffu32, 0.2f32, 0.12f32), (0x30d5ff, 0.3, 0.16), (0x3dffa8, 0.4, 0.21)].into_iter().enumerate() {
                let f = i as f32;
                let edge = |x: f32| h * base + (x / w * 5.5 + t * sp * 3. + f * 2.).sin() * h * 0.07 + (x / w * 11. - t * sp * 5. + f).sin() * h * 0.025;
                let thick = |x: f32| h * (0.17 + 0.07 * (x / w * 3. + t * sp * 2. + f * 1.3).sin());
                let xs: Vec<f32> = (0..=40).map(|j| j as f32 / 40. * w).collect();
                for (part, a) in [(1f32, 0.15f32), (0.5, 0.2)] {
                    let mut pts: Vec<(f32, f32)> = xs.iter().map(|&x| (x, edge(x) - thick(x) * part)).collect();
                    pts.extend(xs.iter().rev().map(|&x| (x, edge(x))));
                    p.shape(&pts, hex, a);
                }
                let line: Vec<(f32, f32)> = xs.iter().map(|&x| (x, edge(x))).collect();
                p.line(&line, 8. * k, hex, 0.14);
                p.line(&line, 2.4 * k, mix(hex, 0xffffff, 0.4), 0.6);
                for j in 0..30 {
                    let x = (j as f32 + 0.5) / 30. * w + (t * 0.7 + j as f32).sin() * 6. * k;
                    let y = edge(x);
                    let len = thick(x) * (0.55 + 0.45 * (t * 1.3 + j as f32 * 1.7 + f).sin());
                    p.line(&[(x, y), (x, y - len)], 4. * k, hex, 0.09 + 0.06 * (t * 2. + j as f32 * 0.9).sin());
                }
            }
            let (gy, ga, gw, gp) = (h - 20. * k, 7. * k, 1.2, 0.8);
            p.hills(w, gy, ga, gw, gp, h, 0x041019, 0.97);
            let step = 21. * k.max(0.4);
            for j in 0..(w / step) as usize {
                // (Some trees stand out tall, most are low: not a saw's teeth.)
                if hash(j as u32 + 300) < 0.25 {
                    continue;
                }
                let x = j as f32 * step + hash(j as u32) * 14. * k;
                let gy = hill_y(x, w, gy, ga, gw, gp) + 3. * k;
                let tall = hash(j as u32 + 99);
                let ht = (10. + tall * tall * 42.) * k;
                p.shape(&[(x, gy - ht), (x + ht * 0.3, gy), (x - ht * 0.3, gy)], 0x041019, 0.97);
            }
        }
        "neon" => {
            let hz = h * 0.62;
            for st in &s.a {
                if st.y < hz - 6. {
                    p.dot(st.x, st.y, st.r, 0xffffff, 0.2 + 0.45 * (t * st.s + st.ph).sin().abs());
                }
            }
            // The sun on the horizon, cut in stripes that widen toward its foot.
            let (sx, sr) = (w * 0.66, h * 0.3);
            let sy = hz - sr * 0.3;
            p.glow(sx, sy, sr * 2.4, 0xff2975, 0.34);
            let rows = 22;
            let row = sr * 2. / rows as f32;
            for j in 0..rows {
                let y0 = sy - sr + j as f32 * row;
                if y0 >= hz {
                    break;
                }
                let u = j as f32 / rows as f32;
                let gap = if u > 0.45 { (u - 0.45) * 1.8 * row } else { 0. };
                let dy = y0 + row / 2. - sy;
                let half = (sr * sr - dy * dy).max(0.).sqrt();
                p.rect(sx - half, y0, half * 2., (row - gap).min(hz - y0), mix(0xffd319, 0xff2975, u), 0.95);
            }
            // The floor, its lines running toward the eye.
            p.grad(0., hz, w, h - hz, 180., (0x1a0533, 0.9), (0x0a0118, 0.96));
            for j in -16i32..=16 {
                let (top, foot) = (sx + j as f32 * 16. * k, sx + j as f32 * 130. * k);
                p.line(&[(top, hz), (foot, h)], 1.3 * k, 0xff2bd6, 0.5);
            }
            for m in 0..10 {
                let z = (m as f32 + (t * 0.6).fract()) / 10.;
                let y = hz + (h - hz) * z * z;
                p.line(&[(0., y), (w, y)], 1.3 * k, 0xff2bd6, 0.15 + 0.55 * z);
            }
            p.line(&[(0., hz), (w, hz)], 7. * k, 0xff2bd6, 0.22);
            p.line(&[(0., hz), (w, hz)], 2. * k, 0xff7ae6, 0.95);
        }
        "storm" => {
            for c in &mut s.b {
                c.x += c.v * dt;
                if c.x > w + c.r {
                    c.x = -c.r;
                }
                p.glow(c.x, c.y, c.r * 1.2, 0x1c2230, 0.95);
                p.glow(c.x + c.r * 0.4, c.y + c.r * 0.12, c.r * 0.9, 0x323b4e, 0.75);
            }
            // Lightning now and then: a forked bolt, a flicker and a flash over the whole banner.
            if t > s.next {
                s.next = t + rnd(2.5, 6.);
                s.c.push(Pt { x: rnd(0.3, 0.95) * w, ph: t, n: rnd(0., 250.) as u8, ..Default::default() });
            }
            for b in &s.c {
                let age = t - b.ph;
                let a = match age {
                    a if a < 0.07 => 1.,
                    a if a < 0.13 => 0.25,
                    a if a < 0.2 => 0.9,
                    a => (1. - (a - 0.2) / 0.3).max(0.),
                };
                p.rect(0., 0., w, h, 0xdfe8ff, 0.2 * a);
                let seed = b.n as u32 * 97;
                let mut pts = vec![(b.x, 0.)];
                for j in 1..=12 {
                    let (px_, py_) = pts[j - 1];
                    pts.push((px_ + (hash(seed + j as u32) - 0.5) * 34. * k, py_ + h * 0.065));
                }
                let fork: Vec<(f32, f32)> = (0..6).scan(pts[4], |at, j| {
                    *at = (at.0 + (12. + hash(seed + 50 + j) * 16.) * k, at.1 + h * 0.05);
                    Some(*at)
                }).collect();
                let fork: Vec<(f32, f32)> = std::iter::once(pts[4]).chain(fork).collect();
                for path in [&pts, &fork] {
                    p.line(path, 8. * k, 0x8fb4ff, 0.25 * a);
                    p.line(path, 2.2 * k, 0xffffff, a);
                }
                p.glow(b.x, 10. * k, 120. * k, 0xcfe0ff, 0.5 * a);
            }
            s.c.retain(|b| t - b.ph < 0.5);
            for d in &mut s.a {
                d.y += d.v * dt;
                d.x += d.v * dt * d.s;
                if d.y > h {
                    if s.d.len() < 60 {
                        s.d.push(Pt { x: d.x, y: h - 2. * k, l: 1., ..Default::default() });
                    }
                    d.y = rnd(-40., -6.);
                    d.x = rnd(-0.2 * w, w);
                }
                p.line(&[(d.x - d.l * d.s, d.y - d.l), (d.x, d.y)], 1.1 * k, 0xb8d2ff, 0.45);
            }
            for sp in &mut s.d {
                sp.l -= dt * 4.;
                let u = 1. - sp.l;
                p.pill(sp.x, sp.y, (3. + u * 10.) * k, 2. * k, 0xb8d2ff, sp.l.max(0.) * 0.6);
            }
            s.d.retain(|sp| sp.l > 0.);
        }
        "fireflies" => {
            for b in &s.b {
                let sway = (t * 1.2 + b.ph).sin() * 3. * k;
                p.shape(&[(b.x - 2.6 * k, h), (b.x + 2.6 * k, h), (b.x + sway, h - b.l)], 0x06140b, 0.96);
            }
            for f in &mut s.a {
                f.vx = (f.vx + rnd(-40., 40.) * k * dt).clamp(-20. * k, 20. * k);
                f.vy = (f.vy + rnd(-40., 40.) * k * dt).clamp(-14. * k, 14. * k);
                f.x += f.vx * dt;
                f.y += f.vy * dt;
                if f.x < 0. || f.x > w {
                    f.vx = -f.vx;
                    f.x = f.x.clamp(0., w);
                }
                if f.y < h * 0.12 || f.y > h * 0.95 {
                    f.vy = -f.vy;
                    f.y = f.y.clamp(h * 0.12, h * 0.95);
                }
                let pulse = ((t * f.s * 2. + f.ph).sin() * 0.5 + 0.5).powi(2);
                p.glow(f.x, f.y, 18. * k, 0xd9ff6a, 0.06 + 0.55 * pulse);
                p.dot(f.x, f.y, f.r, 0xf4ffb0, 0.3 + 0.7 * pulse);
            }
        }
        "embers" => {
            for e in &mut s.a {
                e.y -= e.v * dt;
                e.x += (t * e.s + e.ph).sin() * 18. * k * dt;
                if e.y < -10. {
                    e.y = h + 10.;
                    e.x = rnd(0., w);
                }
                let hex = [0xffb347, 0xff7a1f, 0xffd36b][e.n as usize % 3];
                let low = (e.y / h).clamp(0., 1.);
                let fl = 0.6 + 0.4 * (t * 9. * e.s + e.ph).sin();
                p.glow(e.x, e.y, e.r * 7., hex, 0.5 * (0.3 + 0.7 * low) * fl);
                p.dot(e.x, e.y, e.r, mix(hex, 0xffffff, 0.3), (0.5 + 0.5 * low) * fl);
            }
        }
        _ => {}
    }
}

/// The theme playing in a box, `k` the size of its things (1: the banner at 290 px high). Its moving things are kept
/// by `key` between frames.
pub fn theme(key: impl Into<SharedString>, theme: &'static str, k: f32) -> AnyElement {
    let key: SharedString = key.into();
    canvas(|_, _, _| {}, move |bounds: Bounds<Pixels>, _, window, cx| {
        if theme == "none" {
            return;
        }
        let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
        let t = now();
        let k = if k > 0. { k } else { (h / 290.).clamp(0.55, 1.2) };
        SURFS.with(|s| {
            let mut s = s.borrow_mut();
            let surf = s.entry(key.to_string()).or_insert_with(|| Surf { theme, w, h, last: t, sim: init(theme, w, h, k, t) });
            if surf.theme != theme || (surf.w - w).abs() > 1. || (surf.h - h).abs() > 1. {
                *surf = Surf { theme, w, h, last: t, sim: init(theme, w, h, k, t) };
            }
            let dt = (t - surf.last).clamp(0., 0.05);
            surf.last = t;
            window.with_content_mask(Some(ContentMask { bounds }), |win| {
                let mut pen = Pen { w: win, cx, o: bounds.origin };
                draw(theme, &mut surf.sim, &mut pen, t, dt, w, h, k);
            });
        });
    })
    .absolute()
    .inset_0()
    .into_any_element()
}

// ───────────────────────── The rings ─────────────────────────

/// Points round a tilted circle, turning: where each is and whether it is in front of the picture.
fn orbit(n: usize, t: f32, speed: f32, r: f32, tilt: f32) -> Vec<(f32, f32, bool, f32)> {
    (0..n).map(|i| {
        let a = t * speed + i as f32 * TAU / n as f32;
        (a.cos() * r, a.sin() * r * tilt, a.sin() > 0., a)
    }).collect()
}

fn wavy(r: f32, amp: f32, waves: f32, turn: f32) -> Vec<(f32, f32)> {
    (0..=120).map(|i| {
        let a = i as f32 / 120. * TAU;
        let rr = r + (a * waves + turn).sin() * amp;
        (a.cos() * rr, a.sin() * rr)
    }).collect()
}

/// A ring's part: behind the picture (`front` false) or over it. Drawn round 0, 0 (the pen's corner is the middle).
fn ring_part(id: &str, p: &mut Pen, t: f32, r: f32, front: bool) {
    // A soft light of the ring's colour behind the picture (bats, hearts and the orbit have their own).
    let tone = match id {
        "classic" => Some(0xffcf4a),
        "lights" => Some(0xffd166),
        "frost" => Some(0xbfe6ff),
        "pumpkin" => Some(0xff8a1f),
        "mai" => Some(0xf5c542),
        "petals" => Some(0xff9ec0),
        "leaves" => Some(0xe8892f),
        "sun" => Some(0xffc83d),
        "aurora" => Some(0x3dffa8),
        "neon" => Some(0xff2bd6),
        "electric" => Some(0x6fb6ff),
        "fireflies" => Some(0xd9ff6a),
        "fire" => Some(0xff6a1f),
        "glitch" => Some(0x00fff0),
        _ => None,
    };
    if !front && let Some(hex) = tone {
        p.glow(0., 0., r * 2.1, hex, 0.3);
    }
    match id {
        "classic" if front => {
            // A dashed gold circle, turning, with four beads.
            let turn = t * 0.4;
            for i in 0..24 {
                let a0 = turn + i as f32 * TAU / 24.;
                let pts: Vec<(f32, f32)> = (0..=4).map(|j| {
                    let a = a0 + j as f32 / 4. * (TAU / 24. * 0.66);
                    (a.cos() * r * 1.1, a.sin() * r * 1.1)
                }).collect();
                p.line(&pts, r * 0.04, 0xffcf4a, 1.);
            }
            for i in 0..4 {
                let a = turn + i as f32 * TAU / 4.;
                p.dot(a.cos() * r * 1.1, a.sin() * r * 1.1, r * 0.05, 0xffcf4a, 1.);
            }
            p.circle(0., 0., r * 1.03, r * 0.015, 0xffe7a0, 0.7);
            p.circle(0., 0., r * 1.18, r * 0.012, 0xffcf4a, 0.45);
            for i in 0..6 {
                let a = -turn * 0.5 + i as f32 * TAU / 6. + 0.4;
                let tw = (t * 2.2 + i as f32 * 1.9).sin().max(0.);
                p.twinkle(a.cos() * r * 1.18, a.sin() * r * 1.18, r * 0.1 * tw, 0xfff4c8, tw);
            }
        }
        "lights" if front => {
            p.line(&wavy(r * 1.1, r * 0.025, 14., 0.), r * 0.035, 0x1f5a34, 1.);
            for i in 0..14 {
                let a = i as f32 / 14. * TAU + 0.1;
                let br = 0.3 + 0.7 * (t * 3.4 - i as f32 * 0.95).sin().max(0.);
                let hex = BULBS[i % 5];
                let (x, y) = (a.cos() * r * 1.22, a.sin() * r * 1.22);
                p.glow(x, y, r * 0.22, hex, 0.8 * br);
                p.dot(x, y, r * 0.07, hex, 0.45 + 0.55 * br);
            }
        }
        "frost" if front => {
            // Ice spikes round it, then the frosted line, its snow and flakes.
            for i in 0..18 {
                let a = i as f32 * TAU / 18. + 0.2;
                let len = r * (0.08 + 0.1 * hash(i as u32 + 7));
                let tip = r * 1.1 + len;
                p.shape(&[((a - 0.06).cos() * r * 1.1, (a - 0.06).sin() * r * 1.1), (a.cos() * tip, a.sin() * tip), ((a + 0.06).cos() * r * 1.1, (a + 0.06).sin() * r * 1.1)], 0xe8f7ff, 0.85);
            }
            for i in 0..3 {
                let a = t * 0.3 + i as f32 * TAU / 3.;
                let tw = (t * 2.6 + i as f32 * 2.).sin().max(0.);
                p.twinkle(a.cos() * r * 1.32, a.sin() * r * 1.32, r * 0.12 * tw, 0xffffff, tw);
            }
            p.circle(0., 0., r * 1.08, r * 0.07, 0xd6f0ff, 1.);
            for i in 0..9 {
                let a = -PI * 0.85 + i as f32 * PI * 0.7 / 8.;
                p.dot(a.cos() * r * 1.1, a.sin() * r * 1.1 - r * 0.03, r * (0.07 + 0.03 * (i as f32 * 2.3).sin()), 0xffffff, 0.95);
            }
            for (x, y, _, a) in orbit(3, t, 0.5, r * 1.2, 1.) {
                p.spr("snowflake", x, y, r * 0.32, t + a, 1., 1., 1.);
            }
        }
        "bats" => {
            if !front {
                p.glow(0., 0., r * 2.3, 0xa875ff, 0.35);
                p.circle(0., 0., r * 1.08, r * 0.045, 0x8b5cf6, 1.);
            }
            for (x, y, f, a) in orbit(3, t, 0.9, r * 1.32, 0.42) {
                if f == front {
                    p.spr("bat", x, y - r * 0.15, r * if f { 0.42 } else { 0.32 }, 0., if f { 1. } else { 0.8 }, 1., 0.5 + 0.5 * (t * 9. + a).sin().abs());
                }
            }
        }
        "pumpkin" if front => {
            p.circle(0., 0., r * 1.08, r * 0.05, 0xff8a1f, 1.);
            p.line(&wavy(r * 1.08, r * 0.05, 9., t), r * 0.03, 0x3d8b3a, 1.);
            let fl = 0.75 + 0.25 * (t * 13.).sin() * (t * 7.3).sin();
            p.glow(r * 0.78, r * 0.78, r * 0.55, 0xff8c28, 0.45 * fl);
            p.spr("jack_o_lantern", r * 0.78, r * 0.78, r * 0.6, 0., 1., 1., 1.);
            p.spr("jack_o_lantern", -r * 0.86, -r * 0.62, r * 0.34, 0., 1., 1., 1.);
        }
        "mai" if front => {
            p.circle(0., 0., r * 1.08, r * 0.045, 0xf5c542, 1.);
            p.circle(0., 0., r * 1.15, r * 0.02, 0xd8343f, 1.);
            for i in 0..8 {
                let a = t * 0.15 + i as f32 * TAU / 8.;
                p.spr("mai", a.cos() * r * 1.12, a.sin() * r * 1.12, r * if i % 2 == 1 { 0.26 } else { 0.34 }, a * 2., 1., 1., 1.);
            }
        }
        "hearts" => {
            let beat = 1. + 0.05 * (t * 4.2).sin().max(0.).powi(8);
            if !front {
                p.glow(0., 0., r * 2.3 * beat, 0xff5a8a, 0.3);
            } else {
                p.circle(0., 0., r * 1.08 * beat, r * 0.045, 0xff5a8a, 1.);
                for (i, (x, y, _, _)) in orbit(6, t, 0.35, r * 1.2, 1.).into_iter().enumerate() {
                    p.spr(if i % 3 == 0 { "sparkling_heart" } else { "red_heart" }, x, y, r * if i % 2 == 1 { 0.22 } else { 0.3 }, 0., 1., 1., 1.);
                }
            }
        }
        "orbit" => {
            // A tilted path round the picture: its far half behind, its near half in front; a planet and a star on it,
            // and a rocket going round.
            let (sa, ca) = (-0.35f32).sin_cos();
            let tilt = |x: f32, y: f32| (x * ca - y * sa, x * sa + y * ca);
            let half: Vec<(f32, f32)> = (0..=40).map(|i| {
                let a = if front { i as f32 / 40. * PI } else { PI + i as f32 / 40. * PI };
                tilt(a.cos() * r * 1.5, a.sin() * r * 0.42)
            }).collect();
            p.line(&half, r * 0.02, 0xffffff, 0.35);
            for (i, (x, y, f, _)) in orbit(2, t, 0.6, 1., 1.).into_iter().enumerate() {
                if f == front {
                    let (x, y) = tilt(x * r * 1.5, y * r * 0.42);
                    p.spr(if i == 1 { "star" } else { "ringed_planet" }, x, y, r * if i == 1 { 0.24 } else { 0.4 } * if front { 1. } else { 0.75 }, 0., if front { 1. } else { 0.7 }, 1., 1.);
                }
            }
            let rk = t * 0.6 + PI * 0.5;
            if (rk.sin() > 0.) == front {
                let (x, y) = tilt(rk.cos() * r * 1.5, rk.sin() * r * 0.42);
                let heading = (rk.cos() * 0.42).atan2(-rk.sin() * 1.5) - 0.35 + PI / 4. + PI;
                p.spr("rocket", x, y, r * 0.3 * if front { 1. } else { 0.75 }, heading, if front { 1. } else { 0.7 }, 1., 1.);
            }
        }
        "petals" if front => {
            p.circle(0., 0., r * 1.08, r * 0.045, 0xffb3c8, 1.);
            // Petals let go of the ring and drift off it.
            for i in 0..8 {
                let u = (t * 0.22 + i as f32 / 8.).fract();
                let a = i as f32 * TAU / 8. + t * 0.3 + u * 1.4;
                let rad = r * (1.15 + u * 0.55);
                let turn = (t * 2.4 + i as f32).cos().abs().max(0.2);
                p.pill(a.cos() * rad, a.sin() * rad, r * 0.11 * turn, r * 0.065, 0xffc1d6, (1. - u) * 0.9);
            }
            for i in 0..7 {
                let a = t * 0.2 + i as f32 * TAU / 7.;
                p.spr("cherry_blossom", a.cos() * r * 1.1, a.sin() * r * 1.1, r * if i % 2 == 1 { 0.24 } else { 0.32 }, a, 1., 1., 1.);
            }
        }
        "leaves" if front => {
            p.circle(0., 0., r * 1.08, r * 0.045, 0xd9822b, 1.);
            for i in 0..8 {
                let a = i as f32 * TAU / 8. + 0.3;
                p.spr(if i % 2 == 1 { "fallen_leaf" } else { "maple_leaf" }, a.cos() * r * 1.12, a.sin() * r * 1.12, r * 0.3, a + (t * 1.5 + i as f32).sin() * 0.35, 1., 1., 1.);
            }
        }
        "sun" if front => {
            let turn = t * 0.25;
            for i in 0..24 {
                let a = turn + i as f32 * TAU / 24.;
                let l = if i % 2 == 1 { r * 0.1 } else { r * 0.18 };
                p.line(&[(a.cos() * r * 1.08, a.sin() * r * 1.08), (a.cos() * (r * 1.08 + l), a.sin() * (r * 1.08 + l))], r * 0.04, 0xffc83d, 1.);
            }
            p.circle(0., 0., r * 1.06, r * 0.05, 0xffc83d, 1.);
        }
        "sun" => {
            // Behind the picture: soft beams turning slowly.
            for i in 0..12 {
                p.ray(0., 0., -t * 0.12 + i as f32 * TAU / 12., r * 1.75, 0.13, 0xfff0a0, 0.18);
            }
        }
        "aurora" => {
            if !front {
                p.line(&wavy(r * 1.22, r * 0.05, 5., t * 0.8), r * 0.1, 0x30d5ff, 0.22);
                p.line(&wavy(r * 1.3, r * 0.06, 4., -t * 0.6), r * 0.08, 0xa45cff, 0.2);
            } else {
                // A ring whose colour runs round it: green, blue, violet.
                let n = 72;
                for i in 0..n {
                    let (a0, a1) = (i as f32 / n as f32 * TAU, (i + 1) as f32 / n as f32 * TAU + 0.012);
                    let hex = cycle(i as f32 / n as f32 + t * 0.08, &[0x3dffa8, 0x30d5ff, 0xa45cff, 0x30d5ff]);
                    p.line(&[(a0.cos() * r * 1.08, a0.sin() * r * 1.08), (a1.cos() * r * 1.08, a1.sin() * r * 1.08)], r * 0.07, hex, 0.95);
                }
                for i in 0..3 {
                    let a = -t * 0.4 + i as f32 * TAU / 3.;
                    let tw = (t * 2. + i as f32 * 2.1).sin().max(0.);
                    p.twinkle(a.cos() * r * 1.2, a.sin() * r * 1.2, r * 0.12 * tw, 0xeafff6, tw);
                }
            }
        }
        "neon" if front => {
            // A pink tube that flickers now and then, and a dashed blue one turning round it.
            let flick = if hash((t * 8.) as u32) > 0.94 { 0.35 } else { 1. };
            p.circle(0., 0., r * 1.08, r * 0.11, 0xff2bd6, 0.22 * flick);
            p.circle(0., 0., r * 1.08, r * 0.04, 0xff8ae9, flick);
            for i in 0..16 {
                let a0 = t * 0.5 + i as f32 * TAU / 16.;
                let pts: Vec<(f32, f32)> = (0..=4).map(|j| {
                    let a = a0 + j as f32 / 4. * TAU / 16. * 0.55;
                    (a.cos() * r * 1.2, a.sin() * r * 1.2)
                }).collect();
                p.line(&pts, r * 0.07, 0x00e5ff, 0.2);
                p.line(&pts, r * 0.025, 0x9ff6ff, 0.95);
            }
        }
        "electric" if front => {
            p.circle(0., 0., r * 1.08, r * 0.03, 0x6fb6ff, 0.6);
            // Arcs crackling round it, new ones fourteen times a second.
            let seed = (t * 14.) as u32;
            for arc in 0..4u32 {
                let start = hash(seed * 7 + arc) * TAU;
                let span = 0.6 + hash(seed * 13 + arc) * 0.9;
                let pts: Vec<(f32, f32)> = (0..=14u32).map(|j| {
                    let a = start + span * j as f32 / 14.;
                    let off = if j == 0 || j == 14 { 0. } else { (hash(seed * 31 + arc * 17 + j) - 0.5) * r * 0.18 };
                    (a.cos() * (r * 1.08 + off), a.sin() * (r * 1.08 + off))
                }).collect();
                p.line(&pts, r * 0.09, 0x4d9bff, 0.25);
                p.line(&pts, r * 0.025, 0xeaf4ff, 0.95);
            }
            for j in 0..3u32 {
                let a = hash(seed * 3 + j + 90) * TAU;
                p.glow(a.cos() * r * 1.12, a.sin() * r * 1.12, r * 0.25, 0x8fc4ff, 0.5);
                p.dot(a.cos() * r * 1.12, a.sin() * r * 1.12, r * 0.03, 0xffffff, 1.);
            }
        }
        "fireflies" if front => {
            p.circle(0., 0., r * 1.06, r * 0.02, 0x9fd86a, 0.45);
            for i in 0..9 {
                let f = i as f32;
                let a = t * (0.2 + 0.05 * f) * if i % 2 == 0 { 1. } else { -1. } + f * 0.7;
                let rad = r * (1.16 + 0.14 * (t * 0.7 + f * 1.3).sin());
                let (x, y) = (a.cos() * rad, a.sin() * rad * 0.94);
                let pulse = ((t * 2. + f * 1.7).sin() * 0.5 + 0.5).powi(2);
                p.glow(x, y, r * 0.32, 0xd9ff6a, 0.05 + 0.5 * pulse);
                p.dot(x, y, r * 0.04, 0xf4ffb0, 0.35 + 0.65 * pulse);
            }
        }
        "fire" if front => {
            // Tongues of flame licking outward, then the burning line over their feet.
            for i in 0..30 {
                let f = i as f32;
                let a = f / 30. * TAU;
                let flick = 0.55 + 0.45 * ((t * 7. + f * 2.1).sin() * (t * 4.3 + f * 1.3).cos()).abs();
                let len = r * (0.16 + 0.24 * flick) * if i % 2 == 0 { 1. } else { 0.7 };
                let tip = a + (t * 3. + f).sin() * 0.07;
                let base = r * 1.06;
                for (scale, half, hex, al) in [(1f32, 0.11f32, 0xff4d1f, 0.85f32), (0.55, 0.06, 0xffd23f, 0.95)] {
                    p.shape(&[((a - half).cos() * base, (a - half).sin() * base), (tip.cos() * (base + len * scale), tip.sin() * (base + len * scale)), ((a + half).cos() * base, (a + half).sin() * base)], hex, al);
                }
            }
            p.circle(0., 0., r * 1.06, r * 0.06, 0xff7a1f, 1.);
            p.circle(0., 0., r * 1.06, r * 0.02, 0xffe08a, 0.9);
        }
        "glitch" if front => {
            // A white line with a red and a blue copy, torn apart now and then, with slices and loose pixels.
            let seed = (t * 10.) as u32;
            let on = hash(seed) > 0.7;
            let j = if on { (hash(seed * 3 + 1) - 0.5) * r * 0.14 } else { r * 0.015 };
            p.circle(j, 0., r * 1.08, r * 0.04, 0xff004d, 0.85);
            p.circle(-j, j * 0.3, r * 1.08, r * 0.04, 0x00fff0, 0.85);
            p.circle(0., 0., r * 1.08, r * 0.03, 0xffffff, 0.95);
            if on {
                for n in 0..3u32 {
                    let y = (hash(seed * 5 + n) - 0.5) * r * 2.2;
                    let x = -r * 1.3 + hash(seed * 11 + n) * r * 0.5;
                    p.rect(x, y, r * (0.4 + hash(seed * 17 + n) * 1.3), r * (0.03 + 0.04 * hash(seed * 19 + n)), if n % 2 == 0 { 0x00fff0 } else { 0xff004d }, 0.7);
                }
                for n in 0..6u32 {
                    let a = hash(seed * 23 + n) * TAU;
                    let s = r * 0.05;
                    p.rect(a.cos() * r * 1.22 - s / 2., a.sin() * r * 1.22 - s / 2., s, s, if n % 2 == 0 { 0xffffff } else { 0x00fff0 }, 0.85);
                }
            }
        }
        _ => {}
    }
}

/// A ring's part round a picture `r` in radius, laid over a box centred on the picture (any size: nothing is cut).
pub fn ring(id: &'static str, r: f32, front: bool) -> AnyElement {
    canvas(|_, _, _| {}, move |bounds: Bounds<Pixels>, _, window, cx| {
        if id == "none" {
            return;
        }
        let mid = bounds.center();
        let mut pen = Pen { w: window, cx, o: mid };
        ring_part(id, &mut pen, now(), r, front);
    })
    .absolute()
    .inset_0()
    .into_any_element()
}

/// The picker's small round: a grey face with the ring round it, both parts.
pub fn ring_tile(id: &'static str) -> AnyElement {
    canvas(|_, _, _| {}, move |bounds: Bounds<Pixels>, _, window, cx| {
        let mid = bounds.center();
        let r = f32::from(bounds.size.width) / 3.4;
        let mut pen = Pen { w: window, cx, o: mid };
        let t = now();
        ring_part(id, &mut pen, t, r, false);
        pen.dot(0., 0., r, 0x4a4352, 1.);
        ring_part(id, &mut pen, t, r, true);
    })
    .absolute()
    .inset_0()
    .into_any_element()
}

/// Keeps the window drawing 30 times a second while something here moves and the window is in front.
pub fn clock(id: &'static str) -> AnyElement {
    let d = div().absolute().size_0();
    if crate::kit::looping() {
        d.with_animation(id, Animation::new(std::time::Duration::from_secs(1)).repeat().with_max_fps(60.), |d, _| d).into_any_element()
    } else {
        d.into_any_element()
    }
}

// ───────────────────────── With the music ─────────────────────────
//
// The profile moving with its song (beat.rs reads it), done as the Trap Nation videos do it (and js.nation, the open
// one made after them): round the picture, the bass's curve as filled rings, one over another, each colour a step
// later and reaching further when the bass is strong, so the colours trail behind the white; the picture swells with
// the bass and everything shakes with it; sparks fly out from the picture toward the eye, faster as the bass hits.
// The colours come from the song's cover. All shapes, nothing kept but the last 7 curves and the sparks; `power` from
// 0.45 (light) to 1 (strong).

use std::collections::VecDeque;

/// The rings' colours, the white one on top, then ever further out: from the cover's colour, turned round the wheel.
fn ring_tones(hex: u32) -> [u32; 7] {
    [0xffffff, mix(hex, 0xffffff, 0.55), hex, turn_hue(hex, 40.), turn_hue(hex, -40.), turn_hue(hex, 85.), mix(turn_hue(hex, 180.), 0x000000, 0.25)]
}

/// How far each ring reaches past the others when the bass is strong, and how much its curve is smoothed.
const REACH: [f32; 7] = [1., 1.12, 1.14, 1.3, 1.33, 1.36, 1.5];
const SMOOTH: [usize; 7] = [0, 2, 2, 3, 3, 3, 5];

/// A colour turned round the colour wheel by `deg`, its lightness and strength kept.
fn turn_hue(hex: u32, deg: f32) -> u32 {
    let mut h: Hsla = rgb(hex).into();
    h.s = h.s.max(0.55);
    h.l = h.l.clamp(0.45, 0.62);
    h.h = (h.h + deg / 360.).rem_euclid(1.);
    let c = Rgba::from(h);
    ((c.r * 255.).round() as u32) << 16 | ((c.g * 255.).round() as u32) << 8 | (c.b * 255.).round() as u32
}

#[derive(Default)]
struct Pulse {
    curves: VecDeque<[f32; crate::beat::LOW]>,
    ring_last: f32,
    sparks: Vec<Pt>,
    banner_last: f32,
    shake: (f32, f32, f32, f32, f32, bool, bool),
    shake_last: f32,
}

thread_local! {
    static PULSE: RefCell<Pulse> = RefCell::new(Pulse::default());
}

/// How far everything is shaken now (px across, down): a swing that starts anew, another way and strength, each time it
/// has run an eighth of a turn, as far as 13 px with the bass full and `power` 1.
pub fn shake(punch: f32, power: f32) -> (f32, f32) {
    let t = now();
    PULSE.with(|s| {
        let mut s = s.borrow_mut();
        let dt = (t - s.shake_last).clamp(0., 0.05);
        s.shake_last = t;
        let (mut ph, ax, ay, sx, sy, cx_, cy_) = s.shake;
        ph += PI / 3. * punch * dt * 60.;
        if ph > PI / 8. || ax == 0. {
            ph = 0.;
            let sign = |_: ()| if rnd(0., 1.) < 0.5 { -1. } else { 1. };
            s.shake = (ph, rnd(0.9, 1.6), rnd(0.9, 1.6), rnd(0.9, 1.6) * sign(()), rnd(0.9, 1.6) * sign(()), rnd(0., 1.) < 0.5, rnd(0., 1.) < 0.5);
        } else {
            s.shake = (ph, ax, ay, sx, sy, cx_, cy_);
        }
        let (_, ax, ay, sx, sy, cx_, cy_) = s.shake;
        let trig = |c: bool, v: f32| if c { v.cos() } else { v.sin() };
        (trig(cx_, ph * sx) * 8. * ax * punch * power, trig(cy_, ph * sy) * 8. * ay * punch * power)
    })
}

/// The song round the picture (`r` its radius, already swollen with the bass), laid behind it in a box centred on it.
pub fn pulse_ring(b: crate::beat::Beat, hex: u32, power: f32, r: f32) -> AnyElement {
    use crate::beat::LOW;
    canvas(|_, _, _| {}, move |bounds: Bounds<Pixels>, _, window, cx| {
        let mut p = Pen { w: window, cx, o: bounds.center() };
        let t = now();
        PULSE.with(|s| {
            let mut s = s.borrow_mut();
            // (One curve a frame; ring n draws the one n frames old.)
            if t - s.ring_last > 0.012 {
                s.ring_last = t;
                s.curves.push_front(b.low);
                s.curves.truncate(REACH.len());
            }
            let tones = ring_tones(hex);
            p.glow(0., 0., r * (1.8 + 0.6 * b.punch * power), hex, (0.2 + 0.45 * b.punch * power).min(0.65));
            for layer in (0..REACH.len()).rev() {
                let Some(curve) = s.curves.get(layer).or(s.curves.back()) else { continue };
                let m = SMOOTH[layer];
                let reach = |i: usize| {
                    // (A triangle's weights over the neighbours, `m` each side.)
                    let (mut sum, mut wsum) = (0., 0.);
                    for j in i.saturating_sub(m)..=(i + m).min(LOW - 1) {
                        let wgt = (m + 1 - j.abs_diff(i)) as f32;
                        sum += curve[j] * wgt;
                        wsum += wgt;
                    }
                    let v = sum / wsum.max(1.);
                    (r * 0.12 * ((1. + 4. * v).powf(REACH[layer]) - 1.) * power).min(r * (0.32 + 0.1 * layer as f32))
                };
                let pts: Vec<(f32, f32)> = (0..LOW).map(|i| {
                    let a = -PI / 2. + PI * i as f32 / (LOW - 1) as f32;
                    let rad = r + reach(i);
                    (a.cos() * rad, a.sin() * rad)
                }).collect();
                // The right half, then the left one mirrored, back to the top.
                let shape: Vec<(f32, f32)> = pts.iter().copied().chain(pts.iter().rev().map(|&(x, y)| (-x, y))).collect();
                let soft: Vec<(f32, f32)> = shape.iter().map(|&(x, y)| (x * 1.05, y * 1.05)).collect();
                let al = 0.95 - 0.04 * layer as f32;
                p.shape(&soft, tones[layer], 0.22);
                p.shape(&shape, tones[layer], al);
            }
        });
    })
    .absolute()
    .inset_0()
    .into_any_element()
}

/// The song over the banner: sparks flying out from the picture (at `from`, in the banner) toward the eye, and a flash of
/// the cover's colour at each beat.
pub fn pulse_banner(b: crate::beat::Beat, hex: u32, power: f32, from: (f32, f32)) -> AnyElement {
    canvas(|_, _, _| {}, move |bounds: Bounds<Pixels>, _, window, cx| {
        let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
        let k = (h / 290.).clamp(0.55, 1.2);
        let t = now();
        window.with_content_mask(Some(ContentMask { bounds }), |win| {
            let mut p = Pen { w: win, cx, o: bounds.origin };
            PULSE.with(|s| {
                let mut s = s.borrow_mut();
                let dt = (t - s.banner_last).clamp(0., 0.05);
                s.banner_last = t;
                let n = ((w * h / 2_400.) as usize).clamp(80, 220);
                let fresh = || Pt { x: rnd(-PI * 0.97, 0.2), y: rnd(10., 120.), l: rnd(0., 200.), v: rnd(1.1, 1.45), ph: rnd(0., 1.), vr: rnd(0.1, 0.25), s: rnd(0.05, 0.4), n: u8::from(rnd(0., 1.) < 0.4), ..Default::default() };
                while s.sparks.len() < n {
                    s.sparks.push(fresh());
                }
                s.sparks.truncate(n);
                p.glow(from.0, from.1, w * 0.35, hex, 0.3 * b.punch * power);
                p.rect(0., 0., w, h, hex, 0.1 * b.kick * power);
                let light = mix(hex, 0xffffff, 0.6);
                for sp in &mut s.sparks {
                    // Out from the middle and toward the eye (x and depth move together), at the bass's speed, with a
                    // small wobble of its own; from the eye's plane it starts again.
                    let speed = (sp.v * b.punch).max(0.15) * dt * 60.;
                    sp.l += speed;
                    sp.ph += sp.vr * (0.025 + 0.375 * b.punch) * dt * 60.;
                    if sp.l > 196. {
                        *sp = Pt { l: 0., ..fresh() };
                    }
                    let near = 200. - sp.l;
                    let reach = sp.y * k * (sp.l / 200.) * (150. / near.max(4.));
                    let a = sp.x + (TAU * sp.ph).sin() * sp.s * (0.1 + 0.9 * b.punch) * 0.3;
                    let (x, y) = (from.0 + a.cos() * reach * 1.6, from.1 + a.sin() * reach);
                    let size = (0.5 + 3.5 * (sp.l / 200.).powi(3)) * k;
                    let al = (sp.l / 40.).min(1.) * 0.9;
                    p.dot(x, y, size, if sp.n == 0 { 0xffffff } else { light }, al);
                }
            });
        });
    })
    .absolute()
    .inset_0()
    .into_any_element()
}
