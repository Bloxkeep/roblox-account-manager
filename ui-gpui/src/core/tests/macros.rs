// Tests/Bloxkeep.Tests/MacroSuite.cs: macros (Beta): the checks on a macro, the player on a made-up desktop (its own
// clock, windows that are only records: nothing is ever sent, focused or launched), the turn every window input waits
// for, the pictures' maths, TinyTask files, settings ("$o1") and areas ("@a1"), the panel's values and when it shows by
// itself, the dots and boxes drawn on the game (their maths only: no window is opened), the templates and "What it sees".
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use base64::Engine;
use serde_json::json;

use super::Checks;
use crate::core::base::{Cancel, Fail, Guid, Json, Res, Stamp, new_guid};
use crate::core::input_turn::{self, Lease};
use crate::core::macro_desk::{HeldInputs, MacroDesk, PersonAtFn, RawWorksFn};
use crate::core::macro_expr::{self, MacroExprError};
use crate::core::macro_follow::{self, MacroMarker, Pd, Pixels, Predictor};
use crate::core::macro_model::macro_rules::page_json::{self, Shape};
use crate::core::macro_model::{Macro, MacroKey, MacroRect, MacroSize, MacroStep, MacroTest, macro_keys, macro_rules};
use crate::core::macro_ocr::{Line, RectangleF, Word};
use crate::core::macro_outside::{self, IMacroOutside};
use crate::core::macro_panel::area_geometry::{self, Dot, Grip};
use crate::core::macro_panel::{MacroOption, MacroPanel, macro_options};
use crate::core::macro_player::{self, IMacroDesk, IMacroTurn, MacroPlayer, MacroWindow, Rng, RunRef, macro_geometry};
use crate::core::macro_recorder::{MacroEvent, macro_recording};
use crate::core::macro_values;
use crate::core::macro_vision;
use crate::core::macro_watch;
use crate::core::macro_templates;
use crate::core::win_kit::{Picture, Point, Rect};
use crate::core::{l, prefs};

pub async fn run(c: &mut Checks, _dir: &str) {
    input_turn::reset();
    validation(c);
    geometry(c);
    vision(c);
    tiny_task(c);
    values(c);
    follow_maths(c);
    turns(c);
    player(c);
    features(c);
    bounds(c);
    panels(c);
    templates(c);
    watch(c);
    input_turn::reset();
}

// ───────────────────────── helpers ─────────────────────────

const BLACK: u32 = 0xFF00_0000;
const LIME: u32 = 0xFF00_FF00;
const WHITE: u32 = 0xFFFF_FFFF;
const RED: u32 = 0xFFFF_0000;
const BLUE: u32 = 0xFF00_00FF;

fn msg(f: &Fail) -> String {
    f.message().to_string()
}

fn parse(text: &str) -> Json {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("bad JSON in a check: {e}: {text}"))
}

fn read(steps: &str, extra: &str) -> Macro {
    let text = format!(r#"{{"name":"Test","jitterMs":0,"jitterPx":0{extra},"steps":{steps}}}"#);
    macro_rules::read(&parse(&text)).unwrap_or_else(|e| panic!("macro not read: {e:?}"))
}

fn check_macro(m: &mut Macro) -> Option<String> {
    macro_rules::check(m, &HashSet::new(), &[]).err().map(|e| msg(&e))
}

/// A checked macro with an id, ready to queue.
pub(crate) fn mk(steps: &str, extra: &str) -> Macro {
    let mut m = read(steps, extra);
    if let Some(e) = check_macro(&mut m) {
        println!("NOTE  a macro the checks use was refused: {e}");
    }
    m.id = new_guid();
    m
}

fn check_of(steps: &str, extra: &str) -> Option<String> {
    let text = format!(r#"{{"name":"Test","jitterMs":0,"jitterPx":0{extra},"steps":{steps}}}"#);
    match macro_rules::read(&parse(&text)) {
        Ok(mut m) => check_macro(&mut m),
        Err(e) => Some(msg(&e)),
    }
}

fn page(m: &Macro) -> String {
    page_json::write(m, Shape::Macro).to_string()
}

fn page_step(s: &MacroStep) -> String {
    page_json::write(s, Shape::Step).to_string()
}

/// A black picture with coloured columns (x, width, colour).
fn bars(w: i32, h: i32, columns: &[(i32, i32, u32)]) -> Picture {
    let mut b = Picture::new(w, h);
    b.fill(BLACK);
    for &(x, bw, colour) in columns {
        fill(&mut b, Rect::new(x, 0, bw, h), colour);
    }
    b
}

fn fill(b: &mut Picture, r: Rect, colour: u32) {
    for y in r.y.max(0)..(r.y + r.height).min(b.height) {
        for x in r.x.max(0)..(r.x + r.width).min(b.width) {
            b.set(x, y, colour);
        }
    }
}

fn patches(w: i32, h: i32, rects: &[Rect]) -> Picture {
    let mut b = Picture::new(w, h);
    b.fill(BLACK);
    for &r in rects {
        fill(&mut b, r, LIME);
    }
    b
}

/// Text as Windows would read it: one line each, its words side by side.
fn said(lines: &[&str]) -> Vec<Line> {
    lines
        .iter()
        .enumerate()
        .map(|(k, line)| Line {
            words: line
                .split(' ')
                .enumerate()
                .map(|(i, w)| Word { text: w.to_string(), r#box: RectangleF { x: i as f32 * 60.0, y: k as f32 * 20.0, width: 50.0, height: 16.0 } })
                .collect(),
        })
        .collect()
}

pub(crate) fn lk<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

// ───────────────────────── a made-up desktop ─────────────────────────

#[derive(Default)]
struct FakeOutside {
    clip: Mutex<String>,
    files: Mutex<HashMap<String, String>>,
    web: Mutex<HashMap<String, String>>,
    posts: Mutex<Vec<String>>,
}

impl IMacroOutside for FakeOutside {
    fn clipboard(&self) -> Res<Option<String>> {
        Ok(Some(lk(&self.clip).clone()))
    }
    fn copy(&self, text: &str) -> Res<()> {
        *lk(&self.clip) = text.to_string();
        Ok(())
    }
    fn read_file(&self, name: &str) -> Res<Option<String>> {
        Ok(lk(&self.files).get(name).and_then(|f| f.split('\n').filter(|l| !l.trim().is_empty()).next_back().map(str::to_string)))
    }
    fn append_file(&self, name: &str, line: &str) -> Res<()> {
        let mut files = lk(&self.files);
        let now = match files.get(name) {
            Some(f) => format!("{f}\n{line}"),
            None => line.to_string(),
        };
        files.insert(name.to_string(), now);
        Ok(())
    }
    fn web(&self, url: &str, post: Option<&str>, _ct: &Cancel) -> Res<Option<String>> {
        if let Some(p) = post {
            lk(&self.posts).push(format!("{url} {p}"));
            return Ok(Some("ok".into()));
        }
        Ok(lk(&self.web).get(url).cloned())
    }
}

type Hook<T> = Arc<dyn Fn(&FakeDesk) -> T + Send + Sync>;

const START_MS: i64 = 1_767_225_600_000; // 2026-01-01 00:00 UTC

pub(crate) struct FakeDesk {
    me: Weak<FakeDesk>,
    clock: AtomicI64,
    log: Mutex<Vec<String>>,
    windows: Mutex<HashMap<Guid, MacroWindow>>,
    hold_now: Mutex<Option<Hook<Option<String>>>>,
    on_delay: Mutex<Option<Hook<()>>>,
    clear_at: Mutex<Option<Arc<dyn Fn(&FakeDesk, Point) -> bool + Send + Sync>>>,
    picture: Mutex<Option<Hook<Option<Picture>>>>,
    area: Mutex<Option<Arc<dyn Fn(&FakeDesk, Rect) -> Option<Picture> + Send + Sync>>>,
    /// When the person last touched the PC (ms on this clock); None: not since the start.
    person_at: Mutex<Option<Hook<Option<i64>>>>,
    /// A wait asked while this is true doesn't end (until its run is called off): the app stopped with a run halfway.
    pub(crate) suspend_when: Mutex<Option<Hook<bool>>>,
    suspended: AtomicBool,
    /// Giving the window back fails (a turn's end throwing). PORT: IMacroTurn::dispose can't fail in Rust; the end isn't logged.
    throw_on_end: AtomicBool,
    lines: Mutex<Option<Hook<Option<Vec<Vec<Line>>>>>>,
}

impl FakeDesk {
    fn new() -> Arc<FakeDesk> {
        Arc::new_cyclic(|me| FakeDesk {
            me: me.clone(),
            clock: AtomicI64::new(0),
            log: Mutex::new(Vec::new()),
            windows: Mutex::new(HashMap::new()),
            hold_now: Mutex::new(None),
            on_delay: Mutex::new(None),
            clear_at: Mutex::new(None),
            picture: Mutex::new(None),
            area: Mutex::new(None),
            person_at: Mutex::new(None),
            suspend_when: Mutex::new(None),
            suspended: AtomicBool::new(false),
            throw_on_end: AtomicBool::new(false),
            lines: Mutex::new(None),
        })
    }
    fn ms(&self) -> i64 {
        self.clock.load(Ordering::SeqCst)
    }
    fn add(&self, line: String) {
        lk(&self.log).push(line);
    }
    pub(crate) fn log(&self) -> Vec<String> {
        lk(&self.log).clone()
    }
    pub(crate) fn any(&self, part: &str) -> bool {
        lk(&self.log).iter().any(|l| l.contains(part))
    }
    fn count(&self, part: &str) -> usize {
        lk(&self.log).iter().filter(|l| l.contains(part)).count()
    }
    fn joined(&self) -> String {
        self.log().join(" | ")
    }
    fn on_delay(&self, f: impl Fn(&FakeDesk) + Send + Sync + 'static) {
        *lk(&self.on_delay) = Some(Arc::new(f));
    }
    fn hold_now(&self, f: impl Fn(&FakeDesk) -> Option<String> + Send + Sync + 'static) {
        *lk(&self.hold_now) = Some(Arc::new(f));
    }
    fn picture(&self, f: impl Fn(&FakeDesk) -> Option<Picture> + Send + Sync + 'static) {
        *lk(&self.picture) = Some(Arc::new(f));
    }
    fn area(&self, f: impl Fn(&FakeDesk, Rect) -> Option<Picture> + Send + Sync + 'static) {
        *lk(&self.area) = Some(Arc::new(f));
    }
    fn lines(&self, f: impl Fn(&FakeDesk) -> Option<Vec<Vec<Line>>> + Send + Sync + 'static) {
        *lk(&self.lines) = Some(Arc::new(f));
    }
    fn person_at(&self, f: impl Fn(&FakeDesk) -> Option<i64> + Send + Sync + 'static) {
        *lk(&self.person_at) = Some(Arc::new(f));
    }
    fn person_at_now(&self) -> Option<i64> {
        let f = lk(&self.person_at).clone();
        f.and_then(|f| f(self))
    }
}

impl IMacroDesk for FakeDesk {
    fn now(&self) -> Stamp {
        Stamp::from_unix_ms(START_MS + self.ms())
    }

    fn delay(&self, ms: i32, ct: &Cancel) -> Res<()> {
        let suspend = lk(&self.suspend_when).clone();
        if suspend.is_some_and(|f| f(self)) {
            // The C# returned a task that never ends; here the wait lasts until its run is called off.
            self.suspended.store(true, Ordering::SeqCst);
            let begun = Instant::now();
            while !ct.is_cancelled() && begun.elapsed() < Duration::from_secs(5) {
                std::thread::sleep(Duration::from_millis(5));
            }
            return Err(Fail::other("cancelled"));
        }
        self.clock.fetch_add(ms.max(0) as i64, Ordering::SeqCst);
        let hook = lk(&self.on_delay).clone();
        if let Some(f) = hook {
            f(self);
        }
        if ct.is_cancelled() {
            return Err(Fail::other("cancelled"));
        }
        Ok(())
    }

    fn hold(&self) -> Option<String> {
        let f = lk(&self.hold_now).clone();
        f.and_then(|f| f(self))
    }

    fn window(&self, account_id: Guid) -> Option<MacroWindow> {
        lk(&self.windows).get(&account_id).cloned()
    }

    fn begin_turn(&self, window: &MacroWindow, _ct: &Cancel) -> Res<Option<Arc<dyn IMacroTurn>>> {
        self.add(format!("turn {}", window.pid));
        let desk = self.me.upgrade().expect("the desk");
        let started = self.ms();
        Ok(Some(Arc::new(FakeTurn { desk, w: window.clone(), at: Mutex::new(Point::new(0, 0)), started, down: Mutex::new(Vec::new()) })))
    }

    fn picture(&self, _window: &MacroWindow, _turn: Option<&dyn IMacroTurn>) -> Option<Picture> {
        let f = lk(&self.picture).clone();
        f.and_then(|f| f(self))
    }

    fn ime_on(&self) -> bool {
        false
    }

    fn read(&self, _picture: &Picture, _area: Rect) -> Res<Option<Vec<Vec<Line>>>> {
        let f = lk(&self.lines).clone();
        Ok(f.and_then(|f| f(self)))
    }

    fn area(&self, _window: &MacroWindow, _turn: &dyn IMacroTurn, area: Rect) -> Option<Picture> {
        let f = lk(&self.area).clone();
        f.and_then(|f| f(self, area))
    }
}

struct FakeTurn {
    desk: Arc<FakeDesk>,
    w: MacroWindow,
    at: Mutex<Point>,
    started: i64,
    /// What is down now, as the real turn keeps it to let go of all at once.
    down: Mutex<Vec<(String, bool)>>,
}

impl FakeTurn {
    fn note(&self, name: &str, key: bool, up: bool) {
        let mut d = lk(&self.down);
        d.retain(|x| !(x.0 == name && x.1 == key));
        if !up {
            d.push((name.to_string(), key));
        }
    }
}

impl IMacroTurn for FakeTurn {
    fn client(&self) -> Rect {
        self.w.client
    }
    fn still_front(&self) -> bool {
        true
    }
    fn person_input(&self) -> bool {
        self.desk.person_at_now().is_some_and(|at| at > self.started && at <= self.desk.ms())
    }
    fn key(&self, key: &MacroKey, up: bool) {
        self.note(&key.name, true, up);
        self.desk.add(format!("{} key {} {} @{}", self.w.pid, key.name, if up { "up" } else { "down" }, self.desk.ms()));
    }
    fn char(&self, c: char, up: bool) {
        self.desk.add(format!("{} char {c} {}", self.w.pid, if up { "up" } else { "down" }));
    }
    fn move_to(&self, screen: Point) {
        *lk(&self.at) = screen;
        self.desk.add(format!("{} move {},{}", self.w.pid, screen.x, screen.y));
    }
    fn move_by(&self, dx: i32, dy: i32) {
        self.desk.add(format!("{} by {dx},{dy}", self.w.pid));
    }
    fn button(&self, button: &str, up: bool) {
        self.note(button, false, up);
        self.desk.add(format!("{} {button} {} @{}", self.w.pid, if up { "up" } else { "down" }, self.desk.ms()));
    }
    fn wheel(&self, notches: i32) {
        self.desk.add(format!("{} wheel {notches}", self.w.pid));
    }
    fn clear(&self, screen: Point) -> bool {
        let f = lk(&self.desk.clear_at).clone();
        f.is_none_or(|f| f(&self.desk, screen))
    }
    fn cursor_at(&self) -> Point {
        *lk(&self.at)
    }
    /// Everything let go and the PC given back at once, with no wait (Bloxkeep closing).
    fn abort(&self) {
        let down: Vec<(String, bool)> = std::mem::take(&mut *lk(&self.down));
        for (name, key) in down {
            let ms = self.desk.ms();
            self.desk.add(if key { format!("{} key {name} up @{ms}", self.w.pid) } else { format!("{} {name} up @{ms}", self.w.pid) });
        }
        self.desk.add(format!("abort {}", self.w.pid));
    }
    fn dispose(&self) {
        if self.desk.throw_on_end.load(Ordering::SeqCst) {
            return; // PORT: the C#'s DisposeAsync threw here; the trait can't fail, so the end goes unlogged.
        }
        self.desk.add(format!("end {}", self.w.pid));
    }
}

pub(crate) fn setup(accounts: usize) -> (Arc<MacroPlayer>, Arc<FakeDesk>, Vec<Guid>) {
    let desk = FakeDesk::new();
    let ids: Vec<Guid> = (0..accounts).map(|_| new_guid()).collect();
    for (i, id) in ids.iter().enumerate() {
        lk(&desk.windows).insert(
            *id,
            MacroWindow { account_id: *id, pid: i as i32 + 1, handle: 0, client: Rect::new(100, 100, 800, 600), fps: 60, in_game: true, elevated: false },
        );
    }
    let player = MacroPlayer::new(desk.clone() as Arc<dyn IMacroDesk>);
    *lk(&player.random) = Rng::seeded(1);
    (player, desk, ids)
}

/// Until the queue is played through (10 s at most, then everything is stopped).
fn done(p: &Arc<MacroPlayer>) {
    let begun = Instant::now();
    while p.active() && begun.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(2));
    }
    if p.active() {
        println!("NOTE  a queue didn't finish in 10 s; stopped");
        p.stop(None, "guard", None, false);
    }
    p.idle();
}

pub(crate) fn queue(p: &Arc<MacroPlayer>, m: &Macro, accounts: &[Guid]) -> i32 {
    p.queue(m.clone(), accounts, false).unwrap_or(-1)
}

fn queue_trial(p: &Arc<MacroPlayer>, m: &Macro, accounts: &[Guid]) -> i32 {
    p.queue(m.clone(), accounts, true).unwrap_or(-1)
}

pub(crate) fn run_at(p: &MacroPlayer, i: usize) -> RunRef {
    p.runs()[i].clone()
}

pub(crate) fn state(run: &RunRef) -> String {
    let r = lk(run);
    format!("{}/{}", r.status, r.reason.clone().unwrap_or_default())
}

pub(crate) fn is(run: &RunRef, status: &str, reason: &str) -> bool {
    let r = lk(run);
    r.status == status && r.reason.as_deref() == Some(reason)
}

fn status(run: &RunRef) -> String {
    lk(run).status.clone()
}

fn changed(p: &Arc<MacroPlayer>, f: impl Fn() + Send + Sync + 'static) {
    *p.changed.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(f));
}

/// The time on the desk's clock a log line has ("… @1234").
fn at(line: &str) -> i64 {
    line.split('@').nth(1).and_then(|t| t.trim().parse().ok()).unwrap_or(-1)
}

fn var(p: &MacroPlayer, account: Guid, name: &str) -> Option<f64> {
    p.state().vars.get(&account).and_then(|v| v.get(name)).copied()
}

/// A turn asked for on a thread of its own (InputTurn.EnterAsync), once it has its place in the queue.
struct Asked {
    who: String,
    rx: Receiver<Lease>,
    got: Option<Lease>,
}

impl Asked {
    fn new(who: &str) -> Asked {
        let (tx, rx) = channel();
        let name = who.to_string();
        std::thread::spawn(move || {
            if let Ok(lease) = input_turn::enter(&name, None) {
                let _ = tx.send(lease);
            }
        });
        let mut asked = Asked { who: who.to_string(), rx, got: None };
        // In the queue (or holding the turn) before this returns, as EnterAsync's place is taken at once.
        let begun = Instant::now();
        while !input_turn::waits(who) && !asked.done() && begun.elapsed() < Duration::from_secs(2) {
            std::thread::sleep(Duration::from_millis(1));
        }
        asked
    }
    fn done(&mut self) -> bool {
        if self.got.is_none() {
            self.got = self.rx.try_recv().ok();
        }
        // Granted already (the turn is ours by name), the thread not yet back with it: as EnterAsync's task completes
        // the moment the turn is handed on, it is taken now.
        if self.got.is_none() && input_turn::holder().as_deref() == Some(self.who.as_str()) {
            self.got = self.rx.recv_timeout(Duration::from_secs(1)).ok();
        }
        self.got.is_some()
    }
    fn wait(&mut self, limit: Duration) -> bool {
        let begun = Instant::now();
        while !self.done() && begun.elapsed() < limit {
            std::thread::sleep(Duration::from_millis(2));
        }
        self.done()
    }
}

// ───────────────────────── checks on a macro ─────────────────────────

fn validation(c: &mut Checks) {
    let mut nameless = Macro { steps: vec![MacroStep { kind: "wait".into(), ms: Some(1), ..Default::default() }], ..Default::default() };
    c.check("a macro needs a name", check_macro(&mut nameless).is_some(), "");
    c.check("a macro needs a step", check_of("[]", "").is_some(), "");
    c.check("an unknown step is refused", check_of(r#"[{"kind":"jump"}]"#, "").is_some(), "");
    let loop_end = check_of(r#"[{"kind":"loop","times":2},{"kind":"key","key":"A"}]"#, "");
    c.check("a loop without its end is refused", loop_end.as_deref().is_some_and(|e| e.contains("End repeat")), &loop_end);
    c.check("a key held and never released is refused", check_of(r#"[{"kind":"down","key":"W"}]"#, "").is_some(), "");
    let mut key = read(r#"[{"kind":"key","key":"E"}]"#, "");
    check_macro(&mut key);
    c.check("a key press is held 60 ms unless told", key.steps[0].ms == Some(60), key.steps[0].ms);
    c.check("a macro starting by itself on 'selected' is refused", check_of(r#"[{"kind":"wait","ms":10}]"#, r#","trigger":{"kind":"join"}"#).is_some(), "");
    c.check("a mouse hold of 0..10000 ms is taken", check_of(r#"[{"kind":"click","x":0.5,"y":0.5,"holdMs":800}]"#, "").is_none(), "");
    c.check("a mouse hold over 10 s is refused", check_of(r#"[{"kind":"click","x":0.5,"y":0.5,"holdMs":20000}]"#, "").is_some(), "");
    c.check(
        "Hold and Release take the mouse buttons",
        check_of(r#"[{"kind":"down","key":"MouseLeft"},{"kind":"up","key":"MouseLeft"}]"#, "").is_none(),
        check_of(r#"[{"kind":"down","key":"MouseLeft"},{"kind":"up","key":"MouseLeft"}]"#, ""),
    );
    c.check("follow needs its area", check_of(r##"[{"kind":"follow","target":"#00ff00","marker":"#ffffff"}]"##, "").is_some(), "");
    let mut follow = read(r##"[{"kind":"follow","region":{"x":0,"y":0.5,"w":1,"h":0.1},"target":"#00ff00","marker":{"left":"#ff0000","right":"$edge"}}]"##, "");
    let fe = check_macro(&mut follow);
    let f = &follow.steps[0];
    c.check(
        "follow takes box edges and fills its defaults (onOff, 1.0/0.4, 60 fps)",
        fe.is_none() && f.mode.as_deref() == Some("onOff") && f.kp == Some(1.0) && f.kd == Some(0.4) && f.fps == Some(60) && f.button.as_deref() == Some("left"),
        (fe, page_step(f)),
    );
    c.check(
        "follow at 300 fps is refused",
        check_of(r##"[{"kind":"follow","region":{"x":0,"y":0,"w":1,"h":1},"target":"#00ff00","marker":"#ffffff","fps":300}]"##, "").is_some(),
        "",
    );
    c.check(
        "follow's marker must be a colour",
        check_of(r##"[{"kind":"follow","region":{"x":0,"y":0,"w":1,"h":1},"target":"#00ff00","marker":{"left":"red"}}]"##, "").is_some(),
        "",
    );
    let mut cc = read(r##"[{"kind":"clickColour","colour":"#00ff00","region":{"x":0,"y":0,"w":1,"h":1}}]"##, "");
    check_macro(&mut cc);
    c.check("clickColour ignores specks under 4 px unless told", cc.steps[0].min_size == Some(4) && cc.steps[0].button.as_deref() == Some("left"), page_step(&cc.steps[0]));
    c.check("waitColour searches an area too", check_of(r##"[{"kind":"waitColour","colour":"#00ff00","region":{"x":0,"y":0,"w":0.5,"h":0.5}}]"##, "").is_none(), "");
    let stuck = check_of(r#"[{"kind":"key","key":"E"},{"kind":"recover"},{"kind":"key","key":"Escape"}]"#, r#","onFail":"restart","stuckMinutes":5"#);
    c.check("an If stuck part is taken at the top level", stuck.is_none(), &stuck);
    c.check("an If stuck part inside a loop is refused", check_of(r#"[{"kind":"loop","times":0},{"kind":"recover"},{"kind":"endLoop"}]"#, "").is_some(), "");
    c.check("a second If stuck part is refused", check_of(r#"[{"kind":"recover"},{"kind":"key","key":"E"},{"kind":"recover"}]"#, "").is_some(), "");
    c.check(
        "a key still held where the If stuck part begins is refused",
        check_of(r#"[{"kind":"down","key":"W"},{"kind":"recover"},{"kind":"up","key":"W"}]"#, "").is_some(),
        "",
    );
    c.check("stuck after over an hour is refused", check_of(r#"[{"kind":"key","key":"E"}]"#, r#","stuckMinutes":90"#).is_some(), "");
    let jumps = macro_rules::jumps(
        &read(
            r#"[{"kind":"loop","times":2},{"kind":"if","test":{"kind":"var","name":"n","op":"=","value":1}},{"kind":"break"},{"kind":"else"},{"kind":"key","key":"A"},{"kind":"endIf"},{"kind":"endLoop"}]"#,
            "",
        )
        .steps,
    )
    .map(|j| j.iter().map(i32::to_string).collect::<Vec<_>>().join(","))
    .unwrap_or_else(|e| msg(&e));
    c.check("jumps: loop↔endLoop, if→else→endIf, break→endLoop", jumps == "6,3,6,5,-1,-1,0", &jumps);
    c.check(
        "scan codes: letters by place, arrows extended, the pad apart",
        macro_keys::get("A").is_some_and(|k| k.scan == 0x1E)
            && macro_keys::get("Up").is_some_and(|k| k.scan == 0x48 && k.extended)
            && macro_keys::from_scan(0x48, true, 0).is_some_and(|k| k.name == "Up")
            && macro_keys::from_scan(0x48, false, 0).is_some_and(|k| k.name == "Numpad8"),
        "",
    );
    c.check(
        "a press lasts two frames at least, never under 60 ms",
        macro_player::hold_for(10, 30) == 67 && macro_player::hold_for(10, 240) == 60 && macro_player::hold_for(500, 60) == 500,
        (macro_player::hold_for(10, 30), macro_player::hold_for(10, 240), macro_player::hold_for(500, 60)),
    );
}

fn geometry(c: &mut Checks) {
    let left = Rect::new(-1920, 0, 1920, 1080);
    c.check(
        "a place in a window on a screen left of the main one",
        macro_geometry::client_point(left, 0.5, 0.5) == Point::new(-960, 540) && macro_geometry::client_point(left, 1.0, 1.0) == Point::new(-1, 1079),
        (macro_geometry::client_point(left, 0.5, 0.5), macro_geometry::client_point(left, 1.0, 1.0)),
    );
    let desktop = Rect::new(-1920, 0, 3840, 1080);
    c.check(
        "absolute input covers the whole desktop from its left edge",
        macro_geometry::absolute(Point::new(-1920, 0), desktop) == (0, 0) && macro_geometry::absolute(Point::new(1919, 1079), desktop) == (65535, 65535),
        macro_geometry::absolute(Point::new(1919, 1079), desktop),
    );
    let pieces = macro_geometry::pieces(100, -7, 6);
    c.check(
        "a camera move's pieces add up to it",
        pieces.iter().map(|p| p.0).sum::<i32>() == 100 && pieces.iter().map(|p| p.1).sum::<i32>() == -7,
        &pieces,
    );
}

// ───────────────────────── pictures ─────────────────────────

fn vision(c: &mut Checks) {
    // Blocks of grey, so a picture scaled up still looks alike.
    let mut seed: u64 = 7;
    let mut next = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((seed >> 33) % 256) as u32
    };
    let mut picture = Picture::new(240, 160);
    for y in (0..160).step_by(8) {
        for x in (0..240).step_by(8) {
            let v = next();
            fill(&mut picture, Rect::new(x, y, 8, 8), 0xFF00_0000 | (v << 16) | (v << 8) | v);
        }
    }
    let mut cut = Picture::new(32, 32);
    for y in 0..32 {
        for x in 0..32 {
            cut.set(x, y, picture.get(120 + x, 40 + y));
        }
    }
    let png = macro_player::png_of(&cut).unwrap_or_default();
    let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
    let gray = macro_vision::to_gray(&picture, None);
    let found = macro_vision::template(&b64, 1.0).and_then(|t| macro_vision::find(&gray, &t, Rect::new(0, 0, gray.w, gray.h)));
    c.check(
        "a picture is found where it was cut",
        found.is_some_and(|(score, f)| score > 0.95 && (f.x - 120).abs() <= 1 && (f.y - 40).abs() <= 1),
        found,
    );
    // Nearest neighbour, 1.5×.
    let mut big = Picture::new(360, 240);
    for y in 0..240 {
        for x in 0..360 {
            let sx = (((x as f64 + 0.5) / 1.5) as i32).min(239);
            let sy = (((y as f64 + 0.5) / 1.5) as i32).min(159);
            big.set(x, y, picture.get(sx, sy));
        }
    }
    let big_gray = macro_vision::to_gray(&big, None);
    let scaled = macro_vision::template(&b64, 1.5).and_then(|t| macro_vision::find(&big_gray, &t, Rect::new(0, 0, big_gray.w, big_gray.h)));
    c.check(
        "in a window 1.5× as big, the cut is scaled and still found",
        scaled.is_some_and(|(score, s)| score > 0.85 && (s.x - 180).abs() <= 3 && (s.y - 60).abs() <= 3),
        scaled,
    );
    let area = Rect::new(0, 0, gray.w, gray.h);
    let same = macro_vision::difference(&macro_vision::signature(&gray, area), &macro_vision::signature(&macro_vision::to_gray(&picture, None), area));
    let other = bars(240, 160, &[(0, 120, WHITE)]);
    let moved = macro_vision::difference(&macro_vision::signature(&gray, area), &macro_vision::signature(&macro_vision::to_gray(&other, None), area));
    c.check(
        "waitStill: the same picture is still, another isn't",
        same <= macro_vision::STILL_DIFFERENCE && moved > macro_vision::STILL_DIFFERENCE,
        (same, moved),
    );
    let dot = bars(10, 10, &[(5, 1, 0xFFC8_1414)]);
    c.check(
        "a colour at a place, within its tolerance",
        macro_vision::colour_at(&dot, 0.5, 0.5, "#c81414", 0) && macro_vision::colour_at(&dot, 0.5, 0.5, "#d01010", 10) && !macro_vision::colour_at(&dot, 0.1, 0.5, "#c81414", 24),
        "",
    );
}

fn tiny_task(c: &mut Checks) {
    fn rec(events: &[(u32, u32, u32, u32)]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for &(message, l, h, time) in events {
            for v in [message, l, h, time, 0u32] {
                bytes.extend_from_slice(&v.to_le_bytes());
            }
        }
        bytes
    }
    let e: u32 = 0x45 | (0x12 << 8);
    let file = rec(&[(0x100, e, 0, 1000), (0x101, e, 0, 1120), (0x201, 960, 540, 1500), (0x202, 960, 540, 1560)]);
    match macro_recording::from_tiny_task(&file, "rec") {
        Ok(m) => {
            let key_step = m.steps.iter().find(|s| s.kind == "key");
            let click = m.steps.iter().find(|s| s.kind == "click");
            c.check(
                "a TinyTask file: a key held 120 ms, then a click in the middle of a 1920×1080 screen",
                key_step.is_some_and(|k| k.key.as_deref() == Some("E") && k.ms == Some(120))
                    && click.is_some_and(|k| k.x == Some(0.5) && k.y == Some(0.5))
                    && m.recorded_size.is_some_and(|s: MacroSize| s.w == 1920 && s.h == 1080),
                m.steps.iter().map(page_step).collect::<Vec<_>>().join(","),
            );
        }
        Err(e) => c.check("a TinyTask file: a key held 120 ms, then a click in the middle of a 1920×1080 screen", false, e),
    }
    c.check(
        "a file that isn't one is refused",
        macro_recording::read_tiny_task(&[0u8; 19]).is_err() && macro_recording::read_tiny_task(&rec(&[(0x9999, 0, 0, 0)])).is_err(),
        "",
    );
    let ev = |ms: i32, kind: &str, key: &str| MacroEvent { key: Some(key.into()), ..MacroEvent::new(ms, kind) };
    let steps = macro_recording::to_steps(&[ev(0, "keyDown", "W"), ev(50, "keyDown", "Space"), ev(80, "keyUp", "Space"), ev(900, "keyUp", "W")], &|_| Some((0.5, 0.5)));
    let has = |kind: &str, key: &str| steps.iter().any(|s| s.kind == kind && s.key.as_deref() == Some(key));
    c.check(
        "a key held across another is a Hold and a Release around it",
        has("down", "W") && has("up", "W") && has("key", "Space"),
        steps.iter().map(|s| format!("{}:{}", s.kind, s.key.clone().unwrap_or_default())).collect::<Vec<_>>().join(" "),
    );
}

// ───────────────────────── settings and areas in steps ─────────────────────────

#[derive(Debug, PartialEq)]
enum Ex {
    Expr,
    Missing,
}

impl From<MacroExprError> for Ex {
    fn from(_: MacroExprError) -> Ex {
        Ex::Expr
    }
}

fn values(c: &mut Checks) {
    let mut m = read(
        r#"[{"kind":"wait","ms":"$o1"},{"kind":"waitColour","colour":"$o2","region":"@a1","tolerance":"$o3"},{"kind":"key","key":"$o4"}]"#,
        "",
    );
    c.check(
        "\"$o1\" is kept apart from the number it stands for",
        m.steps[0].ms.is_none() && macro_values::ref_of(&m.steps[0].extra, "ms").as_deref() == Some("o1"),
        "",
    );
    let mut ms_area = serde_json::Map::new();
    ms_area.insert("ms".into(), json!("@a1"));
    c.check(
        "\"@a1\" names an area, in an area's field only",
        macro_values::ref_of(&m.steps[1].extra, "region").as_deref() == Some("@a1") && m.steps[1].region.is_none() && macro_values::ref_of(&ms_area, "ms").is_none(),
        "",
    );
    let refused = check_macro(&mut m);
    c.check("a macro naming settings and areas passes its checks", refused.is_none(), &refused);
    let mut names = macro_values::names(&m);
    names.sort();
    c.check("its names are listed", names.join(",") == "@a1,o1,o2,o3,o4", names.join(","));
    let p = page(&m);
    c.check(
        "\"$o1\" and \"@a1\" go back to the page as written",
        p.contains("\"ms\":\"$o1\"") && p.contains("\"region\":\"@a1\"") && p.contains("\"key\":\"$o4\""),
        &p,
    );
    let vault: Option<Macro> = serde_json::to_string(&m).ok().and_then(|t| serde_json::from_str(&t).ok());
    c.check(
        "and survive the vault",
        vault.as_ref().is_some_and(|v| macro_values::ref_of(&v.steps[0].extra, "ms").as_deref() == Some("o1") && v.steps[0].ms.is_none()),
        "",
    );
    c.check(
        "a bad name is refused",
        check_of(r#"[{"kind":"wait","ms":"$9lives"}]"#, "").is_some()
            && check_of(r##"[{"kind":"move","x":0.5,"y":0.5},{"kind":"waitColour","colour":"#000000","region":"@-"}]"##, "").is_some(),
        "",
    );
    let mut vals: HashMap<String, Json> = HashMap::new();
    vals.insert("o1".into(), json!(250));
    vals.insert("o2".into(), json!("#00ff00"));
    vals.insert("@a1".into(), json!({ "x": 0.1, "y": 0.2, "w": 0.3, "h": 0.4 }));
    vals.insert("o3".into(), json!(500));
    vals.insert("o4".into(), json!("q"));
    let resolve = |s: &MacroStep, vals: &HashMap<String, Json>| macro_values::resolve_step(s, &mut |n: &str| vals.get(n).cloned()).map(|c| c.into_owned());
    let wait = resolve(&m.steps[0], &vals);
    let colour = resolve(&m.steps[1], &vals);
    let key = resolve(&m.steps[2], &vals);
    c.check(
        "values are put in as it plays (a tolerance kept to 255, a key spelled as the list has it)",
        wait.as_ref().is_ok_and(|w| w.ms == Some(250))
            && colour.as_ref().is_ok_and(|s| {
                s.colour.as_deref() == Some("#00ff00") && s.region.is_some_and(|r: MacroRect| r.x == 0.1 && r.w == 0.3) && s.tolerance == Some(255)
            })
            && key.as_ref().is_ok_and(|k| k.key.as_deref() == Some("Q")),
        colour.as_ref().map(page_step).unwrap_or_default(),
    );
    c.check("the step itself is left as it was", m.steps[0].ms.is_none(), "");
    vals.remove("o1");
    c.check("a missing value stops it (missingValue)", resolve(&m.steps[0], &vals).is_err(), "");
    vals.insert("o1".into(), json!("soon"));
    vals.insert("o4".into(), json!("NoSuchKey"));
    c.check("a value of the wrong kind too", resolve(&m.steps[0], &vals).is_err() && resolve(&m.steps[2], &vals).is_err(), "");
    let mut var_extra = serde_json::Map::new();
    var_extra.insert("value".into(), json!("Gold"));
    let var_test = MacroTest { kind: "var".into(), name: Some("o5".into()), op: Some("=".into()), extra: var_extra, ..Default::default() };
    c.check(
        "a var check compares a choice's word",
        macro_values::var_test(&var_test, &mut |n: &str| (n == "o5").then(|| json!("gold")), false) == Ok(true)
            && macro_values::var_test(&var_test, &mut |n: &str| (n == "o5").then(|| json!("Wood")), false) == Ok(false),
        "",
    );
    let count = MacroTest { kind: "var".into(), name: Some("n".into()), op: Some("<".into()), value: Some(3.0), ..Default::default() };
    c.check(
        "a variable the macro sets counts as 0 before it is set",
        macro_values::var_test(&count, &mut |_: &str| None, true) == Ok(true) && macro_values::var_test(&count, &mut |_: &str| None, false).is_err(),
        "",
    );
    // What a run reads: its variables first, then the macro's settings; an area by "@".
    let own = read(
        r#"[{"kind":"wait","ms":1}]"#,
        r#","options":[{"name":"o1","label":"Every","kind":"number","value":5,"default":1}],"areas":[{"name":"a1","label":"Bar","rect":{"x":0.1,"y":0.1,"w":0.2,"h":0.2} }]"#,
    );
    let nine: HashMap<String, f64> = HashMap::from([("o1".to_string(), 9.0)]);
    c.check(
        "a run reads a variable, else the macro's setting, else an area",
        macro_player::value_in(&own, None, "o1").and_then(|v| v.as_f64()) == Some(5.0)
            && macro_player::value_in(&own, Some(&nine), "o1").and_then(|v| v.as_f64()) == Some(9.0)
            && macro_player::value_in(&own, None, "@a1").and_then(|v| v.get("w").and_then(Json::as_f64)) == Some(0.2)
            && macro_player::value_in(&own, None, "@a2").is_none()
            && macro_player::value_in(&own, None, "o2").is_none(),
        "",
    );

    // Formulas (macro_expr.rs): what AutoHotkey's expressions do, in a Set variable step.
    let ev = |f: &str, v: Option<&HashMap<String, f64>>| -> Result<f64, Ex> {
        macro_expr::eval(f, &mut |n: &str| v.and_then(|v| v.get(n).copied()).ok_or(Ex::Missing), None)
    };
    let e = |f: &str| ev(f, None).unwrap_or(f64::NAN);
    let vs: HashMap<String, f64> = HashMap::from([("coins".to_string(), 250.0), ("n".to_string(), 7.0)]);
    let ev_vs = |f: &str| ev(f, Some(&vs)).unwrap_or(f64::NAN);
    c.check(
        "a formula: order, brackets, names, {name}, % never below 0",
        e("2 + 3 * 4") == 14.0
            && e("(2 + 3) * 4") == 20.0
            && ev_vs("coins / 2 + {n}") == 132.0
            && e("-1 % 5") == 4.0
            && e("2 ^ 3 ^ 2") == 512.0
            && e("-2 ^ 2") == -4.0
            && e("10 - 2 - 3") == 5.0,
        (e("-1 % 5"), e("2 ^ 3 ^ 2"), e("-2 ^ 2")),
    );
    c.check(
        "a formula's functions and comparisons",
        e("round(2.5)") == 3.0
            && e("round(3.14159, 2)") == 3.14
            && e("floor(2.9) + ceil(2.1)") == 5.0
            && ev_vs("max(1, n, 3)") == 7.0
            && e("min(4, 2)") == 2.0
            && e("abs(-3)") == 3.0
            && ev_vs("coins >= 100") == 1.0
            && ev_vs("n = 8 or n < 0") == 0.0
            && ev_vs("not (n > 1) + 2") == 2.0
            && e("sqrt(9)") == 3.0,
        (e("round(2.5)"), e("round(3.14159, 2)"), ev_vs("not (n > 1) + 2")),
    );
    let dice: Vec<f64> = (0..300).map(|_| e("random(1, 6)")).collect();
    let distinct: HashSet<i64> = dice.iter().map(|d| *d as i64).collect();
    c.check(
        "random(1, 6) is a whole number from 1 to 6, each seen",
        dice.iter().all(|d| *d == d.floor() && (1.0..=6.0).contains(d)) && distinct.len() == 6,
        &distinct,
    );
    let pr = |t: &str| macro_expr::problem(t);
    c.check(
        "a formula's mistakes show where",
        pr("2 +").is_some_and(|a| a.contains("stops"))
            && pr("2 + * 3").is_some_and(|a| a.contains("* 3"))
            && pr("foo(1)").is_some_and(|a| a.contains("foo"))
            && pr("round()").is_some()
            && pr("(1 + 2").is_some()
            && pr("coins * 2 + random(1, 3)").is_none(),
        format!("{:?} / {:?} / {:?}", pr("2 +"), pr("2 + * 3"), pr("foo(1)")),
    );
    c.check(
        "a formula that divides by 0 fails as it plays",
        ev("1 / (n - 7)", Some(&vs)) == Err(Ex::Expr) && ev("nope + 1", None) == Err(Ex::Missing),
        (ev("1 / (n - 7)", Some(&vs)), ev("nope + 1", None)),
    );
    let n = macro_expr::number_in;
    c.check(
        "numbers read off the screen, as games write them",
        n("Coins: 1,234") == Some(1234.0)
            && n("1.234.567") == Some(1234567.0)
            && n("12.5") == Some(12.5)
            && n("3.2K") == Some(3200.0)
            && n("1.5M coins") == Some(1_500_000.0)
            && n("2B") == Some(2e9)
            && n("x 12,5") == Some(12.5)
            && n("1,234.56") == Some(1234.56)
            && n("Level 7 of 10") == Some(7.0)
            && n("-15 HP") == Some(-15.0)
            && n("no number").is_none()
            && n("Lv5") == Some(5.0)
            && n("Wave 3 10 left") == Some(3.0),
        format!("{:?} {:?} {:?}", n("3.2K"), n("x 12,5"), n("Lv5")),
    );
    let mut has_extra = serde_json::Map::new();
    has_extra.insert("value".into(), json!("reached"));
    let has = MacroTest { kind: "var".into(), name: Some("said".into()), op: Some("has".into()), extra: has_extra, ..Default::default() };
    let mut lost_extra = serde_json::Map::new();
    lost_extra.insert("value".into(), json!("lost"));
    let not_has = MacroTest { kind: "var".into(), name: Some("said".into()), op: Some("!has".into()), extra: lost_extra, ..Default::default() };
    c.check(
        "a var check: has (any case) and doesn't have",
        macro_values::var_test(&has, &mut |_: &str| Some(json!("Level 12 REACHED")), false) == Ok(true)
            && macro_values::var_test(&has, &mut |_: &str| Some(json!("Level 12")), false) == Ok(false)
            && macro_values::var_test(&not_has, &mut |_: &str| Some(json!("You won")), false) == Ok(true),
        "",
    );
}

// ───────────────────────── follow's maths ─────────────────────────

fn follow_maths(c: &mut Checks) {
    let dz = macro_follow::DEAD_ZONE;
    c.check(
        "on/off: hold while behind the target, let go past it, keep as is on it",
        macro_follow::on_off(20.0, 50.0, dz) == Some(true) && macro_follow::on_off(80.0, 50.0, dz) == Some(false) && macro_follow::on_off(51.0, 50.0, dz).is_none(),
        "",
    );
    let bar = bars(100, 10, &[(49, 3, LIME), (19, 3, WHITE)]);
    let px = Pixels::of(&bar, None);
    let white = MacroMarker { colour: Some("#ffffff".into()), left: None, right: None };
    let sight = macro_follow::look(&px, "#00ff00", &white, 24);
    c.check(
        "the target line and the marker are found by colour",
        sight.as_ref().is_ok_and(|s| s.target == Some(50.0) && s.marker == Some(20.0)),
        &sight,
    );
    let bx = bars(100, 10, &[(49, 2, LIME), (10, 2, RED), (30, 2, BLUE)]);
    let edges = macro_follow::look(
        &Pixels::of(&bx, None),
        "#00ff00",
        &MacroMarker { colour: None, left: Some("#ff0000".into()), right: Some("#0000ff".into()) },
        24,
    );
    c.check(
        "a box's marker is the middle between its edges",
        edges.as_ref().is_ok_and(|e| e.marker == Some(20.5) && e.left == Some(10.5) && e.right == Some(30.5)),
        &edges,
    );
    let none = macro_follow::look(&Pixels::of(&bx, None), "#ff00ff", &white, 24);
    c.check("a colour not there is null", none.as_ref().is_ok_and(|n| n.target.is_none() && n.marker.is_none()), &none);

    let mut pd = Pd::new(1.0, 0.0, 100.0);
    let a = pd.press_ms(60.0, 50.0, 16.0);
    let b = pd.press_ms(40.0, 50.0, 16.0);
    let d = pd.press_ms(90.0, 10.0, 16.0);
    c.check("PD: the press grows with the distance, none once past, a whole frame at most", (a - 6.4).abs() < 1e-9 && b == 0.0 && d == 16.0, (a, b, d));
    let mut damped = Pd::new(1.0, 0.4, 100.0);
    damped.press_ms(80.0, 50.0, 16.0); // error 0.3
    let closing_fast = damped.press_ms(60.0, 50.0, 16.0); // error 0.1, closing by 0.2 a frame
    let mut plain = Pd::new(1.0, 0.0, 100.0);
    plain.press_ms(80.0, 50.0, 16.0);
    let undamped = plain.press_ms(60.0, 50.0, 16.0);
    c.check(
        "PD: closing in fast, damping presses less",
        closing_fast < undamped && (closing_fast - 1.28).abs() < 1e-9 && (undamped - 6.4).abs() < 1e-9,
        (closing_fast, undamped),
    );
    let mut predictor = Predictor::new();
    let mut ahead = 0.0;
    for x in [0.0, 4.0, 8.0, 12.0, 16.0, 20.0] {
        ahead = predictor.ahead(x);
    }
    c.check(
        "the predictor learns the speed and leads by two frames",
        (predictor.speed() - 3.875).abs() < 1e-9 && (ahead - (20.0 + 2.0 * 3.875)).abs() < 1e-9,
        (predictor.speed(), ahead),
    );
    let spots = patches(100, 100, &[Rect::new(10, 10, 20, 20), Rect::new(60, 60, 2, 2), Rect::new(70, 20, 10, 10)]);
    let blobs = macro_follow::blobs(&Pixels::of(&spots, None), "#00ff00", 24, 4).unwrap_or_default();
    c.check(
        "patches of a colour, largest first, specks left out",
        blobs.len() == 2 && blobs[0] == Rect::new(10, 10, 20, 20) && blobs[1] == Rect::new(70, 20, 10, 10),
        &blobs,
    );
}

// ───────────────────────── the turn ─────────────────────────

fn turns(c: &mut Checks) {
    input_turn::reset();
    let a = input_turn::try_enter("afk");
    c.check("a turn taken is busy, a second one waits", a.is_some() && input_turn::busy() && input_turn::try_enter("picture").is_none(), "");
    let mut b = Asked::new("macro");
    let mut d = Asked::new("arrange");
    c.check("those waiting wait", !b.done() && !d.done(), "");
    drop(a);
    b.wait(Duration::from_secs(2));
    c.check(
        "the first to ask goes next, alone",
        b.done() && !d.done() && input_turn::holder().as_deref() == Some("macro"),
        input_turn::holder(),
    );
    b.got.take();
    d.wait(Duration::from_secs(2));
    c.check("then the next", d.done() && input_turn::holder().as_deref() == Some("arrange"), input_turn::holder());
    d.got.take();
    c.check("then it is free", !input_turn::busy(), input_turn::holder());
    let held = input_turn::try_enter("afk");
    let ran = Arc::new(AtomicBool::new(false));
    let r = ran.clone();
    input_turn::run_when_free("hotkey", move || r.store(true, Ordering::SeqCst));
    c.check("a hotkey waits while anti-AFK has a window", !ran.load(Ordering::SeqCst), "");
    drop(held);
    // Ran on a thread of its own: the turn is handed back just after the action, not with it.
    for _ in 0..100 {
        if ran.load(Ordering::SeqCst) && !input_turn::busy() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    c.check("and runs once it is free", ran.load(Ordering::SeqCst) && !input_turn::busy(), "");

    // A macro asked while anti-AFK has a window waits ("busy"), then plays.
    let (player, desk, accounts) = setup(1);
    let afk = input_turn::try_enter("afk");
    let m = mk(r#"[{"kind":"key","key":"E"}]"#, "");
    queue(&player, &m, &accounts);
    let run = run_at(&player, 0);
    let begun = Instant::now();
    while !is(&run, "paused", "busy") && begun.elapsed() < Duration::from_secs(2) {
        std::thread::sleep(Duration::from_millis(2));
    }
    c.check("a run waits for anti-AFK's turn", is(&run, "paused", "busy") && !desk.any("key"), state(&run));
    drop(afk);
    done(&player);
    c.check("then plays", status(&run) == "done" && desk.any("key E down"), format!("{} {}", state(&run), desk.joined()));
    input_turn::reset();
}

// ───────────────────────── the player ─────────────────────────

fn player(c: &mut Checks) {
    {
        let (player, desk, accounts) = setup(2);
        let m = mk(r#"[{"kind":"loop","times":3},{"kind":"key","key":"A"},{"kind":"wait","ms":2000},{"kind":"endLoop"}]"#, "");
        queue(&player, &m, &accounts);
        done(&player);
        let turns: Vec<String> = desk.log().iter().filter(|l| l.starts_with("turn")).map(|l| l[5..].to_string()).collect();
        let turns = turns.join(",");
        c.check(
            "windows take turns round the queue",
            turns == "1,2,1,2,1,2" && player.runs().iter().all(|r| status(r) == "done"),
            &turns,
        );
        c.check("a turn gives the window back as it ends", desk.log().iter().filter(|l| l.starts_with("end")).count() == 6, desk.joined());
    }
    {
        let (player, desk, accounts) = setup(1);
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        desk.hold_now(|d| (d.ms() < 2000).then(|| "personActive".to_string()));
        {
            let (p, s) = (Arc::downgrade(&player), seen.clone());
            changed(&player, move || {
                if let Some(p) = p.upgrade()
                    && let Some(r) = p.runs().first()
                {
                    lk(&s).push(state(r));
                }
            });
        }
        queue(&player, &mk(r#"[{"kind":"key","key":"E"}]"#, ""), &accounts);
        done(&player);
        let pressed_at = desk.log().iter().find(|l| l.contains("key E down")).map_or(-1, |l| at(l));
        let seen = lk(&seen).clone();
        c.check(
            "a person at the PC pauses runs, which go on once they stop",
            seen.contains(&"paused/personActive".to_string()) && status(&run_at(&player, 0)) == "done" && pressed_at >= 2000,
            format!("{pressed_at} {}", seen.join(" ")),
        );
    }
    {
        let (player, desk, accounts) = setup(1);
        let stopped = Arc::new(AtomicBool::new(false));
        {
            let (p, s) = (player.clone(), stopped.clone());
            desk.on_delay(move |d| {
                if !s.load(Ordering::SeqCst) && d.any("key W down") && d.ms() > 300 {
                    s.store(true, Ordering::SeqCst);
                    p.stop(None, "byUser", None, false);
                }
            });
        }
        queue(&player, &mk(r#"[{"kind":"down","key":"W"},{"kind":"wait","ms":1000},{"kind":"key","key":"E"},{"kind":"up","key":"W"}]"#, ""), &accounts);
        done(&player);
        let log = desk.log();
        let down = log.iter().position(|l| l.contains("key W down"));
        let up = log.iter().position(|l| l.contains("key W up"));
        let run = run_at(&player, 0);
        c.check(
            "stopped while a key is held: it is let go, nothing after",
            is(&run, "stopped", "byUser") && down.is_some() && up > down && !desk.any("key E") && lk(&run).held.is_empty(),
            desk.joined(),
        );
    }
    {
        // "Stop trying" in the editor: only Try it's run goes, the saved macro (same id) goes on on its other account.
        let (player, desk, accounts) = setup(2);
        let m = mk(r#"[{"kind":"wait","ms":5000}]"#, "");
        let stopped = Arc::new(AtomicI32::new(-1));
        {
            let (p, s, id, a1) = (player.clone(), stopped.clone(), m.id, accounts[1]);
            desk.on_delay(move |d| {
                if s.load(Ordering::SeqCst) < 0 && p.runs().len() == 2 && d.ms() > 1000 {
                    s.store(p.stop(Some(id), "byUser", Some(a1), true), Ordering::SeqCst);
                }
            });
        }
        queue(&player, &m, &[accounts[0]]);
        queue_trial(&player, &m, &[accounts[1]]);
        done(&player);
        let runs = player.runs();
        let real = runs.iter().find(|r| !lk(r).trial).cloned();
        let tried = runs.iter().find(|r| lk(r).trial).cloned();
        let stopped = stopped.load(Ordering::SeqCst);
        c.check(
            "stopping Try it leaves the saved macro's run going",
            stopped == 1 && real.as_ref().is_some_and(|r| status(r) == "done") && tried.as_ref().is_some_and(|r| status(r) == "stopped"),
            format!("{stopped} {:?}", real.map(|r| state(&r))),
        );
    }
    {
        // Restart: a wait that runs out goes through the If stuck part, then from the top; it gives up after five.
        let (player, desk, accounts) = setup(1);
        desk.picture(|_| Some(Picture::new(10, 10)));
        queue(
            &player,
            &mk(
                r##"[{"kind":"waitColour","x":0.5,"y":0.5,"colour":"#ff0000","timeoutMs":1000},{"kind":"key","key":"E"},{"kind":"recover"},{"kind":"key","key":"Q"}]"##,
                r#","onFail":"restart" "#,
            ),
            &accounts,
        );
        done(&player);
        let q = desk.count("key Q down");
        let run = run_at(&player, 0);
        c.check(
            "a wait run out: the If stuck part, then from the top; it gives up after 5 in a row",
            is(&run, "failed", "stuck") && q == macro_player::MAX_RESTARTS as usize && !desk.any("key E"),
            format!("{} Q×{q}", state(&run)),
        );
    }
    {
        // Stuck: waits passed over ("skip") still count; time held back while the person uses the PC doesn't.
        let (player, desk, accounts) = setup(1);
        desk.picture(|_| Some(Picture::new(10, 10)));
        desk.hold_now(|d| (d.ms() >= 20_000 && d.ms() < 180_000).then(|| "personActive".to_string()));
        queue(
            &player,
            &mk(
                r##"[{"kind":"loop","times":0},{"kind":"waitColour","x":0.5,"y":0.5,"colour":"#ff0000","timeoutMs":1000},{"kind":"key","key":"E"},{"kind":"endLoop"},{"kind":"recover"},{"kind":"key","key":"Q"}]"##,
                r#","onFail":"skip","stuckMinutes":1,"maxMinutes":30 "#,
            ),
            &accounts,
        );
        done(&player);
        let first = desk.log().iter().find(|l| l.contains("key Q down")).map_or(-1, |l| at(l));
        c.check(
            "finding nothing for its stuck time runs the If stuck part, time held back left out",
            (220_000..240_000).contains(&first) && desk.any("key E"),
            first,
        );
        let run = run_at(&player, 0);
        c.check("still stuck after five restarts: it gives up", is(&run, "failed", "stuck"), state(&run));
    }
    {
        // A question answered "no" isn't stuck: "if the pop-up shows, close it" can say no for hours.
        let (player, desk, accounts) = setup(1);
        desk.picture(|_| Some(Picture::new(10, 10)));
        queue(
            &player,
            &mk(
                r##"[{"kind":"loop","times":0},{"kind":"if","test":{"kind":"colour","x":0.5,"y":0.5,"colour":"#ff0000"}},{"kind":"key","key":"Escape"},{"kind":"endIf"},{"kind":"wait","ms":30000},{"kind":"endLoop"}]"##,
                r#","onFail":"restart","stuckMinutes":1,"maxMinutes":10 "#,
            ),
            &accounts,
        );
        done(&player);
        let run = run_at(&player, 0);
        c.check("an If answered no for a long time isn't stuck", is(&run, "stopped", "maxTime"), state(&run));
    }
    {
        let (player, desk, accounts) = setup(1);
        desk.picture(|_| Some(Picture::new(10, 10)));
        queue(
            &player,
            &mk(r##"[{"kind":"waitColour","x":0.5,"y":0.5,"colour":"#ff0000","timeoutMs":1000},{"kind":"key","key":"K"}]"##, r#","onFail":"skip" "#),
            &accounts,
        );
        done(&player);
        c.check(
            "onFail skip: past a wait that timed out to the next step",
            status(&run_at(&player, 0)) == "done" && desk.any("key K down"),
            state(&run_at(&player, 0)),
        );
        let (player2, desk2, accounts2) = setup(1);
        desk2.picture(|_| Some(Picture::new(10, 10)));
        queue(&player2, &mk(r##"[{"kind":"waitColour","x":0.5,"y":0.5,"colour":"#ff0000","timeoutMs":1000},{"kind":"key","key":"K"}]"##, ""), &accounts2);
        done(&player2);
        c.check(
            "onFail stop: the run fails (timeout)",
            is(&run_at(&player2, 0), "failed", "timeout") && !desk2.any("key K"),
            state(&run_at(&player2, 0)),
        );
    }
    {
        let (player, desk, accounts) = setup(1);
        queue(
            &player,
            &mk(
                r#"[{"kind":"loop","times":3},{"kind":"addVar","name":"n","value":1},{"kind":"endLoop"},{"kind":"if","test":{"kind":"var","name":"n","op":"=","value":3}},{"kind":"key","key":"Y"},{"kind":"else"},{"kind":"key","key":"N"},{"kind":"endIf"}]"#,
                "",
            ),
            &accounts,
        );
        done(&player);
        c.check(
            "loops and variables: three passes, then the if's yes",
            var(&player, accounts[0], "n") == Some(3.0) && desk.any("key Y down") && !desk.any("key N"),
            (var(&player, accounts[0], "n"), desk.joined()),
        );
    }
    {
        let (player, desk, accounts) = setup(1);
        queue(
            &player,
            &mk(
                r#"[{"kind":"wait","ms":"$o1"},{"kind":"key","key":"$o2"},{"kind":"key","key":"E","ms":"$o9"}]"#,
                r#","options":[{"name":"o1","label":"Every","kind":"number","value":250,"default":1000},{"name":"o2","label":"Key","kind":"key","value":"Q","default":"E"}]"#,
            ),
            &accounts,
        );
        done(&player);
        let run = run_at(&player, 0);
        c.check(
            "a setting is read as it plays (a number, a key); a missing one fails the run (missingValue)",
            is(&run, "failed", "missingValue") && desk.ms() >= 250 && desk.any("key Q down") && !desk.any("key E"),
            format!("{} {}", state(&run), desk.joined()),
        );
    }
    {
        let (player, desk, accounts) = setup(1);
        desk.picture(|_| Some(patches(800, 600, &[Rect::new(500, 400, 20, 20), Rect::new(100, 100, 20, 20)])));
        queue(
            &player,
            &mk(
                r##"[{"kind":"clickColour","colour":"#00ff00","region":"@a1"}]"##,
                r#","areas":[{"name":"a1","label":"Button area","rect":{"x":0.5,"y":0.5,"w":0.5,"h":0.5} }]"#,
            ),
            &accounts,
        );
        done(&player);
        c.check(
            "a step looks in its area (\"@a1\"): the patch inside it is clicked, not the one outside",
            desk.log().contains(&"1 move 610,510".to_string()) && status(&run_at(&player, 0)) == "done",
            desk.joined(),
        );
        let (player2, _desk2, accounts2) = setup(1);
        queue(&player2, &mk(r##"[{"kind":"clickColour","colour":"#00ff00","region":"@a2"}]"##, ""), &accounts2);
        done(&player2);
        c.check("an area the macro doesn't have fails the run (missingValue)", is(&run_at(&player2, 0), "failed", "missingValue"), state(&run_at(&player2, 0)));
    }
    {
        let (player, desk, accounts) = setup(1);
        let asked = Arc::new(AtomicI32::new(0));
        {
            let a = asked.clone();
            *lk(&desk.clear_at) = Some(Arc::new(move |_, _| a.fetch_add(1, Ordering::SeqCst) + 1 > 1));
        }
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        {
            let (p, s) = (Arc::downgrade(&player), seen.clone());
            changed(&player, move || {
                if let Some(p) = p.upgrade()
                    && let Some(r) = p.runs().first()
                {
                    lk(&s).push(state(r));
                }
            });
        }
        queue(&player, &mk(r#"[{"kind":"click","x":0.5,"y":0.5,"holdMs":500}]"#, ""), &accounts);
        done(&player);
        let log = desk.log();
        let down = log.iter().find(|l| l.contains("left down"));
        let up = log.iter().find(|l| l.contains("left up"));
        let held = match (down, up) {
            (Some(d), Some(u)) => at(u) - at(d),
            _ => -1,
        };
        let seen = lk(&seen).clone();
        c.check(
            "another app's window at the click: no click, 'covered', tried again",
            seen.contains(&"paused/covered".to_string()) && asked.load(Ordering::SeqCst) >= 2 && status(&run_at(&player, 0)) == "done",
            seen.join(" "),
        );
        c.check("a click held 500 ms, in the middle of the window", held == 500 && log.contains(&"1 move 500,400".to_string()), desk.joined());
    }
    {
        let (player, desk, accounts) = setup(1);
        // A bar 800 px wide: the marker behind, then past, then the bar goes.
        let frame = Arc::new(AtomicI32::new(0));
        {
            let f = frame.clone();
            desk.area(move |_, area| {
                let n = f.fetch_add(1, Ordering::SeqCst) + 1;
                Some(match n {
                    ..=3 => bars(area.width, area.height, &[(400, 3, LIME), (100, 3, WHITE)]),
                    ..=6 => bars(area.width, area.height, &[(400, 3, LIME), (600, 3, WHITE)]),
                    _ => bars(area.width, area.height, &[]),
                })
            });
        }
        queue(
            &player,
            &mk(
                r##"[{"kind":"follow","region":{"x":0,"y":0.5,"w":1,"h":0.1},"target":"#00ff00","marker":"#ffffff","untilGoneMs":100,"timeoutMs":5000},{"kind":"key","key":"Z"}]"##,
                "",
            ),
            &accounts,
        );
        done(&player);
        let presses: Vec<String> = desk.log().iter().filter(|l| l.contains("left ")).map(|l| l.split(' ').nth(2).unwrap_or("").to_string()).collect();
        c.check(
            "follow (on/off): holds while the marker is behind, lets go past, ends when the bar goes",
            presses.join(",") == "down,up" && desk.any("key Z down") && status(&run_at(&player, 0)) == "done",
            format!("{} {}", state(&run_at(&player, 0)), desk.joined()),
        );
        c.check("follow points into the bar's area first", desk.log().contains(&"1 move 500,430".to_string()), desk.joined());
    }
    {
        let (player, desk, accounts) = setup(1);
        desk.area(|_, area| Some(bars(area.width, area.height, &[(400, 3, LIME), (100, 3, WHITE)])));
        queue(
            &player,
            &mk(
                r##"[{"kind":"follow","region":{"x":0,"y":0.5,"w":1,"h":0.1},"target":"#00ff00","marker":"#ffffff","mode":"smooth","timeoutMs":200}]"##,
                "",
            ),
            &accounts,
        );
        done(&player);
        let (downs, ups) = (desk.count("left down"), desk.count("left up"));
        c.check(
            "follow (smooth): short pulses, every one let go; the timeout is the macro's onFail",
            downs > 2 && downs == ups && is(&run_at(&player, 0), "failed", "timeout"),
            format!("{downs}/{ups} {}", state(&run_at(&player, 0))),
        );
    }
    {
        let (player, desk, accounts) = setup(1);
        desk.picture(|_| Some(patches(800, 600, &[Rect::new(400, 300, 20, 20)])));
        queue(&player, &mk(r##"[{"kind":"clickColour","colour":"#00ff00","region":{"x":0,"y":0,"w":1,"h":1}}]"##, ""), &accounts);
        done(&player);
        c.check(
            "clickColour clicks the middle of the patch",
            desk.log().contains(&"1 move 510,410".to_string()) && desk.any("left down") && status(&run_at(&player, 0)) == "done",
            desk.joined(),
        );
    }
    {
        // Each patch once, when it has shown 300 ms: one there 200 ms only isn't clicked.
        let (player, desk, accounts) = setup(1);
        desk.picture(|d| {
            Some(match d.ms() {
                ..1000 => patches(800, 600, &[Rect::new(100, 100, 20, 20), Rect::new(400, 300, 20, 20)]),
                ..1500 => Picture::new(800, 600),
                ..1700 => patches(800, 600, &[Rect::new(600, 100, 20, 20)]),
                _ => Picture::new(800, 600),
            })
        });
        let first_at = Arc::new(AtomicI64::new(-1));
        {
            let f = first_at.clone();
            desk.on_delay(move |d| {
                if f.load(Ordering::SeqCst) < 0 && d.any("left down") {
                    f.store(d.ms(), Ordering::SeqCst);
                }
            });
        }
        queue(
            &player,
            &mk(r##"[{"kind":"clickColour","colour":"#00ff00","region":{"x":0,"y":0,"w":1,"h":1},"eachAfterMs":300,"repeatUntilGoneMs":1500}]"##, ""),
            &accounts,
        );
        done(&player);
        let log = desk.log();
        let first_at = first_at.load(Ordering::SeqCst);
        c.check(
            "click each patch: every one once, after it has shown that long; one gone sooner isn't clicked",
            log.iter().filter(|l| l.ends_with("move 210,210")).count() == 1
                && log.iter().filter(|l| l.ends_with("move 510,410")).count() == 1
                && !log.iter().any(|l| l.ends_with("move 710,210"))
                && desk.count("left down") == 2
                && first_at >= 300
                && status(&run_at(&player, 0)) == "done",
            format!("{first_at} {} {}", state(&run_at(&player, 0)), desk.joined()),
        );
    }
    {
        // Text read from a list the player ticks: a letter read wrong is forgiven; a longer name isn't a shorter one.
        let (player, desk, accounts) = setup(1);
        let reads = Arc::new(AtomicI32::new(0));
        desk.picture(|_| Some(Picture::new(800, 600)));
        {
            let r = reads.clone();
            desk.lines(move |_| {
                let n = r.fetch_add(1, Ordering::SeqCst) + 1;
                Some(vec![if n <= 2 { said(&["Enchant", "Blessed Song"]) } else { said(&["Enchant", "Abysal"]) }])
            });
        }
        queue(
            &player,
            &mk(
                r#"[{"kind":"waitText","words":"$o1","region":{"x":0,"y":0,"w":1,"h":1}},{"kind":"key","key":"Y"}]"#,
                r#","options":[{"name":"o1","label":"Want","kind":"list","choices":["Abyssal","Blessed","Blessed Song"],"value":["abyssal","Blessed"]}]"#,
            ),
            &accounts,
        );
        done(&player);
        let reads = reads.load(Ordering::SeqCst);
        c.check(
            "wait for any ticked word: Abysal read counts as Abyssal, Blessed Song doesn't count as Blessed",
            reads == 3 && desk.any("key Y down") && status(&run_at(&player, 0)) == "done",
            format!("{reads} {}", state(&run_at(&player, 0))),
        );
    }
    {
        let (player, desk, accounts) = setup(1);
        desk.picture(|_| Some(Picture::new(800, 600)));
        desk.lines(|_| Some(vec![said(&["Abyssal"])]));
        queue(
            &player,
            &mk(
                r#"[{"kind":"if","test":{"kind":"text","words":"$o1"}},{"kind":"key","key":"Y"},{"kind":"else"},{"kind":"key","key":"N"},{"kind":"endIf"}]"#,
                r#","options":[{"name":"o1","label":"Want","kind":"list","choices":["Abyssal","Blessed"],"value":[]}]"#,
            ),
            &accounts,
        );
        done(&player);
        c.check("…none ticked: nothing counts", desk.any("key N down") && !desk.any("key Y"), desk.joined());
    }
    {
        let (player, desk, accounts) = setup(2);
        let shots = Arc::new(AtomicI32::new(0));
        {
            let s = shots.clone();
            desk.picture(move |_| {
                let n = s.fetch_add(1, Ordering::SeqCst) + 1;
                Some(if n <= 4 { patches(800, 600, &[Rect::new(100, 100, 20, 20)]) } else { Picture::new(800, 600) })
            });
        }
        queue(
            &player,
            &mk(r##"[{"kind":"clickColour","colour":"#00ff00","region":{"x":0,"y":0,"w":1,"h":1},"repeatUntilGoneMs":300}]"##, ""),
            &[accounts[0]],
        );
        queue(&player, &mk(r#"[{"kind":"key","key":"Q"}]"#, ""), &[accounts[1]]);
        done(&player);
        let log = desk.log();
        let clicks = log.iter().filter(|l| l.starts_with("1 left down")).count();
        let last_click = log.iter().rposition(|l| l.starts_with("1 left up"));
        let q = log.iter().position(|l| l.contains("key Q"));
        c.check(
            "a repeating clickColour clicks each time it shows, holding the turn: the other window waits",
            clicks == 4 && q.is_some() && q > last_click,
            format!("{clicks} {}", desk.joined()),
        );
    }
    {
        let (player, _desk, accounts) = setup(1);
        let said_text = Arc::new(Mutex::new(None::<String>));
        {
            let s = said_text.clone();
            *player.alert.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(move |_, _, text: &str, _| *lk(&s) = Some(text.to_string())));
        }
        queue(
            &player,
            &mk(
                r#"[{"kind":"setVar","name":"caught","value":3},{"kind":"alert","text":"Caught {caught} with {o1} {nope}"}]"#,
                r#","options":[{"name":"o1","label":"Rod","kind":"choice","choices":["Wood","Gold"],"value":"gold"}]"#,
            ),
            &accounts,
        );
        done(&player);
        let said_text = lk(&said_text).clone();
        c.check("an alert's text takes {name}: counters and settings", said_text.as_deref() == Some("Caught 3 with Gold {nope}"), &said_text);
    }
    {
        // Read a number into a variable, work it out in a formula, keep a text, type it.
        let (player, desk, accounts) = setup(1);
        let reads = Arc::new(AtomicI32::new(0));
        desk.picture(|_| Some(Picture::new(800, 600)));
        {
            let r = reads.clone();
            desk.lines(move |_| {
                let n = r.fetch_add(1, Ordering::SeqCst) + 1;
                Some(vec![if n <= 1 { said(&["Coins:"]) } else { said(&["Coins: 1,250"]) }])
            });
        }
        let said_text = Arc::new(Mutex::new(None::<String>));
        {
            let s = said_text.clone();
            *player.alert.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(move |_, _, text: &str, _| *lk(&s) = Some(text.to_string())));
        }
        queue(
            &player,
            &mk(
                r#"[{"kind":"read","name":"coins","region":{"x":0,"y":0,"w":0.5,"h":0.2}},{"kind":"setVar","name":"left","expr":"max(0, 2000 - coins)"},{"kind":"addVar","name":"left","expr":"round(coins / 100)"},{"kind":"setVar","name":"note","text":"need {left} more"},{"kind":"if","test":{"kind":"var","name":"note","op":"has","value":"763"}},{"kind":"alert","text":"{note}"},{"kind":"endIf"}]"#,
                "",
            ),
            &accounts,
        );
        done(&player);
        let said_text = lk(&said_text).clone();
        let note = player.state().texts.get(&accounts[0]).and_then(|t| t.get("note").cloned());
        let reads = reads.load(Ordering::SeqCst);
        c.check(
            "read a number (tried again until one shows), formulas, a text variable, has",
            reads == 2
                && var(&player, accounts[0], "coins") == Some(1250.0)
                && var(&player, accounts[0], "left") == Some(763.0)
                && note.as_deref() == Some("need 763 more")
                && said_text.as_deref() == Some("need 763 more")
                && status(&run_at(&player, 0)) == "done",
            format!("{reads} {said_text:?} {}", state(&run_at(&player, 0))),
        );
    }
    {
        let (player, desk, accounts) = setup(1);
        desk.picture(|_| Some(Picture::new(800, 600)));
        desk.lines(|_| Some(vec![said(&["nothing here"])]));
        queue(&player, &mk(r#"[{"kind":"read","name":"coins","region":{"x":0,"y":0,"w":0.5,"h":0.2},"timeoutMs":1000},{"kind":"key","key":"K"}]"#, ""), &accounts);
        done(&player);
        let (player2, _desk2, accounts2) = setup(1);
        queue(&player2, &mk(r#"[{"kind":"setVar","name":"n","value":0},{"kind":"setVar","name":"x","expr":"5 / n"},{"kind":"key","key":"K"}]"#, ""), &accounts2);
        done(&player2);
        c.check(
            "no number read in time fails as a wait does; a division by 0 fails the run (badFormula)",
            is(&run_at(&player, 0), "failed", "timeout") && is(&run_at(&player2, 0), "failed", "badFormula"),
            format!("{} {}", state(&run_at(&player, 0)), state(&run_at(&player2, 0))),
        );
    }
    {
        // Run a macro: the other's steps play here, its settings read as it starts; its If stuck part doesn't.
        let (player, desk, accounts) = setup(1);
        let inner = mk(
            r#"[{"kind":"key","key":"$o1"},{"kind":"addVar","name":"n","value":1},{"kind":"recover"},{"kind":"key","key":"Z"}]"#,
            r#","options":[{"name":"o1","label":"Key","kind":"key","value":"Q","default":"E"}]"#,
        );
        let outer = mk(&format!(r#"[{{"kind":"loop","times":2}},{{"kind":"call","macroId":"{}"}},{{"kind":"endLoop"}},{{"kind":"key","key":"Y"}}]"#, inner.id), "");
        {
            let inner = inner.clone();
            *player.find_macro.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(move |id| (id == inner.id).then(|| macro_rules::copy(&inner))));
        }
        queue(&player, &outer, &accounts);
        done(&player);
        c.check(
            "run a macro: its steps play in place, twice in a repeat, with its own setting; not its If stuck part",
            desk.count("key Q down") == 2
                && var(&player, accounts[0], "n") == Some(2.0)
                && desk.any("key Y down")
                && !desk.any("key Z")
                && status(&run_at(&player, 0)) == "done",
            format!("{} {}", state(&run_at(&player, 0)), desk.joined()),
        );
        let (player2, desk2, accounts2) = setup(1);
        *player2.find_macro.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(|_| None));
        queue(
            &player2,
            &mk(&format!(r#"[{{"kind":"key","key":"A"}},{{"kind":"call","macroId":"{}"}},{{"kind":"key","key":"B"}}]"#, new_guid()), ""),
            &accounts2,
        );
        done(&player2);
        let r2 = run_at(&player2, 0);
        let shown = lk(&r2).shown_pc();
        c.check(
            "a macro it runs that's gone fails the run there (missingMacro), shown at the Run a macro step",
            is(&r2, "failed", "missingMacro") && shown == 1 && desk2.any("key A down") && !desk2.any("key B"),
            format!("{} {shown}", state(&r2)),
        );
        let mut me = mk(r#"[{"kind":"key","key":"A"}]"#, "");
        let other = mk(&format!(r#"[{{"kind":"call","macroId":"{}"}}]"#, me.id), "");
        me.steps.push(MacroStep { kind: "call".into(), macro_id: Some(other.id), ..Default::default() });
        let calls = |m: &Macro, all: &[Macro]| macro_rules::check_calls(m, all).err().map(|e| msg(&e));
        let back = calls(&me, &[me.clone(), other.clone()]);
        let gone = calls(&other, &[other.clone()]);
        c.check(
            "saving: the macro to run is saved, not itself, and doesn't run this one back",
            back.as_deref().is_some_and(|b| b.contains("back"))
                && gone.as_deref().is_some_and(|g| g.contains("isn't there"))
                && calls(&mk(&format!(r#"[{{"kind":"call","macroId":"{}"}}]"#, new_guid()), ""), &[]).is_some()
                && calls(&other, &[other.clone(), mk(r#"[{"kind":"key","key":"A"}]"#, "")]).is_some()
                && check_of(r#"[{"kind":"call"}]"#, "").is_some(),
            format!("{back:?} / {gone:?}"),
        );
    }
    {
        // Read from the clipboard, a file, the web; write to them. A fake outside: nothing on this PC is touched.
        let (player, _desk, accounts) = setup(1);
        let outside = Arc::new(FakeOutside::default());
        *lk(&outside.clip) = "Gems 3.5K".into();
        lk(&outside.files).insert("coins log".into(), "12\n40".into());
        lk(&outside.web).insert("https://x.test/n".into(), "{\"count\": 7}".into());
        *player.outside.write().unwrap_or_else(|e| e.into_inner()) = outside.clone();
        queue(
            &player,
            &mk(
                r#"[{"kind":"read","from":"clipboard","name":"gems"},{"kind":"read","from":"file","fileName":"coins log","name":"coins"},{"kind":"read","from":"web","url":"https://x.test/n","name":"n"},{"kind":"write","to":"file","fileName":"out","text":"gems {gems} coins {coins} n {n}"},{"kind":"write","to":"clipboard","text":"{coins}"},{"kind":"write","to":"web","url":"https://x.test/hook","text":"done {n}"}]"#,
                "",
            ),
            &accounts,
        );
        done(&player);
        let v = player.state().vars.get(&accounts[0]).cloned().unwrap_or_default();
        let out = lk(&outside.files).get("out").cloned();
        c.check(
            "read from the clipboard, a file's last line, a web answer; write a line, the clipboard, a web post",
            v.get("gems") == Some(&3500.0)
                && v.get("coins") == Some(&40.0)
                && v.get("n") == Some(&7.0)
                && out.as_deref() == Some("gems 3500 coins 40 n 7")
                && *lk(&outside.clip) == "40"
                && *lk(&outside.posts) == vec!["https://x.test/hook done 7".to_string()]
                && status(&run_at(&player, 0)) == "done",
            format!("{} {v:?} {out:?}", state(&run_at(&player, 0))),
        );
        c.check(
            "saving: a file is a plain name, a web address https",
            check_of(r#"[{"kind":"write","to":"file","fileName":"../x","text":"a"}]"#, "").is_some()
                && check_of(r#"[{"kind":"write","to":"web","url":"http://x.test","text":"a"}]"#, "").is_some()
                && check_of(r#"[{"kind":"read","from":"file","fileName":"coins log","name":"c"}]"#, "").is_none()
                && check_of(r#"[{"kind":"read","name":"c"}]"#, "").is_some(),
            "",
        );
        let p = |ip: &str| macro_outside::public(ip.parse().expect("an address"));
        c.check(
            "web steps go only to public addresses",
            !p("127.0.0.1") && !p("192.168.1.5") && !p("10.0.0.1") && !p("::1") && !p("169.254.169.254") && p("162.159.135.232"),
            "",
        );
    }
    {
        let (player, desk, accounts) = setup(1);
        queue(
            &player,
            &mk(
                r#"[{"kind":"if","test":{"kind":"var","name":"o1","op":"=","value":1}},{"kind":"key","key":"Y"},{"kind":"else"},{"kind":"key","key":"N"},{"kind":"endIf"}]"#,
                r#","options":[{"name":"o1","label":"Auto sell","kind":"check","value":true,"default":false}]"#,
            ),
            &accounts,
        );
        done(&player);
        c.check("an if on a switch the player sets", desk.any("key Y down") && !desk.any("key N"), desk.joined());
    }
    {
        // A point the player sets on the panel: the click's x and y both name it.
        let (player, desk, accounts) = setup(1);
        let m = mk(
            r#"[{"kind":"click","x":"$o1","y":"$o1","button":"left"}]"#,
            r#","options":[{"name":"o1","label":"Water","kind":"point","value":{"x":0.25,"y":0.75}}]"#,
        );
        queue(&player, &m, &accounts);
        done(&player);
        c.check(
            "a click at a point the player set (x and y from one setting)",
            desk.log().iter().any(|l| l.ends_with("move 300,550")) && status(&run_at(&player, 0)) == "done",
            desk.joined(),
        );
        let dots = area_geometry::dots(&m);
        c.check("…and that point shows on the game as the step's dot", dots == vec![Dot { step: 0, x: 0.25, y: 0.75 }], &dots);
    }
    {
        let (player, desk, accounts) = setup(1);
        lk(&desk.windows).clear();
        queue(&player, &mk(r#"[{"kind":"key","key":"E"}]"#, ""), &accounts);
        done(&player);
        c.check("a window gone fails the run", is(&run_at(&player, 0), "failed", "windowGone"), state(&run_at(&player, 0)));
    }
    {
        let (player, desk, accounts) = setup(1);
        queue(&player, &mk(r#"[{"kind":"loop","times":0},{"kind":"key","key":"E"},{"kind":"endLoop"}]"#, r#","maxMinutes":1"#), &accounts);
        done(&player);
        c.check(
            "a run ends at its longest time (maxTime)",
            is(&run_at(&player, 0), "stopped", "maxTime") && desk.ms() >= 60_000 && desk.ms() < 70_000,
            (state(&run_at(&player, 0)), desk.ms()),
        );
    }
}

// ───────────────────────── features (a switch with its own steps) ─────────────────────────

/// A macro with two features, Sell (presses S) and Eat (presses E), both switched on unless told.
fn feats(sell: &str, eat: &str, sell_more: &str, sell_on: i32, eat_on: i32) -> String {
    format!(
        r#","options":[{{"name":"o1","label":"Sell","kind":"check","value":{sell_on}}},{{"name":"o2","label":"Eat","kind":"check","value":{eat_on}}},{{"name":"o3","label":"Key","kind":"key","value":"K"}}],"features":[{{"id":"f1","option":"o1"{sell_more},"steps":{sell}}},{{"id":"f2","option":"o2","steps":{eat}}}]"#
    )
}

const SELL: &str = r#"[{"kind":"key","key":"S"}]"#;
const EAT: &str = r#"[{"kind":"key","key":"E"}]"#;

fn feats_all() -> String {
    feats(SELL, EAT, "", 1, 1)
}

/// The keys pressed, in order (one letter each).
fn keys(desk: &FakeDesk) -> String {
    desk.log().iter().filter(|l| l.contains(" key ") && l.contains(" down")).map(|l| l.split(' ').nth(2).unwrap_or("").to_string()).collect()
}

/// A setting changed on the panel while a run plays it (as the app's Live does: the run's copy of the macro replaced).
fn set_live_option(run: &RunRef, index: usize, value: Json) {
    let mut r = lk(run);
    let mut copy = (*r.r#macro).clone();
    copy.options[index].value = Some(value);
    r.r#macro = Arc::new(copy);
}

fn features(c: &mut Checks) {
    let a = r#"[{"kind":"key","key":"A"}]"#;
    let taken = check_of(a, &feats_all());
    c.check("a feature is taken with its own steps", taken.is_none(), &taken);
    c.check("a feature whose switch is gone is refused", check_of(a, &feats_all().replace("\"option\":\"o2\"", "\"option\":\"o9\"")).is_some(), "");
    c.check("a feature on a setting that isn't a switch is refused", check_of(a, &feats_all().replace("\"option\":\"o2\"", "\"option\":\"o3\"")).is_some(), "");
    let never_ended = check_of(a, &feats(r#"[{"kind":"loop","times":2}]"#, EAT, "", 1, 1));
    c.check("a repeat never ended in a feature is refused, naming it", never_ended.as_deref().is_some_and(|e| e.starts_with("Sell:")), &never_ended);
    c.check("a key a feature holds and never lets go is refused", check_of(a, &feats(r#"[{"kind":"down","key":"W"}]"#, EAT, "", 1, 1)).is_some(), "");
    c.check("If stuck in a feature is refused", check_of(a, &feats(r#"[{"kind":"recover"}]"#, EAT, "", 1, 1)).is_some(), "");
    let many = format!("[{}]", vec![r#"{"kind":"key","key":"A"}"#; 499].join(","));
    c.check("the steps of the macro and its features count together (500)", check_of(&many, &feats_all()).is_some(), "");
    let mut ends: Vec<i32> = macro_player::safe_ends(
        &read(
            r#"[{"kind":"loop","times":0},{"kind":"if","test":{"kind":"var","name":"n","op":"=","value":1}},{"kind":"loop","times":2},{"kind":"endLoop"},{"kind":"endIf"},{"kind":"endLoop"},{"kind":"loop","times":2},{"kind":"endLoop"}]"#,
            "",
        )
        .steps,
    )
    .into_iter()
    .collect();
    ends.sort();
    let ends = ends.iter().map(i32::to_string).collect::<Vec<_>>().join(",");
    c.check("the main steps' safe points: the end of each pass of a repeat inside no other block", ends == "5,7", &ends);
    {
        // Two features due at every pass: each plays whole, in the list's order, then the repeat goes on (its passes kept).
        let (player, desk, accounts) = setup(1);
        queue(&player, &mk(r#"[{"kind":"loop","times":3},{"kind":"key","key":"A"},{"kind":"endLoop"}]"#, &feats_all()), &accounts);
        done(&player);
        c.check(
            "features play at each pass's end and the end, in order, never inside other steps",
            keys(&desk) == "ASEASEASESE" && status(&run_at(&player, 0)) == "done",
            keys(&desk),
        );
    }
    {
        let (player, desk, accounts) = setup(1);
        queue(&player, &mk(r#"[{"kind":"loop","times":2},{"kind":"key","key":"A"},{"kind":"endLoop"}]"#, &feats(SELL, EAT, "", 1, 0)), &accounts);
        done(&player);
        c.check("a feature switched off doesn't play", keys(&desk) == "ASASS", keys(&desk));
    }
    {
        // Sell every 10 s over a minute of a 1 s round: about six times, the first at the first safe point.
        let (player, desk, accounts) = setup(1);
        queue(
            &player,
            &mk(
                r#"[{"kind":"loop","times":0},{"kind":"key","key":"A"},{"kind":"wait","ms":1000},{"kind":"endLoop"}]"#,
                &format!(r#","maxMinutes":1{}"#, feats(SELL, EAT, r#","everyMs":10000"#, 1, 0)),
            ),
            &accounts,
        );
        done(&player);
        let k = keys(&desk);
        let sells = k.chars().filter(|&x| x == 'S').count();
        c.check(
            "a feature every 10 s plays every 10 s, not every round",
            (5..=7).contains(&sells) && k.starts_with("AS") && k.chars().filter(|&x| x == 'A').count() > 50,
            format!("{sells} {k}"),
        );
    }
    {
        // Switched off on the panel while it plays: it stops at its next step, the main steps go on.
        let (player, desk, accounts) = setup(1);
        let m = mk(
            r#"[{"kind":"loop","times":2},{"kind":"key","key":"A"},{"kind":"endLoop"}]"#,
            &feats(r#"[{"kind":"key","key":"S"},{"kind":"wait","ms":1000},{"kind":"key","key":"T"}]"#, EAT, "", 1, 0),
        );
        {
            let p = player.clone();
            desk.on_delay(move |d| {
                if d.any("key S down")
                    && let Some(run) = p.runs().first()
                {
                    let on = lk(run).r#macro.options[0].value.clone();
                    if on != Some(json!(0)) {
                        set_live_option(run, 0, json!(0));
                    }
                }
            });
        }
        queue(&player, &m, &accounts);
        done(&player);
        c.check(
            "a feature switched off while it plays stops there; the rest goes on",
            keys(&desk) == "ASA" && status(&run_at(&player, 0)) == "done",
            keys(&desk),
        );
    }
    {
        // W held through the repeat: no feature at its passes (they'd play with W down); at the end, once let go.
        let (player, desk, accounts) = setup(1);
        queue(
            &player,
            &mk(r#"[{"kind":"down","key":"W"},{"kind":"loop","times":2},{"kind":"key","key":"A"},{"kind":"endLoop"},{"kind":"up","key":"W"}]"#, &feats(SELL, EAT, "", 1, 0)),
            &accounts,
        );
        done(&player);
        let log = desk.log();
        let up = log.iter().position(|l| l.contains("key W up"));
        let sell = log.iter().position(|l| l.contains("key S down"));
        c.check("no feature plays while a key is held", keys(&desk) == "WAAS" && sell > up, keys(&desk));
    }
    {
        // Its picture never shows: skipped this time (said on the panel), the main steps go on, the run isn't failed.
        let (player, desk, accounts) = setup(1);
        desk.picture(|_| Some(Picture::new(10, 10)));
        queue(
            &player,
            &mk(a, &feats(r##"[{"kind":"waitColour","x":0.5,"y":0.5,"colour":"#ff0000","timeoutMs":500},{"kind":"key","key":"T"}]"##, EAT, "", 1, 1)),
            &accounts,
        );
        done(&player);
        let run = run_at(&player, 0);
        let key = (lk(&run).r#macro.id, accounts[0], "f1".to_string());
        let record = player.state().features.get(&key).cloned();
        c.check(
            "a feature that finds nothing is skipped, the next plays, the run goes on",
            keys(&desk) == "AE" && status(&run) == "done" && record.as_ref().is_some_and(|r| r.failed.as_deref() == Some("timeout") && r.failed_step == 0),
            format!("{} {} {:?}", keys(&desk), state(&run), record),
        );
    }
    {
        let (player, desk, accounts) = setup(1);
        queue(&player, &mk(a, &feats(r#"[{"kind":"key","key":"$o3"}]"#, EAT, "", 1, 0)), &accounts);
        done(&player);
        c.check("a feature's step reads the panel's settings", keys(&desk) == "AK", keys(&desk));
    }
    {
        // One macro on a window at a time.
        let (player, desk, accounts) = setup(1);
        let first = mk(r#"[{"kind":"wait","ms":3000}]"#, "");
        let two = Arc::new(AtomicI32::new(-1));
        let busy = Arc::new(Mutex::new(None::<Guid>));
        {
            let (p, t, b, acc) = (player.clone(), two.clone(), busy.clone(), accounts.clone());
            desk.on_delay(move |d| {
                // Queued while the first plays.
                if t.load(Ordering::SeqCst) < 0 && d.ms() > 1000 {
                    t.store(queue(&p, &mk(r#"[{"kind":"key","key":"A"}]"#, ""), &acc), Ordering::SeqCst);
                    *lk(&b) = p.busy_with(acc[0], new_guid()).map(|m| m.id);
                }
            });
        }
        let one = queue(&player, &first, &accounts);
        done(&player);
        let two = two.load(Ordering::SeqCst);
        let busy = *lk(&busy);
        c.check(
            "a second macro on a window running one isn't queued, and which is running there is said",
            one == 1 && two == 0 && !keys(&desk).contains('A') && busy == Some(first.id),
            format!("{one} {two}"),
        );
    }
}

// ───────────────────────── how long a turn keeps the PC ─────────────────────────

fn guard(p: &Arc<MacroPlayer>, desk: &FakeDesk, past: i64) {
    let p = p.clone();
    desk.on_delay(move |d| {
        if d.ms() > past {
            p.stop(None, "guard", None, false);
        }
    });
}

fn bounds(c: &mut Checks) {
    input_turn::reset();
    {
        // A colour that never goes (it also matches a permanent button): clicked again and again.
        let (player, desk, accounts) = setup(1);
        desk.picture(|_| Some(patches(200, 150, &[Rect::new(100, 75, 10, 10)])));
        guard(&player, &desk, 300_000);
        queue(
            &player,
            &mk(r##"[{"kind":"clickColour","colour":"#00ff00","region":{"x":0,"y":0,"w":1,"h":1},"repeatUntilGoneMs":300}]"##, r#","maxMinutes":1"#),
            &accounts,
        );
        done(&player);
        c.check(
            "a repeating clickColour whose colour never goes ends at the run's longest time, the turn given back",
            is(&run_at(&player, 0), "stopped", "maxTime") && desk.ms() < 70_000 && !input_turn::busy(),
            format!("{} {} busy={}", state(&run_at(&player, 0)), desk.ms(), input_turn::busy()),
        );
    }
    {
        // Within a long longest time: the step itself gives up after 10 minutes (its timeout if longer): the macro's onFail.
        let (player, desk, accounts) = setup(1);
        desk.picture(|_| Some(patches(40, 30, &[Rect::new(20, 15, 6, 6)])));
        guard(&player, &desk, 1_500_000);
        queue(
            &player,
            &mk(r##"[{"kind":"clickColour","colour":"#00ff00","region":{"x":0,"y":0,"w":1,"h":1},"repeatUntilGoneMs":300}]"##, r#","maxMinutes":60"#),
            &accounts,
        );
        done(&player);
        c.check(
            "…and within a longer one, the step gives up after 10 minutes (the macro's onFail)",
            is(&run_at(&player, 0), "failed", "timeout") && (600_000..620_000).contains(&desk.ms()) && !input_turn::busy(),
            format!("{} {}", state(&run_at(&player, 0)), desk.ms()),
        );
    }
    {
        // Click each patch: a patch somewhere new every look, so "nothing new for a while" never comes.
        let (player, desk, accounts) = setup(1);
        let look = Arc::new(AtomicI32::new(0));
        {
            let l = look.clone();
            desk.picture(move |_| {
                let k = l.fetch_add(1, Ordering::SeqCst) / 4;
                Some(patches(200, 150, &[Rect::new(k * 37 % 180, k * 23 % 130, 8, 8)]))
            });
        }
        guard(&player, &desk, 300_000);
        queue(
            &player,
            &mk(
                r##"[{"kind":"clickColour","colour":"#00ff00","region":{"x":0,"y":0,"w":1,"h":1},"eachAfterMs":50,"repeatUntilGoneMs":1000}]"##,
                r#","maxMinutes":1"#,
            ),
            &accounts,
        );
        done(&player);
        c.check(
            "click each patch with patches that keep coming ends at the run's longest time",
            is(&run_at(&player, 0), "stopped", "maxTime") && desk.ms() < 70_000 && !input_turn::busy(),
            format!("{} {}", state(&run_at(&player, 0)), desk.ms()),
        );
    }
    {
        // follow on a bar that never goes, with a 10-minute timeout.
        let (player, desk, accounts) = setup(1);
        desk.area(|_, area| Some(bars(area.width, area.height, &[(area.width / 2, 3, LIME), (2, 3, WHITE)])));
        guard(&player, &desk, 300_000);
        queue(
            &player,
            &mk(
                r##"[{"kind":"follow","region":{"x":0,"y":0.5,"w":0.1,"h":0.1},"target":"#00ff00","marker":"#ffffff","timeoutMs":600000}]"##,
                r#","maxMinutes":1"#,
            ),
            &accounts,
        );
        done(&player);
        c.check(
            "follow ends at the run's longest time too, everything let go",
            is(&run_at(&player, 0), "stopped", "maxTime") && desk.ms() < 70_000 && !input_turn::busy() && desk.count("left down") == desk.count("left up"),
            format!("{} {}", state(&run_at(&player, 0)), desk.ms()),
        );
    }
    {
        // Anti-AFK asks for the PC while a follow waits on its bar for minutes.
        let (player, desk, accounts) = setup(1);
        desk.area(|_, area| Some(bars(area.width, area.height, &[(area.width / 2, 3, LIME), (2, 3, WHITE)])));
        let afk: Arc<Mutex<Option<Asked>>> = Arc::new(Mutex::new(None));
        let granted_at = Arc::new(AtomicI64::new(-1));
        {
            let (p, afk, g) = (player.clone(), afk.clone(), granted_at.clone());
            desk.on_delay(move |d| {
                let mut slot = lk(&afk);
                if slot.is_none() && d.ms() >= 5000 {
                    *slot = Some(Asked::new("afk"));
                }
                if g.load(Ordering::SeqCst) < 0
                    && let Some(asked) = slot.as_mut()
                    && asked.done()
                {
                    g.store(d.ms(), Ordering::SeqCst);
                    asked.got.take(); // the nudge done
                }
                drop(slot);
                if d.ms() > 300_000 {
                    p.stop(None, "guard", None, false);
                }
            });
        }
        queue(
            &player,
            &mk(
                r##"[{"kind":"follow","region":{"x":0,"y":0.5,"w":0.1,"h":0.1},"target":"#00ff00","marker":"#ffffff","timeoutMs":120000}]"##,
                r#","maxMinutes":30"#,
            ),
            &accounts,
        );
        done(&player);
        let granted_at = granted_at.load(Ordering::SeqCst);
        c.check("anti-AFK asking during a long follow gets the PC within the turn's budget (30 s)", (5000..40_000).contains(&granted_at), granted_at);
        input_turn::reset();
    }
    {
        // A key held for a minute (walking), and the person touches the PC a second in.
        let (player, desk, accounts) = setup(1);
        desk.person_at(|d| (d.ms() >= 1000).then_some(1000));
        desk.hold_now(|d| (d.ms() >= 1000 && d.ms() < 6000).then(|| "personActive".to_string()));
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        {
            let (p, s) = (Arc::downgrade(&player), seen.clone());
            changed(&player, move || {
                if let Some(p) = p.upgrade()
                    && let Some(r) = p.runs().first()
                {
                    lk(&s).push(state(r));
                }
            });
        }
        let busy_while_paused = Arc::new(AtomicBool::new(false));
        {
            let (p, b) = (player.clone(), busy_while_paused.clone());
            desk.on_delay(move |d| {
                if d.ms() > 2500 && d.ms() < 5500 && input_turn::busy() {
                    b.store(true, Ordering::SeqCst);
                }
                if d.ms() > 4_000_000 {
                    p.stop(None, "guard", None, false);
                }
            });
        }
        queue(&player, &mk(r#"[{"kind":"down","key":"W"},{"kind":"wait","ms":60000},{"kind":"up","key":"W"}]"#, r#","maxMinutes":5"#), &accounts);
        done(&player);
        let log = desk.log();
        let downs: Vec<i64> = log.iter().filter(|l| l.contains("key W down")).map(|l| at(l)).collect();
        let ups: Vec<i64> = log.iter().filter(|l| l.contains("key W up")).map(|l| at(l)).collect();
        let mut distinct = lk(&seen).clone();
        distinct.dedup();
        let busy = busy_while_paused.load(Ordering::SeqCst);
        c.check(
            "a person touching the PC during a held wait: the key let go at once, the run paused, the PC free",
            !ups.is_empty() && ups[0] < 2000 && distinct.contains(&"paused/personActive".to_string()) && !busy,
            format!("{} busy={busy} | {}", distinct.join(" "), desk.joined()),
        );
        c.check(
            "…once they stop, the key is held again for the rest of the wait only",
            downs.len() == 2 && downs[1] >= 6000 && ups.len() == 2 && (58_000..62_000).contains(&(ups[1] - downs[1])) && status(&run_at(&player, 0)) == "done",
            format!("{} | {}", state(&run_at(&player, 0)), desk.joined()),
        );
    }
    {
        // A held wait at its longest time: an hour's wait in a 1-minute run.
        let (player, desk, accounts) = setup(1);
        guard(&player, &desk, 4_000_000);
        queue(&player, &mk(r#"[{"kind":"down","key":"W"},{"kind":"wait","ms":3600000},{"kind":"up","key":"W"}]"#, r#","maxMinutes":1"#), &accounts);
        done(&player);
        let up = desk.log().into_iter().find(|l| l.contains("key W up"));
        c.check(
            "a held wait stops at the run's longest time, the key let go then",
            is(&run_at(&player, 0), "stopped", "maxTime") && up.as_deref().is_some_and(|u| at(u) < 61_000) && !input_turn::busy(),
            format!("{} {up:?}", state(&run_at(&player, 0))),
        );
    }
    {
        // Paused between holding a key and the wait: the next turn holds it again before the wait counts.
        let (player, desk, accounts) = setup(1);
        desk.person_at(|d| (d.ms() >= 10).then_some(10));
        desk.hold_now(|d| (d.ms() >= 10 && d.ms() < 5010).then(|| "personActive".to_string()));
        guard(&player, &desk, 4_000_000);
        queue(&player, &mk(r#"[{"kind":"down","key":"W"},{"kind":"wait","ms":3000},{"kind":"up","key":"W"}]"#, r#","maxMinutes":5"#), &accounts);
        done(&player);
        let log = desk.log();
        let downs: Vec<i64> = log.iter().filter(|l| l.contains("key W down")).map(|l| at(l)).collect();
        let ups: Vec<i64> = log.iter().filter(|l| l.contains("key W up")).map(|l| at(l)).collect();
        c.check(
            "paused right after a key is held: it is held again when the run goes on, for the whole wait",
            downs.len() == 2 && downs[1] >= 5010 && ups.len() == 2 && ups[1] - downs[1] >= 3000 && status(&run_at(&player, 0)) == "done",
            desk.joined(),
        );
    }
    {
        // The key setting changed on the panel while its key is held.
        let (player, desk, accounts) = setup(1);
        let changed_once = Arc::new(AtomicBool::new(false));
        {
            let (p, ch) = (player.clone(), changed_once.clone());
            desk.on_delay(move |d| {
                if !ch.load(Ordering::SeqCst)
                    && d.any("key W down")
                    && let Some(run) = p.runs().first()
                {
                    ch.store(true, Ordering::SeqCst);
                    set_live_option(run, 0, json!("E"));
                }
            });
        }
        queue(
            &player,
            &mk(
                r#"[{"kind":"down","key":"$o1"},{"kind":"wait","ms":100},{"kind":"up","key":"$o1"},{"kind":"wait","ms":5000},{"kind":"key","key":"Q"}]"#,
                r#","options":[{"name":"o1","label":"Key","kind":"key","value":"W"}]"#,
            ),
            &accounts,
        );
        done(&player);
        let log = desk.log();
        let w_up = log.iter().position(|l| l.contains("key W up"));
        let q = log.iter().position(|l| l.contains("key Q down"));
        c.check(
            "a key setting changed while its key is held: Release lets go of the key its Hold pressed",
            w_up.is_some() && q.is_some() && w_up < q && !desk.any("key E") && status(&run_at(&player, 0)) == "done",
            desk.joined(),
        );
        c.check(
            "Hold one setting's key and Release another's is refused",
            check_of(
                r#"[{"kind":"down","key":"$o1"},{"kind":"up","key":"$o2"}]"#,
                r#","options":[{"name":"o1","label":"A","kind":"key","value":"W"},{"name":"o2","label":"B","kind":"key","value":"W"}]"#,
            )
            .is_some(),
            "",
        );
        let same = check_of(r#"[{"kind":"down","key":"$o1"},{"kind":"up","key":"$o1"}]"#, r#","options":[{"name":"o1","label":"A","kind":"key","value":"W"}]"#);
        c.check("…the same setting's key held and let go passes", same.is_none(), &same);
    }
    {
        // A Press step whose key setting holds a mouse button.
        let (player, _desk, accounts) = setup(1);
        queue(&player, &mk(r#"[{"kind":"key","key":"$o1"}]"#, r#","options":[{"name":"o1","label":"Key","kind":"key","value":"MouseLeft"}]"#), &accounts);
        done(&player);
        c.check("a Press step given a mouse button stops with a reason (missingValue)", is(&run_at(&player, 0), "failed", "missingValue"), state(&run_at(&player, 0)));
    }
    {
        // Giving the window back fails: the PC's turn is handed on all the same.
        let (player, desk, accounts) = setup(1);
        desk.throw_on_end.store(true, Ordering::SeqCst);
        queue(&player, &mk(r#"[{"kind":"key","key":"E"}]"#, ""), &accounts);
        done(&player);
        let run = run_at(&player, 0);
        let finished = lk(&run).finished();
        c.check("a turn whose end fails still hands the PC on", !input_turn::busy() && finished, format!("{} holder={:?}", state(&run), input_turn::holder()));
        input_turn::reset();
    }
    {
        // The account's window replaced (a rejoin, a crash) between two turns.
        let (player, desk, accounts) = setup(1);
        {
            let a0 = accounts[0];
            desk.on_delay(move |d| {
                if d.ms() > 1000 {
                    let mut w = lk(&d.windows);
                    if let Some(win) = w.get_mut(&a0)
                        && win.pid == 1
                    {
                        win.pid = 7;
                    }
                }
            });
        }
        queue(&player, &mk(r#"[{"kind":"key","key":"A"},{"kind":"wait","ms":3000},{"kind":"key","key":"B"}]"#, ""), &accounts);
        done(&player);
        c.check(
            "a run whose window was replaced starts over from the top in the new one",
            desk.log().iter().any(|l| l.starts_with("7 key A down")) && status(&run_at(&player, 0)) == "done",
            desk.joined(),
        );
    }
    {
        // Run (the saved macro) while the editor's Try it plays it on the same account.
        let (player, _desk, accounts) = setup(1);
        let m = mk(r#"[{"kind":"wait","ms":5000}]"#, "");
        let hold = input_turn::try_enter("afk");
        queue_trial(&player, &m, &accounts);
        let tried = run_at(&player, 0);
        let queued = queue(&player, &m, &accounts);
        drop(hold);
        done(&player);
        let runs = player.runs();
        let real = runs.iter().find(|r| !lk(r).trial).cloned();
        let count = runs.iter().filter(|r| lk(r).r#macro.id == m.id).count();
        c.check(
            "Run while Try it plays the macro there: the saved macro runs, Try it stops",
            queued == 1 && real.as_ref().is_some_and(|r| status(r) == "done") && status(&tried) == "stopped" && count == 1,
            format!("{queued} {:?} {}", real.map(|r| state(&r)), state(&tried)),
        );
        input_turn::reset();
    }
    {
        // Bloxkeep closing while a key is held and another pressed: nothing will run again on this thread.
        let (player, desk, accounts) = setup(1);
        *lk(&desk.suspend_when) = Some(Arc::new(|d: &FakeDesk| d.any("key E down")));
        queue(&player, &mk(r#"[{"kind":"down","key":"W"},{"kind":"wait","ms":500},{"kind":"key","key":"E","ms":5000},{"kind":"up","key":"W"}]"#, ""), &accounts);
        let begun = Instant::now();
        while !desk.suspended.load(Ordering::SeqCst) && begun.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(2));
        }
        player.release_all("closing");
        let run = run_at(&player, 0);
        c.check(
            "closing mid-turn: every key held or pressed is let go at once, the PC given back",
            desk.any("key W up") && desk.any("key E up") && desk.log().iter().any(|l| l.starts_with("abort")) && is(&run, "stopped", "closing"),
            format!("{} | {}", state(&run), desk.joined()),
        );
        done(&player);
        input_turn::reset();
    }
    {
        // The real turn's own list of what is down: let go, forgotten; still down, owed at the close, newest first.
        let mut held = HeldInputs::<String>::new();
        held.note("key W", false, "W up".into());
        held.note("button left", false, "left up".into());
        held.note("key E", false, "E up".into());
        held.note("key E", true, "E up".into());
        held.note("key W", false, "W up".into());
        let ups = held.ups();
        c.check(
            "closing: the real turn lets go of what is still down, once each",
            ups.join(",") == "W up,left up" && held.ups().is_empty(),
            ups.join(","),
        );
    }
    {
        // Without working raw input, a turn still notices the person (input Windows got well after our own).
        let t0 = Stamp::from_unix_ms(START_MS);
        let nobody: PersonAtFn = Arc::new(|| None);
        let raw_off: RawWorksFn = Arc::new(|| false);
        let raw_on: RawWorksFn = Arc::new(|| true);
        let later: PersonAtFn = Arc::new(move || Some(t0.plus_secs(1.0)));
        let earlier: PersonAtFn = Arc::new(move || Some(t0.plus_secs(-1.0)));
        c.check(
            "raw input not working: a person's input is seen from Windows' last input",
            MacroDesk::person_since(t0, Some(&nobody), Some(&raw_off), 10_000, 5_000) && !MacroDesk::person_since(t0, Some(&nobody), Some(&raw_off), 5_050, 5_000),
            "",
        );
        c.check(
            "raw input working: only a device's input counts",
            !MacroDesk::person_since(t0, Some(&nobody), Some(&raw_on), 10_000, 5_000)
                && MacroDesk::person_since(t0, Some(&later), Some(&raw_on), 5_000, 5_000)
                && !MacroDesk::person_since(t0, Some(&earlier), Some(&raw_on), 5_000, 5_000),
            "",
        );
    }
    input_turn::reset();
}

// ───────────────────────── the panel: settings, areas, on the game ─────────────────────────

fn num(v: &Option<Json>) -> Option<f64> {
    v.as_ref().and_then(Json::as_f64)
}

fn text(v: &Option<Json>) -> Option<&str> {
    v.as_ref().and_then(Json::as_str)
}

fn panels(c: &mut Checks) {
    const STEP: &str = r#"[{"kind":"wait","ms":"$o1"}]"#;
    let with = |options: &str, areas: &str, panel: &str| format!(r#","options":{options},"areas":{areas},"panel":{panel}"#);
    const NO_PANEL: &str = r#"{"autoShow":false,"placeId":null}"#;
    let mut good = read(
        STEP,
        &with(
            r##"[
              {"name":"o1","label":"  Hold time ","kind":"number","value":99999,"default":500,"min":0,"max":10000,"unit":"s"},
              {"name":"o2","label":"Bar colour","kind":"colour","value":"#ABCDEF","default":"nope"},
              {"name":"o3","label":"Auto sell","kind":"check","value":true},
              {"name":"o4","label":"Key","kind":"key","value":"space","default":"E"},
              {"name":"o5","label":"Rod","kind":"choice","choices":["Wood","Gold"],"value":"gold","default":"Iron"},
              {"name":"o6","label":"Strength","kind":"number","value":"2.5","min":5,"max":0,"unit":"furlongs"}
            ]"##,
            r##"[{"name":"a1","label":"Fish area","colour":"#3B82F6","rect":{"x":0.3,"y":0.7,"w":0.4,"h":0.1} }]"##,
            r#"{"autoShow":true,"placeId":-5}"#,
        ),
    );
    let error = check_macro(&mut good);
    c.check("a macro with every kind of setting passes", error.is_none(), &error);
    let o = good.options.clone();
    let opts_page = Json::Array(o.iter().map(|x| page_json::write(x, Shape::Option)).collect()).to_string();
    c.check(
        "values are kept to their kind (a number to its range, a colour in small letters, a check as 1, a key and a choice as the list spells them)",
        o.len() == 6
            && num(&o[0].value) == Some(10000.0)
            && o[0].label == "Hold time"
            && text(&o[1].value) == Some("#abcdef")
            && num(&o[2].value) == Some(1.0)
            && text(&o[3].value) == Some("Space")
            && text(&o[4].value) == Some("Gold"),
        &opts_page,
    );
    if o.len() < 6 {
        return;
    }
    c.check(
        "a default that doesn't fit becomes a plain one (white, the first choice)",
        text(&o[1].default) == Some("#ffffff") && text(&o[4].default) == Some("Wood") && num(&o[2].default) == Some(0.0),
        &opts_page,
    );
    c.check(
        "a range given backwards is turned round; an unknown unit dropped",
        o[5].min == Some(0.0) && o[5].max == Some(5.0) && o[5].unit.as_deref() == Some("") && num(&o[5].value) == Some(2.5),
        &opts_page,
    );
    c.check(
        "only a number keeps a range and unit, only a choice its choices",
        o[1].min.is_none() && o[1].unit.is_none() && o[0].choices.is_none(),
        &opts_page,
    );
    c.check(
        "an area inside the window is kept, colour in small letters; a place below 0 is no game",
        good.areas[0].colour == "#3b82f6" && good.panel.place_id.is_none() && good.panel.auto_show,
        (&good.areas[0].colour, good.panel.place_id),
    );
    let opt = |options: &str, areas: &str| check_of(STEP, &with(options, areas, NO_PANEL));
    let mut tabbed = read(
        STEP,
        &with(
            r#"[
              {"name":"o1","label":"Wait","kind":"number","value":5,"tab":"fishing"},
              {"name":"o2","label":"Water","kind":"point","value":{"x":1.5,"y":"x"}},
              {"name":"o3","label":"Key","kind":"key","value":"E","tab":"gone"}
            ]"#,
            "[]",
            r#"{"autoShow":false,"placeId":null,"tabs":[" Casting ","Fishing"]}"#,
        ),
    );
    let error = check_macro(&mut tabbed);
    c.check(
        "tabs: names trimmed, a setting's tab spelled as the panel's; an unknown one goes to the first",
        error.is_none() && tabbed.panel.tabs == ["Casting", "Fishing"] && tabbed.options[0].tab.as_deref() == Some("Fishing") && tabbed.options[2].tab.is_none(),
        (&error, &tabbed.panel.tabs),
    );
    let pt = tabbed.options[1].value.clone();
    c.check(
        "a point that isn't one is put in the middle",
        pt.as_ref().is_some_and(|p| p.get("x").and_then(Json::as_f64) == Some(0.5) && macro_options::show(&tabbed.options[1], p) == "50%, 50%"),
        &pt,
    );
    let mut one_tab = read(
        STEP,
        &with(r#"[{"name":"o1","label":"Wait","kind":"number","value":5,"tab":"Only"}]"#, "[]", r#"{"autoShow":false,"placeId":null,"tabs":["Only"]}"#),
    );
    check_macro(&mut one_tab);
    let mut headed = read(STEP, &with(r#"[{"name":"o1","label":"Bait","kind":"heading","value":5}]"#, "[]", NO_PANEL));
    check_macro(&mut headed);
    c.check("a heading has no value", headed.options[0].value.is_none() && headed.options[0].default.is_none(), &headed.options[0].value);
    let mut listed = read(STEP, &with(r#"[{"name":"o1","label":"Want","kind":"list","choices":["Abyssal","Blessed"],"value":["blessed","nope","abyssal"]}]"#, "[]", NO_PANEL));
    check_macro(&mut listed);
    let lv = listed.options[0].value.clone();
    c.check(
        "a list keeps its ticked words, spelled and ordered as the list",
        lv.as_ref().is_some_and(|v| v.to_string() == r#"["Abyssal","Blessed"]"# && macro_options::show(&listed.options[0], v) == "2 of 2"),
        &lv,
    );
    c.check("a list needs its words", opt(r#"[{"name":"o1","label":"x","kind":"list","choices":[]}]"#, "[]").is_some(), "");
    c.check(
        "click each: after 0 to 10 s",
        check_of(r##"[{"kind":"clickColour","colour":"#00ff00","region":{"x":0,"y":0,"w":1,"h":1},"eachAfterMs":20000}]"##, "").is_some(),
        "",
    );
    c.check("one tab is no tabs", one_tab.panel.tabs.is_empty() && one_tab.options[0].tab.is_none(), &one_tab.panel.tabs);
    c.check(
        "two tabs of the same name are refused",
        opt("[]", "[]").is_none() && check_of(STEP, &with("[]", "[]", r#"{"autoShow":false,"placeId":null,"tabs":["A","a"]}"#)).is_some(),
        "",
    );
    c.check(
        "a setting needs a label of 1 to 30 characters",
        opt(r#"[{"name":"o1","label":" ","kind":"number"}]"#, "[]").is_some()
            && opt(&format!(r#"[{{"name":"o1","label":"{}","kind":"number"}}]"#, "a".repeat(31)), "[]").is_some(),
        "",
    );
    c.check("an unknown kind of setting is refused", opt(r#"[{"name":"o1","label":"x","kind":"slider"}]"#, "[]").is_some(), "");
    c.check(
        "two settings of one name are refused",
        opt(r#"[{"name":"o1","label":"x","kind":"check"},{"name":"o1","label":"y","kind":"check"}]"#, "[]").is_some(),
        "",
    );
    c.check(
        "a choice needs its choices, each once",
        opt(r#"[{"name":"o1","label":"x","kind":"choice","choices":[]}]"#, "[]").is_some() && opt(r#"[{"name":"o1","label":"x","kind":"choice","choices":["a","A"]}]"#, "[]").is_some(),
        "",
    );
    let many = format!("[{}]", (1..=31).map(|i| format!(r#"{{"name":"o{i}","label":"x","kind":"check"}}"#)).collect::<Vec<_>>().join(","));
    let areas9 = format!("[{}]", (1..=9).map(|i| format!(r#"{{"name":"a{i}","label":"x","rect":{{"x":0,"y":0,"w":0.1,"h":0.1}} }}"#)).collect::<Vec<_>>().join(","));
    c.check("30 settings at most, 8 areas at most", opt(&many, "[]").is_some() && opt("[]", &areas9).is_some(), "");
    c.check(
        "an area outside the window is refused",
        opt("[]", r#"[{"name":"a1","label":"x","rect":{"x":0.9,"y":0,"w":0.5,"h":0.1} }]"#).is_some(),
        "",
    );
    c.check("a new macro comes with an empty id", read(r#"[{"kind":"wait","ms":1}]"#, r#","id":"" "#).id == Guid::nil(), "");
    let old: Option<Macro> = serde_json::from_str(r#"{"Name":"Old","Steps":[{"Kind":"wait","Ms":5}]}"#).ok();
    c.check(
        "a macro saved before settings existed has none, and a panel that doesn't show by itself",
        old.as_ref().is_some_and(|m| m.options.is_empty() && m.areas.is_empty() && !m.panel.auto_show),
        "",
    );

    // As the panel shows and takes them.
    let hold = &o[0];
    c.check(
        "seconds are shown in seconds and typed in seconds",
        macro_options::show(hold, &json!(1500)) == "1.5 s" && macro_options::typed_number(hold, "2,5 s") == Some(2500.0) && macro_options::typed_number(hold, "lots").is_none(),
        macro_options::show(hold, &json!(1500)),
    );
    let small = MacroOption { kind: "number".into(), value: Some(json!(0.3)), min: Some(0.0), max: Some(10.0), ..Default::default() };
    c.check(
        "− and + move a number by a step, kept in its range",
        macro_options::nudge(hold, 1).as_f64() == Some(10000.0) && macro_options::step(hold) == 100.0 && macro_options::nudge(&small, -1).as_f64() == Some(0.2),
        (macro_options::nudge(hold, 1), macro_options::step(hold), macro_options::nudge(&small, -1)),
    );
    let part = MacroOption { kind: "number".into(), min: Some(0.5), max: Some(1.0), unit: Some("%".into()), ..Default::default() };
    c.check(
        "a part of 1 shown as percent",
        macro_options::show(&part, &json!(0.9)) == "90%" && macro_options::typed_number(&part, "80%") == Some(0.8),
        macro_options::show(&part, &json!(0.9)),
    );
    c.check(
        "a switch shows On or Off",
        macro_options::show(&o[2], &json!(1)) == l::t("On") && macro_options::show(&o[2], &json!(0)) == l::t("Off"),
        "",
    );
    let language = prefs::language();
    prefs::set_language("vi");
    let vi = macro_options::show(hold, &json!(1500));
    let vi_ok = vi == "1,5 giây" && macro_options::typed_number(hold, "2,5 giây") == Some(2500.0) && macro_options::typed_number(hold, "0.8") == Some(800.0);
    prefs::set_language(&language);
    c.check("in Vietnamese a number has a comma and its unit in words, and is typed either way", vi_ok, &vi);
    c.check(
        "a setting changed from the panel or the page is kept to its kind",
        macro_options::normalize(&o[4], &json!("WOOD")).as_ref().and_then(Json::as_str) == Some("Wood")
            && macro_options::normalize(&o[4], &json!("Iron")).is_none()
            && macro_options::normalize(&o[2], &json!(false)).as_ref().and_then(Json::as_f64) == Some(0.0)
            && macro_options::normalize(&o[1], &json!("red")).is_none(),
        "",
    );
    let counted = read(r#"[{"kind":"setVar","name":"fish","value":0},{"kind":"addVar","name":"fish","value":1},{"kind":"addVar","name":"coins","value":5}]"#, "");
    c.check("its counters: every variable it sets or adds to, once", macro_options::counters(&counted).join(",") == "fish,coins", macro_options::counters(&counted).join(","));

    // A file carries the settings and areas.
    let file = serde_json::to_string_pretty(&page_json::write(&macro_rules::copy(&good), Shape::Macro)).unwrap_or_default();
    match macro_rules::read(&parse(&file)) {
        Ok(mut back) => {
            let ok = back.options.len() == 6
                && text(&back.options[4].value) == Some("Gold")
                && back.areas.first().is_some_and(|a| a.label == "Fish area")
                && back.panel.auto_show;
            let refused = check_macro(&mut back);
            c.check("a .bloxkeepmacro file carries its settings, areas and panel", ok && refused.is_none(), &refused);
        }
        Err(e) => c.check("a .bloxkeepmacro file carries its settings, areas and panel", false, e),
    }

    // On the game: dots and boxes.
    let client = Rect::new(100, 50, 800, 600);
    let r = MacroRect { x: 0.25, y: 0.25, w: 0.5, h: 0.5 };
    let screen = area_geometry::to_screen(&r, client);
    let back2 = area_geometry::to_fraction(screen, client);
    c.check("an area in screen pixels and back", screen == Rect::new(300, 200, 400, 300) && back2.x == 0.25 && back2.w == 0.5, (screen, back2));
    c.check(
        "a press takes a corner, the inside, or nothing",
        area_geometry::hit_test(screen, Point::new(303, 197), 8) == Grip::TopLeft
            && area_geometry::hit_test(screen, Point::new(698, 502), 8) == Grip::BottomRight
            && area_geometry::hit_test(screen, Point::new(500, 300), 8) == Grip::Move
            && area_geometry::hit_test(screen, Point::new(50, 50), 8) == Grip::None,
        "",
    );
    c.check(
        "moved, it stays inside the window",
        area_geometry::drag(screen, Grip::Move, 1000, -1000, client, 12) == Rect::new(500, 50, 400, 300),
        area_geometry::drag(screen, Grip::Move, 1000, -1000, client, 12),
    );
    c.check(
        "sized, it keeps a few pixels",
        area_geometry::drag(screen, Grip::BottomRight, -1000, -1000, client, 12) == Rect::new(300, 200, 12, 12)
            && area_geometry::drag(screen, Grip::TopLeft, -1000, 0, client, 12) == Rect::from_ltrb(100, 200, 700, 500),
        "",
    );
    let outside = area_geometry::to_fraction(Rect::new(0, 0, 10, 10), client);
    c.check("an area outside the window falls back to all of it", outside.x == 0.0 && outside.w == 1.0, outside);
    let pointed = read(
        r##"[{"kind":"click","x":0.5,"y":0.5,"holdMs":500},{"kind":"key","key":"E"},{"kind":"move","x":0.1,"y":0.9},
             {"kind":"waitColour","x":0.2,"y":0.2,"colour":"#000000"},{"kind":"waitColour","colour":"#000000","region":"@a1"},{"kind":"click","x":"$o1","y":0.5}]"##,
        "",
    );
    let dots = area_geometry::dots(&pointed);
    let shown = dots.iter().map(|d| format!("{}@{},{}", d.step + 1, d.x, d.y)).collect::<Vec<_>>().join(" ");
    c.check(
        "the dots: click, hold, move and a colour at a place, each with its step; not an area's step nor a setting's place",
        shown == "1@0.5,0.5 3@0.1,0.9 4@0.2,0.2",
        &shown,
    );
    let pt = area_geometry::to_screen_point(0.5, 0.5, client);
    c.check(
        "a dot on the screen, taken by a press near it, and back",
        pt == Point::new(500, 350)
            && area_geometry::hit_dot(&dots, Point::new(507, 356), client, 12) == 0
            && area_geometry::hit_dot(&dots, Point::new(530, 350), client, 12) == -1
            && area_geometry::to_fraction_point(Point::new(140, 590), client) == (0.05, 0.9)
            && area_geometry::to_fraction_point(Point::new(5000, -20), client) == (1.0, 0.0),
        "",
    );

    // Shown by itself while its game is played.
    let fish = Macro { id: new_guid(), name: "f".into(), panel: MacroPanel { auto_show: true, place_id: Some(123), ..Default::default() }, ..Default::default() };
    let never = Macro { id: new_guid(), name: "n".into(), panel: MacroPanel { auto_show: false, place_id: Some(123), ..Default::default() }, ..Default::default() };
    let both = [fish.clone(), never.clone()];
    let mut auto = HashSet::new();
    let mut closed = HashSet::new();
    let mut visible: HashSet<Guid> = HashSet::new();
    let mut tick = |places: &[i64], auto: &mut HashSet<Guid>, closed: &mut HashSet<Guid>, visible: &mut HashSet<Guid>| {
        let played: HashSet<i64> = places.iter().copied().collect();
        let shown_now = visible.clone();
        let (show, hide) = macro_options::auto_show(&both, &played, auto, closed, &|id| shown_now.contains(&id), None, i32::MAX);
        for id in &show {
            visible.insert(*id);
        }
        for id in &hide {
            visible.remove(id);
        }
        (show, hide)
    };
    let t1 = tick(&[123], &mut auto, &mut closed, &mut visible);
    let t2 = tick(&[123], &mut auto, &mut closed, &mut visible);
    c.check("a panel shows as its game starts, once; one without autoShow never", t1.0 == vec![fish.id] && t2.0.is_empty(), (&t1, &t2));
    let t3 = tick(&[], &mut auto, &mut closed, &mut visible);
    c.check("and hides as the game ends", t3.1 == vec![fish.id], &t3);
    tick(&[123], &mut auto, &mut closed, &mut visible);
    visible.remove(&fish.id);
    auto.remove(&fish.id);
    closed.insert(fish.id); // the person closed it
    let t4 = tick(&[123], &mut auto, &mut closed, &mut visible);
    c.check("closed by the person, it stays closed while the game goes on", t4.0.is_empty(), &t4);
    tick(&[], &mut auto, &mut closed, &mut visible);
    let t5 = tick(&[123], &mut auto, &mut closed, &mut visible);
    c.check("and shows again next time the game is played", t5.0 == vec![fish.id], &t5);

    // "Any game" (no place): only a macro with a window in the game played, and never more than three at once.
    let any_games: Vec<Macro> = (0..5)
        .map(|i| Macro { id: new_guid(), name: format!("a{i}"), panel: MacroPanel { auto_show: true, place_id: None, ..Default::default() }, ..Default::default() })
        .collect();
    let mine: HashSet<Guid> = any_games.iter().take(4).map(|m| m.id).collect();
    let in_mine = |m: &Macro| mine.contains(&m.id);
    let mut auto2 = HashSet::new();
    let none = macro_options::auto_show(&any_games, &HashSet::new(), &mut auto2, &mut HashSet::new(), &|_| false, Some(&in_mine), 3);
    let some = macro_options::auto_show(&any_games, &HashSet::from([99]), &mut auto2, &mut HashSet::new(), &|_| false, Some(&in_mine), 3);
    c.check(
        "a panel for any game shows only while a game is played in one of its accounts, three at most",
        none.0.is_empty() && some.0.len() == 3 && some.0.iter().all(|id| mine.contains(id)) && !some.0.contains(&any_games[4].id),
        &some.0,
    );
    let gone = macro_options::auto_show(&any_games, &HashSet::new(), &mut auto2, &mut HashSet::new(), &|_| true, Some(&in_mine), 3);
    c.check("and they hide as the games end", gone.1.len() == 3 && auto2.is_empty(), &gone.1);
}

// ───────────────────────── templates and "What it sees" ─────────────────────────

fn templates(c: &mut Checks) {
    let all = match macro_templates::build() {
        Ok(all) => all,
        Err(e) => {
            c.check("the templates are built", false, e);
            return;
        }
    };
    c.check(
        "five templates: keys, click, cast, rejoin, blank",
        all.iter().map(|t| t.id.as_str()).collect::<Vec<_>>().join(",") == "keys,click,cast,rejoin,blank",
        all.iter().map(|t| t.id.clone()).collect::<Vec<_>>(),
    );
    c.check("a blank template has no steps", all[4].r#macro.steps.is_empty(), "");
    for t in all.iter().take(4) {
        let mut m = macro_rules::copy(&t.r#macro);
        let error = check_macro(&mut m);
        let own: HashSet<String> = m.options.iter().map(|o| o.name.clone()).chain(m.areas.iter().map(|a| format!("@{}", a.name))).collect();
        let missing: Vec<String> = macro_values::names(&m).into_iter().filter(|n| !own.contains(n)).collect();
        c.check(
            &format!("template {}: it passes, and every setting and area it names is its own", t.id),
            error.is_none() && missing.is_empty(),
            error.unwrap_or_else(|| missing.join(",")),
        );
    }
    let tpl_page = match macro_templates::all() {
        Ok(p) => p,
        Err(e) => {
            c.check("macroTemplates: { macros: Macro & {about, icon} } and nothing else", false, e);
            return;
        }
    };
    let json = tpl_page.to_string();
    let macros = tpl_page.get("macros").and_then(Json::as_array).cloned().unwrap_or_default();
    c.check(
        "macroTemplates: { macros: Macro & {about, icon} } and nothing else",
        tpl_page.as_object().is_some_and(|o| o.keys().collect::<Vec<_>>() == ["macros"])
            && macros.len() == 5
            && macros[0].get("about").and_then(Json::as_str).is_some_and(|a| !a.is_empty())
            && macros[0].get("icon").and_then(Json::as_str) == Some("keyboard")
            && macros[0].get("id").and_then(Json::as_str) == Some(""),
        json.chars().take(400).collect::<String>(),
    );
    if macros.len() < 5 {
        return;
    }
    c.check(
        "each comes with its settings and areas",
        macros[2].get("options").and_then(Json::as_array).map(Vec::len) == Some(5)
            && macros[2].pointer("/areas/0/label").and_then(Json::as_str).is_some_and(|l| !l.is_empty())
            && macros[1].pointer("/steps/1/region").and_then(Json::as_str) == Some("@a1"),
        "",
    );
    // As the page uses one: read back from the page and saved.
    let mut flow = None;
    for t in macros.iter().take(4) {
        let refused = match macro_rules::read(t) {
            Ok(mut m) => check_macro(&mut m),
            Err(e) => Some(msg(&e)),
        };
        if flow.is_none() {
            flow = refused;
        }
    }
    c.check("a template read back from the page passes", flow.is_none(), &flow);
    let mut cast = macro_rules::read(&macros[2]).unwrap_or_default();
    check_macro(&mut cast);
    let step2 = cast.steps.get(2).map(page_step).unwrap_or_default();
    c.check("a step's to-do comes with it and stays once saved", cast.id == Guid::nil() && page(&cast).contains("\"todo\":["), &step2);
    let dot_steps: Vec<i32> = area_geometry::dots(&cast).iter().map(|d| d.step).collect();
    c.check("its point to set on the game is a dot (step 2)", dot_steps == vec![1], &dot_steps);
    c.check("a to-do names fields, not sentences", step2.contains("\"todo\":[\"region\",\"target\",\"marker\"]"), &step2);
    c.check("a template's steps carry their notes, kept once saved", cast.steps.get(1).map(page_step).unwrap_or_default().contains("\"note\":"), "");
    let mut rejoin = macro_rules::read(&macros[3]).unwrap_or_default();
    check_macro(&mut rejoin);
    c.check("the rejoin template starts by itself after a rejoin, on all accounts", rejoin.trigger.kind == "rejoin" && rejoin.targets == "all", (&rejoin.trigger.kind, &rejoin.targets));
    // A note: plain text up to 80, trimmed; anything else goes.
    let mut noted = macro_rules::read(&parse(&format!(
        r#"{{"name":"n","steps":[{{"kind":"wait","ms":10,"note":"  {} "}},{{"kind":"wait","ms":10,"note":5}},{{"kind":"wait","ms":10,"note":"  "}}]}}"#,
        "a".repeat(90)
    )))
    .unwrap_or_default();
    check_macro(&mut noted);
    let notes = Json::Array(noted.steps.iter().map(|s| page_json::write(s, Shape::Step)).collect()).to_string();
    c.check(
        "a note is cut to 80 and dropped when it isn't text",
        notes.contains(&format!("\"note\":\"{}\"", "a".repeat(80))) && notes.split("\"note\"").count() == 2,
        &notes,
    );
    // The panel's names for counters: a counter's name to its label, nothing else.
    let mut labelled = macro_rules::read(&parse(
        r#"{"name":"n","steps":[{"kind":"addVar","name":"fish","value":1}],"panel":{"autoShow":false,"placeId":null,"counterLabels":{"fish":" Fish caught ","9bad":"x","empty":"","n":3}}}"#,
    ))
    .unwrap_or_default();
    check_macro(&mut labelled);
    let panel = page_json::write(&labelled.panel, Shape::Panel).to_string();
    c.check("counter labels keep only a counter's name with its text", panel.contains("\"counterLabels\":{\"fish\":\"Fish caught\"}"), &panel);
    let games = regex::RegexBuilder::new("Fisch|Blox Fruits|Pet Simulator|Adopt Me").case_insensitive(true).build().expect("pattern");
    c.check("no game's name in them", !games.is_match(&json), "");
}

fn watch(c: &mut Checks) {
    let m = mk(
        r##"[{"kind":"follow","region":{"x":0,"y":0,"w":1,"h":1},"target":"#00ff00","marker":"$o1"},{"kind":"clickColour","colour":"#00ff00","region":{"x":0,"y":0,"w":1,"h":1}}]"##,
        "",
    );
    let picture = bars(200, 50, &[(150, 2, LIME), (50, 2, WHITE)]);
    let seen = macro_watch::look(&picture, &m, &|n: &str| (n == "o1").then(|| json!("#ffffff")));
    let shown = seen.as_ref().map(|s| serde_json::to_string(s).unwrap_or_default());
    match &seen {
        Ok(seen) => {
            let target = seen.boxes.iter().find(|b| b.kind == "target");
            let marker = seen.boxes.iter().find(|b| b.kind == "marker");
            c.check(
                "What it sees: the area, the target and the marker in their places, and how far apart",
                seen.boxes.iter().any(|b| b.kind == "area")
                    && target.is_some_and(|t| (t.x - 0.745).abs() < 0.01)
                    && marker.is_some_and(|m| (m.x - 0.245).abs() < 0.01)
                    && seen.numbers.iter().any(|n| n.value == 100.0),
                &shown,
            );
            c.check(
                "and the patches clickColour would click",
                seen.boxes.iter().any(|b| b.kind == "colour") && seen.numbers.iter().any(|n| n.label.contains("patches") && n.value == 1.0),
                &shown,
            );
        }
        Err(e) => {
            c.check("What it sees: the area, the target and the marker in their places, and how far apart", false, e);
            c.check("and the patches clickColour would click", false, e);
        }
    }
    let missing = macro_watch::look(&picture, &m, &|_: &str| None);
    c.check("a step whose setting is missing is left out", missing.as_ref().is_ok_and(|s| !s.boxes.iter().any(|b| b.kind == "marker")), missing.err());
    let small = macro_watch::scaled(&Picture::new(1280, 720), 640);
    c.check("its picture is made small for the page", small.width == 640 && small.height == 360, (small.width, small.height));
}
