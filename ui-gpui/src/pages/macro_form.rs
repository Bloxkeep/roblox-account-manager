// The macro form (the old MacroEditor.tsx and MacroPicture.tsx), open in place of Auto's list. Left: the macro's name
// and Try it, a template to start from (a new macro with no steps), the steps (add one by kind, a block with its end;
// open one to set what it does, its note and the values the player may change; move, duplicate, delete), a recording's
// steps, and what the player sees (the panel's settings). Right: the game (a still of a Roblox window from
// macroPicture: each step's numbered dot and each area's box, dragged there; a click sets the open step's point, a drag
// draws its area, a picture step cuts its picture from it; Edit on game draws the same on the window itself), the
// areas, Try it's outcome; then when and where it runs. The macro is held as the app keeps it (saveMacro's "macro"), a
// JSON value edited in place, so what this form doesn't draw (a feature's steps, the panel's tabs) rides along.
// What stops a save is marked on its step; the app checks again on Save (MacroRules.Check).
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    rc::Rc,
    sync::Arc,
};

use base64::Engine;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use crate::{i18n::I18n, kit::*, shell::Root, theme::*};
use crate::val::s;
use crate::val::arr;
use crate::ask::answer;

/// The kinds Add step offers, by group (macros.ts's STEP_GROUPS).
const GROUPS: [(&str, &[&str]); 6] = [
    ("keys", &["key", "down", "up", "type"]),
    ("mouse", &["click", "clickImage", "clickColour", "move", "camera", "scroll", "follow"]),
    ("wait", &["wait", "waitImage", "waitText", "waitColour", "waitStill"]),
    ("check", &["if", "else"]),
    ("flow", &["loop", "loopUntil", "break", "stop", "recover"]),
    ("other", &["setVar", "addVar", "read", "write", "call", "rejoin", "alert"]),
];
const OPS: [&str; 8] = [">=", "<=", "=", ">", "<", "!=", "has", "!has"];
const MAX_STEPS: usize = 5000;
/// From this many steps (a long recording) only the rows near the view are drawn; the rest is empty space of their
/// height (MacroEditor.tsx's DRAW_NEAR_FROM): 5000 rows drawn every frame made the editor crawl.
const DRAW_NEAR_FROM: usize = 600;
/// Drawn above and below the view.
const NEAR_PX: f32 = 900.;
const MAX_AREAS: usize = 8;
const MAX_OPTIONS: usize = 30;
const MAX_NOTE: usize = 80;
/// A picture step's picture, cut from a still: at most this many pixels each way.
const MAX_CUT: u32 = 256;
/// The open step's boxes, at most.
const BOXES: usize = 8;
/// Step pictures kept decoded for their thumbnails.
const THUMBS: usize = 24;
const MOUSE_KEYS: [&str; 3] = ["MouseLeft", "MouseRight", "MouseMiddle"];
const AREA_COLOURS: [&str; 8] = ["#22c55e", "#f59e0b", "#3b82f6", "#ec4899", "#a855f7", "#14b8a6", "#ef4444", "#eab308"];
/// The settings made on the panel, kept while no step names them.
const FREE: [&str; 4] = ["check", "choice", "list", "heading"];
/// The smallest box on the still (a share of the window).
const MIN_BOX: f32 = 0.02;
/// The icons a template may name that this page has.
const ICONS: [&str; 22] = [
    "keyboard", "mouse", "image", "clock", "search", "branch", "repeat", "stop", "refresh", "message", "gauge", "play", "bell", "tag", "palette", "sort", "eye", "edit", "wrench", "layers", "star", "gamepad",
];

type Spec = (&'static str, &'static str, Bx);

/// A still of the game: the picture, and its size in the window's pixels.
struct Pic {
    image: Arc<Image>,
    w: f32,
    h: f32,
}

/// What the app saw on the window while Try it ran (macroSeen): its frame and the boxes where things were found.
struct Seen {
    pic: Pic,
    boxes: Vec<[f32; 4]>,
}

struct Trial {
    id: String,
    account: String,
    since: i64,
    /// The steps it plays: a feature's (its id), or the main ones.
    sec: Option<String>,
}

/// A line of Try it's results (MacroPicture.tsx's TryLines): a step it got past ("Step 2 found it: …"), or where it
/// found nothing.
struct TryLine {
    step: usize,
    ok: bool,
    text: String,
}

#[derive(Clone, PartialEq)]
enum DragKind {
    /// A step's dot.
    Dot(usize),
    /// An area moved by its box, or sized by its corner.
    Move(String),
    Size(String),
    /// Around the thing a picture step looks for.
    Cut,
    /// The open step's area drawn.
    Draw(usize),
}

struct Drag {
    kind: DragKind,
    from: (f32, f32),
    to: (f32, f32),
    /// The dot's point or the box as it was ([x, y, w, h], shares of the window).
    orig: [f32; 4],
}

#[derive(Clone, Copy)]
enum ListBox {
    Area,
    Label,
    Choices,
    /// What the panel calls a counter (panel.counterLabels).
    Counter,
}

pub struct Form {
    /// The macro as the app keeps it.
    m: Value,
    is_new: bool,
    /// What the form held as it opened (`snapshot`): leaving with something else asks "Discard changes?" first.
    start: Option<Value>,
    name: Entity<InputState>,
    /// "Every": the number, and whether it counts minutes (else seconds).
    every: Entity<InputState>,
    every_min: bool,
    /// Stop after (minutes); the random extra wait (seconds) and click offset (px).
    max: Entity<InputState>,
    jit_ms: Entity<InputState>,
    jit_px: Entity<InputState>,
    /// The step open, its boxes (what each holds is the step's to say: `specs`) and its note.
    open: Option<usize>,
    boxes: Vec<Entity<InputState>>,
    note: Entity<InputState>,
    /// Add step's kinds shown; the open step's key list shown; its "Press a key" box listening.
    adding: bool,
    keys: bool,
    capture: bool,
    key_focus: FocusHandle,
    pub error: Option<String>,
    /// Save or Try was pressed: the form's own problems are said at its foot too.
    tried: bool,
    saving: bool,
    pub scroll: ScrollHandle,
    // ── the game ──
    /// The account whose window the still, Edit on game, Try it and Record use.
    account: String,
    picture: Option<Pic>,
    seen: Option<Seen>,
    taking: bool,
    /// The picture step being cut, and a drag on the still.
    cutting: Option<usize>,
    drag: Option<Drag>,
    frame: Rc<Cell<Option<Bounds<Pixels>>>>,
    picking: bool,
    /// The step a point or colour picked on the game goes to when the click comes (moved with the steps; None once
    /// that step is gone).
    pick_at: Option<usize>,
    on_game: bool,
    /// A line under the still (what to do now).
    say: Option<String>,
    // ── areas and settings: their name boxes, by name ──
    area_boxes: Vec<(String, Entity<InputState>)>,
    opt_boxes: Vec<(String, Entity<InputState>)>,
    choice_boxes: Vec<(String, Entity<InputState>)>,
    place: Entity<InputState>,
    /// "Area {n}" in the page's language.
    area_n: String,
    // ── templates, Try it, recording ──
    templates: Option<Vec<Value>>,
    trial: Option<Trial>,
    recording: bool,
    /// Recorded steps waiting: added to the end, in place of the steps, or thrown away.
    pending: Option<Value>,
    thumbs: RefCell<Vec<(u64, Arc<Image>)>>,
    // ── features, the panel's tabs, counters ──
    /// The list of steps shown: a feature's (its id), or the main steps (None).
    sec: Option<String>,
    feat_name: Entity<InputState>,
    feat_every: Entity<InputState>,
    feat_every_min: bool,
    /// The panel's tab shown, the shown tab's name box, and the setting whose "Move to tab" row is open.
    tab_sel: Option<String>,
    tab_name: Entity<InputState>,
    tab_menu: Option<String>,
    /// "Add to panel" open, and when it last closed (a click on its button then does not open it again at once).
    add_menu: bool,
    add_closed: Option<std::time::Instant>,
    /// New name… open on a step's counter (the step, where its name goes), and its two boxes.
    naming: Option<(usize, &'static str)>,
    counter_name: Entity<InputState>,
    counter_label: Entity<InputState>,
    counter_boxes: Vec<(String, Entity<InputState>)>,
    /// A step (an If or a Repeat with its block) dragged by its handle: where it is now and how many rows it takes.
    step_drag: Option<(usize, usize)>,
    /// How it runs shown in full under its one-line summary (closed at first, so a new macro's templates stay in view).
    run_open: bool,
    /// The step whose ⋮ menu is open, and the one a click outside just closed (its own ⋮ must not open it again).
    step_menu: Option<usize>,
    step_menu_shut: Option<(usize, std::time::Instant)>,
    /// Add step's search box (MacroEditor.tsx's AddList), and the place a row's "+" chose for the next step (None:
    /// after the open one, else at the end).
    search: Entity<InputState>,
    add_here: Option<usize>,
    /// Try it's lines so far, the step it was on when last drawn, and whether its end was seen (MacroEditor.tsx's
    /// lines, lastStep: written while drawing, as the web editor's effect did on each change of the run).
    lines: RefCell<Vec<TryLine>>,
    /// A long list (DRAW_NEAR_FROM steps on): where the rows' list stood in the window and the window's height, as the
    /// last frame measured them, and each step's rows' height once drawn (by its index).
    near_at: Rc<Cell<(f32, f32)>>,
    near_tall: Rc<RefCell<std::collections::HashMap<usize, f32>>>,
    last_step: Cell<Option<usize>>,
    trial_over: Cell<bool>,
    /// This form's number: a late reply (save, still, Try it, Record) for a form since closed is dropped.
    id: u64,
    /// The problems as last worked out, and the fingerprint of what they were worked out from.
    probs: RefCell<Option<(u64, Rc<Vec<Prob>>)>>,
}

/// The next form's number.
static FORMS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// The form open now, when it is still the one with this number.
fn form_of(r: &mut Root, id: u64) -> Option<&mut Form> {
    r.auto.macro_form.as_mut().filter(|f| f.id == id)
}

/// What the form holds as typed (the macro and the boxes' text): leaving with something else asks first. Not checked
/// for problems, so a macro that can't be saved yet still asks.
fn snapshot(f: &Form, cx: &App) -> Value {
    // Without the form's own marks: a block folded or unfolded is the view, not a change to the macro, and asked
    // "Discard changes?" on Back with nothing changed.
    let mut m = f.m.clone();
    strip_marks(&mut m);
    json!({
        "m": m,
        "boxes": [text(&f.name, cx), text(&f.every, cx), text(&f.max, cx), text(&f.jit_ms, cx), text(&f.jit_px, cx)],
        "everyMin": f.every_min,
    })
}

/// Whether leaving now would lose something.
fn changed(r: &Root, cx: &App) -> bool {
    r.auto.macro_form.as_ref().is_some_and(|f| Some(snapshot(f, cx)) != f.start)
}

const MAX_FEATURES: usize = 10;
const MAX_TABS: usize = 6;
/// The kinds Add step lists as used lately (MacroEditor.tsx's RECENT).
const RECENT: usize = 5;

thread_local! {
    /// The kinds added lately, newest first (MacroEditor.tsx's store "macro.recentKinds"; here kept while the app runs).
    static RECENT_KINDS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
}

fn remember(kind: &'static str) {
    RECENT_KINDS.with(|l| {
        let mut l = l.borrow_mut();
        l.retain(|k| *k != kind);
        l.insert(0, kind);
        l.truncate(RECENT);
    });
}

fn recent_kinds() -> Vec<&'static str> {
    RECENT_KINDS.with(|l| l.borrow().clone())
}

/// Other words a search finds a kind by (macros.ts's SYNONYMS).
fn synonyms(kind: &str) -> &'static [&'static str] {
    match kind {
        "key" => &["press", "hit", "bấm", "nhấn", "phím"],
        "down" => &["press", "bấm", "nhấn"],
        "click" => &["tap", "mouse", "bấm", "nhấn", "chuột"],
        "setVar" => &["variable", "formula", "math", "random", "biến", "công thức", "tính"],
        "addVar" => &["variable", "biến", "cộng"],
        "read" => &["ocr", "number", "coins", "money", "clipboard", "file", "web", "đọc", "số", "tiền"],
        "write" => &["clipboard", "file", "web", "webhook", "log", "save", "ghi", "lưu", "chép"],
        "call" => &["function", "sub", "run", "macro", "gọi", "chạy"],
        "wait" => &["sleep", "delay", "pause", "chờ", "đợi"],
        "waitImage" | "waitText" | "waitColour" | "waitStill" => &["chờ", "đợi"],
        "loop" => &["loop", "again", "lặp"],
        "loopUntil" => &["loop", "lặp"],
        "break" => &["exit", "leave", "thoát"],
        "if" => &["when", "check", "nếu"],
        "else" => &["otherwise", "ngược lại"],
        _ => &[],
    }
}

/// The kinds that fit where a step would go in (macros.ts's kindsAt): an Otherwise straight inside an If without one,
/// Leave the repeat inside a repeat, one If stuck, in the main steps only; in Add step's order.
fn kinds_fit(steps: &[Value], at: usize, feature: bool) -> Vec<&'static str> {
    let mut stack: Vec<(bool, bool)> = Vec::new();
    for st in steps.get(..at.min(steps.len())).unwrap_or(&[]) {
        match s(st, "kind") {
            "if" => stack.push((true, false)),
            "loop" | "loopUntil" => stack.push((false, false)),
            "else" => {
                if let Some(top) = stack.last_mut() {
                    top.1 = true;
                }
            }
            k if ender(k) => {
                stack.pop();
            }
            _ => {}
        }
    }
    let fits = |kind: &str| match kind {
        "else" => stack.last().is_some_and(|(is_if, has_else)| *is_if && !*has_else),
        "break" => stack.iter().any(|(is_if, _)| !*is_if),
        "recover" => !feature && !steps.iter().any(|st| s(st, "kind") == "recover"),
        _ => true,
    };
    GROUPS.iter().flat_map(|(_, kinds)| kinds.iter().copied()).filter(|k| fits(k)).collect()
}

/// Add step's search (MacroEditor.tsx's AddList): the kinds whose name, group or other words hold every word typed
/// (case and Vietnamese marks aside), those whose name starts with the first word first.
fn step_matches(root: &Root, q: &str, fit: &[&'static str]) -> Vec<&'static str> {
    let words: Vec<String> = crate::kit::folded(q.trim()).split_whitespace().map(str::to_string).collect();
    let Some(first) = words.first() else { return Vec::new() };
    let name = |k: &str| root.tr(&format!("macro.kind.{k}"));
    let mut out: Vec<&'static str> = Vec::new();
    for (g, kinds) in GROUPS {
        for k in kinds.iter().copied().filter(|k| fit.contains(k)) {
            let mut hay = vec![name(k), root.tr(&format!("macro.grp.{g}"))];
            hay.extend(synonyms(k).iter().map(|x| x.to_string()));
            let hay = crate::kit::folded(&hay.join(" "));
            if words.iter().all(|w| hay.contains(w.as_str())) {
                out.push(k);
            }
        }
    }
    out.sort_by_key(|k| !crate::kit::folded(&name(k)).starts_with(first.as_str()));
    out
}

// ── the form's own marks on a step, never saved: a folded block, a template's to-dos as they were when it opened ──

const FOLD: &str = "_fold";
const TODO_WAS: &str = "_todoWas";

fn is_folded(st: &Value) -> bool {
    st.get(FOLD).and_then(Value::as_bool) == Some(true)
}

/// A to-do's field as it is now (MacroEditor.tsx's sig): an area by its box on the game, a setting by its value.
fn todo_sig(m: &Value, st: &Value, field: &str) -> String {
    let named = |list: &str, name: &str, key: &str| arr(m, list).iter().find(|x| s(x, "name") == name).and_then(|x| x.get(key)).cloned().unwrap_or(Value::Null);
    let v = if field == "region" {
        match area_ref(st.get("region")) {
            Some(n) => named("areas", n, "rect"),
            None => st.get("region").cloned().unwrap_or(Value::Null),
        }
    } else {
        match opt_ref(st.get(field)) {
            Some(n) => named("options", n, "value"),
            None => st.get(field).cloned().unwrap_or(Value::Null),
        }
    };
    v.to_string()
}

/// A template's to-dos (the fields it marked on its steps, "todo") noted as they are now: each is done once changed.
fn mark_todos(m: &mut Value) {
    let src: &Value = m;
    let was: Vec<Option<Value>> = arr(src, "steps")
        .iter()
        .map(|st| {
            let fields: Vec<&str> = arr(st, "todo").iter().filter_map(Value::as_str).collect();
            (!fields.is_empty() && !has(st, TODO_WAS)).then(|| Value::Object(fields.iter().map(|f| (f.to_string(), json!(todo_sig(src, st, f)))).collect()))
        })
        .collect();
    if let Some(steps) = arr_mut(m, "steps") {
        for (st, w) in steps.iter_mut().zip(was) {
            if let Some(w) = w {
                set(st, TODO_WAS, w);
            }
        }
    }
}

/// The template's to-dos: the step each is on, its field, and whether it is done (MacroEditor.tsx's todoList).
fn todo_items(m: &Value) -> Vec<(usize, String, bool)> {
    let mut out = Vec::new();
    for (i, st) in arr(m, "steps").iter().enumerate() {
        let Some(was) = st.get(TODO_WAS).and_then(Value::as_object) else { continue };
        for field in arr(st, "todo").iter().filter_map(Value::as_str) {
            let done = was.get(field).and_then(Value::as_str).is_none_or(|w| w != todo_sig(m, st, field));
            out.push((i, field.to_string(), done));
        }
    }
    out
}

/// The macro as saved: each step keeps only the to-dos not done yet (MacroEditor.tsx's clean), the form's marks go.
fn finish_marks(m: &mut Value) {
    let src: &Value = m;
    let still: Vec<Option<Vec<String>>> = arr(src, "steps")
        .iter()
        .map(|st| {
            let was = st.get(TODO_WAS).and_then(Value::as_object)?;
            Some(arr(st, "todo").iter().filter_map(Value::as_str).filter(|f| was.get(*f).and_then(Value::as_str) == Some(todo_sig(src, st, f).as_str())).map(str::to_string).collect())
        })
        .collect();
    if let Some(steps) = arr_mut(m, "steps") {
        for (st, left) in steps.iter_mut().zip(still) {
            match left {
                Some(l) if l.is_empty() => drop_key(st, "todo"),
                Some(l) => set(st, "todo", json!(l)),
                None => {}
            }
        }
    }
    strip_marks(m);
}

/// The form's marks off every step (the main ones and each feature's).
fn strip_marks(m: &mut Value) {
    let clean = |l: &mut Vec<Value>| {
        for st in l {
            drop_key(st, FOLD);
            drop_key(st, TODO_WAS);
        }
    };
    if let Some(l) = arr_mut(m, "steps") {
        clean(l);
    }
    if let Some(fs) = arr_mut(m, "features") {
        for fe in fs {
            if let Some(l) = arr_mut(fe, "steps") {
                clean(l);
            }
        }
    }
}

/// What a template needs from the game before it works (MacrosView.tsx's needsText): a picture of the button, an area
/// on it, or that it is ready as it is.
fn needs_text(root: &Root, tp: &Value) -> String {
    let steps = arr(tp, "steps");
    let todo: HashSet<&str> = steps.iter().flat_map(|x| arr(x, "todo").iter().filter_map(Value::as_str)).collect();
    let pic = todo.contains("image") || steps.iter().any(|x| matches!(s(x, "kind"), "clickImage" | "waitImage") && s(x, "image").is_empty());
    let area = todo.contains("region") || !arr(tp, "areas").is_empty();
    let what: Vec<String> = [(pic, "macro.tpl.needs.pic"), (area, "macro.tpl.needs.area")].into_iter().filter(|(on, _)| *on).map(|(_, k)| root.tr(k)).collect();
    match what.as_slice() {
        [] => root.tr("macro.tpl.needs.none"),
        [one] => root.i18n.tv("macro.tpl.needs", &[("what", one)]),
        [a, b, ..] => root.i18n.tv("macro.tpl.needs", &[("what", &root.i18n.tv("macro.tpl.both", &[("a", a), ("b", b)]))]),
    }
}

// ───────────────────────── values ─────────────────────────

fn num(v: &Value, k: &str) -> f64 {
    v.get(k).and_then(Value::as_f64).unwrap_or(0.)
}

fn text(e: &Entity<InputState>, cx: &App) -> String {
    e.read(cx).value().to_string()
}

/// A field set on an object (nothing on anything else: bad data never crashes the window).
fn set(v: &mut Value, k: &str, x: Value) {
    if let Some(o) = v.as_object_mut() {
        o.insert(k.to_string(), x);
    }
}

/// An object's field that is an object itself (made one when it isn't).
fn obj_mut<'a>(v: &'a mut Value, k: &str) -> Option<&'a mut Value> {
    let e = v.as_object_mut()?.entry(k.to_string()).or_insert_with(|| json!({}));
    if !e.is_object() {
        *e = json!({});
    }
    Some(e)
}

fn arr_mut<'a>(v: &'a mut Value, k: &str) -> Option<&'a mut Vec<Value>> {
    if !v.get(k).is_some_and(Value::is_array) {
        set(v, k, json!([]));
    }
    v.get_mut(k).and_then(Value::as_array_mut)
}

fn drop_key(v: &mut Value, key: &str) {
    if let Some(o) = v.as_object_mut() {
        o.remove(key);
    }
}

fn has(v: &Value, k: &str) -> bool {
    v.get(k).is_some_and(|x| !x.is_null())
}

/// "1.5", "2", "0.06": a number without the zeros nobody typed.
fn trim(f: f64) -> String {
    let t = format!("{f:.2}");
    t.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn parse(t: &str) -> Option<f64> {
    t.trim().replace(',', ".").parse::<f64>().ok().filter(|n| n.is_finite())
}

/// Between lo and hi, and never a panic when the data says hi < lo.
fn cl(v: f32, lo: f32, hi: f32) -> f32 {
    v.max(lo).min(hi)
}

fn round4(v: f32) -> f64 {
    (v as f64 * 10000.).round() / 10000.
}

fn get<'a>(st: &'a Value, path: &str) -> Option<&'a Value> {
    match path.split_once('.') {
        Some((a, b)) => st.get(a)?.get(b),
        None => st.get(path),
    }
}

fn put(st: &mut Value, path: &str, v: Value) {
    match path.split_once('.') {
        Some((a, b)) => {
            if let Some(o) = obj_mut(st, a) {
                set(o, b, v);
            }
        }
        None => set(st, path, v),
    }
}

fn drop_at(st: &mut Value, path: &str) {
    match path.split_once('.') {
        Some((a, b)) => {
            if let Some(o) = st.get_mut(a) {
                drop_key(o, b);
            }
        }
        None => drop_key(st, path),
    }
}

fn ident(t: &str) -> bool {
    let mut ch = t.chars();
    ch.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && t.len() <= 24 && ch.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A setting a step's value stands for ("$o1"): its name.
fn opt_ref(v: Option<&Value>) -> Option<&str> {
    v.and_then(Value::as_str).and_then(|x| x.strip_prefix('$')).filter(|x| ident(x))
}

/// An area a step looks in ("@a1"): its name.
fn area_ref(v: Option<&Value>) -> Option<&str> {
    v.and_then(Value::as_str).and_then(|x| x.strip_prefix('@')).filter(|x| ident(x))
}

fn opt_at(m: &Value, v: Option<&Value>) -> Option<usize> {
    let name = opt_ref(v)?;
    arr(m, "options").iter().position(|o| s(o, "name") == name)
}

fn hexv(t: &str) -> Option<u32> {
    let h = t.trim().trim_start_matches('#');
    (h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit())).then(|| u32::from_str_radix(h, 16).ok()).flatten()
}

fn hex_c(t: &str) -> Hsla {
    hexv(t).map(c).unwrap_or(c(WHITE))
}

fn free_name(prefix: &str, taken: &[&str]) -> String {
    (1..).map(|n| format!("{prefix}{n}")).find(|x| !taken.contains(&x.as_str())).unwrap_or_default()
}

fn opener(kind: &str) -> bool {
    matches!(kind, "if" | "loop" | "loopUntil")
}

fn ender(kind: &str) -> bool {
    matches!(kind, "endIf" | "endLoop")
}

fn is_test(kind: &str) -> bool {
    matches!(kind, "if" | "loopUntil")
}

/// How far in each step sits: inside an If or a Repeat, one step in.
fn depths(steps: &[Value]) -> Vec<usize> {
    let mut d = 0usize;
    steps
        .iter()
        .map(|st| match s(st, "kind") {
            k if ender(k) => {
                d = d.saturating_sub(1);
                d
            }
            "else" => d.saturating_sub(1),
            k if opener(k) => {
                d += 1;
                d - 1
            }
            _ => d,
        })
        .collect()
}

/// The end of the block the step at `i` opens.
fn block_end(steps: &[Value], i: usize) -> Option<usize> {
    if !opener(s(steps.get(i)?, "kind")) {
        return None;
    }
    let mut d = 0;
    for (j, st) in steps.iter().enumerate().skip(i + 1) {
        match s(st, "kind") {
            k if opener(k) => d += 1,
            k if ender(k) => {
                if d == 0 {
                    // Closed by the other kind (an If by End repeat: only in a broken macro, an import say): no block,
                    // as macros.ts's blockEnd, so deleting the opener doesn't take everything up to that wrong end.
                    let want = if s(&steps[i], "kind") == "if" { "endIf" } else { "endLoop" };
                    return (k == want).then_some(j);
                }
                d -= 1;
            }
            _ => {}
        }
    }
    None
}

/// The rows a step takes when moved: an If or a Repeat with its whole block, else itself.
fn span(steps: &[Value], i: usize) -> (usize, usize) {
    (i, block_end(steps, i).unwrap_or(i))
}

/// The main steps' safe points (where the features switched on play): each "End repeat" of a repeat inside no other
/// block, before If stuck.
fn safe_ends(steps: &[Value]) -> Vec<usize> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    for (i, st) in steps.iter().enumerate() {
        match s(st, "kind") {
            "recover" => break,
            k if opener(k) => depth += 1,
            "endIf" => depth -= 1,
            "endLoop" => {
                if depth == 1 {
                    out.push(i);
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    out
}

/// A feature by its id.
fn feat_of<'a>(m: &'a Value, id: &str) -> Option<&'a Value> {
    arr(m, "features").iter().find(|f| s(f, "id") == id)
}

/// The features in their switches' order on the panel: the order they play in.
fn feats_in_order(m: &Value) -> Vec<&Value> {
    let opts = arr(m, "options");
    let at = |fe: &Value| opts.iter().position(|o| s(o, "name") == s(fe, "option")).unwrap_or(usize::MAX);
    let mut l: Vec<&Value> = arr(m, "features").iter().collect();
    l.sort_by_key(|fe| at(fe));
    l
}

/// A feature's name: its switch's label on the panel.
fn feat_label(root: &Root, m: &Value, fe: &Value) -> String {
    arr(m, "options").iter().find(|o| s(o, "name") == s(fe, "option")).map(|o| s(o, "label").trim()).filter(|l| !l.is_empty()).map(str::to_string).unwrap_or_else(|| root.tr("macro.feat.unnamed"))
}

/// A feature's switch is on.
fn feat_on(m: &Value, fe: &Value) -> bool {
    arr(m, "options").iter().find(|o| s(o, "name") == s(fe, "option")).and_then(|o| o.get("value")).is_some_and(|v| v.as_bool() == Some(true) || v.as_f64().is_some_and(|x| x != 0.))
}

/// A counter's name: a letter or _, then letters, digits or _, 32 at most.
fn counter_ok(t: &str) -> bool {
    let mut ch = t.chars();
    ch.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && t.len() <= 32 && ch.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// What the panel calls a counter.
fn counter_name<'a>(m: &'a Value, name: &'a str) -> &'a str {
    m.pointer("/panel/counterLabels").and_then(|l| l.get(name)).and_then(Value::as_str).filter(|x| !x.trim().is_empty()).unwrap_or(name)
}

/// What a new step of a kind starts as (macros.ts's newStep), a block with its end (stepsToAdd).
fn new_steps(kind: &str) -> Vec<Value> {
    let one = match kind {
        "key" => json!({ "kind": kind, "key": "Space", "ms": 60 }),
        "down" | "up" => json!({ "kind": kind, "key": "W" }),
        "click" => json!({ "kind": kind, "x": 0.5, "y": 0.5, "button": "left", "double": false }),
        "clickImage" => json!({ "kind": kind, "image": "", "button": "left", "threshold": 0.9, "timeoutMs": 5000 }),
        "camera" => json!({ "kind": kind, "dx": 200, "dy": 0, "ms": 300, "button": "right" }),
        "scroll" => json!({ "kind": kind, "notches": -3 }),
        "move" => json!({ "kind": kind, "x": 0.5, "y": 0.5 }),
        "wait" => json!({ "kind": kind, "ms": 1000 }),
        "waitImage" => json!({ "kind": kind, "image": "", "threshold": 0.9, "timeoutMs": 30000, "gone": false }),
        "waitText" => json!({ "kind": kind, "text": "", "timeoutMs": 30000, "gone": false }),
        "waitColour" => json!({ "kind": kind, "x": 0.5, "y": 0.5, "colour": "#ffffff", "tolerance": 24, "timeoutMs": 30000, "gone": false }),
        "waitStill" => json!({ "kind": kind, "ms": 2000, "timeoutMs": 30000 }),
        "if" => json!({ "kind": kind, "test": { "kind": "text", "text": "" } }),
        "loop" => json!({ "kind": kind, "times": 0 }),
        "loopUntil" => json!({ "kind": kind, "test": { "kind": "text", "text": "" }, "max": 0 }),
        "type" => json!({ "kind": kind, "text": "" }),
        "setVar" => json!({ "kind": kind, "name": "count", "value": 0 }),
        "addVar" => json!({ "kind": kind, "name": "count", "value": 1 }),
        "read" => json!({ "kind": kind, "name": "coins", "mode": "number", "from": "screen", "timeoutMs": 10000 }),
        "write" => json!({ "kind": kind, "to": "file", "fileName": "Bloxkeep log", "text": "" }),
        "alert" => json!({ "kind": kind, "text": "", "picture": false }),
        "follow" => json!({
            "kind": kind, "target": "#22c55e", "marker": "#ffffff", "tolerance": 24, "button": "left", "mode": "onOff", "predict": false,
            "kp": 1, "kd": 0.4, "fps": 60, "untilGoneMs": 1000, "timeoutMs": 30000
        }),
        "clickColour" => json!({ "kind": kind, "colour": "#ffffff", "tolerance": 24, "minSize": 4, "button": "left", "timeoutMs": 30000 }),
        _ => json!({ "kind": kind }),
    };
    match kind {
        "if" => vec![one, json!({ "kind": "endIf" })],
        "loop" | "loopUntil" => vec![one, json!({ "kind": "endLoop" })],
        _ => vec![one],
    }
}

fn new_test(kind: &str) -> Value {
    match kind {
        "image" => json!({ "kind": "image", "image": "", "threshold": 0.9 }),
        "colour" => json!({ "kind": "colour", "x": 0.5, "y": 0.5, "colour": "#ffffff", "tolerance": 24 }),
        "var" => json!({ "kind": "var", "name": "count", "op": ">=", "value": 1 }),
        _ => json!({ "kind": "text", "text": "" }),
    }
}

/// The values a step may hand to the player (MacroEditor's META): the setting's kind, range, unit and first label.
fn meta(kind: &str, path: &str) -> Option<(&'static str, Option<(i64, i64)>, Option<&'static str>, &'static str)> {
    Some(match (kind, path) {
        ("key", "key") => ("key", None, None, "macro.opt.key"),
        ("click" | "move", "point") => ("point", None, None, "macro.opt.point"),
        ("click", "holdMs") => ("number", Some((30, 10000)), Some("s"), "macro.opt.hold"),
        ("clickColour", "eachAfterMs") => ("number", Some((0, 10000)), Some("s"), "macro.opt.after"),
        ("wait", "ms") => ("number", Some((0, 3_600_000)), Some("s"), "macro.opt.wait"),
        ("waitColour" | "clickColour", "colour") => ("colour", None, None, "macro.opt.colour"),
        ("follow", "target") => ("colour", None, None, "macro.opt.target"),
        ("follow", "marker") => ("colour", None, None, "macro.opt.marker"),
        ("loop", "times") => ("number", Some((1, 100000)), Some(""), "macro.opt.times"),
        ("if" | "loopUntil", "test.value") => ("number", Some((0, 100000)), Some(""), "macro.opt.count"),
        _ => return None,
    })
}

/// The open step's values the player may be handed.
fn can_hand(st: &Value) -> Vec<&'static str> {
    let kind = s(st, "kind");
    let mut v: Vec<&'static str> = Vec::new();
    match kind {
        "key" => v.push("key"),
        "click" => {
            v.push("point");
            if has(st, "holdMs") {
                v.push("holdMs");
            }
        }
        "move" => v.push("point"),
        "clickColour" => {
            v.push("colour");
            if has(st, "eachAfterMs") {
                v.push("eachAfterMs");
            }
        }
        "wait" => v.push("ms"),
        "waitColour" => v.push("colour"),
        "follow" => {
            v.push("target");
            if !st.get("marker").is_some_and(Value::is_object) {
                v.push("marker");
            }
        }
        "loop" if st.get("times").is_some_and(|t| t.is_string() || t.as_f64().is_some_and(|x| x > 0.)) => v.push("times"),
        "if" | "loopUntil" if st.get("test").map(|t| s(t, "kind")) == Some("var") => v.push("test.value"),
        _ => {}
    }
    v
}

/// The step's own point (a numbered dot), when it has one: a click, a move, a colour waited for at a point.
fn point_of(st: &Value, m: &Value) -> Option<(f32, f32)> {
    let kind = s(st, "kind");
    let xy = |o: &Value| Some((o.get("x")?.as_f64()? as f32, o.get("y")?.as_f64()? as f32));
    if matches!(kind, "click" | "move") || (kind == "waitColour" && !has(st, "region")) {
        if let Some(i) = opt_at(m, st.get("x")) {
            return arr(m, "options").get(i).and_then(|o| o.get("value")).and_then(xy);
        }
        return xy(st);
    }
    let t = st.get("test")?;
    (is_test(kind) && s(t, "kind") == "colour" && !has(t, "region")).then(|| xy(t)).flatten()
}

/// Where the step names its area ("region", or its test's), when it looks somewhere.
fn region_path(st: &Value) -> Option<&'static str> {
    match s(st, "kind") {
        "waitStill" | "clickImage" | "waitImage" | "waitText" | "waitColour" | "clickColour" | "follow" => Some("region"),
        "read" if matches!(s(st, "from"), "" | "screen") => Some("region"),
        k if is_test(k) && st.get("test").is_some_and(|t| s(t, "kind") != "var") => Some("test.region"),
        _ => None,
    }
}

/// Where a picture step keeps its picture.
fn image_path(st: &Value) -> Option<&'static str> {
    match s(st, "kind") {
        "clickImage" | "waitImage" => Some("image"),
        k if is_test(k) && st.get("test").is_some_and(|t| s(t, "kind") == "image") => Some("test.image"),
        _ => None,
    }
}

fn area_rect(a: &Value) -> [f32; 4] {
    let r = a.get("rect");
    let g = |k: &str, d: f32| r.and_then(|r| r.get(k)).and_then(Value::as_f64).map_or(d, |x| x as f32);
    [g("x", 0.35), g("y", 0.35), g("w", 0.3), g("h", 0.3)]
}

/// The "$name"s the steps use.
fn refs(m: &Value) -> HashSet<String> {
    fn scan(v: &Value, out: &mut HashSet<String>) {
        match v {
            Value::String(_) => {
                if let Some(n) = opt_ref(Some(v)) {
                    out.insert(n.to_string());
                }
            }
            Value::Object(o) => o.values().for_each(|x| scan(x, out)),
            Value::Array(a) => a.iter().for_each(|x| scan(x, out)),
            _ => {}
        }
    }
    let mut out = HashSet::new();
    scan(m.get("steps").unwrap_or(&Value::Null), &mut out);
    for f in arr(m, "features") {
        scan(f.get("steps").unwrap_or(&Value::Null), &mut out);
    }
    out
}

/// Every "$name" in a step (its test's, its marker's) given back as the setting's value.
fn restore(v: &mut Value, name: &str, value: &Value, point: bool) {
    let Some(o) = v.as_object_mut() else { return };
    for (k, x) in o.iter_mut() {
        if x.is_object() {
            restore(x, name, value, point);
        } else if opt_ref(Some(x)) == Some(name) {
            *x = if point { value.get(k.as_str()).cloned().unwrap_or(json!(0.5)) } else { value.clone() };
        }
    }
}

// ───────────────────────── the open step's boxes ─────────────────────────

/// How one of the open step's boxes reads what is typed.
#[derive(Clone, Copy, PartialEq)]
enum Bx {
    Text,
    /// Kept in milliseconds, typed in seconds.
    Secs,
    Int,
    /// Kept 0–1, typed 0–100.
    Pct,
    /// A number, or (a value's) a formula, or (a test's) a word.
    Num,
    /// "#rrggbb".
    Colour,
    /// How close a colour must be: kept 0–255, typed 0–100 %.
    Close,
}

/// The open step's boxes: where each writes, what it is called, how it reads.
fn specs(st: &Value, m: &Value) -> Vec<Spec> {
    use Bx::*;
    let kind = s(st, "kind");
    let point: [Spec; 2] = [("x", "X %", Pct), ("y", "Y %", Pct)];
    let timeout: Spec = ("timeoutMs", "macro.e.giveUpL", Secs);
    let close: Spec = ("tolerance", "macro.tune.close", Close);
    let place = |v: &mut Vec<Spec>, how: &str| match how {
        "file" => v.push(("fileName", "macro.e.fileL", Text)),
        "web" => v.push(("url", "macro.e.urlL", Text)),
        _ => {}
    };
    let mut v: Vec<Spec> = Vec::new();
    match kind {
        "key" => v.push(("ms", "macro.e.keyHoldL", Secs)),
        "type" => v.push(("text", "macro.e.typeL", Text)),
        "click" => {
            v.extend(point);
            if has(st, "holdMs") {
                v.push(("holdMs", "macro.e.holdL", Secs));
            }
        }
        "move" => v.extend(point),
        "scroll" => v.push(("notches", "macro.e.timesL", Int)),
        "wait" => v.push(("ms", "macro.e.waitL", Secs)),
        "waitStill" => v.extend([("ms", "macro.e.waitL", Secs), timeout]),
        "waitText" => {
            if s(st, "words").is_empty() {
                v.push(("text", "macro.e.textL", Text));
            }
            v.push(timeout);
        }
        "clickImage" | "waitImage" => v.extend([timeout, ("threshold", "macro.e.matchL", Pct)]),
        "waitColour" => {
            v.push(("colour", "macro.e.colourL", Colour));
            if !has(st, "region") {
                v.extend(point);
            }
            v.extend([timeout, close]);
        }
        "clickColour" => {
            v.push(("colour", "macro.e.colourL", Colour));
            if has(st, "eachAfterMs") {
                v.push(("eachAfterMs", "macro.e.eachAfterL", Secs));
            }
            v.extend([timeout, close, ("minSize", "macro.e.minSizeL", Int)]);
        }
        "follow" => {
            v.push(("target", "macro.e.targetL", Colour));
            if st.get("marker").is_some_and(Value::is_object) {
                v.extend([("marker.left", "macro.e.leftL", Colour), ("marker.right", "macro.e.rightL", Colour)]);
            } else {
                v.push(("marker", "macro.e.markerL", Colour));
            }
            v.extend([("untilGoneMs", "macro.e.goneL", Secs), close, ("fps", "macro.e.fpsL", Int), timeout]);
        }
        "if" | "loopUntil" => {
            let t = st.get("test").unwrap_or(&Value::Null);
            match s(t, "kind") {
                "text" if s(t, "words").is_empty() => v.push(("test.text", "macro.e.textL", Text)),
                "var" => {
                    let o = arr(m, "options").iter().find(|o| s(o, "name") == s(t, "name"));
                    if !o.is_some_and(|o| matches!(s(o, "kind"), "check" | "choice")) {
                        let word = matches!(s(t, "op"), "has" | "!has");
                        v.push(("test.value", if word { "macro.e.wordL" } else { "macro.e.valueL" }, if word { Text } else { Num }));
                    }
                }
                "colour" => {
                    v.push(("test.colour", "macro.e.colourL", Colour));
                    if !has(t, "region") {
                        v.extend([("test.x", "X %", Pct), ("test.y", "Y %", Pct)]);
                    }
                }
                "image" => v.push(("test.threshold", "macro.e.matchL", Pct)),
                _ => {}
            }
            if kind == "loopUntil" && num(st, "max") > 0. {
                v.push(("max", "macro.e.atMostL", Int));
            }
        }
        "loop" if st.get("times").is_some_and(|t| t.is_string() || t.as_f64().is_some_and(|x| x > 0.)) => v.push(("times", "macro.e.timesL", Int)),
        "setVar" | "addVar" => {
            v.push(if st.get("text").is_some_and(Value::is_string) { ("text", "macro.e.textL", Text) } else { ("value", "macro.e.valueL", Num) });
        }
        "read" => {
            place(&mut v, s(st, "from"));
            v.push(timeout);
        }
        "write" => {
            v.push(("text", "macro.e.textL", Text));
            place(&mut v, Some(s(st, "to")).filter(|x| !x.is_empty()).unwrap_or("file"));
        }
        "alert" => v.push(("text", "macro.e.alertL", Text)),
        _ => {}
    }
    v.truncate(BOXES);
    v
}

/// What a box shows for a value (a value that is one of the macro's settings shows the setting's).
fn shown(st: &Value, path: &str, bx: Bx) -> String {
    if path == "value" {
        if let Some(e) = st.get("expr").and_then(Value::as_str) {
            return e.to_string();
        }
    }
    match get(st, path) {
        Some(Value::String(t)) => t.clone(),
        Some(v) => match (v.as_f64(), bx) {
            (Some(n), Bx::Secs) => trim(n / 1000.),
            (Some(n), Bx::Pct) => trim(n * 100.),
            (Some(n), Bx::Int) => trim(n.abs()),
            (Some(n), Bx::Close) => trim((n / 2.55).round()),
            (Some(n), _) => trim(n),
            (None, _) => String::new(),
        },
        None => String::new(),
    }
}

/// The same through a setting: what the box shows when the step's value is "$o1".
fn shown_in(m: &Value, st: &Value, path: &str, bx: Bx) -> String {
    let Some(o) = opt_at(m, get(st, path)).and_then(|i| arr(m, "options").get(i)) else { return shown(st, path, bx) };
    let v = o.get("value").cloned().unwrap_or(Value::Null);
    if s(o, "kind") == "point" {
        let last = path.rsplit('.').next().unwrap_or(path);
        return shown(&v, last, bx);
    }
    shown(&json!({ "v": v }), "v", bx)
}

/// What is typed in a box, written to the step; a number that doesn't read yet leaves the step as it was.
fn typed(st: &mut Value, path: &str, bx: Bx, t: &str) {
    if bx == Bx::Text {
        return put(st, path, json!(t));
    }
    let t = t.trim();
    if t.starts_with('$') {
        return put(st, path, json!(t));
    }
    if bx == Bx::Colour {
        if let Some(h) = hexv(t) {
            put(st, path, json!(format!("#{h:06x}")));
        }
        return;
    }
    match (bx, parse(t)) {
        (Bx::Secs, Some(n)) => put(st, path, json!((n.max(0.) * 1000.).round() as i64)),
        (Bx::Pct, Some(n)) => put(st, path, json!((n / 100.).clamp(0., 1.))),
        (Bx::Close, Some(n)) => put(st, path, json!((n.clamp(0., 100.) * 2.55).round() as i64)),
        (Bx::Int, Some(n)) => {
            let n = n.abs().round() as i64;
            // (A scroll keeps its way: down is below zero.)
            let n = if path == "notches" { n.max(1) * if num(st, "notches") > 0. { 1 } else { -1 } } else { n };
            put(st, path, json!(n));
        }
        (Bx::Num, Some(n)) => {
            if path == "value" {
                drop_key(st, "expr");
            }
            put(st, path, if n.fract() == 0. { json!(n as i64) } else { json!(n) });
        }
        // Not a number: a value's formula ("coins * 2"), a test's word ("has").
        (Bx::Num, None) if !t.is_empty() => {
            if path == "value" {
                set(st, "expr", json!(t));
            } else {
                put(st, path, json!(t));
            }
        }
        _ => {}
    }
}

impl Form {
    /// The steps shown: a feature's, or the main ones.
    fn steps(&self) -> &[Value] {
        match self.sec.as_deref().and_then(|id| feat_of(&self.m, id)) {
            Some(f) => arr(f, "steps"),
            None => arr(&self.m, "steps"),
        }
    }

    fn steps_mut(&mut self) -> Option<&mut Vec<Value>> {
        match self.sec.clone() {
            Some(id) if feat_of(&self.m, &id).is_some() => {
                let f = arr_mut(&mut self.m, "features")?.iter_mut().find(|f| s(f, "id") == id)?;
                arr_mut(f, "steps")
            }
            _ => arr_mut(&mut self.m, "steps"),
        }
    }

    /// Another list of steps shown (None: the main ones); what was open on the last one closes.
    fn switch_to(&mut self, to: Option<String>) {
        if self.sec == to {
            return;
        }
        self.sec = to.filter(|id| feat_of(&self.m, id).is_some());
        let every = self.sec.as_deref().and_then(|id| feat_of(&self.m, id)).and_then(|fe| fe.get("everyMs")).and_then(Value::as_f64);
        self.feat_every_min = every.is_some_and(|ms| ms >= 60000. && ms % 60000. == 0.);
        self.open = None;
        self.adding = false;
        self.keys = false;
        self.capture = false;
        self.cutting = None;
        self.naming = None;
        self.pick_at = None;
        self.step_drag = None;
        self.pending = None;
        self.add_here = None;
    }

    /// The steps moved: what points at a step by its place (the picture being cut, New name…'s step, a pick on the game
    /// on its way, the open ⋮ menu) follows it; one whose step went is dropped.
    fn remap(&mut self, map: impl Fn(usize) -> Option<usize>) {
        self.cutting = self.cutting.and_then(&map);
        self.naming = self.naming.and_then(|(i, p)| map(i).map(|i| (i, p)));
        self.pick_at = self.pick_at.and_then(&map);
        self.step_menu = self.step_menu.and_then(&map);
        // A row's "+" too: the next step added still goes beside that row, wherever it went.
        self.add_here = self.add_here.and_then(&map);
    }

    /// Steps a..=b gone from the list: those after them move up, those in it are dropped.
    fn remap_gone(&mut self, a: usize, b: usize) {
        self.remap(|o| if o < a { Some(o) } else if o > b { Some(o - (b - a + 1)) } else { None });
    }

    /// The panel's tabs (fewer than two: none) and the one shown.
    fn tabs(&self) -> Vec<String> {
        let t: Vec<String> = self.m.pointer("/panel/tabs").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
        if t.len() >= 2 { t } else { Vec::new() }
    }

    fn cur_tab(&self) -> Option<String> {
        let tabs = self.tabs();
        self.tab_sel.clone().filter(|x| tabs.contains(x)).or_else(|| tabs.first().cloned())
    }

    /// A new setting goes on the tab shown ("tab", unless that is the first).
    fn on_tab(&self, o: &mut Value) {
        let tabs = self.tabs();
        if let Some(t) = self.cur_tab().filter(|t| tabs.first() != Some(t)) {
            set(o, "tab", json!(t));
        }
    }

    fn add_tab(&mut self, first: &str, nth: &str, full: String) {
        let mut next = self.tabs();
        if next.len() >= MAX_TABS {
            self.error = Some(full);
            return;
        }
        if next.is_empty() {
            next.push(first.to_string());
        }
        let name = (next.len() + 1..).map(|n| nth.replace("{n}", &n.to_string())).find(|x| !next.iter().any(|y| y.to_lowercase() == x.to_lowercase())).unwrap_or_default();
        next.push(name.clone());
        if let Some(p) = obj_mut(&mut self.m, "panel") {
            set(p, "tabs", json!(next));
        }
        self.tab_sel = Some(name);
    }

    /// The shown tab renamed as typed: an empty name or another tab's is not taken (the old one stays).
    fn rename_tab(&mut self, nu: &str) {
        let nu: String = nu.trim().chars().take(20).collect();
        let tabs = self.tabs();
        let Some(old) = self.cur_tab() else { return };
        if nu.is_empty() || nu == old || tabs.iter().any(|x| *x != old && x.to_lowercase() == nu.to_lowercase()) {
            return;
        }
        let next: Vec<String> = tabs.iter().map(|x| if *x == old { nu.clone() } else { x.clone() }).collect();
        if let Some(p) = obj_mut(&mut self.m, "panel") {
            set(p, "tabs", json!(next));
        }
        if let Some(l) = arr_mut(&mut self.m, "options") {
            for o in l.iter_mut().filter(|o| s(o, "tab") == old) {
                set(o, "tab", json!(nu));
            }
        }
        self.tab_sel = Some(nu);
    }

    /// A tab taken away: its settings go to the first; one tab left is no tabs.
    fn remove_tab(&mut self, name: &str) {
        let next: Vec<String> = self.tabs().into_iter().filter(|x| x != name).collect();
        let keep = next.len() >= 2;
        let first = next.first().cloned().unwrap_or_default();
        if let Some(l) = arr_mut(&mut self.m, "options") {
            for o in l.iter_mut() {
                let t = s(o, "tab");
                if !(keep && !t.is_empty() && t != name && t != first) {
                    drop_key(o, "tab");
                }
            }
        }
        if let Some(p) = obj_mut(&mut self.m, "panel") {
            set(p, "tabs", json!(if keep { next } else { Vec::new() }));
        }
        if self.tab_sel.as_deref() == Some(name) {
            self.tab_sel = None;
        }
    }

    fn move_to_tab(&mut self, name: &str, tab: &str) {
        let first = self.tabs().first().cloned().unwrap_or_default();
        if let Some(o) = arr_mut(&mut self.m, "options").and_then(|l| l.iter_mut().find(|o| s(o, "name") == name)) {
            if tab == first {
                drop_key(o, "tab");
            } else {
                set(o, "tab", json!(tab));
            }
        }
        self.tab_menu = None;
    }

    /// A feature: a switch on the panel (on, so Try plays it at once) with a list of steps of its own, shown next.
    fn add_feature(&mut self, label: &str, full_feats: String, full_opts: String) {
        let feats = arr(&self.m, "features");
        if feats.len() >= MAX_FEATURES {
            self.error = Some(full_feats);
            return;
        }
        let opts = arr(&self.m, "options");
        if opts.len() >= MAX_OPTIONS {
            self.error = Some(full_opts);
            return;
        }
        let taken: Vec<&str> = opts.iter().map(|o| s(o, "name")).collect();
        let name = free_name("o", &taken);
        let labels: Vec<&str> = opts.iter().map(|o| s(o, "label")).collect();
        let label = (feats.len() + 1..).map(|n| label.replace("{n}", &n.to_string())).find(|l| !labels.contains(&l.as_str())).unwrap_or_default();
        let ids: Vec<&str> = feats.iter().map(|f| s(f, "id")).collect();
        let id = free_name("f", &ids);
        let mut o = json!({ "name": name, "label": label, "kind": "check", "value": 1, "default": 1 });
        self.on_tab(&mut o);
        if let Some(l) = arr_mut(&mut self.m, "options") {
            l.push(o);
        }
        if let Some(l) = arr_mut(&mut self.m, "features") {
            l.push(json!({ "id": id, "option": name, "everyMs": null, "onFail": "skip", "steps": [] }));
        }
        self.switch_to(Some(id));
    }

    /// A feature gone, its steps with it; its switch too, unless a step still reads it (then it stays a plain switch).
    fn drop_feature(&mut self, id: &str) {
        let Some(option) = feat_of(&self.m, id).map(|f| s(f, "option").to_string()) else { return };
        if self.sec.as_deref() == Some(id) {
            self.switch_to(None);
        }
        if let Some(l) = arr_mut(&mut self.m, "features") {
            l.retain(|f| s(f, "id") != id);
        }
        if self.readers_all(&option).is_empty() {
            if let Some(l) = arr_mut(&mut self.m, "options") {
                l.retain(|o| s(o, "name") != option);
            }
        }
    }

    fn feat_mut(&mut self, id: &str) -> Option<&mut Value> {
        arr_mut(&mut self.m, "features")?.iter_mut().find(|f| s(f, "id") == id)
    }

    /// A step (an If or a Repeat with its whole block) taken to the end of another list.
    fn move_to_list(&mut self, i: usize, to: Option<String>) {
        let (a, b) = span(self.steps(), i);
        let Some(moved) = self.steps().get(a..=b).map(<[Value]>::to_vec) else { return };
        if let Some(l) = self.steps_mut() {
            if b < l.len() {
                l.drain(a..=b);
            }
        }
        let target = match to.as_deref() {
            Some(id) => self.feat_mut(id).and_then(|f| arr_mut(f, "steps")),
            None => arr_mut(&mut self.m, "steps"),
        };
        if let Some(l) = target {
            l.extend(moved);
        }
        self.open = None;
        self.remap_gone(a, b);
    }

    /// Dragged over row j: the dragged rows move there at once (as the web editor did), and the open step goes with them.
    fn drag_over(&mut self, j: usize) {
        let Some((from, len)) = self.step_drag else { return };
        if j >= from && j < from + len {
            return;
        }
        let total = self.steps().len();
        if from + len > total || j >= total {
            return;
        }
        let to = if j < from { j } else { j + 1 - len };
        let Some(l) = self.steps_mut() else { return };
        let moved: Vec<Value> = l.drain(from..from + len).collect();
        for (k, st) in moved.into_iter().enumerate() {
            l.insert(to + k, st);
        }
        // The open step (and the rest pointing at a step) follows: inside the block, by its place in it; else as the
        // rest shifted round it.
        let follow = move |o: usize| {
            if o >= from && o < from + len {
                return Some(to + o - from);
            }
            let without = if o >= from + len { o - len } else { o };
            Some(if without >= to { without + len } else { without })
        };
        self.open = self.open.and_then(follow);
        self.remap(follow);
        self.step_drag = Some((to, len));
    }

    /// The counters the steps keep (Set, Add and Read steps' names), in every list.
    fn counters(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let lists = std::iter::once(arr(&self.m, "steps")).chain(arr(&self.m, "features").iter().map(|f| arr(f, "steps")));
        for st in lists.flatten() {
            if matches!(s(st, "kind"), "setVar" | "addVar" | "read") {
                let n = s(st, "name").trim();
                if !n.is_empty() && !out.iter().any(|x| x == n) {
                    out.push(n.to_string());
                }
            }
        }
        out
    }

    /// New name…'s Add: the counter goes on the step, and what the panel calls it when that was typed.
    fn new_counter(&mut self, name: &str, label: &str) -> Result<(), &'static str> {
        let Some((i, path)) = self.naming else { return Ok(()) };
        if !counter_ok(name) {
            return Err("macro.counter.nameHint");
        }
        if self.counters().iter().any(|c| c == name) {
            return Err("macro.counter.taken");
        }
        if let Some(st) = self.step_mut(i) {
            put(st, path, json!(name));
        }
        let label: String = label.trim().chars().take(30).collect();
        if !label.is_empty() && label != name {
            if let Some(l) = obj_mut(&mut self.m, "panel").and_then(|p| obj_mut(p, "counterLabels")) {
                set(l, name, json!(label));
            }
        }
        self.naming = None;
        Ok(())
    }

    fn step_mut(&mut self, i: usize) -> Option<&mut Value> {
        self.steps_mut()?.get_mut(i)
    }

    /// A step's value set, or the setting's when the value is one of the macro's settings ("$o1").
    fn put_val(&mut self, i: usize, path: &str, v: Value) {
        let at = self.steps().get(i).and_then(|st| opt_at(&self.m, get(st, path)));
        match at.and_then(|oi| arr_mut(&mut self.m, "options")?.get_mut(oi)) {
            Some(o) => {
                set(o, "value", v.clone());
                set(o, "default", v);
            }
            None => {
                if let Some(st) = self.step_mut(i) {
                    put(st, path, v);
                }
            }
        }
    }

    /// What was typed in the open step's nth box.
    fn box_typed(&mut self, n: usize, t: &str) {
        let Some(i) = self.open else { return };
        let Some(st) = self.steps().get(i) else { return };
        let Some(&(path, _, bx)) = specs(st, &self.m).get(n) else { return };
        let at = opt_at(&self.m, get(st, path));
        match at.and_then(|oi| arr_mut(&mut self.m, "options")?.get_mut(oi)) {
            Some(o) => {
                // The setting's value changes (a point's x or y; the rest whole).
                let point = s(o, "kind") == "point";
                let mut tmp = if point { o.get("value").cloned().filter(Value::is_object).unwrap_or(json!({ "x": 0.5, "y": 0.5 })) } else { json!({ "v": o.get("value").cloned().unwrap_or(Value::Null) }) };
                let sub = if point { path.rsplit('.').next().unwrap_or(path) } else { "v" };
                if t.trim().starts_with('$') {
                    return;
                }
                typed(&mut tmp, sub, bx, t);
                let v = if point { tmp } else { tmp.get("v").cloned().unwrap_or(Value::Null) };
                set(o, "value", v.clone());
                set(o, "default", v);
            }
            None => {
                if let Some(st) = self.step_mut(i) {
                    typed(st, path, bx, t);
                }
            }
        }
        self.error = None;
    }

    fn note_typed(&mut self, t: &str) {
        let Some(st) = self.open.and_then(|i| self.step_mut(i)) else { return };
        let t: String = t.chars().take(MAX_NOTE).collect();
        if t.trim().is_empty() {
            drop_key(st, "note");
        } else {
            set(st, "note", json!(t));
        }
    }

    /// Where Add step puts a step: where a row's "+" said, else after the open one, else at the end.
    fn add_at(&self) -> usize {
        let len = self.steps().len();
        match (self.add_here, self.open) {
            (Some(k), _) => k.min(len),
            (None, Some(i)) if i < len => i + 1,
            _ => len,
        }
    }

    /// An If or a Repeat folded to its one row, or opened again (MacroEditor.tsx's toggleFold).
    fn toggle_fold(&mut self, i: usize) {
        let Some(st) = self.step_mut(i) else { return };
        if is_folded(st) {
            drop_key(st, FOLD);
        } else {
            set(st, FOLD, json!(true));
        }
    }

    /// Every block around step i unfolded, so its row is drawn (MacroEditor.tsx's unfoldTo).
    fn unfold_to(&mut self, i: usize) {
        let around: Vec<usize> = (0..i.min(self.steps().len())).filter(|&j| is_folded(&self.steps()[j]) && block_end(self.steps(), j).is_some_and(|e| e >= i)).collect();
        for j in around {
            if let Some(st) = self.step_mut(j) {
                drop_key(st, FOLD);
            }
        }
    }

    fn add(&mut self, kind: &'static str, too_many: String) {
        let new = new_steps(kind);
        if self.steps().len() + new.len() > MAX_STEPS {
            self.error = Some(too_many);
            return;
        }
        remember(kind);
        // (If stuck's steps are the macro's last; it turns on starting again and the stuck watch.)
        let at = if kind == "recover" { self.steps().len() } else { self.add_at() };
        if kind == "recover" {
            set(&mut self.m, "onFail", json!("restart"));
            if num(&self.m, "stuckMinutes") <= 0. {
                set(&mut self.m, "stuckMinutes", json!(5));
            }
        }
        let Some(steps) = self.steps_mut() else { return };
        let at = at.min(steps.len());
        let n = new.len();
        for (k, st) in new.into_iter().enumerate() {
            steps.insert(at + k, st);
        }
        self.remap(|o| Some(if o >= at { o + n } else { o }));
        self.unfold_to(at);
        self.open = (!matches!(kind, "else" | "break" | "stop" | "rejoin" | "recover")).then_some(at);
        self.adding = false;
        self.add_here = None;
        self.keys = false;
    }

    /// One place up or down; an If or a Repeat goes with its whole block.
    fn move_step(&mut self, i: usize, dir: i32) {
        let len = self.steps().len();
        if i >= len {
            return;
        }
        let b = block_end(self.steps(), i).unwrap_or(i);
        let Some(steps) = self.steps_mut() else { return };
        if dir < 0 {
            if i == 0 {
                return;
            }
            let x = steps.remove(i - 1);
            steps.insert(b, x);
            // The block went up one; the step over it is now under it.
            self.remap(|o| Some(if (i..=b).contains(&o) { o - 1 } else if o == i - 1 { b } else { o }));
        } else {
            if b + 1 >= len {
                return;
            }
            let x = steps.remove(b + 1);
            steps.insert(i, x);
            self.remap(|o| Some(if (i..=b).contains(&o) { o + 1 } else if o == b + 1 { i } else { o }));
        }
        let to = if dir < 0 { i - 1 } else { i + 1 };
        self.open = match self.open {
            Some(o) if o == i => Some(to),
            _ => None,
        };
    }

    /// A copy right after it; a block is copied whole, to its end.
    fn dup_step(&mut self, i: usize, too_many: String) {
        let end = block_end(self.steps(), i).unwrap_or(i);
        let Some(copy) = self.steps().get(i..=end).map(<[Value]>::to_vec) else { return };
        if self.steps().len() + copy.len() > MAX_STEPS {
            self.error = Some(too_many);
            return;
        }
        let Some(steps) = self.steps_mut() else { return };
        let n = copy.len();
        for (k, st) in copy.into_iter().enumerate() {
            steps.insert(end + 1 + k, st);
        }
        self.remap(|o| Some(if o > end { o + n } else { o }));
        self.open = None;
    }

    /// The steps inside an If or a Repeat (its Otherwise and the ends not counted), as MacroEditor.tsx's fold shape.
    fn inner_steps(&self, i: usize) -> usize {
        let Some(end) = block_end(self.steps(), i) else { return 0 };
        self.steps()[i + 1..end].iter().filter(|x| !matches!(s(x, "kind"), "else" | "endIf" | "endLoop")).count()
    }

    /// The step goes; an If or a Repeat goes whole, the steps inside it too (MacroEditor.tsx's delStep, after it asked).
    fn del_step(&mut self, i: usize) {
        let end = block_end(self.steps(), i).unwrap_or(i);
        let mut gone = None;
        if let Some(steps) = self.steps_mut() {
            let end = end.min(steps.len().saturating_sub(1));
            if i <= end && i < steps.len() {
                steps.drain(i..=end);
                gone = Some(end);
            }
        }
        self.open = None;
        self.keys = false;
        self.cutting = None;
        if let Some(end) = gone {
            self.remap_gone(i, end);
        }
    }

    /// The step's point; one the player sets (a point setting) moves the setting's value.
    fn set_xy(&mut self, i: usize, x: f32, y: f32) {
        let (x, y) = (round4(cl(x, 0., 1.)), round4(cl(y, 0., 1.)));
        let Some(st) = self.steps().get(i) else { return };
        if is_test(s(st, "kind")) {
            if let Some(t) = self.step_mut(i).and_then(|st| obj_mut(st, "test")) {
                set(t, "x", json!(x));
                set(t, "y", json!(y));
            }
            return;
        }
        if opt_at(&self.m, st.get("x")).is_some() {
            return self.put_val(i, "x", json!({ "x": x, "y": y }));
        }
        if let Some(st) = self.step_mut(i) {
            set(st, "x", json!(x));
            set(st, "y", json!(y));
        }
    }

    /// A new area ("Area 3"), its name as a step names it ("@a3"); None when the macro has as many as it may.
    fn new_area(&mut self, rect: [f32; 4]) -> Option<String> {
        let areas = arr(&self.m, "areas");
        if areas.len() >= MAX_AREAS {
            return None;
        }
        let taken: Vec<&str> = areas.iter().map(|a| s(a, "name")).collect();
        let name = free_name("a", &taken);
        let labels: Vec<&str> = areas.iter().map(|a| s(a, "label")).collect();
        let label = (areas.len() + 1..).map(|n| self.area_n.replace("{n}", &n.to_string())).find(|l| !labels.contains(&l.as_str())).unwrap_or_default();
        let colour = AREA_COLOURS.get(areas.len() % AREA_COLOURS.len()).copied().unwrap_or("#22c55e");
        let rect = json!({ "x": round4(rect[0]), "y": round4(rect[1]), "w": round4(rect[2]), "h": round4(rect[3]) });
        arr_mut(&mut self.m, "areas")?.push(json!({ "name": name, "label": label, "colour": colour, "rect": rect }));
        Some(format!("@{name}"))
    }

    fn set_area_rect(&mut self, name: &str, r: [f32; 4]) {
        let rect = json!({ "x": round4(r[0]), "y": round4(r[1]), "w": round4(r[2]), "h": round4(r[3]) });
        if let Some(a) = arr_mut(&mut self.m, "areas").and_then(|l| l.iter_mut().find(|a| s(a, "name") == name)) {
            set(a, "rect", rect);
        }
    }

    /// Where the step looks: anywhere (or its point), an area, or a new area.
    fn set_region(&mut self, i: usize, path: &str, v: &str, full: &str) {
        let r = match v {
            "new" => match self.new_area([0.35, 0.35, 0.3, 0.3]) {
                Some(r) => Some(r),
                None => {
                    self.error = Some(full.to_string());
                    return;
                }
            },
            "point" | "anywhere" => None,
            x => Some(x.to_string()),
        };
        if let Some(st) = self.step_mut(i) {
            match r {
                Some(r) => put(st, path, json!(r)),
                None => drop_at(st, path),
            }
        }
    }

    /// Each step, in the main steps and every feature's, that `yes` picks: as "3", or "Feature 1 · 3".
    fn find_all(&self, yes: impl Fn(&Value) -> bool) -> Vec<String> {
        let mut out: Vec<String> = arr(&self.m, "steps").iter().enumerate().filter(|(_, st)| yes(st)).map(|(i, _)| (i + 1).to_string()).collect();
        for f in arr(&self.m, "features") {
            let label = arr(&self.m, "options").iter().find(|o| s(o, "name") == s(f, "option")).map(|o| s(o, "label")).unwrap_or("…");
            out.extend(arr(f, "steps").iter().enumerate().filter(|(_, st)| yes(st)).map(|(i, _)| format!("{label} · {}", i + 1)));
        }
        out
    }

    /// The steps that look in an area.
    fn area_users(&self, name: &str) -> Vec<String> {
        let at = format!("@{name}");
        self.find_all(|st| s(st, "region") == at || st.get("test").is_some_and(|t| s(t, "region") == at))
    }

    fn del_area(&mut self, name: &str) -> Result<(), Vec<String>> {
        let by = self.area_users(name);
        if !by.is_empty() {
            return Err(by);
        }
        if let Some(l) = arr_mut(&mut self.m, "areas") {
            l.retain(|a| s(a, "name") != name);
        }
        Ok(())
    }

    /// A value handed to the player (a setting made for it), or taken back.
    fn tick(&mut self, i: usize, path: &str, want: bool, label: &str, full: String) -> Result<(), String> {
        let Some(st) = self.steps().get(i) else { return Ok(()) };
        let kind = s(st, "kind").to_string();
        let raw_path = if path == "point" { "x" } else { path };
        let cur = get(st, raw_path).cloned();
        if want {
            if opt_ref(cur.as_ref()).is_some() {
                return Ok(());
            }
            let Some((okind, range, unit, _)) = meta(&kind, path) else { return Ok(()) };
            let opts = arr(&self.m, "options");
            if opts.len() >= MAX_OPTIONS {
                return Err(full);
            }
            let value = if path == "point" {
                let g = |k: &str| st.get(k).and_then(Value::as_f64).unwrap_or(0.5);
                json!({ "x": g("x"), "y": g("y") })
            } else {
                cur.unwrap_or(Value::Null)
            };
            let taken: Vec<&str> = opts.iter().map(|o| s(o, "name")).collect();
            let name = free_name("o", &taken);
            let labels: Vec<&str> = opts.iter().map(|o| s(o, "label")).collect();
            let label = std::iter::once(label.to_string()).chain((2..).map(|n| format!("{label} {n}"))).find(|l| !labels.contains(&l.as_str())).unwrap_or_default();
            let mut o = json!({ "name": name, "label": label, "kind": okind, "value": value, "default": value });
            if let Some((lo, hi)) = range {
                set(&mut o, "min", json!(lo));
                set(&mut o, "max", json!(hi));
            }
            if let Some(u) = unit {
                set(&mut o, "unit", json!(u));
            }
            self.on_tab(&mut o);
            if let Some(l) = arr_mut(&mut self.m, "options") {
                l.push(o);
            }
            let r = json!(format!("${name}"));
            if let Some(st) = self.step_mut(i) {
                if path == "point" {
                    set(st, "x", r.clone());
                    set(st, "y", r);
                } else {
                    put(st, path, r);
                }
            }
        } else {
            let Some(o) = opt_at(&self.m, cur.as_ref()).and_then(|oi| arr(&self.m, "options").get(oi)) else { return Ok(()) };
            let v = o.get("value").cloned().unwrap_or(Value::Null);
            if let Some(st) = self.step_mut(i) {
                if path == "point" {
                    set(st, "x", v.get("x").cloned().unwrap_or(json!(0.5)));
                    set(st, "y", v.get("y").cloned().unwrap_or(json!(0.5)));
                } else {
                    put(st, path, v);
                }
            }
            self.prune_opts();
        }
        Ok(())
    }

    /// The settings no step names go, but those made on the panel (a switch, a choice, a list, a heading).
    fn prune_opts(&mut self) {
        let used = refs(&self.m);
        if let Some(l) = arr_mut(&mut self.m, "options") {
            l.retain(|o| FREE.contains(&s(o, "kind")) || used.contains(s(o, "name")));
        }
    }

    /// A switch, a choice, a list or a heading added on the panel.
    fn add_free(&mut self, kind: &str, label: &str, choices: [String; 2]) -> bool {
        let opts = arr(&self.m, "options");
        if opts.len() >= MAX_OPTIONS {
            return false;
        }
        let taken: Vec<&str> = opts.iter().map(|o| s(o, "name")).collect();
        let name = free_name("o", &taken);
        let labels: Vec<&str> = opts.iter().map(|o| s(o, "label")).collect();
        let label = std::iter::once(label.to_string()).chain((2..).map(|n| format!("{label} {n}"))).find(|l| !labels.contains(&l.as_str())).unwrap_or_default();
        let value = match kind {
            "check" => json!(0),
            "choice" => json!(choices.first().cloned().unwrap_or_default()),
            "list" => json!([]),
            _ => Value::Null,
        };
        let mut o = json!({ "name": name, "label": label, "kind": kind, "value": value, "default": value });
        if matches!(kind, "choice" | "list") {
            set(&mut o, "choices", json!(choices));
        }
        self.on_tab(&mut o);
        arr_mut(&mut self.m, "options").map(|l| l.push(o)).is_some()
    }

    /// The steps, in every list, that read a setting made on the panel.
    fn readers_all(&self, name: &str) -> Vec<String> {
        let at = format!("${name}");
        self.find_all(|st| {
            let t = st.get("test");
            t.is_some_and(|t| s(t, "kind") == "var" && s(t, "name") == name) || s(st, "words") == at || t.is_some_and(|t| s(t, "words") == at)
        })
    }

    /// A setting taken off the panel: one a step reads stays; a value handed over goes back to its steps (in every list).
    fn remove_opt(&mut self, name: &str) -> Result<(), Vec<String>> {
        let Some(o) = arr(&self.m, "options").iter().find(|o| s(o, "name") == name) else { return Ok(()) };
        if FREE.contains(&s(o, "kind")) {
            let by = self.readers_all(name);
            if !by.is_empty() {
                return Err(by);
            }
        } else {
            let (value, point) = (o.get("value").cloned().unwrap_or(Value::Null), s(o, "kind") == "point");
            if let Some(steps) = arr_mut(&mut self.m, "steps") {
                for st in steps.iter_mut() {
                    restore(st, name, &value, point);
                }
            }
            if let Some(feats) = arr_mut(&mut self.m, "features") {
                for st in feats.iter_mut().filter_map(|f| arr_mut(f, "steps")).flat_map(|l| l.iter_mut()) {
                    restore(st, name, &value, point);
                }
            }
        }
        if let Some(l) = arr_mut(&mut self.m, "options") {
            l.retain(|o| s(o, "name") != name);
        }
        Ok(())
    }

    /// Up or down among the rows of its tab.
    fn move_opt(&mut self, name: &str, dir: i32) {
        let tabs = self.tabs();
        let tab_of = |o: &Value| -> String {
            let t = s(o, "tab");
            if !tabs.is_empty() && tabs.iter().any(|x| x == t) { t.to_string() } else { tabs.first().cloned().unwrap_or_default() }
        };
        let used = refs(&self.m);
        let Some(l) = arr_mut(&mut self.m, "options") else { return };
        let Some(i) = l.iter().position(|o| s(o, "name") == name) else { return };
        let Some(mine) = l.get(i).map(&tab_of) else { return };
        let shown = |o: &Value| FREE.contains(&s(o, "kind")) || used.contains(s(o, "name"));
        let mut j = i as i64 + dir as i64;
        while j >= 0 && (j as usize) < l.len() && l.get(j as usize).is_some_and(|o| !shown(o) || tab_of(o) != mine) {
            j += dir as i64;
        }
        if j >= 0 && (j as usize) < l.len() {
            l.swap(i, j as usize);
        }
    }

    /// An area's, a setting's name or a choice's options typed in their boxes.
    fn list_typed(&mut self, kind: ListBox, key: &str, t: &str) {
        let (list, field) = match kind {
            ListBox::Area => ("areas", "label"),
            ListBox::Label | ListBox::Choices => ("options", "label"),
            ListBox::Counter => {
                let label: String = t.trim().chars().take(30).collect();
                if let Some(l) = obj_mut(&mut self.m, "panel").and_then(|p| obj_mut(p, "counterLabels")) {
                    if label.is_empty() || label == key {
                        drop_key(l, key);
                    } else {
                        set(l, key, json!(label));
                    }
                }
                return;
            }
        };
        let Some(x) = arr_mut(&mut self.m, list).and_then(|l| l.iter_mut().find(|x| s(x, "name") == key)) else { return };
        match kind {
            ListBox::Choices => {
                let mut words: Vec<String> = Vec::new();
                for w in t.split(',').map(|w| w.trim().chars().take(30).collect::<String>()).filter(|w| !w.is_empty()) {
                    if !words.iter().any(|x| x.to_lowercase() == w.to_lowercase()) && words.len() < 20 {
                        words.push(w);
                    }
                }
                if words.is_empty() {
                    return;
                }
                let keep = |v: Option<&Value>| -> Value {
                    if s(x, "kind") == "list" {
                        json!(v.and_then(Value::as_array).map(|l| l.iter().filter(|w| w.as_str().is_some_and(|w| words.iter().any(|x| x == w))).cloned().collect::<Vec<_>>()).unwrap_or_default())
                    } else {
                        match v.and_then(Value::as_str) {
                            Some(w) if words.iter().any(|x| x == w) => json!(w),
                            _ => json!(words.first().cloned().unwrap_or_default()),
                        }
                    }
                };
                let (value, default) = (keep(x.get("value")), keep(x.get("default")));
                set(x, "value", value);
                set(x, "default", default);
                set(x, "choices", json!(words));
            }
            _ => set(x, field, json!(t.chars().take(30).collect::<String>())),
        }
        self.error = None;
    }
}

// ───────────────────────── the form's boxes ─────────────────────────

fn mk_box(hint: String, value: String, window: &mut Window, cx: &mut Context<Root>) -> Entity<InputState> {
    let e = cx.new(|cx| InputState::new(window, cx).placeholder(hint));
    if !value.is_empty() {
        e.update(cx, |st, cx| st.set_value(value, window, cx));
    }
    e
}

fn set_box(e: &Entity<InputState>, v: String, window: &mut Window, cx: &mut Context<Root>) {
    if text(e, cx) != v {
        e.update(cx, |b, cx| b.set_value(v, window, cx));
    }
}

/// A box for an area's name, a setting's name or a choice's options: what is typed goes to the macro.
fn list_box(kind: ListBox, key: String, value: String, window: &mut Window, cx: &mut Context<Root>) -> Entity<InputState> {
    let e = mk_box(String::new(), value, window, cx);
    cx.subscribe(&e, move |r: &mut Root, e, ev: &InputEvent, cx| {
        if matches!(ev, InputEvent::Change) {
            let t = e.read(cx).value().to_string();
            if let Some(f) = r.auto.macro_form.as_mut() {
                f.list_typed(kind, &key, &t);
            }
            cx.notify();
        }
    })
    .detach();
    e
}

/// The areas' and settings' boxes follow the macro's areas and settings (one made for each new one, gone with it).
fn sync_lists(f: &mut Form, window: &mut Window, cx: &mut Context<Root>) {
    let areas: Vec<(String, String)> = arr(&f.m, "areas").iter().map(|a| (s(a, "name").to_string(), s(a, "label").to_string())).collect();
    let opts: Vec<(String, String, String)> = arr(&f.m, "options")
        .iter()
        .map(|o| {
            let words: Vec<&str> = arr(o, "choices").iter().filter_map(Value::as_str).collect();
            (s(o, "name").to_string(), s(o, "label").to_string(), if matches!(s(o, "kind"), "choice" | "list") { words.join(", ") } else { String::new() })
        })
        .collect();
    let sync = |list: &mut Vec<(String, Entity<InputState>)>, want: Vec<(String, String)>, kind: ListBox, window: &mut Window, cx: &mut Context<Root>| {
        list.retain(|(k, _)| want.iter().any(|(n, _)| n == k));
        for (name, value) in want {
            match list.iter().find(|(k, _)| *k == name) {
                Some((_, e)) => {
                    // The words typed stay as typed (a choice's "a, b," reads as "a, b").
                    if !matches!(kind, ListBox::Choices) {
                        set_box(e, value, window, cx);
                    }
                }
                None => {
                    let e = list_box(kind, name.clone(), value, window, cx);
                    list.push((name, e));
                }
            }
        }
    };
    sync(&mut f.area_boxes, areas, ListBox::Area, window, cx);
    sync(&mut f.opt_boxes, opts.iter().map(|(n, l, _)| (n.clone(), l.clone())).collect(), ListBox::Label, window, cx);
    sync(&mut f.choice_boxes, opts.into_iter().filter(|(_, _, c)| !c.is_empty()).map(|(n, _, c)| (n, c)).collect(), ListBox::Choices, window, cx);
    let counters: Vec<(String, String)> = f
        .counters()
        .into_iter()
        .map(|c| {
            let label = counter_name(&f.m, &c).to_string();
            (c, label)
        })
        .collect();
    sync(&mut f.counter_boxes, counters, ListBox::Counter, window, cx);
}

/// The open step's boxes filled from the step, the lists' boxes from the macro.
fn fill(f: &mut Form, window: &mut Window, cx: &mut Context<Root>) {
    if let Some(st) = f.open.and_then(|i| f.steps().get(i)) {
        let vals: Vec<String> = specs(st, &f.m).into_iter().map(|(path, _, bx)| shown_in(&f.m, st, path, bx)).collect();
        let note = s(st, "note").to_string();
        for (n, v) in vals.into_iter().enumerate() {
            if let Some(b) = f.boxes.get(n) {
                set_box(b, v, window, cx);
            }
        }
        set_box(&f.note, note, window, cx);
    }
    sync_lists(f, window, cx);
    // The feature shown: its name (its switch's label) and how often it plays; the panel's tab shown: its name.
    if let Some(fe) = f.sec.as_deref().and_then(|id| feat_of(&f.m, id)) {
        let label = arr(&f.m, "options").iter().find(|o| s(o, "name") == s(fe, "option")).map(|o| s(o, "label").to_string()).unwrap_or_default();
        let every = fe.get("everyMs").and_then(Value::as_f64).map(|ms| trim(ms / if f.feat_every_min { 60000. } else { 1000. }));
        set_box(&f.feat_name, label, window, cx);
        if let Some(e) = every {
            set_box(&f.feat_every, e, window, cx);
        }
    }
    if let Some(tab) = f.cur_tab() {
        set_box(&f.tab_name, tab, window, cx);
    }
    // Add step's search starts empty each time it opens (the web editor's list was made anew).
    if !f.adding {
        set_box(&f.search, String::new(), window, cx);
    }
}

/// The settings' boxes from the macro (a template taken).
fn load_top(f: &mut Form, window: &mut Window, cx: &mut Context<Root>) {
    let secs = f.m.pointer("/trigger/everySeconds").and_then(Value::as_f64).unwrap_or(300.);
    f.every_min = secs >= 60. && secs % 60. == 0.;
    let every = trim(if f.every_min { secs / 60. } else { secs });
    let vals = [
        (f.name.clone(), s(&f.m, "name").to_string()),
        (f.every.clone(), every),
        (f.max.clone(), f.m.get("maxMinutes").and_then(Value::as_f64).map(trim).unwrap_or_default()),
        (f.jit_ms.clone(), f.m.get("jitterMs").and_then(Value::as_f64).map(|n| trim(n / 1000.)).unwrap_or_default()),
        (f.jit_px.clone(), f.m.get("jitterPx").and_then(Value::as_f64).map(trim).unwrap_or_default()),
    ];
    for (e, v) in vals {
        set_box(&e, v, window, cx);
    }
}

/// A new macro (None), or one of the saved ones to change.
pub fn open(r: &mut Root, m: Option<Value>, window: &mut Window, cx: &mut Context<Root>) {
    // (One with no id yet is new too: a macro read from a file, saved as a new one.)
    let is_new = m.as_ref().is_none_or(|m| m.get("id").and_then(Value::as_str).unwrap_or_default().is_empty());
    let blank = json!({
        "id": "", "name": "", "steps": [], "targets": "selected", "accountIds": [], "trigger": { "kind": "manual" }, "enabled": true, "hotkey": null,
        "jitterMs": 40, "jitterPx": 3, "maxMinutes": 10, "onFail": "stop", "stuckMinutes": 5, "options": [], "areas": [], "panel": { "autoShow": false, "placeId": null }
    });
    let m = match m.filter(Value::is_object) {
        // What it has over the blank's defaults (MacrosView.tsx's importFile: { ...blankMacro(), ...m, id: '' }).
        Some(Value::Object(given)) if is_new => {
            let mut all = blank;
            for (k, v) in given {
                all[k] = v;
            }
            all["id"] = json!("");
            all
        }
        Some(m) => m,
        None => blank,
    };
    let mut m = m;
    mark_todos(&mut m);
    let watch =|e: &Entity<InputState>, cx: &mut Context<Root>| {
        cx.subscribe(e, |_, _, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();
    };
    let name = mk_box(r.tr("macro.namePh"), String::new(), window, cx);
    let every = mk_box("5".into(), String::new(), window, cx);
    let max = mk_box("10".into(), String::new(), window, cx);
    let jit_ms = mk_box("0.04".into(), String::new(), window, cx);
    let jit_px = mk_box("3".into(), String::new(), window, cx);
    for e in [&name, &every, &max, &jit_ms, &jit_px] {
        watch(e, cx);
    }
    let boxes = (0..BOXES)
        .map(|n| {
            let e = cx.new(|cx| InputState::new(window, cx));
            cx.subscribe(&e, move |r: &mut Root, e, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::Change) {
                    let t = e.read(cx).value().to_string();
                    if let Some(f) = r.auto.macro_form.as_mut() {
                        f.box_typed(n, &t);
                    }
                    push_game(r);
                    cx.notify();
                }
            })
            .detach();
            e
        })
        .collect();
    let note = mk_box(r.tr("macro.note.ph"), String::new(), window, cx);
    cx.subscribe(&note, |r: &mut Root, e, ev: &InputEvent, cx| {
        if matches!(ev, InputEvent::Change) {
            let t = e.read(cx).value().to_string();
            if let Some(f) = r.auto.macro_form.as_mut() {
                f.note_typed(&t);
            }
            cx.notify();
        }
    })
    .detach();
    let place_now = m.pointer("/panel/placeId").and_then(Value::as_i64).map(|p| format!("https://www.roblox.com/games/{p}")).unwrap_or_default();
    // A feature's name (its switch's label: the Player sees row follows) and how often it plays; the shown tab's name;
    // New name…'s two boxes.
    let feat_name = mk_box(String::new(), String::new(), window, cx);
    cx.subscribe_in(&feat_name, window, |r: &mut Root, e, ev: &InputEvent, window, cx| {
        if matches!(ev, InputEvent::Change) {
            let t: String = e.read(cx).value().chars().take(30).collect();
            if let Some(f) = r.auto.macro_form.as_mut() {
                let opt = f.sec.as_deref().and_then(|id| feat_of(&f.m, id)).map(|fe| s(fe, "option").to_string());
                if let Some(o) = opt.and_then(|n| arr_mut(&mut f.m, "options")?.iter_mut().find(|o| s(o, "name") == n)) {
                    set(o, "label", json!(t));
                }
                sync_lists(f, window, cx);
            }
            cx.notify();
        }
    })
    .detach();
    let feat_every = mk_box("5".into(), String::new(), window, cx);
    cx.subscribe(&feat_every, |r: &mut Root, e, ev: &InputEvent, cx| {
        if matches!(ev, InputEvent::Change) {
            let t = e.read(cx).value().to_string();
            if let Some(f) = r.auto.macro_form.as_mut() {
                let unit = if f.feat_every_min { 60000. } else { 1000. };
                // (A number that doesn't read is kept as 0: the check says what's wrong.)
                let ms = parse(&t).map_or(0, |n| (n * unit).round() as i64);
                if let Some(fe) = f.sec.clone().and_then(|id| f.feat_mut(&id)) {
                    set(fe, "everyMs", json!(ms));
                }
            }
            cx.notify();
        }
    })
    .detach();
    let tab_name = mk_box(String::new(), String::new(), window, cx);
    cx.subscribe(&tab_name, |r: &mut Root, e, ev: &InputEvent, cx| {
        if matches!(ev, InputEvent::Change) {
            let t = e.read(cx).value().to_string();
            if let Some(f) = r.auto.macro_form.as_mut() {
                f.rename_tab(&t);
            }
            cx.notify();
        }
    })
    .detach();
    let counter_name = mk_box("coins".into(), String::new(), window, cx);
    let counter_label = mk_box(String::new(), String::new(), window, cx);
    for e in [&counter_name, &counter_label] {
        watch(e, cx);
    }
    let place = mk_box("https://www.roblox.com/games/…".into(), place_now, window, cx);
    cx.subscribe(&place, |r: &mut Root, e, ev: &InputEvent, cx| {
        if matches!(ev, InputEvent::Change) {
            let t = e.read(cx).value().to_string();
            let t = t.trim();
            let digits = t.split("games/").nth(1).unwrap_or(t);
            let id: String = digits.chars().take_while(char::is_ascii_digit).collect();
            if let Some(p) = r.auto.macro_form.as_mut().and_then(|f| obj_mut(&mut f.m, "panel")) {
                set(p, "placeId", if id.len() >= 3 { id.parse::<i64>().map_or(Value::Null, |n| json!(n)) } else { Value::Null });
            }
            cx.notify();
        }
    })
    .detach();
    // Add step's search: Enter adds the first kind it finds (MacroEditor.tsx's AddList).
    let search = mk_box(r.tr("macro.search"), String::new(), window, cx);
    cx.subscribe_in(&search, window, |r: &mut Root, e, ev: &InputEvent, window, cx| {
        match ev {
            InputEvent::Change => cx.notify(),
            InputEvent::PressEnter { .. } => {
                let q = e.read(cx).value().to_string();
                let too_many = r.i18n.tv("macro.err.tooMany", &[("max", &MAX_STEPS.to_string())]);
                let root: &Root = r;
                let first = root.auto.macro_form.as_ref().and_then(|f| step_matches(root, &q, &kinds_fit(f.steps(), f.add_at(), f.sec.is_some())).first().copied());
                if let (Some(kind), Some(f)) = (first, r.auto.macro_form.as_mut()) {
                    f.error = None;
                    f.add(kind, too_many);
                    fill(f, window, cx);
                    push_game(r);
                }
                cx.notify();
            }
            _ => {}
        }
    })
    .detach();
    let wins = windows(r);
    let mine: Vec<&str> = arr(&m, "accountIds").iter().filter_map(Value::as_str).collect();
    let account = wins.iter().find(|(id, _)| mine.contains(&id.as_str())).or(wins.first()).map(|(id, _)| id.clone()).unwrap_or_default();
    let adding = arr(&m, "steps").is_empty();
    let mut f = Form {
        m,
        is_new,
        start: None,
        name,
        every,
        every_min: false,
        max,
        jit_ms,
        jit_px,
        open: None,
        boxes,
        note,
        adding,
        keys: false,
        capture: false,
        key_focus: cx.focus_handle(),
        error: None,
        tried: false,
        saving: false,
        scroll: ScrollHandle::new(),
        account,
        picture: None,
        seen: None,
        taking: false,
        cutting: None,
        drag: None,
        frame: Rc::new(Cell::new(None)),
        picking: false,
        pick_at: None,
        on_game: false,
        say: None,
        area_boxes: Vec::new(),
        opt_boxes: Vec::new(),
        choice_boxes: Vec::new(),
        place,
        area_n: r.i18n.marked("macro.area.n", &[]),
        templates: None,
        trial: None,
        recording: false,
        pending: None,
        thumbs: RefCell::new(Vec::new()),
        sec: None,
        feat_name,
        feat_every,
        feat_every_min: false,
        tab_sel: None,
        tab_name,
        tab_menu: None,
        add_menu: false,
        add_closed: None,
        naming: None,
        counter_name,
        counter_label,
        counter_boxes: Vec::new(),
        step_drag: None,
        run_open: false,
        step_menu: None,
        step_menu_shut: None,
        search,
        add_here: None,
        lines: RefCell::new(Vec::new()),
        near_at: Rc::new(Cell::new((0., 0.))),
        near_tall: Rc::new(RefCell::new(std::collections::HashMap::new())),
        last_step: Cell::new(None),
        trial_over: Cell::new(false),
        id: FORMS.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        probs: RefCell::new(None),
    };
    load_top(&mut f, window, cx);
    fill(&mut f, window, cx);
    f.start = Some(snapshot(&f, cx));
    r.auto.macro_form = Some(f);
    if is_new {
        want_templates(r, cx);
    }
    cx.notify();
}

/// A saved macro opened at a step (the list's Fix on a run that stopped, MacrosView.tsx's editAt): that step's list
/// shown (the main steps, or a feature's), the step open.
pub fn open_at(r: &mut Root, m: Value, step: Option<usize>, sec: Option<String>, window: &mut Window, cx: &mut Context<Root>) {
    open(r, Some(m), window, cx);
    if let Some(f) = r.auto.macro_form.as_mut() {
        let sec = sec.filter(|id| feat_of(&f.m, id).is_some());
        f.switch_to(sec);
        let step = step.filter(|i| *i < f.steps().len());
        show_at(f, (step, f.sec.clone()), window, cx);
    }
    cx.notify();
}

/// Record from the list (MacrosView.tsx's record): with no Roblox window open it says so; else a new macro's form
/// opens and records on the window it uses.
pub fn record_new(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    if windows(r).is_empty() {
        r.toast = Some((r.tr("macro.need.record"), "info".into()));
        return cx.notify();
    }
    open(r, None, window, cx);
    record(r, cx);
}

/// The accounts with a Roblox window open: the list's Record asks which one when there are several (MacrosView.tsx).
pub fn record_windows(root: &Root) -> Vec<(String, String)> {
    windows(root)
}

/// Record from the list on the window picked.
pub fn record_on(r: &mut Root, acc: String, window: &mut Window, cx: &mut Context<Root>) {
    open(r, None, window, cx);
    if let Some(f) = r.auto.macro_form.as_mut() {
        f.account = acc;
    }
    record(r, cx);
}

/// No Roblox window to picture: off to Accounts to open one (app.js's goAccounts), asking first when Save would keep
/// something now.
fn go_accounts(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let changed = changed(r, cx);
    let go = |r: &mut Root, cx: &mut Context<Root>| {
        close(r, cx);
        r.go(crate::pages::Page::Accounts);
        cx.notify();
    };
    if changed {
        // Only if the form is still the one asked about: closed meanwhile (a save's answer), nobody wanted Accounts.
        r.ask_discard(window, cx, move |r, _, cx| {
            if r.auto.macro_form.is_some() {
                go(r, cx);
            }
        });
    } else {
        go(r, cx);
    }
}

/// The form closed: Edit on game ends, and the pictures it held go from GPUI's cache.
/// Back or Cancel: the form goes, asking first when Save would keep something now (a step, a name, a setting).
pub fn leave(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    if changed(r, cx) {
        r.ask_discard(window, cx, |r, _, cx| close(r, cx));
    } else {
        close(r, cx);
    }
}

pub fn close(r: &mut Root, cx: &mut Context<Root>) {
    if let Some(f) = r.auto.macro_form.take() {
        if f.on_game {
            r.send("editOnGame", json!({ "accountId": null }));
        }
        for p in [f.picture, f.seen.map(|x| x.pic)].into_iter().flatten() {
            p.image.remove_asset(cx);
        }
        for (_, t) in f.thumbs.into_inner() {
            t.remove_asset(cx);
        }
    }
    cx.notify();
}

/// A step of the form opened (a shot's way in).
#[cfg(feature = "shot")]
pub fn shot_step(r: &mut Root, i: usize, window: &mut Window, cx: &mut Context<Root>) {
    open_step(r, Some(i), window, cx);
}

/// A shot's further ways in: BLOXKEEP_SHOT_PIC=1 takes a still of the window (=cut: then the open step's picture is
/// being cut, and BLOXKEEP_SHOT_DRAG="x,y,x2,y2" drags on the still once it has come: a cut, a dot, an area);
/// BLOXKEEP_SHOT_ADDING=1 shows Add step's kinds; BLOXKEEP_SHOT_TRY=1 tries it; BLOXKEEP_SHOT_REC=1 records;
/// BLOXKEEP_SHOT_KEYCAP=1 listens for a key.
#[cfg(feature = "shot")]
pub fn shot_more(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    if let Ok(v) = std::env::var("BLOXKEEP_SHOT_PIC") {
        if let Some(f) = r.auto.macro_form.as_mut() {
            if v == "cut" {
                f.cutting = f.open;
            }
        }
        take_picture(r, window, cx);
    }
    if let Some(f) = r.auto.macro_form.as_mut() {
        if std::env::var("BLOXKEEP_SHOT_ADDING").is_ok() {
            f.adding = true;
        }
        if std::env::var("BLOXKEEP_SHOT_KEYCAP").is_ok() {
            f.capture = true;
        }
        // BLOXKEEP_SHOT_STEPMENU=n (counted from 1) opens that step's ⋮ menu; BLOXKEEP_SHOT_RUN=1 opens How it runs.
        if let Some(n) = std::env::var("BLOXKEEP_SHOT_STEPMENU").ok().and_then(|v| v.parse::<usize>().ok()).filter(|n| *n > 0) {
            f.step_menu = Some(n - 1);
        }
        if std::env::var("BLOXKEEP_SHOT_RUN").is_ok() {
            f.run_open = true;
        }
    }
    // BLOXKEEP_SHOT_FEATURE=1 adds a feature with two steps and shows it (=main: then the main steps again);
    // BLOXKEEP_SHOT_TABS=1 adds a panel tab, moves the first setting to it and opens another's Move to tab;
    // BLOXKEEP_SHOT_COUNTER=1 opens New name… on the open step; BLOXKEEP_SHOT_DRAGSTEP="from,over" (counted from 1)
    // drags a step's grip over another row and holds it there.
    let (feat_n, feat_full, opt_full, tab_first, tab_n, tab_full) = (
        r.tr("macro.feat.n"),
        r.i18n.tv("macro.feat.full", &[("n", &MAX_FEATURES.to_string())]),
        r.i18n.tv("macro.opt.full", &[("n", &MAX_OPTIONS.to_string())]),
        r.tr("macro.tab.first"),
        r.tr("macro.tab.n"),
        r.i18n.tv("macro.tab.full", &[("n", &MAX_TABS.to_string())]),
    );
    if let Some(f) = r.auto.macro_form.as_mut() {
        if let Ok(v) = std::env::var("BLOXKEEP_SHOT_FEATURE") {
            f.add_feature(&feat_n, feat_full, opt_full);
            if let Some(l) = f.steps_mut() {
                l.extend(new_steps("key"));
                l.extend(new_steps("wait"));
            }
            if v == "main" {
                f.switch_to(None);
            }
        }
        if std::env::var("BLOXKEEP_SHOT_TABS").is_ok() {
            let names: Vec<String> = arr(&f.m, "options").iter().map(|o| s(o, "name").to_string()).collect();
            f.add_tab(&tab_first, &tab_n, tab_full);
            if let (Some(first), Some(tab)) = (names.first(), f.cur_tab()) {
                f.move_to_tab(first, &tab);
            }
            f.tab_menu = names.get(1).cloned().or_else(|| names.first().cloned());
        }
        if std::env::var("BLOXKEEP_SHOT_COUNTER").is_ok() {
            f.naming = f.open.map(|i| (i, "name"));
        }
        if let Ok(v) = std::env::var("BLOXKEEP_SHOT_DRAGSTEP") {
            let n: Vec<usize> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            if let [a, b] = n.as_slice() {
                if *a > 0 && *b > 0 {
                    let (from, end) = span(f.steps(), a - 1);
                    f.step_drag = Some((from, end - from + 1));
                    f.drag_over(b - 1);
                }
            }
        }
        fill(f, window, cx);
    }
    if std::env::var("BLOXKEEP_SHOT_TRY").is_ok() {
        try_it(r, window, cx);
    }
    if std::env::var("BLOXKEEP_SHOT_REC").is_ok() {
        record(r, cx);
    }
}

#[cfg(feature = "shot")]
fn shot_drag(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let Ok(v) = std::env::var("BLOXKEEP_SHOT_DRAG") else { return };
    let n: Vec<f32> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
    let [x1, y1, x2, y2] = n.as_slice() else { return };
    let Some(f) = r.auto.macro_form.as_mut() else { return };
    // As a press there would: the frame's bounds aren't needed, the shares are given.
    let kind = match (f.cutting, f.open) {
        (Some(_), _) => DragKind::Cut,
        (None, Some(i)) if f.steps().get(i).is_some_and(|st| point_of(st, &f.m).is_some()) => DragKind::Dot(i),
        (None, Some(i)) => DragKind::Draw(i),
        _ => return,
    };
    let d = Drag { kind, from: (*x1, *y1), to: (*x2, *y2), orig: [*x1, *y1, 0., 0.] };
    finish_drag(r, d, window, cx);
}

fn open_step(r: &mut Root, i: Option<usize>, window: &mut Window, cx: &mut Context<Root>) {
    if let Some(f) = r.auto.macro_form.as_mut() {
        f.open = i;
        f.adding = false;
        f.keys = false;
        f.capture = false;
        if f.cutting.is_some() && f.cutting != i {
            f.cutting = None;
        }
        fill(f, window, cx);
    }
    cx.notify();
}

/// A step that needs an area (MacroEditor.tsx's fixOf "area"): with none yet, a new one is made and the step looks in
/// it; else the step opens where its area is chosen.
fn fix_area(r: &mut Root, i: usize, window: &mut Window, cx: &mut Context<Root>) {
    let full = r.i18n.tv("macro.area.full", &[("n", &MAX_AREAS.to_string())]);
    if let Some(f) = r.auto.macro_form.as_mut() {
        if arr(&f.m, "areas").is_empty() {
            let path = f.steps().get(i).and_then(region_path).unwrap_or("region");
            f.set_region(i, path, "new", &full);
        }
        f.unfold_to(i);
    }
    open_step(r, Some(i), window, cx);
    push_game(r);
}

/// "Name it" (MacroEditor.tsx's fixOf "name"): the macro's name box, or the first area with no name, takes the typing.
fn fix_name(r: &mut Root, area: bool, window: &mut Window, cx: &mut Context<Root>) {
    let Some(f) = r.auto.macro_form.as_ref() else { return };
    let target = if area {
        let empty = arr(&f.m, "areas").iter().find(|a| s(a, "label").trim().is_empty()).map(|a| s(a, "name").to_string());
        empty.and_then(|n| f.area_boxes.iter().find(|(k, _)| *k == n).map(|(_, e)| e.clone()))
    } else {
        Some(f.name.clone())
    };
    if let Some(e) = target {
        e.update(cx, |b, cx| b.focus(window, cx));
    }
    cx.notify();
}

/// Where a problem is: its step, in the main steps or a feature's.
type At = (Option<usize>, Option<String>);

/// What Save sends, or what stops it (and the step it is on).
fn collect(root: &Root, f: &Form, cx: &App) -> Result<Value, (String, At)> {
    let mut m = f.m.clone();
    finish_marks(&mut m);
    let name = text(&f.name, cx).trim().to_string();
    set(&mut m, "name", json!(name));
    let kind = m.get("trigger").map(|t| s(t, "kind")).filter(|k| !k.is_empty()).unwrap_or("manual").to_string();
    if kind == "interval" {
        let secs = parse(&text(&f.every, cx)).map(|n| (n * if f.every_min { 60. } else { 1. }).round() as i64).unwrap_or(0);
        if !(10..=86400).contains(&secs) {
            return Err((root.tr("macro.err.every"), (None, None)));
        }
        set(&mut m, "trigger", json!({ "kind": kind, "everySeconds": secs }));
    } else {
        set(&mut m, "trigger", json!({ "kind": kind }));
    }
    if let Some(n) = parse(&text(&f.max, cx)) {
        set(&mut m, "maxMinutes", json!(n.round().clamp(1., 240.) as i64));
    }
    // (The box shows seconds with 2 decimals: left as shown, the exact ms the macro has stay, 45 not 50.)
    let jit = text(&f.jit_ms, cx);
    let shown = f.m.get("jitterMs").and_then(Value::as_f64).map(|n| trim(n / 1000.));
    if let Some(n) = parse(&jit).filter(|_| shown.as_deref() != Some(jit.as_str())) {
        set(&mut m, "jitterMs", json!((n.clamp(0., 0.5) * 1000.).round() as i64));
    }
    if let Some(n) = parse(&text(&f.jit_px, cx)) {
        set(&mut m, "jitterPx", json!(n.clamp(0., 20.).round() as i64));
    }
    // The settings no step names any more go, but those made on the panel.
    let used = refs(&m);
    if let Some(l) = arr_mut(&mut m, "options") {
        l.retain(|o| FREE.contains(&s(o, "kind")) || used.contains(s(o, "name")));
    }
    // The features in their switches' order (the order they play in); counters' labels no step keeps go.
    let order: Vec<String> = feats_in_order(&m).into_iter().map(|fe| s(fe, "id").to_string()).collect();
    if let Some(l) = arr_mut(&mut m, "features") {
        l.sort_by_key(|fe| order.iter().position(|id| id == s(fe, "id")).unwrap_or(usize::MAX));
    }
    let names = f.counters();
    if let Some(p) = m.get_mut("panel").filter(|p| p.is_object()) {
        let keep: serde_json::Map<String, Value> = p
            .get("counterLabels")
            .and_then(Value::as_object)
            .map(|l| l.iter().filter(|(k, v)| names.contains(k) && v.as_str().is_some_and(|x| !x.trim().is_empty())).map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        if keep.is_empty() {
            drop_key(p, "counterLabels");
        } else {
            set(p, "counterLabels", Value::Object(keep));
        }
    }
    let saved = saved_macros(root);
    if let Some(p) = problems(&root.i18n, &m, &name, saved).into_iter().find(|p| !p.warn) {
        return Err((p.text, (p.step, p.sec)));
    }
    Ok(json!({ "macro": m }))
}

/// The saved macros (the app's list).
fn saved_macros(root: &Root) -> &[Value] {
    root.state.pointer("/settings/macros").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

/// A problem's step brought up: its list shown, the step open.
fn show_at(f: &mut Form, at: At, window: &mut Window, cx: &mut Context<Root>) {
    let (step, sec) = at;
    if step.is_some() || sec.is_some() {
        f.switch_to(sec);
    }
    if let Some(i) = step {
        f.unfold_to(i);
        f.open = step;
    }
    fill(f, window, cx);
}

fn save(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let Some(f) = r.auto.macro_form.as_ref() else { return };
    if f.saving {
        return;
    }
    let args = match collect(r, f, cx) {
        Ok(a) => a,
        Err((e, at)) => {
            if let Some(f) = r.auto.macro_form.as_mut() {
                f.error = Some(e);
                f.tried = true;
                show_at(f, at, window, cx);
            }
            cx.notify();
            return;
        }
    };
    let Some(link) = r.link.clone() else { return };
    let mut id = 0;
    if let Some(f) = r.auto.macro_form.as_mut() {
        f.saving = true;
        f.error = None;
        id = f.id;
    }
    let rx = link.call("saveMacro", args);
    cx.notify();
    answer(rx, cx, move |r, answer, cx| {
        // (Left while saving and another form opened: that one stays as it is.)
        if form_of(r, id).is_none() {
            return;
        }
        match answer {
            Ok(Ok(_)) => close(r, cx),
            Ok(Err(e)) => {
                if let Some(f) = form_of(r, id) {
                    f.saving = false;
                    f.error = Some(e);
                }
            }
            Err(_) => {
                if let Some(f) = form_of(r, id) {
                    f.saving = false;
                }
            }
        }
        cx.notify();
    });
}

// ───────────────────────── the game ─────────────────────────

/// The accounts with a Roblox window: their ids and names.
fn windows(root: &Root) -> Vec<(String, String)> {
    root.accounts()
        .iter()
        .filter(|a| s(a, "state") == "running" && a.pointer("/session/pid").and_then(Value::as_i64).unwrap_or(0) > 0)
        .map(|a| (s(a, "id").to_string(), crate::pages::accounts::label(a)))
        .collect()
}

/// The window the form uses: the one chosen, else the first open.
fn window_of(root: &Root, f: &Form) -> Option<(String, String)> {
    let wins = windows(root);
    wins.iter().find(|(id, _)| *id == f.account).or(wins.first()).cloned()
}

fn sniff(b: &[u8]) -> Option<ImageFormat> {
    Some(match b {
        [0x89, b'P', b'N', b'G', ..] => ImageFormat::Png,
        [0xff, 0xd8, ..] => ImageFormat::Jpeg,
        [b'B', b'M', ..] => ImageFormat::Bmp,
        _ if b.iter().take(256).copied().collect::<Vec<u8>>().windows(4).any(|w| w == b"<svg") => ImageFormat::Svg,
        _ => return None,
    })
}

/// A picture from the app: a data: address (a PNG in base64; the dev mock's SVG), or a bare base64 PNG.
fn decode_image(src: &str) -> Option<Arc<Image>> {
    let b64 = base64::engine::general_purpose::STANDARD;
    let bytes = match src.strip_prefix("data:") {
        Some(rest) => {
            let (meta, body) = rest.split_once(',')?;
            if meta.ends_with(";base64") { b64.decode(body).ok()? } else { percent_encoding::percent_decode_str(body).collect() }
        }
        None => b64.decode(src.trim()).ok()?,
    };
    let format = sniff(&bytes)?;
    Some(Arc::new(Image::from_bytes(format, bytes)))
}

/// A step's picture for its thumbnail, decoded once while the form is open (a few kept).
fn thumb(f: &Form, b64: &str, cx: &mut App) -> Option<Arc<Image>> {
    if b64.is_empty() {
        return None;
    }
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    b64.hash(&mut h);
    let key = h.finish();
    let mut kept = f.thumbs.borrow_mut();
    if let Some((_, i)) = kept.iter().find(|(k, _)| *k == key) {
        return Some(i.clone());
    }
    let image = decode_image(b64)?;
    if kept.len() >= THUMBS {
        let (_, old) = kept.remove(0);
        old.remove_asset(cx);
    }
    kept.push((key, image.clone()));
    Some(image)
}

/// A still of the window (macroPicture): the dots and boxes are placed on it, pictures cut from it.
fn take_picture(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let Some(f) = r.auto.macro_form.as_ref() else { return };
    let Some((acc, _)) = window_of(r, f) else {
        let e = r.tr("macro.need.game");
        if let Some(f) = r.auto.macro_form.as_mut() {
            f.error = Some(e);
        }
        return cx.notify();
    };
    let Some(link) = r.link.clone() else { return };
    let mut id = 0;
    if let Some(f) = r.auto.macro_form.as_mut() {
        f.taking = true;
        id = f.id;
    }
    let rx = link.call("macroPicture", json!({ "accountId": acc }));
    cx.notify();
    cx.spawn_in(window, async move |this, cx| {
        let answer = rx.await;
        let _ = this.update_in(cx, |r, window, cx| {
            let Some(f) = form_of(r, id) else { return };
            f.taking = false;
            match answer {
                Ok(Ok(v)) => {
                    let (w, h) = (num(&v, "w") as f32, num(&v, "h") as f32);
                    if let Some(image) = decode_image(s(&v, "image")).filter(|_| w > 0. && h > 0.) {
                        if let Some(old) = f.picture.replace(Pic { image, w, h }) {
                            old.image.remove_asset(cx);
                        }
                        if let Some(old) = f.seen.take() {
                            old.pic.image.remove_asset(cx);
                        }
                        if !has(&f.m, "recordedSize") {
                            set(&mut f.m, "recordedSize", json!({ "w": w, "h": h }));
                        }
                        #[cfg(feature = "shot")]
                        shot_drag(r, window, cx);
                        let _ = window;
                    }
                }
                Ok(Err(e)) => f.error = Some(e),
                Err(_) => {}
            }
            cx.notify();
        });
    })
    .detach();
}

/// A point or a colour picked by clicking on the game itself (the click doesn't reach Roblox; Esc gives up).
fn pick_on_game(r: &mut Root, i: usize, colour: Option<&'static str>, window: &mut Window, cx: &mut Context<Root>) {
    let Some(f) = r.auto.macro_form.as_ref() else { return };
    let Some((acc, label)) = window_of(r, f) else {
        let e = r.tr("macro.need.game");
        if let Some(f) = r.auto.macro_form.as_mut() {
            f.error = Some(e);
        }
        return cx.notify();
    };
    let Some(link) = r.link.clone() else { return };
    let say = r.i18n.tv("macro.pickNow", &[("name", &label)]);
    let mut id = 0;
    if let Some(f) = r.auto.macro_form.as_mut() {
        f.picking = true;
        f.pick_at = Some(i);
        f.say = Some(say);
        id = f.id;
    }
    let rx = link.call("pickOnGame", json!({ "accountId": acc }));
    cx.notify();
    cx.spawn_in(window, async move |this, cx| {
        let answer = rx.await;
        let _ = this.update_in(cx, |r, window, cx| {
            if let Some(f) = form_of(r, id) {
                f.picking = false;
                f.say = None;
                // The step it was for, where it is now (it may have moved, or gone, while the click was awaited).
                let at = f.pick_at.take();
                match answer {
                    Ok(Ok(v)) if v.is_object() && at.is_some() => {
                        let i = at.unwrap_or_default();
                        let (x, y) = (num(&v, "x") as f32, num(&v, "y") as f32);
                        match colour {
                            None => f.set_xy(i, x, y),
                            Some(path) => {
                                // A colour at a point: the point goes with it (Wait for a colour, If a colour shows, without an area).
                                let at = f.steps().get(i).is_some_and(|st| match s(st, "kind") {
                                    "waitColour" => !has(st, "region"),
                                    k if is_test(k) => path == "test.colour" && !st.get("test").is_some_and(|t| has(t, "region")),
                                    _ => false,
                                });
                                if at {
                                    f.set_xy(i, x, y);
                                }
                                if let Some(h) = hexv(s(&v, "colour")) {
                                    f.put_val(i, path, json!(format!("#{h:06x}")));
                                }
                            }
                        }
                        fill(f, window, cx);
                    }
                    Ok(Err(e)) => f.error = Some(e),
                    _ => {}
                }
            }
            push_game(r);
            cx.notify();
        });
    })
    .detach();
}

/// A picture step's picture is cut next, from the still (taken first when there is none).
fn start_cut(r: &mut Root, i: usize, window: &mut Window, cx: &mut Context<Root>) {
    let Some(f) = r.auto.macro_form.as_ref() else { return };
    let none = f.picture.is_none() && f.seen.is_none();
    let win = window_of(r, f).is_some();
    if none && !win {
        let e = r.tr("macro.need.game");
        if let Some(f) = r.auto.macro_form.as_mut() {
            f.error = Some(e);
        }
        return cx.notify();
    }
    if let Some(f) = r.auto.macro_form.as_mut() {
        f.cutting = if f.cutting == Some(i) { None } else { Some(i) };
    }
    if none && !r.auto.macro_form.as_ref().is_some_and(|f| f.taking) {
        take_picture(r, window, cx);
    }
    cx.notify();
}

/// The part of a still inside a box, as a PNG in base64 (cut down to 256 × 256 px), and whether it was cut down.
fn crop(pic: &Arc<Image>, r: [f32; 4], cx: &App) -> Option<(String, bool)> {
    let data = pic.to_image_data(cx.svg_renderer()).ok()?;
    let size = data.size(0);
    let (iw, ih) = (u32::try_from(size.width.0).ok()?, u32::try_from(size.height.0).ok()?);
    if iw == 0 || ih == 0 {
        return None;
    }
    let bytes = data.as_bytes(0)?;
    let sx = ((r[0].max(0.) * iw as f32).round() as u32).min(iw - 1);
    let sy = ((r[1].max(0.) * ih as f32).round() as u32).min(ih - 1);
    let w0 = ((r[2].max(0.) * iw as f32).round() as u32).clamp(1, iw - sx);
    let h0 = ((r[3].max(0.) * ih as f32).round() as u32).clamp(1, ih - sy);
    let (w, h) = (w0.min(MAX_CUT), h0.min(MAX_CUT));
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for y in sy..sy + h {
        let at = ((y * iw + sx) * 4) as usize;
        let row = bytes.get(at..at + (w * 4) as usize)?;
        // GPUI keeps its pixels as BGRA.
        for p in row.chunks_exact(4) {
            if let [b, g, r, a] = *p {
                out.extend_from_slice(&[r, g, b, a]);
            }
        }
    }
    let img = image::RgbaImage::from_raw(w, h, out)?;
    let mut png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).ok()?;
    Some((base64::engine::general_purpose::STANDARD.encode(png), w < w0 || h < h0))
}

/// The still's point under the mouse, as shares of the window, and the frame's size in pixels.
fn frac(f: &Form, pos: Point<Pixels>) -> Option<(f32, f32, f32, f32)> {
    let b = f.frame.get()?;
    let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
    if w <= 0. || h <= 0. {
        return None;
    }
    Some((cl((f32::from(pos.x) - f32::from(b.origin.x)) / w, 0., 1.), cl((f32::from(pos.y) - f32::from(b.origin.y)) / h, 0., 1.), w, h))
}

/// Where the dragged dot or box is now ([x, y, w, h]).
fn moved(d: &Drag) -> [f32; 4] {
    let (dx, dy) = (d.to.0 - d.from.0, d.to.1 - d.from.1);
    let o = d.orig;
    match d.kind {
        DragKind::Dot(_) => [cl(o[0] + dx, 0., 1.), cl(o[1] + dy, 0., 1.), 0., 0.],
        DragKind::Move(_) => [cl(o[0] + dx, 0., 1. - o[2]), cl(o[1] + dy, 0., 1. - o[3]), o[2], o[3]],
        DragKind::Size(_) => [o[0], o[1], cl(o[2] + dx, MIN_BOX, 1. - o[0]), cl(o[3] + dy, MIN_BOX, 1. - o[1])],
        DragKind::Cut | DragKind::Draw(_) => [d.from.0.min(d.to.0), d.from.1.min(d.to.1), dx.abs(), dy.abs()],
    }
}

/// A press on the still: a dot or a box's corner under it is dragged; else the open step's point goes there, an area
/// is moved, or the open step's area is drawn; while a picture is being cut, the drag goes around it.
fn pic_down(r: &mut Root, pos: Point<Pixels>, cx: &mut Context<Root>) {
    let Some(f) = r.auto.macro_form.as_mut() else { return };
    if f.picture.is_none() && f.seen.is_none() {
        return;
    }
    let Some((x, y, w, h)) = frac(f, pos) else { return };
    let at = (x, y);
    let start = |kind: DragKind, orig: [f32; 4]| Drag { kind, from: at, to: at, orig };
    let drag = if f.cutting.is_some() {
        start(DragKind::Cut, [x, y, 0., 0.])
    } else {
        let steps = f.steps();
        let dot = steps
            .iter()
            .enumerate()
            .filter_map(|(i, st)| point_of(st, &f.m).map(|(px_, py_)| (i, ((px_ - x) * w).hypot((py_ - y) * h), px_, py_)))
            .filter(|d| d.1 <= 12.)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let areas = arr(&f.m, "areas");
        let corner = areas.iter().rev().find(|a| {
            let r = area_rect(a);
            ((r[0] + r[2] - x) * w).abs() <= 10. && ((r[1] + r[3] - y) * h).abs() <= 10.
        });
        let inside = areas.iter().rev().find(|a| {
            let r = area_rect(a);
            x >= r[0] && x <= r[0] + r[2] && y >= r[1] && y <= r[1] + r[3]
        });
        let open = f.open.and_then(|i| steps.get(i).map(|st| (i, st)));
        if let Some((i, _, px_, py_)) = dot {
            start(DragKind::Dot(i), [px_, py_, 0., 0.])
        } else if let Some(a) = corner {
            start(DragKind::Size(s(a, "name").to_string()), area_rect(a))
        } else if let Some((i, _)) = open.filter(|(_, st)| point_of(st, &f.m).is_some()) {
            start(DragKind::Dot(i), [x, y, 0., 0.])
        } else if let Some(a) = inside {
            start(DragKind::Move(s(a, "name").to_string()), area_rect(a))
        } else if let Some((i, _)) = open.filter(|(_, st)| region_path(st).is_some()) {
            start(DragKind::Draw(i), [x, y, 0., 0.])
        } else {
            return;
        }
    };
    f.drag = Some(drag);
    cx.notify();
}

fn pic_move(r: &mut Root, pos: Point<Pixels>, cx: &mut Context<Root>) {
    let Some(f) = r.auto.macro_form.as_mut() else { return };
    if f.drag.is_none() {
        return;
    }
    let Some((x, y, _, _)) = frac(f, pos) else { return };
    if let Some(d) = f.drag.as_mut() {
        d.to = (x, y);
    }
    cx.notify();
}

fn pic_up(r: &mut Root, pos: Point<Pixels>, window: &mut Window, cx: &mut Context<Root>) {
    let Some(f) = r.auto.macro_form.as_mut() else { return };
    let to = frac(f, pos);
    let Some(mut d) = f.drag.take() else { return };
    if let Some((x, y, _, _)) = to {
        d.to = (x, y);
    }
    finish_drag(r, d, window, cx);
}

/// A drag on the still done: the cut picture, the dot, the box or the drawn area kept.
fn finish_drag(r: &mut Root, d: Drag, window: &mut Window, cx: &mut Context<Root>) {
    let r4 = moved(&d);
    let cut_max = r.tr("macro.pic.cutMax");
    let full = r.i18n.tv("macro.area.full", &[("n", &MAX_AREAS.to_string())]);
    let Some(f) = r.auto.macro_form.as_mut() else { return };
    match d.kind {
        DragKind::Cut => {
            let Some(i) = f.cutting else { return };
            if r4[2] < 0.005 || r4[3] < 0.005 {
                return cx.notify();
            }
            let Some(pic) = f.seen.as_ref().map(|x| x.pic.image.clone()).or_else(|| f.picture.as_ref().map(|p| p.image.clone())) else { return };
            let got = crop(&pic, r4, cx);
            let Some(f) = r.auto.macro_form.as_mut() else { return };
            if let Some((b64, down)) = got {
                let path = f.steps().get(i).and_then(image_path).unwrap_or("image");
                if let Some(st) = f.step_mut(i) {
                    put(st, path, json!(b64));
                }
                f.cutting = None;
                f.say = down.then_some(cut_max);
            }
        }
        DragKind::Dot(i) => f.set_xy(i, r4[0], r4[1]),
        DragKind::Move(ref n) | DragKind::Size(ref n) => f.set_area_rect(n, r4),
        DragKind::Draw(i) => {
            if r4[2] >= MIN_BOX && r4[3] >= MIN_BOX {
                let Some(path) = f.steps().get(i).and_then(region_path) else { return };
                let mine = f.steps().get(i).and_then(|st| area_ref(get(st, path))).map(str::to_string).filter(|n| arr(&f.m, "areas").iter().any(|a| s(a, "name") == n));
                match mine {
                    Some(n) => f.set_area_rect(&n, r4),
                    None => match f.new_area(r4) {
                        Some(at) => {
                            if let Some(st) = f.step_mut(i) {
                                put(st, path, json!(at));
                            }
                        }
                        None => f.error = Some(full),
                    },
                }
            }
        }
    }
    if let Some(f) = r.auto.macro_form.as_mut() {
        fill(f, window, cx);
    }
    push_game(r);
    cx.notify();
}

/// The macro with the steps shown as its own (Edit on game draws these; a feature's Try plays them alone).
fn shown_macro(f: &Form) -> Value {
    let mut m = f.m.clone();
    if f.sec.is_some() {
        let steps = f.steps().to_vec();
        set(&mut m, "steps", json!(steps));
        set(&mut m, "features", json!([]));
    }
    strip_marks(&mut m);
    m
}

/// Edit on game turned on (a template's "Place the area"): with no window it says so.
fn edit_on_game(r: &mut Root, cx: &mut Context<Root>) {
    let has_win = r.auto.macro_form.as_ref().and_then(|f| window_of(r, f)).is_some();
    let e = r.tr("macro.need.game");
    let Some(f) = r.auto.macro_form.as_mut() else { return };
    if !has_win {
        f.error = Some(e);
    } else if !f.on_game {
        f.on_game = true;
        push_game(r);
    }
    cx.notify();
}

/// Edit on game: the window's dots and boxes drawn again from the form.
fn push_game(r: &Root) {
    let Some(f) = r.auto.macro_form.as_ref() else { return };
    if !f.on_game {
        return;
    }
    let Some((acc, _)) = window_of(r, f) else { return };
    r.send("editOnGame", json!({ "id": s(&f.m, "id"), "accountId": acc, "macro": shown_macro(f) }));
}

/// What the app says about the form's game: a recording's steps (macroRecorded), what Try it sees (macroSeen), a dot or
/// box dragged on the window (onGameEdit).
pub fn on_event(r: &mut Root, name: &str, data: &Value, window: &mut Window, cx: &mut Context<Root>) {
    let nothing = r.tr("macro.nothingRecorded");
    let acc = r.auto.macro_form.as_ref().and_then(|f| window_of(r, f)).map(|(a, _)| a);
    let Some(f) = r.auto.macro_form.as_mut() else { return };
    match name {
        "macroRecorded" => {
            if !f.recording {
                return;
            }
            f.recording = false;
            let steps = arr(data, "steps");
            if steps.is_empty() {
                r.toast = Some((nothing, "info".into()));
            } else if f.steps().is_empty() {
                if let Some(l) = f.steps_mut() {
                    l.extend(steps.iter().cloned());
                }
                set(&mut f.m, "recordedSize", json!({ "w": num(data, "w"), "h": num(data, "h") }));
                f.adding = false;
            } else {
                f.pending = Some(data.clone());
            }
        }
        "macroSeen" => {
            if f.trial.is_none() || acc.as_deref().is_some_and(|a| !s(data, "accountId").is_empty() && s(data, "accountId") != a) {
                return;
            }
            let (w, h) = (num(data, "w") as f32, num(data, "h") as f32);
            let Some(image) = decode_image(s(data, "image")).filter(|_| w > 0. && h > 0.) else { return };
            let boxes = arr(data, "boxes")
                .iter()
                .map(|b| {
                    let g = |k: &str| b.get(k).and_then(Value::as_f64).unwrap_or(0.) as f32;
                    [g("x"), g("y"), g("w"), g("h")]
                })
                .collect();
            if let Some(old) = f.seen.replace(Seen { pic: Pic { image, w, h }, boxes }) {
                old.pic.image.remove_asset(cx);
            }
        }
        "onGameEdit" => {
            if data.get("done").and_then(Value::as_bool) == Some(true) {
                f.on_game = false;
            } else if let (Some(i), Some(x), Some(y)) = (data.get("step").and_then(Value::as_u64), data.get("x").and_then(Value::as_f64), data.get("y").and_then(Value::as_f64)) {
                f.set_xy(i as usize, x as f32, y as f32);
            } else if let Some(rect) = data.get("rect").filter(|x| x.is_object()) {
                let g = |k: &str| rect.get(k).and_then(Value::as_f64).unwrap_or(0.) as f32;
                let r4 = [g("x"), g("y"), g("w"), g("h")];
                if let Some(area) = data.get("area").and_then(Value::as_str) {
                    f.set_area_rect(area, r4);
                } else if let Some(i) = data.get("region").and_then(Value::as_u64) {
                    let test = data.get("test").and_then(Value::as_bool) == Some(true);
                    let region = json!({ "x": round4(r4[0]), "y": round4(r4[1]), "w": round4(r4[2]), "h": round4(r4[3]) });
                    if let Some(st) = f.step_mut(i as usize) {
                        put(st, if test { "test.region" } else { "region" }, region);
                    }
                }
            }
            fill(f, window, cx);
        }
        _ => return,
    }
    cx.notify();
}

/// The app's templates (macroTemplates), for a new macro to start from.
fn want_templates(r: &mut Root, cx: &mut Context<Root>) {
    let Some(link) = r.link.clone() else { return };
    let id = r.auto.macro_form.as_ref().map_or(0, |f| f.id);
    let rx = link.call("macroTemplates", json!({}));
    answer(rx, cx, move |r, answer, cx| {
        if let (Some(f), Ok(Ok(v))) = (form_of(r, id), answer) {
            f.templates = Some(arr(&v, "macros").to_vec());
            cx.notify();
        }
    });
}

/// A template taken: the macro starts as it is (its steps, settings and areas), as a new one.
fn use_template(r: &mut Root, k: usize, window: &mut Window, cx: &mut Context<Root>) {
    let Some(f) = r.auto.macro_form.as_mut() else { return };
    let Some(mut t) = f.templates.as_ref().and_then(|l| l.get(k)).filter(|t| t.is_object()).cloned() else { return };
    for key in ["about", "icon"] {
        drop_key(&mut t, key);
    }
    let mut m = f.m.clone();
    if let (Some(base), Some(over)) = (m.as_object_mut(), t.as_object()) {
        for (key, v) in over {
            base.insert(key.clone(), v.clone());
        }
    }
    set(&mut m, "id", json!(""));
    // Its to-dos (the fields it marked on its steps) are said over the form until each is set.
    mark_todos(&mut m);
    f.switch_to(None);
    f.m = m;
    f.remap(|_| None);
    f.tab_sel = None;
    f.open = None;
    f.adding = false;
    f.error = None;
    load_top(f, window, cx);
    fill(f, window, cx);
    push_game(r);
    cx.notify();
}

/// The live run of Try it, or how it ended.
fn trial_run<'a>(root: &'a Root, f: &Form) -> Option<&'a Value> {
    let t = f.trial.as_ref()?;
    arr(&root.state, "macroRuns")
        .iter()
        .filter(|x| x.get("trial").and_then(Value::as_bool) == Some(true) && s(x, "macroId") == t.id && s(x, "accountId") == t.account)
        .filter(|x| crate::time::iso_secs(s(x, "startedAt")).is_none_or(|at| at >= t.since - 5))
        .last()
}

pub(crate) fn live(run: &Value) -> bool {
    matches!(s(run, "status"), "waiting" | "running" | "paused")
}

/// The steps that look for something: passed, Try it says it "found" it (MacroEditor.tsx's LOOKS).
const LOOKS: [&str; 9] = ["waitImage", "clickImage", "waitColour", "clickColour", "waitText", "if", "loopUntil", "follow", "read"];
/// The steps Try it says nothing of (MacroEditor.tsx's BLOCKS).
const BLOCKS: [&str; 5] = ["loop", "endLoop", "else", "endIf", "break"];

/// Try it's lines followed as its run moves (MacroEditor.tsx's effect on run.step and run.status): each step it got past
/// is a line ("found" for those that look for something), and the step it stopped on says why (a setting empty, or
/// nothing seen). While a feature plays in the middle of the main steps, the lines wait where those were.
fn track_trial(root: &Root, f: &Form, run: Option<&Value>, names: &[(String, String)]) {
    let (Some(t), Some(run)) = (f.trial.as_ref(), run) else { return };
    if f.trial_over.get() {
        return;
    }
    let tried: &[Value] = match t.sec.as_deref().and_then(|id| feat_of(&f.m, id)) {
        Some(fe) => arr(fe, "steps"),
        None => arr(&f.m, "steps"),
    };
    let mut lines = f.lines.borrow_mut();
    let mut line = |k: usize, outcome: &str| {
        let Some(st) = tried.get(k) else { return };
        let n = (k + 1).to_string();
        let what = step_text(root, &f.m, names, st, k + 1);
        let (ok, text) = match outcome {
            "found" => (true, root.i18n.tv("macro.try.found", &[("n", &n), ("what", &what)])),
            "done" => (true, root.i18n.tv("macro.try.done", &[("n", &n), ("what", &what)])),
            "missingValue" => (false, root.i18n.tv("macro.try.empty", &[("n", &n)])),
            _ => (false, root.i18n.tv("macro.try.notSeen", &[("n", &n), ("what", &what)])),
        };
        lines.retain(|x| x.step != k);
        lines.push(TryLine { step: k, ok, text });
    };
    let passed = |k: usize, line: &mut dyn FnMut(usize, &str)| {
        if let Some(st) = tried.get(k) {
            let kind = s(st, "kind");
            if !BLOCKS.contains(&kind) {
                line(k, if LOOKS.contains(&kind) { "found" } else { "done" });
            }
        }
    };
    let in_feature = !s(run, "section").is_empty();
    let last = f.last_step.get();
    let at = if in_feature { last } else { run.get("step").and_then(Value::as_u64).map(|x| x as usize) };
    let over = !live(run);
    match last {
        Some(l) if at != Some(l) && over => {
            let why = s(run, "reason");
            if s(run, "status") == "failed" && matches!(why, "missingValue" | "notFound" | "timeout") {
                line(l, if why == "missingValue" { "missingValue" } else { "notFound" });
            } else if s(run, "status") == "done" {
                passed(l, &mut line);
            }
        }
        Some(l) if at.is_some_and(|a| a != l) => passed(l, &mut line),
        _ => {}
    }
    if over {
        f.trial_over.set(true);
    }
    f.last_step.set(at);
}

/// Each state the app sends (Root::set_state, after the state is kept): Try it's lines follow every step the run is
/// seen on, not only the one the next frame shows (several states can come between two frames).
pub fn on_state(root: &Root) {
    let Some(f) = root.auto.macro_form.as_ref() else { return };
    if f.trial.is_none() || f.trial_over.get() {
        return;
    }
    let my_id = s(&f.m, "id");
    let saved: &[Value] = root.state.pointer("/settings/macros").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    let others: Vec<(String, String)> = saved.iter().filter(|o| s(o, "id") != my_id).map(|o| (s(o, "id").to_string(), s(o, "name").to_string())).collect();
    track_trial(root, f, trial_run(root, f), &others);
}

/// Try it: the macro as it is in the form, once, on the window chosen; again, it stops.
fn try_it(r: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    let Some(f) = r.auto.macro_form.as_ref() else { return };
    if let (Some(t), Some(run)) = (f.trial.as_ref(), trial_run(r, f)) {
        if live(run) {
            r.send("stopMacros", json!({ "id": t.id, "accountId": t.account, "trial": true }));
            return;
        }
    }
    let Some((acc, _)) = window_of(r, f) else {
        let e = r.tr("macro.need.try");
        if let Some(f) = r.auto.macro_form.as_mut() {
            f.error = Some(e);
        }
        return cx.notify();
    };
    let name = text(&f.name, cx).trim().to_string();
    // A feature's Try plays its steps alone: the main steps' problems don't stop it.
    let first = problems(&r.i18n, &f.m, &name, saved_macros(r)).into_iter().find(|p| !p.warn && (f.sec.is_none() || (p.sec == f.sec && p.step.is_some())));
    if let Some(p) = first {
        let e = r.tr("macro.tryFix");
        if let Some(f) = r.auto.macro_form.as_mut() {
            f.error = Some(if p.step.is_some() { e } else { p.text });
            f.tried = true;
            show_at(f, (p.step, p.sec), window, cx);
        }
        return cx.notify();
    }
    let mut draft = shown_macro(f);
    set(&mut draft, "name", json!(name));
    let Some(link) = r.link.clone() else { return };
    let rx = link.call("macroTryRun", json!({ "macro": draft, "accountId": acc }));
    let mut id = 0;
    if let Some(f) = r.auto.macro_form.as_mut() {
        f.error = None;
        id = f.id;
    }
    answer(rx, cx, move |r, answer, cx| {
        let Some(f) = form_of(r, id) else { return };
        match answer {
            Ok(Ok(v)) => {
                let id = s(&v, "macroId").to_string();
                if !id.is_empty() {
                    f.trial = Some(Trial { id, account: acc, since: crate::time::now_secs(), sec: f.sec.clone() });
                    // A new try's lines start over (MacroEditor.tsx's tryIt: setLines([]), lastStep = null).
                    f.lines.borrow_mut().clear();
                    f.last_step.set(None);
                    f.trial_over.set(false);
                }
            }
            Ok(Err(e)) => f.error = Some(e),
            Err(_) => {}
        }
        cx.notify();
    });
}

/// Record: what is done on the window comes in as steps (macroRecorded) once it is stopped.
fn record(r: &mut Root, cx: &mut Context<Root>) {
    let Some(f) = r.auto.macro_form.as_ref() else { return };
    let Some((acc, _)) = window_of(r, f) else {
        let e = r.tr("macro.need.record");
        if let Some(f) = r.auto.macro_form.as_mut() {
            f.error = Some(e);
        }
        return cx.notify();
    };
    let Some(link) = r.link.clone() else { return };
    let mut id = 0;
    if let Some(f) = r.auto.macro_form.as_mut() {
        f.recording = true;
        f.pending = None;
        id = f.id;
    }
    let rx = link.call("recordMacro", json!({ "accountId": acc }));
    cx.notify();
    answer(rx, cx, move |r, answer, cx| {
        if let (Some(f), Ok(Err(e))) = (form_of(r, id), answer) {
            f.recording = false;
            f.error = Some(e);
        }
        cx.notify();
    });
}

// ───────────────────────── what stops a save ─────────────────────────

struct Prob {
    step: Option<usize>,
    text: String,
    /// The words without "Step n:", said on the step's own row.
    bare: String,
    warn: bool,
    /// The feature whose steps it is about (None: the main steps, or the macro itself).
    sec: Option<String>,
    /// The fix its row offers (macros.ts's Problem.fix): "area" (a new area, or the step opened at its areas),
    /// "name" (the name box), "areaName" (an area's name box).
    fix: Option<&'static str>,
}

/// The problem just said offers a fix.
fn fix_last(o: &mut [Prob], fix: &'static str) {
    if let Some(p) = o.last_mut() {
        p.fix = Some(fix);
    }
}

fn say(out: &mut Vec<Prob>, i18n: &I18n, step: Option<usize>, key: &str, vars: &[(&str, &str)], warn: bool) {
    let n = step.map(|x| (x + 1).to_string()).unwrap_or_default();
    let mut all: Vec<(&str, &str)> = vec![("n", n.as_str())];
    all.extend_from_slice(vars);
    let text = i18n.tv(key, &all);
    let lead = format!("{}:", i18n.tv("macro.stepN", &[("n", &n)]));
    let bare = match text.strip_prefix(&lead) {
        Some(rest) if step.is_some() => {
            let rest = rest.trim();
            let mut ch = rest.chars();
            ch.next().map(|c| c.to_uppercase().chain(ch).collect()).unwrap_or_default()
        }
        _ => text.clone(),
    };
    out.push(Prob { step, text, bare, warn, sec: None, fix: None });
}

fn ok_num(v: Option<&Value>, min: f64, max: f64) -> bool {
    opt_ref(v).is_some() || v.and_then(Value::as_f64).is_some_and(|n| n >= min && n <= max)
}

fn ok_num_or(v: Option<&Value>, d: f64, min: f64, max: f64) -> bool {
    match v {
        None | Some(Value::Null) => d >= min && d <= max,
        v => ok_num(v, min, max),
    }
}

fn ok_colour(v: Option<&Value>) -> bool {
    opt_ref(v).is_some() || v.and_then(Value::as_str).is_some_and(|t| t.starts_with('#') && hexv(t).is_some())
}

/// What stops the macro from saving (and a warning or two that don't), each tied to its step (macros.ts's problems):
/// the app checks the same.
/// `name` is the name as typed (the box, not yet in the macro).
fn problems(i18n: &I18n, m: &Value, name: &str, saved: &[Value]) -> Vec<Prob> {
    let mut out: Vec<Prob> = Vec::new();
    let o = &mut out;
    let name = name.trim();
    if name.is_empty() {
        say(o, i18n, None, "macro.err.name", &[], false);
        fix_last(o, "name");
    } else if saved.iter().any(|x| s(x, "id") != s(m, "id") && s(x, "name").trim().to_lowercase() == name.to_lowercase()) {
        say(o, i18n, None, "macro.err.sameName", &[], true);
        fix_last(o, "name");
    }
    if s(m, "targets") == "accounts" && arr(m, "accountIds").is_empty() {
        say(o, i18n, None, "macro.err.noAccounts", &[], false);
    }
    let steps = arr(m, "steps");
    if steps.is_empty() {
        say(o, i18n, None, "macro.err.noSteps", &[], false);
    }
    if steps.len() > MAX_STEPS {
        say(o, i18n, None, "macro.err.tooMany", &[("max", &MAX_STEPS.to_string())], false);
    }
    step_problems(i18n, m, steps, saved, false, o);
    if arr(m, "options").iter().any(|x| s(x, "label").trim().is_empty()) {
        say(o, i18n, None, "macro.err.optLabel", &[], false);
    }
    if arr(m, "areas").iter().any(|x| s(x, "label").trim().is_empty()) {
        say(o, i18n, None, "macro.err.areaName", &[], false);
        fix_last(o, "areaName");
    }
    // Each feature's steps, checked as the main ones are (macros.ts's featureProblems); no If stuck in them; how
    // often it plays; on the main steps, a key held where the features would come in.
    let feats = arr(m, "features");
    for fe in feats {
        let id = s(fe, "id").to_string();
        let lead = arr(m, "options").iter().find(|x| s(x, "name") == s(fe, "option")).map(|x| s(x, "label")).filter(|l| !l.trim().is_empty()).unwrap_or("…");
        let fsteps = arr(fe, "steps");
        let mut mine: Vec<Prob> = Vec::new();
        step_problems(i18n, m, fsteps, saved, true, &mut mine);
        for (i, st) in fsteps.iter().enumerate() {
            if s(st, "kind") == "recover" {
                say(&mut mine, i18n, Some(i), "macro.feat.errRecover", &[], false);
            }
        }
        if fsteps.is_empty() {
            say(&mut mine, i18n, None, "macro.feat.warnEmpty", &[("name", lead)], true);
        }
        let every = fe.get("everyMs").filter(|v| !v.is_null());
        if every.is_some_and(|v| !v.as_f64().is_some_and(|x| (1000. ..=86_400_000.).contains(&x))) {
            say(&mut mine, i18n, None, "macro.feat.errEvery", &[("name", lead)], false);
        }
        for mut p in mine {
            if p.step.is_some() {
                p.text = format!("{lead}: {}", p.text);
            }
            p.sec = Some(id.clone());
            o.push(p);
        }
    }
    if !feats.is_empty() {
        let ends = safe_ends(steps);
        let mut held: Vec<&str> = Vec::new();
        for (i, st) in steps.iter().enumerate() {
            match s(st, "kind") {
                "down" => held.push(s(st, "key")),
                "up" => held.retain(|k| *k != s(st, "key")),
                _ => {}
            }
            if let Some(k) = held.first().filter(|_| ends.contains(&i)) {
                say(o, i18n, Some(i), "macro.feat.warnHeld", &[("k", &key_cap(k))], true);
            }
        }
    }
    out
}

/// What stops one list of steps (the main ones, or a feature's: `feature`) from saving.
fn step_problems(i18n: &I18n, m: &Value, steps: &[Value], saved: &[Value], feature: bool, o: &mut Vec<Prob>) {
    let keys = all_keys();
    let opt_names: HashSet<&str> = arr(m, "options").iter().map(|x| s(x, "name")).collect();
    let area_names: HashSet<&str> = arr(m, "areas").iter().map(|x| s(x, "name")).collect();
    let mut open: Vec<(bool, usize, bool)> = Vec::new();
    let mut held: Vec<(String, usize)> = Vec::new();
    let first_recover = steps.iter().position(|x| s(x, "kind") == "recover");
    for (i, st) in steps.iter().enumerate() {
        let k = s(st, "kind");
        let p = Some(i);
        let key = s(st, "key");
        match k {
            "if" => open.push((true, i, false)),
            "loop" | "loopUntil" => open.push((false, i, false)),
            "else" => match open.last_mut() {
                Some(top) if top.0 => {
                    if top.2 {
                        say(o, i18n, p, "macro.err.elseTwice", &[], false);
                    } else {
                        top.2 = true;
                    }
                }
                _ => say(o, i18n, p, "macro.err.elseAlone", &[], false),
            },
            "endIf" => {
                if open.last().is_some_and(|t| t.0) {
                    open.pop();
                } else {
                    say(o, i18n, p, "macro.err.endIfAlone", &[], false);
                }
            }
            "endLoop" => {
                if open.last().is_some_and(|t| !t.0) {
                    open.pop();
                } else {
                    say(o, i18n, p, "macro.err.endLoopAlone", &[], false);
                }
            }
            "break" => {
                if !open.iter().any(|t| !t.0) {
                    say(o, i18n, p, "macro.err.breakAlone", &[], false);
                }
            }
            "recover" => {
                if !open.is_empty() {
                    say(o, i18n, p, "macro.err.recoverInside", &[], false);
                } else if first_recover != Some(i) {
                    say(o, i18n, p, "macro.err.recoverTwice", &[], false);
                } else if let Some((h, _)) = held.first() {
                    say(o, i18n, p, "macro.err.heldAtRecover", &[("k", &key_cap(h))], false);
                } else if !feature && !matches!(s(m, "onFail"), "restart" | "skip") && num(m, "stuckMinutes") <= 0. {
                    say(o, i18n, p, "macro.warn.recoverOff", &[], true);
                }
            }
            "down" => {
                if held.iter().any(|(h, _)| h == key) {
                    say(o, i18n, p, "macro.err.downTwice", &[("k", &key_cap(key))], false);
                } else {
                    held.push((key.to_string(), i));
                }
            }
            "up" => match held.iter().position(|(h, _)| h == key) {
                Some(at) => {
                    held.remove(at);
                }
                None => say(o, i18n, p, "macro.err.upAlone", &[("k", &key_cap(key))], false),
            },
            _ => {}
        }
        if matches!(k, "key" | "down" | "up") && !(k == "key" && opt_ref(st.get("key")).is_some()) && !keys.iter().any(|x| x == key) && !(k != "key" && MOUSE_KEYS.contains(&key)) {
            say(o, i18n, p, "macro.err.key", &[], false);
        }
        if k == "key" && key == "Alt" {
            say(o, i18n, p, "macro.err.altTap", &[], false);
        }
        if k == "key" && !ok_num(st.get("ms"), 30., 10000.) {
            say(o, i18n, p, "macro.err.hold", &[], false);
        }
        if matches!(k, "key" | "down") && key == "Escape" {
            say(o, i18n, p, "macro.warn.escape", &[], true);
        }
        if matches!(k, "click" | "move" | "waitColour") && !(k == "waitColour" && has(st, "region")) && !(ok_num(st.get("x"), 0., 1.) && ok_num(st.get("y"), 0., 1.)) {
            say(o, i18n, p, "macro.err.point", &[], false);
        }
        if matches!(k, "clickImage" | "waitImage") && s(st, "image").is_empty() {
            say(o, i18n, p, "macro.err.image", &[], false);
        }
        if k == "waitText" && s(st, "words").is_empty() && s(st, "text").trim().is_empty() {
            say(o, i18n, p, "macro.err.text", &[], false);
        }
        if k == "wait" && !ok_num(st.get("ms"), 0., 3_600_000.) {
            say(o, i18n, p, "macro.err.wait", &[], false);
        }
        if matches!(k, "clickImage" | "waitImage" | "waitText" | "waitColour" | "waitStill" | "read") && !ok_num(st.get("timeoutMs"), 100., 3_600_000.) {
            say(o, i18n, p, "macro.err.timeout", &[], false);
        }
        if k == "type" && s(st, "text").trim().is_empty() {
            say(o, i18n, p, "macro.err.typeEmpty", &[], false);
        }
        if k == "type" && s(st, "text").chars().count() > 200 {
            say(o, i18n, p, "macro.err.typeLong", &[], false);
        }
        if matches!(k, "setVar" | "addVar" | "read") && s(st, "name").trim().is_empty() {
            say(o, i18n, p, "macro.err.varName", &[], false);
        }
        if matches!(k, "setVar" | "addVar") && !st.get("text").is_some_and(Value::is_string) {
            let bad = match st.get("expr") {
                Some(e) => e.as_str().is_none_or(|e| e.trim().is_empty()),
                None => !ok_num(st.get("value"), -1e12, 1e12),
            };
            if bad {
                say(o, i18n, p, "macro.err.formula", &[], false);
            }
        }
        if k == "read" && matches!(s(st, "from"), "" | "screen") && !has(st, "region") {
            say(o, i18n, p, "macro.err.readArea", &[], false);
            fix_last(o, "area");
        }
        let place = match k {
            "read" => s(st, "from"),
            "write" => Some(s(st, "to")).filter(|x| !x.is_empty()).unwrap_or("file"),
            _ => "",
        };
        if place == "file" {
            let n = s(st, "fileName").trim();
            if n.is_empty() || n.chars().count() > 40 || !n.chars().all(|c| c.is_alphanumeric() || matches!(c, ' ' | '_' | '-')) {
                say(o, i18n, p, "macro.err.fileName", &[], false);
            }
        }
        if place == "web" {
            let u = s(st, "url").trim();
            if !(u.starts_with("https://") && u.len() > 8 && !u.contains(char::is_whitespace)) {
                say(o, i18n, p, "macro.err.url", &[], false);
            }
        }
        if k == "write" && s(st, "text").trim().is_empty() {
            say(o, i18n, p, "macro.err.writeEmpty", &[], false);
        }
        if k == "call" {
            let id = s(st, "macroId");
            if id.is_empty() {
                say(o, i18n, p, "macro.err.callPick", &[], false);
            } else if !saved.iter().any(|x| s(x, "id") == id) {
                say(o, i18n, p, "macro.err.callGone", &[], false);
            }
        }
        if k == "alert" && s(st, "text").trim().is_empty() {
            say(o, i18n, p, "macro.err.alertEmpty", &[], false);
        }
        if k == "follow" {
            if !has(st, "region") {
                say(o, i18n, p, "macro.err.followArea", &[], false);
                fix_last(o, "area");
            }
            let marker = st.get("marker");
            let marker_ok = match marker {
                Some(mk) if mk.is_object() => ok_colour(mk.get("left")) && ok_colour(mk.get("right")),
                mk => ok_colour(mk),
            };
            if !ok_colour(st.get("target")) || !marker_ok {
                say(o, i18n, p, "macro.err.colour", &[], false);
            }
            if s(st, "mode") == "smooth" && !(ok_num_or(st.get("kp"), 1., 0., 10.) && ok_num_or(st.get("kd"), 0.4, 0., 10.)) {
                say(o, i18n, p, "macro.err.gains", &[], false);
            }
            if !ok_num(st.get("tolerance"), 0., 255.) {
                say(o, i18n, p, "macro.err.tolerance", &[], false);
            }
            if s(st, "button") == "key" && !keys.iter().any(|x| x == key) {
                say(o, i18n, p, "macro.err.key", &[], false);
            }
            if !ok_num(st.get("untilGoneMs"), 100., 60000.) {
                say(o, i18n, p, "macro.err.untilGone", &[], false);
            }
            if !ok_num_or(st.get("fps"), 60., 15., 240.) {
                say(o, i18n, p, "macro.err.fps", &[], false);
            }
            if !ok_num(st.get("timeoutMs"), 100., 3_600_000.) {
                say(o, i18n, p, "macro.err.timeout", &[], false);
            }
        }
        if k == "waitColour" && !ok_colour(st.get("colour")) {
            say(o, i18n, p, "macro.err.colour", &[], false);
        }
        if k == "click" && st.get("holdMs").is_some_and(|h| !h.is_null() && h.as_f64() != Some(0.)) && !ok_num(st.get("holdMs"), 30., 10000.) {
            say(o, i18n, p, "macro.err.holdMs", &[], false);
        }
        if k == "clickColour" {
            if !ok_colour(st.get("colour")) {
                say(o, i18n, p, "macro.err.colour", &[], false);
            }
            if !ok_num(st.get("tolerance"), 0., 255.) {
                say(o, i18n, p, "macro.err.tolerance", &[], false);
            }
            if has(st, "minSize") && !ok_num(st.get("minSize"), 0., 10000.) {
                say(o, i18n, p, "macro.err.minSize", &[], false);
            }
            if !ok_num(st.get("timeoutMs"), 100., 3_600_000.) {
                say(o, i18n, p, "macro.err.timeout", &[], false);
            }
            if has(st, "repeatUntilGoneMs") && !ok_num(st.get("repeatUntilGoneMs"), 0., 600000.) {
                say(o, i18n, p, "macro.err.untilGone", &[], false);
            }
        }
        // A setting or area the macro no longer has: the run would stop there.
        let mut values: Vec<&Value> = st.as_object().map(|x| x.values().collect()).unwrap_or_default();
        for sub in ["test", "marker"] {
            if let Some(x) = st.get(sub).and_then(Value::as_object) {
                values.extend(x.values());
            }
        }
        let (mut no_opt, mut no_area) = (false, false);
        for v in values {
            no_opt |= opt_ref(Some(v)).is_some_and(|n| !opt_names.contains(n));
            no_area |= area_ref(Some(v)).is_some_and(|n| !area_names.contains(n));
        }
        if no_opt {
            say(o, i18n, p, "macro.err.noOption", &[], false);
        }
        if no_area {
            say(o, i18n, p, "macro.err.noArea", &[], false);
            fix_last(o, "area");
        }
        if is_test(k) {
            let t = st.get("test").unwrap_or(&Value::Null);
            match s(t, "kind") {
                "image" if s(t, "image").is_empty() => say(o, i18n, p, "macro.err.image", &[], false),
                "text" if s(t, "words").is_empty() && s(t, "text").trim().is_empty() => say(o, i18n, p, "macro.err.text", &[], false),
                "var" if s(t, "name").trim().is_empty() => say(o, i18n, p, "macro.err.varName", &[], false),
                _ => {}
            }
        }
    }
    for (is_if, at, _) in open {
        say(o, i18n, Some(at), if is_if { "macro.err.ifOpen" } else { "macro.err.loopOpen" }, &[], false);
    }
    for (k, i) in held {
        say(o, i18n, Some(i), "macro.err.neverUp", &[("k", &key_cap(&k))], false);
    }
}

// ───────────────────────── words ─────────────────────────

pub(crate) fn key_cap(k: &str) -> String {
    match k {
        "Minus" => "-".into(),
        "Equals" => "=".into(),
        "LeftBracket" => "[".into(),
        "RightBracket" => "]".into(),
        "Semicolon" => ";".into(),
        "Quote" => "'".into(),
        "Comma" => ",".into(),
        "Period" => ".".into(),
        "Slash" => "/".into(),
        "Backslash" => "\\".into(),
        "Backquote" => "`".into(),
        "Up" => "↑".into(),
        "Down" => "↓".into(),
        "Left" => "←".into(),
        "Right" => "→".into(),
        "PageUp" => "Page Up".into(),
        "PageDown" => "Page Down".into(),
        k if k.starts_with("Numpad") => format!("Num {}", k.trim_start_matches("Numpad")),
        k => k.to_string(),
    }
}

pub(crate) fn key_name(root: &Root, k: &str) -> String {
    if MOUSE_KEYS.contains(&k) { root.tr(&format!("macro.mk.{k}")) } else { key_cap(k) }
}

/// The keys a step may press (macros.ts's KEYS).
pub(crate) fn all_keys() -> Vec<String> {
    let mut v: Vec<String> = ('A'..='Z').map(|ch| ch.to_string()).collect();
    v.extend((0..=9).map(|n| n.to_string()));
    v.extend((1..=12).map(|n| format!("F{n}")));
    v.extend(
        [
            "Space", "Enter", "Tab", "Backspace", "Escape", "Shift", "Ctrl", "Alt", "Up", "Down", "Left", "Right", "Insert", "Delete", "Home", "End", "PageUp", "PageDown", "Minus", "Equals", "LeftBracket",
            "RightBracket", "Semicolon", "Quote", "Comma", "Period", "Slash", "Backslash", "Backquote",
        ]
        .iter()
        .map(|k| k.to_string()),
    );
    v.extend((0..=9).map(|n| format!("Numpad{n}")));
    v
}

/// A key pressed in the "Press a key" box, as one of the keys' names (macros.ts's keyFromEvent); None for one a macro
/// can't press.
pub(crate) fn key_from(k: &str) -> Option<String> {
    let k = k.to_lowercase();
    let mut ch = k.chars();
    if let (Some(c), None) = (ch.next(), ch.next()) {
        if c.is_ascii_alphabetic() {
            return Some(c.to_ascii_uppercase().to_string());
        }
        if c.is_ascii_digit() {
            return Some(c.to_string());
        }
    }
    if let Some(n) = k.strip_prefix('f').and_then(|n| n.parse::<u32>().ok()).filter(|n| (1..=12).contains(n)) {
        return Some(format!("F{n}"));
    }
    Some(
        match k.as_str() {
            "space" => "Space",
            "enter" => "Enter",
            "tab" => "Tab",
            "backspace" => "Backspace",
            "escape" => "Escape",
            "shift" => "Shift",
            "control" | "ctrl" => "Ctrl",
            "alt" => "Alt",
            "up" => "Up",
            "down" => "Down",
            "left" => "Left",
            "right" => "Right",
            "insert" => "Insert",
            "delete" => "Delete",
            "home" => "Home",
            "end" => "End",
            "pageup" => "PageUp",
            "pagedown" => "PageDown",
            "-" => "Minus",
            "=" => "Equals",
            "[" => "LeftBracket",
            "]" => "RightBracket",
            ";" => "Semicolon",
            "'" => "Quote",
            "," => "Comma",
            "." => "Period",
            "/" => "Slash",
            "\\" => "Backslash",
            "`" => "Backquote",
            _ => return None,
        }
        .to_string(),
    )
}

fn dur(root: &Root, secs: i64) -> String {
    let t = |key: &str, n: i64| root.i18n.tv(key, &[("n", &n.to_string())]);
    if secs < 60 {
        return t("macro.dur.s", secs);
    }
    let (h, m, r) = (secs / 3600, secs % 3600 / 60, secs % 60);
    let mut v = Vec::new();
    if h > 0 {
        v.push(t("macro.dur.h", h));
    }
    if m > 0 {
        v.push(t("macro.dur.min", m));
    }
    if r > 0 {
        v.push(t("macro.dur.s", r));
    }
    v.join(" ")
}

/// A value, or the setting of the macro's it stands for ("$o1").
fn val(m: &Value, v: Option<&Value>) -> Value {
    match opt_at(m, v) {
        Some(i) => arr(m, "options").get(i).and_then(|o| o.get("value")).cloned().unwrap_or(Value::Null),
        None => v.cloned().unwrap_or(Value::Null),
    }
}

fn said(v: &Value) -> String {
    match v {
        Value::String(t) => t.clone(),
        Value::Number(n) => n.as_f64().map(trim).unwrap_or_default(),
        Value::Bool(b) => b.to_string(),
        _ => String::new(),
    }
}

fn ms_text(root: &Root, m: &Value, v: Option<&Value>) -> String {
    match val(m, v).as_f64() {
        Some(ms) if ms < 60000. && ms % 1000. != 0. => root.i18n.tv("macro.dur.s", &[("n", &trim(ms / 1000.))]),
        Some(ms) => dur(root, (ms / 1000.).round() as i64),
        None => "…".into(),
    }
}

fn or_dots(t: &str) -> String {
    if t.is_empty() { "…".into() } else { t.to_string() }
}

/// What a step does, as a sentence (macros.ts's stepText): its row's words. `n` is its number, a point's name.
fn step_text(root: &Root, m: &Value, names: &[(String, String)], st: &Value, n: usize) -> String {
    let i = &root.i18n;
    let n = n.to_string();
    let kind = s(st, "kind");
    let area = |r: Option<&Value>| match area_ref(r) {
        Some(a) => {
            let label = arr(m, "areas").iter().find(|x| s(x, "name") == a).map(|x| s(x, "label").to_string()).unwrap_or_else(|| root.tr("macro.s.gone"));
            i.tv("macro.s.inArea", &[("area", &label)])
        }
        None => root.tr("macro.s.anywhere"),
    };
    let wher = area(st.get("region"));
    let shows = if st.get("gone").and_then(Value::as_bool) == Some(true) { "gone" } else { "shows" };
    let option = |name: &str| arr(m, "options").iter().find(|o| s(o, "name") == name);
    let counter = |name: &str| if name.is_empty() { "…".to_string() } else { m.pointer("/panel/counterLabels").and_then(|l| l.get(name)).and_then(Value::as_str).unwrap_or(name).to_string() };
    let list = |words: &str| option(words.trim_start_matches('$')).map(|o| s(o, "label").to_string()).unwrap_or_else(|| root.tr("macro.s.gone"));
    let button = root.tr(&format!("macro.s.b.{}", match s(st, "button") {
        "right" => "right",
        "middle" => "middle",
        _ => "left",
    }));
    let key = |k: &Value| key_name(root, k.as_str().unwrap_or_default());
    let formula = || st.get("expr").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| said(&val(m, st.get("value"))));
    match kind {
        "key" => i.tv("macro.s.key", &[("k", &key(&val(m, st.get("key"))))]),
        "down" => i.tv("macro.s.down", &[("k", &key_name(root, s(st, "key")))]),
        "up" => i.tv("macro.s.up", &[("k", &key_name(root, s(st, "key")))]),
        "type" => i.tv("macro.s.type", &[("text", &or_dots(s(st, "text")))]),
        "click" => {
            if st.get("holdMs").is_some_and(|h| h.is_string() || h.as_f64().is_some_and(|x| x > 0.)) {
                i.tv("macro.s.clickHold", &[("b", &button), ("d", &ms_text(root, m, st.get("holdMs"))), ("n", &n)])
            } else {
                i.tv(if st.get("double").and_then(Value::as_bool) == Some(true) { "macro.s.dblClick" } else { "macro.s.click" }, &[("b", &button), ("n", &n)])
            }
        }
        "clickImage" => i.tv("macro.s.clickImage", &[("where", &wher)]),
        "clickColour" => i.tv(if has(st, "eachAfterMs") { "macro.s.clickEach" } else { "macro.s.clickColour" }, &[("where", &wher)]),
        "move" => i.tv("macro.s.move", &[("n", &n)]),
        "camera" => i.tv("macro.s.camera", &[("dir", &root.tr(&format!("macro.e.dir.{}", cam_dir(st))))]),
        "scroll" => i.tv(if num(st, "notches") > 0. { "macro.s.scrollUp" } else { "macro.s.scrollDown" }, &[("n", &trim(num(st, "notches").abs()))]),
        "wait" => i.tv("macro.s.wait", &[("d", &ms_text(root, m, st.get("ms")))]),
        "waitImage" => i.tv(&format!("macro.s.waitImage.{shows}"), &[("where", &wher)]),
        "waitText" => {
            let words = s(st, "words");
            if words.is_empty() {
                i.tv(&format!("macro.s.waitText.{shows}"), &[("text", &or_dots(s(st, "text"))), ("where", &wher)])
            } else {
                i.tv(&format!("macro.s.waitWords.{shows}"), &[("name", &list(words)), ("where", &wher)])
            }
        }
        "waitColour" => {
            let at = if has(st, "region") { wher.clone() } else { i.tv("macro.s.atPoint", &[("n", &n)]) };
            i.tv(&format!("macro.s.waitColour.{shows}"), &[("where", &at)])
        }
        "waitStill" => i.tv("macro.s.waitStill", &[("d", &ms_text(root, m, st.get("ms")))]),
        "if" | "loopUntil" => {
            let lead = root.tr(if kind == "if" { "macro.s.if" } else { "macro.s.until" });
            let Some(test) = st.get("test").filter(|t| t.is_object()) else { return lead };
            let tk = s(test, "kind");
            if tk == "var" {
                let name = s(test, "name");
                let opt = option(name);
                if let Some(o) = opt.filter(|o| s(o, "kind") == "check") {
                    let on = test.get("value").is_some_and(|v| v.as_bool() == Some(true) || v.as_f64().is_some_and(|x| x != 0.));
                    return format!("{lead} {}", i.tv(if on { "macro.s.switchOn" } else { "macro.s.switchOff" }, &[("name", s(o, "label"))]));
                }
                let shown = opt.map(|o| s(o, "label").to_string()).unwrap_or_else(|| counter(name));
                let op = root.tr(&format!("macro.e.op.{}", Some(s(test, "op")).filter(|o| !o.is_empty()).unwrap_or(">=")));
                return format!("{lead} {}", i.tv(if opt.is_some() { "macro.s.settingIs" } else { "macro.s.counterIs" }, &[("name", &shown), ("op", &op), ("v", &said(&val(m, test.get("value"))))]));
            }
            let at = if has(test, "region") {
                area(test.get("region"))
            } else if tk == "colour" {
                i.tv("macro.s.atPoint", &[("n", &n)])
            } else {
                root.tr("macro.s.anywhere")
            };
            let what = if tk == "text" {
                let words = s(test, "words");
                if words.is_empty() { i.tv("macro.s.theText", &[("text", &or_dots(s(test, "text")))]) } else { i.tv("macro.s.theWords", &[("name", &list(words))]) }
            } else {
                root.tr(&format!("macro.s.the.{tk}"))
            };
            let not = test.get("not").and_then(Value::as_bool) == Some(true);
            format!("{lead} {}", i.tv(if not { "macro.s.isGone" } else { "macro.s.shows" }, &[("what", &what), ("where", &at)]))
        }
        "loop" => {
            let times = val(m, st.get("times"));
            if times.as_f64().is_some_and(|t| t > 0.) { i.tv("macro.s.loopN", &[("n", &said(&times))]) } else { root.tr("macro.s.loop") }
        }
        "setVar" => match st.get("text").and_then(Value::as_str) {
            Some(t) => i.tv("macro.s.setText", &[("name", &counter(s(st, "name"))), ("text", t)]),
            None => i.tv("macro.s.setVar", &[("name", &counter(s(st, "name"))), ("v", &formula())]),
        },
        "addVar" => i.tv("macro.s.addVar", &[("name", &counter(s(st, "name"))), ("v", &formula())]),
        "read" => {
            let from = match s(st, "from") {
                "clipboard" => root.tr("macro.s.fromClip"),
                "file" => i.tv("macro.s.fromFile", &[("name", &or_dots(s(st, "fileName")))]),
                "web" => i.tv("macro.s.fromWeb", &[("host", &or_dots(s(st, "url").trim_start_matches("https://").split('/').next().unwrap_or_default()))]),
                _ => wher.clone(),
            };
            i.tv(&format!("macro.s.read.{}", if s(st, "mode") == "text" { "text" } else { "number" }), &[("name", &counter(s(st, "name"))), ("where", &from)])
        }
        "write" => {
            let to = match s(st, "to") {
                "clipboard" => root.tr("macro.s.toClip"),
                "web" => i.tv("macro.s.toWeb", &[("host", &or_dots(s(st, "url").trim_start_matches("https://").split('/').next().unwrap_or_default()))]),
                _ => i.tv("macro.s.toFile", &[("name", &or_dots(s(st, "fileName")))]),
            };
            i.tv("macro.s.write", &[("text", &or_dots(s(st, "text"))), ("to", &to)])
        }
        "call" => {
            let id = s(st, "macroId");
            let name = names.iter().find(|(i, _)| i == id).map(|(_, n)| n.clone()).unwrap_or_else(|| "…".into());
            i.tv("macro.s.call", &[("name", &name)])
        }
        "alert" => root.tr(if st.get("picture").and_then(Value::as_bool) == Some(true) { "macro.s.alertPic" } else { "macro.s.alert" }),
        "follow" => i.tv("macro.s.follow", &[("where", &wher)]),
        "recover" => root.tr("macro.s.recover"),
        k => root.tr(&format!("macro.kind.{k}")),
    }
}

/// A camera turn's way, from its drag.
fn cam_dir(st: &Value) -> &'static str {
    let (dx, dy) = (num(st, "dx"), num(st, "dy"));
    if dx.abs() >= dy.abs() {
        if dx < 0. { "left" } else { "right" }
    } else if dy < 0. {
        "up"
    } else {
        "down"
    }
}

fn cam_amount(st: &Value) -> &'static str {
    let size = num(st, "dx").abs().max(num(st, "dy").abs());
    if size <= 150. {
        "little"
    } else if size <= 350. {
        "some"
    } else {
        "lot"
    }
}

fn cam_turn(st: &mut Value, dir: &str, amount: &str) {
    let by = match amount {
        "little" => 100,
        "lot" => 500,
        _ => 250,
    };
    set(st, "dx", json!(match dir {
        "left" => -by,
        "right" => by,
        _ => 0,
    }));
    set(st, "dy", json!(match dir {
        "up" => -by,
        "down" => by,
        _ => 0,
    }));
    set(st, "button", json!("right"));
    if !st.get("ms").is_some_and(Value::is_number) {
        set(st, "ms", json!(300));
    }
}

/// The steps' icons (macros.ts's STEP_ICON), the ones this page has.
pub(crate) fn step_icon(kind: &str) -> &'static str {
    match kind {
        "key" | "down" | "up" => "keyboard",
        "click" | "move" => "mouse",
        "clickImage" | "waitImage" => "image",
        "wait" | "waitStill" => "clock",
        "waitText" => "search",
        "if" | "else" | "endIf" => "branch",
        "loop" | "loopUntil" | "endLoop" => "repeat",
        "stop" | "break" => "stop",
        "rejoin" => "refresh",
        "type" => "message",
        "follow" => "gauge",
        "call" => "play",
        "alert" => "bell",
        "setVar" | "addVar" => "tag",
        "waitColour" | "clickColour" => "palette",
        "scroll" => "sort",
        "camera" | "read" => "eye",
        "write" => "edit",
        "recover" => "wrench",
        _ => "info",
    }
}

// ───────────────────────── drawing ─────────────────────────

/// A change to the form from a click; the open step's boxes follow what the step now is, and the game's dots too.
fn edit(cx: &mut Context<Root>, f: impl Fn(&mut Form) + 'static) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    cx.listener(move |r, _, window, cx| {
        if let Some(form) = r.auto.macro_form.as_mut() {
            form.error = None;
            f(form);
            fill(form, window, cx);
        }
        push_game(r);
        cx.notify();
    })
}

/// A row of choices; one that can't be picked now is dimmed.
fn pick(cx: &mut Context<Root>, key: &'static str, items: Vec<(String, String, bool)>, selected: &str, f: impl Fn(&mut Form, &str) + 'static) -> Div {
    let f = Rc::new(f);
    let mut row = div().flex().flex_wrap().gap(px(6.));
    for (i, (value, label, live)) in items.into_iter().enumerate() {
        let on = value == selected;
        let f = f.clone();
        row = row.child(pill((key, i), on, live).child(label).when(live, |d| d.on_click(edit(cx, move |form| f(form, &value)))));
    }
    row
}

/// A row of choices that change the step at `i`.
fn step_pick(cx: &mut Context<Root>, key: &'static str, i: usize, items: Vec<(String, String)>, selected: &str, f: impl Fn(&mut Value, &str) + 'static) -> Div {
    pick(cx, key, items.into_iter().map(|(v, l)| (v, l, true)).collect(), selected, move |form, v| {
        if let Some(st) = form.step_mut(i) {
            f(st, v);
        }
    })
}

fn section(title: String, body: impl IntoElement) -> Div {
    div().flex().flex_col().gap(px(8.)).child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT_2)).child(title)).child(body)
}

fn lab(label: String, body: impl IntoElement) -> Div {
    div().flex().flex_col().gap(px(6.)).child(div().text_size(px(T_S)).text_color(c(MUTED)).child(label)).child(body)
}

fn bad_line(t: String, warn: bool) -> Div {
    let tone = if warn { c(WARN) } else { c(BAD) };
    div().flex().items_start().gap(px(7.)).text_size(px(T_S)).line_height(px(17.)).text_color(tone).child(icon("alert", 13., tone).mt(px(2.))).child(div().min_w_0().child(t))
}

/// A section of the editor: a calm panel on the page, a hairline round it (the preset editor's).
/// The page's sides (the Accounts page's).
const SIDE: f32 = 24.;

fn card() -> Div {
    div().rounded(px(R_L + 2.)).bg(ca(WHITE, 0.03)).border_1().border_color(c(LINE)).p(px(20.)).flex().flex_col().gap(px(18.))
}

fn swatch(hex: &str, size: f32) -> Div {
    div().size(px(size)).flex_none().rounded(px(6.)).bg(hex_c(hex)).border_1().border_color(ca(WHITE, 0.2))
}

/// The key a step presses: chosen from the list, or the next key pressed in the "Press a key" box.
fn key_box(root: &Root, f: &Form, i: usize, path: &'static str, mouse: bool, cx: &mut Context<Root>) -> (Div, Option<Div>) {
    let t = |k: &str| root.tr(k);
    let st = f.steps().get(i).unwrap_or(&Value::Null);
    let cur = val(&f.m, get(st, path)).as_str().unwrap_or_default().to_string();
    let shown = if cur.is_empty() { "—".to_string() } else { key_name(root, &cur) };
    let list = hot("mf-key-btn")
        .h(px(36.))
        .min_w(px(84.))
        .px(px(12.))
        .flex()
        .items_center()
        .gap(px(8.))
        .rounded(px(R_M))
        .border_1()
        .tones(c(RAISED), c(RAISED_2))
        .edge(ca(WHITE, if f.keys { 0.3 } else { 0.1 }), ca(WHITE, 0.3))
        .cursor_pointer()
        .text_size(px(T_M))
        .font_weight(FontWeight::SEMIBOLD)
        .child(div().flex_1().child(shown))
        .child(icon(if f.keys { "chevronUp" } else { "chevronDown" }, 14., c(MUTED)))
        .on_click(edit(cx, |form| {
            form.keys = !form.keys;
            form.capture = false;
        }));
    let listening = f.capture;
    let cap = div()
        .id("mf-key-cap")
        .track_focus(&f.key_focus)
        .h(px(36.))
        .px(px(12.))
        .flex()
        .items_center()
        .gap(px(8.))
        .rounded(px(R_M))
        .border_1()
        .border_color(if listening { ca(FACE, 0.85) } else { ca(WHITE, 0.1) })
        .bg(if listening { ca(FACE, 0.1) } else { c(RAISED) })
        .cursor_pointer()
        .text_size(px(T_M))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(if listening { c(FACE) } else { c(TEXT_2) })
        .child(icon("keyboard", 15., if listening { c(FACE) } else { c(MUTED) }))
        .child(t(if listening { "macro.f.pressing" } else { "macro.f.press" }))
        .tooltip({
            let tip: SharedString = t("macro.f.pressTip").into();
            move |_, cx| cx.new(|_| Tip(tip.clone())).into()
        })
        .on_click(cx.listener(|r, _, window, cx| {
            if let Some(f) = r.auto.macro_form.as_mut() {
                f.capture = !f.capture;
                f.keys = false;
                if f.capture {
                    window.focus(&f.key_focus, cx);
                }
            }
            cx.notify();
        }))
        .on_key_down(cx.listener(move |r, e: &KeyDownEvent, window, cx| {
            if !r.auto.macro_form.as_ref().is_some_and(|f| f.capture) {
                return;
            }
            cx.stop_propagation();
            let bad = r.tr("macro.f.badKey");
            let Some(f) = r.auto.macro_form.as_mut() else { return };
            match key_from(&e.keystroke.key) {
                Some(k) => {
                    f.put_val(i, path, json!(k));
                    f.capture = false;
                    f.error = None;
                    fill(f, window, cx);
                }
                None => f.error = Some(bad),
            }
            cx.notify();
        }));
    let row = div().flex().items_center().gap(px(8.)).child(list).child(cap);
    let under = f.keys.then(|| {
        let mut keys = all_keys();
        if mouse {
            keys.extend(MOUSE_KEYS.iter().map(|k| k.to_string()));
        }
        let mut grid = div().flex().flex_wrap().gap(px(4.));
        for (n, k) in keys.into_iter().enumerate() {
            let on = k == cur;
            let label = key_name(root, &k);
            grid = grid.child(
                hot(("mf-key", n))
                    .h(px(30.))
                    .min_w(px(34.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(7.))
                    .map(|d| if on { d.tones(c(FACE), c(FACE_HI)).text_color(crate::theme::on_text()) } else { d.tones(ca(WHITE, 0.05), ca(WHITE, 0.12)).text_color(c(TEXT_2)) })
                    .cursor_pointer()
                    .text_size(px(T_S))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(label)
                    .on_click(edit(cx, move |form| {
                        form.put_val(i, path, json!(k));
                        form.keys = false;
                    })),
            );
        }
        grid
    });
    (lab(t("macro.f.key"), row), under)
}

/// Where the step looks: anywhere (or at its point), in one of the areas, or in a new one.
fn where_row(root: &Root, f: &Form, i: usize, st: &Value, path: &'static str, with_point: bool, cx: &mut Context<Root>) -> Div {
    let n = (i + 1).to_string();
    let raw = get(st, path);
    let cur = match raw.and_then(Value::as_str) {
        Some(a) if a.starts_with('@') => a.to_string(),
        _ => if with_point { "point" } else { "anywhere" }.to_string(),
    };
    let mut items = vec![if with_point { ("point".to_string(), root.i18n.tv("macro.s.atPoint", &[("n", &n)]), true) } else { ("anywhere".to_string(), root.tr("macro.s.anywhere"), true) }];
    for a in arr(&f.m, "areas") {
        items.push((format!("@{}", s(a, "name")), root.i18n.tv("macro.s.inArea", &[("area", s(a, "label"))]), true));
    }
    if cur.starts_with('@') && !items.iter().any(|(v, _, _)| *v == cur) {
        items.push((cur.clone(), root.i18n.tv("macro.s.inArea", &[("area", &root.tr("macro.s.gone"))]), true));
    }
    items.push(("new".into(), root.tr("macro.area.newIn"), arr(&f.m, "areas").len() < MAX_AREAS));
    let full = root.i18n.tv("macro.area.full", &[("n", &MAX_AREAS.to_string())]);
    lab(root.tr("macro.e.whereL"), pick(cx, "mf-where", items, &cur, move |form, v| form.set_region(i, path, v, &full)))
}

/// Shows or goes away.
fn shows_row(root: &Root, st: &Value, i: usize, path: &'static str, cx: &mut Context<Root>) -> Div {
    let gone = get(st, path).and_then(Value::as_bool) == Some(true);
    lab(
        root.tr("macro.e.showsL"),
        step_pick(cx, "mf-shows", i, vec![("shows".into(), root.tr("macro.e.shows")), ("gone".into(), root.tr("macro.e.gone"))], if gone { "gone" } else { "shows" }, move |st, v| put(st, path, json!(v == "gone"))),
    )
}

/// A text to look for, or any word the player ticks in a list setting (once the panel has one).
fn words_row(root: &Root, f: &Form, st: &Value, i: usize, path: &'static str, cx: &mut Context<Root>) -> Option<Div> {
    let words = get(st, path).and_then(Value::as_str).unwrap_or_default().to_string();
    let lists: Vec<&Value> = arr(&f.m, "options").iter().filter(|o| s(o, "kind") == "list").collect();
    if lists.is_empty() && words.is_empty() {
        return None;
    }
    let mut items = vec![(String::new(), root.tr("macro.e.thisText"))];
    items.extend(lists.iter().map(|o| (format!("${}", s(o, "name")), root.i18n.tv("macro.e.oneOf", &[("name", s(o, "label"))]))));
    Some(lab(
        root.tr("macro.e.readL"),
        step_pick(cx, "mf-words", i, items, &words, move |st, v| {
            if v.is_empty() {
                drop_at(st, path);
            } else {
                put(st, path, json!(v));
            }
        }),
    ))
}

/// A picture step's picture: its thumbnail and the button that cuts it from the still.
fn pic_row(root: &Root, f: &Form, i: usize, st: &Value, path: &'static str, cx: &mut Context<Root>) -> Div {
    let b64 = get(st, path).and_then(Value::as_str).unwrap_or_default();
    let image = thumb(f, b64, cx);
    let cutting = f.cutting == Some(i);
    let label = if cutting {
        "macro.e.cutting"
    } else if b64.is_empty() {
        "macro.e.cut"
    } else {
        "macro.e.recut"
    };
    lab(
        root.tr("macro.e.thePic"),
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(
                div()
                    .w(px(96.))
                    .h(px(48.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(R_S))
                    .bg(c(INSET))
                    .border_1()
                    .border_color(c(LINE))
                    .overflow_hidden()
                    .map(|d| match image {
                        Some(p) => d.child(img(p).size_full().object_fit(ObjectFit::Contain)),
                        None => d.child(icon("image", 18., c(DIM))),
                    }),
            )
            .child(Button::new("mf-cut", root.tr(label)).icon("image").small().map(|b| if cutting { b.tint() } else { b }).on_click(cx.listener(move |r, _, window, cx| start_cut(r, i, window, cx)))),
    )
}

/// A counter by what the panel calls it (for an If, the panel's settings too), or New name…: its name and what the
/// panel calls it, typed under the row.
fn counter_pick(root: &Root, f: &Form, i: usize, path: &'static str, with_settings: bool, cx: &mut Context<Root>) -> (Div, Option<Div>) {
    let t = |k: &str| root.tr(k);
    let m = &f.m;
    let cur = f.steps().get(i).and_then(|st| get(st, path)).and_then(Value::as_str).unwrap_or_default().to_string();
    let mut items: Vec<(String, String)> = f.counters().into_iter().map(|c| {
        let label = counter_name(m, &c).to_string();
        (c, label)
    }).collect();
    if with_settings {
        items.extend(arr(m, "options").iter().filter(|o| matches!(s(o, "kind"), "check" | "choice" | "number")).map(|o| (s(o, "name").to_string(), s(o, "label").to_string())));
    }
    if !cur.is_empty() && !items.iter().any(|(k, _)| *k == cur) {
        items.insert(0, (cur.clone(), counter_name(m, &cur).to_string()));
    }
    let naming = f.naming == Some((i, path));
    let mut row = div().flex().flex_wrap().gap(px(6.));
    for (n, (name, label)) in items.into_iter().enumerate() {
        let on = !naming && name == cur;
        row = row.child(pill(("mf-ctr", n), on, true).child(label).on_click(edit(cx, move |form| {
            form.naming = None;
            if let Some(st) = form.step_mut(i) {
                put(st, path, json!(name));
            }
        })));
    }
    row = row.child(pill("mf-ctr-new", naming, true).gap(px(6.)).child(icon("plus", 13., if naming { crate::theme::on_text() } else { c(TEXT_2) })).child(t("macro.counter.new")).on_click(cx.listener(
        move |r, _, window, cx| {
            if let Some(f) = r.auto.macro_form.as_mut() {
                f.naming = if f.naming == Some((i, path)) { None } else { Some((i, path)) };
                if f.naming.is_some() {
                    set_box(&f.counter_name, String::new(), window, cx);
                    set_box(&f.counter_label, String::new(), window, cx);
                    f.counter_name.update(cx, |b, cx| b.focus(window, cx));
                }
            }
            cx.notify();
        },
    )));
    let new_row = naming.then(|| {
        let typed = text(&f.counter_name, cx);
        let bad = !typed.is_empty() && !counter_ok(&typed);
        let dup = !typed.is_empty() && f.counters().iter().any(|c| *c == typed);
        div()
            .flex()
            .flex_wrap()
            .items_end()
            .gap_x(px(14.))
            .gap_y(px(8.))
            .p(px(12.))
            .rounded(px(R_M))
            .bg(ca(WHITE, 0.04))
            .child(lab(t("macro.e.counterL"), field(&f.counter_name, None).h(px(34.)).w(px(170.))))
            .child(lab(t("macro.counter.label"), field(&f.counter_label, None).h(px(34.)).w(px(200.))))
            .child(
                Button::new("mf-ctr-add", t("macro.opt.add"))
                    .primary()
                    .small()
                    .disabled(typed.is_empty() || bad || dup)
                    .on_click(cx.listener(move |r, _, window, cx| {
                        let Some(f) = r.auto.macro_form.as_mut() else { return };
                        let (name, label) = (text(&f.counter_name, cx).trim().to_string(), text(&f.counter_label, cx));
                        let res = f.new_counter(&name, &label);
                        let err = res.err().map(|k| r.tr(k));
                        if let Some(f) = r.auto.macro_form.as_mut() {
                            f.error = err;
                            fill(f, window, cx);
                        }
                        cx.notify();
                    })),
            )
            .child(Button::new("mf-ctr-x", t("common.cancel")).quiet().small().on_click(edit(cx, |form| form.naming = None)))
            .child(div().w_full().text_size(px(T_S)).text_color(if bad || dup { c(BAD) } else { c(MUTED) }).child(t(if dup { "macro.counter.taken" } else { "macro.counter.nameHint" })))
    });
    (lab(t("macro.e.counterL"), row), new_row)
}

/// What the open step can be set to: its choices, its boxes, its note, the values the player may change.
fn step_body(root: &Root, f: &Form, i: usize, st: &Value, others: &[(String, String)], win: bool, probs: &[&Prob], cx: &mut Context<Root>) -> Div {
    let kind = s(st, "kind").to_string();
    let m = &f.m;
    let t = |k: &str| root.tr(k);
    let pairs = |keys: &[&str], pre: &str| -> Vec<(String, String)> { keys.iter().map(|k| (k.to_string(), root.tr(&format!("{pre}{k}")))).collect() };
    let mut row = div().flex().flex_wrap().items_end().gap_x(px(18.)).gap_y(px(12.));
    let mut notes: Vec<String> = Vec::new();
    let mut under: Vec<AnyElement> = Vec::new();
    let test = st.get("test").cloned().unwrap_or(Value::Null);
    match kind.as_str() {
        "key" | "down" | "up" => {
            let (k, grid) = key_box(root, f, i, "key", kind != "key", cx);
            row = row.child(k);
            under.extend(grid.map(IntoElement::into_any_element));
        }
        "type" => notes.push(t("macro.typeNote")),
        "click" => {
            let how = if has(st, "holdMs") {
                "hold"
            } else if st.get("double").and_then(Value::as_bool) == Some(true) {
                "double"
            } else {
                "click"
            };
            row = row.child(lab(t("macro.e.howL"), step_pick(cx, "mf-how", i, pairs(&["click", "double", "hold"], "macro.e.how."), how, |st, v| {
                set(st, "double", json!(v == "double"));
                if v == "hold" {
                    if !has(st, "holdMs") {
                        set(st, "holdMs", json!(600));
                    }
                } else {
                    drop_key(st, "holdMs");
                }
            })));
            let b = match s(st, "button") {
                "right" => "right",
                "middle" => "middle",
                _ => "left",
            };
            row = row.child(lab(t("macro.e.buttonL"), step_pick(cx, "mf-btn", i, pairs(&["left", "right", "middle"], "macro.s.b."), b, |st, v| set(st, "button", json!(v)))));
        }
        "camera" => {
            let (dir, amount) = (cam_dir(st), cam_amount(st));
            row = row.child(lab(t("macro.e.dirL"), step_pick(cx, "mf-dir", i, pairs(&["left", "right", "up", "down"], "macro.e.dir."), dir, move |st, v| cam_turn(st, v, amount))));
            row = row.child(lab(t("macro.e.amountL"), step_pick(cx, "mf-amt", i, pairs(&["little", "some", "lot"], "macro.e.amount."), amount, move |st, v| cam_turn(st, dir, v))));
        }
        "scroll" => {
            let up = num(st, "notches") > 0.;
            row = row.child(lab(t("macro.e.dirL"), step_pick(cx, "mf-dir", i, pairs(&["up", "down"], "macro.e.dir."), if up { "up" } else { "down" }, |st, v| {
                let n = (num(st, "notches").abs().round() as i64).max(1);
                set(st, "notches", json!(if v == "up" { n } else { -n }));
            })));
        }
        "waitText" => {
            row = row.children(words_row(root, f, st, i, "words", cx)).child(shows_row(root, st, i, "gone", cx)).child(where_row(root, f, i, st, "region", false, cx));
        }
        "waitStill" => row = row.child(where_row(root, f, i, st, "region", false, cx)),
        "clickImage" => {
            row = row.child(pic_row(root, f, i, st, "image", cx)).child(where_row(root, f, i, st, "region", false, cx));
            let b = if s(st, "button") == "right" { "right" } else { "left" };
            row = row.child(lab(t("macro.e.buttonL"), step_pick(cx, "mf-btn", i, pairs(&["left", "right"], "macro.s.b."), b, |st, v| set(st, "button", json!(v)))));
        }
        "waitImage" => {
            row = row.child(pic_row(root, f, i, st, "image", cx)).child(shows_row(root, st, i, "gone", cx)).child(where_row(root, f, i, st, "region", false, cx));
        }
        "waitColour" => row = row.child(shows_row(root, st, i, "gone", cx)).child(where_row(root, f, i, st, "region", true, cx)),
        "clickColour" => {
            row = row.child(where_row(root, f, i, st, "region", false, cx));
            let how = if has(st, "eachAfterMs") {
                "each"
            } else if num(st, "repeatUntilGoneMs") > 0. {
                "again"
            } else {
                "once"
            };
            let items = vec![("once".into(), t("macro.e.clickOnce")), ("again".into(), t("macro.e.again")), ("each".into(), t("macro.e.clickEach"))];
            row = row.child(lab(t("macro.e.clickHowL"), step_pick(cx, "mf-each", i, items, how, |st, v| {
                let again = st.get("repeatUntilGoneMs").and_then(Value::as_f64).filter(|x| *x > 0.).map_or(json!(1000), |x| json!(x));
                match v {
                    "each" => {
                        set(st, "eachAfterMs", json!(300));
                        set(st, "repeatUntilGoneMs", again);
                    }
                    "again" => {
                        drop_key(st, "eachAfterMs");
                        set(st, "repeatUntilGoneMs", again);
                    }
                    _ => {
                        drop_key(st, "eachAfterMs");
                        drop_key(st, "repeatUntilGoneMs");
                    }
                }
            })));
        }
        "follow" => {
            row = row.child(where_row(root, f, i, st, "region", false, cx));
            let b = match s(st, "button") {
                "right" => "right",
                "key" => "key",
                _ => "left",
            };
            let items = vec![("left".into(), t("macro.s.b.left")), ("right".into(), t("macro.s.b.right")), ("key".into(), t("macro.e.aKey"))];
            row = row.child(lab(t("macro.e.buttonL"), step_pick(cx, "mf-btn", i, items, b, |st, v| {
                set(st, "button", json!(v));
                if v == "key" && s(st, "key").is_empty() {
                    set(st, "key", json!("Space"));
                }
            })));
            if b == "key" {
                let (k, grid) = key_box(root, f, i, "key", false, cx);
                row = row.child(k);
                under.extend(grid.map(IntoElement::into_any_element));
            }
            let edges = st.get("marker").is_some_and(Value::is_object);
            let items = vec![("one".into(), t("macro.e.markerOne")), ("edges".into(), t("macro.e.markerEdges"))];
            row = row.child(lab(t("macro.e.markerIsL"), step_pick(cx, "mf-mk", i, items, if edges { "edges" } else { "one" }, |st, v| {
                let now = st.get("marker").cloned().unwrap_or(json!("#ffffff"));
                let one = if now.is_object() { now.get("left").cloned().unwrap_or(json!("#ffffff")) } else { now.clone() };
                if v == "edges" && !now.is_object() {
                    set(st, "marker", json!({ "left": one, "right": one }));
                } else if v == "one" && now.is_object() {
                    set(st, "marker", one);
                }
            })));
            let mode = if s(st, "mode") == "smooth" { "smooth" } else { "onOff" };
            let items = vec![("onOff".into(), t("macro.e.modeOnOff")), ("smooth".into(), t("macro.e.modeSmooth"))];
            row = row.child(lab(t("macro.e.movesL"), step_pick(cx, "mf-mode", i, items, mode, |st, v| set(st, "mode", json!(v)))));
            let on = st.get("predict").and_then(Value::as_bool) == Some(true);
            under.push(
                Check::new("mf-predict", on)
                    .label(t("macro.e.predict"))
                    .on_toggle(cx.listener(move |r, on: &bool, _, cx| {
                        if let Some(st) = r.auto.macro_form.as_mut().and_then(|f| f.step_mut(i)) {
                            set(st, "predict", json!(*on));
                        }
                        cx.notify();
                    }))
                    .into_any_element(),
            );
            notes.push(t("macro.followNote"));
        }
        "if" | "loopUntil" => {
            let tk = s(&test, "kind").to_string();
            row = row.child(lab(t("macro.e.checkL"), step_pick(cx, "mf-check", i, pairs(&["image", "text", "colour", "var"], "macro.e.check."), &tk, |st, v| {
                if st.get("test").map(|t| s(t, "kind")) != Some(v) {
                    set(st, "test", new_test(v));
                }
            })));
            match tk.as_str() {
                "image" => {
                    row = row.child(pic_row(root, f, i, st, "test.image", cx)).child(shows_row(root, st, i, "test.not", cx)).child(where_row(root, f, i, st, "test.region", false, cx));
                }
                "text" => {
                    row = row.children(words_row(root, f, st, i, "test.words", cx)).child(shows_row(root, st, i, "test.not", cx)).child(where_row(root, f, i, st, "test.region", false, cx));
                }
                "colour" => row = row.child(shows_row(root, st, i, "test.not", cx)).child(where_row(root, f, i, st, "test.region", true, cx)),
                _ => {
                    let (pick_row, new_row) = counter_pick(root, f, i, "test.name", true, cx);
                    row = row.child(pick_row);
                    under.extend(new_row.map(IntoElement::into_any_element));
                    let o = arr(m, "options").iter().find(|o| s(o, "name") == s(&test, "name"));
                    match o.map(|o| s(o, "kind")) {
                        Some("check") => {
                            let on = test.get("value").is_some_and(|v| v.as_bool() == Some(true) || v.as_f64().is_some_and(|x| x != 0.));
                            let items = vec![("1".into(), t("macro.e.switchOn")), ("0".into(), t("macro.e.switchOff"))];
                            row = row.child(lab(t("macro.e.valueL"), step_pick(cx, "mf-sw", i, items, if on { "1" } else { "0" }, |st, v| {
                                if let Some(t) = obj_mut(st, "test") {
                                    set(t, "op", json!("="));
                                    set(t, "value", json!(if v == "1" { 1 } else { 0 }));
                                }
                            })));
                        }
                        Some("choice") => {
                            let items: Vec<(String, String)> = o.map(|o| arr(o, "choices").iter().filter_map(Value::as_str).map(|c| (c.to_string(), c.to_string())).collect()).unwrap_or_default();
                            row = row.child(lab(t("macro.e.valueL"), step_pick(cx, "mf-choice", i, items, s(&test, "value"), |st, v| {
                                if let Some(t) = obj_mut(st, "test") {
                                    set(t, "op", json!("="));
                                    set(t, "value", json!(v));
                                }
                            })));
                        }
                        _ => {
                            let op = Some(s(&test, "op")).filter(|o| !o.is_empty()).unwrap_or(">=").to_string();
                            row = row.child(lab(t("macro.e.opL"), step_pick(cx, "mf-op", i, pairs(&OPS, "macro.e.op."), &op, |st, v| {
                                let word = matches!(v, "has" | "!has");
                                if let Some(t) = obj_mut(st, "test") {
                                    let was_word = t.get("value").is_some_and(Value::is_string) && opt_ref(t.get("value")).is_none();
                                    set(t, "op", json!(v));
                                    if word != was_word {
                                        set(t, "value", if word { json!("") } else { json!(1) });
                                    }
                                }
                            })));
                        }
                    }
                }
            }
            if kind == "loopUntil" {
                let none = num(st, "max") <= 0.;
                under.push(
                    Check::new("mf-nolimit", none)
                        .label(t("macro.tune.noLimit"))
                        .on_toggle(cx.listener(move |r, on: &bool, window, cx| {
                            if let Some(f) = r.auto.macro_form.as_mut() {
                                if let Some(st) = f.step_mut(i) {
                                    set(st, "max", json!(if *on { 0 } else { 10 }));
                                }
                                fill(f, window, cx);
                            }
                            cx.notify();
                        }))
                        .into_any_element(),
                );
            }
        }
        "loop" => {
            let count = st.get("times").is_some_and(|t| t.is_string() || t.as_f64().is_some_and(|x| x > 0.));
            row = row.child(lab(t("macro.e.loopL"), step_pick(cx, "mf-loop", i, vec![("forever".into(), t("macro.e.loopForever")), ("n".into(), t("macro.e.loopCount"))], if count { "n" } else { "forever" }, |st, v| {
                if v == "forever" {
                    set(st, "times", json!(0));
                } else if !st.get("times").is_some_and(|t| t.is_string() || t.as_f64().is_some_and(|x| x > 0.)) {
                    set(st, "times", json!(10));
                }
            })));
        }
        "setVar" => {
            let (pick_row, new_row) = counter_pick(root, f, i, "name", false, cx);
            row = row.child(pick_row);
            under.extend(new_row.map(IntoElement::into_any_element));
            let is_text = st.get("text").is_some_and(Value::is_string);
            row = row.child(lab(t("macro.e.asL"), step_pick(cx, "mf-as", i, pairs(&["num", "text"], "macro.e.as."), if is_text { "text" } else { "num" }, |st, v| {
                drop_key(st, "expr");
                if v == "text" {
                    drop_key(st, "value");
                    if !st.get("text").is_some_and(Value::is_string) {
                        set(st, "text", json!(""));
                    }
                } else {
                    drop_key(st, "text");
                    if !has(st, "value") {
                        set(st, "value", json!(0));
                    }
                }
            })));
            notes.push(if is_text { root.i18n.tv("macro.varsNote", &[("x", "{coins}")]) } else { t("macro.formulaNote") });
        }
        "addVar" => {
            let (pick_row, new_row) = counter_pick(root, f, i, "name", false, cx);
            row = row.child(pick_row);
            under.extend(new_row.map(IntoElement::into_any_element));
            notes.push(t("macro.formulaNote"));
        }
        "read" => {
            let (pick_row, new_row) = counter_pick(root, f, i, "name", false, cx);
            row = row.child(pick_row);
            under.extend(new_row.map(IntoElement::into_any_element));
            let mode = if s(st, "mode") == "text" { "text" } else { "number" };
            row = row.child(lab(t("macro.e.readWhatL"), step_pick(cx, "mf-mode", i, pairs(&["number", "text"], "macro.e.read."), mode, |st, v| set(st, "mode", json!(v)))));
            let from = Some(s(st, "from")).filter(|x| !x.is_empty()).unwrap_or("screen").to_string();
            row = row.child(lab(t("macro.e.placeL"), step_pick(cx, "mf-from", i, pairs(&["screen", "clipboard", "file", "web"], "macro.e.place."), &from, |st, v| place_set(st, "from", v))));
            if from == "screen" {
                row = row.child(where_row(root, f, i, st, "region", false, cx));
            }
            notes.push(t(match from.as_str() {
                "screen" if mode == "text" => "macro.readTextNote",
                "screen" => "macro.readNote",
                "file" => "macro.fileNote",
                "web" => "macro.webReadNote",
                _ => "macro.clipNote",
            }));
        }
        "write" => {
            let to = Some(s(st, "to")).filter(|x| !x.is_empty()).unwrap_or("file").to_string();
            row = row.child(lab(t("macro.e.placeL"), step_pick(cx, "mf-to", i, pairs(&["clipboard", "file", "web"], "macro.e.place."), &to, |st, v| place_set(st, "to", v))));
            notes.push(root.i18n.tv("macro.varsNote", &[("x", "{coins}")]));
            if to != "clipboard" {
                notes.push(t(if to == "file" { "macro.fileNote" } else { "macro.webNote" }));
            }
        }
        "call" => {
            let cur = s(st, "macroId").to_string();
            if others.is_empty() {
                notes.push(t("macro.emptyTitle"));
            } else {
                row = row.child(lab(t("macro.e.callL"), step_pick(cx, "mf-call", i, others.to_vec(), &cur, |st, v| set(st, "macroId", json!(v)))));
            }
            notes.push(t("macro.callNote"));
        }
        "alert" => {
            let on = st.get("picture").and_then(Value::as_bool) == Some(true);
            under.push(
                Check::new("mf-alert-pic", on)
                    .label(t("macro.e.alertPic"))
                    .on_toggle(cx.listener(move |r, on: &bool, _, cx| {
                        if let Some(st) = r.auto.macro_form.as_mut().and_then(|f| f.step_mut(i)) {
                            set(st, "picture", json!(*on));
                        }
                        cx.notify();
                    }))
                    .into_any_element(),
            );
            notes.push(t("macro.alertNote"));
        }
        _ => {}
    }
    // The boxes, each with its unit; a colour with its swatch and Pick on game; a point with Pick on game.
    let sp = specs(st, m);
    // A point's X waits for its Y: the two boxes and Pick on game stay one group, so a wrap never splits them.
    let mut point_x: Option<Div> = None;
    for (n, (path, label, bx)) in sp.iter().copied().enumerate() {
        let Some(b) = f.boxes.get(n) else { continue };
        let label = if label.starts_with("macro.") { t(label) } else { label.to_string() };
        let width = match (bx, path) {
            (Bx::Text, "fileName" | "name" | "test.name") => 180.,
            (Bx::Text, _) => 300.,
            (Bx::Colour, _) => 116.,
            _ => 84.,
        };
        let unit = match bx {
            Bx::Secs => Some(t("macro.every.s")),
            Bx::Pct if !matches!(path, "x" | "y" | "test.x" | "test.y") => Some("%".to_string()),
            Bx::Close => Some("%".to_string()),
            _ => None,
        };
        let shown_hex = if bx == Bx::Colour { val(m, get(st, path)).as_str().unwrap_or_default().to_string() } else { String::new() };
        let mut line = div().flex().items_center().gap(px(8.));
        if bx == Bx::Colour {
            line = line.child(swatch(&shown_hex, 26.));
        }
        line = line.child(field(b, None).h(px(36.)).w(px(width))).when_some(unit, |d, u| d.child(div().text_size(px(T_S)).text_color(c(MUTED)).child(u)));
        if bx == Bx::Colour {
            line = line.child(
                Button::new(("mf-pick-c", n), t("macro.e.pick"))
                    .icon("mouse")
                    .small()
                    .tip(t(if win { "macro.e.pickTip" } else { "macro.need.game" }))
                    .disabled(!win || f.picking)
                    .on_click(cx.listener(move |r, _, window, cx| pick_on_game(r, i, Some(path), window, cx))),
            );
        }
        if matches!(path, "x" | "test.x") {
            point_x = Some(lab(label, line));
            continue;
        }
        if matches!(path, "y" | "test.y") {
            line = line.child(Button::new("mf-pick-p", t("macro.fix.pick")).icon("mouse").small().tip(t(if win { "macro.e.pickPoint" } else { "macro.need.game" })).disabled(!win || f.picking).on_click(cx.listener(move |r, _, window, cx| pick_on_game(r, i, None, window, cx))));
            row = row.child(div().flex().items_end().gap(px(12.)).children(point_x.take()).child(lab(label, line)));
            continue;
        }
        row = row.child(lab(label, line));
    }
    row = row.children(point_x);
    // To another list of steps (the main ones, a feature's), at its end: an If or a Repeat with its block.
    let feats = arr(m, "features");
    if !feats.is_empty() && !matches!(kind.as_str(), "else" | "recover") {
        let mut lists: Vec<(Option<String>, String)> = vec![(None, t("macro.feat.main"))];
        lists.extend(feats_in_order(m).into_iter().map(|fe| (Some(s(fe, "id").to_string()), feat_label(root, m, fe))));
        let mut l = div().flex().flex_wrap().gap(px(6.));
        for (k, (key, label)) in lists.into_iter().filter(|(key, _)| *key != f.sec).enumerate() {
            let moved = root.i18n.tv("macro.feat.moved", &[("name", &label)]);
            l = l.child(pill(("mf-move-list", k), false, true).gap(px(6.)).child(icon("chevronRight", 13., c(TEXT_2))).child(label).on_click(cx.listener(move |r, _, window, cx| {
                if let Some(f) = r.auto.macro_form.as_mut() {
                    f.move_to_list(i, key.clone());
                    fill(f, window, cx);
                }
                r.toast = Some((moved.clone(), "info".into()));
                push_game(r);
                cx.notify();
            })));
        }
        under.push(lab(t("macro.feat.moveTo"), l).into_any_element());
    }
    // The values the player may change on the panel.
    let can = can_hand(st);
    let ticks = (!can.is_empty()).then(|| {
        let mut l = div().flex().flex_wrap().items_center().gap_x(px(18.)).gap_y(px(8.)).child(div().text_size(px(T_S)).text_color(c(MUTED)).child(t("macro.opt.can")));
        for (k, path) in can.into_iter().enumerate() {
            let raw = get(st, if path == "point" { "x" } else { path });
            let o = opt_at(m, raw).and_then(|oi| arr(m, "options").get(oi));
            let first = meta(&kind, path).map(|x| t(x.3)).unwrap_or_default();
            let label = o.map(|o| s(o, "label").to_string()).filter(|l| !l.is_empty()).unwrap_or_else(|| first.clone());
            let full = root.i18n.tv("macro.opt.full", &[("n", &MAX_OPTIONS.to_string())]);
            l = l.child(Check::new(("mf-tick", k), o.is_some()).label(label).on_toggle(cx.listener(move |r, on: &bool, window, cx| {
                if let Some(f) = r.auto.macro_form.as_mut() {
                    if let Err(e) = f.tick(i, path, *on, &first, full.clone()) {
                        f.error = Some(e);
                    }
                    fill(f, window, cx);
                }
                push_game(r);
                cx.notify();
            })));
        }
        l
    });
    let note_row = lab(t("macro.note.label"), field(&f.note, None).h(px(34.)).w(px(300.)));
    // What is wrong with it, each with its fix when it has one (an area to choose: MacroEditor.tsx's fixOf "area").
    let wrong: Vec<Div> = probs
        .iter()
        .enumerate()
        .map(|(k, p)| {
            let line = div().flex().items_center().gap(px(10.)).child(div().flex_1().min_w_0().child(bad_line(p.bare.clone(), p.warn)));
            match p.fix {
                Some("area") if !p.warn => line.child(Button::new(("mf-fix-area", k), t("macro.fix.area")).small().on_click(cx.listener(move |r, _, window, cx| fix_area(r, i, window, cx)))),
                _ => line,
            }
        })
        .collect();
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .children(wrong)
        .child(row)
        .children(under)
        .children(notes.into_iter().map(small_note))
        .children(ticks)
        .child(note_row)
}

/// Where a Read step reads, or a Write step writes; a file or a web address gets its first name.
fn place_set(st: &mut Value, field: &str, v: &str) {
    set(st, field, json!(v));
    if v == "file" {
        if s(st, "fileName").is_empty() {
            set(st, "fileName", json!("Bloxkeep log"));
        }
    } else {
        drop_key(st, "fileName");
    }
    if v == "web" {
        if s(st, "url").is_empty() {
            set(st, "url", json!("https://"));
        }
    } else {
        drop_key(st, "url");
    }
    if field == "from" && v != "screen" {
        drop_key(st, "region");
    }
}

/// The still of the game with each step's dot and each area's box (MacroPicture's GameFrame).
fn frame(root: &Root, f: &Form, w: f32, cx: &mut Context<Root>) -> AnyElement {
    let m = &f.m;
    let shown = f.seen.as_ref().map(|x| &x.pic).or(f.picture.as_ref());
    let recorded = m.get("recordedSize").map(|r| (num(r, "w") as f32, num(r, "h") as f32)).filter(|(a, b)| *a > 0. && *b > 0.);
    let aspect = shown.map(|p| p.w / p.h).or(recorded.map(|(a, b)| a / b)).filter(|a| a.is_finite() && *a > 0.2 && *a < 5.).unwrap_or(16. / 9.);
    let h = (w / aspect).round();
    let bounds = f.frame.clone();
    // No still yet: Take picture is in the box, with what it will show (the points and areas the steps use).
    let can_take = shown.is_none() && window_of(root, f).is_some() && !f.taking;
    let dots = f.steps().iter().filter(|st| point_of(st, m).is_some()).count();
    let areas = arr(m, "areas").len();
    let waits = (dots + areas > 0).then(|| root.i18n.tv("macro.pic.waits", &[("n", &dots.to_string()), ("a", &areas.to_string())]));
    // No still yet: the box only as tall as what it says (a game-sized empty box was 400 px of nothing, review 5.5).
    let h = if shown.is_none() { h.min(250.) } else { h };
    let mut d = div()
        .id("mf-pic")
        .relative()
        .w(px(w))
        .h(px(h))
        .flex_none()
        .rounded(px(R_M))
        .overflow_hidden()
        .bg(c(INSET))
        .border_1()
        .border_color(if f.cutting.is_some() { ca(FACE, 0.7) } else { c(LINE) })
        .child(canvas(move |b, _, _| bounds.set(Some(b)), |_, _, _, _| {}).absolute().inset_0());
    d = match shown {
        Some(p) => d.child(img(p.image.clone()).absolute().inset_0().size_full().object_fit(ObjectFit::Fill)),
        None => d.child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(10.))
                .bg(ca(WHITE, 0.02))
                .child(div().size(px(44.)).rounded(px(R_M)).bg(ca(WHITE, 0.05)).border_1().border_color(c(LINE)).flex().items_center().justify_center().child(icon("image", 20., c(MUTED))))
                .child(div().px(px(24.)).max_w(px(340.)).text_center().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).text_color(c(TEXT)).child(root.tr("macro.pic.blank")))
                .child(div().px(px(24.)).max_w(px(340.)).text_center().text_size(px(T_S)).line_height(px(17.)).text_color(c(MUTED)).child(root.tr("macro.pic.why")))
                .children(waits.map(|w| div().text_size(px(T_S)).text_color(c(TEXT_2)).font_features(tnum()).child(w)))
                .child(
                    div()
                        .mt(px(4.))
                        .flex()
                        .gap(px(8.))
                        .child(Button::new("mf-take", root.tr("macro.pic.take")).icon("image").small().disabled(!can_take).on_click(cx.listener(|r, _, window, cx| take_picture(r, window, cx))))
                        // No window at all: the way to open one (MacroPicture.tsx's goAccounts).
                        .when(window_of(root, f).is_none(), |d| d.child(Button::new("mf-go-acc", root.tr("macro.goAccounts")).small().quiet().on_click(cx.listener(|r, _, window, cx| go_accounts(r, window, cx))))),
                ),
        ),
    };
    // No still yet: the empty box alone. The dots and boxes are places on a picture; without it they float on nothing
    // (a lone "4" in the dark).
    if shown.is_none() {
        return d.into_any_element();
    }
    let live = f.drag.as_ref().map(|x| (x.kind.clone(), moved(x)));
    let at = |r: [f32; 4]| (r[0] * w, r[1] * h, r[2] * w, r[3] * h);
    let open_area = f.open.and_then(|i| f.steps().get(i)).and_then(|st| region_path(st).and_then(|p| area_ref(get(st, p)))).map(str::to_string);
    for a in arr(m, "areas") {
        let name = s(a, "name");
        let r = match &live {
            Some((DragKind::Move(n) | DragKind::Size(n), r)) if n == name => *r,
            _ => area_rect(a),
        };
        let (x, y, bw, bh) = at(r);
        let tone = hex_c(s(a, "colour"));
        let mine = open_area.as_deref() == Some(name);
        d = d.child(
            div()
                .absolute()
                .left(px(x))
                .top(px(y))
                .w(px(bw))
                .h(px(bh))
                .border_2()
                .border_color(tone)
                .bg(if mine { tone.opacity(0.16) } else { tone.opacity(0.06) })
                .child(div().absolute().top(px(-1.)).left(px(-1.)).px(px(5.)).py(px(1.)).bg(tone).text_color(c(INK)).text_size(px(T_XS)).font_weight(FontWeight::SEMIBOLD).child(s(a, "label").to_string()))
                .child(div().absolute().right(px(-5.)).bottom(px(-5.)).size(px(10.)).rounded(px(2.)).bg(tone)),
        );
    }
    if let Some(seen) = f.seen.as_ref() {
        for b in &seen.boxes {
            let (x, y, bw, bh) = at(*b);
            d = d.child(div().absolute().left(px(x)).top(px(y)).w(px(bw)).h(px(bh)).border_2().border_color(c(OK)));
        }
    }
    for (i, st) in f.steps().iter().enumerate() {
        let Some((px_, py_)) = point_of(st, m) else { continue };
        let (x, y) = match &live {
            Some((DragKind::Dot(k), r)) if *k == i => (r[0], r[1]),
            _ => (px_, py_),
        };
        let open = f.open == Some(i);
        d = d.child(
            div()
                .absolute()
                .left(px(x * w - 11.))
                .top(px(y * h - 11.))
                .size(px(22.))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(c(FACE))
                .border_2()
                .border_color(if open { c(WHITE) } else { c(INK) })
                .text_color(c(INK))
                .text_size(px(T_XS))
                .font_weight(FontWeight::BOLD)
                .child((i + 1).to_string()),
        );
    }
    if let Some((DragKind::Cut | DragKind::Draw(_), r)) = &live {
        let (x, y, bw, bh) = at(*r);
        d = d.child(div().absolute().left(px(x)).top(px(y)).w(px(bw)).h(px(bh)).border_2().border_color(c(FACE)).bg(ca(FACE, 0.12)));
    }
    if shown.is_some() {
        d = d
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, cx.listener(|r, e: &MouseDownEvent, _, cx| pic_down(r, e.position, cx)))
            .on_mouse_move(cx.listener(|r, e: &MouseMoveEvent, _, cx| pic_move(r, e.position, cx)))
            .on_mouse_up(MouseButton::Left, cx.listener(|r, e: &MouseUpEvent, window, cx| pic_up(r, e.position, window, cx)))
            .on_mouse_up_out(MouseButton::Left, cx.listener(|r, e: &MouseUpEvent, window, cx| pic_up(r, e.position, window, cx)));
    }
    d.into_any_element()
}

/// The game: whose window, the still with the dots and boxes, Edit on game, Try it's outcome, the areas.
fn game_card(root: &Root, f: &Form, pic_w: f32, run: Option<&Value>, cx: &mut Context<Root>) -> Div {
    let t = |k: &str| root.tr(k);
    let wins = windows(root);
    let win = window_of(root, f);
    let wname = win.as_ref().map(|w| w.1.clone()).unwrap_or_default();
    let head = section_head(t("macro.game.title"), None)
        .child(
            Button::new("mf-ongame", t(if f.on_game { "macro.game.done" } else { "macro.game.edit" }))
                .icon(if f.on_game { "check" } else { "screens" })
                .small()
                .disabled(win.is_none())
                .on_click(cx.listener(|r, _, _, cx| {
                    let on = match r.auto.macro_form.as_mut() {
                        Some(f) => {
                            f.on_game = !f.on_game;
                            f.on_game
                        }
                        None => return,
                    };
                    if on {
                        push_game(r);
                    } else {
                        r.send("editOnGame", json!({ "accountId": null }));
                    }
                    cx.notify();
                })),
        );
    let mut body = card().gap(px(12.)).child(head);
    if wins.len() > 1 {
        let cur = win.as_ref().map(|w| w.0.clone()).unwrap_or_default();
        let items = wins.iter().map(|(id, l)| (id.clone(), l.clone(), true)).collect();
        body = body.child(lab(t("macro.pic.account"), pick(cx, "mf-win", items, &cur, |form, v| form.account = v.to_string())));
    }
    let say = if let Some(x) = f.say.clone() {
        x
    } else if f.cutting.is_some() {
        t(if f.picture.is_some() || f.seen.is_some() { "macro.pic.cutHow" } else { "macro.pic.cutNeedsPic" })
    } else if win.is_none() {
        t("macro.game.noWindow")
    } else if f.on_game {
        root.i18n.tv("macro.game.editing", &[("name", &wname)])
    } else {
        root.i18n.tv("macro.game.how", &[("name", &wname)])
    };
    body = body.child(small_note(say)).child(frame(root, f, pic_w, cx));
    let has_pic = f.picture.is_some();
    let size = f.picture.as_ref().map(|p| root.i18n.tv("macro.pic.size", &[("w", &trim(p.w as f64)), ("h", &trim(p.h as f64))])).unwrap_or_else(|| t("macro.pic.why"));
    body = body.when(has_pic || f.cutting.is_some() || f.seen.is_some(), |b| b.child(
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .when(has_pic || f.seen.is_some(), |d| d.child(Button::new("mf-take", t(if has_pic { "macro.pic.again" } else { "macro.pic.take" })).icon("image").small().disabled(win.is_none() || f.taking).on_click(cx.listener(|r, _, window, cx| take_picture(r, window, cx)))))
            .when(f.cutting.is_some(), |d| {
                d.child(Button::new("mf-cut-x", t("macro.pic.cancel")).quiet().small().on_click(edit(cx, |form| {
                    form.cutting = None;
                    form.say = None;
                })))
            })
            .child(div().flex_1().min_w_0().text_size(px(T_S)).text_color(c(MUTED)).child(size)),
    ));
    // A window of another shape than the one the macro was made on: its points land elsewhere.
    if let (Some(p), Some(r)) = (f.picture.as_ref(), f.m.get("recordedSize")) {
        let (rw, rh) = (num(r, "w") as f32, num(r, "h") as f32);
        if rw > 0. && rh > 0. && ((p.w / p.h) / (rw / rh) - 1.).abs() > 0.05 {
            let a = format!("{} × {}", trim(p.w as f64), trim(p.h as f64));
            let b = format!("{} × {}", trim(rw as f64), trim(rh as f64));
            body = body.child(bad_line(root.i18n.tv("macro.pic.aspect", &[("a", &a), ("b", &b)]), true));
        }
    }
    // Try it: running, or how it ended.
    if let (Some(run), Some(_)) = (run, f.trial.as_ref()) {
        let (ok, words) = if live(run) {
            (true, root.i18n.tv("macro.try.running", &[("name", &wname)]))
        } else if s(run, "status") == "done" {
            (true, root.i18n.tv("macro.try.end.done", &[("name", &wname)]))
        } else {
            (false, root.i18n.tv("macro.try.end.stopped", &[("name", &wname)]))
        };
        let tone = if ok { c(OK) } else { c(BAD) };
        // A line a step (MacroPicture.tsx's TryLines: "Step 2 found it: …", "Step 3 didn't see it yet: …"), then
        // how the run is doing or ended.
        let said = |ok: bool, mark: &'static str, words: String| {
            let tone = if ok { c(OK) } else { c(BAD) };
            div().flex().items_start().gap(px(8.)).text_size(px(T_M)).line_height(px(19.)).text_color(c(TEXT_2)).child(icon(mark, 14., tone).mt(px(2.))).child(div().flex_1().min_w_0().child(words))
        };
        let mut list = div().flex().flex_col().gap(px(6.));
        for l in f.lines.borrow().iter() {
            list = list.child(said(l.ok, if l.ok { "check" } else { "alert" }, l.text.clone()));
        }
        list = list.child(
            div().flex().items_start().gap(px(8.)).text_size(px(T_M)).line_height(px(19.)).text_color(c(TEXT_2)).child(icon(if live(run) { "clock" } else if ok { "check" } else { "alert" }, 14., tone).mt(px(2.))).child(div().flex_1().min_w_0().child(words)),
        );
        body = body.child(section(t("macro.try.title"), list));
    }
    // The areas: each one's colour, its name (what the steps say: "in Fish area"), and taking it away.
    let areas = arr(&f.m, "areas");
    let mut list = div().flex().flex_col().gap(px(8.));
    if areas.is_empty() {
        list = list.child(small_note(t("macro.area.none")));
    }
    for (k, a) in areas.iter().enumerate() {
        let name = s(a, "name").to_string();
        let Some((_, b)) = f.area_boxes.iter().find(|(n, _)| *n == name) else { continue };
        list = list.child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(div().size(px(14.)).flex_none().rounded(px(3.)).border_2().border_color(hex_c(s(a, "colour"))))
                .child(field(b, None).h(px(32.)).flex_1().min_w_0())
                .child(IconButton::new(("mf-area-del", k), "trash").size(30.).tip(t("macro.area.del")).on_click(cx.listener(move |r, _, window, cx| {
                    let res = r.auto.macro_form.as_mut().map(|f| f.del_area(&name));
                    let err = match res {
                        Some(Err(by)) => Some(used_text(&r.i18n, &by)),
                        _ => None,
                    };
                    if let Some(f) = r.auto.macro_form.as_mut() {
                        f.error = err;
                        fill(f, window, cx);
                    }
                    push_game(r);
                    cx.notify();
                }))),
        );
    }
    let full = root.i18n.tv("macro.area.full", &[("n", &MAX_AREAS.to_string())]);
    body.child(section(
        t("macro.area.title"),
        div().flex().flex_col().gap(px(10.)).child(list).child(div().flex().child(Button::new("mf-area-add", t("macro.area.new")).icon("plus").small().disabled(areas.len() >= MAX_AREAS).on_click(edit(cx, move |form| {
            if form.new_area([0.35, 0.35, 0.3, 0.3]).is_none() {
                form.error = Some(full.clone());
            }
        })))),
    ))
}

/// "Used by steps 2 and 5".
fn used_text(i18n: &I18n, by: &[String]) -> String {
    let list = by.join(", ");
    i18n.tn("macro.ref.used", by.len() as i64, &[("list", &list)])
}

/// What the player sees: the panel's status line with its counters, its tabs (the shown one named in place), the
/// settings on the tab shown (named here, moved within the tab or to another, taken off), switches, features, choices,
/// lists, headings and tabs added, what the counters are called, and whether the panel shows by itself in a game. A
/// feature's switch links to its steps; the switches' order is the order the features play in.
fn player_card(root: &Root, f: &Form, cx: &mut Context<Root>) -> Div {
    let t = |k: &str| root.tr(k);
    let m = &f.m;
    let used = refs(m);
    let tabs = f.tabs();
    let cur = f.cur_tab();
    let tab_of = |o: &Value| -> Option<String> {
        let x = s(o, "tab");
        if tabs.is_empty() {
            None
        } else if tabs.iter().any(|y| y == x) {
            Some(x.to_string())
        } else {
            tabs.first().cloned()
        }
    };
    let all: Vec<&Value> = arr(m, "options").iter().filter(|o| FREE.contains(&s(o, "kind")) || used.contains(s(o, "name"))).collect();
    let opts: Vec<&Value> = all.iter().copied().filter(|o| tabs.is_empty() || tab_of(o) == cur).collect();
    let feats = arr(m, "features");

    // The status line as the panel shows it: Ready, then each counter by what the panel calls it.
    let counters = f.counters();
    let status = std::iter::once(t("macro.panel.idle")).chain(counters.iter().map(|n| format!("{}: 0", counter_name(m, n)))).collect::<Vec<_>>().join(" · ");
    let status_line = div().text_size(px(T_S)).text_color(c(MUTED)).font_features(tnum()).child(status);

    // The tabs: the shown one's name typed in place and its ×, the others to pick, + for one more.
    let tab_row = (!tabs.is_empty()).then(|| {
        let mut row = div().flex().flex_wrap().items_center().gap(px(6.));
        for (k, x) in tabs.iter().enumerate() {
            if cur.as_ref() == Some(x) {
                let gone = x.clone();
                row = row.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(2.))
                        .child(field(&f.tab_name, None).h(px(30.)).w(px(130.)))
                        .child(IconButton::new(("mf-tab-x", k), "x").size(28.).tip(t("macro.tab.remove")).on_click(edit(cx, move |form| form.remove_tab(&gone)))),
                );
            } else {
                let to = x.clone();
                row = row.child(pill(("mf-tab", k), false, true).child(x.clone()).on_click(edit(cx, move |form| {
                    form.tab_sel = Some(to.clone());
                    form.tab_menu = None;
                })));
            }
        }
        if tabs.len() < MAX_TABS {
            let (first, nth, full) = (t("macro.tab.first"), t("macro.tab.n"), root.i18n.tv("macro.tab.full", &[("n", &MAX_TABS.to_string())]));
            row = row.child(pill("mf-tab-add", false, true).child(icon("plus", 13., c(TEXT_2))).on_click(edit(cx, move |form| form.add_tab(&first, &nth, full.clone()))));
        }
        row
    });

    let mut list = div().flex().flex_col().gap(px(8.));
    if opts.is_empty() {
        list = list.child(small_note(t(if tabs.is_empty() { "macro.panel.none" } else { "macro.tab.empty" })));
    }
    let count = opts.len();
    for (k, o) in opts.iter().enumerate() {
        let name = s(o, "name").to_string();
        let Some((_, label)) = f.opt_boxes.iter().find(|(n, _)| *n == name) else { continue };
        let kind = s(o, "kind");
        let v = o.get("value").unwrap_or(&Value::Null);
        let feat = feats.iter().find(|fe| s(fe, "option") == name);
        let value: AnyElement = match kind {
            "colour" => div().flex().items_center().gap(px(6.)).child(swatch(v.as_str().unwrap_or_default(), 18.)).child(v.as_str().unwrap_or_default().to_string()).into_any_element(),
            "check" => {
                let on_off = t(if v.as_bool() == Some(true) || v.as_f64().is_some_and(|x| x != 0.) { "macro.feat.isOn" } else { "macro.feat.isOff" });
                match feat {
                    // A feature's switch: its steps, a click away.
                    Some(fe) => {
                        let id = s(fe, "id").to_string();
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .child(
                                hot(("mf-opt-feat", k))
                                    .h(px(28.))
                                    .px(px(10.))
                                    .rounded(px(R_S))
                                    .flex()
                                    .items_center()
                                    .gap(px(4.))
                                    .tones(ca(WHITE, 0.05), ca(WHITE, 0.1))
                                    .cursor_pointer()
                                    .text_color(c(TEXT))
                                    .child(root.i18n.tn("macro.feat.steps", arr(fe, "steps").len() as i64, &[]))
                                    .child(icon("chevronRight", 13., c(TEXT_2)))
                                    .tooltip({
                                        let tip: SharedString = root.tr("macro.feat.open").into();
                                        move |_, cx| cx.new(|_| Tip(tip.clone())).into()
                                    })
                                    .on_click(edit(cx, move |form| form.switch_to(Some(id.clone())))),
                            )
                            .child(on_off)
                            .into_any_element()
                    }
                    None => on_off.into_any_element(),
                }
            }
            "key" => key_name(root, v.as_str().unwrap_or_default()).into_any_element(),
            "point" => format!("{} %, {} %", trim(num(v, "x") * 100.), trim(num(v, "y") * 100.)).into_any_element(),
            "number" if matches!(s(o, "unit"), "s" | "ms") => ms_text(root, m, Some(v)).into_any_element(),
            "number" => format!("{} {}", said(v), s(o, "unit")).trim().to_string().into_any_element(),
            "list" => root.i18n.tv("macro.opt.ticked", &[("n", &arr(o, "value").len().to_string()), ("all", &arr(o, "choices").len().to_string())]).into_any_element(),
            _ => div().into_any_element(),
        };
        let choices = f.choice_boxes.iter().find(|(n, _)| *n == name).map(|(_, b)| {
            let tip: SharedString = root.tr("macro.opt.choicesTip").into();
            div().id(SharedString::from(format!("mf-choices-{name}"))).child(field(b, None).h(px(32.)).w(px(200.))).tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
        });
        let (n_up, n_down, n_del, n_tab) = (name.clone(), name.clone(), name.clone(), name.clone());
        let menu_open = f.tab_menu.as_deref() == Some(name.as_str());
        list = list.child(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(field(label, None).h(px(32.)).w(px(190.)).when(kind == "heading", |d| d.font_weight(FontWeight::SEMIBOLD)))
                .child(div().flex_1().min_w_0().text_size(px(T_M)).text_color(c(TEXT_2)).child(value))
                .children(choices)
                .when(!tabs.is_empty(), |d| {
                    d.child(IconButton::new(("mf-opt-tab", k), if menu_open { "chevronDown" } else { "chevronRight" }).size(28.).tip(t("macro.tab.moveTo")).on_click(edit(cx, move |form| {
                        form.tab_menu = if form.tab_menu.as_deref() == Some(n_tab.as_str()) { None } else { Some(n_tab.clone()) };
                    })))
                })
                .child(if k == 0 {
                    div().w(px(28.)).flex_none().into_any_element()
                } else {
                    IconButton::new(("mf-opt-up", k), "chevronUp").size(28.).tip(t("lo.up")).on_click(edit(cx, move |form| form.move_opt(&n_up, -1))).into_any_element()
                })
                .child(if k + 1 >= count {
                    div().w(px(28.)).flex_none().into_any_element()
                } else {
                    IconButton::new(("mf-opt-down", k), "chevronDown").size(28.).tip(t("lo.down")).on_click(edit(cx, move |form| form.move_opt(&n_down, 1))).into_any_element()
                })
                .child(IconButton::new(("mf-opt-x", k), "x").size(28.).tip(t("macro.panel.remove")).on_click(cx.listener(move |r, _, window, cx| remove_row(r, &n_del, window, cx)))),
        );
        // Move to tab: the other tabs, in a row under it.
        if menu_open {
            let mine = tab_of(o);
            let mut row = div().ml(px(200.)).flex().flex_wrap().items_center().gap(px(6.)).child(div().text_size(px(T_S)).text_color(c(MUTED)).child(t("macro.tab.moveTo")));
            for (j, x) in tabs.iter().enumerate().filter(|(_, x)| mine.as_ref() != Some(*x)) {
                let (n, to) = (name.clone(), x.clone());
                row = row.child(pill(("mf-opt-tab-to", k * 8 + j), false, true).gap(px(6.)).child(icon("chevronRight", 12., c(TEXT_2))).child(x.clone()).on_click(edit(cx, move |form| form.move_to_tab(&n, &to))));
            }
            list = list.child(row);
        }
    }

    // Add to panel: one button, its kinds in a menu under it (a switch, a feature with steps of its own, a choice, a
    // list, a heading; a tab while there are none).
    let room = all.len() < MAX_OPTIONS;
    let add_open = f.add_menu;
    let add_button = Button::new("mf-opt-add", t("au.addPanel")).icon("plus").icon_end(if add_open { "chevronUp" } else { "chevronDown" }).small().disabled(!room && !tabs.is_empty()).on_click(cx.listener(|r, _, _, cx| {
        if let Some(f) = r.auto.macro_form.as_mut() {
            let just = f.add_closed.is_some_and(|at| at.elapsed().as_millis() < 300);
            f.add_closed = None;
            f.add_menu = !just && !f.add_menu;
        }
        cx.notify();
    }));
    let mut add_wrap = div().relative().flex_none().child(add_button);
    if add_open {
        let mut menu = div()
            .id("mf-opt-add-menu")
            .occlude()
            .min_w(px(240.))
            .p(px(4.))
            .rounded(px(R_M))
            .bg(c(RAISED))
            .border_1()
            .border_color(c(LINE))
            .flex()
            .flex_col()
            .on_mouse_down_out(cx.listener(|r, _, _, cx| {
                if let Some(f) = r.auto.macro_form.as_mut().filter(|f| f.add_menu) {
                    f.add_menu = false;
                    f.add_closed = Some(std::time::Instant::now());
                    cx.notify();
                }
            }));
        let full = root.i18n.tv("macro.opt.full", &[("n", &MAX_OPTIONS.to_string())]);
        for (k, kind) in ["check", "feature", "choice", "list", "heading"].into_iter().enumerate() {
            let row = menu_row(("mf-opt-add", k))
                .when(!room, |d| d.text_color(c(DIM)).cursor_default().hover(|d| d.text_color(c(DIM))))
                .child(icon(match kind {
                    "feature" => "layers",
                    "check" => "power",
                    "choice" => "sliders",
                    "heading" => "tag",
                    _ => "rows",
                }, 15., if room { c(TEXT_2) } else { c(DIM) }))
                .child(div().flex_1().min_w_0().whitespace_nowrap().child(t(&format!("macro.opt.add.{kind}"))));
            let full = full.clone();
            menu = menu.child(if !room {
                row.into_any_element()
            } else if kind == "feature" {
                let (name, full_feats) = (t("macro.feat.n"), root.i18n.tv("macro.feat.full", &[("n", &MAX_FEATURES.to_string())]));
                row.on_click(edit(cx, move |form| {
                    form.add_menu = false;
                    form.add_feature(&name, full_feats.clone(), full.clone());
                }))
                .into_any_element()
            } else {
                let label = t(&format!("macro.opt.new.{kind}"));
                let words = [root.i18n.tv("macro.opt.choiceN", &[("n", "1")]), root.i18n.tv("macro.opt.choiceN", &[("n", "2")])];
                row.on_click(edit(cx, move |form| {
                    form.add_menu = false;
                    if !form.add_free(kind, &label, words.clone()) {
                        form.error = Some(full.clone());
                    }
                }))
                .into_any_element()
            });
        }
        if tabs.is_empty() {
            let (first, nth, full) = (t("macro.tab.first"), t("macro.tab.n"), root.i18n.tv("macro.tab.full", &[("n", &MAX_TABS.to_string())]));
            menu = menu.child(div().my(px(4.)).mx(px(6.)).h(px(1.)).bg(c(LINE))).child(
                menu_row("mf-opt-add-tab")
                    .child(icon("plus", 15., c(TEXT_2)))
                    .child(div().flex_1().min_w_0().whitespace_nowrap().child(t("macro.tab.add")))
                    .on_click(edit(cx, move |form| {
                        form.add_menu = false;
                        form.add_tab(&first, &nth, full.clone());
                    })),
            );
        }
        add_wrap = add_wrap.child(popup(PopAt::BelowRight(36.), "mf-opt-add-pop", true, menu));
    }

    // What the panel calls each counter (the steps' Set, Add and Read names); empty: the name itself.
    let names = (!f.counter_boxes.is_empty()).then(|| {
        let mut rows = div().flex().flex_col().gap(px(6.));
        for (n, b) in &f.counter_boxes {
            rows = rows.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(div().w(px(130.)).flex_none().truncate().text_size(px(T_M)).text_color(c(TEXT_2)).child(n.clone()))
                    .child(field(b, None).h(px(32.)).w(px(190.))),
            );
        }
        section(t("macro.counter.label"), rows)
    });

    let auto = m.pointer("/panel/autoShow").and_then(Value::as_bool) == Some(true);
    let auto_row = div()
        .flex()
        .items_start()
        .gap(px(12.))
        .child(Switch::new("mf-panel-auto", auto).on_toggle(cx.listener(|r, on: &bool, _, cx| {
            if let Some(p) = r.auto.macro_form.as_mut().and_then(|f| obj_mut(&mut f.m, "panel")) {
                set(p, "autoShow", json!(*on));
            }
            cx.notify();
        })))
        .child(div().flex().flex_col().gap(px(2.)).child(div().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).child(t("macro.panel.auto"))).child(small_note(t("macro.panel.autoDesc"))));

    // The panel as the player sees it: a small window of its own, its status line on top, its tabs, then its rows.
    let preview = div()
        .rounded(px(R_M))
        .bg(c(INSET))
        .border_1()
        .border_color(c(LINE))
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(36.))
                .px(px(14.))
                .flex()
                .items_center()
                .gap(px(8.))
                .border_b_1()
                .border_color(c(LINE))
                .bg(ca(WHITE, 0.025))
                .child(div().size(px(7.)).flex_none().rounded_full().bg(c(OK)))
                .child(div().flex_1().min_w_0().truncate().child(status_line)),
        )
        .children(tab_row.map(|row| div().px(px(12.)).pt(px(12.)).child(row)))
        .child(div().p(px(12.)).child(list));
    card()
        .gap(px(16.))
        .child(div().flex().flex_col().gap(px(2.)).child(section_head(t("macro.panel.title"), Some(all.len())).child(add_wrap)).child(small_note(t("macro.panel.sub"))))
        .child(preview)
        .children(names)
        .child(div().pt(px(14.)).border_t_1().border_color(ca(WHITE, 0.05)).flex().flex_col().gap(px(10.)).child(auto_row).when(auto, |d| d.child(field(&f.place, Some("link")).h(px(36.)))))
}

/// A row's × on the panel: a feature's switch takes its feature with it (asked first when it has steps); a setting a
/// step reads stays, and says which steps.
fn remove_row(r: &mut Root, name: &str, window: &mut Window, cx: &mut Context<Root>) {
    let Some(f) = r.auto.macro_form.as_ref() else { return };
    let feat = arr(&f.m, "features").iter().find(|fe| s(fe, "option") == name).map(|fe| (s(fe, "id").to_string(), arr(fe, "steps").len(), feat_label(r, &f.m, fe)));
    if let Some((id, n, label)) = feat {
        let gone = move |r: &mut Root, cx: &mut Context<Root>| {
            if let Some(f) = r.auto.macro_form.as_mut() {
                f.error = None;
                f.drop_feature(&id);
            }
            push_game(r);
            cx.notify();
        };
        if n == 0 {
            return gone(r, cx);
        }
        let (title, text) = (r.tr("macro.feat.del"), r.i18n.tn("macro.feat.delAsk", n as i64, &[("name", &label)]));
        return r.confirm(title.clone(), text, title, true, None, window, cx, gone);
    }
    let res = r.auto.macro_form.as_mut().map(|f| f.remove_opt(name));
    let err = match res {
        Some(Err(by)) => Some(used_text(&r.i18n, &by)),
        _ => None,
    };
    if let Some(f) = r.auto.macro_form.as_mut() {
        f.error = err;
        fill(f, window, cx);
    }
    push_game(r);
    cx.notify();
}

/// A drag of a step's grip let go.
fn end_step_drag(r: &mut Root, cx: &mut Context<Root>) {
    if let Some(f) = r.auto.macro_form.as_mut() {
        if f.step_drag.take().is_some() {
            cx.notify();
        }
    }
}

/// + Feature: a switch on the panel with steps of its own, shown next. Under the mouse it says so (a stack icon and
/// "Feature" alone gave no hint what it adds, review 5.3).
fn feat_add(root: &Root, id: &'static str, cx: &mut Context<Root>) -> Stateful<Div> {
    let (name, full_feats, full_opts) = (root.tr("macro.feat.n"), root.i18n.tv("macro.feat.full", &[("n", &MAX_FEATURES.to_string())]), root.i18n.tv("macro.opt.full", &[("n", &MAX_OPTIONS.to_string())]));
    let tip: SharedString = root.tr("macro.feat.addTip").into();
    div()
        .id(SharedString::from(format!("{id}-tip")))
        .flex_none()
        .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
        .child(Button::new(id, root.tr("macro.feat.add")).icon("layers").small().quiet().on_click(edit(cx, move |form| form.add_feature(&name, full_feats.clone(), full_opts.clone()))))
}

/// The main steps and each feature's, as tabs over the list: a feature's switch on or off, how many steps, a dot where
/// something stops a save. They play left to right (their switches' order on the panel).
fn feat_tabs(root: &Root, f: &Form, probs: &[Prob], cx: &mut Context<Root>) -> Option<Div> {
    let m = &f.m;
    let feats = feats_in_order(m);
    if feats.is_empty() {
        return None;
    }
    let mut row = div().flex().flex_wrap().items_center().gap(px(6.));
    let lists = std::iter::once((None, root.tr("macro.feat.main"), None, arr(m, "steps").len())).chain(feats.iter().map(|fe| (Some(s(fe, "id").to_string()), feat_label(root, m, fe), Some(feat_on(m, fe)), arr(fe, "steps").len())));
    for (k, (key, name, lit, n)) in lists.enumerate() {
        let on = f.sec == key;
        let bad = probs.iter().any(|p| !p.warn && p.sec == key);
        row = row.child(
            pill(("mf-ftab", k), on, true)
                .gap(px(7.))
                .when_some(lit, |d, lit| d.child(div().size(px(7.)).flex_none().rounded_full().bg(if lit { c(OK) } else if on { ca(INK, 0.3) } else { ca(WHITE, 0.3) })))
                .child(name)
                .child(div().text_size(px(T_XS)).font_features(tnum()).text_color(if on { ca(INK, 0.65) } else { c(MUTED) }).child(n.to_string()))
                .when(bad, |d| d.child(div().size(px(7.)).flex_none().rounded_full().bg(c(BAD))))
                .when(!on, |d| d.on_click(edit(cx, move |form| form.switch_to(key.clone())))),
        );
    }
    if feats.len() < MAX_FEATURES {
        let (name, full_feats, full_opts) = (root.tr("macro.feat.n"), root.i18n.tv("macro.feat.full", &[("n", &MAX_FEATURES.to_string())]), root.i18n.tv("macro.opt.full", &[("n", &MAX_OPTIONS.to_string())]));
        row = row.child(
            pill("mf-ftab-add", false, true)
                .gap(px(6.))
                .child(icon("plus", 13., c(TEXT_2)))
                .child(root.tr("macro.feat.add"))
                .on_click(edit(cx, move |form| form.add_feature(&name, full_feats.clone(), full_opts.clone()))),
        );
    }
    let mut out = div().flex().flex_col().gap(px(6.)).child(row);
    if feats.len() > 1 {
        out = out.child(small_note(root.tr("macro.feat.order")));
    }
    Some(out)
}

/// The feature shown: its name (its switch's label on the panel), on or off, when it plays, what a failure does in it.
fn feat_head(root: &Root, f: &Form, probs: &[Prob], cx: &mut Context<Root>) -> Option<Div> {
    let t = |k: &str| root.tr(k);
    let id = f.sec.clone()?;
    let fe = feat_of(&f.m, &id)?;
    let option = s(fe, "option").to_string();
    let on = feat_on(&f.m, fe);
    let every = fe.get("everyMs").filter(|v| !v.is_null()).is_some();
    let fail = if s(fe, "onFail") == "macro" { "macro" } else { "skip" };
    let opt = option.clone();
    let name_row = div()
        .flex()
        .items_end()
        .gap(px(14.))
        .child(lab(t("macro.feat.name"), field(&f.feat_name, None).h(px(36.)).w(px(240.))))
        .child(
            div()
                .h(px(36.))
                .flex()
                .items_center()
                .gap(px(10.))
                .child(Switch::new("mf-feat-on", on).on_toggle(cx.listener(move |r, on: &bool, _, cx| {
                    if let Some(o) = r.auto.macro_form.as_mut().and_then(|f| arr_mut(&mut f.m, "options")).and_then(|l| l.iter_mut().find(|o| s(o, "name") == opt)) {
                        let v = json!(if *on { 1 } else { 0 });
                        set(o, "value", v.clone());
                        set(o, "default", v);
                    }
                    push_game(r);
                    cx.notify();
                })))
                .child(div().text_size(px(T_M)).text_color(c(TEXT_2)).child(t(if on { "macro.feat.isOn" } else { "macro.feat.isOff" }))),
        )
        .child(div().flex_1())
        .child(IconButton::new("mf-feat-del", "trash").size(34.).tip(t("macro.feat.del")).on_click(cx.listener(move |r, _, window, cx| remove_row(r, &option, window, cx))));
    let when_items = vec![("round".to_string(), t("macro.feat.eachRound"), true), ("every".to_string(), t("macro.feat.everyOpt"), true)];
    let mut when = div().flex().flex_wrap().items_center().gap(px(10.)).child(pick(cx, "mf-feat-when", when_items, if every { "every" } else { "round" }, |form, v| {
        let every = v == "every";
        if let Some(fe) = form.sec.clone().and_then(|id| form.feat_mut(&id)) {
            set(fe, "everyMs", if every { json!(300_000) } else { Value::Null });
        }
        form.feat_every_min = every;
    }));
    if every {
        when = when
            .child(div().text_size(px(T_M)).text_color(c(TEXT_2)).child(t("macro.feat.everyL")))
            .child(field(&f.feat_every, None).h(px(36.)).w(px(84.)))
            .child(pick(cx, "mf-feat-unit", vec![("s".into(), t("macro.every.s"), true), ("min".into(), t("macro.every.min"), true)], if f.feat_every_min { "min" } else { "s" }, |form, v| {
                let min = v == "min";
                if form.feat_every_min == min {
                    return;
                }
                let (old, new) = if min { (1000., 60000.) } else { (60000., 1000.) };
                if let Some(fe) = form.sec.clone().and_then(|id| form.feat_mut(&id)) {
                    if let Some(ms) = fe.get("everyMs").and_then(Value::as_f64) {
                        set(fe, "everyMs", json!((ms / old * new).round() as i64));
                    }
                }
                form.feat_every_min = min;
            }));
    }
    let fail_items = vec![("skip".to_string(), t("macro.feat.fail.skip"), true), ("macro".to_string(), t("macro.feat.fail.macro"), true)];
    let on_fail = pick(cx, "mf-feat-fail", fail_items, fail, |form, v| {
        if let Some(fe) = form.sec.clone().and_then(|id| form.feat_mut(&id)) {
            set(fe, "onFail", json!(v));
        }
    });
    let errs: Vec<&Prob> = probs.iter().filter(|p| p.step.is_none() && p.sec.as_deref() == Some(id.as_str())).collect();
    Some(
        div()
            .p(px(14.))
            .rounded(px(R_M))
            .bg(ca(WHITE, 0.04))
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(name_row)
            .child(section(t("macro.feat.when"), when))
            .child(section(t("macro.feat.onFail"), on_fail))
            .child(small_note(t(if every { "macro.feat.sayEvery" } else { "macro.feat.sayRound" })))
            .children(errs.into_iter().map(|p| bad_line(p.text.clone(), p.warn))),
    )
}

/// A step's ⋮: Duplicate and Delete in a small menu under it (the web editor's row menu).
fn step_more(root: &Root, f: &Form, i: usize, kind: &str, too_many: String, cx: &mut Context<Root>) -> Div {
    let t = |k: &str| root.tr(k);
    let open = f.step_menu == Some(i);
    let no_dup = kind == "else" || kind == "recover";
    let button = IconButton::new(("mf-more", i), "more").size(30.).selected(open).tip(t("card.moreTip")).on_click(cx.listener(move |r, _, _, cx| {
        if let Some(f) = r.auto.macro_form.as_mut() {
            let just = f.step_menu_shut.take().is_some_and(|(k, at)| k == i && at.elapsed().as_millis() < 300);
            f.step_menu = if just || f.step_menu == Some(i) { None } else { Some(i) };
        }
        cx.notify();
    }));
    let mut wrap = div().relative().flex_none().child(button);
    if open {
        let dup = menu_row(("mf-dup", i)).child(icon("copy", 15., c(TEXT_2))).child(div().flex_1().child(t("macro.dupStep")));
        let dup = if no_dup {
            dup.opacity(0.38).cursor_default()
        } else {
            dup.on_click(edit(cx, move |form| {
                form.step_menu = None;
                form.dup_step(i, too_many.clone());
            }))
        };
        // An If or a Repeat with steps in it asks first: they go with it.
        let inner = f.inner_steps(i);
        let what = t(if kind == "if" { "macro.block.if" } else { "macro.block.loop" });
        let ask = (inner > 0).then(|| root.i18n.tn("macro.block.deleteAsk", inner as i64, &[("what", &what)]));
        let del = menu_row(("mf-del", i)).text_color(c(BAD)).hover(|d| d.text_color(c(BAD))).child(icon("trash", 15., c(BAD))).child(div().flex_1().child(t("macro.delStep"))).on_click(cx.listener(move |r, _, window, cx| {
            if let Some(form) = r.auto.macro_form.as_mut() {
                form.step_menu = None;
            }
            // (No step is open after it, so only the lists' boxes would need filling, and deleting steps leaves them.)
            let gone = move |r: &mut Root, cx: &mut Context<Root>| {
                if let Some(form) = r.auto.macro_form.as_mut() {
                    form.error = None;
                    form.del_step(i);
                }
                push_game(r);
                cx.notify();
            };
            match ask.clone() {
                None => gone(r, cx),
                Some(text) => {
                    let (title, ok) = (r.tr("macro.delStep"), r.tr("macro.delete"));
                    r.confirm(title, text, ok, true, None, window, cx, gone);
                }
            }
            cx.notify();
        }));
        let menu = div()
            .id(("mf-step-menu", i))
            .occlude()
            .w(px(200.))
            .p(px(6.))
            .rounded(px(R_L))
            .bg(c(RAISED))
            .border_1()
            .border_color(ca(WHITE, 0.1))
            .flex()
            .flex_col()
            .on_mouse_down_out(cx.listener(move |r, _, _, cx| {
                if let Some(f) = r.auto.macro_form.as_mut().filter(|f| f.step_menu == Some(i)) {
                    f.step_menu = None;
                    f.step_menu_shut = Some((i, std::time::Instant::now()));
                    cx.notify();
                }
            }))
            .child(dup)
            .child(del);
        wrap = wrap.child(popup(PopAt::BelowRight(34.), format!("mf-step-pop-{i}"), true, menu));
    }
    wrap
}

/// Add step's kinds (MacroEditor.tsx's AddList): a search box, the kinds used lately, then every kind in its group;
/// only those that fit where it goes. Words typed show the kinds they find (Enter adds the first), or that none is
/// called so.
fn add_list(root: &Root, f: &Form, fit: &[&'static str], cx: &mut Context<Root>) -> Div {
    let t = |k: &str| root.tr(k);
    let too_many = root.i18n.tv("macro.err.tooMany", &[("max", &MAX_STEPS.to_string())]);
    let mut n = 0usize;
    let mut kinds = |list: &[&'static str], cx: &mut Context<Root>| {
        let mut line = div().flex().flex_wrap().gap(px(6.));
        for &kind in list {
            n += 1;
            let too_many = too_many.clone();
            line = line.child(
                pill(("mf-kind", n), false, true)
                    .gap(px(7.))
                    .child(icon(step_icon(kind), 14., c(TEXT_2)))
                    .child(t(&format!("macro.kind.{kind}")))
                    .on_click(edit(cx, move |form| form.add(kind, too_many.clone()))),
            );
        }
        line
    };
    let row = |label: String, line: Div| div().flex().items_start().gap(px(12.)).child(div().w(px(110.)).flex_none().pt(px(7.)).text_size(px(T_S)).text_color(c(MUTED)).child(label)).child(line.flex_1().min_w_0());
    let mut panel = div().p(px(14.)).rounded(px(R_M)).bg(ca(WHITE, 0.04)).flex().flex_col().gap(px(12.)).child(field(&f.search, Some("search")).h(px(34.)).max_w(px(320.)));
    let q = text(&f.search, cx);
    if !q.trim().is_empty() {
        let found = step_matches(root, &q, fit);
        return if found.is_empty() { panel.child(small_note(t("macro.search.none"))) } else { panel.child(kinds(&found, cx)) };
    }
    let recent: Vec<&'static str> = recent_kinds().into_iter().filter(|k| fit.contains(k)).collect();
    if !recent.is_empty() {
        let line = kinds(&recent, cx);
        panel = panel.child(row(t("macro.recent"), line));
    }
    for (group, list) in GROUPS {
        let here: Vec<&'static str> = list.iter().copied().filter(|k| fit.contains(k)).collect();
        if here.is_empty() {
            continue;
        }
        let line = kinds(&here, cx);
        panel = panel.child(row(t(&format!("macro.grp.{group}")), line));
    }
    panel
}

/// The form's problems, worked out again only when the macro, its name, the saved ones or the language changed.
fn form_problems(root: &Root, f: &Form, saved: &[Value], cx: &App) -> Rc<Vec<Prob>> {
    use std::hash::{Hash, Hasher};
    let name = text(&f.name, cx);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    crate::pages::games::sig_of(&f.m, &mut h);
    saved.iter().for_each(|x| crate::pages::games::sig_of(x, &mut h));
    (&name, root.i18n.vietnamese).hash(&mut h);
    let key = h.finish();
    if let Some((k, p)) = f.probs.borrow().as_ref()
        && *k == key
    {
        return p.clone();
    }
    let p = Rc::new(problems(&root.i18n, &f.m, &name, saved));
    *f.probs.borrow_mut() = Some((key, p.clone()));
    p
}

pub fn form(root: &Root, width: f32, cx: &mut Context<Root>) -> AnyElement {
    let Some(f) = root.auto.macro_form.as_ref() else { return div().into_any_element() };
    let m = &f.m;
    let t = |k: &str| root.tr(k);
    let my_id = s(m, "id").to_string();
    let saved: &[Value] = root.state.pointer("/settings/macros").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    // The other macros: a Run step's choices, and the hotkeys taken.
    let others: Vec<(String, String)> = saved.iter().filter(|o| s(o, "id") != my_id).map(|o| (s(o, "id").to_string(), s(o, "name").to_string())).collect();
    let taken: std::collections::HashMap<i64, String> = saved.iter().filter(|o| s(o, "id") != my_id).filter_map(|o| Some((o.get("hotkey").and_then(Value::as_i64)?, s(o, "name").to_string()))).collect();
    let win = window_of(root, f);
    let wide = width >= 1100.;
    // The game and the settings in a column at the right, wider as the window is; its picture fills it.
    let side_w = ((width - 2. * SIDE) * 0.32).clamp(380., 500.);
    let pic_w = if wide { side_w - 42. } else { (width - 2. * SIDE - 42.).clamp(240., 640.) };

    // What stops a save: on its step, and (once Save or Try was pressed) at the foot.
    let probs = form_problems(root, f, saved, cx);
    let run = trial_run(root, f);
    let running_at = run.filter(|x| live(x)).and_then(|x| x.get("step")).and_then(Value::as_u64).map(|x| x as usize);
    let stopped = run.filter(|x| !live(x) && s(x, "status") == "failed").and_then(|x| Some((x.get("lastStep")?.as_u64()? as usize, s(x, "reason").to_string())));
    let trying = run.is_some_and(live);
    track_trial(root, f, run, &others);

    // Name and Try it.
    let try_tip = match win.as_ref() {
        Some((_, name)) => root.i18n.tv("macro.tryItTip", &[("name", name)]),
        None => t("macro.need.try"),
    };
    let head_row = div()
        .flex()
        .items_end()
        .gap(px(10.))
        .child(lab(t("macro.name"), field(&f.name, None).h(px(38.))).flex_1().min_w_0())
        .child(
            Button::new("mf-try", t(if trying { "macro.tryStop" } else if f.sec.is_some() { "macro.feat.try" } else { "macro.tryIt" }))
                .icon(if trying { "stop" } else { "play" })
                .tip(try_tip)
                .disabled(win.is_none())
                .on_click(cx.listener(|r, _, window, cx| try_it(r, window, cx))),
        );

    // A template to start from (a new macro with no steps yet): what each does and what it needs from the game
    // (MacrosView.tsx's template cards), or that there are none.
    let templates = (f.is_new && f.steps().is_empty()).then(|| f.templates.as_ref()).flatten().map(|list| {
        let mut tiles = div().flex().flex_wrap().gap(px(10.));
        if list.is_empty() {
            tiles = tiles.child(small_note(t("macro.noTemplates")));
        }
        for (k, tp) in list.iter().enumerate() {
            let ic = Some(s(tp, "icon")).filter(|x| ICONS.contains(x)).unwrap_or("play");
            tiles = tiles.child(
                crate::kit::card(("mf-tpl", k))
                    .w(px(250.))
                    .p(px(12.))
                    .flex()
                    .items_start()
                    .gap(px(10.))
                    .cursor_pointer()
                    .child(div().flex_none().child(icon(ic, 17., c(TEXT_2))))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(3.))
                            .child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).child(s(tp, "name").to_string()))
                            .child(div().text_size(px(T_S)).line_height(px(16.)).text_color(c(MUTED)).child(s(tp, "about").to_string()))
                            .when(!arr(tp, "steps").is_empty(), |d| d.child(div().pt(px(2.)).text_size(px(T_S)).line_height(px(16.)).text_color(c(TEXT_2)).child(needs_text(root, tp)))),
                    )
                    .on_click(cx.listener(move |r, _, window, cx| use_template(r, k, window, cx))),
            );
        }
        card().gap(px(12.)).child(div().flex().flex_col().gap(px(3.)).child(div().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).child(t("macro.fromTemplate"))).child(small_note(t("macro.tile.template")))).child(tiles)
    });

    // A template's to-dos over the form (MacroEditor.tsx's mc-todo): what it still needs set, each ticked once done,
    // with the way to it (an area: Edit on game; the rest: its step).
    let todos = todo_items(m);
    let todo_box = (!todos.is_empty()).then(|| {
        let all_done = todos.iter().all(|(_, _, d)| *d);
        let mut list = div().flex().flex_col().gap(px(8.));
        for (k, (i, fld, done)) in todos.iter().enumerate() {
            let (i, done) = (*i, *done);
            let area = if fld == "region" {
                arr(m, "steps").get(i).and_then(|st| area_ref(st.get("region"))).and_then(|n| arr(m, "areas").iter().find(|a| s(a, "name") == n)).map(|a| s(a, "label").to_string()).filter(|l| !l.is_empty())
            } else {
                None
            };
            let words = match area {
                Some(a) => root.i18n.tv("macro.todo.area", &[("area", &a)]),
                None => t(&format!("macro.todo.{fld}")),
            };
            let step_n = root.i18n.tv("macro.stepN", &[("n", &(i + 1).to_string())]);
            let mut row = div()
                .min_h(px(30.))
                .flex()
                .items_center()
                .gap(px(10.))
                .child(icon(if done { "check" } else { "record" }, 14., if done { c(OK) } else { c(MUTED) }))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .items_baseline()
                        .gap(px(6.))
                        .text_size(px(T_M))
                        .text_color(if done { c(MUTED) } else { c(TEXT) })
                        .child(words)
                        .child(div().flex_none().text_size(px(T_S)).text_color(c(MUTED)).child(format!("· {step_n}"))),
                );
            if !done {
                row = row.child(if fld == "region" {
                    Button::new(("mf-todo", k), t("macro.game.edit")).icon("screens").small().disabled(f.on_game).on_click(cx.listener(|r, _, _, cx| edit_on_game(r, cx)))
                } else {
                    Button::new(("mf-todo", k), t("macro.todo.go")).small().on_click(cx.listener(move |r, _, window, cx| {
                        if let Some(f) = r.auto.macro_form.as_mut() {
                            show_at(f, (Some(i), None), window, cx);
                        }
                        cx.notify();
                    }))
                });
            }
            list = list.child(row);
        }
        card().gap(px(10.)).child(div().text_size(px(T_H)).font_weight(FontWeight::SEMIBOLD).child(t(if all_done { "macro.todo.allDone" } else { "macro.todo.title" }))).child(list)
    });

    // When it runs.
    let trig = m.get("trigger").map(|x| s(x, "kind")).filter(|k| !k.is_empty()).unwrap_or("manual").to_string();
    let trig_items = ["manual", "interval", "join", "rejoin"].iter().map(|k| (k.to_string(), t(&format!("macro.trigOpt.{k}")), true)).collect();
    let mut when = div().flex().flex_col().gap(px(10.)).child(pick(cx, "mf-trig", trig_items, &trig, |form, v| {
        if let Some(tr) = obj_mut(&mut form.m, "trigger") {
            set(tr, "kind", json!(v));
        }
    }));
    if trig == "interval" {
        when = when.child(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(div().text_size(px(T_M)).text_color(c(TEXT_2)).child(t("macro.everyMin")))
                .child(field(&f.every, None).h(px(36.)).w(px(84.)))
                .child(pick(cx, "mf-unit", vec![("s".into(), t("macro.every.s"), true), ("min".into(), t("macro.every.min"), true)], if f.every_min { "min" } else { "s" }, |form, v| form.every_min = v == "min")),
        );
    }
    if trig != "manual" {
        let on = m.get("enabled").and_then(Value::as_bool) != Some(false);
        when = when.child(
            div()
                .flex()
                .items_start()
                .gap(px(12.))
                .child(Switch::new("mf-enabled", on).on_toggle(cx.listener(|r, on: &bool, _, cx| {
                    if let Some(f) = r.auto.macro_form.as_mut() {
                        set(&mut f.m, "enabled", json!(*on));
                    }
                    cx.notify();
                })))
                .child(div().flex().flex_col().gap(px(2.)).child(div().text_size(px(T_M)).font_weight(FontWeight::MEDIUM).child(t("macro.auto"))).child(small_note(t("macro.autoDesc")))),
        );
    }

    // Which accounts.
    let targets = Some(s(m, "targets")).filter(|k| !k.is_empty()).unwrap_or("selected").to_string();
    let tg_items = ["selected", "all", "accounts"].iter().map(|k| (k.to_string(), t(&format!("macro.tg.{k}")), true)).collect();
    let mut who = div().flex().flex_col().gap(px(10.)).child(pick(cx, "mf-tg", tg_items, &targets, |form, v| set(&mut form.m, "targets", json!(v))));
    if targets == "accounts" {
        let ticked: Vec<&str> = arr(m, "accountIds").iter().filter_map(Value::as_str).collect();
        let mut list = div().flex().flex_wrap().gap_x(px(22.)).gap_y(px(10.));
        for (n, a) in root.accounts().iter().enumerate() {
            let id = s(a, "id").to_string();
            let on = ticked.contains(&id.as_str());
            list = list.child(Check::new(("mf-acc", n), on).label(crate::pages::accounts::label(a)).on_toggle(cx.listener(move |r, on: &bool, _, cx| {
                if let Some(form) = r.auto.macro_form.as_mut() {
                    let mut ids: Vec<String> = arr(&form.m, "accountIds").iter().filter_map(Value::as_str).map(str::to_string).collect();
                    ids.retain(|x| x != &id);
                    if *on {
                        ids.push(id.clone());
                    }
                    set(&mut form.m, "accountIds", json!(ids));
                    form.error = None;
                }
                cx.notify();
            })));
        }
        who = who.child(list);
    }

    // Hotkey: Ctrl+Alt and a digit; one another macro has can't be picked.
    let hot_now = m.get("hotkey").and_then(Value::as_i64).map(|n| n.to_string()).unwrap_or_default();
    let mut hk_items = vec![(String::new(), t("macro.hk.none"), true)];
    // One another macro has says whose it is (MacroEditor.tsx's macro.hk.taken: "3 · Farm").
    hk_items.extend((1..=9i64).map(|n| match taken.get(&n) {
        Some(owner) => (n.to_string(), root.i18n.tv("macro.hk.taken", &[("k", &n.to_string()), ("name", owner)]), false),
        None => (n.to_string(), n.to_string(), true),
    }));
    let hk_title = if hot_now.is_empty() { t("macro.hotkey") } else { format!("{} · Ctrl+Alt+{hot_now}", t("macro.hotkey")) };
    let hotkey = pick(cx, "mf-hk", hk_items, &hot_now, |form, v| set(&mut form.m, "hotkey", v.parse::<i64>().map_or(Value::Null, |n| json!(n))));
    // A hotkey while the app's hotkeys are off (Settings): it won't work until they are on (MacroEditor.tsx).
    let hk_off = !hot_now.is_empty() && root.state.pointer("/hotkeys/on").and_then(Value::as_bool) == Some(false);
    let hotkey = div().flex().flex_col().gap(px(8.)).child(hotkey).when(hk_off, |d| d.child(small_note(t("macro.hk.off"))));

    // What a wait that runs out does, when a run counts as stuck, how long it may run, its random extras.
    let fail = Some(s(m, "onFail")).filter(|k| !k.is_empty()).unwrap_or("stop").to_string();
    let recover = f.steps().iter().any(|x| s(x, "kind") == "recover");
    let fail_items = ["stop", "skip", "restart", "rejoin", "alert"].iter().map(|k| (k.to_string(), if *k == "restart" && recover { t("macro.fail.recover") } else { t(&format!("macro.fail.{k}")) }, true)).collect();
    let on_fail = pick(cx, "mf-fail", fail_items, &fail, |form, v| set(&mut form.m, "onFail", json!(v)));
    let stuck_now = m.get("stuckMinutes").and_then(Value::as_i64).filter(|n| *n > 0);
    let mut mins = vec![2i64, 5, 10, 30];
    if let Some(n) = stuck_now.filter(|n| !mins.contains(n)) {
        mins.push(n);
        mins.sort_unstable();
    }
    let mut stuck_items = vec![("0".to_string(), t("macro.stuck.off"), true)];
    stuck_items.extend(mins.into_iter().map(|n| (n.to_string(), root.i18n.tv("macro.stuck.after", &[("n", &n.to_string())]), true)));
    // Only a step that looks for something can find nothing (MacroEditor.tsx's LOOK_STEPS, main steps and features).
    const LOOK: [&str; 7] = ["waitImage", "waitText", "waitColour", "waitStill", "clickImage", "clickColour", "follow"];
    let looks = arr(m, "steps").iter().chain(arr(m, "features").iter().flat_map(|fe| arr(fe, "steps").iter())).any(|x| LOOK.contains(&s(x, "kind")));
    let stuck = pick(cx, "mf-stuck", stuck_items, &stuck_now.unwrap_or(0).to_string(), |form, v| set(&mut form.m, "stuckMinutes", v.parse::<i64>().ok().filter(|n| *n > 0).map_or(Value::Null, |n| json!(n))));
    let unit = |u: String| div().text_size(px(T_S)).text_color(c(MUTED)).child(u);
    let limits = div()
        .flex()
        .flex_wrap()
        .gap_x(px(18.))
        .gap_y(px(12.))
        .child(lab(t("macro.e.stopAfterL"), field(&f.max, None).h(px(36.)).w(px(84.))))
        .child(lab(t("macro.e.randomMs"), div().flex().items_center().gap(px(8.)).child(field(&f.jit_ms, None).h(px(36.)).w(px(84.))).child(unit(t("macro.unit.s")))))
        .child(lab(t("macro.e.randomPx"), div().flex().items_center().gap(px(8.)).child(field(&f.jit_px, None).h(px(36.)).w(px(84.))).child(unit("px".into()))));

    // When it runs, on whom, its hotkey: one line under the name (the web editor's), that opens to all of it.
    let mut run_words = crate::pages::auto::when_text(root, m);
    if m.get("hotkey").and_then(Value::as_i64).is_none() {
        run_words = format!("{run_words} · {}", t("macro.hk.noneLine"));
    }
    let run_open = f.run_open;
    let run_line = hot("mf-run-line")
        .w_full()
        .h(px(42.))
        .px(px(14.))
        .rounded(px(R_M))
        .border_1()
        .tones(ca(WHITE, 0.03), ca(WHITE, 0.06))
        .edge(c(LINE), ca(WHITE, 0.17))
        .flex()
        .items_center()
        .gap(px(10.))
        .cursor_pointer()
        .child(icon("clock", 15., c(TEXT_2)))
        .child(div().flex_1().min_w_0().truncate().text_size(px(T_M)).text_color(c(TEXT)).child(run_words))
        .child(div().flex_none().text_size(px(T_S)).text_color(c(MUTED)).child(t(if run_open { "common.close" } else { "macro.runChange" })))
        .child(icon(if run_open { "chevronUp" } else { "chevronDown" }, 15., c(MUTED)))
        .on_click(cx.listener(|r, _, _, cx| {
            if let Some(f) = r.auto.macro_form.as_mut() {
                f.run_open = !f.run_open;
            }
            cx.notify();
        }));
    let top = card()
        .child(section(t("macro.trigger"), when))
        .child(section(t("macro.targets"), who))
        .child(section(hk_title, hotkey))
        .child(section(t("macro.onFail"), on_fail))
        .when(looks, |d| d.child(section(t("macro.stuckL"), stuck)))
        .child(limits);

    // The steps.
    let steps = f.steps();
    let deep = depths(steps);
    let count = steps.len();
    let feats = arr(m, "features");
    // Where the features switched on come in (the main steps' safe points), when there are features.
    let safe = if f.sec.is_none() && !feats.is_empty() { safe_ends(steps) } else { Vec::new() };
    let dragged = f.step_drag.map(|(from, len)| from..from + len);
    // A row dragged by its grip: the rows move as it passes over others (as the web editor's did); let go anywhere ends it.
    let mut rows = div()
        .id("mf-rows")
        .flex()
        .flex_col()
        .when(dragged.is_some(), |d| d.cursor_grabbing())
        .on_mouse_up(MouseButton::Left, cx.listener(|r, _, _, cx| end_step_drag(r, cx)))
        .on_mouse_up_out(MouseButton::Left, cx.listener(|r, _, _, cx| end_step_drag(r, cx)));
    // Add step's kinds: under a row's "+" when one was pressed, else under the Add step button.
    let fit = kinds_fit(steps, f.add_at(), f.sec.is_some());
    let mut add_kinds = f.adding.then(|| add_list(root, f, &fit, cx));
    // A long list: only the rows near the view are drawn, the others are space of their height (measured once drawn,
    // else the average of those measured). Where the list stands comes from the frame before: one scroll step late,
    // which NEAR_PX covers; a jump further asks for another frame at once.
    let draw_near = count >= DRAW_NEAR_FROM;
    let mut near_y = 0f32;
    let mut near_gap = 0f32;
    let (near_top, near_view) = f.near_at.get();
    let near_view = if near_view > 0. { near_view } else { 1200. };
    let (near_from, near_to) = (-near_top - NEAR_PX, -near_top + near_view + NEAR_PX);
    let near_guess = {
        let tall = f.near_tall.borrow();
        if tall.is_empty() { 54. } else { tall.values().sum::<f32>() / tall.len() as f32 }
    };
    if draw_near {
        let at = f.near_at.clone();
        rows = rows.child(
            canvas(
                move |b, window, _| {
                    let (old_top, old_view) = at.get();
                    let now = (f32::from(b.origin.y), f32::from(window.viewport_size().height));
                    at.set(now);
                    if (now.0 - old_top).abs() > NEAR_PX / 3. || now.1 != old_view {
                        window.refresh();
                    }
                },
                |_, _, _, _| {},
            )
            .h(px(0.)),
        );
    }
    // A folded block's rows after its opener are not drawn (MacroEditor.tsx's shapes: hidden).
    let mut hide_to: Option<usize> = None;
    for (i, st) in steps.iter().enumerate() {
        if hide_to.is_some_and(|e| i <= e) {
            continue;
        }
        let kind = s(st, "kind");
        let open = f.open == Some(i);
        if draw_near {
            // A folded block's opener hides its rows here as well, drawn or not.
            if opener(kind) && is_folded(st) {
                hide_to = block_end(steps, i);
            }
            let h = f.near_tall.borrow().get(&i).copied().unwrap_or(near_guess);
            let keep = open || f.add_here == Some(i) || f.step_drag.is_some_and(|(from, _)| from == i) || running_at == Some(i);
            let far = near_y + h < near_from || near_y > near_to;
            near_y += h;
            if far && !keep {
                near_gap += h;
                continue;
            }
            if near_gap > 0. {
                rows = rows.child(div().h(px(near_gap)).flex_none());
                near_gap = 0.;
            }
        }
        // This step's rows, measured as drawn (a long list).
        let outer = std::mem::replace(&mut rows, div().id(("mf-steprows", i)).flex().flex_col());
        let indent = deep.get(i).copied().unwrap_or(0) as f32 * 20.;
        let quiet = ender(kind) || kind == "else";
        let has_body = !ender(kind) && !matches!(kind, "else" | "break" | "stop" | "recover" | "rejoin");
        let mine: Vec<&Prob> = probs.iter().filter(|p| p.step == Some(i) && p.sec == f.sec).collect();
        let bad = mine.iter().any(|p| !p.warn);
        let warn = !bad && !mine.is_empty();
        // An If or a Repeat with its end folds to its one row: "· 4 steps", and how many problems are hidden in it.
        let end = if opener(kind) { block_end(steps, i) } else { None };
        let fold = end.is_some() && is_folded(st);
        if fold {
            hide_to = end;
        }
        let hidden_bad = if fold { probs.iter().filter(|p| !p.warn && p.sec == f.sec && p.step.is_some_and(|k| k > i && end.is_some_and(|e| k <= e))).count() } else { 0 };
        // Between the rows: a "+" under the mouse puts a step at that place (MacroEditor.tsx's mc-ins).
        rows = rows.child(if dragged.is_some() {
            div().h(px(8.)).into_any_element()
        } else {
            let g = SharedString::from(format!("mf-ins-{i}"));
            let tip: SharedString = t("macro.insHere").into();
            div()
                .id(("mf-ins", i))
                .group(g.clone())
                .ml(px(indent + 24.))
                .h(px(8.))
                .flex()
                .items_center()
                .gap(px(4.))
                .cursor_pointer()
                .child(div().flex_none().size(px(16.)).rounded_full().bg(c(PANEL)).border_1().border_color(ca(FACE, 0.6)).flex().items_center().justify_center().opacity(0.).group_hover(g.clone(), |s| s.opacity(1.)).child(icon("plus", 10., c(FACE))))
                .child(div().flex_1().h(px(1.)).bg(ca(FACE, 0.)).group_hover(g, |s| s.bg(ca(FACE, 0.45))))
                .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
                .on_click(edit(cx, move |form| {
                    if form.adding && form.add_here == Some(i) {
                        form.adding = false;
                        form.add_here = None;
                    } else {
                        form.adding = true;
                        form.add_here = Some(i);
                        form.keys = false;
                    }
                }))
                .into_any_element()
        });
        if f.add_here == Some(i) {
            if let Some(p) = add_kinds.take() {
                rows = rows.child(div().ml(px(indent + 24.)).mr(px(126.)).mb(px(6.)).child(p));
            }
        }
        let fold_button = end.map(|_| {
            IconButton::new(("mf-fold", i), if fold { "chevronRight" } else { "chevronDown" })
                .size(26.)
                .tip(t(if fold { "macro.block.unfold" } else { "macro.block.fold" }))
                .on_click(edit(cx, move |form| form.toggle_fold(i)))
        });
        let folded_words = fold.then(|| root.i18n.tn("macro.block.steps", f.inner_steps(i) as i64, &[]));
        let hidden_mark = (hidden_bad > 0).then(|| {
            let tip: SharedString = root.i18n.tn("macro.block.probs", hidden_bad as i64, &[]).into();
            div()
                .id(("mf-hidden-bad", i))
                .flex_none()
                .min_w(px(20.))
                .h(px(20.))
                .px(px(6.))
                .rounded_full()
                .border_1()
                .border_color(c(BAD))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(T_XS))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(BAD))
                .child(hidden_bad.to_string())
                .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
        });
        let main = hot(("mf-step", i))
            .flex_1()
            .min_w_0()
            .h(px(38.))
            .px(px(10.))
            .flex()
            .items_center()
            .gap(px(10.))
            .rounded(px(R_S))
            .map(|d| if open { d.tones(ca(WHITE, 0.09), ca(WHITE, 0.09)) } else { d })
            .cursor_pointer()
            .child(div().w(px(24.)).flex_none().text_size(px(T_S)).text_color(c(MUTED)).font_features(tnum()).child((i + 1).to_string()))
            .child(icon(step_icon(kind), 15., if opener(kind) { c(FACE) } else { c(TEXT_2) }))
            .child(
                div()
                    .map(|d| if fold { d.flex_shrink(1.) } else { d.flex_1() })
                    .min_w_0()
                    .truncate()
                    .text_size(px(T_M))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if quiet { c(MUTED) } else { c(TEXT) })
                    .child(step_text(root, m, &others, st, i + 1)),
            )
            .when_some(folded_words, |d, w| d.child(div().flex_none().text_size(px(T_S)).text_color(c(MUTED)).child(w)).child(div().flex_1()))
            .children(hidden_mark)
            .when(running_at == Some(i), |d| d.child(div().flex_none().text_size(px(T_S)).font_weight(FontWeight::SEMIBOLD).text_color(c(OK)).child(t("macro.run.here"))))
            .when(bad || warn, |d| d.child(div().size(px(8.)).flex_none().rounded_full().bg(if bad { c(BAD) } else { c(WARN) })))
            .on_click(cx.listener(move |r, _, window, cx| open_step(r, if open { None } else { Some(i) }, window, cx)));
        let too_many = root.i18n.tv("macro.err.tooMany", &[("max", &MAX_STEPS.to_string())]);
        // The grip: an If or a Repeat takes its whole block with it; an End moves with its block only.
        let grip = if ender(kind) {
            div().w(px(20.)).flex_none().into_any_element()
        } else {
            div()
                .id(("mf-grip", i))
                .w(px(20.))
                .h(px(38.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .cursor_grab()
                .child(icon("grip", 14., if dragged.as_ref().is_some_and(|d| d.contains(&i)) { c(TEXT) } else { c(DIM) }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |r, _, _, cx| {
                        if let Some(f) = r.auto.macro_form.as_mut() {
                            let (a, b) = span(f.steps(), i);
                            f.step_drag = Some((a, b - a + 1));
                        }
                        cx.stop_propagation();
                        cx.notify();
                    }),
                )
                .into_any_element()
        };
        let mut line = div()
            .ml(px(indent))
            .flex()
            .items_center()
            .gap(px(2.))
            .when(dragged.as_ref().is_some_and(|d| d.contains(&i)), |d| d.opacity(0.55))
            .when(dragged.is_some(), |d| {
                d.on_mouse_move(cx.listener(move |r, ev: &MouseMoveEvent, _, cx| {
                    let Some(f) = r.auto.macro_form.as_mut() else { return };
                    if ev.pressed_button != Some(MouseButton::Left) {
                        f.step_drag = None;
                        return cx.notify();
                    }
                    let was = f.step_drag;
                    f.drag_over(i);
                    if f.step_drag != was {
                        push_game(r);
                        cx.notify();
                    }
                }))
            })
            .child(grip)
            .children(fold_button)
            .child(main)
            .child(if has_body {
                IconButton::new(("mf-open", i), if open { "chevronUp" } else { "chevronDown" })
                    .size(30.)
                    .tip(t(if open { "macro.stepClose" } else { "macro.stepOpen" }))
                    .on_click(cx.listener(move |r, _, window, cx| open_step(r, if open { None } else { Some(i) }, window, cx)))
                    .into_any_element()
            } else {
                div().w(px(30.)).flex_none().into_any_element()
            })
            // An arrow that can't move the step (the first one's up, the last one's down, a block's end) leaves its
            // place empty, as the preset editor does: a greyed one read as another icon weight (review 5.3).
            .child(if i == 0 || ender(kind) {
                div().w(px(30.)).flex_none().into_any_element()
            } else {
                IconButton::new(("mf-up", i), "chevronUp").size(30.).tip(t("lo.up")).on_click(edit(cx, move |form| form.move_step(i, -1))).into_any_element()
            })
            .child(if block_end(steps, i).unwrap_or(i) + 1 >= count || ender(kind) {
                div().w(px(30.)).flex_none().into_any_element()
            } else {
                IconButton::new(("mf-down", i), "chevronDown").size(30.).tip(t("lo.down")).on_click(edit(cx, move |form| form.move_step(i, 1))).into_any_element()
            });
        if !ender(kind) {
            line = line.child(step_more(root, f, i, kind, too_many, cx));
        } else {
            line = line.child(div().w(px(30.)).flex_none());
        }
        rows = rows.child(line);
        // Under a closed row: its note, and where Try it stopped.
        let under_ml = indent + 66.;
        if !open && !s(st, "note").is_empty() {
            rows = rows.child(div().ml(px(under_ml)).mb(px(4.)).text_size(px(T_S)).text_color(c(MUTED)).child(s(st, "note").to_string()));
        }
        if let Some((_, why)) = stopped.as_ref().filter(|(k, _)| *k == i) {
            let words = if why.is_empty() || why == "notFound" || why == "timeout" { t("macro.stop.here") } else { root.i18n.tv("macro.stop.hereWhy", &[("why", &crate::pages::auto::why_text(root, why))]) };
            rows = rows.child(div().ml(px(under_ml)).mb(px(4.)).child(bad_line(words, false)));
        }
        if open && has_body {
            rows = rows.child(div().ml(px(indent + 56.)).mr(px(126.)).mt(px(2.)).mb(px(8.)).pl(px(16.)).py(px(4.)).border_l_2().border_color(ca(WHITE, 0.1)).child(step_body(root, f, i, st, &others, win.is_some(), &mine, cx)));
        } else if !mine.is_empty() {
            // What is wrong, under its row (not only a dot), and a closed row's Fix that opens it.
            // A step with no area to look in: its own fix (MacroEditor.tsx's fixOf "area"); else Fix opens it.
            let area_fix = mine.iter().find(|p| !p.warn).is_some_and(|p| p.fix == Some("area"));
            let fix = (!open && bad).then(|| {
                if area_fix {
                    Button::new(("mf-fix", i), t("macro.fix.area")).small().on_click(cx.listener(move |r, _, window, cx| fix_area(r, i, window, cx)))
                } else {
                    Button::new(("mf-fix", i), t("macro.fix")).small().on_click(cx.listener(move |r, _, window, cx| open_step(r, Some(i), window, cx)))
                }
            });
            rows = rows.child(
                div()
                    .ml(px(indent + 66.))
                    .mr(px(126.))
                    .mb(px(6.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(div().flex_1().min_w_0().flex().flex_col().gap(px(4.)).children(mine.iter().map(|p| bad_line(p.bare.clone(), p.warn))))
                    .children(fix),
            );
        }
        if safe.contains(&i) || (fold && end.is_some_and(|e| safe.contains(&e))) {
            rows = rows.child(
                div()
                    .ml(px(22.))
                    .my(px(4.))
                    .h(px(28.))
                    .px(px(10.))
                    .rounded(px(R_S))
                    .border_1()
                    .border_dashed()
                    .border_color(ca(FACE, 0.45))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .text_size(px(T_S))
                    .text_color(c(TEXT_2))
                    .child(icon("layers", 13., c(FACE)))
                    .child(t("macro.feat.here")),
            );
        }
        let mine_rows = std::mem::replace(&mut rows, outer);
        rows = if draw_near {
            let tall = f.near_tall.clone();
            rows.child(mine_rows.relative().child(canvas(move |b, _, _| {
                tall.borrow_mut().insert(i, f32::from(b.size.height));
            }, |_, _, _, _| {}).absolute().inset_0()))
        } else {
            rows.child(mine_rows)
        };
    }
    if near_gap > 0. {
        rows = rows.child(div().h(px(near_gap)).flex_none());
    }

    // Add step (its kinds: add_list), and Record.
    let rec = root.state.get("macroRecording").filter(|v| v.is_object());
    let open_here = f.adding && f.add_here.is_none();
    let mut add = div().flex().flex_col().gap(px(12.)).child(
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(
                Button::new("mf-add", t("macro.addStep")).icon("plus").icon_end(if open_here { "chevronUp" } else { "chevronDown" }).on_click(edit(cx, |form| {
                    // (Open under a row's "+": it comes here instead.)
                    if form.add_here.take().is_some() {
                        form.adding = true;
                    } else {
                        form.adding = !form.adding;
                    }
                    form.keys = false;
                })),
            )
            .child(
                Button::new("mf-rec", t("macro.recordBtn"))
                    .icon("record")
                    .tip(t(if win.is_some() { "macro.rec.tip" } else { "macro.need.record" }))
                    .disabled(win.is_none() || rec.is_some())
                    .on_click(cx.listener(|r, _, _, cx| record(r, cx))),
            ),
    );
    // (Not placed under a row, as its "+" is gone with the rows: here.)
    if let Some(p) = add_kinds.take() {
        add = add.child(p);
    }

    // A recording under way (its countdown, then its steps so far), or its steps waiting to be added.
    let recording = rec.filter(|_| f.recording).map(|rec| {
        let left = crate::time::iso_secs(s(rec, "startsAt")).map_or(0, |at| at - crate::time::now_secs());
        let words = if left > 0 { root.i18n.tv("macro.rec.startIn", &[("n", &left.to_string())]) } else { root.i18n.tv(if count > 0 { "macro.rec.new" } else { "macro.rec.count" }, &[("n", &trim(num(rec, "steps")))]) };
        div()
            .px(px(14.))
            .py(px(10.))
            .rounded(px(R_M))
            .bg(ca(BAD, 0.1))
            .flex()
            .items_center()
            .gap(px(10.))
            .child(div().size(px(9.)).flex_none().rounded_full().bg(c(BAD)))
            .child(div().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).child(words))
            .child(div().flex_1().text_size(px(T_S)).text_color(c(MUTED)).child(t("macro.rec.hint")))
            .child(Button::new("mf-rec-stop", t("macro.rec.stop")).icon("stop").small().on_click(cx.listener(|r, _, _, cx| {
                r.send("stopRecording", json!({}));
                cx.notify();
            })))
    });
    let pending = f.pending.as_ref().map(|p| {
        let n = arr(p, "steps").len() as i64;
        div()
            .px(px(14.))
            .py(px(10.))
            .rounded(px(R_M))
            .bg(ca(WHITE, 0.05))
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.))
            .child(div().flex_1().text_size(px(T_M)).font_weight(FontWeight::SEMIBOLD).child(root.i18n.tn("macro.rec.done", n, &[])))
            .child(Button::new("mf-rec-add", t("macro.rec.append")).primary().small().on_click(edit(cx, |form| {
                if let Some(p) = form.pending.take() {
                    let more = arr(&p, "steps").to_vec();
                    if let Some(l) = form.steps_mut() {
                        l.extend(more);
                    }
                }
            })))
            .child(Button::new("mf-rec-swap", t("macro.rec.replace")).small().on_click(edit(cx, |form| {
                if let Some(p) = form.pending.take() {
                    if let Some(l) = form.steps_mut() {
                        *l = arr(&p, "steps").to_vec();
                    }
                    set(&mut form.m, "recordedSize", json!({ "w": num(&p, "w"), "h": num(&p, "h") }));
                    form.open = None;
                    form.remap(|_| None);
                }
            })))
            .child(Button::new("mf-rec-drop", t("macro.rec.discard")).quiet().small().on_click(edit(cx, |form| form.pending = None)))
    });

    let steps_card = card()
        .gap(px(14.))
        .child(
            section_head(t("macro.steps"), Some(count))
                .when(feats.is_empty(), |d| d.child(feat_add(root, "mf-feat-add-head", cx))),
        )
        .children(feat_tabs(root, f, &probs[..], cx))
        .children(feat_head(root, f, &probs[..], cx))
        .children(recording)
        .children(pending)
        .when(count == 0, |d| d.child(small_note(t(if f.sec.is_some() { "macro.feat.noSteps" } else { "macro.noSteps" }))))
        .when(count > 0, |d| d.child(rows))
        .child(add);

    let game = game_card(root, f, pic_w, run, cx);
    let player = player_card(root, f, cx);

    // The steps are the form's main thing: they lead, with the game and the settings in a column beside them when
    // there is room.
    let body = if wide {
        div()
            .px(px(SIDE))
            .pt(px(4.))
            .pb(px(32.))
            .flex()
            .items_start()
            .gap(px(GRID_GAP))
            .child(div().flex_1().min_w_0().flex().flex_col().gap(px(GRID_GAP)).children(todo_box).child(div().flex().flex_col().gap(px(10.)).child(head_row).child(run_line).when(run_open, |d| d.child(top))).children(templates).child(steps_card).child(player))
            .child(div().w(px(side_w)).flex_none().flex().flex_col().gap(px(GRID_GAP)).child(game))
    } else {
        div().px(px(SIDE)).pt(px(4.)).pb(px(32.)).flex().flex_col().gap(px(GRID_GAP)).children(todo_box).child(div().flex().flex_col().gap(px(10.)).child(head_row).child(run_line).when(run_open, |d| d.child(top))).children(templates).child(game).child(steps_card).child(player)
    };

    // When it runs, on whom, its hotkey, how many steps: the web editor's summary line, said at once under its name.
    let mut summary: Vec<(String, Option<Hsla>)> = Vec::new();
    summary.push((root.i18n.tn("au.stepsN", arr(m, "steps").len() as i64, &[]), None));
    if trying {
        summary.push((t("macro.run.here"), Some(c(OK))));
    }
    let typed = text(&f.name, cx);
    let kind_title = t(if f.is_new { "macro.newTitle" } else { "macro.editTitle" });
    let header = div()
        .flex_none()
        .px(px(SIDE))
        .pt(px(14.))
        .pb(px(18.))
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .child(tool("mf-back", Some("chevronLeft"), false).ml(px(-10.)).child(t("nav.auto")).on_click(cx.listener(|r, _, window, cx| leave(r, window, cx))))
                .child(div().text_size(px(T_M)).text_color(c(DIM)).child("/"))
                .child(div().px(px(6.)).text_size(px(T_M)).text_color(c(MUTED)).child(kind_title.clone())),
        )
        .child(div().text_size(px(T_TITLE)).line_height(px(38.)).font_weight(FontWeight::BOLD).truncate().child(if typed.trim().is_empty() { kind_title } else { typed }))
        .child(facts(summary));

    let del = (!f.is_new).then(|| {
        let id = my_id.clone();
        Button::new("mf-del-macro", t("macro.delete")).icon("trash").danger().on_click(cx.listener(move |r, _, window, cx| {
            let name = r.auto.macro_form.as_ref().map(|f| f.name.read(cx).value().trim().to_string()).unwrap_or_default();
            let id = id.clone();
            r.confirm(r.tr("macro.deleteTitle"), r.i18n.tv("macro.deleteQ", &[("name", &name)]), r.tr("macro.delete"), true, None, window, cx, move |r, cx| {
                let Some(link) = r.link.clone() else { return };
                let rx = link.call("deleteMacro", json!({ "id": id }));
                answer(rx, cx, move |r, answer, cx| {
                    match answer {
                        Ok(Ok(_)) => {
                            close(r, cx);
                            r.toast = Some((r.tr("macro.deleted"), "ok".into()));
                        }
                        Ok(Err(e)) => r.toast = Some((e, "bad".into())),
                        Err(_) => {}
                    }
                    cx.notify();
                });
            });
        }))
    });
    // The foot: the error, or (once Save or Try was pressed) how many problems stop it; a click opens the first.
    let errors: Vec<&Prob> = probs.iter().filter(|p| !p.warn).collect();
    let first_step = errors.iter().find(|p| p.step.is_some() || p.sec.is_some()).map(|p| (p.step, p.sec.clone()));
    let say = f.error.clone().or_else(|| (f.tried && !errors.is_empty()).then(|| root.i18n.tn("macro.problemsN", errors.len() as i64, &[])));
    // The first problem a name would fix (the macro's, or an area's): "Name it" takes you to the box
    // (MacroEditor.tsx's fixOf "name").
    let name_fix = errors.first().and_then(|p| p.fix).filter(|x| matches!(*x, "name" | "areaName")).filter(|_| say.is_some()).map(|x| {
        let area = x == "areaName";
        Button::new("mf-fix-name", t("macro.fix.name")).small().on_click(cx.listener(move |r, _, window, cx| fix_name(r, area, window, cx)))
    });
    let foot = div()
        .flex_none()
        .px(px(SIDE))
        .py(px(14.))
        .border_t_1()
        .border_color(c(LINE))
        .bg(ca(SHADE, 0.12))
        .flex()
        .items_center()
        .gap(px(10.))
        .children(del)
        .child(
            div().id("mf-foot-say").flex_1().min_w_0().when_some(say, |d, e| {
                d.flex()
                    .items_center()
                    .gap(px(8.))
                    .text_size(px(T_M))
                    .text_color(c(BAD))
                    .child(icon("alert", 15., c(BAD)))
                    .child(div().min_w_0().child(e))
                    .when_some(first_step, |d, at| {
                        d.cursor_pointer().on_click(cx.listener(move |r, _, window, cx| {
                            if let Some(f) = r.auto.macro_form.as_mut() {
                                show_at(f, at.clone(), window, cx);
                            }
                            cx.notify();
                        }))
                    })
            }),
        )
        .children(name_fix)
        .child(Button::new("mf-cancel", t("common.cancel")).quiet().on_click(cx.listener(|r, _, window, cx| leave(r, window, cx))))
        .child(Button::new("mf-save", t("macro.save")).icon("check").primary().disabled(f.saving).on_click(cx.listener(|r, _, window, cx| save(r, window, cx))));

    div()
        .id("mf-form")
        .size_full()
        .rounded(px(R_PANEL))
        .bg(c(PANEL))
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(header)
        // (The bar at its right edge, always there, says more is below: no fade.)
        .child(
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .child(div().id("mf-form-body").size_full().overflow_y_scroll().track_scroll(&f.scroll).child(body))
                .child(crate::pages::auto::scroll_bar("mf-form-bar", &f.scroll)),
        )
        .child(foot)
        .into_any_element()
}
