// An avatar with depth: Roblox's own model of it (the mesh, its materials and their pictures, which its site turns
// with three.js), drawn here into a picture from any side. No graphics card is asked: a few thousand triangles into a
// small square is a few milliseconds of plain arithmetic on a thread of its own.
//
// The model is Roblox's "avatar-3d" thumbnail (roblox::solid): a Wavefront OBJ in the world's own coordinates (Y up,
// right-handed, the avatar facing -Z), an MTL whose map_Kd names are the pictures, and the camera Roblox would look
// through. The camera here stands where Roblox's does and looks at the middle of the model, which turns about its
// own upright axis.
use std::collections::HashMap;

type V3 = [f32; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn norm(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    if l > 1e-9 { [a[0] / l, a[1] / l, a[2] / l] } else { [0., 1., 0.] }
}

struct Tex {
    w: usize,
    h: usize,
    rgb: Vec<u8>,
}

impl Tex {
    /// The colour at a place on the picture (0..1 each way, repeating), between its four nearest dots.
    fn at(&self, u: f32, v: f32) -> [f32; 3] {
        let x = (u - u.floor()) * self.w as f32 - 0.5;
        let y = (1. - (v - v.floor())) * self.h as f32 - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let px = |xi: f32, yi: f32| {
            let xi = (xi as isize).rem_euclid(self.w as isize) as usize;
            let yi = (yi as isize).clamp(0, self.h as isize - 1) as usize;
            let i = (yi * self.w + xi) * 3;
            [self.rgb[i] as f32, self.rgb[i + 1] as f32, self.rgb[i + 2] as f32]
        };
        let (a, b, c, d) = (px(x0, y0), px(x0 + 1., y0), px(x0, y0 + 1.), px(x0 + 1., y0 + 1.));
        let mut out = [0.; 3];
        for k in 0..3 {
            out[k] = (a[k] * (1. - fx) + b[k] * fx) * (1. - fy) + (c[k] * (1. - fx) + d[k] * fx) * fy;
        }
        out
    }
}

struct Mat {
    kd: [f32; 3],
    tex: Option<Tex>,
}

struct Tri {
    p: [V3; 3],
    uv: [[f32; 2]; 3],
    n: [V3; 3],
    mat: usize,
}

pub struct Model {
    tris: Vec<Tri>,
    mats: Vec<Mat>,
    /// The model's middle and the reach of its farthest corner from it, and where the camera stands.
    center: V3,
    radius: f32,
    eye: V3,
}

impl Model {
    /// Reads the OBJ and its MTL. `pictures` are the MTL's pictures by the names it gives them; `eye` is the camera's
    /// place and `min`/`max` the model's box, as Roblox says them. None when there is nothing to draw.
    pub fn parse(obj: &str, mtl: &str, pictures: &HashMap<String, Vec<u8>>, eye: V3, min: V3, max: V3) -> Option<Model> {
        let mut mats: Vec<Mat> = Vec::new();
        let mut names: HashMap<String, usize> = HashMap::new();
        for line in mtl.lines() {
            let mut it = line.split_whitespace();
            match it.next() {
                Some("newmtl") => {
                    names.insert(it.next().unwrap_or_default().to_string(), mats.len());
                    mats.push(Mat { kd: [0.8; 3], tex: None });
                }
                Some("Kd") => {
                    let v: Vec<f32> = it.filter_map(|t| t.parse().ok()).collect();
                    if let (Some(m), 3) = (mats.last_mut(), v.len()) {
                        m.kd = [v[0], v[1], v[2]];
                    }
                }
                Some("map_Kd") => {
                    // (The name is the last word: options may stand before it. A path's last part is the name.)
                    let name = it.last().unwrap_or_default();
                    let name = name.rsplit(['/', '\\']).next().unwrap_or(name);
                    let tex = pictures.get(name).and_then(|b| image::load_from_memory(b).ok()).map(|i| {
                        let i = i.to_rgb8();
                        Tex { w: i.width() as usize, h: i.height() as usize, rgb: i.into_raw() }
                    });
                    if let Some(m) = mats.last_mut() {
                        m.tex = tex.filter(|t| t.w > 0 && t.h > 0);
                    }
                }
                _ => {}
            }
        }
        if mats.is_empty() {
            mats.push(Mat { kd: [0.8; 3], tex: None });
        }
        let (mut v, mut vt, mut vn): (Vec<V3>, Vec<[f32; 2]>, Vec<V3>) = (Vec::new(), Vec::new(), Vec::new());
        let mut tris: Vec<Tri> = Vec::new();
        let mut mat = 0;
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
                    for k in 1..corners.len().saturating_sub(1) {
                        let (a, b, c) = (corners[0], corners[k], corners[k + 1]);
                        let flat = norm(cross(sub(b.0, a.0), sub(c.0, a.0)));
                        tris.push(Tri { p: [a.0, b.0, c.0], uv: [a.1, b.1, c.1], n: [a.2.unwrap_or(flat), b.2.unwrap_or(flat), c.2.unwrap_or(flat)], mat });
                    }
                }
                _ => {}
            }
        }
        if tris.is_empty() {
            return None;
        }
        // Roblox's box when it gives one; else the model's own.
        let (mut lo, mut hi) = (min, max);
        if (0..3).any(|k| hi[k] <= lo[k]) {
            lo = [f32::MAX; 3];
            hi = [f32::MIN; 3];
            for t in &tris {
                for p in t.p {
                    for k in 0..3 {
                        lo[k] = lo[k].min(p[k]);
                        hi[k] = hi[k].max(p[k]);
                    }
                }
            }
        }
        let center = [(lo[0] + hi[0]) / 2., (lo[1] + hi[1]) / 2., (lo[2] + hi[2]) / 2.];
        let half = sub(hi, center);
        let radius = dot(half, half).sqrt().max(0.01);
        // A camera inside the model (or none given) stands back in front of it instead (the avatar faces -Z).
        // (So does one straight over or under it, which has no upright to hold on to.)
        let away = sub(eye, center);
        let far = dot(away, away).sqrt();
        let level = far > 0. && (away[1] / far).abs() < 0.95;
        let eye = if eye == [0.; 3] || far < radius * 1.5 || !level { [center[0], center[1] + radius * 0.25, center[2] - radius * 3.6] } else { eye };
        Some(Model { tris, mats, center, radius, eye })
    }

    /// The model turned by `yaw` (radians, about its upright axis) and drawn into a square of `size` dots: BGRA, clear
    /// where the model isn't. Each dot is the mean of four, so edges come out smooth.
    pub fn draw(&self, yaw: f32, size: usize) -> Vec<u8> {
        const SS: usize = 2;
        let n = size * SS;
        let mut depth = vec![f32::MAX; n * n];
        let mut col = vec![[0u8; 3]; n * n];
        // The camera: looking from its place at the model's middle, the world's up being up.
        let f = norm(sub(self.center, self.eye));
        let s = norm(cross(f, [0., 1., 0.]));
        let u = cross(s, f);
        let d = dot(sub(self.center, self.eye), f).max(self.radius * 1.2);
        // Wide enough that the model fits whichever way it is turned.
        let t = self.radius * 1.04 / (d * d - self.radius * self.radius).max(1e-6).sqrt();
        let (sin, cos) = yaw.sin_cos();
        let turn = |p: V3| -> V3 { [p[0] * cos + p[2] * sin, p[1], -p[0] * sin + p[2] * cos] };
        // The light: from over the viewer's left shoulder.
        let light = norm([-f[0] * 0.75 + u[0] * 0.55 - s[0] * 0.35, -f[1] * 0.75 + u[1] * 0.55 - s[1] * 0.35, -f[2] * 0.75 + u[2] * 0.55 - s[2] * 0.35]);
        let half = n as f32 / 2.;
        for tri in &self.tris {
            let mut sx = [0f32; 3];
            let mut sy = [0f32; 3];
            let mut z = [0f32; 3];
            let mut nn = [[0f32; 3]; 3];
            let mut behind = false;
            for k in 0..3 {
                let w = sub(turn(sub(tri.p[k], self.center)), sub(self.eye, self.center));
                z[k] = dot(w, f);
                if z[k] < 0.05 {
                    behind = true;
                    break;
                }
                sx[k] = half + dot(w, s) / (z[k] * t) * half;
                sy[k] = half - dot(w, u) / (z[k] * t) * half;
                nn[k] = turn(tri.n[k]);
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
            if x1 < 0 || y1 < 0 {
                continue;
            }
            let m = &self.mats[tri.mat.min(self.mats.len() - 1)];
            let iz = [1. / z[0], 1. / z[1], 1. / z[2]];
            for y in y0..=y1 as usize {
                let py = y as f32 + 0.5;
                for x in x0..=x1 as usize {
                    let px = x as f32 + 0.5;
                    // How much of the dot belongs to each corner (all of one sign: the dot is inside).
                    let w0 = ((sx[1] - px) * (sy[2] - py) - (sx[2] - px) * (sy[1] - py)) / area;
                    let w1 = ((sx[2] - px) * (sy[0] - py) - (sx[0] - px) * (sy[2] - py)) / area;
                    let w2 = 1. - w0 - w1;
                    if w0 < 0. || w1 < 0. || w2 < 0. {
                        continue;
                    }
                    let inv = w0 * iz[0] + w1 * iz[1] + w2 * iz[2];
                    let zz = 1. / inv;
                    let at = y * n + x;
                    if zz >= depth[at] {
                        continue;
                    }
                    depth[at] = zz;
                    // (Through the depth, so a picture doesn't swim on a face seen at a slant.)
                    let (a, b, c) = (w0 * iz[0] * zz, w1 * iz[1] * zz, w2 * iz[2] * zz);
                    let base = match &m.tex {
                        Some(tex) => tex.at(a * tri.uv[0][0] + b * tri.uv[1][0] + c * tri.uv[2][0], a * tri.uv[0][1] + b * tri.uv[1][1] + c * tri.uv[2][1]),
                        None => [m.kd[0] * 255., m.kd[1] * 255., m.kd[2] * 255.],
                    };
                    let nrm = norm([a * nn[0][0] + b * nn[1][0] + c * nn[2][0], a * nn[0][1] + b * nn[1][1] + c * nn[2][1], a * nn[0][2] + b * nn[1][2] + c * nn[2][2]]);
                    let lit = 0.6 + 0.4 * dot(nrm, light).max(0.);
                    col[at] = [(base[0] * lit).min(255.) as u8, (base[1] * lit).min(255.) as u8, (base[2] * lit).min(255.) as u8];
                }
            }
        }
        let mut out = vec![0u8; size * size * 4];
        for y in 0..size {
            for x in 0..size {
                let (mut r, mut g, mut b, mut hit) = (0u32, 0u32, 0u32, 0u32);
                for dy in 0..SS {
                    for dx in 0..SS {
                        let at = (y * SS + dy) * n + x * SS + dx;
                        if depth[at] < f32::MAX {
                            r += col[at][0] as u32;
                            g += col[at][1] as u32;
                            b += col[at][2] as u32;
                            hit += 1;
                        }
                    }
                }
                if hit > 0 {
                    let o = (y * size + x) * 4;
                    out[o] = (b / hit) as u8;
                    out[o + 1] = (g / hit) as u8;
                    out[o + 2] = (r / hit) as u8;
                    out[o + 3] = (hit * 255 / (SS * SS) as u32) as u8;
                }
            }
        }
        out
    }
}
