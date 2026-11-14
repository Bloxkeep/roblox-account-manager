// Macros (Beta): what a macro is, as the vault keeps it and the page edits it (the shape is the page's and the core's
// shared contract), the keys it may press, and the checks a macro passes before it is saved. Played by macro_player.rs.
//
// Two JSON shapes, as in the C# app: the vault's (System.Text.Json's defaults: property names as written, "Steps",
// "JitterMs"…), which the serde derives below give, and the page's (camelCase, read case-insensitively, numbers given as
// text taken too: MacroRules.PageJson), which `macro_rules::page_json` makes from and back to the vault's.
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock};

use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::core::base::{Extra, Fail, Guid, Json, Res};
use crate::core::l;
use crate::core::macro_panel::{MacroArea, MacroOption, MacroPanel};

/// A sequence of steps played into the Roblox windows of some accounts, one window at a time.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Macro {
    pub id: Guid,
    #[serde(deserialize_with = "net::null_default")]
    pub name: String,
    #[serde(deserialize_with = "net::steps_de", serialize_with = "net::steps_ser")]
    pub steps: Vec<MacroStep>,
    /// "selected" (the accounts selected when run), "all" (every account with a window) or "accounts" (`account_ids`).
    #[serde(deserialize_with = "net::null_default")]
    pub targets: String,
    #[serde(deserialize_with = "net::null_default")]
    pub account_ids: Vec<Guid>,
    #[serde(deserialize_with = "net::null_default")]
    pub trigger: MacroTrigger,
    /// Triggers fire only while on; running it by hand always works.
    pub enabled: bool,
    /// 1..9: Ctrl+Alt+that digit runs it; null: none. Unique across macros.
    pub hotkey: Option<i32>,
    /// ± this much random time on every wait and hold.
    pub jitter_ms: i32,
    /// ± this many pixels on every click.
    pub jitter_px: i32,
    /// A run on one window stops after this.
    pub max_minutes: i32,
    /// When a wait times out or an image isn't found, or it is stuck: "stop", "skip", "restart" (through its
    /// "recover" steps, if any, then from the top), "rejoin" or "alert". Stuck with "skip": restarted.
    #[serde(deserialize_with = "net::null_default")]
    pub on_fail: String,
    /// Stuck when what its steps look for hasn't been seen this long (then `on_fail`); null: never.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stuck_minutes: Option<i32>,
    /// The window's client size it was recorded or pictured at; images are scaled from it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recorded_size: Option<MacroSize>,
    /// What the player may change on its panel, in the panel's order (a step's field holds "$o1"; macro_panel.rs).
    #[serde(deserialize_with = "net::null_default")]
    pub options: Vec<MacroOption>,
    /// Named boxes on the game; a step's area holds "@a1".
    #[serde(deserialize_with = "net::null_default")]
    pub areas: Vec<MacroArea>,
    #[serde(deserialize_with = "net::null_default")]
    pub panel: MacroPanel,
    /// Switches on the panel with steps of their own, played at the main steps' safe points while on (macro_player.rs).
    #[serde(deserialize_with = "net::features_de")]
    pub features: Vec<MacroFeature>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for Macro {
    fn default() -> Macro {
        Macro {
            id: Guid::nil(),
            name: String::new(),
            steps: Vec::new(),
            targets: "selected".into(),
            account_ids: Vec::new(),
            trigger: MacroTrigger::default(),
            enabled: false,
            hotkey: None,
            jitter_ms: 40,
            jitter_px: 3,
            max_minutes: 10,
            on_fail: "stop".into(),
            stuck_minutes: None,
            recorded_size: None,
            options: Vec::new(),
            areas: Vec::new(),
            panel: MacroPanel::default(),
            features: Vec::new(),
            extra: Extra::new(),
        }
    }
}

/// A feature: a switch on the panel (`option`, a "check" option) and the steps it plays while on. They play
/// whole, one feature after another in the order of the list, where the main steps reach a safe point (their end, or the
/// end of a pass of a repeat that isn't inside another block) with no key held; never in the middle of other steps.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct MacroFeature {
    /// Made by the page ("f1", "f2"…), never shown.
    #[serde(deserialize_with = "net::null_default")]
    pub id: String,
    /// Its switch: the option's name ("o3").
    #[serde(deserialize_with = "net::null_default")]
    pub option: String,
    /// Not again sooner than this after it last played (per account); null: at every safe point.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub every_ms: Option<i32>,
    /// A wait that times out or a picture not found in its steps: "skip" (this time, back to the main steps) or
    /// "macro" (the macro's own `Macro::on_fail`).
    #[serde(deserialize_with = "net::null_default")]
    pub on_fail: String,
    #[serde(deserialize_with = "net::steps_de", serialize_with = "net::steps_ser")]
    pub steps: Vec<MacroStep>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for MacroFeature {
    fn default() -> MacroFeature {
        MacroFeature { id: String::new(), option: String::new(), every_ms: None, on_fail: "skip".into(), steps: Vec::new(), extra: Extra::new() }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct MacroTrigger {
    /// "manual", "interval", "join" (the account got into a game) or "rejoin" (auto-rejoin got it back in).
    #[serde(deserialize_with = "net::null_default")]
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub every_seconds: Option<i32>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for MacroTrigger {
    fn default() -> MacroTrigger {
        MacroTrigger { kind: "manual".into(), every_seconds: None, extra: Extra::new() }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct MacroSize {
    pub w: i32,
    pub h: i32,
}

/// Part of a window's client area, each value 0..1 of its width or height.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct MacroRect {
    #[serde(serialize_with = "net::num_f64")]
    pub x: f64,
    #[serde(serialize_with = "net::num_f64")]
    pub y: f64,
    #[serde(serialize_with = "net::num_f64")]
    pub w: f64,
    #[serde(serialize_with = "net::num_f64")]
    pub h: f64,
}

/// One step; which fields it has depends on `kind`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct MacroStep {
    #[serde(deserialize_with = "net::null_default")]
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ms: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "net::num_opt")]
    pub x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "net::num_opt")]
    pub y: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub button: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub double: Option<bool>,
    /// A PNG as base64. On a recorded click, the place clicked (the page may offer "click by image").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<MacroRect>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "net::num_opt")]
    pub threshold: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dx: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dy: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notches: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gone: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colour: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test: Option<MacroTest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub times: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "net::num_opt")]
    pub value: Option<f64>,
    /// setVar, addVar: a formula worked out when the step plays, instead of `value` (macro_expr.rs).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expr: Option<String>,
    /// call: the other macro whose steps play here, on the same account.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub macro_id: Option<Guid>,
    /// read: where from ("screen", "clipboard", "file", "web"); write: where to ("clipboard", "file", "web").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// read, write: a text file in Documents\Bloxkeep, by its name without ".txt" (macro_outside.rs).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    /// read, write: an https address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// follow: the colour of what to keep up with (a line), and the marker kept on it: one colour, or a box's
    /// edges {left, right} (each a colour or "$name"; see macro_follow.rs).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marker: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fps: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until_gone_ms: Option<i32>,
    /// follow: "onOff" (held while behind, let go past) or "smooth" (short presses sized by distance and speed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "net::num_opt")]
    pub kp: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "net::num_opt")]
    pub kd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub predict: Option<bool>,
    /// click: held this long (a charge, a cast); 0 or none: a plain click.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hold_ms: Option<i32>,
    /// clickColour: patches smaller than this (px across) are specks, not the button.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_size: Option<i32>,
    /// clickColour: clicks again each time it shows up, until it has stayed away this long.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_until_gone_ms: Option<i32>,
    /// clickColour: every patch is clicked, each once, after it has shown this long (rings to hit as they come);
    /// null: the first patch, at once.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub each_after_ms: Option<i32>,
    /// waitText: any of these read counts (in place of `text`); in practice "$o1", a list the
    /// player ticks on the panel.
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "net::words_de")]
    pub words: Option<Vec<String>>,
    /// alert: the window's picture goes with it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picture: Option<bool>,
    /// Unknown fields, and board values named instead of a number, colour or area ("ms": "$wait"; see macro_values.rs).
    #[serde(flatten)]
    pub extra: Extra,
    /// A step a Run a macro step put in place: the macro it is from, whose settings and areas it reads.
    #[serde(skip)]
    pub owner: Option<Arc<Macro>>,
    /// The list held null here (C#'s `List<MacroStep>` may hold null): "Step {0} is empty." when checked.
    #[serde(skip)]
    pub(crate) is_null: bool,
}

/// What an "if" or "loopUntil" asks: "image", "text", "colour" (on the window's picture) or "var".
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct MacroTest {
    #[serde(deserialize_with = "net::null_default")]
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<MacroRect>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "net::num_opt")]
    pub threshold: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub not: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// text: any of these read counts, as a waitText's `MacroStep::words`.
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "net::words_de")]
    pub words: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "net::num_opt")]
    pub x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "net::num_opt")]
    pub y: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colour: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub op: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "net::num_opt")]
    pub value: Option<f64>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// A key a macro may press: its scan code (set 1), whether it is an extended key, and its virtual-key code.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MacroKey {
    pub name: String,
    pub scan: u16,
    pub extended: bool,
    pub vk: u16,
}

/// The keys macros may press, by the names the page uses. Sent as scan codes: games read those (Roblox among them).
pub mod macro_keys {
    use super::*;

    /// Every key, in the order they were added (C#'s `All`, a Dictionary: its enumeration order).
    pub static ALL: LazyLock<Vec<MacroKey>> = LazyLock::new(build);
    static BY_NAME: LazyLock<HashMap<String, usize>> = LazyLock::new(|| ALL.iter().enumerate().map(|(i, k)| (k.name.clone(), i)).collect());

    /// `All[name]` / `All.TryGetValue(name)`: the key by its exact name.
    pub fn get(name: &str) -> Option<&'static MacroKey> {
        BY_NAME.get(name).map(|&i| &ALL[i])
    }

    /// `All.ContainsKey(name)`.
    pub fn contains(name: &str) -> bool {
        BY_NAME.contains_key(name)
    }

    fn build() -> Vec<MacroKey> {
        let mut keys: Vec<MacroKey> = Vec::new();
        let mut add = |name: String, scan: u16, vk: u16, extended: bool| {
            let key = MacroKey { name: name.clone(), scan, extended, vk };
            match keys.iter_mut().find(|k| k.name == name) {
                Some(k) => *k = key,
                None => keys.push(key),
            }
        };
        // Letters, by their place on a US keyboard (scan codes name places, not letters).
        let letters: [u16; 26] =
            [0x1E, 0x30, 0x2E, 0x20, 0x12, 0x21, 0x22, 0x23, 0x17, 0x24, 0x25, 0x26, 0x32, 0x31, 0x18, 0x19, 0x10, 0x13, 0x1F, 0x14, 0x16, 0x2F, 0x11, 0x2D, 0x15, 0x2C];
        for i in 0..26u16 {
            add(((b'A' + i as u8) as char).to_string(), letters[i as usize], b'A' as u16 + i, false);
        }
        for d in 1..=9u16 {
            add(d.to_string(), 0x01 + d, b'0' as u16 + d, false);
        }
        add("0".into(), 0x0B, b'0' as u16, false);
        for f in 1..=10u16 {
            add(format!("F{f}"), 0x3A + f, 0x6F + f, false);
        }
        add("F11".into(), 0x57, 0x7A, false);
        add("F12".into(), 0x58, 0x7B, false);
        add("Space".into(), 0x39, 0x20, false);
        add("Enter".into(), 0x1C, 0x0D, false);
        add("Tab".into(), 0x0F, 0x09, false);
        add("Backspace".into(), 0x0E, 0x08, false);
        add("Escape".into(), 0x01, 0x1B, false);
        add("Shift".into(), 0x2A, 0xA0, false);
        add("Ctrl".into(), 0x1D, 0xA2, false);
        add("Alt".into(), 0x38, 0xA4, false);
        add("Up".into(), 0x48, 0x26, true);
        add("Down".into(), 0x50, 0x28, true);
        add("Left".into(), 0x4B, 0x25, true);
        add("Right".into(), 0x4D, 0x27, true);
        add("Insert".into(), 0x52, 0x2D, true);
        add("Delete".into(), 0x53, 0x2E, true);
        add("Home".into(), 0x47, 0x24, true);
        add("End".into(), 0x4F, 0x23, true);
        add("PageUp".into(), 0x49, 0x21, true);
        add("PageDown".into(), 0x51, 0x22, true);
        add("Minus".into(), 0x0C, 0xBD, false);
        add("Equals".into(), 0x0D, 0xBB, false);
        add("LeftBracket".into(), 0x1A, 0xDB, false);
        add("RightBracket".into(), 0x1B, 0xDD, false);
        add("Semicolon".into(), 0x27, 0xBA, false);
        add("Quote".into(), 0x28, 0xDE, false);
        add("Comma".into(), 0x33, 0xBC, false);
        add("Period".into(), 0x34, 0xBE, false);
        add("Slash".into(), 0x35, 0xBF, false);
        add("Backslash".into(), 0x2B, 0xDC, false);
        add("Backquote".into(), 0x29, 0xC0, false);
        // The number pad without Num Lock's say: its own scan codes, not extended (the arrows' are).
        let pad: [u16; 10] = [0x52, 0x4F, 0x50, 0x51, 0x4B, 0x4C, 0x4D, 0x47, 0x48, 0x49];
        for n in 0..=9u16 {
            add(format!("Numpad{n}"), pad[n as usize], 0x60 + n, false);
        }
        keys
    }

    /// The key a scan code (and extended flag) names, as a keyboard hook or TinyTask reports it; null for others.
    pub fn from_scan(scan: u16, extended: bool, vk: u16) -> Option<&'static MacroKey> {
        // Right Ctrl and right Alt are the extended Ctrl and Alt; right Shift has a scan code of its own.
        if scan == 0x1D {
            return get("Ctrl");
        }
        if scan == 0x38 {
            return get("Alt");
        }
        if scan == 0x36 {
            return get("Shift");
        }
        // The number pad's keys share scan codes with the arrows and Insert…: told apart by the extended flag.
        if let Some(key) = ALL.iter().find(|k| k.scan == scan && k.extended == extended) {
            return Some(key);
        }
        // No scan code (some programs' recordings): by virtual key.
        if scan == 0 && vk != 0 {
            return from_vk(vk);
        }
        None
    }

    pub fn from_vk(vk: u16) -> Option<&'static MacroKey> {
        let vk = match vk {
            0x10 | 0xA1 => 0xA0,
            0x11 | 0xA3 => 0xA2,
            0x12 | 0xA5 => 0xA4,
            _ => vk,
        };
        ALL.iter().find(|k| k.vk == vk)
    }

    /// Keys a Vietnamese Telex input method rewrites (UniKey, EVKey): typed into Roblox they may come out changed.
    pub fn telex_rewrites(key: &str) -> bool {
        matches!(key, "W" | "A" | "O" | "E" | "D" | "S" | "F" | "R" | "X" | "J" | "Z")
    }

    /// The key and Shift for a character typed into chat (US layout); null for one typed as itself (Unicode).
    pub fn for_char(c: char) -> Option<(&'static MacroKey, bool)> {
        if c.is_ascii_lowercase() {
            return get(&c.to_ascii_uppercase().to_string()).map(|k| (k, false));
        }
        if c.is_ascii_uppercase() {
            return get(&c.to_string()).map(|k| (k, true));
        }
        if c.is_ascii_digit() {
            return get(&c.to_string()).map(|k| (k, false));
        }
        const PLAIN: &str = " -=[];',./\\`";
        const SHIFTED: &str = "_+{}:\"<>?|~";
        const DIGITS_SHIFTED: &str = ")!@#$%^&*(";
        const NAMES: [&str; 12] = ["Space", "Minus", "Equals", "LeftBracket", "RightBracket", "Semicolon", "Quote", "Comma", "Period", "Slash", "Backslash", "Backquote"];
        if let Some(i) = PLAIN.chars().position(|p| p == c) {
            return get(NAMES[i]).map(|k| (k, false));
        }
        if let Some(i) = SHIFTED.chars().position(|p| p == c) {
            return get(NAMES[i + 1]).map(|k| (k, true));
        }
        if let Some(i) = DIGITS_SHIFTED.chars().position(|p| p == c) {
            return get(&i.to_string()).map(|k| (k, true));
        }
        None
    }
}

/// The checks a macro passes before it is saved, each refusal a sentence for the person; and the jumps its
/// if/else/loop steps make, worked out once.
pub mod macro_rules {
    use super::net::{is_null_or_white_space, len16};
    use super::*;
    use crate::core::macro_panel::macro_options;
    use crate::core::macro_values;

    pub const MAX_MACROS: i32 = 100;
    pub const MAX_NAME: i32 = 60;
    pub const MAX_STEPS: i32 = 5000;
    pub const MAX_IMAGE_SIDE: i32 = 256;
    pub const MAX_DEPTH: i32 = 20;
    pub const MAX_TYPE_TEXT: i32 = 200;
    pub const MAX_WAIT_MS: i32 = 3_600_000;
    /// Base64 of every picture in one macro together: they ride in the encrypted vault, saved every few seconds.
    const MAX_IMAGES_BASE64: i64 = 4 * 1024 * 1024;
    const TARGET_KINDS: [&str; 3] = ["selected", "all", "accounts"];
    const TRIGGER_KINDS: [&str; 4] = ["manual", "interval", "join", "rejoin"];
    const ON_FAILS: [&str; 5] = ["stop", "skip", "restart", "rejoin", "alert"];
    const OPS: [&str; 8] = ["<", "<=", "=", ">=", ">", "!=", "has", "!has"];
    // .NET's `$` also matches before a final "\n": `\n?\z` keeps that.
    static VAR_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z_][A-Za-z0-9_]{0,31}\n?\z").unwrap());
    static COLOUR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#[0-9a-fA-F]{6}\n?\z").unwrap());

    fn user(text: String) -> Fail {
        Fail::User(text)
    }

    fn within(v: Option<i32>, lo: i32, hi: i32) -> bool {
        matches!(v, Some(x) if x >= lo && x <= hi)
    }

    fn within_f(v: Option<f64>, lo: f64, hi: f64) -> bool {
        matches!(v, Some(x) if x >= lo && x <= hi)
    }

    /// Checks `m` and fills in defaults. `known` are the vault's accounts (others are
    /// dropped); `others` the other macros (a hotkey is one macro's).
    pub fn check(m: &mut Macro, known: &HashSet<Guid>, others: &[Macro]) -> Res<()> {
        m.name = m.name.trim().to_string();
        if m.name.is_empty() {
            return Err(user(l::t("Give the macro a name.")));
        }
        if len16(&m.name) > MAX_NAME as usize {
            m.name = net::cut16(&m.name, MAX_NAME as usize).trim_end().to_string();
        }
        if m.steps.is_empty() {
            return Err(user(l::t("Add at least one step.")));
        }
        if m.steps.len() > MAX_STEPS as usize {
            return Err(user(l::f("A macro can have up to {0} steps.", &[&MAX_STEPS])));
        }
        if !TARGET_KINDS.contains(&m.targets.as_str()) {
            m.targets = "selected".into();
        }
        let mut seen = HashSet::new();
        m.account_ids = m.account_ids.iter().copied().filter(|id| known.contains(id) && seen.insert(*id)).collect();
        if m.targets == "accounts" && m.account_ids.is_empty() {
            return Err(user(l::t("Pick at least one account for the macro.")));
        }
        if !TRIGGER_KINDS.contains(&m.trigger.kind.as_str()) {
            return Err(user(l::t("Unknown way to start the macro.")));
        }
        if m.trigger.kind == "interval" {
            if !within(m.trigger.every_seconds, 10, 86400) {
                return Err(user(l::t("Repeat every 10 seconds to 24 hours.")));
            }
        } else {
            m.trigger.every_seconds = None;
        }
        // A macro that starts by itself has no selection to go by.
        if m.trigger.kind != "manual" && m.targets == "selected" {
            return Err(user(l::t("A macro that starts by itself needs its accounts: choose All or pick them.")));
        }
        if let Some(hotkey) = m.hotkey {
            if !(1..=9).contains(&hotkey) {
                return Err(user(l::t("The hotkey is Ctrl+Alt+1 to Ctrl+Alt+9.")));
            }
            if let Some(holder) = others.iter().find(|o| o.id != m.id && o.hotkey == Some(hotkey)) {
                return Err(user(l::f("Ctrl+Alt+{0} already runs {1}.", &[&hotkey, &holder.name])));
            }
        }
        if m.jitter_ms < 0 || m.jitter_ms > 500 {
            return Err(user(l::t("Random extra wait is 0 to 0.5 s.")));
        }
        if m.jitter_px < 0 || m.jitter_px > 20 {
            return Err(user(l::t("Random click offset is 0 to 20 px.")));
        }
        if m.max_minutes < 1 || m.max_minutes > 240 {
            return Err(user(l::t("A run lasts 1 to 240 minutes.")));
        }
        if !ON_FAILS.contains(&m.on_fail.as_str()) {
            m.on_fail = "stop".into();
        }
        if matches!(m.stuck_minutes, Some(s) if s <= 0) {
            m.stuck_minutes = None;
        }
        if matches!(m.stuck_minutes, Some(s) if s > 60) {
            return Err(user(l::t("Stuck after 1 to 60 minutes.")));
        }
        if let Some(size) = m.recorded_size {
            if size.w < 16 || size.w > 16384 || size.h < 16 || size.h > 16384 {
                m.recorded_size = None;
            }
        }
        let mut images = check_steps(&mut m.steps)?;
        macro_options::check(m)?;
        images += check_features(m)?;
        counter_labels(m);
        if images > MAX_IMAGES_BASE64 {
            return Err(user(l::t("The macro's pictures are too big together. Use smaller ones.")));
        }
        pairs(&m.steps)?;
        jumps(&m.steps)?;
        Ok(())
    }

    pub const MAX_FEATURES: i32 = 10;
    const FEATURE_FAILS: [&str; 2] = ["skip", "macro"];

    /// Each step checked and its note kept tidy; returns the size of their pictures.
    fn check_steps(steps: &mut [MacroStep]) -> Res<i64> {
        let mut images: i64 = 0;
        for (i, step) in steps.iter_mut().enumerate() {
            if step.is_null {
                return Err(user(l::f("Step {0} is empty.", &[&(i + 1)])));
            }
            match check_step(step) {
                Ok(()) => {}
                Err(Fail::User(e)) => return Err(user(l::f("Step {0}: {1}", &[&(i + 1), &e]))),
                Err(other) => return Err(other),
            }
            images += step.image.as_deref().map_or(0, len16) as i64 + step.test.as_ref().and_then(|t| t.image.as_deref()).map_or(0, len16) as i64;
            note(step);
        }
        Ok(images)
    }

    /// The features: each its own switch, its blocks closed and keys let go within its own steps (it plays whole,
    /// between the main steps), no If stuck part. Refusals name the feature by its switch's label. Returns the size of
    /// their pictures.
    fn check_features(m: &mut Macro) -> Res<i64> {
        if m.features.len() > MAX_FEATURES as usize {
            return Err(user(l::f("A macro can have up to {0} features.", &[&MAX_FEATURES])));
        }
        if m.steps.len() + m.features.iter().map(|f| f.steps.len()).sum::<usize>() > MAX_STEPS as usize {
            return Err(user(l::f("A macro can have up to {0} steps.", &[&MAX_STEPS])));
        }
        let mut images: i64 = 0;
        let mut ids: HashSet<String> = HashSet::new();
        let mut switches: HashSet<String> = HashSet::new();
        let options = &m.options;
        for f in m.features.iter_mut() {
            if !macro_values::NAME.is_match(&f.id) || !ids.insert(f.id.clone()) {
                return Err(user(l::t("A feature has no name.")));
            }
            let option = options.iter().find(|o| o.name == f.option && o.kind == "check");
            let Some(option) = option.filter(|_| switches.insert(f.option.clone())) else {
                return Err(user(l::t("A feature has lost its switch.")));
            };
            if f.every_ms.is_some() && !within(f.every_ms, 1000, 86_400_000) {
                return Err(user(l::f("{0}: play it every 1 second to 24 hours.", &[&option.label])));
            }
            if !FEATURE_FAILS.contains(&f.on_fail.as_str()) {
                f.on_fail = "skip".into();
            }
            let checked = (|| -> Res<i64> {
                let size = check_steps(&mut f.steps)?;
                if let Some(at) = f.steps.iter().position(|s| s.kind == "recover") {
                    return Err(user(l::f("Step {0}: “If stuck…” goes in the main steps only.", &[&(at + 1)])));
                }
                pairs(&f.steps)?;
                jumps(&f.steps)?;
                Ok(size)
            })();
            match checked {
                Ok(size) => images += size,
                Err(Fail::User(e)) => return Err(user(l::f("{0}: {1}", &[&option.label, &e]))),
                Err(other) => return Err(other),
            }
        }
        Ok(images)
    }

    /// The main steps and every feature's.
    pub fn all_steps(m: &Macro) -> impl Iterator<Item = &MacroStep> {
        m.steps.iter().chain(m.features.iter().flat_map(|f| f.steps.iter()))
    }

    /// The longest note a step carries (why it is there, shown under it in the editor).
    pub const MAX_NOTE: i32 = 80;

    /// A step's note: plain text up to `MAX_NOTE`, kept trimmed; anything else is dropped.
    fn note(s: &mut MacroStep) {
        let Some(note) = s.extra.get("note") else {
            return;
        };
        let text = match note {
            Json::String(t) => t.trim().to_string(),
            _ => String::new(),
        };
        let text: String = text.chars().filter(|ch| !ch.is_control()).collect();
        if text.is_empty() {
            s.extra.remove("note");
        } else {
            let kept = if len16(&text) > MAX_NOTE as usize { net::cut16(&text, MAX_NOTE as usize).trim_end().to_string() } else { text };
            s.extra.insert("note".into(), Json::String(kept));
        }
    }

    /// What the panel calls each counter ({ name: label }): a counter's name to up to 30 characters of plain
    /// text; anything else is dropped.
    fn counter_labels(m: &mut Macro) {
        let extra = &mut m.panel.extra;
        let Some(labels) = extra.get("counterLabels") else {
            return;
        };
        let mut kept = serde_json::Map::new();
        if let Json::Object(labels) = labels {
            for (name, value) in labels {
                let text = match value {
                    Json::String(t) => t.chars().filter(|ch| !ch.is_control()).collect::<String>().trim().to_string(),
                    _ => String::new(),
                };
                if VAR_NAME.is_match(name) && !text.is_empty() && kept.len() < 100 {
                    let text = if len16(&text) > 30 { net::cut16(&text, 30).trim_end().to_string() } else { text };
                    kept.insert(name.clone(), Json::String(text));
                }
            }
        }
        if kept.is_empty() {
            extra.remove("counterLabels");
        } else {
            extra.insert("counterLabels".into(), Json::Object(kept));
        }
    }

    fn check_step(s: &mut MacroStep) -> Res<()> {
        // A field naming a board value passes with a stand-in; the value is checked when it plays.
        let filled = macro_values::fill_step(s)?;
        check_step_filled(s)?;
        macro_values::unfill_step(s, &filled);
        Ok(())
    }

    fn check_step_filled(s: &mut MacroStep) -> Res<()> {
        match s.kind.as_str() {
            "key" => {
                need_key(s.key.as_deref(), false)?;
                s.ms = if s.ms.is_none() || s.ms == Some(0) {
                    Some(60)
                } else if within(s.ms, 30, 10_000) {
                    s.ms
                } else {
                    return Err(user(l::t("Hold a key 0.03 to 10 s.")));
                };
            }
            "down" | "up" => need_key(s.key.as_deref(), true)?,
            "click" => {
                need_point(s.x, s.y)?;
                s.button.get_or_insert_with(|| "left".into());
                if !matches!(s.button.as_deref(), Some("left" | "right" | "middle")) {
                    return Err(user(l::t("Unknown mouse button.")));
                }
                // A recorded click's picture of where it clicked: kept when it is a usable one, else dropped quietly.
                if s.image.as_deref().is_some_and(|i| !image_ok(i)) {
                    s.image = None;
                }
                // A plain click (none, 0), or held long enough to be a hold.
                if matches!(s.hold_ms, Some(h) if h < 0 || (h > 0 && h < 30) || h > 10_000) {
                    return Err(user(l::t("Hold the button 0.03 to 10 s.")));
                }
                if s.hold_ms == Some(0) {
                    s.hold_ms = None;
                }
            }
            "clickImage" => {
                need_image(s.image.as_deref())?;
                s.button.get_or_insert_with(|| "left".into());
                if !matches!(s.button.as_deref(), Some("left" | "right")) {
                    return Err(user(l::t("Unknown mouse button.")));
                }
                need_region(s.region.as_ref())?;
                s.threshold = Some(threshold(s.threshold)?);
                s.timeout_ms = Some(timeout(s.timeout_ms)?);
            }
            "camera" => {
                if !within(s.dx, -100_000, 100_000) || !within(s.dy, -100_000, 100_000) {
                    return Err(user(l::t("Turn the camera by at most 100000 in each direction.")));
                }
                s.ms = if within(s.ms, 0, 10_000) {
                    s.ms
                } else if s.ms.is_none() {
                    Some(200)
                } else {
                    return Err(user(l::t("A camera turn takes 0 to 10 s.")));
                };
                s.button.get_or_insert_with(|| "right".into());
                if !matches!(s.button.as_deref(), Some("right" | "none")) {
                    return Err(user(l::t("Unknown mouse button.")));
                }
            }
            "scroll" => {
                if !within(s.notches, -50, 50) || s.notches == Some(0) {
                    return Err(user(l::t("Scroll 1 to 50 notches up or down.")));
                }
            }
            "move" => need_point(s.x, s.y)?,
            "wait" => {
                if !within(s.ms, 0, MAX_WAIT_MS) {
                    return Err(user(l::t("Wait up to 1 hour.")));
                }
            }
            "waitImage" => {
                need_image(s.image.as_deref())?;
                need_region(s.region.as_ref())?;
                s.threshold = Some(threshold(s.threshold)?);
                s.timeout_ms = Some(timeout(s.timeout_ms)?);
            }
            "waitText" => {
                need_words(s.text.as_deref(), s.words.as_deref())?;
                need_region(s.region.as_ref())?;
                s.timeout_ms = Some(timeout(s.timeout_ms)?);
            }
            "waitColour" => {
                // A place, or an area searched for the colour.
                if s.region.is_none() {
                    need_point(s.x, s.y)?;
                }
                need_region(s.region.as_ref())?;
                need_colour(s.colour.as_deref())?;
                s.tolerance = Some(tolerance(s.tolerance)?);
                s.timeout_ms = Some(timeout(s.timeout_ms)?);
            }
            "waitStill" => {
                need_region(s.region.as_ref())?;
                if !within(s.ms, 100, 60_000) {
                    return Err(user(l::t("Still for 0.1 to 60 s.")));
                }
                s.timeout_ms = Some(timeout(s.timeout_ms)?);
            }
            "if" => check_test(s.test.as_mut())?,
            "loop" => {
                if !within(s.times, 0, 100_000) {
                    return Err(user(l::t("Repeat up to 100000 times.")));
                }
            }
            "loopUntil" => {
                check_test(s.test.as_mut())?;
                s.max.get_or_insert(0);
                if matches!(s.max, Some(m) if !(0..=100_000).contains(&m)) {
                    return Err(user(l::t("Repeat up to 100000 times.")));
                }
            }
            "else" | "endIf" | "endLoop" | "break" | "rejoin" | "stop" | "recover" => {}
            "type" => need_text(s.text.as_deref(), MAX_TYPE_TEXT)?,
            "setVar" | "addVar" => {
                need_var(s.name.as_deref())?;
                // A text (setVar only, "{name}" put in when it plays), a formula, or a number.
                if s.kind == "setVar" && s.text.is_some() {
                    if s.text.as_deref().is_some_and(|t| !t.is_empty()) {
                        need_text(s.text.as_deref(), MAX_TYPE_TEXT)?;
                    }
                    (s.value, s.expr) = (None, None);
                } else if let Some(expr) = s.expr.as_deref() {
                    let expr = expr.trim().to_string();
                    let problem = crate::core::macro_expr::problem(&expr);
                    s.expr = Some(expr);
                    if let Some(problem) = problem {
                        return Err(user(problem));
                    }
                    (s.value, s.text) = (None, None);
                } else if !s.value.is_some_and(f64::is_finite) {
                    return Err(user(l::t("Enter a number.")));
                } else {
                    s.text = None;
                }
            }
            "read" => {
                need_var(s.name.as_deref())?;
                if !matches!(s.mode.as_deref(), Some("number" | "text")) {
                    s.mode = Some("number".into());
                }
                s.timeout_ms = Some(timeout(s.timeout_ms)?);
                let from = s.from.get_or_insert_with(|| "screen".into()).clone();
                match from.as_str() {
                    "screen" => {
                        if s.region.is_none() && macro_values::ref_of(&s.extra, "region").is_none() {
                            return Err(user(l::t("Choose the area to read.")));
                        }
                        need_region(s.region.as_ref())?;
                        (s.file_name, s.url) = (None, None);
                    }
                    "clipboard" => (s.region, s.file_name, s.url) = (None, None, None),
                    "file" => {
                        need_file(s.file_name.as_deref())?;
                        (s.region, s.url) = (None, None);
                    }
                    "web" => {
                        need_url(s.url.as_deref())?;
                        (s.region, s.file_name) = (None, None);
                    }
                    _ => return Err(user(l::t("Choose where to read from."))),
                }
            }
            "write" => {
                need_text(s.text.as_deref(), MAX_TYPE_TEXT)?;
                match s.to.as_deref() {
                    Some("clipboard") => (s.file_name, s.url) = (None, None),
                    Some("file") => {
                        need_file(s.file_name.as_deref())?;
                        s.url = None;
                    }
                    Some("web") => {
                        need_url(s.url.as_deref())?;
                        s.file_name = None;
                    }
                    _ => return Err(user(l::t("Choose where to write."))),
                }
            }
            "call" => {
                if !s.macro_id.is_some_and(|callee| callee != Guid::nil()) {
                    return Err(user(l::t("Choose the macro to run.")));
                }
            }
            "alert" => {
                need_text(s.text.as_deref(), 200)?;
                if s.picture != Some(true) {
                    s.picture = None;
                }
            }
            "clickColour" => {
                need_colour(s.colour.as_deref())?;
                need_region(s.region.as_ref())?;
                s.tolerance = Some(tolerance(s.tolerance)?);
                s.button.get_or_insert_with(|| "left".into());
                if !matches!(s.button.as_deref(), Some("left" | "right")) {
                    return Err(user(l::t("Unknown mouse button.")));
                }
                s.min_size.get_or_insert(4);
                if matches!(s.min_size, Some(m) if !(1..=500).contains(&m)) {
                    return Err(user(l::t("The smallest patch is 1 to 500 px.")));
                }
                s.timeout_ms = Some(timeout(s.timeout_ms)?);
                if matches!(s.repeat_until_gone_ms, Some(r) if !(0..=MAX_WAIT_MS).contains(&r)) {
                    return Err(user(l::t("Wait up to 1 hour.")));
                }
                if s.repeat_until_gone_ms == Some(0) {
                    s.repeat_until_gone_ms = None;
                }
                if matches!(s.each_after_ms, Some(e) if !(0..=10_000).contains(&e)) {
                    return Err(user(l::t("Click after 0 to 10 seconds.")));
                }
                if s.each_after_ms.is_some() {
                    s.repeat_until_gone_ms.get_or_insert(1000); // each patch as it comes: goes on until none comes for a while
                }
            }
            "follow" => {
                need_region(s.region.as_ref())?;
                if s.region.is_none() {
                    return Err(user(l::t("Draw the area the bar is in.")));
                }
                need_colour(s.target.as_deref())?;
                crate::core::macro_follow::check_marker(s.marker.as_ref())?;
                s.tolerance = Some(tolerance(s.tolerance)?);
                s.button.get_or_insert_with(|| "left".into());
                if !matches!(s.button.as_deref(), Some("left" | "right" | "key")) {
                    return Err(user(l::t("Unknown mouse button.")));
                }
                if s.button.as_deref() == Some("key") {
                    need_key(s.key.as_deref(), false)?;
                } else {
                    s.key = None;
                }
                s.mode.get_or_insert_with(|| "onOff".into());
                if !matches!(s.mode.as_deref(), Some("onOff" | "smooth")) {
                    return Err(user(l::t("Unknown way to follow.")));
                }
                s.kp.get_or_insert(1.0);
                s.kd.get_or_insert(0.4);
                if !within_f(s.kp, 0.0, 10.0) || !within_f(s.kd, 0.0, 10.0) {
                    return Err(user(l::t("Strength and damping are 0 to 10.")));
                }
                if s.predict != Some(true) {
                    s.predict = None;
                }
                s.fps.get_or_insert(60);
                if matches!(s.fps, Some(f) if !(15..=240).contains(&f)) {
                    return Err(user(l::t("Check 15 to 240 times a second.")));
                }
                s.until_gone_ms.get_or_insert(1000);
                if matches!(s.until_gone_ms, Some(u) if !(0..=60_000).contains(&u)) {
                    return Err(user(l::t("Gone for up to 60 s.")));
                }
                s.timeout_ms = Some(timeout(s.timeout_ms)?);
            }
            _ => return Err(user(l::t("Unknown step."))),
        }
        Ok(())
    }

    pub fn check_test(t: Option<&mut MacroTest>) -> Res<()> {
        let Some(t) = t else {
            return Err(user(l::t("Choose what to check.")));
        };
        let filled = macro_values::fill_test(t)?;
        check_test_filled(t)?;
        macro_values::unfill_test(t, &filled);
        Ok(())
    }

    fn check_test_filled(t: &mut MacroTest) -> Res<()> {
        match t.kind.as_str() {
            "image" => {
                need_image(t.image.as_deref())?;
                need_region(t.region.as_ref())?;
                t.threshold = Some(threshold(t.threshold)?);
            }
            "text" => {
                need_words(t.text.as_deref(), t.words.as_deref())?;
                need_region(t.region.as_ref())?;
            }
            "colour" => {
                need_point(t.x, t.y)?;
                need_colour(t.colour.as_deref())?;
                t.tolerance = Some(tolerance(t.tolerance)?);
            }
            "var" => {
                need_var(t.name.as_deref())?;
                if !t.op.as_deref().is_some_and(|op| OPS.contains(&op)) {
                    return Err(user(l::t("Unknown comparison.")));
                }
                // A number, a word (a choice's option), or "$name".
                if let Some(Json::String(word)) = t.extra.get("value") {
                    let length = len16(word);
                    t.value = None;
                    if length == 0 || length > 40 {
                        return Err(user(l::t("Enter a number.")));
                    }
                } else if !t.value.is_some_and(f64::is_finite) {
                    return Err(user(l::t("Enter a number.")));
                }
            }
            _ => return Err(user(l::t("Choose what to check."))),
        }
        Ok(())
    }

    /// The mouse buttons a Hold / Release step may press, by the names a key has there (in C#'s order).
    pub const MOUSE_KEYS: [(&str, &str); 3] = [("MouseLeft", "left"), ("MouseRight", "right"), ("MouseMiddle", "middle")];

    /// `MouseKeys.TryGetValue(name)`: the button a Hold / Release key name presses.
    pub fn mouse_key(name: &str) -> Option<&'static str> {
        MOUSE_KEYS.iter().find(|(k, _)| *k == name).map(|(_, b)| *b)
    }

    fn need_key(key: Option<&str>, allow_alt: bool) -> Res<()> {
        if allow_alt && key.is_some_and(|k| mouse_key(k).is_some()) {
            return Ok(());
        }
        let Some(key) = key.filter(|k| macro_keys::contains(k)) else {
            return Err(user(l::t("Choose a key.")));
        };
        if key == "Alt" && !allow_alt {
            return Err(user(l::t("Alt only works as Hold and Release, around another key.")));
        }
        Ok(())
    }

    fn need_point(x: Option<f64>, y: Option<f64>) -> Res<()> {
        if !within_f(x, 0.0, 1.0) || !within_f(y, 0.0, 1.0) {
            return Err(user(l::t("The place must be inside the window.")));
        }
        Ok(())
    }

    fn need_region(r: Option<&MacroRect>) -> Res<()> {
        let Some(r) = r else {
            return Ok(());
        };
        if !(r.x >= 0.0 && r.y >= 0.0 && r.w > 0.0 && r.h > 0.0 && r.x + r.w <= 1.0001 && r.y + r.h <= 1.0001) {
            return Err(user(l::t("The area must be inside the window.")));
        }
        Ok(())
    }

    /// A text to read, or a list of them (any counts).
    fn need_words(text: Option<&str>, words: Option<&[String]>) -> Res<()> {
        let Some(words) = words else {
            return need_text(text, 100);
        };
        if words.len() > macro_options::MAX_CHOICES as usize || words.iter().any(|w| is_null_or_white_space(Some(w.as_str())) || len16(w) > 100) {
            return Err(user(l::f("A list has up to {0} words of 1 to 100 characters.", &[&macro_options::MAX_CHOICES])));
        }
        Ok(())
    }

    fn need_text(text: Option<&str>, max: i32) -> Res<()> {
        if is_null_or_white_space(text) {
            return Err(user(l::t("Enter the text.")));
        }
        let text = text.unwrap_or_default();
        if len16(text) > max as usize {
            return Err(user(l::f("Text can be up to {0} characters.", &[&max])));
        }
        if text.chars().any(char::is_control) {
            return Err(user(l::t("Text can't have line breaks or tabs.")));
        }
        Ok(())
    }

    fn need_var(name: Option<&str>) -> Res<()> {
        if !name.is_some_and(|n| VAR_NAME.is_match(n)) {
            return Err(user(l::t("A counter's name is letters, digits and _ (up to 32), not starting with a digit.")));
        }
        Ok(())
    }

    fn need_file(name: Option<&str>) -> Res<()> {
        if !name.is_some_and(|n| crate::core::macro_outside::file_name_ok(n.trim())) {
            return Err(user(l::t("A file's name is letters, digits, spaces, - and _.")));
        }
        Ok(())
    }

    fn need_url(url: Option<&str>) -> Res<()> {
        // Uri.TryCreate(…, UriKind.Absolute) and its scheme https.
        let ok = url.is_some_and(|u| len16(u) <= 500 && url::Url::parse(u.trim()).is_ok_and(|uri| uri.scheme() == "https" && uri.has_host()));
        if !ok {
            return Err(user(l::t("A web address starts with https://.")));
        }
        Ok(())
    }

    pub const MAX_CALL_DEPTH: i32 = 5;

    /// The macros a macro runs (Run a macro steps) are saved ones, not itself, and none of them runs it back, at most
    /// `MAX_CALL_DEPTH` deep. `all` are the saved macros.
    pub fn check_calls(m: &Macro, all: &[Macro]) -> Res<()> {
        let mut by_id: HashMap<Guid, &Macro> = HashMap::new();
        for x in all.iter().filter(|x| x.id != m.id) {
            if by_id.insert(x.id, x).is_some() {
                // ToDictionary on two macros with one id.
                return Err(Fail::other("An item with the same key has already been added."));
            }
        }
        by_id.insert(m.id, m);
        walk(m, &by_id, m, &[m.id])
    }

    fn walk(m: &Macro, by_id: &HashMap<Guid, &Macro>, at: &Macro, chain: &[Guid]) -> Res<()> {
        for s in all_steps(at).filter(|s| s.kind == "call") {
            let id = s.macro_id.unwrap_or(Guid::nil());
            let Some(&callee) = by_id.get(&id) else {
                if std::ptr::eq(at, m) {
                    return Err(user(l::t("The macro to run isn't there any more. Choose another.")));
                }
                continue; // one of the others is out of date: it fails as it plays
            };
            if chain.contains(&id) {
                return Err(user(if std::ptr::eq(at, m) && id == m.id {
                    l::t("A macro can't run itself.")
                } else {
                    l::f("“{0}” runs this macro back: they would go round forever.", &[&at.name])
                }));
            }
            if chain.len() >= MAX_CALL_DEPTH as usize {
                return Err(user(l::f("Macros can run other macros {0} deep at most.", &[&MAX_CALL_DEPTH])));
            }
            let mut next = chain.to_vec();
            next.push(id);
            walk(m, by_id, callee, &next)?;
        }
        Ok(())
    }

    fn need_colour(colour: Option<&str>) -> Res<()> {
        if !colour.is_some_and(|c| COLOUR.is_match(c)) {
            return Err(user(l::t("Choose a color.")));
        }
        Ok(())
    }

    fn need_image(image: Option<&str>) -> Res<()> {
        if !image.is_some_and(image_ok) {
            return Err(user(l::f("Cut a picture of up to {0}×{0} px from the window.", &[&MAX_IMAGE_SIDE])));
        }
        Ok(())
    }

    /// A PNG (its header only is read) no bigger than `MAX_IMAGE_SIDE` on each side.
    pub fn image_ok(base64: &str) -> bool {
        if len16(base64) as i64 > MAX_IMAGES_BASE64 {
            return false;
        }
        let Some((w, h)) = png_size(base64) else {
            return false;
        };
        (1..=MAX_IMAGE_SIDE).contains(&w) && (1..=MAX_IMAGE_SIDE).contains(&h)
    }

    /// A PNG's width and height from its IHDR; null when it isn't one.
    pub fn png_size(base64: &str) -> Option<(i32, i32)> {
        if len16(base64) < 44 {
            return None;
        }
        let first: String = base64.chars().take(44).collect();
        // Convert.TryFromBase64String into 33 bytes.
        let head = net::from_base64(&first)?;
        if head.len() < 24 || head.len() > 33 {
            return None;
        }
        const SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        if head[..8] != SIGNATURE || head[12] != b'I' || head[13] != b'H' || head[14] != b'D' || head[15] != b'R' {
            return None;
        }
        let w = i32::from_be_bytes([head[16], head[17], head[18], head[19]]);
        let h = i32::from_be_bytes([head[20], head[21], head[22], head[23]]);
        Some((w, h))
    }

    fn threshold(threshold: Option<f64>) -> Res<f64> {
        match threshold {
            None => Ok(0.9),
            Some(t) if (0.5..=1.0).contains(&t) => Ok(t),
            Some(_) => Err(user(l::t("Match 50 to 100 %."))),
        }
    }

    fn timeout(timeout: Option<i32>) -> Res<i32> {
        match timeout {
            Some(t) if (0..=MAX_WAIT_MS).contains(&t) => Ok(t),
            None => Ok(10_000),
            Some(_) => Err(user(l::t("Wait up to 1 hour."))),
        }
    }

    fn tolerance(tolerance: Option<i32>) -> Res<i32> {
        match tolerance {
            None => Ok(24),
            Some(t) if (0..=255).contains(&t) => Ok(t),
            Some(_) => Err(user(l::t("Color closeness is 0 to 255."))),
        }
    }

    /// The key a Hold or Release step names as written: the key ("W"), or the setting it reads ("$o1").
    pub fn key_named(s: &MacroStep) -> String {
        match &s.key {
            Some(k) => k.clone(),
            None => macro_values::ref_of(&s.extra, "key").map(|option| format!("${option}")).unwrap_or_default(),
        }
    }

    /// C#'s HashSet<string> as `pairs` uses it: `First()` is the lowest slot in use, a removed key's slot taken by the
    /// next one added (last freed, first reused), as .NET's HashSet enumerates.
    #[derive(Default)]
    struct Held {
        slots: Vec<Option<String>>,
        free: Vec<usize>,
    }

    impl Held {
        fn add(&mut self, key: String) -> bool {
            if self.slots.iter().flatten().any(|k| *k == key) {
                return false;
            }
            match self.free.pop() {
                Some(i) => self.slots[i] = Some(key),
                None => self.slots.push(Some(key)),
            }
            true
        }
        fn remove(&mut self, key: &str) -> bool {
            let Some(i) = self.slots.iter().position(|k| k.as_deref() == Some(key)) else {
                return false;
            };
            self.slots[i] = None;
            self.free.push(i);
            true
        }
        fn first(&self) -> Option<&str> {
            self.slots.iter().flatten().next().map(String::as_str)
        }
    }

    /// Every key held (down) is let go (up) later, and only a held key is let go. A key from a setting pairs with
    /// the same setting only ("$o1" with "$o1"): two settings may hold different keys.
    fn pairs(steps: &[MacroStep]) -> Res<()> {
        let mut held = Held::default();
        for (i, s) in steps.iter().enumerate() {
            // The macro's own steps end where its If stuck part begins.
            if s.kind == "recover" {
                if let Some(first) = held.first() {
                    return Err(user(l::f("{0} is held down and never let go. Add a “Let go of a key” step.", &[&first])));
                }
            }
            if s.kind == "down" && !held.add(key_named(s)) {
                return Err(user(l::f("Step {0}: {1} is already held down.", &[&(i + 1), &key_named(s)])));
            }
            if s.kind == "up" && !held.remove(&key_named(s)) {
                return Err(user(l::f("Step {0}: lets go of {1}, which isn't held down.", &[&(i + 1), &key_named(s)])));
            }
        }
        if let Some(first) = held.first() {
            return Err(user(l::f("{0} is held down and never let go. Add a “Let go of a key” step.", &[&first])));
        }
        Ok(())
    }

    /// Where each control step goes: an "if" to its "else" or "endIf", an "else" to its "endIf", a loop to its "endLoop"
    /// and back, a "break" to its loop's "endLoop". Refuses steps that don't nest. -1: a step that doesn't jump.
    pub fn jumps(steps: &[MacroStep]) -> Res<Vec<i32>> {
        let mut jump = vec![-1i32; steps.len()];
        let mut open: Vec<(&str, usize)> = Vec::new(); // a stack: its top is the last
        let mut breaks: HashMap<usize, Vec<usize>> = HashMap::new(); // a loop's start → its breaks
        for i in 0..steps.len() {
            match steps[i].kind.as_str() {
                "if" => open.push(("if", i)),
                "else" => {
                    if open.last().is_none_or(|o| o.0 != "if") {
                        return Err(user(l::f("Step {0}: “Otherwise” needs an “If…” before it.", &[&(i + 1)])));
                    }
                    let at = open.pop().map(|o| o.1).unwrap_or_default();
                    jump[at] = i as i32;
                    open.push(("else", i));
                }
                "endIf" => {
                    if open.last().is_none_or(|o| o.0 != "if" && o.0 != "else") {
                        return Err(user(l::f("Step {0}: “End if” has no If to end.", &[&(i + 1)])));
                    }
                    let at = open.pop().map(|o| o.1).unwrap_or_default();
                    jump[at] = i as i32;
                }
                "loop" | "loopUntil" => {
                    open.push(("loop", i));
                    breaks.insert(i, Vec::new());
                }
                "endLoop" => {
                    if open.last().is_none_or(|o| o.0 != "loop") {
                        return Err(user(l::f("Step {0}: “End repeat” has no repeat to end.", &[&(i + 1)])));
                    }
                    let start = open.pop().map(|o| o.1).unwrap_or_default();
                    jump[start] = i as i32;
                    jump[i] = start as i32;
                    for &b in breaks.get(&start).map(Vec::as_slice).unwrap_or_default() {
                        jump[b] = i as i32;
                    }
                }
                "recover" => {
                    // The steps after it run only to get unstuck: it ends the macro's own steps, so it can't be inside one.
                    if !open.is_empty() {
                        return Err(user(l::f("Step {0}: “If stuck…” can't be inside a repeat or an If.", &[&(i + 1)])));
                    }
                    if steps.iter().position(|st| st.kind == "recover") != Some(i) {
                        return Err(user(l::f("Step {0}: a macro has one If stuck part.", &[&(i + 1)])));
                    }
                }
                "break" => {
                    // The innermost loop (a Stack enumerates from its top).
                    let Some(&(_, at)) = open.iter().rev().find(|o| o.0 == "loop") else {
                        return Err(user(l::f("Step {0}: “Leave the repeat” only works inside a repeat.", &[&(i + 1)])));
                    };
                    breaks.entry(at).or_default().push(i);
                }
                _ => {}
            }
            if open.len() > MAX_DEPTH as usize {
                return Err(user(l::f("Steps can be nested {0} deep at most.", &[&MAX_DEPTH])));
            }
        }
        if let Some(&(kind, at)) = open.last() {
            return Err(user(if kind == "loop" {
                l::f("Step {0}: this repeat never ends. Add “End repeat”.", &[&(at + 1)])
            } else {
                l::f("Step {0}: this If never ends. Add “End if”.", &[&(at + 1)])
            }));
        }
        Ok(jump)
    }

    /// A macro the page or a file sent, read leniently (the checks above decide what is kept).
    pub fn read(json: &Json) -> Res<Macro> {
        macro_values::read(json)
    }

    /// A copy, through its JSON (pictures and unknown fields too).
    pub fn copy(m: &Macro) -> Macro {
        serde_json::to_value(m).and_then(serde_json::from_value).unwrap_or_else(|_| {
            let mut c = m.clone();
            c.steps.iter_mut().chain(c.features.iter_mut().flat_map(|f| f.steps.iter_mut())).for_each(|s| s.owner = None);
            c
        })
    }

    /// The page's shape (camelCase), numbers given as text taken too (C#'s `PageJson` options): the vault's shape
    /// (the serde derives) renamed field by field, unknown fields ([JsonExtensionData]) left as they are.
    pub mod page_json {
        use serde::Serialize;
        use serde::de::DeserializeOwned;

        use super::super::net;
        use crate::core::base::{Fail, Json, Res};

        /// Which class a JSON object is.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Shape {
            Macro,
            Feature,
            Trigger,
            Size,
            Rect,
            Step,
            Test,
            Option,
            Area,
            Panel,
        }

        #[derive(Clone, Copy)]
        enum Kid {
            /// A string, a bool, a Guid, a list of strings or a JsonElement: as it is.
            Plain,
            /// int, int?, long?: a number given as text is taken.
            Int,
            /// double, double?: a number given as text is taken.
            Real,
            One(Shape),
            Many(Shape),
        }

        fn fields(shape: Shape) -> &'static [(&'static str, Kid)] {
            use Kid::*;
            match shape {
                Shape::Macro => &[
                    ("Id", Plain),
                    ("Name", Plain),
                    ("Steps", Many(Shape::Step)),
                    ("Targets", Plain),
                    ("AccountIds", Plain),
                    ("Trigger", One(Shape::Trigger)),
                    ("Enabled", Plain),
                    ("Hotkey", Int),
                    ("JitterMs", Int),
                    ("JitterPx", Int),
                    ("MaxMinutes", Int),
                    ("OnFail", Plain),
                    ("StuckMinutes", Int),
                    ("RecordedSize", One(Shape::Size)),
                    ("Options", Many(Shape::Option)),
                    ("Areas", Many(Shape::Area)),
                    ("Panel", One(Shape::Panel)),
                    ("Features", Many(Shape::Feature)),
                ],
                Shape::Feature => &[("Id", Plain), ("Option", Plain), ("EveryMs", Int), ("OnFail", Plain), ("Steps", Many(Shape::Step))],
                Shape::Trigger => &[("Kind", Plain), ("EverySeconds", Int)],
                Shape::Size => &[("W", Int), ("H", Int)],
                Shape::Rect => &[("X", Real), ("Y", Real), ("W", Real), ("H", Real)],
                Shape::Step => &[
                    ("Kind", Plain),
                    ("Key", Plain),
                    ("Ms", Int),
                    ("X", Real),
                    ("Y", Real),
                    ("Button", Plain),
                    ("Double", Plain),
                    ("Image", Plain),
                    ("Region", One(Shape::Rect)),
                    ("Threshold", Real),
                    ("TimeoutMs", Int),
                    ("Dx", Int),
                    ("Dy", Int),
                    ("Notches", Int),
                    ("Gone", Plain),
                    ("Text", Plain),
                    ("Colour", Plain),
                    ("Tolerance", Int),
                    ("Test", One(Shape::Test)),
                    ("Times", Int),
                    ("Max", Int),
                    ("Name", Plain),
                    ("Value", Real),
                    ("Expr", Plain),
                    ("MacroId", Plain),
                    ("From", Plain),
                    ("To", Plain),
                    ("FileName", Plain),
                    ("Url", Plain),
                    ("Target", Plain),
                    ("Marker", Plain),
                    ("Fps", Int),
                    ("UntilGoneMs", Int),
                    ("Mode", Plain),
                    ("Kp", Real),
                    ("Kd", Real),
                    ("Predict", Plain),
                    ("HoldMs", Int),
                    ("MinSize", Int),
                    ("RepeatUntilGoneMs", Int),
                    ("EachAfterMs", Int),
                    ("Words", Plain),
                    ("Picture", Plain),
                ],
                Shape::Test => &[
                    ("Kind", Plain),
                    ("Image", Plain),
                    ("Region", One(Shape::Rect)),
                    ("Threshold", Real),
                    ("Not", Plain),
                    ("Text", Plain),
                    ("Words", Plain),
                    ("X", Real),
                    ("Y", Real),
                    ("Colour", Plain),
                    ("Tolerance", Int),
                    ("Name", Plain),
                    ("Op", Plain),
                    ("Value", Real),
                ],
                // macro_panel.rs's classes (MacroPanel.cs), as the C# declares them.
                Shape::Option => &[
                    ("Name", Plain),
                    ("Label", Plain),
                    ("Kind", Plain),
                    ("Tab", Plain),
                    ("Value", Plain),
                    ("Default", Plain),
                    ("Min", Real),
                    ("Max", Real),
                    ("Unit", Plain),
                    ("Choices", Plain),
                ],
                Shape::Area => &[("Name", Plain), ("Label", Plain), ("Colour", Plain), ("Rect", One(Shape::Rect))],
                Shape::Panel => &[("AutoShow", Plain), ("PlaceId", Int), ("Tabs", Plain)],
            }
        }

        /// JsonNamingPolicy.CamelCase: the leading capital lowered ("JitterMs" → "jitterMs", "W" → "w").
        fn camel(name: &str) -> String {
            let mut chars = name.chars();
            match chars.next() {
                Some(c) => c.to_lowercase().chain(chars).collect(),
                None => String::new(),
            }
        }

        /// The vault's shape of a `shape` object to the page's.
        pub fn to_page(v: Json, shape: Shape) -> Json {
            let Json::Object(o) = v else {
                return v;
            };
            let list = fields(shape);
            let mut out = serde_json::Map::new();
            for (key, value) in o {
                match list.iter().find(|(f, _)| *f == key) {
                    Some(&(field, kid)) => {
                        let value = match kid {
                            Kid::One(s) => to_page(value, s),
                            Kid::Many(s) => match value {
                                Json::Array(items) => Json::Array(items.into_iter().map(|i| to_page(i, s)).collect()),
                                other => other,
                            },
                            _ => value,
                        };
                        out.insert(camel(field), value);
                    }
                    None => {
                        out.insert(key, value);
                    }
                }
            }
            Json::Object(out)
        }

        /// The page's shape (any case, numbers as text) of a `shape` object to the vault's.
        pub fn from_page(v: Json, shape: Shape) -> Json {
            let Json::Object(o) = v else {
                return v;
            };
            let list = fields(shape);
            let mut out = serde_json::Map::new();
            for (key, value) in o {
                match list.iter().find(|(f, _)| net::eq_ci(f, &key)) {
                    Some(&(field, kid)) => {
                        let value = match (kid, value) {
                            (Kid::Int, Json::String(text)) => match text.parse::<i64>() {
                                Ok(n) => Json::from(n),
                                Err(_) => Json::String(text),
                            },
                            (Kid::Real, Json::String(text)) => match net::parse_json_number(&text) {
                                Some(d) => serde_json::Number::from_f64(d).map(Json::Number).unwrap_or(Json::String(text)),
                                None => Json::String(text),
                            },
                            (Kid::One(s), value) => from_page(value, s),
                            (Kid::Many(s), Json::Array(items)) => Json::Array(items.into_iter().map(|i| from_page(i, s)).collect()),
                            (_, value) => value,
                        };
                        out.insert(field.to_string(), value);
                    }
                    None => {
                        out.insert(key, value);
                    }
                }
            }
            Json::Object(out)
        }

        /// JsonSerializer.SerializeToNode(x, PageJson).
        pub fn write<T: Serialize>(x: &T, shape: Shape) -> Json {
            to_page(serde_json::to_value(x).unwrap_or(Json::Null), shape)
        }

        /// JsonSerializer.Deserialize<T>(json, PageJson); a JsonException is a `Fail::Other`.
        pub fn read<T: DeserializeOwned>(v: &Json, shape: Shape) -> Res<T> {
            serde_json::from_value(from_page(v.clone(), shape)).map_err(|e| Fail::Other(e.to_string()))
        }
    }
}

/// What .NET did for the C# without being asked: string lengths in UTF-16 units, OrdinalIgnoreCase, double.ToString(),
/// lenient base64, how System.Text.Json writes numbers and takes nulls. Shared by the macro files.
pub(crate) mod net {
    use super::*;

    /// string.Length: UTF-16 units.
    pub(crate) fn len16(s: &str) -> usize {
        s.encode_utf16().count()
    }

    /// s[..n] in UTF-16 units (a character it would cut in two is left out).
    pub(crate) fn cut16(s: &str, n: usize) -> String {
        let mut used = 0;
        let mut out = String::new();
        for c in s.chars() {
            used += c.len_utf16();
            if used > n {
                break;
            }
            out.push(c);
        }
        out
    }

    /// string.IsNullOrWhiteSpace.
    pub(crate) fn is_null_or_white_space(s: Option<&str>) -> bool {
        s.is_none_or(|s| s.chars().all(char::is_whitespace))
    }

    /// One character as OrdinalIgnoreCase compares it (its invariant upper case when that is one character).
    pub(crate) fn fold_char(c: char) -> char {
        let mut up = c.to_uppercase();
        match (up.next(), up.next()) {
            (Some(u), None) => u,
            _ => c,
        }
    }

    pub(crate) fn fold(s: &str) -> Vec<char> {
        s.chars().map(fold_char).collect()
    }

    /// string.Equals(a, b, OrdinalIgnoreCase).
    pub(crate) fn eq_ci(a: &str, b: &str) -> bool {
        a.chars().map(fold_char).eq(b.chars().map(fold_char))
    }

    /// IndexOf(needle, OrdinalIgnoreCase), in characters.
    pub(crate) fn index_of_ci(hay: &[char], needle: &[char]) -> Option<usize> {
        if needle.is_empty() {
            return Some(0);
        }
        if needle.len() > hay.len() {
            return None;
        }
        (0..=hay.len() - needle.len()).find(|&i| hay[i..i + needle.len()] == *needle)
    }

    /// Contains(needle, OrdinalIgnoreCase).
    pub(crate) fn contains_ci(hay: &str, needle: &str) -> bool {
        index_of_ci(&fold(hay), &fold(needle)).is_some()
    }

    /// double.TryParse(text, NumberStyles.Float, CultureInfo.InvariantCulture): spaces around, a sign, a point, an
    /// exponent; "NaN" and "Infinity" as .NET spells them.
    pub(crate) fn parse_float(text: &str) -> Option<f64> {
        let t = text.trim();
        let (sign, rest) = match t.strip_prefix('-') {
            Some(r) => (-1.0, r),
            None => (1.0, t.strip_prefix('+').unwrap_or(t)),
        };
        if rest.eq_ignore_ascii_case("nan") {
            return Some(f64::NAN);
        }
        if rest.eq_ignore_ascii_case("infinity") || rest == "∞" {
            return Some(sign * f64::INFINITY);
        }
        if rest.starts_with(['+', '-']) || !rest.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-')) {
            return None;
        }
        t.parse::<f64>().ok()
    }

    /// A number System.Text.Json takes from text (AllowReadingFromString, no named literals).
    pub(crate) fn parse_json_number(text: &str) -> Option<f64> {
        if text.is_empty() || !text.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-')) {
            return None;
        }
        text.parse::<f64>().ok().filter(|d| d.is_finite())
    }

    /// double.ToString() (shortest round trip): "12", "0.5", "1E+20", "1E-05".
    pub(crate) fn double_text(d: f64) -> String {
        if d.is_nan() {
            return "NaN".into();
        }
        if d.is_infinite() {
            return if d > 0.0 { "∞".into() } else { "-∞".into() };
        }
        if d == 0.0 {
            return if d.is_sign_negative() { "-0".into() } else { "0".into() };
        }
        let sci = format!("{d:e}");
        let (mantissa, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
        let exp: i32 = exp.parse().unwrap_or(0);
        if !(-5..15).contains(&exp) {
            format!("{mantissa}E{}{:02}", if exp < 0 { '-' } else { '+' }, exp.abs())
        } else {
            format!("{d}")
        }
    }

    /// A JSON value's text as JsonElement.GetRawText() gives it (numbers as .NET wrote them).
    pub(crate) fn raw_text(v: &Json) -> String {
        match v {
            Json::Number(n) if n.is_i64() || n.is_u64() => n.to_string(),
            Json::Number(n) => double_text(n.as_f64().unwrap_or_default()),
            other => other.to_string(),
        }
    }

    static BASE64: base64::engine::GeneralPurpose = base64::engine::GeneralPurpose::new(
        &base64::alphabet::STANDARD,
        base64::engine::GeneralPurposeConfig::new().with_decode_allow_trailing_bits(true),
    );

    /// Convert.FromBase64String: white space inside is skipped; null when it isn't base64.
    pub(crate) fn from_base64(text: &str) -> Option<Vec<u8>> {
        use base64::Engine;
        let clean: String = text.chars().filter(|c| !matches!(c, ' ' | '\t' | '\r' | '\n')).collect();
        BASE64.decode(clean).ok()
    }

    /// How System.Text.Json writes a double: "1", not "1.0".
    fn write_double<S: Serializer>(d: f64, s: S) -> Result<S::Ok, S::Error> {
        if d.is_finite() && d.fract() == 0.0 && d.abs() < 1e15 && !(d == 0.0 && d.is_sign_negative()) {
            s.serialize_i64(d as i64)
        } else {
            s.serialize_f64(d)
        }
    }

    pub(crate) fn num_f64<S: Serializer>(d: &f64, s: S) -> Result<S::Ok, S::Error> {
        write_double(*d, s)
    }

    pub(crate) fn num_opt<S: Serializer>(d: &Option<f64>, s: S) -> Result<S::Ok, S::Error> {
        match d {
            Some(d) => write_double(*d, s),
            None => s.serialize_none(),
        }
    }

    /// A double as a JSON value, as System.Text.Json writes it.
    pub(crate) fn double_json(d: f64) -> Json {
        if d.is_finite() && d.fract() == 0.0 && d.abs() < 1e15 && !(d == 0.0 && d.is_sign_negative()) {
            Json::from(d as i64)
        } else {
            serde_json::Number::from_f64(d).map(Json::Number).unwrap_or(Json::Null)
        }
    }

    /// A class or list field sent null: what `??= new()` later makes of it.
    pub(crate) fn null_default<'de, D: Deserializer<'de>, T: Deserialize<'de> + Default>(d: D) -> Result<T, D::Error> {
        Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
    }

    /// A list of steps; one sent null is kept as a step marked null ("Step {0} is empty." when checked).
    pub(crate) fn steps_de<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<MacroStep>, D::Error> {
        let list: Option<Vec<Option<MacroStep>>> = Option::deserialize(d)?;
        Ok(list.unwrap_or_default().into_iter().map(|s| s.unwrap_or_else(|| MacroStep { is_null: true, ..MacroStep::default() })).collect())
    }

    pub(crate) fn steps_ser<S: Serializer>(steps: &[MacroStep], s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = s.serialize_seq(Some(steps.len()))?;
        for step in steps {
            if step.is_null { seq.serialize_element(&Json::Null)? } else { seq.serialize_element(step)? }
        }
        seq.end()
    }

    /// A list of features; one sent null has no name (as C#'s null one: "A feature has no name.").
    pub(crate) fn features_de<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<MacroFeature>, D::Error> {
        let list: Option<Vec<Option<MacroFeature>>> = Option::deserialize(d)?;
        Ok(list
            .unwrap_or_default()
            .into_iter()
            .map(|f| f.unwrap_or_else(|| MacroFeature { id: String::new(), on_fail: String::new(), ..MacroFeature::default() }))
            .collect())
    }

    /// A list of words; one sent null is "" (IsNullOrWhiteSpace refuses both).
    pub(crate) fn words_de<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<String>>, D::Error> {
        let list: Option<Vec<Option<String>>> = Option::deserialize(d)?;
        Ok(list.map(|l| l.into_iter().map(Option::unwrap_or_default).collect()))
    }
}
