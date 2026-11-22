// An avatar with depth: Roblox's own model of it (the mesh, its materials and their pictures, which its site turns
// with three.js), drawn here into a picture from any side, standing in a room. No graphics card is asked: a few
// thousand triangles into a square is a few milliseconds of plain arithmetic on a thread of its own.
//
// The model is Roblox's "avatar-3d" thumbnail (roblox::solid): a Wavefront OBJ in the world's own coordinates (Y up,
// right-handed, the avatar facing -Z), an MTL whose map_Kd names are the pictures (map_d the see-through parts), and
// the camera Roblox would look through. The camera here starts where Roblox's does and circles the model: the avatar
// and the floor it stands on turn together (yaw), the camera rises or sinks (pitch) and comes closer (zoom, nearer
// the head the closer it comes).
use std::{collections::HashMap, sync::Arc};

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
    /// What it does besides (gestures, looking, sitting…).
    pub act: Act,
    /// The animation pack it wears (Roblox's assets, 0 for none, their motions kept by `keep_motion`): idle, walk,
    /// jump, fall.
    pub pack: [i64; 4],
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
    /// (Shared: the materials of one model, and its mask, mostly name the same picture, and each kept its own copy:
    /// a model held 9.5 MB of pictures, the owner's error.log of alpha.15, 2026-11-22.)
    tex: Option<Arc<Tex>>,
    /// The see-through map (map_d): its alpha, or its grey where it has none.
    mask: Option<Arc<Tex>>,
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
    /// By OBJ group, a worn thing that hangs (long hair below the neck, a cape): the point it hangs from, how far it
    /// hangs below it (it bends: nothing at that point, all of the swing at its bottom) and how
    /// much it swings (0 for a small thing, a hat; 1 for one as long as the head is high and more).
    hang: Vec<Option<(V3, f32, f32)>>,
}

/// What the avatar does besides standing and walking, as avatar.rs drives it: a gesture under way, where it looks,
/// sitting, what hangs from it swung out of place, its mood, and a worn thing it is shown.
#[derive(Clone, Copy, PartialEq, Default)]
pub struct Act {
    /// The gesture (WAVE…) and how far into it, in seconds.
    pub kind: u8,
    pub g: f32,
    /// Where the mouse is over the box (-1 to 1 each way: right, up) and how much it looks there (0 to 1).
    pub mouse: [f32; 2],
    pub mouse_w: f32,
    /// Sat on the floor: 0 standing, 1 sat.
    pub sit: f32,
    /// What hangs from it swung out of place (radians: to its side, to its front).
    pub swing: [f32; 2],
    /// Sleepy (late at night), 0 to 1.
    pub sleepy: f32,
    /// A worn thing it is shown (the mouse on it in the list): the head's turn and nod (down +), a bow, a twist of the
    /// body, an arm raised (+ its right, - its left); and how much, 0 to 1.
    pub focus: [f32; 5],
    pub focus_w: f32,
    /// The emote played (EMOTE): its asset on Roblox, whose motion is kept (`keep_motion`).
    pub emote: i64,
}

/// The gestures: a wave, both arms up, a look down at itself and a turn to show a new thing, a stretch, a yawn, a dance.
pub const WAVE: u8 = 1;
pub const CHEER: u8 = 2;
pub const ADMIRE: u8 = 3;
pub const STRETCH: u8 = 4;
pub const YAWN: u8 = 5;
pub const DANCE: u8 = 6;
/// One of the account's own emotes, as Roblox plays it (its motion read from Roblox: `motion_of`).
pub const EMOTE: u8 = 7;

/// How long a gesture lasts, in seconds.
pub fn gesture_length(kind: u8) -> f32 {
    match kind {
        WAVE => 2.4,
        CHEER => 1.5,
        ADMIRE => 2.9,
        STRETCH => 3.4,
        YAWN => 3.0,
        DANCE => 3.8,
        _ => 0.,
    }
}

/// How long the gesture under way lasts (an emote: its motion's length; 0 when its motion isn't here).
pub fn act_length(a: &Act) -> f32 {
    if a.kind == EMOTE { motion(a.emote).map_or(0., |m| m.length) } else { gesture_length(a.kind) }
}

/// An emote's motion, from Roblox's KeyframeSequence: for each joint that moves the parts of this rig (the lower and
/// upper torso, the head, each upper arm and upper leg; R6's Torso and limbs too) its turns and shifts at the
/// keyframes' times. The hands, lower arms and lower legs are left out: here an arm or a leg is one piece.
pub struct Motion {
    pub length: f32,
    tracks: [Vec<(f32, [f32; 4], V3)>; 7],
}

/// A turn as a unit quaternion (w, x, y, z), and back.
fn quat(m: &M3) -> [f32; 4] {
    let t = m[0][0] + m[1][1] + m[2][2];
    let q = if t > 0. {
        let s = (t + 1.).sqrt() * 2.;
        [0.25 * s, (m[2][1] - m[1][2]) / s, (m[0][2] - m[2][0]) / s, (m[1][0] - m[0][1]) / s]
    } else if m[0][0] > m[1][1] && m[0][0] > m[2][2] {
        let s = (1. + m[0][0] - m[1][1] - m[2][2]).sqrt() * 2.;
        [(m[2][1] - m[1][2]) / s, 0.25 * s, (m[0][1] + m[1][0]) / s, (m[0][2] + m[2][0]) / s]
    } else if m[1][1] > m[2][2] {
        let s = (1. + m[1][1] - m[0][0] - m[2][2]).sqrt() * 2.;
        [(m[0][2] - m[2][0]) / s, (m[0][1] + m[1][0]) / s, 0.25 * s, (m[1][2] + m[2][1]) / s]
    } else {
        let s = (1. + m[2][2] - m[0][0] - m[1][1]).sqrt() * 2.;
        [(m[1][0] - m[0][1]) / s, (m[0][2] + m[2][0]) / s, (m[1][2] + m[2][1]) / s, 0.25 * s]
    };
    let l = q.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
    q.map(|x| x / l)
}

fn mat(q: [f32; 4]) -> M3 {
    let [w, x, y, z] = q;
    [
        [1. - 2. * (y * y + z * z), 2. * (x * y - z * w), 2. * (x * z + y * w)],
        [2. * (x * y + z * w), 1. - 2. * (x * x + z * z), 2. * (y * z - x * w)],
        [2. * (x * z - y * w), 2. * (y * z + x * w), 1. - 2. * (x * x + y * y)],
    ]
}

/// Between two turns, `k` of the way (the short way round).
fn nlerp(a: [f32; 4], b: [f32; 4], k: f32) -> [f32; 4] {
    let sign = if a.iter().zip(&b).map(|(x, y)| x * y).sum::<f32>() < 0. { -1. } else { 1. };
    let q = [0, 1, 2, 3].map(|i| a[i] * (1. - k) + b[i] * sign * k);
    let l = q.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
    q.map(|x| x / l)
}

/// Turn `a` brought `k` of the way to `b`.
fn mix(a: &M3, b: &M3, k: f32) -> M3 {
    if k <= 0. {
        *a
    } else {
        mat(nlerp(quat(a), quat(b), k.min(1.)))
    }
}

const IDENTITY: M3 = [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]];

/// An emote's motion from its KeyframeSequence file; None when it has no keyframes of the joints above.
pub fn motion_of(bytes: &[u8]) -> Option<Motion> {
    let doc = crate::rbxm::read(bytes).ok()?;
    let mut tracks: [Vec<(f32, [f32; 4], V3)>; 7] = Default::default();
    for p in doc.all("Pose") {
        if doc.float(p, "Weight").is_some_and(|w| w <= 0.) {
            continue;
        }
        let Some(name) = doc.text(p, "Name") else { continue };
        let track = match name {
            b"LowerTorso" | b"Torso" => 0,
            b"UpperTorso" => 1,
            b"Head" => 2,
            b"LeftUpperArm" | b"Left Arm" => 3,
            b"RightUpperArm" | b"Right Arm" => 4,
            b"LeftUpperLeg" | b"Left Leg" => 5,
            b"RightUpperLeg" | b"Right Leg" => 6,
            _ => continue,
        };
        // (Its keyframe: up the tree of poses.)
        let mut at = p;
        let mut time = None;
        for _ in 0..64 {
            let Some(&up) = doc.parent.get(&at) else { break };
            if doc.class.get(&up).is_some_and(|c| c == "Keyframe") {
                time = Some(doc.float(up, "Time").unwrap_or(0.));
                break;
            }
            at = up;
        }
        let (Some(time), Some((r, shift))) = (time, doc.frame(p, "CFrame")) else { continue };
        let m: M3 = [[r[0], r[1], r[2]], [r[3], r[4], r[5]], [r[6], r[7], r[8]]];
        tracks[track].push((time, quat(&m), shift));
    }
    for t in &mut tracks {
        t.sort_by(|a, b| a.0.total_cmp(&b.0));
    }
    let length = tracks.iter().filter_map(|t| t.last().map(|k| k.0)).fold(0f32, f32::max);
    (length > 0.05 && tracks.iter().any(|t| t.len() > 1)).then_some(Motion { length, tracks })
}

impl Motion {
    /// Each joint's turn and shift at `t` seconds (between the keyframes either side).
    fn at(&self, t: f32) -> [(M3, V3); 7] {
        std::array::from_fn(|i| {
            let k = &self.tracks[i];
            if k.is_empty() {
                return (IDENTITY, [0.; 3]);
            }
            let n = k.partition_point(|x| x.0 <= t);
            if n == 0 {
                return (mat(k[0].1), k[0].2);
            }
            if n >= k.len() {
                let l = &k[k.len() - 1];
                return (mat(l.1), l.2);
            }
            let (a, b) = (&k[n - 1], &k[n]);
            let f = ((t - a.0) / (b.0 - a.0).max(1e-6)).clamp(0., 1.);
            (mat(nlerp(a.1, b.1, f)), add(mul(a.2, 1. - f), mul(b.2, f)))
        })
    }
}

static MOTIONS: std::sync::Mutex<Vec<(i64, std::sync::Arc<Motion>)>> = std::sync::Mutex::new(Vec::new());

/// Keeps an emote's motion for `motion` (the last 16 played).
pub fn keep_motion(emote: i64, m: Motion) {
    use crate::threads::Hold;
    let mut l = MOTIONS.hold();
    l.retain(|(id, _)| *id != emote);
    if l.len() >= 16 {
        l.remove(0);
    }
    l.push((emote, std::sync::Arc::new(m)));
}

/// An emote's motion, when kept.
pub fn motion(emote: i64) -> Option<std::sync::Arc<Motion>> {
    use crate::threads::Hold;
    MOTIONS.hold().iter().find(|(id, _)| *id == emote).map(|(_, m)| m.clone())
}

/// How far a gesture has turned the avatar about (ADMIRE turns it once round to show a new thing), in radians.
pub fn twirl(a: &Act) -> f32 {
    if a.kind == ADMIRE { std::f32::consts::TAU * ease((a.g - 1.1) / 1.25) } else { 0. }
}

/// Eased 0 to 1 (and held at either end).
fn ease(x: f32) -> f32 {
    let x = x.clamp(0., 1.);
    x * x * (3. - 2. * x)
}

/// How much of a gesture shows at `g` seconds: in over `inn`, held until `hold`, out by `end`.
fn env(g: f32, inn: f32, hold: f32, end: f32) -> f32 {
    if g < 0. {
        0.
    } else if g < inn {
        ease(g / inn)
    } else if g < hold {
        1.
    } else {
        1. - ease((g - hold) / (end - hold).max(1e-3))
    }
}

/// The parts as posed: for each (the body, the head, each arm, each leg) the point it turns about, the turn, and how
/// far it then moves; how far everything sinks (the walk's spread legs); and a whole turn of it about its upright
/// axis (a gesture's twirl).
struct Posed {
    bones: [Option<(V3, M3, V3)>; 6],
    sink: f32,
    twirl: f32,
}

impl Rig {
    /// The pose at this moment: the idle (breathing, looking round now and then, the weight moving from foot to foot),
    /// the walk, and what `act` adds; `look` the way to the mouse from the head, in the model's own frame.
    ///
    /// The parts hang together: the head and the arms go with the body (its bow, twist, lean and shift), the legs hold
    /// their feet on the floor while the hips move over them, or swing from the hips (walking, sitting).
    fn pose(&self, v: &View, look: Option<V3>) -> Posed {
        let (t, a) = (v.t, &v.act);
        let up = [0., 1., 0.];
        let mut e = [0.; 3];
        e[self.side] = 1.;
        // The axis it faces along (the other level one): an arm turned about it swings out from the body.
        let mut fwd = [0.; 3];
        fwd[2 - self.side] = 1.;
        // Where it faces in its own frame (Roblox's models face -Z), and the turns that tip a part toward it (`fs`)
        // and lean the body toward its own +side (`rs`).
        let facing: V3 = if self.side == 0 { [0., 0., -1.] } else { [-1., 0., 0.] };
        let fs = if dot(mv(&rot(e, 0.1), [0., -1., 0.]), facing) >= 0. { 1. } else { -1. };
        let rs = if mv(&rot(fwd, 0.1), up)[self.side] >= 0. { 1. } else { -1. };
        let out = |p: V3| {
            let probe = mv(&rot(fwd, 0.1), [0., -1., 0.])[self.side];
            if probe * (p[self.side] - self.neck[self.side]) >= 0. { 1. } else { -1. }
        };
        let pace = v.pace;
        let still = 1. - pace;
        let sat = ease(a.sit);
        // Standing and doing nothing else: what the idle moves.
        let calm = still * (1. - sat);
        let sleepy = a.sleepy;
        let (mut hy, mut hn, mut hr) = (0f32, 0f32, 0f32);
        let (mut bow, mut twist, mut lean, mut shift, mut lift) = (0f32, 0f32, 0f32, 0f32, 0f32);
        let mut fw = [0f32; 2];
        let mut twirl = 0.;

        // It breathes (about 3 s a breath, slower when sleepy): the arms open a little from the body and close, the
        // head lifts a touch. The body stands straight; it was sheared from side to side over its planted feet and
        // leaned as if about to fall (the owner, 2026-11-20).
        let breath = (t * (2.1 - 0.7 * sleepy)).sin();
        // It looks round now and then, as someone waiting does (`glance`).
        let (gaze, tilt) = glance(t);
        let looks = (0.4 + 0.6 * still) * (1. - 0.6 * sleepy);
        hy += looks * (gaze + 0.025 * (t * 0.9).sin());
        hn += -looks * 0.03 * breath + 0.13 * sleepy;
        hr += still * tilt;
        let open = still * (0.045 + 0.03 * breath) * (1. - 0.4 * sleepy);
        let mut ow = [open, open];
        fw[0] += still * 0.04 * (t * 1.05).sin();
        fw[1] += still * 0.04 * (t * 1.05 + 1.9).sin();
        // Its weight goes from one foot to the other and back, slowly (about 15 s): the hips move over the feet, the
        // body leans the other way a little to stay over them, the head stays level.
        let sway = (t * 0.42).sin();
        shift += calm * self.leg * 0.045 * sway;
        lean -= calm * 0.03 * sway;
        hr += calm * 0.025 * sway;

        // Walking: the legs step against each other, each arm swings against its own side's leg.
        let step = pace * v.stride.sin();
        fw[0] -= 0.55 * step;
        fw[1] += 0.55 * step;
        let swing_leg = 0.5 * step;

        // The mouse over the box: it looks at it, the body turning a little with the head; not behind it.
        if let Some(d) = look.filter(|_| a.mouse_w > 0.) {
            let flat = [d[0], 0., d[2]];
            let yaw = cross(facing, flat)[1].atan2(dot(facing, flat));
            let nod = -(d[1] / dot(d, d).sqrt().max(1e-6)).asin();
            let w = a.mouse_w * still * ((2.0 - yaw.abs()) / 0.8).clamp(0., 1.);
            let body = (yaw * 0.25).clamp(-0.25, 0.25);
            twist += w * body;
            hy += w * ((yaw - body).clamp(-1.05, 1.05) - hy);
            hn += w * (nod.clamp(-0.4, 0.45) - hn);
            hr *= 1. - 0.5 * w;
        }

        // A worn thing it is shown: it looks at where it wears it (up at a hat, down at shoes, over its shoulder at
        // what is on its back), an arm out for what is worn on an arm.
        if a.focus_w > 0. {
            let w = a.focus_w * still;
            let f = a.focus;
            hy += w * (f[0] - hy);
            hn += w * (f[1] - hn);
            bow += w * f[2];
            twist += w * f[3];
            let arm = usize::from(f[4] > 0.);
            ow[arm] += w * f[4].abs();
        }

        // The gestures, over the idle.
        let g = a.g;
        let gw = still;
        match a.kind {
            WAVE => {
                let w = gw * env(g, 0.35, 1.9, 2.4);
                ow[1] += w * (2.5 + 0.3 * ((g - 0.35).max(0.) * 11.).sin());
                fw[1] += w * 0.25;
                hr += w * 0.12;
                hn -= w * 0.06;
            }
            CHEER => {
                let w = gw * env(g, 0.2, 1.0, 1.5);
                ow[0] += w * 2.8;
                ow[1] += w * 2.8;
                bow -= w * 0.1;
                hn -= w * 0.25;
                lift += w * self.leg * 0.14 * (std::f32::consts::PI * (g / 0.95).clamp(0., 1.)).sin();
            }
            ADMIRE => {
                // A look down at itself, arms a little out, then a turn about to show it all, and a look up.
                let down = gw * env(g, 0.4, 1.0, 1.4);
                hn += down * 0.45;
                bow += down * 0.1;
                ow[0] += down * 0.35;
                ow[1] += down * 0.35;
                twirl = self::twirl(a);
                let after = gw * env(g - 2.35, 0.15, 0.3, 0.55);
                hn -= after * 0.15;
                hr += after * 0.1;
            }
            STRETCH => {
                let w = gw * env(g, 0.9, 2.2, 3.4);
                ow[0] += w * 2.75;
                ow[1] += w * 2.75;
                fw[0] += w * 0.15;
                fw[1] += w * 0.15;
                bow -= w * 0.14;
                hn -= w * 0.3;
                lift += w * self.leg * 0.03;
            }
            YAWN => {
                let w = gw * env(g, 0.6, 1.9, 3.0);
                hn -= w * 0.4;
                hr += w * 0.1;
                fw[1] += w * 2.3;
                ow[1] -= w * 0.5;
                ow[0] += w * 0.15;
                bow -= w * 0.06;
            }
            DANCE => {
                let w = gw * env(g, 0.3, 3.3, 3.8);
                let beat = g * 7.;
                fw[0] += w * (1.0 + 0.7 * beat.sin());
                fw[1] += w * (1.0 - 0.7 * beat.sin());
                ow[0] += w * 0.35;
                ow[1] += w * 0.35;
                twist += w * 0.3 * (beat / 2.).sin();
                shift += w * self.leg * 0.06 * (beat / 2.).sin();
                lift += w * self.leg * 0.05 * beat.sin().abs();
                hn += w * 0.12 * beat.sin();
            }
            _ => {}
        }

        // Sitting on the floor: the legs out in front from the hips, the body down onto the floor, leaning back on its
        // hands a little.
        lift -= sat * self.leg * 0.78;
        bow -= sat * 0.12;
        fw[0] -= sat * 0.45;
        fw[1] -= sat * 0.45;
        ow[0] += sat * 0.2;
        ow[1] += sat * 0.2;

        // The body turns about its hips' middle.
        let hip_y = {
            let ys: Vec<f32> = self.hips.iter().flatten().map(|h| h[1]).collect();
            if ys.is_empty() { self.neck[1] - self.leg } else { ys.iter().sum::<f32>() / ys.len() as f32 }
        };
        let root = [self.neck[0], hip_y, self.neck[2]];
        let mut body = mm(&rot(up, twist), &mm(&rot(e, -fs * bow), &rot(fwd, rs * lean)));
        let mut moved = add(mul(e, shift), [0., lift, 0.]);
        // Roblox's own motions, each eased in over what is under it: the account's animation pack as it wears it (its
        // idle while it stands, its walk while it walks, its jump going up and its fall coming down), then an emote.
        // Each layer: every joint's turn and shift, how much of it over the body and limbs, and over the head (less
        // while it looks at the mouse or at a worn thing). Their joints turn in Roblox's frame, the model's own when
        // it faces -Z with its right at +X: a model turned the other way plays none.
        let mut layers: Vec<([(M3, V3); 7], f32, f32)> = Vec::new();
        if self.side == 0 {
            let aim = a.mouse_w.max(a.focus_w);
            let gesture = if a.kind != 0 && a.kind != EMOTE {
                let l = gesture_length(a.kind);
                env(g, 0.3, l - 0.3, l)
            } else {
                0.
            };
            let air = v.up > 0. || v.rise > 0.;
            if !air {
                if let Some(m) = motion(v.pack[0]) {
                    let w = calm * (1. - gesture);
                    layers.push((m.at(t.rem_euclid(m.length)), w, w * (1. - aim)));
                }
                if let Some(m) = motion(v.pack[1]) {
                    // (One walk round is one round of the stride.)
                    let at = (v.stride / std::f32::consts::TAU).rem_euclid(1.) * m.length;
                    layers.push((m.at(at), pace, pace));
                }
            } else if let Some(m) = motion(v.pack[if v.rise > 0. { 2 } else { 3 }]) {
                let at = if v.rise > 0. { 0.5 * m.length } else { t.rem_euclid(m.length) };
                layers.push((m.at(at), 1., 1.));
            }
            if a.kind == EMOTE {
                if let Some(m) = motion(a.emote) {
                    let w = still * (1. - sat) * env(g, 0.2, m.length - 0.25, m.length);
                    layers.push((m.at(g.min(m.length)), w, w));
                }
            }
            layers.retain(|l| l.1 > 0. || l.2 > 0.);
        }
        // (The lower torso's own turn, which the legs go with.)
        let mut lower = IDENTITY;
        for (j, w, _) in &layers {
            lower = mix(&lower, &j[0].0, *w);
            body = mix(&body, &mm(&j[0].0, &j[1].0), *w);
            moved = add(mul(moved, 1. - w), mul(add([0., lift.min(0.), 0.], j[0].1), *w));
        }
        // A part hung on the body: its own turn about `at`, then the body's.
        let on_body = |at: V3, own: M3| -> (V3, M3, V3) {
            let m = mm(&body, &own);
            let tr = add(sub(add(mv(&body, sub(at, root)), root), at), moved);
            (at, m, tr)
        };
        let mut head = mm(&rot(up, hy), &mm(&rot(e, -fs * hn), &rot(fwd, hr)));
        for (j, _, w) in &layers {
            head = mix(&head, &j[2].0, *w);
        }
        let arm = |i: usize| {
            self.shoulders[i].map(|p| {
                let mut own = mm(&rot(e, fs * fw[i]), &rot(fwd, out(p) * ow[i]));
                for (j, w, _) in &layers {
                    own = mix(&own, &j[3 + i].0, *w);
                }
                on_body(p, own)
            })
        };
        // A leg: standing, its foot stays where it is and its top goes with the hips; else it swings from the hip.
        let leg = |i: usize, swing: f32| {
            self.hips[i].map(|h| {
                if !layers.is_empty() {
                    // (With the lower torso, from the hip.)
                    let mut own = rot(e, swing + fs * 1.45 * sat);
                    for (j, w, _) in &layers {
                        own = mix(&own, &j[5 + i].0, *w);
                    }
                    let tr = add(sub(add(mv(&lower, sub(h, root)), root), h), moved);
                    return (h, mm(&lower, &own), tr);
                }
                if pace <= 0. && sat <= 0.001 {
                    let foot = [h[0], h[1] - self.leg, h[2]];
                    let tip = (shift / self.leg.max(1e-3)).clamp(-0.5, 0.5).asin();
                    (foot, rot(fwd, rs * tip), [0., lift, 0.])
                } else {
                    (h, rot(e, swing + fs * 1.45 * sat), moved)
                }
            })
        };
        let sink = if motion(v.pack[1]).is_some() && self.side == 0 { 0. } else { -self.leg * (1. - swing_leg.cos()) };
        Posed {
            bones: [Some((root, body, moved)), Some(on_body(self.neck, head)), arm(0), arm(1), leg(0, swing_leg), leg(1, -swing_leg)],
            sink,
            twirl,
        }
    }
}

/// Where the head looks at `t` seconds of the idle (a turn about the up axis), and how it tips as it turns: an 11 s
/// round of looking ahead, then a look to one side, held, a look to the other, held, and back, each move eased in and
/// out; every other round it looks the other way first.
fn glance(t: f32) -> (f32, f32) {
    const ROUND: f32 = 11.;
    let c = t.rem_euclid(ROUND);
    let first = if (t / ROUND).floor() as i64 % 2 == 0 { 1. } else { -1. };
    let ease = |x: f32| {
        let x = x.clamp(0., 1.);
        x * x * (3. - 2. * x)
    };
    // (from, to, start, length): the moves of a round; between them the head holds.
    let moves = [(0., 0.36, 5.0, 0.7), (0.36, -0.28, 7.2, 0.9), (-0.28, 0., 9.4, 0.8)];
    let mut at = 0.;
    for (from, to, start, len) in moves {
        if c >= start {
            at = from + (to - from) * ease((c - start) / len);
        }
    }
    let gaze = first * at;
    (gaze, -0.12 * gaze)
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
    // A worn thing goes with the head when it reaches over the head's middle, above it: a hat, glasses, hair. One that
    // also reaches well down below the neck goes with it only when it covers the top of the head (long hair): a scarf
    // reaches just over the head's middle and turned with the head (the owner's avatar, 2026-11-20).
    let hc = c(head);
    let reach = (hi[head][0] - lo[head][0]).max(hi[head][2] - lo[head][2]) * 1.5;
    let head_h = hi[head][1] - lo[head][1];
    for g in (0..n).filter(|&g| used(g) && !body.contains(&g)) {
        let k = c(g);
        let near = (k[0] - hc[0]).abs() < reach && (k[2] - hc[2]).abs() < reach;
        let on_head = lo[g][1] >= lo[head][1] - 0.05 * h || hi[g][1] >= hi[head][1] - 0.1 * head_h;
        if hi[g][1] > hc[1] && near && on_head {
            bone[g] = HEAD;
        }
    }
    for t in tris.iter_mut() {
        t.bone = bone[t.group as usize];
    }
    // What hangs swings: hair as far as it hangs below the neck, and a cape (a thin sheet behind the body). Glasses, a
    // hat, a scarf, wings or a sword on the back are held fast, as in Roblox: they swung about their tops and came off
    // the avatar (the owner's avatar, 2026-11-20).
    let neck_y = lo[head][1];
    let fb = 2 - side;
    let hang = (0..n)
        .map(|g| {
            if !used(g) || body.contains(&g) {
                return None;
            }
            let k = c(g);
            let (top, len) = if bone[g] == HEAD {
                (neck_y, neck_y - lo[g][1])
            } else {
                let deep = hi[g][fb] - lo[g][fb];
                let tall = hi[g][1] - lo[g][1];
                // (Behind: on the side away from where it faces, -Z, as Roblox's models do.)
                let behind = if side == 0 { k[2] > mid[2] } else { k[0] > mid[0] };
                if deep > 0.35 * tall || !behind {
                    return None;
                }
                (hi[g][1], tall)
            };
            let w = ((len - 0.1 * h) / (0.25 * h)).clamp(0., 1.);
            (w > 0.).then(|| ([k[0], top, k[2]], w, len.max(1e-3)))
        })
        .collect();
    Some(Rig { side, neck: [hc[0], lo[head][1], hc[2]], shoulders: [tops[0].map(shoulder), tops[1].map(shoulder)], hips: [hips[0].map(hip), hips[1].map(hip)], leg, hang })
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
        // Each picture read once, however many materials name it.
        let mut read: HashMap<String, Option<Arc<Tex>>> = HashMap::new();
        let mut picture = |name: &str| -> Option<Arc<Tex>> {
            read.entry(name.to_string()).or_insert_with(|| pictures.get(name).and_then(|b| Tex::of(b)).map(Arc::new)).clone()
        };
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
                    let mask = picture(&name).map(|t| {
                        // A map of its own with no alpha says it in grey (a copy: the picture may be a colour too).
                        if !own && t.rgba.chunks(4).all(|p| p[3] == 255) {
                            let mut g = (*t).clone();
                            for p in g.rgba.chunks_mut(4) {
                                p[3] = p[0];
                            }
                            Arc::new(g)
                        } else {
                            t
                        }
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
        // A picture several body parts share is repainted once, and they keep sharing it.
        let mut done: Vec<(*const Tex, Arc<Tex>)> = Vec::new();
        let near = |r: i32, g: i32, b: i32| from.iter().any(|f| (f[0] as i32 - r).abs() <= 6 && (f[1] as i32 - g).abs() <= 6 && (f[2] as i32 - b).abs() <= 6);
        for &i in &self.body {
            let Some(mat) = m.mats.get_mut(i) else { continue };
            if near((mat.kd[0] * 255.).round() as i32, (mat.kd[1] * 255.).round() as i32, (mat.kd[2] * 255.).round() as i32) {
                mat.kd = [to[0] as f32 / 255., to[1] as f32 / 255., to[2] as f32 / 255.];
            }
            if let Some(t) = mat.tex.as_mut() {
                let was = Arc::as_ptr(t);
                if let Some((_, new)) = done.iter().find(|d| d.0 == was) {
                    *t = new.clone();
                    continue;
                }
                let mut new = (**t).clone();
                for p in new.rgba.chunks_mut(4) {
                    if p[3] > 0 && near(p[0] as i32, p[1] as i32, p[2] as i32) {
                        p[..3].copy_from_slice(&to);
                    }
                }
                *t = Arc::new(new);
                done.push((was, t.clone()));
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
        let turn = |p: V3, s: f32, c: f32| -> V3 { [p[0] * c + p[2] * s, p[1], -p[0] * s + p[2] * c] };
        // Where the mouse is, as a way to look from the head in the model's own frame: at the point under the mouse
        // as deep as the head is, and toward the viewer (the mouse on the head: it looks out at you).
        let look = self.rig.as_ref().filter(|_| view.act.mouse_w > 0.).map(|r| {
            let (s0, c0) = (view.yaw + view.heading).sin_cos();
            let head = add(turn(sub(r.neck, self.center), s0, c0), self.center);
            let zh = dot(sub(head, eye), f).max(1e-3);
            let ray = add(add(f, mul(s, view.act.mouse[0] * t)), mul(u, view.act.mouse[1] * t));
            let w = add(sub(add(eye, mul(ray, zh)), head), mul(norm(sub(eye, head)), self.radius * 0.9));
            turn(w, -s0, c0)
        });
        // Each part's turn at this moment.
        let moving = view.t > 0. || view.pace > 0. || view.up > 0. || view.rise > 0. || view.act != Act::default();
        let posed = self.rig.as_ref().filter(|_| moving).map(|r| r.pose(&view, look));
        let sink = posed.as_ref().map_or(0., |p| p.sink) + view.up.max(0.);
        // What hangs from it, swung out of place.
        let hang = self.rig.as_ref().filter(|_| view.act.swing != [0., 0.]).map(|r| {
            let (mut e, mut fwd) = ([0.; 3], [0.; 3]);
            e[r.side] = 1.;
            fwd[2 - r.side] = 1.;
            // (Its front swing turns about `e` the way that takes the bottom forward, as the pose's `fs`.)
            let facing: V3 = if r.side == 0 { [0., 0., -1.] } else { [-1., 0., 0.] };
            let fs = if dot(mv(&rot(e, 0.1), [0., -1., 0.]), facing) >= 0. { 1. } else { -1. };
            (r, [view.act.swing[0], fs * view.act.swing[1]], e, fwd)
        });
        // The avatar faces where it walked (and turns about in a gesture); the floor turns only with the camera.
        let (hs, hc) = (view.yaw + view.heading + posed.as_ref().map_or(0., |p| p.twirl)).sin_cos();
        let face = |p: V3| -> V3 { turn(p, hs, hc) };
        let mut shown: Vec<Shown> = Vec::with_capacity(self.tris.len());
        let mut transparent = Vec::new();
        for tri in &self.tris {
            let (mut tp, mut tn) = (tri.p, tri.n);
            if let Some((r, sw, e, fwd)) = hang.as_ref() {
                if let Some(Some((at, w, len))) = r.hang.get(tri.group as usize) {
                    // (It bends: the further below where it hangs from, the more it swings; above it, not at all.)
                    let mut bent = [IDENTITY; 3];
                    for k in 0..3 {
                        let f = w * ((at[1] - tp[k][1]) / len).clamp(0., 1.);
                        bent[k] = mm(&rot(*fwd, sw[0] * f), &rot(*e, sw[1] * f));
                    }
                    for k in 0..3 {
                        tp[k] = add(mv(&bent[k], sub(tp[k], *at)), *at);
                        tn[k] = mv(&bent[k], tn[k]);
                    }
                }
            }
            if let Some((at, m, tr)) = posed.as_ref().and_then(|p| p.bones[tri.bone as usize].as_ref()) {
                tp = tp.map(|p| add(add(mv(m, sub(p, *at)), *at), *tr));
                tn = tn.map(|v| mv(m, v));
            }
            let tp = tp.map(|mut p| {
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
                let normal = tn[k];
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
