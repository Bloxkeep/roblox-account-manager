// A macro's panel beside the game (PanelView.tsx and PanelControls.tsx, app.css .pn-*, part for part): its name, Run/Stop
// with the hotkey, one status line with the counters, its tabs, the settings the maker let the player change, Move areas
// and Reset all. Folded, a pill: the light, the name, Run/Stop. The core keeps every value and its maths; what is
// clicked goes to it as an "act" and comes back in the next "panel".
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    time::Duration,
};

use gpui_kit::{
    component::input::{Input, InputEvent, InputState},
    prelude::*,
    *,
};
use serde_json::{Value, json};

use super::{Body, Link, ToolPage, drag_row, title_icon};
use crate::{kit::*, theme::*};

const ERROR_FOR: u64 = 5000;
const RESET_FOR: u64 = 6000;
const AWAY_AFTER: u64 = 400;
const REPEAT_AFTER: u64 = 400;
const REPEAT_EVERY: u64 = 40;
const WHEEL_REST: u64 = 300;
const WIDTH: f32 = 360.;
const MAX_ROWS: usize = 8;
const ROW: f32 = 32.;

thread_local! {
    /// The tab last shown, by macro (the page kept it in localStorage).
    static TABS: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}

/// An open list: pick one (`many` false) or several (ticked meanwhile, sent as it closes); `active` has the keyboard.
struct List {
    name: String,
    many: bool,
    ticked: Vec<String>,
    active: usize,
    typed: (String, std::time::Instant),
}

pub struct Panel {
    data: Value,
    id: String,
    pub pill: bool,
    tab: Option<String>,
    hints: HashMap<String, (String, u64)>,
    status: Option<(String, bool, u64)>,
    turn: u64,
    menu: Option<(Point<Pixels>, usize)>,
    list: Option<List>,
    /// Steps a number was moved here and not yet answered, by its name; the one held (− or +) and its repeat.
    moved: HashMap<String, i64>,
    held: Option<String>,
    repeat: Option<Task<()>>,
    wheel: Option<Task<()>>,
    typing: Option<(String, String, Entity<InputState>)>,
    waiting_key: Option<String>,
    dirty: bool,
    away: Option<Task<()>>,
    tick: Option<Task<()>>,
    /// The rows' natural height past what is shown (they scroll when the screen caps the window).
    hidden_rows: Rc<Cell<f32>>,
    /// The height it would take with nothing cut, rows not scrolled (0: as drawn): what is said as its size, so the core
    /// sizes the window to it (capped at the screen). Read from where its last part ends, which lies past the window's
    /// foot when the window is shorter than it (its parts keep their own heights and the panel clips them).
    pub natural: Rc<Cell<f32>>,
    /// Rows whose control is wider than 55 % of them: it goes under the label, at full width.
    stacked: Rc<RefCell<HashSet<String>>>,
}

fn f(v: &Value, k: &str) -> Option<f64> {
    v.get(k).and_then(Value::as_f64)
}
fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or_default().to_string()
}
fn strs(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array).map(|l| l.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

impl Panel {
    pub fn new(data: Value) -> Panel {
        let id = data.pointer("/macro/id").and_then(Value::as_str).unwrap_or_default().to_string();
        let pill = data.get("pill").and_then(Value::as_bool).unwrap_or(false);
        let mut p = Panel {
            data: Value::Null,
            id: id.clone(),
            pill,
            tab: TABS.with(|t| t.borrow().get(&id).cloned()),
            hints: HashMap::new(),
            status: None,
            turn: 0,
            menu: None,
            list: None,
            moved: HashMap::new(),
            held: None,
            repeat: None,
            wheel: None,
            typing: None,
            waiting_key: None,
            dirty: false,
            away: None,
            tick: None,
            hidden_rows: Rc::new(Cell::new(0.)),
            natural: Rc::new(Cell::new(0.)),
            stacked: Rc::new(RefCell::new(HashSet::new())),
        };
        p.data = data;
        p
    }

    /// Another macro: drawn afresh (the page keyed its view by the macro).
    pub fn is_for(&self, data: &Value) -> bool {
        data.pointer("/macro/id").and_then(Value::as_str).unwrap_or_default() == self.id
    }

    /// The core's next "panel": a number's value it answered replaces what was moved here meanwhile.
    pub fn set(&mut self, data: Value, cx: &mut Context<ToolPage>) {
        let old: HashMap<String, (Value, Value)> = options(&self.data).iter().map(|o| (s(o, "name"), (o.get("value").cloned().unwrap_or(Value::Null), o.get("shown").cloned().unwrap_or(Value::Null)))).collect();
        for o in options(&data) {
            let name = s(o, "name");
            let now = (o.get("value").cloned().unwrap_or(Value::Null), o.get("shown").cloned().unwrap_or(Value::Null));
            if old.get(&name) != Some(&now) && self.held.as_deref() != Some(name.as_str()) {
                self.moved.remove(&name);
            }
        }
        self.data = data;
        self.ticking(cx);
    }

    /// A change the core refused: under its row, or in the status line.
    pub fn error(&mut self, data: Value, cx: &mut Context<ToolPage>) {
        let text = s(&data, "text");
        match data.get("name").and_then(Value::as_str).filter(|n| !n.is_empty()) {
            Some(n) => self.hint(n, text, ERROR_FOR, cx),
            None => self.say(text, true, ERROR_FOR, cx),
        }
    }

    fn now(&self) -> &Value {
        self.data.get("now").unwrap_or(&Value::Null)
    }
    fn state(&self) -> String {
        s(self.now(), "state")
    }
    fn running(&self) -> bool {
        matches!(self.state().as_str(), "running" | "paused")
    }
    fn editing(&self) -> bool {
        self.now().get("editingAreas").and_then(Value::as_bool).unwrap_or(false)
    }
    fn has_window(&self) -> bool {
        self.now().get("hasWindow").and_then(Value::as_bool).unwrap_or(false)
    }
    /// Something holds the keyboard (a number typed, a key waited for, a list open): Esc is theirs.
    fn holds(&self) -> bool {
        self.list.is_some() || self.typing.is_some() || self.waiting_key.is_some()
    }

    fn hint(&mut self, name: &str, text: String, ms: u64, cx: &mut Context<ToolPage>) {
        self.turn += 1;
        let turn = self.turn;
        self.hints.insert(name.to_string(), (text, turn));
        let name = name.to_string();
        later(cx, ms, move |p| {
            if p.hints.get(&name).is_some_and(|h| h.1 == turn) {
                p.hints.remove(&name);
            }
        });
    }

    fn say(&mut self, text: String, error: bool, ms: u64, cx: &mut Context<ToolPage>) {
        self.turn += 1;
        let turn = self.turn;
        self.status = Some((text, error, turn));
        later(cx, ms, move |p| {
            if p.status.as_ref().is_some_and(|s| s.2 == turn) {
                p.status = None;
            }
        });
    }

    /// A clock goes while it runs (the time since it started, a feature's "next in"): redrawn each second, only then.
    fn ticking(&mut self, cx: &mut Context<ToolPage>) {
        let wants = self.running();
        if !wants {
            self.tick = None;
            return;
        }
        if self.tick.is_some() {
            return;
        }
        self.tick = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let go = this
                    .update(cx, |t, cx| {
                        cx.notify();
                        matches!(&t.body, Body::Panel(p) if p.running())
                    })
                    .unwrap_or(false);
                if !go {
                    break;
                }
            }
        }));
    }

    /// A list closed: several ticked are sent now (if they changed).
    fn close_list(&mut self, link: Link) {
        let Some(l) = self.list.take() else { return };
        if !l.many {
            return;
        }
        let Some(o) = options(&self.data).into_iter().find(|o| s(o, "name") == l.name) else { return };
        let choices = strs(o.get("choices"));
        let sent = strs(o.get("value"));
        let now: Vec<String> = choices.iter().filter(|c| l.ticked.contains(c)).cloned().collect();
        let before: Vec<String> = choices.iter().filter(|c| sent.contains(c)).cloned().collect();
        if now != before {
            link.act("set", json!({ "name": l.name, "value": now }));
            self.dirty = true;
        }
    }

    /// Its window lost the keyboard: a list closes, a key stops being waited for.
    pub fn blurred(&mut self, link: Link) {
        self.close_list(link);
        self.waiting_key = None;
        self.menu = None;
    }
}

fn options(data: &Value) -> Vec<&Value> {
    data.pointer("/macro/options").and_then(Value::as_array).map(|l| l.iter().collect()).unwrap_or_default()
}

fn panel(t: &mut ToolPage) -> Option<&mut Panel> {
    match &mut t.body {
        Body::Panel(p) => Some(p),
        _ => None,
    }
}

/// Runs `f` on the panel after `ms`, then redraws.
fn later(cx: &mut Context<ToolPage>, ms: u64, f: impl FnOnce(&mut Panel) + 'static) {
    cx.spawn(async move |this, cx| {
        cx.background_executor().timer(Duration::from_millis(ms)).await;
        let _ = this.update(cx, |t, cx| {
            if let Some(p) = panel(t) {
                f(p);
            }
            cx.notify();
        });
    })
    .detach();
}

// ───────────────────────── numbers ─────────────────────────

struct Num {
    raw: f64,
    step: f64,
    scale: f64,
    min: f64,
    max: f64,
    suffix: String,
}

impl Num {
    fn of(o: &Value) -> Num {
        Num {
            raw: f(o, "value").unwrap_or(0.),
            step: f(o, "step").filter(|v| *v != 0.).unwrap_or(1.),
            scale: f(o, "scale").filter(|v| *v != 0.).unwrap_or(1.),
            min: f(o, "min").unwrap_or(f64::NEG_INFINITY),
            max: f(o, "max").unwrap_or(f64::INFINITY),
            suffix: s(o, "suffix"),
        }
    }
    fn at(&self, by: i64) -> f64 {
        ((self.raw + by as f64 * self.step) / self.step).round() * self.step
    }
    fn value_at(&self, by: i64) -> f64 {
        self.at(by).clamp(self.min, self.max)
    }
    fn fmt(&self, v: f64) -> String {
        let mut t = format!("{:.2}", v / self.scale);
        while t.contains('.') && (t.ends_with('0') || t.ends_with('.')) {
            t.pop();
        }
        if t == "-0" {
            t = "0".into();
        }
        crate::lang::cur().decimal(&t) + &self.suffix
    }
}

/// Moves a number by `by` steps here (not sent yet); false at its end.
fn move_by(p: &mut Panel, name: &str, by: i64) -> bool {
    let Some(o) = options(&p.data).into_iter().find(|o| s(o, "name") == name) else { return false };
    let n = Num::of(o);
    let was = p.moved.get(name).copied().unwrap_or(0);
    let next = was + by;
    if n.value_at(next) == n.value_at(was) {
        return false;
    }
    p.moved.insert(name.to_string(), next);
    true
}

/// The steps moved here, told to the core at once (added up).
fn send_moved(p: &mut Panel, link: Link, name: &str) {
    let by = p.moved.get(name).copied().unwrap_or(0);
    if by == 0 {
        return;
    }
    link.act("nudge", json!({ "name": name, "by": by }));
    p.dirty = true;
}

fn press(t: &mut ToolPage, name: String, by: i64, cx: &mut Context<ToolPage>) {
    let Some(p) = panel(t) else { return };
    move_by(p, &name, by);
    p.held = Some(name.clone());
    // Held: it repeats after 0.4 s, every 40 ms, until let go (or at its end).
    p.repeat = Some(cx.spawn(async move |this, cx| {
        cx.background_executor().timer(Duration::from_millis(REPEAT_AFTER)).await;
        loop {
            let go = this
                .update(cx, |t, cx| {
                    if !left_down() {
                        release(t, cx);
                        return false;
                    }
                    cx.notify();
                    panel(t).is_some_and(|p| p.held.as_deref() == Some(name.as_str()) && move_by(p, &name, by))
                })
                .unwrap_or(false);
            if !go {
                break;
            }
            cx.background_executor().timer(Duration::from_millis(REPEAT_EVERY)).await;
        }
    }));
    cx.notify();
}

/// The left button is down now (a hold let go outside the window sends no mouse-up here).
fn left_down() -> bool {
    // SAFETY: plain query of the button's state.
    unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(0x01) as u16 & 0x8000 != 0 }
}

fn release(t: &mut ToolPage, cx: &mut Context<ToolPage>) {
    let link = t.link;
    let Some(p) = panel(t) else { return };
    let Some(name) = p.held.take() else { return };
    p.repeat = None;
    send_moved(p, link, &name);
    cx.notify();
}

fn wheel(t: &mut ToolPage, name: String, by: i64, cx: &mut Context<ToolPage>) {
    let Some(p) = panel(t) else { return };
    if p.typing.is_some() {
        return;
    }
    move_by(p, &name, by);
    p.wheel = Some(cx.spawn(async move |this, cx| {
        cx.background_executor().timer(Duration::from_millis(WHEEL_REST)).await;
        let _ = this.update(cx, |t, cx| {
            let link = t.link;
            if let Some(p) = panel(t) {
                send_moved(p, link, &name);
            }
            cx.notify();
        });
    }));
    cx.notify();
}

/// The value clicked: typed into (Enter or leaving keeps it, Esc puts it back).
fn begin_typing(t: &mut ToolPage, name: String, shown: String, window: &mut Window, cx: &mut Context<ToolPage>) {
    let input = cx.new(|cx| InputState::new(window, cx));
    input.update(cx, |st, cx| {
        st.set_value(shown.clone(), window, cx);
        st.select_all(window, cx);
        st.focus(window, cx);
    });
    cx.subscribe(&input, |t: &mut ToolPage, _, ev: &InputEvent, cx| {
        if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
            end_typing(t, true, cx);
        }
    })
    .detach();
    if let Some(p) = panel(t) {
        p.typing = Some((name, shown, input));
    }
    cx.notify();
}

fn end_typing(t: &mut ToolPage, keep: bool, cx: &mut Context<ToolPage>) {
    let link = t.link;
    let Some(p) = panel(t) else { return };
    let Some((name, shown, input)) = p.typing.take() else { return };
    let text = input.read(cx).value().to_string();
    if keep && text.trim() != shown.trim() {
        link.act("typed", json!({ "name": name, "text": text }));
        p.dirty = true;
    }
    cx.notify();
}

// ───────────────────────── keys ─────────────────────────

/// Esc hides it (ends moving the areas), unless a list, a typed number or a waited key takes it first; the lists and the
/// menu go by the arrows, Enter and the first letters.
pub fn key(t: &mut ToolPage, key: &str, window: &mut Window, cx: &mut Context<ToolPage>) -> bool {
    let link = t.link;
    if matches!(&t.body, Body::Panel(p) if p.typing.is_some()) {
        if key == "escape" {
            end_typing(t, false, cx);
            window.focus(&t.focus, cx);
            return true;
        }
        return false;
    }
    let i18n_bad = t.t("panel.badKey");
    let all_none = [t.t("panel.all"), t.t("panel.none")];
    let Some(p) = panel(t) else { return false };

    if let Some(name) = p.waiting_key.clone() {
        if key == "tab" {
            p.waiting_key = None;
            cx.notify();
            return false;
        }
        if key == "escape" {
            p.waiting_key = None;
        } else {
            match crate::pages::macro_form::key_from(key) {
                None => p.hint(&name, i18n_bad, 3000, cx),
                Some(k) => {
                    p.waiting_key = None;
                    let now = options(&p.data).into_iter().find(|o| s(o, "name") == name).map(|o| s(o, "value")).unwrap_or_default();
                    if k != now {
                        link.act("set", json!({ "name": name, "key": k }));
                        p.dirty = true;
                    }
                }
            }
        }
        cx.notify();
        return true;
    }

    if let Some((at, active)) = p.menu {
        let n = 3;
        match key {
            "down" => p.menu = Some((at, (active + 1) % n)),
            "up" => p.menu = Some((at, (active + n - 1) % n)),
            "enter" | "space" => {
                p.menu = None;
                menu_run(p, link, active);
            }
            "escape" | "tab" => p.menu = None,
            _ => return false,
        }
        cx.notify();
        return true;
    }

    if let Some(l) = &mut p.list {
        let rows = list_rows(&p.data, l, &all_none);
        let n = rows.len().max(1);
        match key {
            "down" => l.active = (l.active + 1) % n,
            "up" => l.active = (l.active + n - 1) % n,
            "home" => l.active = 0,
            "end" => l.active = n - 1,
            "enter" | "space" => {
                let i = l.active;
                list_pick(t, i, cx);
                return true;
            }
            "escape" | "tab" => {
                // (Esc changes nothing for pick one; several are sent as the list closes, as the page did.)
                p.close_list(link);
            }
            k if k.chars().count() == 1 => {
                let now = std::time::Instant::now();
                let typed = if now.duration_since(l.typed.1) < Duration::from_millis(700) { format!("{}{k}", l.typed.0) } else { k.to_string() };
                l.typed = (typed.clone(), now);
                if let Some(i) = rows.iter().position(|r| !r.is_empty() && r.to_lowercase().starts_with(&typed)) {
                    l.active = i;
                }
            }
            _ => return false,
        }
        cx.notify();
        return true;
    }

    if key == "escape" {
        link.act(if p.editing() { "areas" } else { "hide" }, json!({}));
        return true;
    }
    false
}

/// A list's rows: its choices, after "All" and "None" when it ticks several.
fn list_rows(data: &Value, l: &List, all_none: &[String; 2]) -> Vec<String> {
    let choices = options(data).into_iter().find(|o| s(o, "name") == l.name).map(|o| strs(o.get("choices"))).unwrap_or_default();
    if l.many {
        let mut v = all_none.to_vec();
        v.extend(choices);
        v
    } else {
        choices
    }
}

fn list_pick(t: &mut ToolPage, i: usize, cx: &mut Context<ToolPage>) {
    let link = t.link;
    let Some(p) = panel(t) else { return };
    let Some(l) = &mut p.list else { return };
    let choices = options(&p.data).into_iter().find(|o| s(o, "name") == l.name).map(|o| strs(o.get("choices"))).unwrap_or_default();
    if l.many {
        match i {
            0 => l.ticked = choices,
            1 => l.ticked.clear(),
            _ => {
                if let Some(c) = choices.get(i - 2) {
                    if let Some(at) = l.ticked.iter().position(|x| x == c) {
                        l.ticked.remove(at);
                    } else {
                        l.ticked.push(c.clone());
                    }
                }
            }
        }
        l.active = i;
    } else {
        let name = l.name.clone();
        p.list = None;
        let now = options(&p.data).into_iter().find(|o| s(o, "name") == name).map(|o| s(o, "value")).unwrap_or_default();
        if let Some(c) = choices.get(i).filter(|c| **c != now) {
            link.act("set", json!({ "name": name, "value": c }));
            p.dirty = true;
        }
    }
    cx.notify();
}

fn menu_run(p: &mut Panel, link: Link, i: usize) {
    match i {
        0 => link.act("snap", json!({})),
        1 => {
            // (The pill's menu opens the macro at the step it stopped at; the panel's has its own Fix for that.)
            let err = p.now().get("lastError").filter(|e| e.is_object() && p.pill).cloned();
            match err {
                Some(e) => link.act("edit", json!({ "step": e.get("step").cloned().unwrap_or(Value::Null), "section": e.get("section").cloned().unwrap_or(Value::Null) })),
                None => link.act("edit", json!({})),
            }
        }
        _ => link.act("hide", json!({})),
    }
}

// ───────────────────────── drawing ─────────────────────────

fn state_tone(state: &str, editing: bool) -> Hsla {
    if editing {
        return c(FACE);
    }
    match state {
        "ready" | "running" => c(OK),
        "paused" => c(WARN),
        "stopped" | "recording" => c(BAD),
        _ => c(DIM),
    }
}

/// The status line: what the macro does now.
fn status_text(t: &ToolPage, now: &Value) -> String {
    let state = s(now, "state");
    if state == "recording" {
        let account = now.pointer("/recording/account").and_then(Value::as_str).unwrap_or_default();
        return t.i18n.tv("panel.recording", &[("account", account)]);
    }
    if now.get("editingAreas").and_then(Value::as_bool).unwrap_or(false) {
        return t.t("panel.editing");
    }
    if state == "running" || state == "paused" {
        let runs = now.get("runs").and_then(Value::as_array).cloned().unwrap_or_default();
        let Some(first) = runs.first() else { return t.t("panel.ready") };
        let name = s(first, "name");
        let one = match s(first, "status").as_str() {
            "paused" => t.i18n.tv("panel.paused", &[("name", &name), ("reason", &s(first, "reason"))]),
            "waiting" => t.i18n.tv("panel.waits", &[("name", &name)]),
            "recovering" => t.i18n.tv("panel.unstuck", &[("name", &name)]),
            _ => t.i18n.tv("panel.runs", &[("name", &name)]),
        };
        return if runs.len() > 1 { format!("{one} · {}", t.i18n.tv("panel.more", &[("n", &(runs.len() - 1).to_string())])) } else { one };
    }
    if state == "stopped" {
        let reason = now.pointer("/lastError/text").and_then(Value::as_str).unwrap_or_default();
        return t.i18n.tv("panel.stopped", &[("reason", reason)]);
    }
    if state == "noGame" {
        return t.t("panel.noGame");
    }
    t.t("panel.ready")
}

fn dur_text(t: &ToolPage, secs: i64) -> String {
    if secs < 60 {
        return t.i18n.tv("macro.dur.s", &[("n", &secs.to_string())]);
    }
    let (h, m, r) = (secs / 3600, secs % 3600 / 60, secs % 60);
    let mut v = Vec::new();
    if h > 0 {
        v.push(t.i18n.tv("macro.dur.h", &[("n", &h.to_string())]));
    }
    if m > 0 {
        v.push(t.i18n.tv("macro.dur.min", &[("n", &m.to_string())]));
    }
    if r > 0 {
        v.push(t.i18n.tv("macro.dur.s", &[("n", &r.to_string())]));
    }
    v.join(" ")
}

fn key_name(t: &ToolPage, k: &str) -> String {
    if k.is_empty() {
        return "—".into();
    }
    if matches!(k, "MouseLeft" | "MouseRight" | "MouseMiddle") { t.t(&format!("macro.mk.{k}")) } else { crate::pages::macro_form::key_cap(k) }
}

/// A small button of the panel (.pn .btn.sm: 28 high); `off` looks and acts as not available but keeps its tip.
fn small(id: impl Into<ElementId>, words: String, off: bool, tip: Option<String>) -> Button {
    let b = Button::new(id, words).small();
    let b = match tip {
        Some(t) => b.tip(t),
        None => b,
    };
    b.disabled(off)
}

/// Run or Stop: filled, as wide as the panel (the pill's is small).
fn run_button(t: &ToolPage, p: &Panel, wide: bool, cx: &mut Context<ToolPage>) -> AnyElement {
    let running = p.running();
    let state = p.state();
    let blocked = match state.as_str() {
        "recording" => Some(t.t("panel.recordingWait")),
        "noGame" => Some(t.t("panel.noGame")),
        _ => None,
    };
    let off = !running && blocked.is_some();
    let link = t.link;
    let (ground, over, ink) = if running { (c(BAD), mix(c(BAD), c(WHITE), 0.12), c(WHITE)) } else { (c(FACE), c(FACE_HI), c(INK)) };
    let hotkey = p.data.pointer("/macro/hotkey").and_then(Value::as_i64).filter(|_| p.now().get("hotkeysOn").and_then(Value::as_bool).unwrap_or(false)).map(|n| format!("Ctrl+Alt+{n}"));
    let words = t.t(if running { "macro.stop" } else { "macro.run" });
    let mut b = hot(if wide { "pn-run" } else { "pn-run-pill" })
        .tones(ground, over)
        .h(px(if wide { 40. } else { 28. }))
        .px(px(if wide { 12. } else { 10. }))
        .flex()
        .flex_none()
        .items_center()
        .gap(px(if wide { 8. } else { 6. }))
        .rounded(px(R_S + 1.))
        .text_color(ink)
        .text_size(px(if wide { T_M } else { T_S }))
        .font_weight(FontWeight::SEMIBOLD)
        .child(icon(if running { "stop" } else { "play" }, if wide { 16. } else { 13. }, ink))
        .child(div().when(wide, |d| d.flex_1()).child(words));
    if wide {
        b = b.w_full();
        if let Some(k) = hotkey {
            b = b.child(div().px(px(6.)).py(px(1.)).rounded(px(4.)).bg(ink.opacity(0.14)).text_size(px(T_XS)).font_weight(FontWeight::SEMIBOLD).child(k));
        }
    }
    if off {
        b = b.opacity(0.55);
    } else {
        b = b.cursor_pointer();
    }
    if let Some(tip) = blocked.filter(|_| !running) {
        let tip: SharedString = tip.into();
        b = b.tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into());
    }
    b.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(move |t, _, _, cx| {
            let Some(p) = panel(t) else { return };
            if p.running() {
                link.act("stop", json!({}));
            } else if !off {
                link.act("run", json!({}));
            }
            cx.notify();
        }))
        .into_any_element()
}

pub fn render(t: &ToolPage, window: &mut Window, cx: &mut Context<ToolPage>) -> AnyElement {
    let Body::Panel(p) = &t.body else { return div().into_any_element() };
    if p.data.get("macro").is_none_or(Value::is_null) || p.now().is_null() {
        return div().into_any_element();
    }
    let link = t.link;
    let name = p.data.pointer("/macro/name").and_then(Value::as_str).unwrap_or_default().to_string();
    let light = div().size(px(8.)).flex_none().rounded_full().bg(state_tone(&p.state(), p.editing()));
    let from = Rc::new(Cell::new(None));

    // Leaving it after a change hands the keyboard back to the game; coming back in keeps it here.
    let hover = cx.listener(|t, over: &bool, _, cx| {
        let Some(p) = panel(t) else { return };
        if *over {
            p.away = None;
            return;
        }
        if !p.dirty {
            return;
        }
        p.away = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(AWAY_AFTER)).await;
            let _ = this.update(cx, |t, _| {
                let link = t.link;
                if let Some(p) = panel(t) {
                    if p.holds() || p.menu.is_some() {
                        return;
                    }
                    p.dirty = false;
                    link.act("away", json!({}));
                }
            });
        }));
    });
    // A press anywhere closes an open list (its rows and its button keep theirs) and stops waiting for a key.
    let press_out = cx.listener(|t, _: &MouseDownEvent, _, cx| {
        let link = t.link;
        if let Some(p) = panel(t) {
            if p.list.is_some() || p.waiting_key.is_some() || p.menu.is_some() {
                p.close_list(link);
                p.waiting_key = None;
                p.menu = None;
                cx.notify();
            }
        }
    });
    let title_menu = cx.listener(|t, e: &MouseDownEvent, _, cx| {
        let link = t.link;
        if let Some(p) = panel(t) {
            p.close_list(link);
            p.menu = Some((e.position, 0));
        }
        cx.stop_propagation();
        cx.notify();
    });

    if p.pill {
        p.natural.set(0.);
        let fold_out = cx.listener(|t, e: &MouseDownEvent, _, cx| {
            if e.click_count == 2 {
                if let Some(p) = panel(t) {
                    p.pill = false;
                    p.menu = None;
                }
                cx.notify();
            }
        });
        let pill = drag_row(div().id("pn-pill"), link, from)
            .w(px(WIDTH))
            .h(px(40.))
            .flex()
            .items_center()
            .gap(px(8.))
            .pl(px(12.))
            .pr(px(4.))
            .on_hover(hover)
            .on_mouse_down(MouseButton::Left, press_out)
            .on_mouse_down(MouseButton::Left, fold_out)
            .on_mouse_down(MouseButton::Right, title_menu)
            .child(light)
            .child(div().flex_1().min_w_0().truncate().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).child(name))
            .child(run_button(t, p, false, cx))
            .child(title_icon("pn-unfold", "chevronDown", t.t("panel.unfold")).on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()).on_click(cx.listener(|t, _, _, cx| {
                if let Some(p) = panel(t) {
                    p.pill = false;
                    p.menu = None;
                }
                cx.notify();
            })))
            .children(p.menu.map(|(at, active)| menu(t, at, active, cx)));
        return pill.into_any_element();
    }

    let tabs: Vec<String> = strs(p.data.pointer("/macro/tabs")).into_iter().collect::<Vec<_>>();
    let tabs = if tabs.len() >= 2 { tabs } else { Vec::new() };
    let shown_tab = p.tab.clone().filter(|x| tabs.contains(x)).or_else(|| tabs.first().cloned());

    let head = drag_row(div().id("pn-title"), link, from)
        .h(px(36.))
        .flex_none()
        .flex()
        .items_center()
        .gap(px(8.))
        .pl(px(12.))
        .pr(px(4.))
        .border_b_1()
        .border_color(c(LINE))
        .on_mouse_down(MouseButton::Right, title_menu)
        .child(light)
        .child(div().flex_1().min_w_0().truncate().text_size(px(T_L)).font_weight(FontWeight::SEMIBOLD).child(name))
        .child({
            let tip: SharedString = t.t("macro.betaTip").into();
            div()
                .id("pn-beta")
                .flex_none()
                .px(px(6.))
                .py(px(1.))
                .rounded(px(4.))
                .bg(ca(WARN, 0.16))
                .text_color(c(WARN))
                .text_size(px(T_XS))
                .font_weight(FontWeight::SEMIBOLD)
                .child(t.t("panel.beta"))
                .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
        })
        .child(title_icon("pn-fold", "chevronUp", t.t("panel.fold")).on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()).on_click(cx.listener(|t, _, _, cx| {
            let link = t.link;
            if let Some(p) = panel(t) {
                p.close_list(link);
                p.pill = true;
                p.menu = None;
            }
            cx.notify();
        })))
        .child(title_icon("pn-hide", "x", t.t("panel.hide")).on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()).on_click(move |_, _, _| link.act("hide", json!({}))));

    // The status line: what it does now, or a refused change, or "Settings reset · Undo"; "Fix" after a stop.
    let now = p.now().clone();
    let mut status = div().flex().items_baseline().min_w_0().min_h(px(18.)).text_size(px(T_S)).line_height(px(18.)).text_color(c(TEXT_2)).whitespace_nowrap();
    let link_button = |id: &'static str, words: String| {
        div()
            .id(id)
            .flex_none()
            .text_color(c(TEXT))
            .font_weight(FontWeight::SEMIBOLD)
            .underline()
            .cursor_pointer()
            .hover(|d| d.text_color(c(FACE)))
            .child(words)
    };
    let sep = || div().flex_none().px(px(5.)).text_color(c(MUTED)).child("·");
    match &p.status {
        Some((text, true, _)) => status = status.text_color(c(BAD)).child(div().min_w_0().truncate().child(text.clone())),
        Some((text, false, _)) => {
            status = status.child(div().min_w_0().truncate().child(text.clone()));
            if now.get("undoable").and_then(Value::as_bool).unwrap_or(false) {
                status = status.child(sep()).child(link_button("pn-undo", t.t("panel.undo")).on_click(cx.listener(move |t, _, _, cx| {
                    link.act("undoReset", json!({}));
                    if let Some(p) = panel(t) {
                        p.status = None;
                    }
                    cx.notify();
                })));
            }
        }
        None => {
            let said = status_text(t, &now);
            let tip: SharedString = said.clone().into();
            status = status.child(div().id("pn-say").min_w_0().truncate().child(said).tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()));
            if s(&now, "state") == "stopped" && now.get("lastError").is_some_and(Value::is_object) {
                let step = now.pointer("/lastError/step").cloned().unwrap_or(Value::Null);
                let section = now.pointer("/lastError/section").cloned().unwrap_or(Value::Null);
                status = status.child(sep()).child(link_button("pn-fix", t.t("panel.fix")).on_click(move |_, _, _| link.act("edit", json!({ "step": step, "section": section }))));
            }
        }
    }

    // The counters, and the clock of a run going on.
    let counters: Vec<(String, String)> = p
        .data
        .pointer("/macro/counters")
        .and_then(Value::as_array)
        .map(|l| l.iter().map(|c| (s(c, "name"), s(c, "label"))).filter(|(n, _)| now.get("counters").and_then(|m| m.get(n)).is_some()).collect())
        .unwrap_or_default();
    let started = now.get("startedAt").and_then(Value::as_str).and_then(crate::time::iso_secs).filter(|_| p.running());
    let chip = |label: Option<String>, value: String, clock: bool| {
        div()
            .h(px(22.))
            .max_w_full()
            .px(px(8.))
            .flex()
            .items_center()
            .gap(px(6.))
            .rounded_full()
            .bg(c(RAISED))
            .border_1()
            .border_color(c(LINE))
            .text_size(px(T_S))
            .text_color(c(TEXT_2))
            .when(clock, |d| d.child(icon("clock", 13., c(TEXT_2))))
            .children(label.map(|l| div().min_w_0().truncate().child(l)))
            .child(div().text_color(c(TEXT)).font_weight(FontWeight::SEMIBOLD).child(value))
    };
    let chips = (!counters.is_empty() || started.is_some()).then(|| {
        div()
            .flex()
            .flex_wrap()
            .gap(px(6.))
            .children(counters.iter().map(|(n, l)| {
                let v = now.get("counters").and_then(|m| m.get(n)).map(|v| v.to_string()).unwrap_or_default();
                chip(Some(l.clone()), v, false)
            }))
            .children(started.map(|at| chip(None, crate::time::clock(crate::time::now_secs() - at), true)))
    });

    let tab_row = (!tabs.is_empty()).then(|| {
        crate::kit::slide("pn-tabs", div().id("pn-tabs").w_full().h(px(32.)).flex().gap(px(2.)).p(px(2.)).rounded(px(R_M)).bg(c(INSET)).border_1().border_color(c(LINE)).overflow_x_scroll().children(tabs.iter().enumerate().map(|(i, x)| {
            let on = Some(x) == shown_tab.as_ref();
            let pick = x.clone();
            let tip: SharedString = x.clone().into();
            segment(("pn-tab", i), on)
                .h(px(26.))
                .px(px(10.))
                .max_w(px(132.))
                .text_size(px(T_S))
                .child(div().truncate().child(x.clone()))
                .flex_none()
                .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
                .on_click(cx.listener(move |t, _, _, cx| {
                    if let Some(p) = panel(t) {
                        p.tab = Some(pick.clone());
                        TABS.with(|m| m.borrow_mut().insert(p.id.clone(), pick.clone()));
                    }
                    cx.notify();
                }))
        }))
    )});

    let top = div().flex_none().flex().flex_col().gap(px(8.)).pt(px(12.)).px(px(12.)).pb(px(6.)).child(run_button(t, p, true, cx)).child(status).children(chips).children(tab_row);

    // The rows of the tab shown (all of them without tabs).
    let all = options(&p.data);
    let shown_rows: Vec<&Value> = all
        .iter()
        .copied()
        .filter(|o| {
            if tabs.is_empty() {
                return true;
            }
            let own = o.get("tab").and_then(Value::as_str).filter(|x| tabs.iter().any(|y| y == x)).map(str::to_string).or_else(|| tabs.first().cloned());
            own == shown_tab
        })
        .collect();
    let natural = p.hidden_rows.clone();
    let seen_h = Rc::new(Cell::new(0f32));
    let seen_w = seen_h.clone();
    let rows = div()
        .id("pn-rows")
        .relative()
        .flex_shrink_1()
        .min_h_0()
        .overflow_y_scroll()
        .child(canvas(move |b, _, _| seen_h.set(f32::from(b.size.height)), |_, _, _, _| {}).absolute().inset_0())
        .child(
            div()
                .relative()
                .pt(px(2.))
                .px(px(12.))
                .pb(px(8.))
                .when(shown_rows.is_empty(), |d| d.hidden())
                .children(shown_rows.iter().enumerate().map(|(i, o)| row(t, p, o, i == 0, window, cx)))
                .child(
                    canvas(
                        move |b, _, _| natural.set((f32::from(b.size.height) - seen_w.get()).max(0.)),
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .inset_0(),
                ),
        );

    let has_settings = all.iter().any(|o| s(o, "kind") != "heading");
    let areas = p.data.pointer("/macro/areasCount").and_then(Value::as_i64).unwrap_or(0);
    let foot = (areas > 0 || has_settings).then(|| {
        let editing = p.editing();
        let has_window = p.has_window();
        let changed = now.get("changed").and_then(Value::as_bool).unwrap_or(false);
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(8.))
            .pt(px(8.))
            .px(px(12.))
            .pb(px(12.))
            .border_t_1()
            .border_color(c(LINE))
            .when(areas > 0, |d| {
                let off = !editing && !has_window;
                let b = small("pn-areas", t.t(if editing { "panel.editingStop" } else { "panel.areas" }), off, off.then(|| t.t("panel.openGame")));
                let b = if editing { b.primary() } else { b };
                d.child(b.on_click(move |_, _, _| link.act("areas", json!({}))))
            })
            .when(has_settings, |d| {
                d.child(div().ml_auto().child(small("pn-reset", t.t("panel.reset"), !changed, (!changed).then(|| t.t("panel.resetNone"))).quiet().on_click(cx.listener(move |t, _, _, cx| {
                    let done = t.t("panel.resetDone");
                    link.act("reset", json!({}));
                    if let Some(p) = panel(t) {
                        p.say(done, false, RESET_FOR, cx);
                    }
                    cx.notify();
                }))))
            })
    });

    // (Never taller than its window: the rows scroll; the window is sized to what it would be, rows not scrolled.)
    let tall = f32::from(window.viewport_size().height);
    let (natural, hidden) = (p.natural.clone(), p.hidden_rows.clone());
    let end = div().flex_none().w_full().h_0().relative().child(canvas(move |b, _, _| natural.set(f32::from(b.origin.y) + hidden.get()), |_, _, _, _| {}).absolute().size_0());
    div()
        .id("pn")
        .w(px(WIDTH))
        .max_h(px(tall.max(120.)))
        .flex()
        .flex_col()
        .overflow_hidden()
        .text_size(px(T_M))
        .on_hover(hover)
        .on_mouse_down(MouseButton::Left, press_out)
        .on_mouse_up(MouseButton::Left, cx.listener(|t, _, _, cx| release(t, cx)))
        .child(head)
        .child(top)
        .child(rows)
        .children(foot)
        .child(end)
        .children(p.menu.map(|(at, active)| menu(t, at, active, cx)))
        .into_any_element()
}

/// The title row's menu (right-click): Snap to game, Edit macro, Hide.
fn menu(t: &ToolPage, at: Point<Pixels>, active: usize, cx: &mut Context<ToolPage>) -> AnyElement {
    let items = [("screens", t.t("panel.snap")), ("edit", t.t("panel.editMacro")), ("eyeOff", t.t("panel.hide"))];
    let list = div()
        .id("pn-menu")
        .w(px(190.))
        .p(px(4.))
        .flex()
        .flex_col()
        .rounded(px(R_M))
        .border_1()
        .border_color(c(LINE))
        .bg(solid_panel())
        .shadow_lg()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .children(items.into_iter().enumerate().map(|(i, (mark, words))| {
            let on = i == active;
            div()
                .id(("pn-menu-row", i))
                .h(px(34.))
                .px(px(10.))
                .flex()
                .items_center()
                .gap(px(10.))
                .rounded(px(R_S))
                .cursor_pointer()
                .text_color(if on { c(TEXT) } else { c(TEXT_2) })
                .when(on, |d| d.bg(ca(WHITE, 0.07)))
                .child(icon(mark, 16., if on { c(TEXT) } else { c(TEXT_2) }))
                .child(words)
                .on_mouse_move(cx.listener(move |t, _, _, cx| {
                    if let Some(p) = panel(t) {
                        if let Some((at, a)) = p.menu {
                            if a != i {
                                p.menu = Some((at, i));
                                cx.notify();
                            }
                        }
                    }
                }))
                .on_click(cx.listener(move |t, _, _, cx| {
                    let link = t.link;
                    if let Some(p) = panel(t) {
                        p.menu = None;
                        menu_run(p, link, i);
                    }
                    cx.notify();
                }))
        }));
    at_pointer(at, list)
}

/// One setting: its label (cut with … and whole in its tip) left, its control right; a control wider than 55 % of the
/// row goes under the label at full width. A refused change says why under it for a few seconds; a feature's switch
/// says what it does now while the macro runs.
fn row(t: &ToolPage, p: &Panel, o: &Value, first: bool, window: &mut Window, cx: &mut Context<ToolPage>) -> AnyElement {
    let name = s(o, "name");
    let kind = s(o, "kind");
    let label = s(o, "label");
    if kind == "heading" {
        return div()
            .mt(px(if first { 4. } else { 10. }))
            .mb(px(2.))
            .when(!first, |d| d.pt(px(10.)).border_t_1().border_color(c(LINE)))
            .truncate()
            .text_size(px(T_S))
            .font_weight(FontWeight::BOLD)
            .text_color(c(TEXT_2))
            .child(label)
            .into_any_element();
    }
    let flex = kind == "choice" || kind == "list";
    let stack = !flex && p.stacked.borrow().contains(&name);
    let control: AnyElement = match kind.as_str() {
        "number" => stepper(t, p, o, cx),
        "check" => {
            let on = f(o, "value").is_some_and(|v| v != 0.);
            let link = t.link;
            let n = name.clone();
            Switch::new(SharedString::from(format!("pn-sw-{name}")), on)
                .on_toggle(cx.listener(move |t, v: &bool, _, cx| {
                    link.act("set", json!({ "name": n, "value": if *v { 1 } else { 0 } }));
                    if let Some(p) = panel(t) {
                        p.dirty = true;
                    }
                    cx.notify();
                }))
                .into_any_element()
        }
        "choice" | "list" => select(t, p, o, kind == "list", window, cx),
        "key" => key_field(t, p, o, cx),
        "colour" => colour_field(t, o, cx),
        "point" => spot_field(t, p, o, cx),
        _ => div().text_size(px(T_S)).text_color(c(TEXT_2)).child(s(o, "shown")).into_any_element(),
    };
    // The control's own width against the row's, read as drawn: stacked from the next frame when it doesn't fit.
    let stacked = p.stacked.clone();
    let row_w = Rc::new(Cell::new(0f32));
    let rw = row_w.clone();
    let key = name.clone();
    let measure = canvas(
        move |b, window, _| {
            let fits = f32::from(b.size.width) <= rw.get() * 0.55 + 0.5;
            let mut st = stacked.borrow_mut();
            if rw.get() > 0. && fits == st.contains(&key) {
                if fits {
                    st.remove(&key);
                } else {
                    st.insert(key.clone());
                }
                window.refresh();
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0();
    let inner = div().relative().flex().items_center().gap(px(6.)).flex_none().when(flex, |d| d.flex_1().min_w_0()).child(control).when(!flex, |d| d.child(measure));
    let hint = p.hints.get(&name).map(|h| h.0.clone());
    let feature = (kind == "check").then(|| p.now().get("features").and_then(|m| m.get(&name)).cloned()).flatten().filter(|v| v.is_object());
    let tip: SharedString = label.clone().into();
    div()
        .relative()
        .flex()
        .flex_wrap()
        .items_center()
        .gap_x(px(8.))
        .min_h(px(36.))
        .when(stack, |d| d.py(px(4.)).gap_y(px(4.)))
        .child(canvas(move |b, _, _| row_w.set(f32::from(b.size.width)), |_, _, _, _| {}).absolute().inset_0())
        .child(
            div()
                .id(SharedString::from(format!("pn-label-{name}")))
                .min_w_0()
                .truncate()
                .text_color(c(TEXT))
                // (45 % of the row, as .pn-label; a share of a width Taffy isn't given here came out as the words' own width.)
                .map(|d| if stack { d.w_full() } else { d.w(px((WIDTH - 24.) * 0.45)).flex_none() })
                .child(label)
                .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into()),
        )
        .child(div().flex_1().min_w_0().flex().map(|d| if stack { d.w_full().justify_start() } else { d.justify_end() }).child(inner))
        .children(hint.map(|h| div().w_full().mb(px(6.)).text_size(px(T_S)).text_color(c(BAD)).child(h)))
        .children(feature.filter(|_| !p.hints.contains_key(&name)).map(|v| feature_line(t, &v)))
        .into_any_element()
}

/// Under a feature's switch while the macro runs: what it does now.
fn feature_line(t: &ToolPage, v: &Value) -> Div {
    let state = s(v, "state");
    let text = match state.as_str() {
        "playing" => t.i18n.tv("panel.feat.playing", &[("n", &v.get("step").map(|x| x.to_string()).unwrap_or_default()), ("of", &v.get("of").map(|x| x.to_string()).unwrap_or_default())]),
        "due" => t.t("panel.feat.due"),
        "failed" => t.i18n.tv("panel.feat.failed", &[("why", &s(v, "why"))]),
        "next" => {
            let left = v.get("at").and_then(Value::as_str).and_then(crate::time::iso_secs).map(|at| (at - crate::time::now_secs()).max(0)).unwrap_or(0);
            t.i18n.tv("panel.feat.next", &[("d", &dur_text(t, if left >= 60 { (left + 59) / 60 * 60 } else { left }))])
        }
        _ => t.t("panel.feat.round"),
    };
    let tone = match state.as_str() {
        "playing" => c(TEXT),
        "failed" => c(WARN),
        _ => c(MUTED),
    };
    div().w_full().mt(px(-6.)).mb(px(6.)).text_size(px(T_S)).text_color(tone).child(text)
}

/// [−][ value unit ][+]: held repeats, the wheel moves it, the value is typed; the core is told once, steps added up.
fn stepper(t: &ToolPage, p: &Panel, o: &Value, cx: &mut Context<ToolPage>) -> AnyElement {
    let name = s(o, "name");
    let n = Num::of(o);
    let moved = p.moved.get(&name).copied().unwrap_or(0);
    let shown = if moved != 0 { n.fmt(n.value_at(moved)) } else { s(o, "shown") };
    let now = if moved != 0 { n.value_at(moved) } else { n.raw };
    let range = (o.get("minShown").and_then(Value::as_str).is_some_and(|x| !x.is_empty()) && o.get("maxShown").and_then(Value::as_str).is_some_and(|x| !x.is_empty())).then(|| {
        let min = s(o, "minShown");
        let min = if !n.suffix.is_empty() && min.ends_with(&n.suffix) { min[..min.len() - n.suffix.len()].to_string() } else { min };
        t.i18n.tv("panel.range", &[("min", &min), ("max", &s(o, "maxShown"))])
    });
    let step_button = |dir: i64| {
        let off = if dir < 0 { now <= n.min } else { now >= n.max };
        let nm = name.clone();
        let mark: AnyElement = if dir < 0 { div().w(px(10.)).h(px(1.5)).rounded_full().bg(c(TEXT_2)).into_any_element() } else { icon("plus", 14., c(TEXT_2)).into_any_element() };
        div()
            .id(SharedString::from(format!("pn-step-{name}-{dir}")))
            .w(px(28.))
            .h_full()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .when(off, |d| d.opacity(0.35))
            .when(!off, |d| d.cursor_pointer().hover(|d| d.bg(ca(WHITE, 0.07))))
            .child(mark)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |t, e: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    if !off {
                        press(t, nm.clone(), dir * if e.modifiers.shift { 10 } else { 1 }, cx);
                    }
                }),
            )
            .on_mouse_up(MouseButton::Left, cx.listener(|t, _, _, cx| release(t, cx)))
    };
    let typing = p.typing.as_ref().filter(|x| x.0 == name).map(|x| x.2.clone());
    let nm = name.clone();
    let value: AnyElement = match typing {
        Some(input) => div().w(px(74.)).h_full().px(px(4.)).flex().items_center().bg(c(RAISED)).border_x_1().border_color(c(LINE)).child(Input::new(&input).appearance(false).text_size(px(T_M))).into_any_element(),
        None => {
            let sh = shown.clone();
            div()
                .id(SharedString::from(format!("pn-num-{name}")))
                .w(px(74.))
                .h_full()
                .px(px(4.))
                .flex()
                .items_center()
                .justify_center()
                .border_x_1()
                .border_color(c(LINE))
                .text_size(px(T_M))
                .cursor_text()
                .child(div().truncate().child(shown))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |t, _, window, cx| begin_typing(t, nm.clone(), sh.clone(), window, cx)))
                .into_any_element()
        }
    };
    let nw = name.clone();
    let mut box_ = div()
        .id(SharedString::from(format!("pn-stepper-{name}")))
        .h(px(30.))
        .flex()
        .rounded(px(R_S))
        .border_1()
        .border_color(if p.typing.as_ref().is_some_and(|x| x.0 == name) { ca(TEXT, 0.78) } else { ca(TEXT, 0.26) })
        .bg(c(INSET))
        .overflow_hidden()
        .text_color(c(TEXT))
        .on_scroll(cx.listener(move |t, e: &ScrollEvent, window, cx| {
            let dy = f32::from(e.delta.pixel_delta(window.line_height()).y);
            if dy == 0. {
                return;
            }
            let by = if dy > 0. { 1 } else { -1 } * if e.modifiers.shift { 10 } else { 1 };
            wheel(t, nw.clone(), by, cx);
        }))
        .child(step_button(-1))
        .child(value)
        .child(step_button(1));
    if let Some(r) = range {
        let tip: SharedString = r.into();
        box_ = box_.tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into());
    }
    box_.into_any_element()
}

/// The choice (or "2 of 5") in a box opening its list (≤ 8 rows, then scrolling); a second click closes it.
fn select(t: &ToolPage, p: &Panel, o: &Value, many: bool, window: &mut Window, cx: &mut Context<ToolPage>) -> AnyElement {
    let name = s(o, "name");
    let choices = strs(o.get("choices"));
    let open = p.list.as_ref().filter(|l| l.name == name);
    let words = if many {
        let shown: Vec<String> = match open {
            Some(l) => l.ticked.clone(),
            None => strs(o.get("value")),
        };
        let n = choices.iter().filter(|c| shown.contains(c)).count();
        t.i18n.tv("panel.of", &[("n", &n.to_string()), ("all", &choices.len().to_string())])
    } else {
        Some(s(o, "value")).filter(|v| !v.is_empty()).unwrap_or_else(|| "—".into())
    };
    let is_open = open.is_some();
    let nm = name.clone();
    let sent = if many { strs(o.get("value")) } else { Vec::new() };
    let now = s(o, "value");
    let button = div()
        .id(SharedString::from(format!("pn-select-{name}")))
        .w_full()
        .min_w_0()
        .h(px(30.))
        .pl(px(10.))
        .pr(px(8.))
        .flex()
        .items_center()
        .gap(px(6.))
        .rounded(px(R_S))
        .border_1()
        .border_color(if is_open { ca(TEXT, 0.78) } else { ca(TEXT, 0.26) })
        .hover(|d| d.border_color(ca(TEXT, 0.4)))
        .bg(c(INSET))
        .text_color(c(TEXT))
        .cursor_pointer()
        .child(div().flex_1().min_w_0().truncate().child(words))
        .child(icon(if is_open { "chevronUp" } else { "chevronDown" }, 14., c(MUTED)))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(move |t, _, _, cx| {
            let link = t.link;
            let Some(p) = panel(t) else { return };
            if p.list.as_ref().is_some_and(|l| l.name == nm) {
                p.close_list(link);
            } else {
                p.close_list(link);
                p.menu = None;
                let active = if many { 2 } else { choices.iter().position(|c| *c == now).unwrap_or(0) };
                p.list = Some(List { name: nm.clone(), many, ticked: sent.clone(), active, typed: (String::new(), std::time::Instant::now()) });
            }
            cx.notify();
        }));
    let mut wrap = div().relative().w_full().min_w_0().child(button);
    if let Some(l) = open {
        let all_none = [t.t("panel.all"), t.t("panel.none")];
        let rows = list_rows(&p.data, l, &all_none);
        let cap = rows.len().min(MAX_ROWS + if many { 1 } else { 0 }) as f32 * ROW + 10.;
        let room = (f32::from(window.viewport_size().height) - 12.).max(ROW + 10.);
        let now = s(o, "value");
        let list = div()
            .id(SharedString::from(format!("pn-list-{name}")))
            .min_w(px(150.))
            .max_w(px(WIDTH - 12.))
            .max_h(px(cap.min(room)))
            .overflow_y_scroll()
            .p(px(4.))
            .flex()
            .flex_col()
            .rounded(px(R_M))
            .border_1()
            .border_color(c(LINE))
            .bg(solid_panel())
            .shadow_lg()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .children(rows.iter().enumerate().map(|(i, r)| {
                let on = i == l.active;
                let mark: Option<AnyElement> = if many {
                    Some(match i {
                        0 => icon("check", 15., c(TEXT_2)).into_any_element(),
                        1 => icon("x", 15., c(TEXT_2)).into_any_element(),
                        _ => tick_box(l.ticked.contains(r)).into_any_element(),
                    })
                } else {
                    None
                };
                let checked = !many && *r == now;
                let tip: SharedString = r.clone().into();
                div()
                    .when(many && i == 2, |d| d.child(div().mx(px(4.)).my(px(4.)).h(px(1.)).bg(c(LINE))))
                    .child(
                        div()
                            .id(("pn-list-row", i))
                            .h(px(ROW))
                            .px(px(10.))
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .rounded(px(R_S))
                            .cursor_pointer()
                            .text_color(c(TEXT))
                            .when(checked, |d| d.font_weight(FontWeight::SEMIBOLD))
                            .when(on, |d| d.bg(ca(WHITE, 0.07)))
                            .children(mark)
                            .child(div().flex_1().min_w_0().truncate().child(r.clone()))
                            .when(checked, |d| d.child(icon("check", 15., c(FACE))))
                            .tooltip(move |_, cx| cx.new(|_| Tip(tip.clone())).into())
                            .on_mouse_move(cx.listener(move |t, _, _, cx| {
                                if let Some(l) = panel(t).and_then(|p| p.list.as_mut()) {
                                    if l.active != i {
                                        l.active = i;
                                        cx.notify();
                                    }
                                }
                            }))
                            .on_click(cx.listener(move |t, _, _, cx| list_pick(t, i, cx))),
                    )
            }));
        wrap = wrap.child(float(PopAt::Under, 2, div().mt(px(4.)).child(list)));
    }
    wrap.into_any_element()
}

fn tick_box(on: bool) -> Div {
    div()
        .size(px(16.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.))
        .border_1()
        .border_color(if on { c(FACE) } else { ca(TEXT, 0.35) })
        .when(on, |d| d.bg(c(FACE)).child(icon("check", 12., crate::theme::on_text())))
}

/// The key in a cap and Change; while it waits the cap says "Press a key…"; Esc or a press elsewhere changes nothing.
fn key_field(t: &ToolPage, p: &Panel, o: &Value, cx: &mut Context<ToolPage>) -> AnyElement {
    let name = s(o, "name");
    let wait = p.waiting_key.as_deref() == Some(name.as_str());
    let cap = if wait { t.t("panel.pressKey") } else { key_name(t, &s(o, "value")) };
    let nm = name.clone();
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .child(
            div()
                .min_w(px(30.))
                .h(px(24.))
                .px(px(8.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(5.))
                .border_1()
                .border_color(if wait { ca(TEXT, 0.4) } else { ca(TEXT, 0.2) })
                .when(wait, |d| d.border_dashed())
                .bg(c(RAISED))
                .text_size(px(T_S))
                .font_weight(if wait { FontWeight::MEDIUM } else { FontWeight::SEMIBOLD })
                .text_color(if wait { c(TEXT_2) } else { c(TEXT) })
                .whitespace_nowrap()
                .child(cap),
        )
        .child(
            div().on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()).child(
                {
                    let b = Button::new(SharedString::from(format!("pn-key-{name}")), t.t("panel.change")).small();
                    if wait { b.primary() } else { b }
                }
                .on_click(cx.listener(move |t, _, window, cx| {
                    window.focus(&t.focus, cx);
                    if let Some(p) = panel(t) {
                        p.waiting_key = if p.waiting_key.as_deref() == Some(nm.as_str()) { None } else { Some(nm.clone()) };
                    }
                    cx.notify();
                })),
            ),
        )
        .into_any_element()
}

/// The colour's swatch, its hex, and Pick (on the game; on the screen when no game is open).
fn colour_field(t: &ToolPage, o: &Value, cx: &mut Context<ToolPage>) -> AnyElement {
    let name = s(o, "name");
    let hex = Some(s(o, "value")).filter(|v| v.starts_with('#')).unwrap_or_else(|| "#ffffff".into());
    let rgb = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0xffffff);
    let link = t.link;
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .child(div().size(px(18.)).flex_none().rounded(px(4.)).border_1().border_color(ca(TEXT, 0.2)).bg(rgb_colour(rgb)))
        .child(div().font_family("Cascadia Mono").text_size(px(T_S)).text_color(c(TEXT_2)).whitespace_nowrap().child(hex))
        .child(small(SharedString::from(format!("pn-colour-{name}")), t.t("panel.pick"), false, None).on_click(cx.listener(move |t, _, _, cx| {
            link.act("pickColour", json!({ "name": name }));
            if let Some(p) = panel(t) {
                p.dirty = true;
            }
            cx.notify();
        })))
        .into_any_element()
}

fn rgb_colour(rgb: u32) -> Hsla {
    gpui_kit::rgb(rgb).into()
}

/// Where on the game: a small map with the dot, the place in percent, and Pick spot (needs a game window).
fn spot_field(t: &ToolPage, p: &Panel, o: &Value, cx: &mut Context<ToolPage>) -> AnyElement {
    let name = s(o, "name");
    let v = o.get("value").filter(|v| v.is_object());
    let (x, y) = v.map(|v| (f(v, "x").unwrap_or(0.5), f(v, "y").unwrap_or(0.5))).unwrap_or((0.5, 0.5));
    let pct = |v: f64| format!("{} %", (v * 100.).round() as i64);
    let has = p.has_window();
    let link = t.link;
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .child(
            div()
                .relative()
                .w(px(24.))
                .h(px(16.))
                .flex_none()
                .rounded(px(3.))
                .border_1()
                .border_color(ca(TEXT, 0.2))
                .bg(c(INSET))
                .child(div().absolute().left(px(x as f32 * 22. - 2.)).top(px(y as f32 * 14. - 2.)).size(px(4.)).rounded_full().bg(c(FACE))),
        )
        .child(div().text_size(px(T_S)).text_color(c(TEXT_2)).whitespace_nowrap().child(format!("{}, {}", pct(x), pct(y))))
        .child(small(SharedString::from(format!("pn-spot-{name}")), t.t("panel.pickSpot"), !has, (!has).then(|| t.t("panel.openGame"))).on_click(cx.listener(move |t, _, _, cx| {
            link.act("pickPoint", json!({ "name": name }));
            if let Some(p) = panel(t) {
                p.dirty = true;
            }
            cx.notify();
        })))
        .into_any_element()
}
