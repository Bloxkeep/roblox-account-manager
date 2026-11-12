// Settings and areas in macros: a step's or check's number, colour or key may be "$o1", one of the macro's options
// (macro_panel.rs), and its area "@a1", one of the macro's areas; read when the step plays. Such a field is kept as text
// in the step's unknown fields (`MacroStep::extra`, under the field's own name), so the page and the file show it where
// the number would be, and the typed field stays empty.
use std::borrow::Cow;
use std::sync::LazyLock;

use regex::Regex;

use crate::core::base::{Extra, Fail, Json, Res};
use crate::core::l;
use crate::core::macro_model::macro_rules::page_json;
use crate::core::macro_model::macro_rules::{self, MAX_WAIT_MS};
use crate::core::macro_model::net::{self, raw_text};
use crate::core::macro_model::{Macro, MacroRect, MacroStep, MacroTest, macro_keys};

/// A run needs a setting or area that isn't there (or isn't a number, colour, key or area as the step needs).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MacroValueMissing {
    pub name: String,
}

impl MacroValueMissing {
    pub fn new(name: impl Into<String>) -> MacroValueMissing {
        MacroValueMissing { name: name.into() }
    }
}

impl std::fmt::Display for MacroValueMissing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "No value {}.", self.name)
    }
}

impl std::error::Error for MacroValueMissing {}

impl From<MacroValueMissing> for Fail {
    fn from(e: MacroValueMissing) -> Fail {
        Fail::Other(e.to_string())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Shape {
    Int,
    Real,
    Colour,
    Region,
    Key,
    Words,
}

/// A step's fields that may name a value, in C#'s order.
const STEP_FIELDS: [(&str, Shape); 25] = [
    ("ms", Shape::Int),
    ("x", Shape::Real),
    ("y", Shape::Real),
    ("dx", Shape::Int),
    ("dy", Shape::Int),
    ("notches", Shape::Int),
    ("timeoutMs", Shape::Int),
    ("threshold", Shape::Real),
    ("tolerance", Shape::Int),
    ("times", Shape::Int),
    ("max", Shape::Int),
    ("value", Shape::Real),
    ("fps", Shape::Int),
    ("untilGoneMs", Shape::Int),
    ("holdMs", Shape::Int),
    ("kp", Shape::Real),
    ("kd", Shape::Real),
    ("minSize", Shape::Int),
    ("repeatUntilGoneMs", Shape::Int),
    ("colour", Shape::Colour),
    ("target", Shape::Colour),
    ("region", Shape::Region),
    ("key", Shape::Key),
    ("eachAfterMs", Shape::Int),
    ("words", Shape::Words),
];

/// A check's fields that may name a value, in C#'s order.
const TEST_FIELDS: [(&str, Shape); 8] = [
    ("x", Shape::Real),
    ("y", Shape::Real),
    ("threshold", Shape::Real),
    ("tolerance", Shape::Int),
    ("value", Shape::Real),
    ("colour", Shape::Colour),
    ("region", Shape::Region),
    ("words", Shape::Words),
];

/// An option's or area's name ("o1", "a1"). (.NET's `$` also matches before a final "\n": `\n?\z` keeps that.)
pub static NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z_][A-Za-z0-9_]{0,23}\n?\z").unwrap());
static COLOUR_TEXT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#[0-9a-fA-F]{6}\n?\z").unwrap());

/// The key of `o` that `name` names, case aside (JsonNode with PropertyNameCaseInsensitive).
fn key_ci(o: &serde_json::Map<String, Json>, name: &str) -> Option<String> {
    o.keys().find(|k| net::eq_ci(k, name)).cloned()
}

/// A macro the page or a file sent: "$o1" and "@a1" fields are set aside before the typed reading, then kept as text.
pub fn read(json: &Json) -> Res<Macro> {
    let Json::Object(root) = json else {
        return Err(Fail::user(l::t("That isn't a macro.")));
    };
    let mut root = root.clone();
    drop_empty(&mut root, "id"); // a new macro comes with an empty id
    let main = match key_ci(&root, "steps") {
        Some(k) => take_steps(root.get_mut(&k)),
        None => Vec::new(),
    };
    // Each feature's steps the same way, by the feature's place in the list.
    let features: Vec<Taken> = match key_ci(&root, "features").and_then(|k| root.get_mut(&k)) {
        Some(Json::Array(list)) => list
            .iter_mut()
            .map(|f| match f {
                Json::Object(o) => match key_ci(o, "steps") {
                    Some(k) => take_steps(o.get_mut(&k)),
                    None => Vec::new(),
                },
                _ => Vec::new(),
            })
            .collect(),
        _ => Vec::new(),
    };
    let mut m: Macro = page_json::read(&Json::Object(root), page_json::Shape::Macro)?;
    put_back(&mut m.steps, main);
    let count = features.len().min(m.features.len());
    for (f, set) in features.into_iter().enumerate().take(count) {
        put_back(&mut m.features[f].steps, set);
    }
    Ok(m)
}

/// A list of steps' "$o1" and "@a1" fields, taken out of the JSON: (step, test or not, field, value).
type Taken = Vec<(usize, bool, String, Json)>;

/// A list of steps' "$o1" and "@a1" fields, taken out of the JSON (by step, test or not).
fn take_steps(node: Option<&mut Json>) -> Taken {
    let mut set = Taken::new();
    let Some(Json::Array(steps)) = node else {
        return set;
    };
    for (i, step) in steps.iter_mut().enumerate() {
        let Json::Object(step) = step else {
            continue;
        };
        for (field, value) in take(step, &STEP_FIELDS, false) {
            set.push((i, false, field, value));
        }
        if let Some(k) = key_ci(step, "test") {
            if let Some(Json::Object(test)) = step.get_mut(&k) {
                for (field, value) in take(test, &TEST_FIELDS, true) {
                    set.push((i, true, field, value));
                }
            }
        }
    }
    set
}

/// The fields `take_steps` took, kept as text in the typed steps.
fn put_back(steps: &mut [MacroStep], set: Taken) {
    for (i, test, field, value) in set {
        let Some(step) = steps.get_mut(i).filter(|s| !s.is_null) else {
            continue;
        };
        if test {
            if let Some(t) = step.test.as_mut() {
                t.extra.insert(field, value);
            }
        } else {
            step.extra.insert(field, value);
        }
    }
}

/// Text where a number, colour, key or area goes: "$o1" anywhere, "@a1" for an area, and a var check's word
/// to compare a choice with.
fn take(o: &mut serde_json::Map<String, Json>, fields: &[(&str, Shape)], test: bool) -> Vec<(String, Json)> {
    let mut taken = Vec::new();
    let keys: Vec<String> = o.keys().cloned().collect();
    for key in keys {
        let Some(&(field, shape)) = fields.iter().find(|(f, _)| net::eq_ci(f, &key)) else {
            continue;
        };
        let Some(Json::String(text)) = o.get(&key) else {
            continue;
        };
        let text = text.clone();
        let word = test && field == "value" && net::parse_float(&text).is_none();
        if !text.starts_with('$') && !(text.starts_with('@') && shape == Shape::Region) && !word {
            continue;
        }
        o.remove(&key);
        taken.push((field.to_string(), Json::String(text)));
    }
    taken
}

/// A field sent empty ("") where a Guid goes: left out (a new macro's id).
pub fn drop_empty(o: &mut serde_json::Map<String, Json>, field: &str) {
    if let Some(k) = key_ci(o, field) {
        if matches!(o.get(&k), Some(Json::String(text)) if text.is_empty()) {
            o.remove(&k);
        }
    }
}

/// What a field names: an option ("o1" for "$o1"), an area as itself ("@a1"), or null.
pub fn ref_of(extra: &Extra, field: &str) -> Option<String> {
    match extra.get(field) {
        Some(Json::String(v)) => {
            if let Some(name) = v.strip_prefix('$') {
                Some(name.to_string())
            } else if v.starts_with('@') && field == "region" {
                Some(v.clone())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// The fields of a step (or check) that name a setting or an area.
fn refs(extra: &Extra, fields: &[(&'static str, Shape)]) -> Vec<(&'static str, String, Shape)> {
    fields.iter().filter_map(|&(field, shape)| ref_of(extra, field).map(|name| (field, name, shape))).collect()
}

/// Every option ("o1") and area ("@a1") the macro names, for the checks.
pub fn names(m: &Macro) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for s in macro_rules::all_steps(m) {
        let mut all: Vec<String> = refs(&s.extra, &STEP_FIELDS).into_iter().map(|r| r.1).collect();
        if let Some(t) = &s.test {
            all.extend(refs(&t.extra, &TEST_FIELDS).into_iter().map(|r| r.1));
        }
        for name in all {
            if !found.contains(&name) {
                found.push(name);
            }
        }
    }
    found
}

// ───────────────────────── checking ─────────────────────────

/// Before a step is checked: each field naming a setting or area gets a stand-in that passes (the value itself is checked
/// when it plays). Returns the fields to empty again after the check (`unfill_step`).
pub fn fill_step(s: &mut MacroStep) -> Res<Vec<&'static str>> {
    let mut filled = Vec::new();
    for &(field, shape) in STEP_FIELDS.iter() {
        let Some(Json::String(text)) = s.extra.get(field) else {
            continue;
        };
        let text = text.clone();
        if text.starts_with('$') || (text.starts_with('@') && shape == Shape::Region) {
            if !NAME.is_match(&text[1..]) {
                return Err(Fail::user(l::f("{0} isn't a setting of the macro.", &[&text])));
            }
            set_step(s, field, &stand_in(field, shape), false);
            filled.push(field);
        } else if field != "value" {
            s.extra.remove(field); // text where only a number goes, not a name: dropped
        }
    }
    Ok(filled)
}

pub fn fill_test(t: &mut MacroTest) -> Res<Vec<&'static str>> {
    let mut filled = Vec::new();
    for &(field, shape) in TEST_FIELDS.iter() {
        let Some(Json::String(text)) = t.extra.get(field) else {
            continue;
        };
        let text = text.clone();
        if text.starts_with('$') || (text.starts_with('@') && shape == Shape::Region) {
            if !NAME.is_match(&text[1..]) {
                return Err(Fail::user(l::f("{0} isn't a setting of the macro.", &[&text])));
            }
            set_test(t, field, &stand_in(field, shape), false);
            filled.push(field);
        } else if field != "value" {
            t.extra.remove(field); // text where only a number goes, not a name: dropped
        }
    }
    Ok(filled)
}

fn stand_in(field: &str, shape: Shape) -> Json {
    match shape {
        Shape::Colour => Json::String("#000000".into()),
        Shape::Region => serde_json::json!({ "x": 0, "y": 0, "w": 1, "h": 1 }),
        Shape::Key => Json::String("Space".into()),
        Shape::Words => serde_json::json!(["x"]),
        // A double in C# (the switch's common type): written as a number either way.
        _ => net::double_json(match field {
            "x" | "y" => 0.5,
            "threshold" => 0.9,
            "ms" => 100.0,
            "timeoutMs" => 1000.0,
            "tolerance" => 24.0,
            "fps" => 60.0,
            "untilGoneMs" => 500.0,
            "kp" => 1.0,
            "kd" => 0.4,
            "minSize" => 4.0,
            "notches" | "times" | "max" => 1.0,
            _ => 0.0,
        }),
    }
}

pub fn unfill_step(s: &mut MacroStep, fields: &[&str]) {
    for field in fields {
        clear_step(s, field);
    }
}

pub fn unfill_test(t: &mut MacroTest, fields: &[&str]) {
    for field in fields {
        clear_test(t, field);
    }
}

// ───────────────────────── playing ─────────────────────────

/// The step with the settings and areas it names put in (the step itself when it names none).
/// Err(MacroValueMissing): a value it needs isn't there, or isn't what the field takes.
pub fn resolve_step<'a>(s: &'a MacroStep, value: &mut dyn FnMut(&str) -> Option<Json>) -> Result<Cow<'a, MacroStep>, MacroValueMissing> {
    let refs = refs(&s.extra, &STEP_FIELDS);
    let test = match &s.test {
        Some(t) if !refs_of_test(t).is_empty() => resolve_test(t, value)?,
        Some(t) => Cow::Borrowed(t),
        None => return resolve_fields(s, None, refs, value),
    };
    resolve_fields(s, Some(test), refs, value)
}

fn refs_of_test(t: &MacroTest) -> Vec<(&'static str, String, Shape)> {
    refs(&t.extra, &TEST_FIELDS)
}

fn resolve_fields<'a>(
    s: &'a MacroStep,
    test: Option<Cow<'_, MacroTest>>,
    refs: Vec<(&'static str, String, Shape)>,
    value: &mut dyn FnMut(&str) -> Option<Json>,
) -> Result<Cow<'a, MacroStep>, MacroValueMissing> {
    let test_changed = matches!(test, Some(Cow::Owned(_)));
    if refs.is_empty() && !test_changed {
        return Ok(Cow::Borrowed(s));
    }
    let mut copy = s.clone();
    copy.test = test.map(Cow::into_owned);
    for (field, name, _) in refs {
        match value(&name) {
            Some(v) if set_step(&mut copy, field, &part(&v, field), true) => {}
            _ => return Err(MacroValueMissing::new(name)),
        }
    }
    Ok(Cow::Owned(copy))
}

/// A point setting ({x, y}) gives x and y each their own part.
fn part(v: &Json, field: &str) -> Json {
    if matches!(field, "x" | "y") {
        if let Some(p) = v.as_object().and_then(|o| o.get(field)) {
            return p.clone();
        }
    }
    v.clone()
}

pub fn resolve_test<'a>(t: &'a MacroTest, value: &mut dyn FnMut(&str) -> Option<Json>) -> Result<Cow<'a, MacroTest>, MacroValueMissing> {
    let refs: Vec<_> = refs(&t.extra, &TEST_FIELDS).into_iter().filter(|r| !(t.kind == "var" && r.0 == "value")).collect();
    if refs.is_empty() {
        return Ok(Cow::Borrowed(t));
    }
    let mut copy = t.clone();
    for (field, name, _) in refs {
        match value(&name) {
            Some(v) if set_test(&mut copy, field, &part(&v, field), true) => {}
            _ => return Err(MacroValueMissing::new(name)),
        }
    }
    Ok(Cow::Owned(copy))
}

/// A var check: the account's variable or a setting, against a number, a word (a choice's option) or another
/// value. A variable the macro sets itself counts as 0 before it is first set.
pub fn var_test(t: &MacroTest, value: &mut dyn FnMut(&str) -> Option<Json>, set_by_macro: bool) -> Result<bool, MacroValueMissing> {
    let name = t.name.clone().unwrap_or_default();
    let left = match value(&name) {
        Some(v) => v,
        None if set_by_macro => Json::from(0),
        None => return Err(MacroValueMissing::new(name)),
    };
    let right = if let Some(other) = ref_of(&t.extra, "value") {
        match value(&other) {
            Some(v) => v,
            None => return Err(MacroValueMissing::new(other)),
        }
    } else if let Some(word @ Json::String(_)) = t.extra.get("value") {
        word.clone()
    } else {
        net::double_json(t.value.unwrap_or(0.0))
    };
    let (l, r) = (text(&left), text(&right));
    let op = t.op.as_deref().unwrap_or_default();
    // Has: the words read (or a setting's) hold the text, whatever the case ("Level 12 reached" has "reached").
    if op == "has" || op == "!has" {
        return Ok(net::contains_ci(&l, &r) == (op == "has"));
    }
    if let (Some(a), Some(b)) = (number(&left), number(&right)) {
        return Ok(crate::core::macro_player::compare(a, op, b));
    }
    let same = net::eq_ci(l.trim(), r.trim());
    Ok(match op {
        "=" => same,
        "!=" => !same,
        _ => false,
    })
}

fn number(v: &Json) -> Option<f64> {
    match v {
        Json::Number(n) => n.as_f64(),
        Json::Bool(true) => Some(1.0),
        Json::Bool(false) => Some(0.0),
        Json::String(s) => net::parse_float(s),
        _ => None,
    }
}

fn text(v: &Json) -> String {
    match v {
        Json::String(s) => s.clone(),
        other => raw_text(other),
    }
}

fn region(v: &Json, clamp: bool) -> Option<MacroRect> {
    if !v.is_object() {
        return None;
    }
    let mut r: MacroRect = page_json::read(v, page_json::Shape::Rect).ok()?;
    if !(r.x + r.y + r.w + r.h).is_finite() {
        return None;
    }
    if clamp {
        r.x = r.x.clamp(0.0, 0.99);
        r.y = r.y.clamp(0.0, 0.99);
        r.w = r.w.clamp(0.01, 1.0 - r.x);
        r.h = r.h.clamp(0.01, 1.0 - r.y);
    }
    Some(r)
}

fn int(v: &Json, field: &str, clamp: bool) -> Option<i32> {
    let d = number(v).filter(|d| d.is_finite())?;
    // Math.Round: halves to even.
    let n = d.clamp(i32::MIN as f64, i32::MAX as f64).round_ties_even() as i64;
    if !clamp {
        return Some(n as i32);
    }
    let (lo, hi): (i64, i64) = match field {
        "dx" | "dy" => (-100_000, 100_000),
        "notches" => (-50, 50),
        "tolerance" => (0, 255),
        "times" | "max" => (0, 100_000),
        "fps" => (15, 240),
        "holdMs" | "eachAfterMs" => (0, 10_000),
        "minSize" => (1, 500),
        _ => (0, MAX_WAIT_MS as i64),
    };
    Some(n.clamp(lo, hi) as i32)
}

fn real(v: &Json, field: &str, clamp: bool) -> Option<f64> {
    let d = number(v).filter(|d| d.is_finite())?;
    if !clamp {
        return Some(d);
    }
    Some(match field {
        "x" | "y" => d.clamp(0.0, 1.0),
        "threshold" => d.clamp(0.5, 1.0),
        "kp" | "kd" => d.clamp(0.0, 10.0),
        _ => d,
    })
}

fn colour(v: &Json) -> Option<String> {
    match v {
        Json::String(c) if COLOUR_TEXT.is_match(c) => Some(c.clone()),
        _ => None,
    }
}

/// A key a step presses (a mouse button too, for Hold and Release only), as the keys list spells it.
fn key(v: &Json, mouse: bool) -> Option<String> {
    let Json::String(k) = v else {
        return None;
    };
    let mouse_names = macro_rules::MOUSE_KEYS.iter().map(|(name, _)| *name).filter(|_| mouse);
    macro_keys::ALL.iter().map(|key| key.name.as_str()).chain(mouse_names).find(|name| net::eq_ci(name, k)).map(str::to_string)
}

/// A list's words (a list setting's ticked ones; none ticked: an empty list, so nothing is ever read).
fn words(v: &Json) -> Option<Vec<String>> {
    match v {
        Json::Array(items) => Some(items.iter().filter_map(|w| w.as_str().map(str::to_string)).collect()),
        _ => None,
    }
}

/// false when the value isn't what the field takes.
fn set_step(s: &mut MacroStep, field: &str, v: &Json, clamp: bool) -> bool {
    match field {
        "x" => (s.x = real(v, field, clamp), s.x.is_some()).1,
        "y" => (s.y = real(v, field, clamp), s.y.is_some()).1,
        "threshold" => (s.threshold = real(v, field, clamp), s.threshold.is_some()).1,
        "value" => (s.value = real(v, field, clamp), s.value.is_some()).1,
        "ms" => (s.ms = int(v, field, clamp), s.ms.is_some()).1,
        "dx" => (s.dx = int(v, field, clamp), s.dx.is_some()).1,
        "dy" => (s.dy = int(v, field, clamp), s.dy.is_some()).1,
        "notches" => (s.notches = int(v, field, clamp), s.notches.is_some()).1,
        "timeoutMs" => (s.timeout_ms = int(v, field, clamp), s.timeout_ms.is_some()).1,
        "tolerance" => (s.tolerance = int(v, field, clamp), s.tolerance.is_some()).1,
        "times" => (s.times = int(v, field, clamp), s.times.is_some()).1,
        "max" => (s.max = int(v, field, clamp), s.max.is_some()).1,
        "fps" => (s.fps = int(v, field, clamp), s.fps.is_some()).1,
        "holdMs" => (s.hold_ms = int(v, field, clamp), s.hold_ms.is_some()).1,
        "minSize" => (s.min_size = int(v, field, clamp), s.min_size.is_some()).1,
        "repeatUntilGoneMs" => (s.repeat_until_gone_ms = int(v, field, clamp), s.repeat_until_gone_ms.is_some()).1,
        "eachAfterMs" => (s.each_after_ms = int(v, field, clamp), s.each_after_ms.is_some()).1,
        "words" => (s.words = words(v), s.words.is_some()).1,
        "kp" => (s.kp = real(v, field, clamp), s.kp.is_some()).1,
        "kd" => (s.kd = real(v, field, clamp), s.kd.is_some()).1,
        "untilGoneMs" => (s.until_gone_ms = int(v, field, clamp), s.until_gone_ms.is_some()).1,
        "colour" => (s.colour = colour(v), s.colour.is_some()).1,
        "target" => (s.target = colour(v), s.target.is_some()).1,
        "region" => (s.region = region(v, clamp), s.region.is_some()).1,
        // A Press step or follow's key can't be a mouse button (a setting of the key kind may hold one): missingValue.
        "key" => {
            s.key = key(v, matches!(s.kind.as_str(), "down" | "up"));
            s.key.is_some()
        }
        _ => false,
    }
}

fn set_test(t: &mut MacroTest, field: &str, v: &Json, clamp: bool) -> bool {
    match field {
        "x" => (t.x = real(v, field, clamp), t.x.is_some()).1,
        "y" => (t.y = real(v, field, clamp), t.y.is_some()).1,
        "threshold" => (t.threshold = real(v, field, clamp), t.threshold.is_some()).1,
        "value" => (t.value = real(v, field, clamp), t.value.is_some()).1,
        "tolerance" => (t.tolerance = int(v, field, clamp), t.tolerance.is_some()).1,
        "colour" => (t.colour = colour(v), t.colour.is_some()).1,
        "region" => (t.region = region(v, clamp), t.region.is_some()).1,
        "words" => (t.words = words(v), t.words.is_some()).1,
        _ => false,
    }
}

fn clear_step(s: &mut MacroStep, field: &str) {
    match field {
        "x" => s.x = None,
        "y" => s.y = None,
        "threshold" => s.threshold = None,
        "value" => s.value = None,
        "ms" => s.ms = None,
        "dx" => s.dx = None,
        "dy" => s.dy = None,
        "notches" => s.notches = None,
        "timeoutMs" => s.timeout_ms = None,
        "tolerance" => s.tolerance = None,
        "times" => s.times = None,
        "max" => s.max = None,
        "fps" => s.fps = None,
        "holdMs" => s.hold_ms = None,
        "minSize" => s.min_size = None,
        "repeatUntilGoneMs" => s.repeat_until_gone_ms = None,
        "eachAfterMs" => s.each_after_ms = None,
        "words" => s.words = None,
        "kp" => s.kp = None,
        "kd" => s.kd = None,
        "untilGoneMs" => s.until_gone_ms = None,
        "colour" => s.colour = None,
        "target" => s.target = None,
        "region" => s.region = None,
        "key" => s.key = None,
        _ => {}
    }
}

fn clear_test(t: &mut MacroTest, field: &str) {
    match field {
        "x" => t.x = None,
        "y" => t.y = None,
        "threshold" => t.threshold = None,
        "value" => t.value = None,
        "tolerance" => t.tolerance = None,
        "colour" => t.colour = None,
        "region" => t.region = None,
        "words" => t.words = None,
        _ => {}
    }
}
