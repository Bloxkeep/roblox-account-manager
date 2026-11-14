// Another fishing macro's settings read into Bloxkeep's "Cast and reel" (macro_templates.rs): the file such a tool
// keeps next to its exe, picked in Automation › Import. Two are known, both Python apps people pass around for
// Roblox fishing games, read from what their own code writes (nothing of theirs is run):
//
//   IRUS Glorp      GlorpSettings.json  {area_coords:{x,y,width,height}, cast_hold_time, cast_wait_time, kp, kd,
//                                         pd_clamp, action_threshold, detection_timeout, recast_delay, tolerance, …}
//                                        Its colours are in its code: the green zone R155–180 G190–225 B115–150.
//   Idiotproof      Config.txt (JSON)   {areas:{fish:{x,y,width,height}, shake:{…}}, cast:{method, …},
//                                         fish:{delay_after_end, rod_type, kp, kd, rods:{name:{target_colors:[[b,g,r]],
//                                         bar_colors:[[b,g,r],…]}}, target_tolerance, bar_tolerance}, …}
//
// What carries over: the bar's area (from the screen's pixels to parts of it), the colours, how long to hold to cast,
// how long to wait for the bar, the break before the next cast. Their steering numbers (kp, kd) are on another scale
// than Bloxkeep's and stay at Bloxkeep's; their own extras (a perfect cast, the shake, zooming in) aren't steps here:
// each is said, so the person knows.
use serde_json::Value;

use crate::core::l;
use crate::core::macro_model::Macro;
use crate::core::macro_templates::{self, CastParams};

/// The macro, and the sentences on what didn't carry over. None: not a file of either tool.
pub fn from_settings(json: &Value, screen: (f64, f64)) -> Option<(Macro, Vec<String>)> {
    let (p, said, tool) = if json.get("area_coords").is_some() && json.get("cast_hold_time").is_some() {
        glorp(json, screen)
    } else if json.pointer("/areas/fish").is_some() && json.get("fish").is_some_and(Value::is_object) {
        idiotproof(json, screen)
    } else {
        return None;
    };
    let mut m = macro_templates::cast_with(&p).ok()?.r#macro;
    m.name = format!("{} ({tool})", l::t("Cast and reel"));
    Some((m, said))
}

/// What Glorp starts with before it has saved anything: its box is set for a 2560×1440 screen and scaled to the
/// screen it runs on (get_scaled_default_area), the same parts of any screen.
pub fn glorp_defaults() -> Value {
    serde_json::json!({
        "area_coords": {"x": 765, "y": 1138, "width": 1030, "height": 118, "of": [2560, 1440]},
        "cast_hold_time": 0.5, "cast_wait_time": 2.0, "detection_timeout": 5.0, "recast_delay": 0.5
    })
}

fn num(v: &Value, path: &str) -> Option<f64> {
    let v = v.pointer(path)?;
    v.as_f64().or_else(|| v.as_str().and_then(|t| t.trim().parse().ok()))
}

/// A box in the screen's pixels as parts of it.
fn area(b: &Value, screen: (f64, f64)) -> Option<(f64, f64, f64, f64)> {
    let (sw, sh) = match (num(b, "/of/0"), num(b, "/of/1")) {
        (Some(w), Some(h)) => (w, h),
        _ => screen,
    };
    let (x, y, w, h) = (num(b, "/x")?, num(b, "/y")?, num(b, "/width")?, num(b, "/height")?);
    (w > 0. && h > 0. && sw > 0. && sh > 0.).then(|| (x / sw, y / sh, w / sw, h / sh))
}

fn hex((r, g, b): (u8, u8, u8)) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// [b, g, r] as their code keeps colours (OpenCV's order).
fn bgr(v: &Value) -> Option<(u8, u8, u8)> {
    let a = v.as_array()?;
    let at = |i: usize| a.get(i).and_then(Value::as_f64).map(|x| x.clamp(0., 255.) as u8);
    Some((at(2)?, at(1)?, at(0)?))
}

fn ms(secs: f64) -> i64 {
    (secs * 1000.).round() as i64
}

fn glorp(j: &Value, screen: (f64, f64)) -> (CastParams, Vec<String>, &'static str) {
    let mut p = CastParams::default();
    let mut said = Vec::new();
    match j.get("area_coords").and_then(|b| area(b, screen)) {
        Some(a) => p.area = a,
        None => said.push(l::t("The bar's area wasn't in the file: move the box onto the bar.")),
    }
    if let Some(h) = num(j, "/cast_hold_time") {
        p.hold_ms = ms(h);
    }
    // It waits after casting, then looks for the green zone for at most detection_timeout.
    p.bar_wait_ms = ms(num(j, "/cast_wait_time").unwrap_or(2.) + num(j, "/detection_timeout").unwrap_or(5.)).max(1000);
    if let Some(r) = num(j, "/recast_delay") {
        p.rest_ms = ms(r);
    }
    if let Some(t) = num(j, "/tolerance") {
        p.tolerance = (t.round() as i64).max(8);
    }
    // Glorp's zone colour is fixed in its code (the middle of the range it looks for); the marker it follows is the
    // bar's edges, which Bloxkeep finds as the marker colour: white, as most of these games draw it.
    p.target = hex((168, 208, 132));
    said.push(l::t("Its steering (kp, kd) is on another scale and wasn't copied: change Strength on the panel if the marker overshoots."));
    (p, said, "IRUS Glorp")
}

fn idiotproof(j: &Value, screen: (f64, f64)) -> (CastParams, Vec<String>, &'static str) {
    let mut p = CastParams::default();
    let mut said = Vec::new();
    match j.pointer("/areas/fish").and_then(|b| area(b, screen)) {
        Some(a) => p.area = a,
        None => said.push(l::t("The bar's area wasn't in the file: move the box onto the bar.")),
    }
    let rod = j.pointer("/fish/rod_type").and_then(Value::as_str).unwrap_or("Default");
    let rods = j.pointer("/fish/rods");
    let colours = rods.and_then(|r| r.get(rod)).or_else(|| rods.and_then(|r| r.get("Default")));
    if let Some(t) = colours.and_then(|c| c.pointer("/target_colors/0")).and_then(bgr) {
        p.target = hex(t);
    }
    if let Some(b) = colours.and_then(|c| c.pointer("/bar_colors/0")).and_then(bgr) {
        p.marker = hex(b);
    }
    if let Some(t) = num(j, "/fish/target_tolerance").filter(|t| *t > 0.) {
        p.tolerance = (t.round() as i64).max(8);
    }
    if let Some(d) = num(j, "/fish/delay_after_end") {
        p.rest_ms = ms(d);
    }
    if j.pointer("/cast/method").and_then(Value::as_str) == Some("Perfect") {
        said.push(l::t("Its perfect cast (letting go when the power bar is full) isn't a step here: the cast holds for the time on the panel."));
    }
    if j.pointer("/shake/method").and_then(Value::as_str).is_some_and(|m| m != "Disabled") {
        said.push(l::t("Its shake step isn't copied: add a Click a color step for the shake button if your game has one."));
    }
    said.push(l::t("Its steering (kp, kd) is on another scale and wasn't copied: change Strength on the panel if the marker overshoots."));
    (p, said, "Idiotproof")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn opt<'a>(m: &'a Macro, name: &str) -> &'a Value {
        m.options.iter().find(|o| o.name == name).and_then(|o| o.value.as_ref()).unwrap()
    }

    #[test]
    fn glorp_settings() {
        let j = json!({"start_stop_key":"f3","area_coords":{"x":765,"y":1138,"width":1030,"height":118},"kp":15.0,"kd":30.0,
            "pd_clamp":30.0,"action_threshold":2.0,"cast_hold_time":0.5,"cast_wait_time":2.0,"detection_timeout":5.0,"recast_delay":0.5,"tolerance":20});
        let (m, said) = from_settings(&j, (2560., 1440.)).unwrap();
        assert!(m.name.contains("Glorp"));
        assert_eq!(opt(&m, "o1"), &json!(500));
        assert_eq!(opt(&m, "o5"), &json!(7000));
        let r = &m.areas[0].rect;
        assert!((r.x - 765. / 2560.).abs() < 1e-3 && (r.h - 118. / 1440.).abs() < 1e-3);
        assert!(!said.is_empty());
        let mut m = m;
        assert!(crate::core::macro_model::macro_rules::check(&mut m, &std::collections::HashSet::new(), &[]).is_ok());
    }

    #[test]
    fn idiotproof_config() {
        let j = json!({"areas":{"shake":{"x":530,"y":235,"width":1500,"height":665},"fish":{"x":765,"y":1217,"width":1032,"height":38}},
            "cast":{"method":"Perfect"},"shake":{"method":"Pixel"},
            "fish":{"method":"Color","delay_after_end":1.0,"rod_type":"Default","kp":0.5,"kd":0.3,
                    "rods":{"Default":{"target_colors":[[91,75,67]],"bar_colors":[[255,255,255],[241,241,241]]}},"target_tolerance":0,"bar_tolerance":5}});
        let (m, said) = from_settings(&j, (2560., 1440.)).unwrap();
        assert!(m.name.contains("Idiotproof"));
        assert_eq!(opt(&m, "o2"), &json!("#434b5b"));
        assert_eq!(opt(&m, "o3"), &json!("#ffffff"));
        assert_eq!(said.len(), 3);
        let mut m = m;
        assert!(crate::core::macro_model::macro_rules::check(&mut m, &std::collections::HashSet::new(), &[]).is_ok());
    }

    #[test]
    fn glorp_never_saved() {
        let (m, _) = from_settings(&glorp_defaults(), (1920., 1080.)).unwrap();
        let r = &m.areas[0].rect;
        assert!((r.x - 765. / 2560.).abs() < 1e-3 && (r.y - 1138. / 1440.).abs() < 1e-3);
    }

    #[test]
    fn something_else() {
        assert!(from_settings(&json!({"name":"x","steps":[]}), (1920., 1080.)).is_none());
    }
}
