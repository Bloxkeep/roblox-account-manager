// An avatar with depth: Roblox's own model of it (the mesh, its materials and their pictures, which its site turns
// with three.js), drawn here into a picture from any side, standing in a room. No graphics card is asked: a few
// thousand triangles into a square is a few milliseconds of plain arithmetic on a thread of its own.
//
// The model is Roblox's "avatar-3d" thumbnail (roblox::solid): a Wavefront OBJ in the world's own coordinates (Y up,
// right-handed, the avatar facing -Z), an MTL whose map_Kd names are the pictures (map_d the see-through parts), and
// the camera Roblox would look through. The camera here starts where Roblox's does and circles the model: the avatar
// and the floor it stands on turn together (yaw), the camera rises or sinks (pitch) and comes closer (zoom, nearer
// the head the closer it comes).
use std::collections::HashMap;

/// The most bands a picture is drawn in at once.
const BAND_THREADS: usize = 16;

/// One band of a picture being drawn, borrowing the stage it reads and the rows it writes.
type Band<'a> = Box<dyn FnOnce() + Send + 'a>;

/// Runs the bands at once on the shared pool (threads.rs) and returns when every one is done; a band that panics
/// panics here once all are. A thread made and ended for every band of every picture left ~184 bytes behind in
/// NVIDIA's graphics driver each time, never given back: 16 threads a picture, 30 pictures a second while the avatar
/// moves, and Bloxkeep passed 1 GB within the hour (the owner, 2026-11-19: "sao kinh dị vậy").
fn run_bands(bands: Vec<Band<'_>>) {
    crate::threads::scope(|s| {
        for band in bands {
            s.spawn(band);
        }
    });
}

type V3 = [f32; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn mul(a: V3, k: f32) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
type M3 = [V3; 3];

/// A turn of `a` radians about the unit `axis` (right-handed).
fn rot(axis: V3, a: f32) -> M3 {
    let (s, c) = a.sin_cos();
    let [x, y, z] = axis;
    let k = 1. - c;
    [[c + x * x * k, x * y * k - z * s, x * z * k + y * s], [y * x * k + z * s, c + y * y * k, y * z * k - x * s], [z * x * k - y * s, z * y * k + x * s, c + z * z * k]]
}

fn mv(m: &M3, v: V3) -> V3 {
    [dot(m[0], v), dot(m[1], v), dot(m[2], v)]
}

fn mm(a: &M3, b: &M3) -> M3 {
    let col = |j: usize| [b[0][j], b[1][j], b[2][j]];
    let row = |i: usize| [dot(a[i], col(0)), dot(a[i], col(1)), dot(a[i], col(2))];
    [row(0), row(1), row(2)]
}

fn norm(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    if l > 1e-9 { [a[0] / l, a[1] / l, a[2] / l] } else { [0., 1., 0.] }
}

/// Where the avatar stands: the wall behind, the floor, the round plate under its feet and the plate's edge (0xRRGGBB).
/// The first is no room at all: the avatar alone on a clear ground, as the page's own box shows it.
pub struct Room {
    pub id: &'static str,
    pub wall: u32,
    pub floor: u32,
    pub plate: u32,
    pub rim: u32,
}

pub const ROOMS: [Room; 6] = [
    Room { id: "clear", wall: 0, floor: 0, plate: 0, rim: 0 },
    Room { id: "studio", wall: 0x1a1c21, floor: 0x24272e, plate: 0x30343c, rim: 0x454b56 },
    Room { id: "day", wall: 0x9fcdee, floor: 0x78ad55, plate: 0xe2dccd, rim: 0xbdb4a0 },
    Room { id: "dusk", wall: 0xeeb089, floor: 0xa9674f, plate: 0xead7c3, rim: 0xc8a68b },
    Room { id: "night", wall: 0x0d1322, floor: 0x161f35, plate: 0x223050, rim: 0x3a4d7a },
    Room { id: "paper", wall: 0xeeeae2, floor: 0xdcd6ca, plate: 0xf7f4ee, rim: 0xc9c1b1 },
];

fn rgb(c: u32) -> [f32; 3] {
    [((c >> 16) & 255) as f32, ((c >> 8) & 255) as f32, (c & 255) as f32]
}

/// How the avatar is looked at: turned (radians about its upright axis), the camera raised (radians, + is from above),
/// how near (1 = the whole avatar fits; less is nearer), the room (ROOMS) and where its idle is (seconds; 0 stands
/// still), and its walk: where on the floor it has walked to (from the plate's middle), the way it faces (radians, as
/// yaw turns), how far its stride has gone (radians) and how much it is walking (0 standing, 1 walking).
#[derive(Clone, Copy, PartialEq, Default)]
pub struct View {
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
    pub room: usize,
    pub t: f32,
    pub at: [f32; 2],
    pub heading: f32,
    pub stride: f32,
    pub pace: f32,
    /// A jump: how high off the floor it is and how fast it is going up (down when below 0), both in model units.
    pub up: f32,
    pub rise: f32,
}

pub const ZOOM_NEAR: f32 = 0.42;
pub const ZOOM_FAR: f32 = 1.35;
pub const PITCH_LOW: f32 = -0.1;
pub const PITCH_HIGH: f32 = 1.05;

#[derive(Clone)]
struct Tex {
    w: usize,
    h: usize,
    rgba: Vec<u8>,
}

impl Tex {
    fn of(bytes: &[u8]) -> Option<Tex> {
        let i = image::load_from_memory(bytes).ok()?.to_rgba8();
        let (w, h) = (i.width() as usize, i.height() as usize);
        (w > 0 && h > 0).then(|| Tex { w, h, rgba: i.into_raw() })
    }

    /// The colour and how solid it is at a place on the picture (0..1 each way, repeating), between its four nearest
    /// dots.
    fn at(&self, u: f32, v: f32) -> [f32; 4] {
        let x = (u - u.floor()) * self.w as f32 - 0.5;
        let y = (1. - (v - v.floor())) * self.h as f32 - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let px = |xi: f32, yi: f32| {
            let xi = (xi as isize).rem_euclid(self.w as isize) as usize;
            let yi = (yi as isize).clamp(0, self.h as isize - 1) as usize;
            let i = (yi * self.w + xi) * 4;
            [self.rgba[i] as f32, self.rgba[i + 1] as f32, self.rgba[i + 2] as f32, self.rgba[i + 3] as f32]
        };
        let (a, b, c, d) = (px(x0, y0), px(x0 + 1., y0), px(x0, y0 + 1.), px(x0 + 1., y0 + 1.));
        let mut out = [0.; 4];
        for k in 0..4 {
            out[k] = (a[k] * (1. - fx) + b[k] * fx) * (1. - fy) + (c[k] * (1. - fx) + d[k] * fx) * fy;
        }
        out
    }
}

#[derive(Clone)]
struct Mat {
    kd: [f32; 3],
    tex: Option<Tex>,
    /// The see-through map (map_d): its alpha, or its grey where it has none.
    mask: Option<Tex>,
    /// How solid the whole material is (d); a material all but invisible isn't drawn.
    d: f32,
}

/// A triangle as it lands on the stage this picture: its corners, their depth (1/z), normals and picture places,
/// and the rows and columns it can cover.
struct Shown {
    sx: [f32; 3],
    sy: [f32; 3],
    iz: [f32; 3],
    nn: [V3; 3],
    uv: [[f32; 2]; 3],
    mat: usize,
    area: f32,
    x0: usize,
    x1: usize,
    y0: usize,
    y1: usize,
}

#[derive(Clone)]
struct Tri {
    p: [V3; 3],
    uv: [[f32; 2]; 3],
    n: [V3; 3],
    mat: usize,
    /// The OBJ group it is of, and the part it moves with (STILL, HEAD, ARM_A, ARM_B).
    group: u16,
    bone: u8,
}

/// What a triangle moves with in the idle: nothing, the head (and what is worn on it), or one arm or the other.
const STILL: u8 = 0;
const HEAD: u8 = 1;
const ARM_A: u8 = 2;
const LEG_A: u8 = 4;

/// How the avatar moves. Roblox's model keeps each body part a group of its own (Player1, Player2… one per part: six
/// for R6, fifteen for R15) and each worn thing another (Handle1…); which part is which is found from where it is,
/// not its number: the head the highest, an arm far out to one side. `side` is the axis across the shoulders (0 = X,
/// 2 = Z); the neck and each shoulder are where those turn.
#[derive(Clone)]
struct Rig {
    side: usize,
    neck: V3,
    shoulders: [Option<V3>; 2],
    /// Each hip (a leg's top) and how high the hips stand over the feet.
    hips: [Option<V3>; 2],
    leg: f32,
}

impl Rig {
    /// The turn of each part at `t` seconds, with the point it turns about: a slow idle, the head looking a little
    /// round and nodding, the arms swinging gently against each other. Small, so a worn thing that isn't quite where
    /// it was found to be (a long cape) never shows a gap.
    ///
    /// Walking (`pace` of it, at `stride`), the legs step against each other and each arm swings against its own side's
    /// leg; the body sinks as the legs spread, so the foot in front stays on the floor (the second value: how far).
    fn pose(&self, t: f32, stride: f32, pace: f32) -> ([Option<(V3, M3)>; 6], f32) {
        let mut e = [0.; 3];
        e[self.side] = 1.;
        let still = 1. - pace;
        let look = 0.4 + 0.6 * still;
        let head = mm(&rot([0., 1., 0.], look * (0.19 * (t * 0.72).sin() + 0.045 * (t * 1.8).sin())), &rot(e, look * 0.085 * (t * 1.55).sin()));
        let idle_a = still * (0.22 * (t * 1.7).sin() + 0.035 * (t * 3.4).sin());
        let idle_b = still * (-0.20 * (t * 1.7).sin() + 0.045 * (t * 2.35).sin());
        let step = pace * stride.sin();
        let (arm, leg) = (0.55 * step, 0.5 * step);
        let sink = -self.leg * (1. - leg.cos());
        (
            [
                None,
                Some((self.neck, head)),
                self.shoulders[0].map(|p| (p, rot(e, idle_a - arm))),
                self.shoulders[1].map(|p| (p, rot(e, idle_b + arm))),
                self.hips[0].map(|p| (p, rot(e, leg))).filter(|_| pace > 0.),
                self.hips[1].map(|p| (p, rot(e, -leg))).filter(|_| pace > 0.),
            ],
            sink,
        )
    }
}

/// The rig of a model whose groups are named as Roblox's are; None when they aren't (it then stands still).
fn rig_of(tris: &mut [Tri], names: &[String]) -> Option<Rig> {
    let n = names.len();
    let (mut lo, mut hi) = (vec![[f32::MAX; 3]; n], vec![[f32::MIN; 3]; n]);
    for t in tris.iter() {
        let g = t.group as usize;
        for p in t.p {
            for k in 0..3 {
                lo[g][k] = lo[g][k].min(p[k]);
                hi[g][k] = hi[g][k].max(p[k]);
            }
        }
    }
    let used = |g: usize| lo[g][0] <= hi[g][0];
    let body: Vec<usize> = (0..n).filter(|&g| used(g) && names[g].to_ascii_lowercase().starts_with("player")).collect();
    if body.len() < 6 {
        return None;
    }
    let (mut blo, mut bhi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for &g in &body {
        for k in 0..3 {
            blo[k] = blo[k].min(lo[g][k]);
            bhi[k] = bhi[k].max(hi[g][k]);
        }
    }
    let h = bhi[1] - blo[1];
    if h <= 0. {
        return None;
    }
    let mid = [(blo[0] + bhi[0]) / 2., 0., (blo[2] + bhi[2]) / 2.];
    let c = |g: usize| [(lo[g][0] + hi[g][0]) / 2., (lo[g][1] + hi[g][1]) / 2., (lo[g][2] + hi[g][2]) / 2.];
    let spread = |k: usize| body.iter().map(|&g| (c(g)[k] - mid[k]).abs()).fold(0f32, f32::max);
    let side = if spread(0) >= spread(2) { 0 } else { 2 };
    let head = *body.iter().max_by(|&&a, &&b| c(a)[1].total_cmp(&c(b)[1]))?;
    let mut bone = vec![STILL; n];
    bone[head] = HEAD;
    // An arm: a part far out to a side and not down at the feet; its top part holds the shoulder.
    let mut tops: [Option<usize>; 2] = [None, None];
    for &g in &body {
        let off = c(g)[side] - mid[side];
        if g == head || off.abs() < 0.2 * h || c(g)[1] < blo[1] + 0.3 * h {
            continue;
        }
        let s = usize::from(off > 0.);
        bone[g] = ARM_A + s as u8;
        if tops[s].is_none_or(|t| hi[g][1] > hi[t][1]) {
            tops[s] = Some(g);
        }
    }
    let shoulder = |g: usize| {
        let mut p = c(g);
        p[1] = hi[g][1] - 0.25 * (hi[g][1] - lo[g][1]);
        p
    };
    // A leg: a part low down, a little to a side; its top part holds the hip.
    let mut hips: [Option<usize>; 2] = [None, None];
    for &g in &body {
        let off = c(g)[side] - mid[side];
        if bone[g] != STILL || off.abs() < 0.02 * h || c(g)[1] > blo[1] + 0.36 * h {
            continue;
        }
        let s = usize::from(off > 0.);
        bone[g] = LEG_A + s as u8;
        if hips[s].is_none_or(|t| hi[g][1] > hi[t][1]) {
            hips[s] = Some(g);
        }
    }
    let hip = |g: usize| {
        let mut p = c(g);
        p[1] = hi[g][1];
        p
    };
    let leg = hips.iter().flatten().map(|&g| hi[g][1] - blo[1]).fold(0f32, f32::max);
    // A worn thing goes with the head when it reaches over the head's middle, above it: a hat, hair (long hair too).
    let hc = c(head);
    let reach = (hi[head][0] - lo[head][0]).max(hi[head][2] - lo[head][2]) * 1.5;
    for g in (0..n).filter(|&g| used(g) && !body.contains(&g)) {
        let k = c(g);
        if hi[g][1] > hc[1] && (k[0] - hc[0]).abs() < reach && (k[2] - hc[2]).abs() < reach {
            bone[g] = HEAD;
        }
    }
    for t in tris.iter_mut() {
        t.bone = bone[t.group as usize];
    }
    Some(Rig { side, neck: [hc[0], lo[head][1], hc[2]], shoulders: [tops[0].map(shoulder), tops[1].map(shoulder)], hips: [hips[0].map(hip), hips[1].map(hip)], leg })
}

/// The shadow's map: how dark the floor is under the model, over a square of the floor around its middle.
const SH: usize = 128;

#[derive(Clone)]
pub struct Model {
    tris: Vec<Tri>,
    mats: Vec<Mat>,
    /// The model's middle and the reach of its farthest corner from it, and where the camera stands.
    center: V3,
    radius: f32,
    eye: V3,
    /// The height of the floor (the model's lowest point), the plate's reach, and the shadow on the floor (SH × SH over
    /// ±`radius` around the middle, in the model's own unturned frame).
    floor: f32,
    plate: f32,
    shadow: Vec<f32>,
    /// How it moves in its idle; None: it stands still.
    rig: Option<Rig>,
    /// The materials of the body's own parts (the OBJ's Player groups), whose skin a colour change repaints.
    body: Vec<usize>,
}

impl Model {
    /// Reads the OBJ and its MTL. `pictures` are the MTL's pictures by the names it gives them; `eye` is the camera's
    /// place and `min`/`max` the model's box, as Roblox says them. None when there is nothing to draw.
    pub fn parse(obj: &str, mtl: &str, pictures: &HashMap<String, Vec<u8>>, eye: V3, min: V3, max: V3) -> Option<Model> {
        let mut mats: Vec<Mat> = Vec::new();
        let mut names: HashMap<String, usize> = HashMap::new();
        // (The name is the last word: options may stand before it. A path's last part is the name.)
        let named = |it: std::str::SplitWhitespace| -> String {
            let name = it.last().unwrap_or_default();
            name.rsplit(['/', '\\']).next().unwrap_or(name).to_string()
        };
        let picture = |name: &str| -> Option<Tex> { pictures.get(name).and_then(|b| Tex::of(b)) };
        // The colour picture of the material being read, by name: its mask is often that same file.
        let mut colours = String::new();
        for line in mtl.lines() {
            let mut it = line.split_whitespace();
            match it.next() {
                Some("newmtl") => {
                    names.insert(it.next().unwrap_or_default().to_string(), mats.len());
                    mats.push(Mat { kd: [0.8; 3], tex: None, mask: None, d: 1. });
                    colours.clear();
                }
                Some("Kd") => {
                    let v: Vec<f32> = it.filter_map(|t| t.parse().ok()).collect();
                    if let (Some(m), 3) = (mats.last_mut(), v.len()) {
                        m.kd = [v[0], v[1], v[2]];
                    }
                }
                Some("d") => {
                    if let (Some(m), Some(d)) = (mats.last_mut(), it.next().and_then(|t| t.parse::<f32>().ok()).filter(|v| v.is_finite())) {
                        m.d = d.clamp(0., 1.);
                    }
                }
                Some("Tr") => {
                    if let (Some(m), Some(tr)) = (mats.last_mut(), it.next().and_then(|t| t.parse::<f32>().ok()).filter(|v| v.is_finite())) {
                        m.d = 1. - tr.clamp(0., 1.);
                    }
                }
                Some("map_Kd") => {
                    if let Some(m) = mats.last_mut() {
                        colours = named(it);
                        // A missing or unreadable colour atlas would leave the face or clothing blank.
                        m.tex = Some(picture(&colours)?);
                    }
                }
                Some("map_d") => {
                    let name = named(it);
                    // Roblox names the colour picture itself as every material's mask: its own alpha is the mask
                    // then, and a picture without any is solid. Read as grey (below), a dark worn thing went
                    // see-through: on R15, where each worn thing has a picture of its own, black sunglasses and a
                    // dark scarf were not there at all, while R6's one shared picture (with alpha) drew them (the
                    // owner's two models, read from the files Roblox gave, 2026-11-18).
                    let own = name == colours;
                    let mask = picture(&name).map(|mut t| {
                        // A map of its own with no alpha says it in grey.
                        if !own && t.rgba.chunks(4).all(|p| p[3] == 255) {
                            for p in t.rgba.chunks_mut(4) {
                                p[3] = p[0];
                            }
                        }
                        t
                    });
                    if let Some(m) = mats.last_mut() {
                        m.mask = mask;
                    }
                }
                _ => {}
            }
        }
        if mats.is_empty() {
            mats.push(Mat { kd: [0.8; 3], tex: None, mask: None, d: 1. });
        }
        let (mut v, mut vt, mut vn): (Vec<V3>, Vec<[f32; 2]>, Vec<V3>) = (Vec::new(), Vec::new(), Vec::new());
        let mut tris: Vec<Tri> = Vec::new();
        let mut mat = 0;
        // The groups, by name (a body part, a worn thing); before any, the nameless one.
        let mut group_names: Vec<String> = vec![String::new()];
        let mut group = 0u16;
        // An OBJ counts from 1, and from the end when the number is below zero.
        let pick = |i: i64, len: usize| -> Option<usize> {
            let k = if i > 0 { i - 1 } else { len as i64 + i };
            (k >= 0 && (k as usize) < len).then_some(k as usize)
        };
        for line in obj.lines() {
            let mut it = line.split_whitespace();
            match it.next() {
                Some("v") => {
                    let a: Vec<f32> = it.filter_map(|t| t.parse().ok()).collect();
                    if a.len() >= 3 {
                        v.push([a[0], a[1], a[2]]);
                    }
                }
                Some("vt") => {
                    let a: Vec<f32> = it.filter_map(|t| t.parse().ok()).collect();
                    if a.len() >= 2 {
                        vt.push([a[0], a[1]]);
                    }
                }
                Some("vn") => {
                    let a: Vec<f32> = it.filter_map(|t| t.parse().ok()).collect();
                    if a.len() >= 3 {
                        vn.push([a[0], a[1], a[2]]);
                    }
                }
                Some("usemtl") => mat = names.get(it.next().unwrap_or_default()).copied().unwrap_or(0),
                Some("g") | Some("o") => {
                    let name = it.next().unwrap_or_default().to_string();
                    group = match group_names.iter().position(|g| *g == name) {
                        Some(i) => i as u16,
                        None if group_names.len() < u16::MAX as usize => {
                            group_names.push(name);
                            (group_names.len() - 1) as u16
                        }
                        None => 0,
                    };
                }
                Some("f") => {
                    // Each corner is place/picture/normal; the last two may be missing.
                    let corners: Vec<(V3, [f32; 2], Option<V3>)> = it
                        .filter_map(|t| {
                            let mut parts = t.split('/');
                            let p = pick(parts.next()?.parse().ok()?, v.len())?;
                            let uv = parts.next().and_then(|s| s.parse().ok()).and_then(|i| pick(i, vt.len())).map(|i| vt[i]).unwrap_or([0., 0.]);
                            let n = parts.next().and_then(|s| s.parse().ok()).and_then(|i| pick(i, vn.len())).map(|i| vn[i]);
                            Some((v[p], uv, n))
                        })
                        .collect();
                    // (A material all but invisible, such as the root part, is left out.)
                    if mats.get(mat).is_some_and(|m| m.d < 0.05) {
                        continue;
                    }
                    for k in 1..corners.len().saturating_sub(1) {
                        let (a, b, c) = (corners[0], corners[k], corners[k + 1]);
                        let flat = norm(cross(sub(b.0, a.0), sub(c.0, a.0)));
                        tris.push(Tri { p: [a.0, b.0, c.0], uv: [a.1, b.1, c.1], n: [a.2.unwrap_or(flat), b.2.unwrap_or(flat), c.2.unwrap_or(flat)], mat, group, bone: STILL });
                    }
                }
                _ => {}
            }
        }
        if tris.is_empty() {
            return None;
        }
        // The model's own box: its floor, and its middle when Roblox gives no box.
        let (mut olo, mut ohi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for t in &tris {
            for p in t.p {
                for k in 0..3 {
                    olo[k] = olo[k].min(p[k]);
                    ohi[k] = ohi[k].max(p[k]);
                }
            }
        }
        let (lo, hi) = if (0..3).any(|k| max[k] <= min[k]) { (olo, ohi) } else { (min, max) };
        let center = [(lo[0] + hi[0]) / 2., (lo[1] + hi[1]) / 2., (lo[2] + hi[2]) / 2.];
        let half = sub(hi, center);
        let radius = dot(half, half).sqrt().max(0.01);
        // A camera inside the model (or none given) stands back in front of it instead (the avatar faces -Z).
        // (So does one straight over or under it, which has no upright to hold on to.)
        let away = sub(eye, center);
        let far = dot(away, away).sqrt();
        let level = far > 0. && (away[1] / far).abs() < 0.95;
        let eye = if eye == [0.; 3] || far < radius * 1.5 || !level { [center[0], center[1] + radius * 0.25, center[2] - radius * 3.6] } else { eye };
        // The plate reaches a little past the widest the model is, seen from above.
        let reach = tris.iter().flat_map(|t| t.p).map(|p| ((p[0] - center[0]).powi(2) + (p[2] - center[2]).powi(2)).sqrt()).fold(0f32, f32::max);
        let plate = (reach * 1.15).clamp(radius * 0.45, radius * 0.95);
        let shadow = shadow_of(&tris, center, radius, olo[1], ohi[1]);
        let rig = rig_of(&mut tris, &group_names);
        let mut body: Vec<usize> = tris.iter().filter(|t| group_names.get(t.group as usize).is_some_and(|g| g.to_ascii_lowercase().starts_with("player"))).map(|t| t.mat).collect();
        body.sort_unstable();
        body.dedup();
        Some(Model { tris, mats, center, radius, eye, floor: olo[1], plate, shadow, rig, body })
    }

    /// The model with its skin in another colour, at once: on the body's parts, the dots and material colours that are
    /// one of the old skin colours (within a few steps) become the new one; what is worn over the skin keeps its own.
    /// Roblox takes seconds to make the new model after a change (owner, 2026-11-17: "đổi skin color… load khá lâu"):
    /// this one shows meanwhile, and Roblox's replaces it when it comes.
    pub fn recoloured(&self, from: &[[u8; 3]], to: [u8; 3]) -> Model {
        let mut m = self.clone();
        let near = |r: i32, g: i32, b: i32| from.iter().any(|f| (f[0] as i32 - r).abs() <= 6 && (f[1] as i32 - g).abs() <= 6 && (f[2] as i32 - b).abs() <= 6);
        for &i in &self.body {
            let Some(mat) = m.mats.get_mut(i) else { continue };
            if near((mat.kd[0] * 255.).round() as i32, (mat.kd[1] * 255.).round() as i32, (mat.kd[2] * 255.).round() as i32) {
                mat.kd = [to[0] as f32 / 255., to[1] as f32 / 255., to[2] as f32 / 255.];
            }
            if let Some(t) = mat.tex.as_mut() {
                for p in t.rgba.chunks_mut(4) {
                    if p[3] > 0 && near(p[0] as i32, p[1] as i32, p[2] as i32) {
                        p[..3].copy_from_slice(&to);
                    }
                }
            }
        }
        m
    }

    /// Whether it can move (its parts were found).
    pub fn moves(&self) -> bool {
        self.rig.is_some()
    }

    /// `dt` seconds of walking: `ahead` and `right` (-1 to 1) as the camera sees them (W away from you, D to your
    /// right). It walks that way at an easy pace, turning to face where it goes; let go, it slows to a stand.
    /// `run`: Shift held, it runs, twice as fast with a longer, quicker stride. A jump under way (`jump` starts one
    /// from the floor) goes up and comes back down in about two thirds of a second, as high as a third of its height.
    pub fn step(&self, v: &mut View, ahead: f32, right: f32, run: bool, jump: bool, dt: f32) {
        if self.rig.is_some() {
            let fall = self.radius * 9.;
            if jump && v.up <= 0. {
                v.rise = self.radius * 3.3;
            }
            if v.up > 0. || v.rise > 0. {
                v.up += v.rise * dt - fall * dt * dt / 2.;
                v.rise -= fall * dt;
                if v.up <= 0. {
                    (v.up, v.rise) = (0., 0.);
                }
            }
        }
        let start = sub(self.eye, self.center);
        let flat = {
            let f = [start[0], 0., start[2]];
            if dot(f, f) < 1e-9 { [0., 0., -1.] } else { norm(f) }
        };
        // The camera's way and its right, back on the floor's own (unturned) frame.
        let (s, c) = v.yaw.sin_cos();
        let undo = |p: V3| [p[0] * c - p[2] * s, 0., p[0] * s + p[2] * c];
        let look = mul(flat, -1.);
        let m = add(mul(undo(look), ahead), mul(undo(cross(look, [0., 1., 0.])), right));
        let len = dot(m, m).sqrt();
        if len > 1e-3 && self.rig.is_some() {
            let m = mul(m, 1. / len);
            let speed = self.radius * if run { 1.9 } else { 0.9 };
            v.at[0] += m[0] * speed * dt;
            v.at[1] += m[2] * speed * dt;
            // (Not off into the distance: the plate stays findable.)
            let far = (v.at[0] * v.at[0] + v.at[1] * v.at[1]).sqrt();
            let most = self.radius * 6.;
            if far > most {
                v.at = [v.at[0] * most / far, v.at[1] * most / far];
            }
            // It faces -Z, turned the way yaw turns: facing (-sin h, -cos h).
            let want = (-m[0]).atan2(-m[2]);
            let pi = std::f32::consts::PI;
            let turn = (want - v.heading + pi).rem_euclid(2. * pi) - pi;
            v.heading += turn * (dt * 10.).min(1.);
            v.stride = (v.stride + speed * dt / (self.radius * if run { 0.6 } else { 0.42 })).rem_euclid(2. * pi);
            v.pace = (v.pace + dt * 6.).min(1.);
        } else {
            v.pace = (v.pace - dt * 5.).max(0.);
        }
    }

    /// The model looked at as `view` says, drawn into a square of `size` dots: BGRA, in its room (clear where the model
    /// isn't, in the clear room). Each dot is the mean of four, so edges come out smooth.
    pub fn draw(&self, view: View, size: usize) -> Vec<u8> {
        self.draw_on(view, size, std::thread::available_parallelism().map(|c| c.get()).unwrap_or(4))
    }

    /// The same, in `cores` bands of rows at once (1: the whole stage on this thread, as it was first drawn).
    pub fn draw_on(&self, view: View, size: usize, cores: usize) -> Vec<u8> {
        const SS: usize = 2;
        let n = size * SS;
        let zoom = view.zoom.clamp(ZOOM_NEAR, ZOOM_FAR);
        // The camera circles the model's upright axis: from Roblox's own place, raised by the pitch, nearer by the
        // zoom, and looking a little higher the nearer it is (at the face, close up).
        let start = sub(self.eye, self.center);
        let d0 = dot(start, start).sqrt().max(self.radius * 1.2);
        let flat = {
            let f = [start[0], 0., start[2]];
            if dot(f, f) < 1e-9 { [0., 0., -1.] } else { norm(f) }
        };
        let el = ((start[1] / d0).clamp(-1., 1.).asin() + view.pitch).clamp(-0.2, 1.3);
        let lift = ((1. - zoom) / (1. - ZOOM_NEAR)).clamp(0., 1.);
        // (Whole, it stands a little high, so the plate under its feet shows.)
        let target = [self.center[0], self.center[1] + self.radius * (0.5 * lift - 0.1 * (1. - lift)), self.center[2]];
        let dir = [flat[0] * el.cos(), el.sin(), flat[2] * el.cos()];
        let eye = add(target, mul(dir, d0 * zoom));
        let f = mul(dir, -1.);
        let s = norm(cross(f, [0., 1., 0.]));
        let u = cross(s, f);
        // Wide enough that the model fits whichever way it is turned, at zoom 1; nearer, it fills more.
        let t = self.radius * 1.16 / (d0 * d0 - self.radius * self.radius).max(1e-6).sqrt();
        let (sin, cos) = view.yaw.sin_cos();
        // The light: from over the viewer's left shoulder, and from above.
        let light = norm(add(add(mul(f, -0.7), mul(u, 0.6)), mul(s, -0.35)));
        let half = n as f32 / 2.;
        // Each part's turn in the idle, at this moment.
        let posed = self.rig.as_ref().filter(|_| view.t > 0. || view.pace > 0.).map(|r| r.pose(view.t, view.stride, view.pace));
        let sink = posed.as_ref().map_or(0., |p| p.1) + view.up.max(0.);
        // Lean through the body while the feet stay planted. Fade the idle out into a stride or jump.
        let sway = self.rig.as_ref().filter(|_| view.t > 0. && view.up <= 0.).map(|rig| {
            let blend = (1. - view.pace).powi(2);
            (rig.side, blend * (0.059 * (view.t * 1.7).sin() + 0.012 * (view.t * 3.4).sin()))
        });
        // The avatar faces where it walked; the floor turns only with the camera.
        let (hs, hc) = (view.yaw + view.heading).sin_cos();
        let face = |p: V3| -> V3 { [p[0] * hc + p[2] * hs, p[1], -p[0] * hs + p[2] * hc] };
        let mut shown: Vec<Shown> = Vec::with_capacity(self.tris.len());
        let mut transparent = Vec::new();
        for tri in &self.tris {
            let (tp, tn) = match posed.as_ref().and_then(|p| p.0[tri.bone as usize].as_ref()) {
                Some((at, m)) => (tri.p.map(|p| add(mv(m, sub(p, *at)), *at)), tri.n.map(|v| mv(m, v))),
                None => (tri.p, tri.n),
            };
            let tp = tp.map(|mut p| {
                if let Some((side, lean)) = sway {
                    p[side] += (p[1] - self.floor).max(0.) * lean;
                }
                p[1] += sink;
                p
            });
            let mut sx = [0f32; 3];
            let mut sy = [0f32; 3];
            let mut z = [0f32; 3];
            let mut nn = [[0f32; 3]; 3];
            let mut behind = false;
            for k in 0..3 {
                let w = sub(add(face(sub(tp[k], self.center)), self.center), eye);
                z[k] = dot(w, f);
                if z[k] < 0.05 {
                    behind = true;
                    break;
                }
                sx[k] = half + dot(w, s) / (z[k] * t) * half;
                sy[k] = half - dot(w, u) / (z[k] * t) * half;
                let mut normal = tn[k];
                if let Some((side, lean)) = sway { normal[1] -= normal[side] * lean; }
                nn[k] = face(normal);
            }
            if behind {
                continue;
            }
            let area = (sx[1] - sx[0]) * (sy[2] - sy[0]) - (sx[2] - sx[0]) * (sy[1] - sy[0]);
            if area.abs() < 1e-6 {
                continue;
            }
            let x0 = sx.iter().cloned().fold(f32::MAX, f32::min).floor().max(0.) as usize;
            let x1 = (sx.iter().cloned().fold(f32::MIN, f32::max).ceil() as isize).min(n as isize - 1);
            let y0 = sy.iter().cloned().fold(f32::MAX, f32::min).floor().max(0.) as usize;
            let y1 = (sy.iter().cloned().fold(f32::MIN, f32::max).ceil() as isize).min(n as isize - 1);
            if x1 < 0 || y1 < 0 || x0 > x1 as usize || y0 > y1 as usize {
                continue;
            }
            let mat = tri.mat.min(self.mats.len() - 1);
            let projected = Shown { sx, sy, iz: [1. / z[0], 1. / z[1], 1. / z[2]], nn, uv: tri.uv, mat, area, x0, x1: x1 as usize, y0, y1: y1 as usize };
            if self.mats[mat].d < 1. {
                transparent.push(projected);
            } else {
                shown.push(projected);
            }
        }
        // Draw opaque depth first, then blend translucent triangles from far to near.
        transparent.sort_by(|a, b| a.iz.iter().sum::<f32>().total_cmp(&b.iz.iter().sum::<f32>()));
        shown.extend(transparent);
        let room = &ROOMS[view.room.min(ROOMS.len() - 1)];
        let walled = room.wall != 0;
        // The room where the model isn't: the floor where a dot's line of sight comes down to it, else the wall.
        let (wall, floor, plate, rim) = (rgb(room.wall), rgb(room.floor), rgb(room.plate), rgb(room.rim));
        let ground = |x: usize, y: usize| -> [f32; 3] {
            let nx = (x as f32 + 0.5 - half) / half * t;
            let ny = (half - (y as f32 + 0.5)) / half * t;
            let ray = add(f, add(mul(s, nx), mul(u, ny)));
            if ray[1] >= -1e-4 {
                return wall;
            }
            let k = (self.floor - eye[1]) / ray[1];
            if k <= 0. {
                return wall;
            }
            let hit = sub(add(eye, mul(ray, k)), self.center);
            // Back into the floor's own frame (it turns with the camera), from where the avatar stands; then the room's
            // (the plate stays where it was as the avatar walks off it) and the avatar's own (its shadow).
            let (mx, mz) = (hit[0] * cos - hit[2] * sin, hit[0] * sin + hit[2] * cos);
            let (wx, wz) = (mx + view.at[0], mz + view.at[1]);
            let (hs, hc) = view.heading.sin_cos();
            let (lx, lz) = (mx * hc - mz * hs, mx * hs + mz * hc);
            let rho = (wx * wx + wz * wz).sqrt();
            let mut c = if rho < self.plate {
                plate
            } else if rho < self.plate + self.radius * 0.035 {
                rim
            } else {
                // Faint tiles, so a walk shows going by.
                let tile = self.radius * 0.6;
                let line = |v: f32| {
                    let f = (v / tile).rem_euclid(1.);
                    f.min(1. - f) * tile < self.radius * 0.007
                };
                if line(wx) || line(wz) {
                    [floor[0] * 0.8 + rim[0] * 0.2, floor[1] * 0.8 + rim[1] * 0.2, floor[2] * 0.8 + rim[2] * 0.2]
                } else {
                    floor
                }
            };
            // The floor far off goes over into the wall, as a room's far side would.
            let far = (((mx * mx + mz * mz).sqrt() - self.radius * 4.) / (self.radius * 8.)).clamp(0., 1.);
            // (Its shadow fades as it jumps away from it.)
            let dark = 1. - 0.55 * self.shade(lx, lz) * (1. - view.up / self.radius).clamp(0.35, 1.);
            for i in 0..3 {
                c[i] = (c[i] * dark) * (1. - far) + wall[i] * far;
            }
            c
        };
        // Drawn in bands of rows, one per core, each with its own depth and colours: a 560-dot stage is 1.25 million
        // dots a picture at four per dot, which one core drew far too slowly to follow a drag or a walk (the owner,
        // 2026-11-15: "lag kinh khủng khiếp"). The same picture as before, dot for dot. The bands run on threads kept
        // for them (`run_bands`), not threads made for each picture.
        let mut out = vec![0u8; size * size * 4];
        let cores = cores.clamp(1, BAND_THREADS);
        let rows_out = size.div_ceil(cores).max(1);
        {
            let mut bands: Vec<Band<'_>> = Vec::new();
            for (i, chunk) in out.chunks_mut(rows_out * size * 4).enumerate() {
                let (shown, ground) = (&shown, &ground);
                bands.push(Box::new(move || {
                    let y_out = i * rows_out;
                    let rows = chunk.len() / (size * 4);
                    let (top, bottom) = (y_out * SS, (y_out + rows) * SS);
                    let mut depth = vec![f32::MAX; (bottom - top) * n];
                    let mut col = vec![[0u8; 4]; (bottom - top) * n];
                    for t in shown.iter() {
                        if t.y1 < top || t.y0 >= bottom {
                            continue;
                        }
                        let material = &self.mats[t.mat];
                        for y in t.y0.max(top)..=t.y1.min(bottom - 1) {
                            let py = y as f32 + 0.5;
                            for x in t.x0..=t.x1 {
                                let px = x as f32 + 0.5;
                                // How much of the dot belongs to each corner (all of one sign: the dot is inside).
                                let w0 = ((t.sx[1] - px) * (t.sy[2] - py) - (t.sx[2] - px) * (t.sy[1] - py)) / t.area;
                                let w1 = ((t.sx[2] - px) * (t.sy[0] - py) - (t.sx[0] - px) * (t.sy[2] - py)) / t.area;
                                let w2 = 1. - w0 - w1;
                                if w0 < 0. || w1 < 0. || w2 < 0. {
                                    continue;
                                }
                                let inv = w0 * t.iz[0] + w1 * t.iz[1] + w2 * t.iz[2];
                                let zz = 1. / inv;
                                let at = (y - top) * n + x;
                                if zz >= depth[at] {
                                    continue;
                                }
                                // (Through the depth, so a picture doesn't swim on a face seen at a slant.)
                                let (a, b, c) = (w0 * t.iz[0] * zz, w1 * t.iz[1] * zz, w2 * t.iz[2] * zz);
                                let (tu, tv) = (a * t.uv[0][0] + b * t.uv[1][0] + c * t.uv[2][0], a * t.uv[0][1] + b * t.uv[1][1] + c * t.uv[2][1]);
                                let base = match &material.tex {
                                    Some(tex) => tex.at(tu, tv),
                                    None => [material.kd[0] * 255., material.kd[1] * 255., material.kd[2] * 255., 255.],
                                };
                                // The see-through parts (hair's strands, a decal's edges) let what is behind show.
                                let solid = material.mask.as_ref().map(|k| k.at(tu, tv)[3]).unwrap_or(base[3]);
                                if solid < 128. {
                                    continue;
                                }
                                let nrm = norm([a * t.nn[0][0] + b * t.nn[1][0] + c * t.nn[2][0], a * t.nn[0][1] + b * t.nn[1][1] + c * t.nn[2][1], a * t.nn[0][2] + b * t.nn[1][2] + c * t.nn[2][2]]);
                                // Light on both sides of a thin part (a cape, a leaf): the side facing the camera.
                                let nrm = if dot(nrm, f) > 0. { mul(nrm, -1.) } else { nrm };
                                let lit = 0.58 + 0.42 * dot(nrm, light).max(0.);
                                if material.d >= 1. {
                                    depth[at] = zz;
                                    col[at] = [(base[0] * lit).min(255.) as u8, (base[1] * lit).min(255.) as u8, (base[2] * lit).min(255.) as u8, 255];
                                } else {
                                    let alpha = material.d;
                                    let under = 1. - alpha;
                                    let pixel = &mut col[at];
                                    for channel in 0..3 {
                                        pixel[channel] = ((base[channel] * lit).min(255.) * alpha + pixel[channel] as f32 * under).round() as u8;
                                    }
                                    pixel[3] = (255. * alpha + pixel[3] as f32 * under).round() as u8;
                                }
                            }
                        }
                    }
                    let out = chunk;
                    for y in y_out..y_out + rows {
                        for x in 0..size {
                            let (mut r, mut g, mut b, mut hit) = (0f32, 0f32, 0f32, 0f32);
                            for dy in 0..SS {
                                for dx in 0..SS {
                                    let (sxp, syp) = (x * SS + dx, y * SS + dy);
                                    let at = (syp - top) * n + sxp;
                                    let [red, green, blue, alpha] = col[at];
                                    let covered = alpha as f32 / 255.;
                                    r += red as f32;
                                    g += green as f32;
                                    b += blue as f32;
                                    hit += covered;
                                    if walled && alpha < 255 {
                                        let c = ground(sxp, syp);
                                        let uncovered = 1. - covered;
                                        r += c[0] * uncovered;
                                        g += c[1] * uncovered;
                                        b += c[2] * uncovered;
                                        hit += uncovered;
                                    }
                                }
                            }
                            if hit > 0. {
                                let o = ((y - y_out) * size + x) * 4;
                                out[o] = (b / hit) as u8;
                                out[o + 1] = (g / hit) as u8;
                                out[o + 2] = (r / hit) as u8;
                                out[o + 3] = (hit * 255. / (SS * SS) as f32) as u8;
                            }
                        }
                    }
                }));
            }
            run_bands(bands);
        }
        out
    }

    /// How dark the floor is at a place (model's own frame, from its middle): 0 in the open, near 1 right under it.
    fn shade(&self, x: f32, z: f32) -> f32 {
        let gx = (x / self.radius * 0.5 + 0.5) * SH as f32 - 0.5;
        let gz = (z / self.radius * 0.5 + 0.5) * SH as f32 - 0.5;
        if gx < 0. || gz < 0. || gx >= (SH - 1) as f32 || gz >= (SH - 1) as f32 {
            return 0.;
        }
        let (x0, z0) = (gx as usize, gz as usize);
        let (fx, fz) = (gx - x0 as f32, gz - z0 as f32);
        let at = |x: usize, z: usize| self.shadow[z * SH + x];
        (at(x0, z0) * (1. - fx) + at(x0 + 1, z0) * fx) * (1. - fz) + (at(x0, z0 + 1) * (1. - fx) + at(x0 + 1, z0 + 1) * fx) * fz
    }
}

/// The model seen from straight above, each part the darker the nearer the floor it is, and softened: the shadow the
/// avatar casts on what it stands on, under a light from overhead.
fn shadow_of(tris: &[Tri], center: V3, radius: f32, lo: f32, hi: f32) -> Vec<f32> {
    let mut map = vec![0f32; SH * SH];
    let tall = (hi - lo).max(1e-3);
    let to = |p: V3| ((p[0] - center[0]) / radius * 0.5 + 0.5) * SH as f32;
    let tz = |p: V3| ((p[2] - center[2]) / radius * 0.5 + 0.5) * SH as f32;
    for tri in tris {
        let low = tri.p.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
        let k = 1. - ((low - lo) / tall).clamp(0., 1.) * 0.75;
        let (sx, sz) = ([to(tri.p[0]), to(tri.p[1]), to(tri.p[2])], [tz(tri.p[0]), tz(tri.p[1]), tz(tri.p[2])]);
        let area = (sx[1] - sx[0]) * (sz[2] - sz[0]) - (sx[2] - sx[0]) * (sz[1] - sz[0]);
        if area.abs() < 1e-6 {
            continue;
        }
        let x0 = sx.iter().cloned().fold(f32::MAX, f32::min).floor().max(0.) as usize;
        let x1 = (sx.iter().cloned().fold(f32::MIN, f32::max).ceil() as isize).min(SH as isize - 1);
        let z0 = sz.iter().cloned().fold(f32::MAX, f32::min).floor().max(0.) as usize;
        let z1 = (sz.iter().cloned().fold(f32::MIN, f32::max).ceil() as isize).min(SH as isize - 1);
        if x1 < 0 || z1 < 0 {
            continue;
        }
        for z in z0..=z1 as usize {
            let pz = z as f32 + 0.5;
            for x in x0..=x1 as usize {
                let px = x as f32 + 0.5;
                let w0 = ((sx[1] - px) * (sz[2] - pz) - (sx[2] - px) * (sz[1] - pz)) / area;
                let w1 = ((sx[2] - px) * (sz[0] - pz) - (sx[0] - px) * (sz[2] - pz)) / area;
                if w0 < 0. || w1 < 0. || w0 + w1 > 1. {
                    continue;
                }
                let at = z * SH + x;
                map[at] = map[at].max(k);
            }
        }
    }
    // Softened: a few passes of a small box, each way.
    for _ in 0..3 {
        let mut next = vec![0f32; SH * SH];
        for z in 0..SH {
            for x in 0..SH {
                let (mut sum, mut cnt) = (0., 0.);
                for dz in -3i32..=3 {
                    for dx in -3i32..=3 {
                        let (xx, zz) = (x as i32 + dx, z as i32 + dz);
                        if xx >= 0 && zz >= 0 && (xx as usize) < SH && (zz as usize) < SH {
                            sum += map[zz as usize * SH + xx as usize];
                        }
                        cnt += 1.;
                    }
                }
                next[z * SH + x] = sum / cnt;
            }
        }
        map = next;
    }
    map
}

/// A plain avatar for checking the page without Roblox (a hidden shot, BLOXKEEP_SHOT_MODEL=blocks): six boxes in the
/// classic colours, standing on the floor and facing -Z.
#[cfg(any(feature = "shot", feature = "core"))]
pub fn blocks() -> Option<Model> {
    let mut obj = String::new();
    let mut mtl = String::new();
    let mut base = 0;
    let parts: [(&str, V3, V3, [f32; 3]); 6] = [
        ("head", [-0.6, 4.5, -0.6], [0.6, 5.7, 0.6], [0.96, 0.8, 0.29]),
        ("torso", [-1., 2., -0.5], [1., 4.5, 0.5], [0.05, 0.41, 0.67]),
        ("larm", [-2., 2., -0.5], [-1., 4.5, 0.5], [0.96, 0.8, 0.29]),
        ("rarm", [1., 2., -0.5], [2., 4.5, 0.5], [0.96, 0.8, 0.29]),
        ("lleg", [-1., 0., -0.5], [0., 2., 0.5], [0.64, 0.75, 0.29]),
        ("rleg", [0., 0., -0.5], [1., 2., 0.5], [0.64, 0.75, 0.29]),
    ];
    for (name, lo, hi, kd) in parts {
        mtl.push_str(&format!("newmtl {name}\nKd {} {} {}\n", kd[0], kd[1], kd[2]));
        // (Named as Roblox names a body part's group, so it moves as one.)
        obj.push_str(&format!("g Player{}\nusemtl {name}\n", base / 8 + 1));
        for i in 0..8 {
            let x = if i & 1 == 0 { lo[0] } else { hi[0] };
            let y = if i & 2 == 0 { lo[1] } else { hi[1] };
            let z = if i & 4 == 0 { lo[2] } else { hi[2] };
            obj.push_str(&format!("v {x} {y} {z}\n"));
        }
        for q in [[0, 1, 3, 2], [4, 6, 7, 5], [0, 4, 5, 1], [2, 3, 7, 6], [0, 2, 6, 4], [1, 5, 7, 3]] {
            obj.push_str(&format!("f {} {} {} {}\n", base + q[0] + 1, base + q[1] + 1, base + q[2] + 1, base + q[3] + 1));
        }
        base += 8;
    }
    Model::parse(&obj, &mtl, &HashMap::new(), [0.; 3], [0.; 3], [0.; 3])
}
