// MacroWatch.cs: "What it sees".
//! "What it sees": one picture of the game looked at the way the macro's steps look at it, without playing anything.
//! Boxes where its areas are and where it found its colours, pictures and words, and the numbers it decides by
//! (how far the marker is from the target, how well a picture matched). The main tool for tuning colours.
use serde::Serialize;

use crate::core::base::{Json, Res};
use crate::core::win_kit::{Picture as Bitmap, Rect as Rectangle};
use crate::core::l;
use crate::core::macro_follow::{self, MacroMarker, Pixels};
use crate::core::macro_model::{Macro, MacroRect, MacroStep, macro_rules};
use crate::core::macro_ocr;
use crate::core::macro_player::{IMacroDesk, is_value_missing, round_to};
use crate::core::macro_values;
use crate::core::macro_vision::{self, Gray};

const MAX_BOXES: usize = 32;
const MAX_NUMBERS: usize = 12;
const MAX_BLOBS: usize = 5;

/// A box in fractions of the picture (0..1). Kind: area, target, marker, colour, image, text.
/// (Sent to the page in its camelCase shape.)
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Box {
    pub kind: String,
    pub label: String,
    pub colour: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Number {
    pub label: String,
    pub value: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Seen {
    pub boxes: Vec<Box>,
    pub numbers: Vec<Number>,
}

/// A box of picture pixels (x, y, w, h) as fractions of a `pw` × `ph` picture.
fn fraction(kind: &str, label: &str, colour: &str, r: (f64, f64, f64, f64), pw: i32, ph: i32) -> Box {
    let (w, h) = (pw as f64, ph as f64);
    Box {
        kind: kind.into(),
        label: label.into(),
        colour: colour.into(),
        x: round_to(r.0 / w, 4),
        y: round_to(r.1 / h, 4),
        w: round_to(r.2 / w, 4),
        h: round_to(r.3 / h, 4),
    }
}

fn rect(r: Rectangle) -> (f64, f64, f64, f64) {
    (r.x as f64, r.y as f64, r.width as f64, r.height as f64)
}

/// The macro's looking steps on `picture`; steps whose board values are missing are left out.
pub fn look(picture: &Bitmap, m: &Macro, value: &dyn Fn(&str) -> Option<Json>) -> Res<Seen> {
    let mut seen = Seen::default();
    let (w, h) = (picture.width, picture.height);
    if w < 2 || h < 2 {
        return Ok(seen);
    }
    let scale = match &m.recorded_size {
        Some(size) if size.w > 0 => w as f64 / size.w as f64,
        _ => 1.0,
    };
    let mut gray: Option<Gray> = None;
    let mut n = 0;
    for raw in &m.steps {
        n += 1;
        if seen.boxes.len() >= MAX_BOXES {
            break;
        }
        let s: MacroStep = match macro_values::resolve_step(raw, &mut |name: &str| value(name)) {
            Ok(s) => s.into_owned(),
            Err(_missing) => continue,
        };
        let tag = l::f("Step {0}", &[&n]);
        match s.kind.as_str() {
            "follow" if s.region.is_some() && s.target.is_some() => {
                let target = s.target.as_deref().unwrap_or("");
                let area = macro_vision::area(s.region.as_ref(), w, h);
                seen.boxes.push(fraction("area", &tag, "#a3a3a3", rect(area), w, h));
                let marker: MacroMarker = match macro_follow::marker(s.marker.as_ref(), value) {
                    Ok(m) => m,
                    Err(f) if is_value_missing(&f) => continue,
                    Err(f) => return Err(f),
                };
                let px = Pixels::of(picture, Some(area));
                let sight = macro_follow::look(&px, target, &marker, s.tolerance.unwrap_or(24))?;
                let mut line = |kind: &str, label: &str, colour: &str, x: Option<f64>| {
                    if let Some(at) = x {
                        // C# drew it as a RectangleF of floats.
                        let left = ((area.x as f64 + at - 1.0) as f32) as f64;
                        seen.boxes.push(fraction(kind, label, colour, (left, area.y as f64, 3.0, area.height as f64), w, h));
                    }
                };
                line("target", &l::t("Target"), "#22c55e", sight.target);
                if marker.r#box() {
                    line("marker", &l::t("Left edge"), "#f59e0b", sight.left);
                    line("marker", &l::t("Right edge"), "#f59e0b", sight.right);
                }
                line("marker", &l::t("Marker"), "#3b82f6", sight.marker);
                match (sight.target, sight.marker) {
                    (Some(t), Some(mk)) => seen.numbers.push(Number { label: format!("{tag}: {}", l::t("distance (px)")), value: round_to(t - mk, 1) }),
                    _ => seen.numbers.push(Number {
                        label: format!("{tag}: {}", if sight.target.is_none() { l::t("target not found") } else { l::t("marker not found") }),
                        value: 0.0,
                    }),
                }
            }
            "clickColour" if s.region.is_some() && s.colour.is_some() => {
                let colour = s.colour.as_deref().unwrap_or("");
                let area = macro_vision::area(s.region.as_ref(), w, h);
                seen.boxes.push(fraction("area", &tag, "#a3a3a3", rect(area), w, h));
                let blobs = macro_follow::blobs(&Pixels::of(picture, Some(area)), colour, s.tolerance.unwrap_or(24), s.min_size.unwrap_or(4))?;
                for b in blobs.iter().take(MAX_BLOBS) {
                    seen.boxes.push(fraction("colour", &l::t("Found"), colour, ((area.x + b.x) as f64, (area.y + b.y) as f64, b.width as f64, b.height as f64), w, h));
                }
                seen.numbers.push(Number { label: format!("{tag}: {}", l::t("patches found")), value: blobs.len() as f64 });
            }
            "waitColour" if s.colour.is_some() => {
                let colour = s.colour.as_deref().unwrap_or("");
                if let Some(r) = s.region.as_ref() {
                    let area = macro_vision::area(Some(r), w, h);
                    seen.boxes.push(fraction("area", &tag, "#a3a3a3", rect(area), w, h));
                    let blobs = macro_follow::blobs(&Pixels::of(picture, Some(area)), colour, s.tolerance.unwrap_or(24), 2)?;
                    for b in blobs.iter().take(MAX_BLOBS) {
                        seen.boxes.push(fraction("colour", &l::t("Found"), colour, ((area.x + b.x) as f64, (area.y + b.y) as f64, b.width as f64, b.height as f64), w, h));
                    }
                    seen.numbers.push(Number { label: format!("{tag}: {}", l::t("patches found")), value: blobs.len() as f64 });
                } else if let (Some(x), Some(y)) = (s.x, s.y) {
                    point(&mut seen, &tag, picture, x, y, colour, s.tolerance.unwrap_or(24));
                }
            }
            "waitImage" | "clickImage" if s.image.is_some() => {
                let g = gray.get_or_insert_with(|| macro_vision::to_gray(picture, None));
                image(&mut seen, &tag, g, s.image.as_deref().unwrap_or(""), s.region.as_ref(), scale, w, h);
            }
            "if" | "loopUntil" if s.test.is_some() => {
                let Some(test) = s.test.as_ref() else { continue };
                match test.kind.as_str() {
                    "image" if test.image.is_some() => {
                        let g = gray.get_or_insert_with(|| macro_vision::to_gray(picture, None));
                        image(&mut seen, &tag, g, test.image.as_deref().unwrap_or(""), test.region.as_ref(), scale, w, h);
                    }
                    "colour" if test.x.is_some() && test.y.is_some() && test.colour.is_some() => {
                        point(&mut seen, &tag, picture, test.x.unwrap_or(0.0), test.y.unwrap_or(0.0), test.colour.as_deref().unwrap_or(""), test.tolerance.unwrap_or(24));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    seen.numbers.truncate(MAX_NUMBERS);
    Ok(seen)
}

fn point(seen: &mut Seen, tag: &str, picture: &Bitmap, x: f64, y: f64, colour: &str, tolerance: i32) {
    let (w, h) = (picture.width as f64, picture.height as f64);
    let yes = macro_vision::colour_at(picture, x, y, colour, tolerance);
    seen.boxes.push(Box {
        kind: "colour".into(),
        label: if yes { l::t("Color matches") } else { l::t("Color differs") },
        colour: if yes { "#22c55e" } else { "#ef4444" }.into(),
        x: round_to(x - 4.0 / w, 4),
        y: round_to(y - 4.0 / h, 4),
        w: round_to(8.0 / w, 4),
        h: round_to(8.0 / h, 4),
    });
    seen.numbers.push(Number { label: format!("{tag}: {}", l::t("color matches")), value: if yes { 1.0 } else { 0.0 } });
}

#[allow(clippy::too_many_arguments)]
fn image(seen: &mut Seen, tag: &str, gray: &Gray, image: &str, region: Option<&MacroRect>, scale: f64, w: i32, h: i32) {
    let area = macro_vision::area(region, gray.w, gray.h);
    if region.is_some() {
        seen.boxes.push(fraction("area", tag, "#a3a3a3", rect(area), w, h));
    }
    let Some(template) = macro_vision::template(image, scale) else { return };
    let Some((score, found)) = macro_vision::find(gray, &template, area) else { return };
    seen.boxes.push(fraction(
        "image",
        &l::f("Match {0}%", &[&((score * 100.0).round_ties_even() as i32)]),
        if score >= 0.9 { "#22c55e" } else { "#f59e0b" },
        rect(found),
        w,
        h,
    ));
    seen.numbers.push(Number { label: format!("{tag}: {}", l::t("picture match")), value: round_to(score, 3) });
}

/// A line under the picture when there is nothing to see: no step looks at the game, or a value is missing.
pub fn note(m: &Macro, seen: &Seen) -> Option<String> {
    if !seen.boxes.is_empty() {
        return None;
    }
    let looks = m.steps.iter().any(|s| {
        matches!(s.kind.as_str(), "follow" | "clickColour" | "waitColour" | "waitImage" | "clickImage" | "waitText" | "read")
            || matches!(s.kind.as_str(), "if" | "loopUntil") && s.test.as_ref().is_some_and(|t| matches!(t.kind.as_str(), "image" | "colour" | "text"))
    });
    Some(if looks { l::t("Nothing found: check the colors, areas and settings.") } else { l::t("No step looks at the game: add a color, picture or text step.") })
}

/// The words the macro waits for or checks, with their areas.
pub fn text_steps(m: &Macro) -> Vec<(String, Option<MacroRect>)> {
    let mut found = Vec::new();
    for s in macro_rules::all_steps(m) {
        if s.kind == "waitText" && s.text.as_deref().is_some_and(|t| !t.is_empty()) {
            found.push((s.text.clone().unwrap_or_default(), s.region.clone()));
        } else if matches!(s.kind.as_str(), "if" | "loopUntil")
            && let Some(t) = s.test.as_ref()
            && t.kind == "text"
            && t.text.as_deref().is_some_and(|x| !x.is_empty())
        {
            found.push((t.text.clone().unwrap_or_default(), t.region.clone()));
        }
    }
    found
}

/// Boxes around the words the macro looks for, where they are read (text reading takes a while: called at most once a second).
/// Err when the reading failed (the C#'s exception out of ReadAsync).
pub fn read_text(picture: &Bitmap, m: &Macro, desk: &dyn IMacroDesk) -> Res<Vec<Box>> {
    let mut boxes = Vec::new();
    let (w, h) = (picture.width, picture.height);
    for (text, region) in text_steps(m).into_iter().take(4) {
        let area = macro_vision::area(region.as_ref(), w, h);
        let Some(read) = desk.read(picture, area)? else { break };
        let mut at = None;
        for lines in &read {
            at = macro_ocr::locate(lines, &text);
            if at.is_some() {
                break;
            }
        }
        match at {
            // RectangleF.IsEmpty: no width or no height.
            Some(b) if !(b.width <= 0.0 || b.height <= 0.0) => {
                boxes.push(fraction("text", &text, "#a855f7", (b.x as f64, b.y as f64, b.width as f64, b.height as f64), w, h));
            }
            _ if region.is_some() => {
                let label = if at.is_none() { l::f("\"{0}\" not read", &[&text]) } else { text.clone() };
                boxes.push(fraction("area", &label, if at.is_none() { "#ef4444" } else { "#a855f7" }, rect(area), w, h));
            }
            _ => {}
        }
    }
    Ok(boxes)
}

/// A copy at most `max_w` wide (the page and the small window need no more).
/// PORT: GDI+'s HighQualityBilinear is done here as an average of the source pixels each new one covers (it only ever
/// shrinks); the colours can differ by a step or two from the C# picture.
pub fn scaled(picture: &Bitmap, max_w: i32) -> Bitmap {
    let (sw, sh) = (picture.width, picture.height);
    let scale = (max_w as f64 / sw.max(1) as f64).min(1.0);
    let w = ((sw as f64 * scale) as i32).max(1);
    let h = ((sh as f64 * scale) as i32).max(1);
    let mut px = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        let y0 = (y as f64 * sh as f64 / h as f64) as i32;
        let y1 = (((y + 1) as f64 * sh as f64 / h as f64).ceil() as i32).clamp(y0 + 1, sh.max(y0 + 1));
        for x in 0..w {
            let x0 = (x as f64 * sw as f64 / w as f64) as i32;
            let x1 = (((x + 1) as f64 * sw as f64 / w as f64).ceil() as i32).clamp(x0 + 1, sw.max(x0 + 1));
            let (mut a, mut r, mut g, mut b, mut n) = (0u64, 0u64, 0u64, 0u64, 0u64);
            for yy in y0..y1.min(sh) {
                for xx in x0..x1.min(sw) {
                    let p = picture.get(xx, yy);
                    a += ((p >> 24) & 0xFF) as u64;
                    r += ((p >> 16) & 0xFF) as u64;
                    g += ((p >> 8) & 0xFF) as u64;
                    b += (p & 0xFF) as u64;
                    n += 1;
                }
            }
            let n = n.max(1);
            px.push((((a / n) as u32) << 24) | (((r / n) as u32) << 16) | (((g / n) as u32) << 8) | (b / n) as u32);
        }
    }
    Bitmap { width: w, height: h, pixels: px }
}
