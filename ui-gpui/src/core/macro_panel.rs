// MacroPanel.cs.
// Macros (Beta): what the player may change on a macro's panel (panel_window.rs) and the boxes it looks in on the game.
// The maker ticks "Player can change" on a step's value: the value becomes an option ("$o1" in the step), shown on the
// panel; an area is a named box ("@a1" in the step), moved and sized on the game itself (overlay_window.rs). Kept in the
// macro; the page reads and writes the same shape (ui/src/macros.ts).
use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize};

use crate::core::base::{Extra, Fail, Guid, Json, Res};
use crate::core::l;
use crate::core::macro_model::net::{self, double_json, eq_ci, len16, null_default, num_opt, parse_float};
use crate::core::macro_model::{Macro, MacroRect, macro_keys, macro_rules};
use crate::core::macro_values;

/// A value the player may change on the macro's panel.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct MacroOption {
    /// Made by the page ("o1", "o2"…), never shown; steps name it as "$o1".
    #[serde(deserialize_with = "null_default")]
    pub name: String,
    #[serde(deserialize_with = "null_default")]
    pub label: String,
    /// "number", "colour", "check" (1/0), "key", "choice", "point" ({x, y}, parts of the window; a step's x
    /// and y both name it), "list" (the player ticks some of `choices`; a step reading text takes any of them)
    /// or "heading" (a title over the settings below it, no value).
    #[serde(deserialize_with = "null_default")]
    pub kind: String,
    /// The panel's tab it shows on (one of `MacroPanel::tabs`); null: the first.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "num_opt")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "num_opt")]
    pub max: Option<f64>,
    /// A number's unit: "ms", "s" (kept in ms like the step's field, shown in seconds), "px", "%" or "".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "texts_de")]
    pub choices: Option<Vec<String>>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for MacroOption {
    fn default() -> MacroOption {
        MacroOption {
            name: String::new(),
            label: String::new(),
            kind: "number".into(),
            tab: None,
            value: None,
            default: None,
            min: None,
            max: None,
            unit: None,
            choices: None,
            extra: Extra::new(),
        }
    }
}

/// A named box on the game, as parts (0..1) of the window's picture area.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct MacroArea {
    /// Made by the page ("a1", "a2"…); steps name it as "@a1".
    #[serde(deserialize_with = "null_default")]
    pub name: String,
    #[serde(deserialize_with = "null_default")]
    pub label: String,
    /// null is taken as "" (CheckArea gives it the area colour, as C# did with null).
    #[serde(deserialize_with = "null_default")]
    pub colour: String,
    /// null is taken as an empty box, which CheckArea refuses as C# refused null.
    #[serde(deserialize_with = "null_default")]
    pub rect: MacroRect,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for MacroArea {
    fn default() -> MacroArea {
        MacroArea {
            name: String::new(),
            label: String::new(),
            colour: "#22c55e".into(),
            rect: MacroRect { x: 0.25, y: 0.25, w: 0.5, h: 0.5 },
            extra: Extra::new(),
        }
    }
}

/// The macro's panel: shown by itself while an account plays `place_id`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct MacroPanel {
    pub auto_show: bool,
    pub place_id: Option<i64>,
    /// Its tabs' names, in order; fewer than two: no tabs. (A null name is kept as "", which the checks refuse.)
    #[serde(deserialize_with = "tabs_de")]
    pub tabs: Vec<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// A list of texts whose items may be null (C#'s `List<string>`): each null is "" (`c ?? ""`).
fn texts_de<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<String>>, D::Error> {
    let list: Option<Vec<Option<String>>> = Option::deserialize(d)?;
    Ok(list.map(|l| l.into_iter().map(Option::unwrap_or_default).collect()))
}

fn tabs_de<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    Ok(texts_de(d)?.unwrap_or_default())
}

/// Math.Round(d, digits) (MidpointRounding.ToEven).
fn round_even(d: f64, digits: i32) -> f64 {
    if !d.is_finite() {
        return d;
    }
    let p = 10f64.powi(digits);
    let r = (d * p).round_ties_even() / p;
    if r.is_finite() { r } else { d }
}

/// Math.Clamp, without its throw for a minimum above the maximum (the callers never pass one).
fn clamp_f(v: f64, lo: f64, hi: f64) -> f64 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

fn clamp_i(v: i32, lo: i32, hi: i32) -> i32 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

/// A macro's options and areas: their checks (each refusal a sentence for the person), their values as the
/// panel shows and takes them, and when a panel shows by itself.
pub mod macro_options {
    use super::*;

    pub const MAX_OPTIONS: i32 = 30;
    pub const MAX_AREAS: i32 = 8;
    pub const MAX_LABEL: i32 = 30;
    pub const MAX_CHOICES: i32 = 60;
    pub const MAX_TABS: i32 = 6;
    pub const MAX_TAB: i32 = 20;
    const KINDS: [&str; 8] = ["number", "colour", "check", "key", "choice", "point", "list", "heading"];
    const UNITS: [&str; 5] = ["ms", "s", "px", "%", ""];
    /// "^#[0-9a-fA-F]{6}$" (.NET's `$` also matches before a final "\n").
    static COLOUR_TEXT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#[0-9a-fA-F]{6}\n?\z").unwrap());
    pub const AREA_COLOUR: &str = "#22c55e";

    fn user(text: String) -> Fail {
        Fail::user(text)
    }

    /// Checks the macro's options, areas and panel, and fills in defaults; a value that no longer fits goes back
    /// to its default.
    pub fn check(m: &mut Macro) -> Res<()> {
        if m.options.len() > MAX_OPTIONS as usize {
            return Err(user(l::f("A macro can have up to {0} settings for the player.", &[&MAX_OPTIONS])));
        }
        let mut names: HashSet<String> = HashSet::new();
        for o in m.options.iter_mut() {
            check_option(o)?;
            if !names.insert(o.name.clone()) {
                return Err(user(l::t("Two settings for the player have the same name.")));
            }
        }
        if m.areas.len() > MAX_AREAS as usize {
            return Err(user(l::f("A macro can have up to {0} areas.", &[&MAX_AREAS])));
        }
        names.clear();
        for a in m.areas.iter_mut() {
            check_area(a)?;
            if !names.insert(a.name.clone()) {
                return Err(user(l::t("Two areas have the same name.")));
            }
        }
        if m.panel.place_id.is_some_and(|p| p <= 0) {
            m.panel.place_id = None;
        }
        m.panel.tabs = m.panel.tabs.iter().map(|t| t.trim().to_string()).collect();
        if m.panel.tabs.len() > MAX_TABS as usize {
            return Err(user(l::f("A panel can have up to {0} tabs.", &[&MAX_TABS])));
        }
        if m.panel.tabs.iter().any(|t| len16(t) == 0 || len16(t) > MAX_TAB as usize) {
            return Err(user(l::f("A tab's name is 1 to {0} characters.", &[&MAX_TAB])));
        }
        if distinct_ci(&m.panel.tabs) != m.panel.tabs.len() {
            return Err(user(l::t("Two tabs have the same name.")));
        }
        if m.panel.tabs.len() < 2 {
            m.panel.tabs.clear();
        }
        let tabs = m.panel.tabs.clone();
        for o in m.options.iter_mut() {
            let wanted = o.tab.as_deref().map(str::trim);
            o.tab = tabs.iter().find(|t| wanted.is_some_and(|w| eq_ci(t, w))).cloned();
        }
        Ok(())
    }

    /// Distinct(StringComparer.OrdinalIgnoreCase).Count().
    fn distinct_ci(list: &[String]) -> usize {
        let mut seen: Vec<Vec<char>> = Vec::new();
        for t in list {
            let f = net::fold(t);
            if !seen.contains(&f) {
                seen.push(f);
            }
        }
        seen.len()
    }

    fn check_option(o: &mut MacroOption) -> Res<()> {
        if !macro_values::NAME.is_match(&o.name) {
            return Err(user(l::t("A setting for the player has no name.")));
        }
        o.label = o.label.trim().to_string();
        if len16(&o.label) == 0 || len16(&o.label) > MAX_LABEL as usize {
            return Err(user(l::f("A setting's label is 1 to {0} characters.", &[&MAX_LABEL])));
        }
        if !KINDS.contains(&o.kind.as_str()) {
            return Err(user(l::t("Unknown kind of setting.")));
        }
        if o.kind == "number" {
            if o.min.is_some_and(|min| !min.is_finite()) {
                o.min = None;
            }
            if o.max.is_some_and(|max| !max.is_finite()) {
                o.max = None;
            }
            // `o.Min > o.Max` is false when either is null.
            if let (Some(min), Some(max)) = (o.min, o.max)
                && min > max
            {
                (o.min, o.max) = (o.max, o.min);
            }
            if o.unit.is_none() {
                o.unit = Some(String::new());
            }
            if !UNITS.contains(&o.unit.as_deref().unwrap_or("")) {
                o.unit = Some(String::new());
            }
        } else {
            o.min = None;
            o.max = None;
            o.unit = None;
        }
        if o.kind == "choice" || o.kind == "list" {
            let choices: Vec<String> = o.choices.take().unwrap_or_default().iter().map(|c| c.trim().to_string()).collect();
            if choices.is_empty() || choices.len() > MAX_CHOICES as usize {
                return Err(user(l::f("A choice has 1 to {0} options.", &[&MAX_CHOICES])));
            }
            if choices.iter().any(|c| len16(c) == 0 || len16(c) > MAX_LABEL as usize) {
                return Err(user(l::f("An option is 1 to {0} characters.", &[&MAX_LABEL])));
            }
            if distinct_ci(&choices) != choices.len() {
                return Err(user(l::t("Two options are the same.")));
            }
            o.choices = Some(choices);
        } else {
            o.choices = None;
        }
        if o.kind == "heading" {
            o.default = None;
            o.value = None;
            return Ok(());
        }
        o.default = match o.default.clone().and_then(|d| normalize(o, &d)) {
            Some(fits) => Some(fits),
            None => Some(plain(o)),
        };
        o.value = match o.value.clone().and_then(|v| normalize(o, &v)) {
            Some(now) => Some(now),
            None => o.default.clone(),
        };
        Ok(())
    }

    fn check_area(a: &mut MacroArea) -> Res<()> {
        if !macro_values::NAME.is_match(&a.name) {
            return Err(user(l::t("An area has no name.")));
        }
        a.label = a.label.trim().to_string();
        if len16(&a.label) == 0 || len16(&a.label) > MAX_LABEL as usize {
            return Err(user(l::f("An area's name is 1 to {0} characters.", &[&MAX_LABEL])));
        }
        a.colour = if COLOUR_TEXT.is_match(&a.colour) { a.colour.to_lowercase() } else { AREA_COLOUR.to_string() };
        a.rect = match rect(Some(a.rect)) {
            Some(r) => r,
            None => return Err(user(l::f("{0}: the area must be inside the window.", &[&a.label]))),
        };
        Ok(())
    }

    /// A box inside the window (a little room for rounding), rounded to 4 places; null when it isn't one.
    pub fn rect(r: Option<MacroRect>) -> Option<MacroRect> {
        let r = r?;
        if !(r.x >= 0.0 && r.y >= 0.0 && r.w > 0.001 && r.h > 0.001 && r.x + r.w <= 1.0001 && r.y + r.h <= 1.0001) {
            return None;
        }
        Some(MacroRect { x: round(r.x), y: round(r.y), w: round(r.w).min(1.0 - round(r.x)), h: round(r.h).min(1.0 - round(r.y)) })
    }

    fn round(d: f64) -> f64 {
        round_even(d, 4)
    }

    fn num(d: f64) -> Json {
        double_json(d)
    }

    fn str_json(s: &str) -> Json {
        Json::String(s.to_string())
    }

    /// A value as the option takes it (a number kept in its range, a colour in small letters, a check as 1/0, a
    /// known key or choice as it is spelled there), or null.
    pub fn normalize(o: &MacroOption, v: &Json) -> Option<Json> {
        match o.kind.as_str() {
            "number" => {
                let d = match v {
                    Json::Number(n) => n.as_f64(),
                    Json::String(t) => parse_float(t),
                    _ => None,
                };
                let n = d.filter(|n| n.is_finite())?;
                Some(num(clamp_f(n, o.min.unwrap_or(f64::MIN), o.max.unwrap_or(f64::MAX))))
            }
            "colour" => match v {
                Json::String(t) if COLOUR_TEXT.is_match(t) => Some(str_json(&t.to_lowercase())),
                _ => None,
            },
            "check" => match v {
                Json::Bool(true) => Some(num(1.0)),
                Json::Bool(false) => Some(num(0.0)),
                Json::Number(n) if n.as_f64().is_some_and(f64::is_finite) => Some(num(if n.as_f64().unwrap_or(0.0) != 0.0 { 1.0 } else { 0.0 })),
                Json::String(t) if t == "1" || t == "true" => Some(num(1.0)),
                Json::String(t) if t == "0" || t == "false" => Some(num(0.0)),
                _ => None,
            },
            "key" => {
                let Json::String(text) = v else {
                    return None;
                };
                let key = macro_keys::ALL
                    .iter()
                    .map(|k| k.name.as_str())
                    .chain(macro_rules::MOUSE_KEYS.iter().map(|(name, _)| *name))
                    .find(|k| eq_ci(k, text))?;
                Some(str_json(key))
            }
            "choice" => match v {
                Json::String(t) => o.choices.as_ref()?.iter().find(|c| eq_ci(c, t)).map(|c| str_json(c)),
                _ => None,
            },
            "list" => {
                // The ticked ones, spelled and ordered as the list.
                let Json::Array(items) = v else {
                    return None;
                };
                let ticked: Vec<&str> = items.iter().filter_map(|w| w.as_str()).collect();
                let kept: Vec<Json> = o.choices.clone().unwrap_or_default().into_iter().filter(|c| ticked.iter().any(|t| eq_ci(t, c))).map(Json::String).collect();
                Some(Json::Array(kept))
            }
            "point" => {
                let x = v.get("x").filter(|_| v.is_object())?;
                let y = v.get("y")?;
                match (x, y) {
                    (Json::Number(x), Json::Number(y)) => {
                        let (x, y) = (x.as_f64()?, y.as_f64()?);
                        if x.is_finite() && y.is_finite() { Some(point(x, y)) } else { None }
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// An option's value when nothing else is set: its smallest number (or 0), white, off, Space, its first choice.
    pub fn plain(o: &MacroOption) -> Json {
        match o.kind.as_str() {
            "number" => num(clamp_f(0.0, o.min.unwrap_or(f64::MIN), o.max.unwrap_or(f64::MAX))),
            "colour" => str_json("#ffffff"),
            "check" => num(0.0),
            "key" => str_json("Space"),
            "point" => point(0.5, 0.5),
            "list" => Json::Array(Vec::new()),
            // a heading's "": nothing a step can use
            _ => str_json(o.choices.as_ref().and_then(|c| c.first()).map(String::as_str).unwrap_or("")),
        }
    }

    /// A point on the window, kept inside it, rounded to 4 places.
    pub fn point(x: f64, y: f64) -> Json {
        serde_json::json!({ "x": num(round(clamp_f(x, 0.0, 1.0))), "y": num(round(clamp_f(y, 0.0, 1.0))) })
    }

    /// The option's value now (its default, else a plain one, when it has none).
    pub fn value_of(o: &MacroOption) -> Json {
        o.value.clone().or_else(|| o.default.clone()).unwrap_or_else(|| plain(o))
    }

    /// Seconds shown in seconds; a part (0..1) in percent shown as percent.
    fn shown(o: &MacroOption) -> f64 {
        match o.unit.as_deref() {
            Some("s") => 1000.0,
            Some("%") if o.max.is_some_and(|m| m <= 1.0) => 0.01,
            _ => 1.0,
        }
    }

    /// What a kept number is divided by to show it (1000 for seconds kept in ms, 0.01 for a part shown in percent).
    pub fn shown_scale(o: &MacroOption) -> f64 {
        shown(o)
    }

    /// A number's unit as written after it in the app's language (" s" / " giây", "%", " px").
    pub fn suffix(o: &MacroOption) -> String {
        match o.unit.as_deref() {
            Some("s") => format!(" {}", l::t("s")),
            Some("%") => "%".into(),
            Some(u) if !u.is_empty() => format!(" {u}"),
            _ => String::new(),
        }
    }

    /// `d.ToString("0.##", InvariantCulture)`: up to two decimals, rounded half away from zero, no trailing zeros.
    fn two_places(d: f64) -> String {
        if !d.is_finite() {
            return net::double_text(d);
        }
        let r = (d * 100.0).round() / 100.0;
        let mut text = format!("{:.2}", r.abs());
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
        // .NET writes "-0" for a negative number that rounds to zero.
        if d < 0.0 { format!("-{text}") } else { text }
    }

    /// A number in the app language's way (Vietnamese: a decimal comma). By hand: the app runs with invariant
    /// globalization, where asking for vi-VN throws.
    fn number(d: f64) -> String {
        let text = two_places(d);
        crate::lang::by_code(&l::language()).map_or(text.clone(), |l| l.decimal(&text))
    }

    /// A number as the panel shows it, with its unit ("0.5 s", "120 px"; "0,5 giây" in Vietnamese).
    pub fn show(o: &MacroOption, v: &Json) -> String {
        match o.kind.as_str() {
            "number" => {
                let d = match v {
                    Json::Number(n) => n.as_f64().unwrap_or(0.0) / shown(o),
                    _ => 0.0,
                };
                number(d) + &suffix(o)
            }
            "check" => {
                if v.as_f64().is_some_and(|d| d != 0.0) {
                    l::t("On")
                } else {
                    l::t("Off")
                }
            }
            "list" => match v {
                Json::Array(items) => l::f("{0} of {1}", &[&items.len(), &o.choices.as_ref().map_or(0, Vec::len)]),
                _ => String::new(),
            },
            "point" => {
                // As parts of the window: the same place whatever its size.
                match (v.is_object(), v.get("x"), v.get("y")) {
                    (true, Some(x), Some(y)) => format!(
                        "{}%, {}%",
                        net::double_text((x.as_f64().unwrap_or(0.0) * 100.0).round_ties_even()),
                        net::double_text((y.as_f64().unwrap_or(0.0) * 100.0).round_ties_even())
                    ),
                    _ => String::new(),
                }
            }
            _ => v.as_str().map(str::to_string).unwrap_or_default(),
        }
    }

    /// `\s*(ms|px|s|giây|%)$`, ignoring case.
    static UNIT_AFTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\s*(ms|px|s|giây|%)\n?\z").unwrap());

    /// The number typed (in seconds for seconds, a decimal comma or point, its unit or none) as the option keeps
    /// it, before it is kept in its range; null when it isn't one.
    pub fn typed_number(o: &MacroOption, text: &str) -> Option<f64> {
        let t = UNIT_AFTER.replace(text.trim(), "").replace(',', ".");
        let d = parse_float(&t).filter(|d| d.is_finite())?;
        Some(round_even(d * shown(o), 6))
    }

    /// What − and + add on the panel: a tenth of a second, 10 ms, 1 % or a small step for a small range.
    pub fn step(o: &MacroOption) -> f64 {
        match o.unit.as_deref() {
            Some("s") => return 100.0,
            Some("ms") => return 10.0,
            Some("%") if o.max.is_some_and(|m| m <= 1.0) => return 0.01,
            _ => {}
        }
        let span = match (o.min, o.max) {
            (Some(min), Some(max)) => max - min,
            _ => f64::MAX,
        };
        if span <= 1.0 {
            0.01
        } else if span <= 20.0 {
            0.1
        } else {
            1.0
        }
    }

    /// A number moved by − or + (`by` steps), kept in its range.
    pub fn nudge(o: &MacroOption, by: i32) -> Json {
        let now = match value_of(o) {
            Json::Number(n) => n.as_f64().unwrap_or(0.0),
            _ => 0.0,
        };
        let step = step(o);
        let next = ((now + by as f64 * step) / step).round_ties_even() * step;
        normalize(o, &num(round_even(next, 6))).unwrap_or(Json::Null)
    }

    /// The variables the macro counts (setVar, addVar), shown on its panel as counters.
    pub fn counters(m: &Macro) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for s in macro_rules::all_steps(m) {
            let counted = (matches!(s.kind.as_str(), "setVar" | "addVar") && s.name.is_some() && s.text.is_none())
                || (s.kind == "read" && s.name.is_some() && s.mode.as_deref() != Some("text"));
            if counted && let Some(name) = &s.name
                && !out.contains(name)
            {
                out.push(name.clone());
            }
        }
        out
    }

    /// Panels shown by themselves while an account plays their game: those to show now (their game is played, not shown,
    /// not closed by the person meanwhile) and those to hide (shown by themselves, their game no longer played). A panel
    /// the person closed stays closed until nobody plays its game. `auto` and `closed` are kept up to date.
    ///
    /// A panel with no game set shows on any game, but only while one of the accounts it runs on plays one
    /// (`any_game`), and at most `max` panels are shown by themselves at once: macros saved long ago with autoShow on
    /// and no game would otherwise all pop up at the first start.
    #[allow(clippy::too_many_arguments)]
    pub fn auto_show(
        macros: &[Macro],
        played: &HashSet<i64>,
        auto: &mut HashSet<Guid>,
        closed: &mut HashSet<Guid>,
        shown: &dyn Fn(Guid) -> bool,
        any_game: Option<&dyn Fn(&Macro) -> bool>,
        max: i32,
    ) -> (Vec<Guid>, Vec<Guid>) {
        let mut show = Vec::new();
        let mut hide = Vec::new();
        for m in macros {
            if !m.panel.auto_show {
                continue;
            }
            let in_game = match m.panel.place_id {
                Some(place) => played.contains(&place),
                None => !played.is_empty() && any_game.is_some_and(|f| f(m)),
            };
            if !in_game {
                closed.remove(&m.id);
                if auto.remove(&m.id) {
                    hide.push(m.id);
                }
            } else if !closed.contains(&m.id) && !shown(m.id) && (auto.len() as i64) < max as i64 {
                auto.insert(m.id);
                show.push(m.id);
            }
        }
        (show, hide)
    }
}

/// The maths of drawing on a game: areas and points kept as parts (0..1) of the window, shown in screen pixels;
/// areas moved by their middle and sized by their corners, never outside the window nor smaller than a few pixels; points
/// as numbered dots (the step's number).
pub mod area_geometry {
    use super::*;
    use crate::core::win_kit::{Point, Rect};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Grip {
        None,
        Move,
        TopLeft,
        TopRight,
        BottomLeft,
        BottomRight,
    }

    /// A step's point on the game: its index (0-based) and where it is.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Dot {
        pub step: i32,
        pub x: f64,
        pub y: f64,
    }

    /// The points of the macro's click, hold, move and colour-at-a-place steps (those with a number, not a setting).
    pub fn dots(m: &Macro) -> Vec<Dot> {
        let mut dots = Vec::new();
        for (i, s) in m.steps.iter().enumerate() {
            let point = !s.is_null
                && (matches!(s.kind.as_str(), "click" | "move") || s.kind == "waitColour" && s.region.is_none() && macro_values::ref_of(&s.extra, "region").is_none());
            if point && let (Some(x), Some(y)) = (s.x, s.y)
                && x.is_finite()
                && y.is_finite()
            {
                dots.push(Dot { step: i as i32, x: clamp_f(x, 0.0, 1.0), y: clamp_f(y, 0.0, 1.0) });
            }
            // A point the player sets: where it is now.
            else if point
                && let Some(name) = macro_values::ref_of(&s.extra, "x")
                && let Some(option) = m.options.iter().find(|o| o.name == name && o.kind == "point")
                && let at @ Json::Object(_) = macro_options::value_of(option)
            {
                let x = at.get("x").and_then(Json::as_f64).unwrap_or(0.0);
                let y = at.get("y").and_then(Json::as_f64).unwrap_or(0.0);
                dots.push(Dot { step: i as i32, x, y });
            }
        }
        dots
    }

    /// Math.Round(d) as an int (ToEven).
    fn ri(d: f64) -> i32 {
        d.round_ties_even() as i32
    }

    pub fn to_screen_point(x: f64, y: f64, client: Rect) -> Point {
        Point::new(client.x + ri(x * client.width as f64), client.y + ri(y * client.height as f64))
    }

    /// A place on the window as parts of it (kept inside), rounded to 4 places.
    pub fn to_fraction_point(p: Point, client: Rect) -> (f64, f64) {
        if client.width <= 0 || client.height <= 0 {
            return (0.5, 0.5);
        }
        (
            round_even(clamp_f((p.x - client.x) as f64 / client.width as f64, 0.0, 1.0), 4),
            round_even(clamp_f((p.y - client.y) as f64 / client.height as f64, 0.0, 1.0), 4),
        )
    }

    /// The dot a press at `p` takes (the last drawn is on top, within `radius` px); -1 for none.
    pub fn hit_dot(dots: &[Dot], p: Point, client: Rect, radius: i32) -> i32 {
        for i in (0..dots.len()).rev() {
            let at = to_screen_point(dots[i].x, dots[i].y, client);
            let (dx, dy) = (at.x - p.x, at.y - p.y);
            if dx * dx + dy * dy <= radius * radius {
                return i as i32;
            }
        }
        -1
    }

    pub fn to_screen(r: &MacroRect, client: Rect) -> Rect {
        Rect::new(
            client.x + ri(r.x * client.width as f64),
            client.y + ri(r.y * client.height as f64),
            1.max(ri(r.w * client.width as f64)),
            1.max(ri(r.h * client.height as f64)),
        )
    }

    pub fn to_fraction(r: Rect, client: Rect) -> MacroRect {
        let inside = Rect::intersect(r, client);
        if inside.width <= 0 || inside.height <= 0 || client.width <= 0 || client.height <= 0 {
            return MacroRect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 };
        }
        MacroRect {
            x: round_even((inside.x - client.x) as f64 / client.width as f64, 4),
            y: round_even((inside.y - client.y) as f64 / client.height as f64, 4),
            w: round_even(inside.width as f64 / client.width as f64, 4),
            h: round_even(inside.height as f64 / client.height as f64, 4),
        }
    }

    /// What a press at `p` takes hold of: a corner (within `grip` px), the inside, or nothing.
    pub fn hit_test(b: Rect, p: Point, grip: i32) -> Grip {
        let near = |a: i32, c: i32| (a - c).abs() <= grip;
        if near(p.x, b.left()) && near(p.y, b.top()) {
            return Grip::TopLeft;
        }
        if near(p.x, b.right()) && near(p.y, b.top()) {
            return Grip::TopRight;
        }
        if near(p.x, b.left()) && near(p.y, b.bottom()) {
            return Grip::BottomLeft;
        }
        if near(p.x, b.right()) && near(p.y, b.bottom()) {
            return Grip::BottomRight;
        }
        if b.contains_point(p) { Grip::Move } else { Grip::None }
    }

    /// The box after dragging `grip` by `dx`, `dy`: inside `bounds`, at least `min` px each way.
    pub fn drag(start: Rect, grip: Grip, dx: i32, dy: i32, bounds: Rect, min: i32) -> Rect {
        let (mut left, mut top, mut right, mut bottom) = (start.left(), start.top(), start.right(), start.bottom());
        match grip {
            Grip::Move => {
                let dx = clamp_i(dx, bounds.left() - left, bounds.right() - right);
                let dy = clamp_i(dy, bounds.top() - top, bounds.bottom() - bottom);
                return Rect::new(left + dx, top + dy, start.width, start.height);
            }
            Grip::TopLeft => {
                left = clamp_i(left + dx, bounds.left(), right - min);
                top = clamp_i(top + dy, bounds.top(), bottom - min);
            }
            Grip::TopRight => {
                right = clamp_i(right + dx, left + min, bounds.right());
                top = clamp_i(top + dy, bounds.top(), bottom - min);
            }
            Grip::BottomLeft => {
                left = clamp_i(left + dx, bounds.left(), right - min);
                bottom = clamp_i(bottom + dy, top + min, bounds.bottom());
            }
            Grip::BottomRight => {
                right = clamp_i(right + dx, left + min, bounds.right());
                bottom = clamp_i(bottom + dy, top + min, bounds.bottom());
            }
            Grip::None => return start,
        }
        Rect::from_ltrb(left, top, right, bottom)
    }
}
