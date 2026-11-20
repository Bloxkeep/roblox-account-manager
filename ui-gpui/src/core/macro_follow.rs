// MacroFollow.cs: the maths of the follow and clickColour steps.
use crate::core::base::{Fail, Json, Res};
use crate::core::win_kit::{Picture as Bitmap, Rect as Rectangle};
use crate::core::l;
use crate::core::macro_player::value_missing;
use crate::core::macro_values;

/// The colours a follow step keeps on its target: one marker colour, or a box's left and right edges.
#[derive(Clone, Debug, PartialEq)]
pub struct MacroMarker {
    pub colour: Option<String>,
    pub left: Option<String>,
    pub right: Option<String>,
}

impl MacroMarker {
    pub fn r#box(&self) -> bool {
        self.left.is_some()
    }
}

/// What a follow step saw in one frame of its area (pixels of the area): None where that colour wasn't there.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FollowSight {
    pub target: Option<f64>,
    pub marker: Option<f64>,
    pub left: Option<f64>,
    pub right: Option<f64>,
}

// The maths of the follow and clickColour steps, kept apart from the desktop so the checks run them on made-up
// pictures and sequences. Colours match when no channel differs by more than the tolerance.

/// The marker this close to the target (px) counts as on it: no press either way.
pub const DEAD_ZONE: f64 = 2.0;
/// Predicting looks this many frames ahead (a frame for the picture, one for the game to read the press).
pub const LEAD_FRAMES: f64 = 2.0;
/// Smooth mode: an error of a quarter of the bar, at strength 1, holds for the whole frame.
pub const GAIN: f64 = 4.0;

// ───────────────────────── the step's marker ─────────────────────────

pub fn check_marker(marker: Option<&Json>) -> Res<()> {
    let m = match marker {
        None | Some(Json::Null) => return Err(Fail::user(l::t("Choose the marker's color."))),
        Some(m) => m,
    };
    if let Json::String(text) = m {
        return need_colour_or_name(Some(text.as_str()));
    }
    let (left, right) = match m {
        Json::Object(o) => match (o.get("left"), o.get("right")) {
            (Some(Json::String(left)), Some(Json::String(right))) => (left, right),
            _ => return Err(Fail::user(l::t("Choose the marker's color."))),
        },
        _ => return Err(Fail::user(l::t("Choose the marker's color."))),
    };
    need_colour_or_name(Some(left.as_str()))?;
    need_colour_or_name(Some(right.as_str()))
}

fn need_colour_or_name(text: Option<&str>) -> Res<()> {
    let bad = match text {
        Some(t) if t.starts_with('$') => !macro_values::NAME.is_match(&t[1..]),
        _ => !is_colour(text),
    };
    if bad { Err(Fail::user(l::t("Choose the marker's color."))) } else { Ok(()) }
}

fn is_colour(c: Option<&str>) -> bool {
    // C# counted UTF-16 units: 7 of them, '#' then six hex digits (hex digits are ASCII, so the same here).
    matches!(c, Some(c) if c.chars().count() == 7 && c.starts_with('#') && c[1..].chars().all(|ch| ch.is_ascii_hexdigit()))
}

/// The marker's colours, board values put in.
/// Errs with macro_player::value_missing (MacroValueMissing) when a "$name" isn't a colour.
pub fn marker(marker: Option<&Json>, value: &dyn Fn(&str) -> Option<Json>) -> Res<MacroMarker> {
    let colour = |text: &str| -> Res<String> {
        let Some(name) = text.strip_prefix('$') else {
            return Ok(text.to_string());
        };
        match value(name) {
            Some(Json::String(v)) if is_colour(Some(v.as_str())) => Ok(v),
            _ => Err(value_missing(name)),
        }
    };
    // C#: marker!.Value on a missing marker throws InvalidOperationException.
    let m = marker.ok_or_else(|| Fail::other("Nullable object must have a value."))?;
    match m {
        Json::String(text) => Ok(MacroMarker { colour: Some(colour(text)?), left: None, right: None }),
        _ => {
            // C#: GetProperty("left").GetString()! throws when it isn't there or isn't text.
            let side = |name: &str| -> Res<String> {
                match m.get(name) {
                    Some(Json::String(s)) => colour(s),
                    _ => Err(Fail::other(format!("The marker has no {name} colour."))),
                }
            };
            Ok(MacroMarker { colour: None, left: Some(side("left")?), right: Some(side("right")?) })
        }
    }
}

/// "#rrggbb" as its three channels (Convert.ToInt32(…, 16): an Err for anything that isn't hex).
pub fn rgb(colour: &str) -> Res<(i32, i32, i32)> {
    let part = |from: usize| -> Res<i32> {
        let text = colour.get(from..from + 2).ok_or_else(|| Fail::other("Index and length must refer to a location within the string."))?;
        i32::from_str_radix(text, 16).map_err(|_| Fail::other("Could not find any recognizable digits."))
    };
    Ok((part(1)?, part(3)?, part(5)?))
}

// ───────────────────────── looking ─────────────────────────

/// A picture's pixels as 0xAARRGGBB, row after row.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pixels {
    pub w: i32,
    pub h: i32,
    pub p: Vec<u32>,
}

impl Pixels {
    pub fn of(bitmap: &Bitmap, area: Option<Rectangle>) -> Pixels {
        let whole = Rectangle { x: 0, y: 0, width: bitmap.width, height: bitmap.height };
        let r = intersect(area.unwrap_or(whole), whole);
        if r.width <= 0 || r.height <= 0 {
            return Pixels { w: 0, h: 0, p: Vec::new() };
        }
        let mut p = Vec::with_capacity((r.width * r.height) as usize);
        for y in 0..r.height {
            for x in 0..r.width {
                p.push(bitmap.get(r.x + x, r.y + y));
            }
        }
        Pixels { w: r.width, h: r.height, p }
    }
}

/// System.Drawing's Rectangle.Intersect: the overlap, or an empty rectangle at 0,0 when there is none.
pub(crate) fn intersect(a: Rectangle, b: Rectangle) -> Rectangle {
    let x1 = a.x.max(b.x);
    let x2 = (a.x + a.width).min(b.x + b.width);
    let y1 = a.y.max(b.y);
    let y2 = (a.y + a.height).min(b.y + b.height);
    if x2 >= x1 && y2 >= y1 { Rectangle { x: x1, y: y1, width: x2 - x1, height: y2 - y1 } } else { Rectangle { x: 0, y: 0, width: 0, height: 0 } }
}

fn near(argb: u32, c: (i32, i32, i32), tolerance: i32) -> bool {
    (((argb >> 16) & 0xFF) as i32 - c.0).abs() <= tolerance && (((argb >> 8) & 0xFF) as i32 - c.1).abs() <= tolerance && ((argb & 0xFF) as i32 - c.2).abs() <= tolerance
}

/// The middle (x, in pixels of the area) of the pixels of a colour; None when there are none.
pub fn centre(px: &Pixels, colour: &str, tolerance: i32) -> Res<Option<f64>> {
    let c = rgb(colour)?;
    let (mut sum, mut count) = (0i64, 0i64);
    for y in 0..px.h {
        let row = (y * px.w) as usize;
        for x in 0..px.w {
            if near(px.p[row + x as usize], c, tolerance) {
                sum += x as i64;
                count += 1;
            }
        }
    }
    Ok(if count == 0 { None } else { Some(sum as f64 / count as f64) })
}

/// One frame of a follow step: where the target and the marker (or its box's edges) are.
pub fn look(px: &Pixels, target: &str, marker: &MacroMarker, tolerance: i32) -> Res<FollowSight> {
    let t = centre(px, target, tolerance)?;
    if !marker.r#box() {
        return Ok(FollowSight { target: t, marker: centre(px, marker.colour.as_deref().unwrap_or(""), tolerance)?, left: None, right: None });
    }
    let left = centre(px, marker.left.as_deref().unwrap_or(""), tolerance)?;
    let right = centre(px, marker.right.as_deref().unwrap_or(""), tolerance)?;
    let middle = match (left, right) {
        (Some(l), Some(r)) => Some((l + r) / 2.0),
        _ => None,
    };
    Ok(FollowSight { target: t, marker: middle, left, right })
}

/// On/off: hold while the marker is left of the target, let go once it is right of it; None: on it, keep as is.
/// (C#'s deadZone defaulted to DEAD_ZONE.)
pub fn on_off(marker: f64, target: f64, dead_zone: f64) -> Option<bool> {
    if marker < target - dead_zone {
        Some(true)
    } else if marker > target + dead_zone {
        Some(false)
    } else {
        None
    }
}

/// Smooth mode: a PD controller. Each frame it says how long to hold (0..frame ms): longer the further the marker is
/// behind the target, shorter when it is already closing in fast (damping), none once it is past.
#[derive(Clone, Debug)]
pub struct Pd {
    kp: f64,
    kd: f64,
    width: f64,
    last_error: Option<f64>,
}

impl Pd {
    pub fn new(kp: f64, kd: f64, width: f64) -> Pd {
        Pd { kp, kd, width, last_error: None }
    }

    pub fn width(&self) -> f64 {
        self.width
    }

    /// `target`: where the target is (px). `marker`: where the marker is (px), predicted ahead already when asked to.
    pub fn press_ms(&mut self, target: f64, marker: f64, frame_ms: f64) -> f64 {
        let error = (target - marker) / self.width.max(1.0);
        let change = match self.last_error {
            Some(last) => error - last,
            None => 0.0,
        }; // per frame
        self.last_error = Some(error);
        let u = self.kp * error + self.kd * change;
        (u * GAIN).clamp(0.0, 1.0) * frame_ms
    }

}

/// The marker's speed (px a frame) from its last places, and where it will be after LEAD_FRAMES.
#[derive(Clone, Debug, Default)]
pub struct Predictor {
    last: Option<f64>,
    speed: f64,
}

impl Predictor {
    pub fn new() -> Predictor {
        Predictor::default()
    }

    pub fn ahead(&mut self, marker: f64) -> f64 {
        if let Some(last) = self.last {
            self.speed = 0.5 * self.speed + 0.5 * (marker - last); // smoothed: one frame's jitter isn't a speed
        }
        self.last = Some(marker);
        marker + self.speed * LEAD_FRAMES
    }

    pub fn speed(&self) -> f64 {
        self.speed
    }
}

// ───────────────────────── clickColour ─────────────────────────

/// Patches of a colour in the area at least `min_size` px across, largest first (looked at every other pixel: a button
/// is never one pixel).
pub fn blobs(px: &Pixels, colour: &str, tolerance: i32, min_size: i32) -> Res<Vec<Rectangle>> {
    let c = rgb(colour)?;
    const STEP: i32 = 2;
    let gw = (px.w + STEP - 1) / STEP;
    let gh = (px.h + STEP - 1) / STEP;
    let cells = (gw.max(0) * gh.max(0)) as usize;
    let mut on = vec![false; cells];
    for gy in 0..gh {
        for gx in 0..gw {
            let at = ((px.h - 1).min(gy * STEP) * px.w + (px.w - 1).min(gx * STEP)) as usize;
            on[(gy * gw + gx) as usize] = near(px.p[at], c, tolerance);
        }
    }
    let mut seen = vec![false; on.len()];
    let mut found: Vec<(Rectangle, i32)> = Vec::new();
    let mut stack: Vec<i32> = Vec::new();
    for i in 0..on.len() {
        if !on[i] || seen[i] {
            continue;
        }
        let (mut min_x, mut min_y, mut max_x, mut max_y, mut count) = (i32::MAX, i32::MAX, -1, -1, 0);
        stack.push(i as i32);
        seen[i] = true;
        while let Some(j) = stack.pop() {
            let (x, y) = (j % gw, j / gw);
            count += 1;
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
            let mut visit = |nx: i32, ny: i32| {
                if nx < 0 || ny < 0 || nx >= gw || ny >= gh {
                    return;
                }
                let k = (ny * gw + nx) as usize;
                if on[k] && !seen[k] {
                    seen[k] = true;
                    stack.push(k as i32);
                }
            };
            visit(x - 1, y);
            visit(x + 1, y);
            visit(x, y - 1);
            visit(x, y + 1);
        }
        let b = Rectangle { x: min_x * STEP, y: min_y * STEP, width: (max_x - min_x + 1) * STEP, height: (max_y - min_y + 1) * STEP };
        if b.width.max(b.height) >= min_size {
            found.push((b, count));
        }
    }
    // OrderByDescending is stable: equal counts keep the order found.
    found.sort_by(|a, b| b.1.cmp(&a.1));
    Ok(found.into_iter().map(|f| f.0).collect())
}
