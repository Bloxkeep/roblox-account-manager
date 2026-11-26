//! The animated themes (the C# app's page drew them with livescene.js; the port showed only their still picture): what
//! moves over a theme's picture, drawn in the picture's own 1600 × 1000 space, fitted like the picture (cover, centred).
//! It runs at 60 frames a second only while the window is seen and in use: never in the tray, minimized or behind
//! Roblox, so it costs nothing while accounts play. A mouse moved over it while another window is in use wakes it for a
//! moment. Each moves as the page's did, from the same numbers (the same seeds give the same fish, flies and petals).
use std::{
    cell::RefCell,
    f32::consts::{FRAC_PI_2, PI, TAU},
    time::{Duration, Instant},
};

use gpui_kit::{prelude::*, *};

use crate::pages::deco::{Pen, col};

const W: f32 = 1600.;
const H: f32 = 1000.;
/// 60 frames a second, as the page's own motion asks for them (`with_max_fps(60.)`): the same step of the shared clock.
const STEP: Duration = Duration::from_nanos(1_000_000_000 / 60);

/// The themes that move, by their picture (scene-<id>.svg).
const MOVING: [&str; 11] = ["koipond", "fireflies", "rainglass", "blockworld", "peachfall", "starrynight", "greatwave", "waterlilies", "cubism", "blocksart", "goldleaf"];

pub fn of(file: &str) -> Option<&'static str> {
    let id = file.strip_prefix("scene-")?.strip_suffix(".svg")?;
    MOVING.into_iter().find(|t| *t == id)
}

fn start(id: &str) -> Box<dyn Effect> {
    match id {
        "koipond" => Box::new(Koi::new()),
        "rainglass" => Box::new(Rain::new()),
        "blockworld" => Box::new(Blocks::new()),
        "peachfall" => Box::new(Petals::new()),
        "starrynight" => Box::new(Starry::new()),
        "greatwave" => Box::new(Wave::new()),
        "waterlilies" => Box::new(Lilies::new()),
        "cubism" => Box::new(Cubism::new()),
        "blocksart" => Box::new(BlocksArt::new()),
        "goldleaf" => Box::new(Gold::new()),
        _ => Box::new(Fireflies::new()),
    }
}

trait Effect {
    fn frame(&mut self, c: &mut Cv, t: f32, dt: f32, p: &Pointer);
    fn click(&mut self, _x: f32, _y: f32) {}
}

/// The page's random numbers (a Park–Miller generator), so each theme starts as it did there.
struct Rng(u64);

impl Rng {
    fn f(&mut self) -> f32 {
        self.0 = self.0 * 16807 % 2147483647;
        self.0 as f32 / 2147483647.
    }
    fn pick<T: Copy>(&mut self, list: &[T]) -> T {
        list[((self.f() * list.len() as f32) as usize).min(list.len() - 1)]
    }
}

/// Where the picture's space lands in the window's box.
#[derive(Clone, Copy)]
struct Fit {
    s: f32,
    ox: f32,
    oy: f32,
}

struct Pointer {
    x: f32,
    y: f32,
    on: bool,
}

fn near(ax: f32, ay: f32, p: &Pointer, d: f32) -> bool {
    (ax - p.x).powi(2) + (ay - p.y).powi(2) < d * d
}

type Pt = (f32, f32);

/// Draws in the picture's space.
struct Cv<'p, 'a> {
    p: &'p mut Pen<'a>,
    f: Fit,
}

impl Cv<'_, '_> {
    fn pt(&self, (x, y): Pt) -> Point<Pixels> {
        self.p.at(self.f.ox + x * self.f.s, self.f.oy + y * self.f.s)
    }
    /// Shapes filled in one colour, as one path (overlaps stay filled).
    fn fill(&mut self, polys: &[Vec<Pt>], hex: u32, a: f32) {
        if a <= 0.01 || polys.iter().all(|p| p.len() < 3) {
            return;
        }
        let mut b = PathBuilder::fill().with_style(PathStyle::Fill(FillOptions::non_zero()));
        for poly in polys.iter().filter(|p| p.len() >= 3) {
            let pts: Vec<_> = poly.iter().map(|&q| self.pt(q)).collect();
            b.add_polygon(&pts, true);
        }
        if let Ok(path) = b.build() {
            self.p.w.paint_path(path, col(hex, a));
        }
    }
    fn fill1(&mut self, poly: Vec<Pt>, hex: u32, a: f32) {
        self.fill(&[poly], hex, a);
    }
    /// Lines in one colour, as one path.
    fn stroke(&mut self, lines: &[Vec<Pt>], width: f32, closed: bool, hex: u32, a: f32) {
        self.stroke_dashed(lines, width, closed, None, hex, a);
    }
    fn stroke_dashed(&mut self, lines: &[Vec<Pt>], width: f32, closed: bool, dash: Option<(f32, f32)>, hex: u32, a: f32) {
        if a <= 0.01 || lines.iter().all(|l| l.len() < 2) {
            return;
        }
        let s = self.f.s;
        let mut b = PathBuilder::stroke(px(width * s));
        if let Some((on, off)) = dash {
            b = b.dash_array(&[px(on * s), px(off * s)]);
        }
        for line in lines.iter().filter(|l| l.len() >= 2) {
            let pts: Vec<_> = line.iter().map(|&q| self.pt(q)).collect();
            b.add_polygon(&pts, closed);
        }
        if let Ok(path) = b.build() {
            self.p.w.paint_path(path, col(hex, a));
        }
    }
    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, hex: u32, a: f32) {
        let f = self.f;
        self.p.rect(f.ox + x * f.s, f.oy + y * f.s, w * f.s, h * f.s, hex, a);
    }
    fn dot(&mut self, x: f32, y: f32, r: f32, hex: u32, a: f32) {
        let f = self.f;
        self.p.dot(f.ox + x * f.s, f.oy + y * f.s, r * f.s, hex, a);
    }
}

/// An ellipse turned by `rot`, as `n` points.
fn ell(cx: f32, cy: f32, rx: f32, ry: f32, rot: f32, n: usize) -> Vec<Pt> {
    let (c, s) = (rot.cos(), rot.sin());
    (0..n)
        .map(|i| {
            let a = i as f32 * TAU / n as f32;
            let (u, v) = (a.cos() * rx, a.sin() * ry);
            (cx + u * c - v * s, cy + u * s + v * c)
        })
        .collect()
}

fn circ(cx: f32, cy: f32, r: f32) -> Vec<Pt> {
    ell(cx, cy, r, r, 0., ((r * 0.8) as usize).clamp(10, 48))
}

/// A quadratic curve's points after `from` (the page's quadraticCurveTo).
fn quad(out: &mut Vec<Pt>, from: Pt, ctrl: Pt, to: Pt, n: usize) {
    for i in 1..=n {
        let k = i as f32 / n as f32;
        let m = 1. - k;
        out.push((m * m * from.0 + 2. * m * k * ctrl.0 + k * k * to.0, m * m * from.1 + 2. * m * k * ctrl.1 + k * k * to.1));
    }
}

/// A cubic curve's points after `from` (bezierCurveTo).
fn cubic(out: &mut Vec<Pt>, from: Pt, c1: Pt, c2: Pt, to: Pt, n: usize) {
    for i in 1..=n {
        let k = i as f32 / n as f32;
        let m = 1. - k;
        let (a, b, c, d) = (m * m * m, 3. * m * m * k, 3. * m * k * k, k * k * k);
        out.push((a * from.0 + b * c1.0 + c * c2.0 + d * to.0, a * from.1 + b * c1.1 + c * c2.1 + d * to.1));
    }
}

/// Points moved: turned by `rot`, scaled by `s`, then put at `at`.
fn place(pts: &[Pt], at: Pt, rot: f32, s: f32) -> Vec<Pt> {
    let (c, sn) = (rot.cos(), rot.sin());
    pts.iter().map(|&(x, y)| (at.0 + (x * c - y * sn) * s, at.1 + (x * sn + y * c) * s)).collect()
}

fn rect_pts(x: f32, y: f32, w: f32, h: f32) -> Vec<Pt> {
    vec![(x, y), (x + w, y), (x + w, y + h), (x, y + h)]
}

/// Rings spreading on water (the page's rippler).
struct Ripples {
    rings: Vec<(f32, f32, f32, f32)>,
    hex: u32,
    a: f32,
}

impl Ripples {
    fn new(hex: u32, a: f32) -> Self {
        Self { rings: Vec::new(), hex, a }
    }
    fn add(&mut self, x: f32, y: f32, size: f32) {
        if self.rings.len() < 14 {
            self.rings.push((x, y, size, 0.));
        }
    }
    fn draw(&mut self, c: &mut Cv, dt: f32) {
        for q in &mut self.rings {
            q.3 += dt;
        }
        self.rings.retain(|q| q.3 / 2.4 < 1.);
        for &(x, y, size, age) in &self.rings {
            let k = age / 2.4;
            let lines: Vec<Vec<Pt>> = [1., 0.62].iter().map(|f| {
                let r = size * k * f + 2.;
                ell(x, y, r, r * 0.62, 0., 40)
            }).collect();
            c.stroke(&lines, 2.5, true, self.hex, self.a * (1. - k) * 0.55);
        }
    }
}

struct Live {
    id: &'static str,
    fx: Box<dyn Effect>,
    t: f32,
    last: Instant,
    clicks: Vec<Pt>,
}

thread_local! {
    static LIVE: RefCell<Option<Live>> = const { RefCell::new(None) };
}

/// Whether the theme's things should move now: the window is seen, and in use or just woken by the mouse.
pub fn running(window: &Window) -> bool {
    // (A hidden shot's clip: its window is never seen, and moves all the same.)
    #[cfg(feature = "shot")]
    if std::env::var("BLOXKEEP_SHOT_FRAMES").is_ok() || std::env::var("BLOXKEEP_SHOT_BENCH").is_ok() || std::env::var("BLOXKEEP_SHOT_TALLY").is_ok() {
        return true;
    }
    window.is_visible() && (window.is_window_active() || !crate::kit::still_behind() || crate::kit::mouse_over_lately())
}

/// Keeps the window drawing 60 times a second while `running`; the task ends with the window. Only the theme's layer
/// is drawn anew (`redraw`): the app over it is kept (shell/parts.rs) unless something in it changed.
///
/// Its frames are on the process's shared clock (`on_frame_step`), the one the page's own motion waits for too (the
/// playing bars on Home, 60 a second on theirs), so both are drawn in the same frames. On a clock of its own the two
/// drifted apart and the window was drawn about 60 times a second, each time the whole page (a real error.log: 66 a
/// second).
pub fn ticker(window: &mut Window, cx: &mut Context<crate::shell::Root>) -> Task<()> {
    // (Each step asks for the next; the chain ends when the task is let go, or with the window.)
    // (Not moving (the window behind, hidden, in the tray): looked at again 4 times a second, off the shared clock, so
    // the fine timer tick it holds is let go (on_frame_step) and Windows isn't woken 30 times a second for nothing.)
    fn step(handle: AnyWindowHandle, alive: std::rc::Weak<()>, cx: &mut App) {
        let moving = handle.update(cx, |_, window, _| running(window)).unwrap_or(false);
        if moving {
            on_frame_step(cx, STEP, move |cx| {
                if alive.upgrade().is_some() && handle.update(cx, |_, window, _| if running(window) { window.redraw() }).is_ok() {
                    step(handle, alive, cx);
                }
            });
        } else {
            cx.spawn(async move |cx| {
                cx.background_executor().timer(Duration::from_millis(250)).await;
                let _ = cx.update(|cx| {
                    if alive.upgrade().is_some() {
                        step(handle, alive, cx);
                    }
                });
            })
            .detach();
        }
    }
    let alive = std::rc::Rc::new(());
    step(window.window_handle(), std::rc::Rc::downgrade(&alive), cx);
    cx.foreground_executor().spawn(async move {
        let _alive = alive;
        std::future::pending::<()>().await
    })
}

/// Lets the moving things go (another theme picked).
pub fn stop() {
    LIVE.with(|l| l.borrow_mut().take());
}

/// The moving layer of theme `id`, over its picture.
pub fn layer(id: &'static str) -> AnyElement {
    canvas(
        |_, _, _| {},
        move |bounds: Bounds<Pixels>, _, window, cx| {
            let (bw, bh) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
            let s = (bw / W).max(bh / H);
            let fit = Fit { s, ox: (bw - W * s) / 2., oy: (bh - H * s) / 2. };
            let to_scene = move |at: Point<Pixels>| ((f32::from(at.x - bounds.origin.x) - fit.ox) / s, (f32::from(at.y - bounds.origin.y) - fit.oy) / s);
            let mouse = window.mouse_position();
            let (x, y) = to_scene(mouse);
            let pointer = Pointer { x, y, on: bounds.contains(&mouse) && running(window) };
            // A click anywhere (on the page too, as the web page's): what the theme does with it, at the next frame.
            window.on_mouse_event(move |e: &MouseDownEvent, phase, _, _| {
                if phase == DispatchPhase::Bubble && e.button == MouseButton::Left && bounds.contains(&e.position) {
                    let at = to_scene(e.position);
                    LIVE.with(|l| {
                        if let Some(l) = l.borrow_mut().as_mut() {
                            l.clicks.push(at);
                        }
                    });
                }
            });
            let run = running(window);
            LIVE.with(|l| {
                let mut l = l.borrow_mut();
                if l.as_ref().is_none_or(|l| l.id != id) {
                    *l = Some(Live { id, fx: start(id), t: 0., last: Instant::now(), clicks: Vec::new() });
                }
                let Some(live) = l.as_mut() else { return };
                // (A still frame drawn again, the window not in use: nothing moves on.)
                let dt = if run { live.last.elapsed().as_secs_f32().clamp(0., 0.1) } else { 0. };
                live.last = Instant::now();
                live.t += dt;
                for (x, y) in std::mem::take(&mut live.clicks) {
                    live.fx.click(x, y);
                }
                window.with_content_mask(Some(ContentMask { bounds }), |win| {
                    let mut pen = Pen { w: win, cx, o: bounds.origin };
                    let mut c = Cv { p: &mut pen, f: fit };
                    live.fx.frame(&mut c, live.t, dt, &pointer);
                });
            });
        },
    )
    .absolute()
    .inset_0()
    .into_any_element()
}

// ───────────────────────── Koi pond ─────────────────────────

struct Fish {
    x: f32,
    y: f32,
    a: f32,
    base: u32,
    patch: u32,
    size: f32,
    seed: f32,
    speed: f32,
    pts: Vec<Pt>,
}

struct Food {
    x: f32,
    y: f32,
    age: f32,
    bits: Vec<Pt>,
}

/// Koi from above: they scatter from the pointer and come to where you click, as to food.
struct Koi {
    r: Rng,
    rings: Ripples,
    food: Vec<Food>,
    fish: Vec<Fish>,
}

const PADS: [(f32, f32, f32, bool); 16] = [
    (120., 120., 74., true), (235., 215., 46., false), (60., 330., 56., false), (1490., 110., 80., true), (1370., 70., 44., false), (1530., 300., 58., false),
    (1440., 860., 76., true), (1300., 930., 50., false), (1550., 700., 46., false), (120., 880., 70., true), (270., 945., 46., false), (690., 950., 52., false),
    (800., 60., 48., true), (640., 40., 36., false), (1040., 520., 40., false), (470., 470., 36., true),
];

impl Koi {
    fn new() -> Self {
        let mut r = Rng(11);
        let looks = [(0xf7f2e6, 0xe8622c), (0xf0a32b, 0xf9dc8c), (0xf7f2e6, 0xc8262c), (0x22222a, 0xf2efe6), (0xf7f3ea, 0xf08a2a), (0xe8622c, 0xf7efe0), (0xf4c542, 0xfff4cc), (0xf7f2e6, 0x30303a)];
        let fish = looks
            .iter()
            .enumerate()
            .map(|(i, &(base, patch))| {
                let (x, y, size) = (200. + r.f() * 1200., 150. + r.f() * 700., 15. + r.f() * 8.);
                let a = r.f() * TAU;
                let speed = 45. + r.f() * 30.;
                Fish { x, y, a, base, patch, size, seed: i as f32 * 1.7, speed, pts: vec![(x, y); 11] }
            })
            .collect();
        Self { r, rings: Ripples::new(0xd9f2ec, 1.), food: Vec::new(), fish }
    }

    fn body(f: &Fish, dx: f32, dy: f32) -> Vec<Pt> {
        let n = f.pts.len();
        let side: Vec<[f32; 4]> = (0..n)
            .map(|i| {
                let (p, q, o) = (f.pts[i], f.pts[(i + 1).min(n - 1)], f.pts[i.saturating_sub(1)]);
                let ang = (o.1 - q.1).atan2(o.0 - q.0) + FRAC_PI_2;
                let w = f.size * (PI * (i as f32 + 0.9) / (n as f32 + 0.6)).sin().powf(0.8);
                [p.0 + ang.cos() * w + dx, p.1 + ang.sin() * w + dy, p.0 - ang.cos() * w + dx, p.1 - ang.sin() * w + dy]
            })
            .collect();
        let mut out = vec![(side[0][0], side[0][1])];
        for i in 1..n {
            let from = *out.last().unwrap();
            quad(&mut out, from, (side[i - 1][0], side[i - 1][1]), ((side[i - 1][0] + side[i][0]) / 2., (side[i - 1][1] + side[i][1]) / 2.), 3);
        }
        out.push((side[n - 1][2], side[n - 1][3]));
        for i in (0..n - 1).rev() {
            let from = *out.last().unwrap();
            quad(&mut out, from, (side[i + 1][2], side[i + 1][3]), ((side[i + 1][2] + side[i][2]) / 2., (side[i + 1][3] + side[i][3]) / 2.), 3);
        }
        out
    }

    fn fin(p: Pt, ang: f32, len: f32, wid: f32) -> Vec<Pt> {
        let mut out = vec![p];
        let tip = (p.0 + ang.cos() * len * 1.25, p.1 + ang.sin() * len * 1.25);
        quad(&mut out, p, (p.0 + (ang - wid).cos() * len, p.1 + (ang - wid).sin() * len), tip, 5);
        quad(&mut out, tip, (p.0 + (ang + wid).cos() * len, p.1 + (ang + wid).sin() * len), p, 5);
        out
    }
}

impl Effect for Koi {
    fn frame(&mut self, c: &mut Cv, t: f32, dt: f32, p: &Pointer) {
        if self.r.f() < dt * 0.3 {
            let (x, y) = (self.r.f() * W, self.r.f() * H);
            self.rings.add(x, y, 60.);
        }
        self.rings.draw(c, dt);
        for f in &mut self.food {
            f.age += dt;
        }
        self.food.retain(|f| f.age <= 6.);
        let bits: Vec<Vec<Pt>> = self.food.iter().flat_map(|f| f.bits.iter().map(|&(dx, dy)| circ(f.x + dx, f.y + dy, 3.))).collect();
        c.fill(&bits, 0xd8b878, 1.);
        for fi in 0..self.fish.len() {
            let f = &mut self.fish[fi];
            let mut want = f.a + (t * 0.4 + f.seed).sin() * 0.9 + (t * 0.13 + f.seed * 3.).sin() * 0.9;
            let mut boost = 1.;
            let goal = (0..self.food.len()).min_by(|&a, &b| {
                let d = |g: &Food| (f.x - g.x).hypot(f.y - g.y);
                d(&self.food[a]).total_cmp(&d(&self.food[b]))
            });
            if let Some(g) = goal {
                want = (self.food[g].y - f.y).atan2(self.food[g].x - f.x);
                boost = 1.9;
            } else if p.on && near(f.x, f.y, p, 170.) {
                want = (f.y - p.y).atan2(f.x - p.x);
                boost = 3.;
            }
            let m = 110.;
            if f.x < m || f.x > W - m || f.y < m || f.y > H - m {
                want = (H / 2. - f.y).atan2(W / 2. - f.x);
            }
            let mut d = (want - f.a) % TAU;
            if d > PI {
                d -= TAU;
            } else if d < -PI {
                d += TAU;
            }
            f.a += (d * 1.6).clamp(-1.6, 1.6) * dt;
            let v = f.speed * boost * (1. + 0.22 * (t * 2.4 + f.seed).sin());
            f.x += f.a.cos() * v * dt;
            f.y += f.a.sin() * v * dt;
            if let Some(g) = goal {
                if (f.x - self.food[g].x).powi(2) + (f.y - self.food[g].y).powi(2) < 24. * 24. {
                    self.food[g].age += dt * 3.;
                }
            }
            f.pts[0] = (f.x, f.y);
            let gap = f.size * 0.95;
            for i in 1..f.pts.len() {
                let (a, b) = (f.pts[i - 1], f.pts[i]);
                let l = (b.0 - a.0).hypot(b.1 - a.1).max(1e-6);
                f.pts[i] = (a.0 + (b.0 - a.0) * gap / l, a.1 + (b.1 - a.1) * gap / l);
            }
            let f = &self.fish[fi];
            c.fill1(Self::body(f, 9., 13.), 0x000e10, 0.28);
            let n = f.pts.len();
            let tail = f.pts[n - 1];
            let back = (tail.1 - f.pts[8].1).atan2(tail.0 - f.pts[8].0);
            let wag = (t * 5. + f.seed).sin() * 0.5;
            let chest = f.pts[2];
            let head = (f.pts[1].1 - chest.1).atan2(f.pts[1].0 - chest.0);
            c.fill(&[Self::fin(tail, back + wag, f.size * 1.9, 0.55), Self::fin(chest, head + 2.2, f.size * 1.2, 0.4), Self::fin(chest, head - 2.2, f.size * 1.2, 0.4)], f.base, 0.85);
            c.fill1(Self::body(f, 0., 0.), f.base, 1.);
            let patches: Vec<Vec<Pt>> = [(1, 0.55), (3, 0.78), (5, 0.62), (7, 0.5)].iter().map(|&(i, k)| ell(f.pts[i].0, f.pts[i].1, f.size * k, f.size * k * 0.72, f.seed + i as f32, 20)).collect();
            c.fill(&patches, f.patch, 1.);
            let eye = f.a + FRAC_PI_2;
            let e = f.pts[0];
            let (ex, ey) = (eye.cos() * f.size * 0.45, eye.sin() * f.size * 0.45);
            c.dot(e.0 + ex, e.1 + ey, 2.2, 0x10131a, 1.);
            c.dot(e.0 - ex, e.1 - ey, 2.2, 0x10131a, 1.);
        }
        for (i, &(x, y, s, bloom)) in PADS.iter().enumerate() {
            let a = (t * 0.35 + i as f32).sin() * 0.06 + i as f32 * 1.3;
            let mut pad = vec![(x, y)];
            for k in 0..=28 {
                let b = a + 0.22 + (TAU - 0.44) * k as f32 / 28.;
                pad.push((x + b.cos() * s, y + b.sin() * s));
            }
            c.fill1(pad, if i % 3 != 0 { 0x2f7a4f } else { 0x3a8c58 }, 1.);
            let veins: Vec<Vec<Pt>> = (1..6).map(|k| {
                let b = a + k as f32 * TAU / 6.;
                vec![(x, y), (x + b.cos() * s * 0.86, y + b.sin() * s * 0.86)]
            }).collect();
            c.stroke(&veins, 1.5, false, 0xbeebbe, 0.3);
            if bloom {
                for (petals, rad, colour) in [(8, s * 0.42, 0xf4a8bf), (6, s * 0.3, 0xfbd6df), (5, s * 0.16, 0xffe9a8)] {
                    let list: Vec<Vec<Pt>> = (0..petals).map(|k| {
                        let b = k as f32 * TAU / petals as f32 + t * 0.05;
                        ell(x + b.cos() * rad * 0.6, y + b.sin() * rad * 0.6, rad * 0.55, rad * 0.28, b, 16)
                    }).collect();
                    c.fill(&list, colour, 1.);
                }
            }
        }
    }

    fn click(&mut self, x: f32, y: f32) {
        self.rings.add(x, y, 150.);
        let bits = (0..7).map(|_| ((self.r.f() - 0.5) * 44., (self.r.f() - 0.5) * 44.)).collect();
        self.food.push(Food { x, y, age: 0., bits });
        if self.food.len() > 3 {
            self.food.remove(0);
        }
    }
}

// ───────────────────────── Fireflies ─────────────────────────

struct Fly {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    ph: f32,
    sp: f32,
    wa: f32,
}

/// Fireflies blink and drift, come towards the pointer, and scatter at a click. The grass sways.
struct Fireflies {
    r: Rng,
    flies: Vec<Fly>,
    /// x, height, phase, foot.
    blades: Vec<[f32; 4]>,
}

impl Fireflies {
    fn new() -> Self {
        let mut r = Rng(23);
        let flies = (0..56)
            .map(|_| {
                let (x, y) = (r.f() * W, 300. + r.f() * 650.);
                let (ph, sp, wa) = (r.f() * TAU, 0.7 + r.f() * 1.3, r.f() * TAU);
                Fly { x, y, vx: 0., vy: 0., ph, sp, wa }
            })
            .collect();
        let blades = (0..150).map(|_| [r.f() * W, 26. + r.f() * 74., r.f() * TAU, 1004. - r.f() * 26.]).collect();
        Self { r, flies, blades }
    }
}

impl Effect for Fireflies {
    fn frame(&mut self, c: &mut Cv, t: f32, dt: f32, ptr: &Pointer) {
        let grass: Vec<Vec<Pt>> = self.blades.iter().map(|&[x, h, ph, y]| {
            let sway = (t * 1.1 + ph + x * 0.004).sin() * h * 0.24;
            let mut l = vec![(x, y)];
            quad(&mut l, (x, y), (x + sway * 0.3, y - h * 0.6), (x + sway, y - h), 4);
            l
        }).collect();
        c.stroke(&grass, 3., false, 0x05100f, 1.);
        for f in &mut self.flies {
            f.wa += (self.r.f() - 0.5) * 1.8 * dt * 4.;
            f.vx += f.wa.cos() * 22. * dt;
            f.vy += f.wa.sin() * 22. * dt;
            if ptr.on && near(f.x, f.y, ptr, 300.) {
                f.vx += (ptr.x - f.x) * 0.35 * dt;
                f.vy += (ptr.y - f.y) * 0.35 * dt;
            }
            f.vx *= 1. - 0.9 * dt;
            f.vy *= 1. - 0.9 * dt;
            f.x += f.vx * dt;
            f.y += f.vy * dt;
            if f.x < -20. {
                f.x = W + 20.;
            } else if f.x > W + 20. {
                f.x = -20.;
            }
            if f.y < 200. {
                f.vy += 30. * dt;
            } else if f.y > 960. {
                f.vy -= 30. * dt;
            }
            let k = (t * f.sp + f.ph).sin().max(0.).powi(2) * 0.75 + 0.25;
            for (rad, a, colour) in [(30., 0.1, 0xc9e65a), (15., 0.24, 0xd6ee66), (5., 1., 0xf6ffc0)] {
                c.dot(f.x, f.y, rad, colour, a * k);
            }
        }
    }

    fn click(&mut self, x: f32, y: f32) {
        for f in &mut self.flies {
            let d = (f.x - x).hypot(f.y - y);
            if d < 300. {
                f.vx += (f.x - x) / (d + 1.) * 150.;
                f.vy += (f.y - y) / (d + 1.) * 150.;
                f.ph = FRAC_PI_2 - f.sp;
            }
        }
    }
}

// ───────────────────────── Rain on glass ─────────────────────────

struct Run {
    x: f32,
    y: f32,
    s: f32,
    v: f32,
    wait: f32,
    wob: f32,
}

/// Rain on a window: drops gather, slide down, swallow the small ones and leave a trail. The pointer wipes the glass.
struct Rain {
    r: Rng,
    /// x, y, size.
    small: Vec<Pt3>,
    runs: Vec<Run>,
    /// x, y, speed, length.
    fall: Vec<[f32; 4]>,
}

type Pt3 = (f32, f32, f32);

impl Rain {
    fn new() -> Self {
        let mut r = Rng(31);
        let fall = (0..46).map(|_| [r.f() * W, r.f() * H, 1100. + r.f() * 700., 30. + r.f() * 50.]).collect();
        let small = (0..260).map(|_| (r.f() * W, r.f() * H, 1.4 + r.f() * 3.)).collect();
        Self { r, small, runs: Vec::new(), fall }
    }
}

impl Effect for Rain {
    fn frame(&mut self, c: &mut Cv, t: f32, dt: f32, p: &Pointer) {
        let r = &mut self.r;
        let mut lines = Vec::new();
        for f in &mut self.fall {
            f[1] += f[2] * dt;
            f[0] -= f[2] * dt * 0.08;
            if f[1] > H + 40. {
                f[1] = -f[3];
                f[0] = r.f() * (W + 100.);
            }
            lines.push(vec![(f[0], f[1]), (f[0] - f[3] * 0.08, f[1] + f[3])]);
        }
        c.stroke(&lines, 1.5, false, 0xbed2f0, 0.13);
        for _ in 0..5 {
            if self.small.len() < 340 && r.f() < dt * 7. {
                self.small.push((r.f() * W, r.f() * H, 1.4 + r.f() * 3.));
            }
        }
        if self.runs.len() < 15 && r.f() < dt * 1.2 {
            let (x, y, s) = (r.f() * W, -20. - r.f() * 200., 7. + r.f() * 6.);
            self.runs.push(Run { x, y, s, v: 40., wait: 0., wob: r.f() * TAU });
        }
        let mut i = self.runs.len();
        while i > 0 {
            i -= 1;
            let d = &mut self.runs[i];
            d.wait -= dt;
            if d.wait > 0. {
                continue;
            }
            if r.f() < dt * 0.7 {
                d.wait = r.f() * 1.1;
            }
            d.v = (d.v + 500. * dt).min(420.);
            if d.wait > 0. {
                d.v = 30.;
            }
            d.y += d.v * dt;
            d.x += (t * 2.2 + d.wob).sin() * 22. * dt;
            let (dx, dy) = (d.x, d.y);
            let mut grew = 0;
            let ds = d.s;
            self.small.retain(|q| {
                let hit = (q.0 - dx).abs() < ds + q.2 && (q.1 - dy).abs() < ds * 1.6 + q.2;
                grew += hit as usize;
                !hit
            });
            let d = &mut self.runs[i];
            d.s = (d.s + 0.03 * grew as f32).min(13.);
            if self.small.len() < 340 && r.f() < dt * 16. {
                let q = (d.x + (r.f() - 0.5) * 3., d.y - d.s * 2., 0.9 + r.f() * 1.5);
                self.small.push(q);
            }
            if self.runs[i].y > H + 30. {
                self.runs.remove(i);
            }
        }
        if p.on {
            self.small.retain(|q| !near(q.0, q.1, p, 60.));
            self.runs.retain(|d| !near(d.x, d.y, p, 50.));
        }
        let drops: Vec<Vec<Pt>> = self.small.iter().map(|q| ell(q.0, q.1, q.2, q.2, 0., 10)).collect();
        c.fill(&drops, 0xcddefa, 0.5);
        let shade: Vec<Vec<Pt>> = self.runs.iter().map(|d| ell(d.x, d.y, d.s, d.s * 1.35, 0., 18)).collect();
        c.fill(&shade, 0x141e34, 0.55);
        let light: Vec<Vec<Pt>> = self.runs.iter().map(|d| ell(d.x, d.y + d.s * 0.55, d.s * 0.55, d.s * 0.45, 0., 14)).collect();
        c.fill(&light, 0xdceaff, 0.75);
        let shine: Vec<Vec<Pt>> = self.runs.iter().map(|d| ell(d.x - d.s * 0.35, d.y - d.s * 0.55, d.s * 0.22, d.s * 0.22, 0., 8)).collect();
        c.fill(&shine, 0xffffff, 0.85);
    }

    fn click(&mut self, x: f32, y: f32) {
        for _ in 0..4 {
            let r = &mut self.r;
            let (rx, ry, s, wob) = ((r.f() - 0.5) * 90., (r.f() - 0.5) * 40., 5. + r.f() * 5., r.f() * TAU);
            self.runs.push(Run { x: x + rx, y: y + ry, s, v: 60., wait: 0., wob });
        }
    }
}

// ───────────────────────── Block world ─────────────────────────

const G: f32 = 40.;

fn ground(x: f32) -> f32 {
    let c = (x / G).floor();
    640. - G * (2.4 * (c * 0.33).sin() + 1.5 * (c * 0.11 + 1.).sin() + 0.5).floor()
}

struct Cloud {
    x: f32,
    y: f32,
    sp: f32,
    parts: Vec<Pt3>,
}

struct Folk {
    x: f32,
    dir: f32,
    shirt: u32,
    legs: u32,
    y: f32,
    vy: f32,
    wait: f32,
    sp: f32,
}

struct Block {
    col: i32,
    y: f32,
    vy: f32,
    colour: u32,
    rest: bool,
    age: f32,
}

/// A world of blocks: clouds drift, the windmill turns, little block people hop along, and a click drops a block.
struct Blocks {
    r: Rng,
    clouds: Vec<Cloud>,
    folks: Vec<Folk>,
    blocks: Vec<Block>,
}

impl Blocks {
    fn new() -> Self {
        let mut r = Rng(41);
        let clouds = (0..9)
            .map(|i| {
                let x = r.f() * (W + 400.) - 200.;
                let y = 40. + (i % 5) as f32 * 62. + r.f() * 26.;
                let sp = 9. + r.f() * 14.;
                let n = 3 + (r.f() * 3.) as usize;
                let parts = (0..n).map(|k| {
                    let dy = (r.f() * 2.).floor() * -G * 0.6;
                    let s = G * (1.6 + (r.f() * 2.).floor() * 0.8);
                    (k as f32 * G * 1.6, dy, s)
                }).collect();
                Cloud { x, y, sp, parts }
            })
            .collect();
        let looks = [(0x3b82f6, 0x22458f), (0xef4444, 0x8f2222), (0x22c55e, 0x176f38), (0xa855f7, 0x5d2a8f)];
        let folks = looks
            .iter()
            .enumerate()
            .map(|(i, &(shirt, legs))| {
                let wait = r.f() * 2.;
                let sp = 38. + r.f() * 26.;
                Folk { x: 340. + i as f32 * 330., dir: if i % 2 == 1 { -1. } else { 1. }, shirt, legs, y: 0., vy: 0., wait, sp }
            })
            .collect();
        Self { r, clouds, folks, blocks: Vec::new() }
    }

    fn stacked(&self, col: i32) -> f32 {
        self.blocks.iter().filter(|b| b.col == col && b.rest).count() as f32
    }
}

impl Effect for Blocks {
    fn frame(&mut self, c: &mut Cv, t: f32, dt: f32, _: &Pointer) {
        let (mut white, mut under) = (Vec::new(), Vec::new());
        for k in &mut self.clouds {
            k.x += k.sp * dt;
            if k.x > W + 60. {
                k.x = -420.;
            }
            for &(dx, dy, s) in &k.parts {
                white.push(rect_pts(k.x + dx, k.y + dy, s * 1.5, s * 0.7));
                under.push(rect_pts(k.x + dx, k.y + dy + s * 0.7, s * 1.5, s * 0.16));
            }
        }
        c.fill(&white, 0xffffff, 0.92);
        c.fill(&under, 0xbedcf0, 0.9);
        let (mx, my, spin) = (1218., ground(1218.) - 200., t * 0.9);
        for i in 0..4 {
            let rot = spin + i as f32 * TAU / 4.;
            c.fill1(place(&rect_pts(10., -14., 118., 28.), (mx, my), rot, 1.), 0xfff6e0, 1.);
            c.fill1(place(&rect_pts(0., -5., 124., 10.), (mx, my), rot, 1.), 0xd9503f, 1.);
        }
        c.rect(mx - 11., my - 11., 22., 22., 0x6b4a2e, 1.);
        for i in 0..self.blocks.len() {
            if !self.blocks[i].rest {
                let col = self.blocks[i].col;
                let floor = ground(col as f32 * G) - G * (self.stacked(col) + 1.);
                let b = &mut self.blocks[i];
                b.vy += 1500. * dt;
                b.y += b.vy * dt;
                if b.y >= floor {
                    b.y = floor;
                    if b.vy > 240. {
                        b.vy *= -0.28;
                    } else {
                        b.rest = true;
                        b.age = 0.;
                    }
                }
            } else {
                self.blocks[i].age += dt;
            }
        }
        self.blocks.retain(|b| !(b.rest && b.age > 14.));
        for b in &self.blocks {
            let a = if b.rest && b.age > 13. { 14. - b.age } else { 1. };
            let (x, y) = (b.col as f32 * G, b.y);
            c.rect(x, y, G, G, b.colour, a);
            c.rect(x, y, G, G * 0.22, 0xffffff, 0.3 * a);
            c.rect(x + G * 0.8, y + G * 0.22, G * 0.2, G * 0.78, 0x000000, 0.18 * a);
        }
        for fi in 0..self.folks.len() {
            let fx = self.folks[fi].x;
            let floor = ground(fx) - G * self.stacked((fx / G).floor() as i32);
            let dir = self.folks[fi].dir;
            let ahead = fx + dir * 12.;
            let next = ground(ahead) - G * self.stacked((ahead / G).floor() as i32);
            let r = &mut self.r;
            let f = &mut self.folks[fi];
            if f.y == 0. {
                f.y = floor;
            }
            f.wait -= dt;
            if f.wait <= 0. {
                f.x += f.dir * f.sp * dt;
                if f.x < 330. || f.x > W - 60. || r.f() < dt * 0.08 {
                    f.dir *= -1.;
                }
                if r.f() < dt * 0.12 {
                    f.wait = 0.8 + r.f() * 2.2;
                }
            }
            if f.y >= floor - 1. && next < floor - 1. {
                f.vy = -(2. * 1500. * (floor - next + 16.)).sqrt();
            }
            f.vy += 1500. * dt;
            f.y += f.vy * dt;
            if f.y >= floor {
                f.y = floor;
                f.vy = 0.;
            }
            let step = if f.wait > 0. || f.vy != 0. { 0. } else { (t * 11. + f.x).sin() * 4. };
            let (x, y, d) = (f.x, f.y, f.dir);
            c.rect(x - 10. + step, y - 15., 9., 15., f.legs, 1.);
            c.rect(x + 1. - step, y - 15., 9., 15., f.legs, 1.);
            c.rect(x - 12., y - 38., 24., 24., f.shirt, 1.);
            c.rect(x - 19., y - 36., 7., 18., 0xf7c948, 1.);
            c.rect(x + 12., y - 36., 7., 18., 0xf7c948, 1.);
            c.rect(x - 12., y - 60., 24., 22., 0xf7c948, 1.);
            c.rect(x + d * 3. - 5., y - 54., 3., 4., 0x232323, 1.);
            c.rect(x + d * 3. + 2., y - 54., 3., 4., 0x232323, 1.);
            c.rect(x + d * 3. - 4., y - 46., 8., 2., 0x232323, 1.);
        }
    }

    fn click(&mut self, x: f32, y: f32) {
        if self.blocks.len() > 24 {
            self.blocks.remove(0);
        }
        let colour = self.r.pick(&[0xe8b23a, 0xd9503f, 0x4f8fe0, 0x8d5fd3, 0xf08a3c, 0x35b58a]);
        self.blocks.push(Block { col: (x / G).floor() as i32, y: y.min(300.) - G, vy: 0., colour, rest: false, age: 0. });
    }
}

// ───────────────────────── Peach blossom ─────────────────────────

struct Petal {
    x: f32,
    y: f32,
    z: f32,
    rot: f32,
    spin: f32,
    ph: f32,
    flip: f32,
    colour: u32,
    push: f32,
    push_y: f32,
    extra: bool,
}

const PEACH: [u32; 5] = [0xf7b6c8, 0xf49ab4, 0xfbd3de, 0xee84a4, 0xfde4ea];

/// Peach blossom in the Tết wind: petals tumble in gusts, the pointer stirs them, a click shakes more loose.
struct Petals {
    r: Rng,
    list: Vec<Petal>,
}

impl Petals {
    fn petal(r: &mut Rng) -> Petal {
        let x = r.f() * (W + 300.) - 300.;
        let y = r.f() * H - H * 0.1;
        let z = 0.55 + r.f() * 0.9;
        let rot = r.f() * TAU;
        let spin = (r.f() - 0.5) * 2.6;
        let ph = r.f() * TAU;
        let flip = 1.2 + r.f() * 2.2;
        let colour = r.pick(&PEACH);
        Petal { x, y, z, rot, spin, ph, flip, colour, push: 0., push_y: 0., extra: false }
    }

    fn new() -> Self {
        let mut r = Rng(53);
        let list = (0..80).map(|_| Self::petal(&mut r)).collect();
        Self { r, list }
    }
}

impl Effect for Petals {
    fn frame(&mut self, c: &mut Cv, t: f32, dt: f32, p: &Pointer) {
        let wind = 46. + 40. * (t * 0.21).sin() + 34. * (t * 0.67 + 1.).sin().max(0.).powi(4) * 3.;
        let many = self.list.len() > 80;
        let r = &mut self.r;
        let mut gone = Vec::new();
        let mut by_colour: Vec<(u32, Vec<Vec<Pt>>)> = PEACH.iter().map(|&h| (h, Vec::new())).collect();
        for (i, q) in self.list.iter_mut().enumerate() {
            if p.on && near(q.x, q.y, p, 130.) {
                q.push += (q.x - p.x) * dt * 6.;
                q.push_y += (q.y - p.y) * dt * 6.;
            }
            q.push *= 1. - 1.6 * dt;
            q.push_y *= 1. - 1.6 * dt;
            q.x += (wind * q.z + q.push + (t + q.ph).sin() * 16.) * dt;
            q.y += (34. * q.z + 16. + q.push_y) * dt;
            q.rot += q.spin * dt;
            if q.y > H + 20. || q.x > W + 30. {
                q.x = r.f() * (W + 200.) - 400.;
                q.y = -20. - r.f() * 120.;
                if many && q.extra {
                    gone.push(i);
                }
            }
            let s = q.z * 9.;
            let flat = (t * q.flip + q.ph).cos();
            let (cs, sn) = (q.rot.cos(), q.rot.sin());
            let mut shape = vec![(0., -s)];
            cubic(&mut shape, (0., -s), (s * 1.1, -s * 0.6), (s * 0.9, s * 0.7), (0., s), 6);
            cubic(&mut shape, (0., s), (-s * 0.9, s * 0.7), (-s * 1.1, -s * 0.6), (0., -s), 6);
            // (The page's transform: x' = cos·flat·u − sin·v, y' = sin·flat·u + cos·v.)
            let pts = shape.iter().map(|&(u, v)| (q.x + cs * flat * u - sn * v, q.y + sn * flat * u + cs * v)).collect();
            if let Some(b) = by_colour.iter_mut().find(|b| b.0 == q.colour) {
                b.1.push(pts);
            }
        }
        for i in gone.into_iter().rev() {
            self.list.remove(i);
        }
        for (hex, list) in &by_colour {
            c.fill(list, *hex, 1.);
        }
    }

    fn click(&mut self, x: f32, y: f32) {
        if self.list.len() > 130 {
            return;
        }
        for _ in 0..16 {
            let mut q = Self::petal(&mut self.r);
            let r = &mut self.r;
            q.x = x + (r.f() - 0.5) * 60.;
            q.y = y + (r.f() - 0.5) * 40.;
            q.push = (r.f() - 0.5) * 360.;
            q.push_y = (r.f() - 0.7) * 240.;
            q.extra = true;
            self.list.push(q);
        }
    }
}

// ───────────────────────── Starry night ─────────────────────────

const STAR_COLOURS: [u32; 8] = [0x17357f, 0x1f4aa0, 0x2a5bb8, 0x3a6cc4, 0x6b9be0, 0xbcd8f2, 0xf2e8a0, 0xf7d648];
const STARS: [Pt3; 9] = [(180., 90., 20.), (430., 80., 16.), (790., 70., 18.), (1020., 110., 15.), (1270., 330., 18.), (1480., 400., 16.), (110., 340., 16.), (520., 450., 14.), (1110., 460., 15.)];

struct Dab {
    x: f32,
    y: f32,
    c: usize,
    life: f32,
}

/// Starry night: the sky's strokes flow round its swirls, the stars breathe, and the pointer stirs a swirl of its own.
struct Starry {
    r: Rng,
    swirls: Vec<Pt3>,
    dots: Vec<Dab>,
    village: Vec<Pt>,
    cypress: Vec<Pt>,
}

impl Starry {
    fn tone(r: &mut Rng, x: f32, y: f32) -> usize {
        // Yellow and white only round the stars and the moon; blue everywhere else.
        if STARS.iter().any(|&(sx, sy, s)| (x - sx).hypot(y - sy) < s * 4.5) || (x - 1400.).hypot(y - 120.) < 130. {
            5 + (r.f() * 3.) as usize
        } else if r.f() < 0.07 {
            5
        } else {
            (r.f() * 5.) as usize
        }
    }

    fn new() -> Self {
        let mut r = Rng(61);
        let dots = (0..900)
            .map(|_| {
                let (x, y) = (r.f() * W, r.f() * 660.);
                let c = Self::tone(&mut r, x, y);
                Dab { x, y, c, life: r.f() * 5. }
            })
            .collect();
        // M0 1000 L0 700 Q200 640 420 690 Q640 730 860 680 Q1120 630 1600 700 L1600 1000 Z
        let mut village = vec![(0., 1000.), (0., 700.)];
        quad(&mut village, (0., 700.), (200., 640.), (420., 690.), 10);
        quad(&mut village, (420., 690.), (640., 730.), (860., 680.), 10);
        quad(&mut village, (860., 680.), (1120., 630.), (1600., 700.), 14);
        village.push((1600., 1000.));
        // M430 1000 Q360 800 420 620 Q436 500 468 300 Q470 240 480 210 Q492 250 500 330 Q548 520 556 660 Q588 820 540 1000 Z
        let mut cypress = vec![(430., 1000.)];
        for (from, ctrl, to) in [((430., 1000.), (360., 800.), (420., 620.)), ((420., 620.), (436., 500.), (468., 300.)), ((468., 300.), (470., 240.), (480., 210.)), ((480., 210.), (492., 250.), (500., 330.)), ((500., 330.), (548., 520.), (556., 660.)), ((556., 660.), (588., 820.), (540., 1000.))] {
            quad(&mut cypress, from, ctrl, to, 8);
        }
        Self { r, swirls: vec![(620., 250., 1.5), (930., 330., -1.1), (300., 170., -0.8), (1180., 170., 0.8)], dots, village, cypress }
    }

    fn flow(&self, x: f32, y: f32, p: &Pointer) -> Pt {
        let (mut u, mut v) = (1., 0.22 * (x * 0.006 + y * 0.012).sin());
        let mut swirl = |cx: f32, cy: f32, s: f32, rad: f32| {
            let (dx, dy) = (x - cx, y - cy);
            let d = dx * dx + dy * dy + 900.;
            let k = s * rad * rad / (d + rad * rad);
            u += -dy / d.sqrt() * k * 2.4;
            v += dx / d.sqrt() * k * 2.4;
        };
        for &(cx, cy, s) in &self.swirls {
            swirl(cx, cy, s, 170.);
        }
        if p.on {
            swirl(p.x, p.y, 1.8, 130.);
        }
        let l = u.hypot(v).max(1e-6);
        (u / l, v / l)
    }
}

impl Effect for Starry {
    fn frame(&mut self, c: &mut Cv, t: f32, dt: f32, p: &Pointer) {
        let mut paths: Vec<Vec<Vec<Pt>>> = vec![Vec::new(); STAR_COLOURS.len()];
        for i in 0..self.dots.len() {
            let (x, y) = (self.dots[i].x, self.dots[i].y);
            let (u, v) = self.flow(x, y, p);
            let d = &mut self.dots[i];
            d.x += u * 42. * dt;
            d.y += v * 42. * dt;
            d.life -= dt;
            if d.life < 0. || d.x > W + 20. || d.x < -20. || d.y < -20. || d.y > 700. {
                d.x = self.r.f() * W;
                d.y = self.r.f() * 660.;
                d.c = Self::tone(&mut self.r, d.x, d.y);
                d.life = 3. + self.r.f() * 5.;
            }
            paths[d.c].push(vec![(d.x - u * 26., d.y - v * 26.), (d.x, d.y)]);
        }
        for (i, lines) in paths.iter().enumerate() {
            c.stroke(lines, 7., false, STAR_COLOURS[i], 0.82);
        }
        for (i, &(x, y, s)) in STARS.iter().enumerate() {
            let k = 1. + 0.14 * (t * 1.3 + i as f32 * 1.7).sin();
            for (f, colour) in [(3.1, 0x6f93c4), (2.3, 0xd8c85e), (1.5, 0xf4e27a)] {
                let rad = s * f * k;
                // (The page's dash offset, as the ring's start turned along it.)
                let turn = t * 12. * f / rad;
                let ring: Vec<Pt> = (0..=48).map(|n| {
                    let a = turn + n as f32 * TAU / 48.;
                    (x + a.cos() * rad, y + a.sin() * rad)
                }).collect();
                c.stroke_dashed(&[ring], 5., false, Some((11., 8.)), colour, 1.);
            }
            c.fill1(circ(x, y, s * 0.7), 0xfff6c2, 1.);
        }
        c.fill1(circ(1400., 120., 62.), 0xf7d648, 1.);
        c.fill1(circ(1376., 104., 54.), 0x17357f, 1.);
        c.fill1(self.village.clone(), 0x16264a, 1.);
        c.fill1(self.cypress.clone(), 0x10201c, 1.);
    }

    fn click(&mut self, x: f32, y: f32) {
        let s = if self.r.f() < 0.5 { 1.2 } else { -1.2 };
        self.swirls.push((x, y, s));
        if self.swirls.len() > 7 {
            self.swirls.remove(4);
        }
    }
}

// ───────────────────────── The great wave ─────────────────────────

struct Spray {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    s: f32,
}

/// The great wave, cut in wood: spray flies off the crest, the swell rolls, boats ride it and clouds pass.
struct Wave {
    r: Rng,
    spray: Vec<Spray>,
}

const CREST: [Pt; 7] = [(640., 112.), (720., 118.), (800., 150.), (860., 190.), (905., 244.), (560., 150.), (470., 210.)];

impl Wave {
    fn new() -> Self {
        Self { r: Rng(71), spray: Vec::new() }
    }

    fn band(c: &mut Cv, y: f32, amp: f32, len: f32, sp: f32, t: f32, colour: u32, crest: bool) {
        let line: Vec<Pt> = (0..)
            .map(|i| -10. + i as f32 * 30.)
            .take_while(|&x| x <= W + 20.)
            .map(|x| (x, y + (x / len + t * sp).sin() * amp + (x / (len * 0.43) - t * sp * 1.4).sin() * amp * 0.35))
            .collect();
        let mut shape = vec![(-10., H + 10.)];
        shape.extend(&line);
        shape.push((W + 20., H + 10.));
        c.fill1(shape, colour, 1.);
        if crest {
            c.stroke(&[line], 3.5, false, 0xf4efe0, 1.);
        }
    }
}

impl Effect for Wave {
    fn frame(&mut self, c: &mut Cv, t: f32, dt: f32, _: &Pointer) {
        let mut clouds = Vec::new();
        for (cx, cy, w) in [(200., 90., 300.), (900., 60., 240.), (1300., 180., 330.)] {
            let x = (cx + t * 11.) % (W + 500.) - 300.;
            clouds.push(rect_pts(x, cy, w, 13.));
            clouds.push(rect_pts(x + 60., cy - 13., w * 0.6, 13.));
            clouds.push(rect_pts(x + 110., cy + 13., w * 0.55, 9.));
        }
        c.fill(&clouds, 0xf3e9d0, 1.);
        if self.spray.len() < 130 {
            for (x, y) in CREST {
                let r = &mut self.r;
                if r.f() < dt * 5. {
                    let (dx, dy, vx, vy, s) = ((r.f() - 0.5) * 40., (r.f() - 0.5) * 20., 40. + r.f() * 120., -60. - r.f() * 120., 2. + r.f() * 5.);
                    self.spray.push(Spray { x: x + dx, y: y + dy, vx, vy, s });
                }
            }
        }
        for q in &mut self.spray {
            q.vy += 200. * dt;
            q.x += q.vx * dt;
            q.y += q.vy * dt;
        }
        self.spray.retain(|q| q.y <= 760.);
        let drops: Vec<Vec<Pt>> = self.spray.iter().map(|q| ell(q.x, q.y, q.s, q.s, 0., 10)).collect();
        c.fill(&drops, 0xfbf8ee, 1.);
        Self::band(c, 800., 16., 190., 0.55, t, 0x2a6a9c, true);
        for (i, &(x, y, s)) in [(1050., 840., 1.), (1320., 900., 0.85), (760., 925., 0.9)].iter().enumerate() {
            let bob = (t * 0.9 + i as f32 * 2.).sin() * 13.;
            let tilt = (t * 0.7 + i as f32).sin() * 0.07;
            let at = (x, y - 50. + bob);
            let mut hull = vec![(-120., -6.)];
            quad(&mut hull, (-120., -6.), (0., 22.), (130., -14.), 10);
            quad(&mut hull, (130., -14.), (0., 42.), (-120., -6.), 10);
            c.fill1(place(&hull, at, tilt, s), 0xe8dcb8, 1.);
            let rowers: Vec<Vec<Pt>> = (0..7).map(|k| place(&rect_pts(-84. + k as f32 * 26., -8. - (k as f32 - 3.).abs(), 9., 10.), at, tilt, s)).collect();
            c.fill(&rowers, 0x1b3350, 1.);
        }
        Self::band(c, 860., 20., 150., 0.75, t + 2., 0x174a7e, true);
        Self::band(c, 930., 13., 120., 0.95, t + 5., 0x0f2f57, false);
    }

    fn click(&mut self, x: f32, y: f32) {
        for _ in 0..26 {
            let r = &mut self.r;
            let (a, v) = (r.f() * TAU, 80. + r.f() * 230.);
            let s = 2. + r.f() * 5.;
            self.spray.push(Spray { x, y, vx: a.cos() * v, vy: a.sin() * v - 80., s });
        }
    }
}

// ───────────────────────── Water lilies ─────────────────────────

const POND: [u32; 8] = [0x6e97b8, 0x8cb2c8, 0xa79ac8, 0x5e8f90, 0xb9d0d6, 0x7fa88e, 0xc8b4d8, 0x4c7d8e];
const LILIES: [(f32, f32, f32, u32); 7] = [(420., 300., 34., 0xf6c2d2), (1020., 240., 30., 0xfbf3ea), (1300., 600., 38., 0xf4a8bf), (640., 690., 36., 0xfbf3ea), (240., 640., 28., 0xf4a8bf), (860., 500., 26., 0xf9d5df), (1500., 330., 28., 0xfbf3ea)];

/// Water lilies in dabs of paint: the pond shimmers and sways, and a click sends rings across it.
struct Lilies {
    rings: Ripples,
    /// Per colour: x, y, w, h, phase.
    dabs: Vec<Vec<[f32; 5]>>,
}

impl Lilies {
    fn new() -> Self {
        let mut r = Rng(83);
        let dabs = POND.iter().map(|_| (0..80).map(|_| [r.f() * W, r.f() * H, 22. + r.f() * 36., 4. + r.f() * 5., r.f() * TAU]).collect()).collect();
        Self { rings: Ripples::new(0xf5f5ff, 0.9), dabs }
    }
}

impl Effect for Lilies {
    fn frame(&mut self, c: &mut Cv, t: f32, dt: f32, _: &Pointer) {
        for (g, group) in self.dabs.iter().enumerate() {
            let list: Vec<Vec<Pt>> = group.iter().map(|&[x, y, w, h, ph]| ell(x + (t * 0.6 + ph).sin() * 14., y + (t * 0.4 + ph * 2.).sin() * 3., w, h, 0., 14)).collect();
            c.fill(&list, POND[g], 0.42 + 0.3 * (t * 0.9 + g as f32 * 0.8).sin());
        }
        self.rings.draw(c, dt);
        for (i, &(x, y, s, colour)) in LILIES.iter().enumerate() {
            let b = y + (t * 0.7 + i as f32).sin() * 3.;
            let petals: Vec<Vec<Pt>> = (0..9).map(|k| {
                let a = k as f32 * TAU / 9. + (t * 0.3 + i as f32).sin() * 0.05;
                ell(x + a.cos() * s * 0.55, b + a.sin() * s * 0.32, s * 0.55, s * 0.2, a, 16)
            }).collect();
            c.fill(&petals, colour, 1.);
            c.fill1(ell(x, b - s * 0.08, s * 0.24, s * 0.15, 0., 16), 0xf2cf63, 1.);
        }
    }

    fn click(&mut self, x: f32, y: f32) {
        self.rings.add(x, y, 220.);
    }
}

// ───────────────────────── Cubism ─────────────────────────

struct Plane {
    p: [Pt; 3],
    x: f32,
    y: f32,
    ph: f32,
    sp: f32,
    lit: f32,
}

/// Cubism, an original picture: its planes catch and lose the light, and turn bright under the pointer.
struct Cubism {
    tris: Vec<Plane>,
}

impl Cubism {
    fn new() -> Self {
        let mut r = Rng(97);
        let (nx, ny) = (15usize, 10usize);
        let mut pts = Vec::new();
        for j in 0..=ny {
            for i in 0..=nx {
                let (dx, dy) = ((r.f() - 0.5) * 66., (r.f() - 0.5) * 66.);
                pts.push((i as f32 * W / nx as f32 + dx, j as f32 * H / ny as f32 + dy));
            }
        }
        let mut tris = Vec::new();
        for j in 0..ny {
            for i in 0..nx {
                let a = j * (nx + 1) + i;
                let (b, d) = (a + 1, a + nx + 1);
                let e = d + 1;
                let two = if (i + j) % 2 == 1 { [[a, b, e], [a, e, d]] } else { [[a, b, d], [b, e, d]] };
                for t in two {
                    let p = [pts[t[0]], pts[t[1]], pts[t[2]]];
                    let (ph, sp) = (r.f() * TAU, 0.25 + r.f() * 0.5);
                    tris.push(Plane { p, x: (p[0].0 + p[1].0 + p[2].0) / 3., y: (p[0].1 + p[1].1 + p[2].1) / 3., ph, sp, lit: 0. });
                }
            }
        }
        Self { tris }
    }
}

impl Effect for Cubism {
    fn frame(&mut self, c: &mut Cv, t: f32, dt: f32, p: &Pointer) {
        for q in &mut self.tris {
            let target = if p.on && near(q.x, q.y, p, 190.) { 1. } else { 0. };
            q.lit += (target - q.lit) * (dt * 5.).min(1.);
            let k = (t * q.sp + q.ph).sin() * 0.17 + q.lit * 0.3;
            let poly = q.p.to_vec();
            if k > 0. {
                c.fill1(poly, 0xffecbe, k);
            } else {
                c.fill1(poly, 0x0e0a08, -k);
            }
        }
    }

    fn click(&mut self, x: f32, y: f32) {
        for q in &mut self.tris {
            let d = (q.x - x).hypot(q.y - y);
            if d < 340. {
                q.lit = 1.6 - d / 340.;
            }
        }
    }
}

// ───────────────────────── Colour blocks ─────────────────────────

const XS: [f32; 6] = [120., 340., 620., 980., 1210., 1430.];
const YS: [f32; 5] = [110., 330., 560., 760., 900.];
const PRIMARY: [u32; 4] = [0xd42a20, 0x1f4fa8, 0xf2c81e, 0xf6f1e3];

struct Runner {
    flat: bool,
    i: usize,
    at: f32,
    dir: f32,
    colour: u32,
    sp: f32,
    next: f32,
}

/// Colour blocks and black lines: small squares run along the lines, and the blocks change colour by themselves or at a
/// click.
struct BlocksArt {
    r: Rng,
    runners: Vec<Runner>,
    /// Column, row, colour, age.
    cells: Vec<(usize, usize, u32, f32)>,
}

impl BlocksArt {
    fn new() -> Self {
        let mut r = Rng(103);
        let runners = (0..34)
            .map(|_| {
                let flat = r.f() < 0.5;
                let i = (r.f() * if flat { YS.len() } else { XS.len() } as f32) as usize;
                let at = r.f() * if flat { W } else { H };
                let dir = if r.f() < 0.5 { 1. } else { -1. };
                let colour = r.pick(&PRIMARY);
                let sp = 70. + r.f() * 110.;
                Runner { flat, i, at, dir, colour, sp, next: 0. }
            })
            .collect();
        Self { r, runners, cells: Vec::new() }
    }
}

impl Effect for BlocksArt {
    fn frame(&mut self, c: &mut Cv, _t: f32, dt: f32, _: &Pointer) {
        let r = &mut self.r;
        if r.f() < dt * 0.35 {
            let (i, j) = ((r.f() * (XS.len() - 1) as f32) as usize, (r.f() * (YS.len() - 1) as f32) as usize);
            let colour = r.pick(&PRIMARY);
            self.cells.push((i, j, colour, 0.));
        }
        for q in &mut self.cells {
            q.3 += dt;
        }
        self.cells.retain(|q| q.3 <= 7.);
        for &(i, j, colour, age) in &self.cells {
            c.rect(XS[i] + 5., YS[j] + 5., XS[i + 1] - XS[i] - 10., YS[j + 1] - YS[j] - 10., colour, (age * 1.2).min((7. - age) * 1.2).min(1.));
        }
        for q in &mut self.runners {
            q.at += q.dir * q.sp * dt;
            let end = if q.flat { W } else { H };
            if q.at < 0. {
                q.at = end;
            } else if q.at > end {
                q.at = 0.;
            }
            q.next -= dt;
            if q.next <= 0. && r.f() < 0.5 {
                let cross: &[f32] = if q.flat { &XS } else { &YS };
                if let Some(k) = cross.iter().position(|v| (v - q.at).abs() < 6.) {
                    let line = if q.flat { YS[q.i] } else { XS[q.i] };
                    q.flat = !q.flat;
                    q.i = k;
                    q.at = line;
                    q.next = 1.2;
                    q.dir = if r.f() < 0.5 { 1. } else { -1. };
                }
            }
            let line = if q.flat { YS[q.i] } else { XS[q.i] };
            let (x, y) = if q.flat { (q.at, line) } else { (line, q.at) };
            c.rect(x - 8., y - 8., 16., 16., if q.colour == 0xf6f1e3 { 0xffffff } else { q.colour }, 1.);
        }
    }

    fn click(&mut self, x: f32, y: f32) {
        let i = (0..XS.len() - 1).find(|&k| x >= XS[k] && x < XS[k + 1]);
        let j = (0..YS.len() - 1).find(|&k| y >= YS[k] && y < YS[k + 1]);
        if let (Some(i), Some(j)) = (i, j) {
            let colour = self.r.pick(&PRIMARY);
            self.cells.push((i, j, colour, 0.));
        }
    }
}

// ───────────────────────── Gold leaf ─────────────────────────

const SPIRALS: [Pt3; 7] = [(420., 240., 96.), (790., 640., 130.), (1180., 280., 110.), (1420., 720., 84.), (180., 760., 76.), (980., 860., 66.), (620., 90., 54.)];

/// Gold leaf: a sheen sweeps the tiles, they catch the pointer's light, and the spirals turn.
struct Gold {
    /// x, y, size, phase, speed.
    tiles: Vec<[f32; 5]>,
    /// x, y, radius.
    flash: Option<Pt3>,
}

impl Gold {
    fn new() -> Self {
        let mut r = Rng(109);
        let s = 34.;
        let mut tiles = Vec::new();
        let mut y = 0.;
        while y < H {
            let mut x = 0.;
            while x < W {
                if r.f() < 0.42 {
                    let size = s - 4. - (r.f() * 3.).floor() * 4.;
                    let (ph, sp) = (r.f() * TAU, 0.6 + r.f() * 1.2);
                    tiles.push([x + 2., y + 2., size, ph, sp]);
                }
                x += s;
            }
            y += s;
        }
        Self { tiles, flash: None }
    }
}

impl Effect for Gold {
    fn frame(&mut self, c: &mut Cv, t: f32, dt: f32, p: &Pointer) {
        let sweep = (t * 170.) % (W + H + 900.) - 450.;
        let mut bins: [Vec<Vec<Pt>>; 3] = [Vec::new(), Vec::new(), Vec::new()];
        for &[x, y, s, ph, sp] in &self.tiles {
            let mut k = ((t * sp + ph).sin() + 1.) * 0.22;
            let band = (x + y - sweep).abs();
            if band < 240. {
                k += (1. - band / 240.) * 0.8;
            }
            if p.on && near(x, y, p, 170.) {
                k += 0.7;
            }
            if let Some((fx, fy, fr)) = self.flash {
                let d = ((x - fx).hypot(y - fy) - fr).abs();
                if d < 70. {
                    k += 1. - d / 70.;
                }
            }
            if k > 0.12 {
                bins[if k > 0.9 { 2 } else if k > 0.45 { 1 } else { 0 }].push(rect_pts(x, y, s, s));
            }
        }
        if let Some(f) = &mut self.flash {
            f.2 += 620. * dt;
            if f.2 > 900. {
                self.flash = None;
            }
        }
        for (i, (colour, a)) in [(0xa97c1f, 0.4), (0xe0b13a, 0.62), (0xffe68a, 0.9)].into_iter().enumerate() {
            c.fill(&bins[i], colour, a);
        }
        for (i, &(x, y, s)) in SPIRALS.iter().enumerate() {
            let turn = t * if i % 2 == 1 { -0.28 } else { 0.28 };
            c.fill1(circ(x, y, s), 0x1d150b, 1.);
            let line: Vec<Pt> = (0..80).map(|n| {
                let a = n as f32 * 0.25;
                let rad = s * 0.95 * a / 20.;
                (x + (a + turn).cos() * rad, y + (a + turn).sin() * rad)
            }).collect();
            c.stroke(&[line], 5.5, false, 0xf0c552, 1.);
        }
    }

    fn click(&mut self, x: f32, y: f32) {
        self.flash = Some((x, y, 0.));
    }
}
