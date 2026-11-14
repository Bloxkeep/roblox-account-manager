// MacroPlayer.cs: playing macros into Roblox windows.
use std::collections::{HashMap, HashSet, VecDeque};
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, RwLock};
use std::thread::JoinHandle;
use std::time::Duration;

use regex::Regex;

use crate::core::base::{Cancel, Fail, Guid, Json, Res, Stamp};
use crate::core::macro_expr::{self, MacroExprError};
use crate::core::macro_follow::{self, Pixels};
use crate::core::macro_model::{Macro, MacroFeature, MacroKey, MacroRect, MacroStep, MacroTest, macro_keys, macro_rules};
use crate::core::macro_ocr;
use crate::core::macro_outside::{IMacroOutside, MacroOutside};
use crate::core::macro_panel::macro_options;
use crate::core::macro_values::{self, MacroValueMissing};
use crate::core::macro_vision::{self, Gray};
use crate::core::win_kit::{Picture, Point, Rect};
use crate::core::{input_turn, l, log, program};

/// A Roblox window a macro plays into, as the desktop has it now.
/// `client`: its client area on the screen, in pixels. `fps`: the frame rate limit Bloxkeep gave it; 0: not known.
/// `in_game`: its log says it is in a game (or there is no log to say otherwise). `elevated`: it runs as administrator,
/// which Windows won't let Bloxkeep's input reach.
#[derive(Clone, Debug, PartialEq)]
pub struct MacroWindow {
    pub account_id: Guid,
    pub pid: i32,
    pub handle: isize,
    pub client: Rect,
    pub fps: i32,
    pub in_game: bool,
    pub elevated: bool,
}

/// The desktop as macros see it: the real one (MacroDesk), or a check's stand-in with a clock of its own, so no check
/// ever sends input or brings a window forward.
pub trait IMacroDesk: Send + Sync {
    fn now(&self) -> Stamp;
    /// Task.Delay: Err when `ct` is called off meanwhile.
    fn delay(&self, ms: i32, ct: &Cancel) -> Res<()>;
    /// Why nothing may be played anywhere now (the PC locked, a person at it, a full-screen app in front); None: free.
    fn hold(&self) -> Option<String>;
    /// The account's window now; None when it has none.
    fn window(&self, account_id: Guid) -> Option<MacroWindow>;
    /// The window brought forward for input (shown unseen if minimised or hidden), or None when Windows refused.
    fn begin_turn(&self, window: &MacroWindow, ct: &Cancel) -> Res<Option<Arc<dyn IMacroTurn>>>;
    /// The window's client area as it is now, at full size; None when it can't be pictured.
    fn picture(&self, window: &MacroWindow, turn: Option<&dyn IMacroTurn>) -> Option<Picture>;
    /// A Vietnamese input method (UniKey, EVKey) is running: it may rewrite some keys typed.
    fn ime_on(&self) -> bool;
    /// Words read in an area of a picture, per language; None when this PC can't read text; Err when the reading
    /// failed (the C#'s exception out of ReadAsync).
    fn read(&self, picture: &Picture, area: Rect) -> Res<Option<Vec<Vec<macro_ocr::Line>>>>;
    /// Only an area of the client (pixels of the client), quickly, during a turn: what real-time steps look at many
    /// times a second. None when it can't be pictured.
    fn area(&self, window: &MacroWindow, turn: &dyn IMacroTurn, area: Rect) -> Option<Picture>;
}

/// A window in front with input going to it. `dispose` gives the PC back as it was.
/// (Called from the player's thread and, for `abort`, from the one closing Bloxkeep: implementations lock inside.)
pub trait IMacroTurn: Send + Sync {
    /// The window's client area on the screen now (it may have moved).
    fn client(&self) -> Rect;
    /// It is still the window in front.
    fn still_front(&self) -> bool;
    /// A person's input came since the turn began (ours never counts).
    fn person_input(&self) -> bool;
    fn key(&self, key: &MacroKey, up: bool);
    /// PORT: C# sent one UTF-16 unit; a character past U+FFFF comes here whole. Called once (`up` false): the
    /// implementation sends each half's down and up together, so a pair never interleaves.
    fn char(&self, c: char, up: bool);
    fn move_to(&self, screen: Point);
    fn move_by(&self, dx: i32, dy: i32);
    fn button(&self, button: &str, up: bool);
    fn wheel(&self, notches: i32);
    /// Before a press at a place on the screen: Bloxkeep's own windows there (a macro's panel, the mini window) let clicks
    /// through until the turn ends. False when another app's window is there: the press would go to it.
    fn clear(&self, screen: Point) -> bool;
    /// Where the pointer is now, on the screen.
    fn cursor_at(&self) -> Point;
    /// Bloxkeep closing: every key and button this turn pressed and still holds let go, the pointer and the windows put
    /// back, now, with no wait (nothing after the closing would run). Nothing is sent after it.
    fn abort(&self);
    /// DisposeAsync: the PC given back as it was.
    fn dispose(&self);
}

/// Why a step stopped short (C#'s exceptions): called off, a value missing (MacroValueMissing), a formula or outside
/// name refused (MacroExprError), another app's window over the place (MacroCovered), or anything else.
#[derive(Debug)]
enum Halt {
    Cancelled,
    Missing,
    Expr,
    Covered,
    Fail(Fail),
}

impl From<Fail> for Halt {
    fn from(f: Fail) -> Halt {
        if is_cancelled(&f) {
            Halt::Cancelled
        } else if is_value_missing(&f) {
            Halt::Missing
        } else {
            Halt::Fail(f)
        }
    }
}

impl From<MacroExprError> for Halt {
    fn from(_: MacroExprError) -> Halt {
        Halt::Expr
    }
}

impl From<MacroValueMissing> for Halt {
    fn from(_: MacroValueMissing) -> Halt {
        Halt::Missing
    }
}

fn is_cancelled(f: &Fail) -> bool {
    matches!(f, Fail::Other(m) if m == "cancelled")
}

/// MacroValueMissing as a Fail (macro_values writes it "No value {name}.").
pub fn value_missing(name: &str) -> Fail {
    MacroValueMissing::new(name).into()
}

/// The Fail is a MacroValueMissing.
pub fn is_value_missing(f: &Fail) -> bool {
    matches!(f, Fail::Other(m) if m.starts_with("No value ") && m.ends_with('.'))
}

/// An outside error (MacroOutside): called off and refused names pass up; the rest is noted.
fn outside_halt(f: Fail) -> Result<Fail, Halt> {
    if is_cancelled(&f) {
        Err(Halt::Cancelled)
    } else if matches!(f, Fail::User(_)) {
        Err(Halt::Expr)
    } else {
        Ok(f)
    }
}

/// C#'s ex.GetType().Name for a note. PORT: a Fail carries no type; its kind is named.
fn type_name(f: &Fail) -> &'static str {
    match f {
        Fail::User(_) => "UserError",
        Fail::Roblox { .. } => "RobloxApiException",
        Fail::Other(_) => "Exception",
    }
}

/// `x!.Value` on a missing value.
fn nre() -> Halt {
    Halt::Fail(Fail::other("Nullable object must have a value."))
}

fn need<T: Clone>(v: &Option<T>) -> Result<T, Halt> {
    v.clone().ok_or_else(nre)
}

/// MacroKeys.All[name]: an Err for a key that isn't there.
fn key_of(name: Option<&str>) -> Result<&'static MacroKey, Halt> {
    name.and_then(macro_keys::get)
        .ok_or_else(|| Halt::Fail(Fail::other(format!("The given key '{}' was not present in the dictionary.", name.unwrap_or("")))))
}

/// A held key or mouse button pressed or let go.
fn press_held(input: &dyn IMacroTurn, held: &str, up: bool) {
    match macro_keys::get(held) {
        Some(key) => input.key(key, up),
        None => input.button(macro_rules::mouse_key(held).unwrap_or(held), up),
    }
}

/// Math.Round(v, digits): to even on a tie, as .NET.
pub fn round_to(v: f64, digits: i32) -> f64 {
    let scale = 10f64.powi(digits);
    let scaled = v * scale;
    // Too big to scale (a huge number, many digits): no digits past its last anyway, so it stays as it is.
    if !scaled.is_finite() || scale == 0.0 {
        return v;
    }
    scaled.round_ties_even() / scale
}

/// A picture as PNG bytes (Bitmap.Save(…, ImageFormat.Png)).
pub fn png_of(p: &Picture) -> Res<Vec<u8>> {
    let img = image::RgbaImage::from_raw(p.width.max(0) as u32, p.height.max(0) as u32, p.to_rgba()).ok_or_else(|| Fail::other("The picture's size doesn't fit its pixels."))?;
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).map_err(Fail::other)?;
    Ok(out)
}

fn after_ms(t: Stamp, ms: f64) -> Stamp {
    t.plus_secs(ms / 1000.0)
}

fn ms_since(later: Stamp, earlier: Stamp) -> f64 {
    later.secs_since(earlier) * 1000.0
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// DateTime.MinValue.
fn min_stamp() -> Stamp {
    Stamp::from_unix_secs(-62_135_596_800)
}

fn lk<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// C#'s Random for jitter and offsets: a small splitmix64 (the checks set their own seed).
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new() -> Rng {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
        Rng(nanos ^ 0x9E37_79B9_7F4A_7C15)
    }
    pub fn seeded(seed: u64) -> Rng {
        Rng(seed)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Random.Next(min, max): min ≤ n < max (min when they're equal).
    pub fn next(&mut self, min: i32, max: i32) -> i32 {
        if max <= min {
            return min;
        }
        let span = (max as i64 - min as i64) as u64;
        (min as i64 + (self.next_u64() % span) as i64) as i32
    }
}

impl Default for Rng {
    fn default() -> Rng {
        Rng::new()
    }
}

/// One macro on one account's window, from queued to finished.
pub struct MacroRun {
    pub r#macro: Arc<Macro>,
    pub account_id: Guid,
    pub jumps: Vec<i32>,
    /// A draft run once from the editor ("Try it"): not the saved macro's.
    pub trial: bool,
    /// "waiting", "running", "paused", "done", "failed" or "stopped".
    pub status: String,
    pub reason: Option<String>,
    /// The step to run next (0-based).
    pub pc: i32,
    /// Passes through loops so far.
    pub loops: i32,
    pub started_at: Stamp,
    pub ends_at: Option<Stamp>,
    /// Not before this (a long wait, a picture to look at again).
    pub resume_at: Stamp,
    /// The wait being waited for times out at this.
    pub deadline: Option<Stamp>,
    /// waitStill: the last small picture, and since when it hasn't changed.
    pub last_frame: Option<Gray>,
    pub still_since: Option<Stamp>,
    /// Each loop's passes (by its first step).
    pub passes: HashMap<i32, i32>,
    /// The loop at this step was reached from its end, not from above.
    pub again: i32,
    /// Keys and buttons this run holds down (pressed again when its window's next turn begins), in the order pressed.
    pub held: Vec<String>,
    /// The key each Hold step pressed, by the key as the step names it ("W", or "$o1" for a setting): its Release lets go
    /// of that one, even when the setting was changed on the panel meanwhile.
    pub held_as: HashMap<String, String>,
    /// What is left of a wait cut short (the person, the window leaving the front, anti-AFK's turn), in ms.
    pub wait_left: Option<i32>,
    /// The process of the window its last turn played into; 0 before its first.
    pub pid: i32,
    pub focus_misses: i32,
    /// Its "recover" step (-1: none): the macro's own steps end there, the If stuck part follows.
    pub recover_at: i32,
    /// Running its If stuck part; from the top once that's done.
    pub recovering: bool,
    /// Since when what its steps look for hasn't been seen (time paused left out); None: it was just seen.
    pub miss_since: Option<Stamp>,
    pub paused_at: Option<Stamp>,
    /// Restarts in a row with nothing seen in between.
    pub restarts: i32,
    /// The feature playing now (its id); None: the main steps.
    pub section: Option<String>,
    /// Each feature's jumps, by its id.
    pub feature_jumps: HashMap<String, Vec<i32>>,
    /// The main steps' "End repeat"s of repeats inside no other block: each pass's end is a safe point.
    pub safe_ends: HashSet<i32>,
    /// Features still to play at this safe point, after the one playing.
    pub due: VecDeque<String>,
    /// Where the main steps go on once the features have played, and their repeats' passes meanwhile.
    pub return_to: i32,
    pub main_passes: Option<HashMap<i32, i32>>,
    /// Back from the features at a safe point: that step plays on without looking for them again.
    pub resumed: bool,
    /// Run a macro steps put in place: each step's own number in the macro as saved, by section ("" the main steps);
    /// none when it runs no other macro.
    pub origins: HashMap<String, Vec<i32>>,
    /// The macro the step playing now is from, when a Run a macro step put it here: its settings are read.
    pub owner: Option<Arc<Macro>>,
    /// The last web step, so a loop doesn't ask an address many times a second.
    pub last_web_at: Stamp,
    pub cancel: Cancel,
    /// For error.log: steps played, the last pause written, notes written once, the last picture match.
    pub steps_run: i32,
    pub logged_pause: Option<String>,
    /// The run before it on this account ended well: its start and end aren't logged (a repeat would fill the log).
    pub quiet: bool,
    pub noted: HashSet<String>,
    pub last_score: Option<f64>,
}

impl MacroRun {
    /// The step to show as playing (or where it stopped): the Run a macro step for the other macro's steps.
    pub fn shown_pc(&self) -> i32 {
        match self.origins.get(self.section.as_deref().unwrap_or("")) {
            Some(o) if !o.is_empty() => o[self.pc.clamp(0, o.len() as i32 - 1) as usize],
            _ => self.pc,
        }
    }
    /// The steps playing now: the feature's, or the main ones.
    pub fn steps(&self) -> &[MacroStep] {
        match self.feature() {
            Some(f) => &f.steps,
            None => &self.r#macro.steps,
        }
    }
    pub fn jumps_now(&self) -> &[i32] {
        match self.section.as_ref().and_then(|id| self.feature_jumps.get(id)) {
            Some(j) => j,
            None => &self.jumps,
        }
    }
    pub fn feature(&self) -> Option<&MacroFeature> {
        let id = self.section.as_ref()?;
        self.r#macro.features.iter().find(|f| &f.id == id)
    }
    pub fn finished(&self) -> bool {
        matches!(self.status.as_str(), "done" | "failed" | "stopped")
    }
    /// For error.log: the macro's and the account's ids (first 8 characters), never a name.
    pub fn tag(&self) -> String {
        format!("{}{} on account {}", &self.r#macro.id.to_string()[..8], if self.trial { " (Try it)" } else { "" }, &self.account_id.to_string()[..8])
    }
    fn jump(&self, at: i32) -> i32 {
        self.jumps_now()[at as usize]
    }
}

pub type RunRef = Arc<Mutex<MacroRun>>;

/// A feature on one account: when it last played, and why it didn't play through last time (None: it did).
#[derive(Clone, Debug, Default)]
pub struct FeatureRecord {
    pub last_ran: Option<Stamp>,
    pub failed: Option<String>,
    /// The step (0-based) it stopped at when it failed.
    pub failed_step: i32,
}

/// What the player keeps, behind one lock (never held while playing or waiting).
#[derive(Default)]
pub struct State {
    runs: Vec<RunRef>,
    next: usize,
    loop_running: bool,
    /// The turn going on now (one at a time: InputTurn), for release_all.
    turn: Option<Arc<Turn>>,
    full_screen_since: Option<Stamp>,
    /// error.log: the last queue line per macro that started nothing, and whether the full-screen wait ran out.
    queue_noted: HashMap<Guid, Option<String>>,
    full_screen_noted: bool,
    /// Each account's variables (setVar, addVar), in memory only.
    pub vars: HashMap<Guid, HashMap<String, f64>>,
    /// Each account's text variables (a Set variable step's text, a Read step's words), in memory only.
    pub texts: HashMap<Guid, HashMap<String, String>>,
    /// Each feature's record on each account, kept across runs (a rejoin doesn't make one play again at once).
    pub features: HashMap<(Guid, Guid, String), FeatureRecord>,
}

pub type ChangedFn = Arc<dyn Fn() + Send + Sync>;
pub type PlayedFn = Arc<dyn Fn(Guid, i32) + Send + Sync>;
pub type RejoinFn = Arc<dyn Fn(Guid) -> Option<String> + Send + Sync>;
pub type AlertFn = Arc<dyn Fn(Guid, &Macro, &str, Option<Vec<u8>>) + Send + Sync>;
pub type FindMacroFn = Arc<dyn Fn(Guid) -> Option<Macro> + Send + Sync>;

/// How a step left the turn: go on, give the window back, or the run is over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Then {
    Next,
    EndTurn,
    Over,
}

/// A wait longer than this gives the window back until it is over.
pub const LONG_WAIT: Duration = Duration::from_millis(1500);
/// One turn plays this long at most while other windows wait.
const TURN_SHARE: Duration = Duration::from_secs(8);
/// Between two turns: the anti-AFK nudge or screenshot asked meanwhile goes first.
const TURN_GAP: i32 = 120;
/// How often a wait on the picture looks again.
const LOOK_EVERY: i32 = 500;
const READ_EVERY: i32 = 1000;
const STILL_EVERY: i32 = 250;
/// A full-screen app in front holds runs back this long at most (the person is away: see hold).
const FULL_SCREEN_WAIT: Duration = Duration::from_secs(5 * 60);
/// Steps one turn runs at most (a loop of variables only would never end it otherwise).
const TURN_STEPS: i32 = 2000;
/// A turn this old hands the PC on as soon as anti-AFK asks for it, held keys or not: every other window's nudge waits
/// behind it, and Roblox kicks a window idle for 20 minutes.
pub const AFK_BUDGET: Duration = Duration::from_secs(30);
/// A real-time step (clickColour again and again, or each patch) gives up after this (or its timeout if longer): a
/// colour that never goes (it also matches something always there) would keep the window for good.
pub const REAL_TIME_CAP: Duration = Duration::from_secs(10 * 60);
/// A wait in front is waited in pieces this long, so the person, the window leaving the front, the run's longest time
/// and anti-AFK's turn each end it at once.
const WAIT_SLICE: i32 = 250;
const MAX_KEPT: usize = 200;
/// Restarts in a row, nothing seen in between, before a stuck run gives up.
pub const MAX_RESTARTS: i32 = 5;
/// Steps of a run with its Run a macro steps put in place: ten macros of the most steps.
const MAX_EXPANDED: usize = 10 * crate::core::macro_model::macro_rules::MAX_STEPS as usize;

/// Plays macros into Roblox windows. Runs (macro × account) are queued; one window at a time gets a turn, in rotation:
/// a turn plays steps until a long wait or a wait on the picture, then the window is given back and the next one goes,
/// so long macros on many windows share the PC rather than one holding it. A turn brings its window forward only once
/// it has input to send, takes InputTurn (anti-AFK's nudge and a screenshot take theirs between two turns), and stops
/// when a person touches the PC, or the window is no longer in front. Everything held is let go whenever a turn ends.
/// Runs wait while the PC is locked, in use, or a full-screen app is in front.
///
/// The C# player ran on the window's thread between awaits; here it plays on a std thread of its own, and every other
/// call (queue, stop, the page reading runs) takes the state lock for a moment.
pub struct MacroPlayer {
    desk: Arc<dyn IMacroDesk>,
    state: Mutex<State>,
    loop_thread: Mutex<Option<JoinHandle<()>>>,
    /// Something to show changed (throttled by the app).
    pub changed: RwLock<Option<ChangedFn>>,
    /// A turn sent input to the window of this process (anti-AFK counts it).
    pub played: RwLock<Option<PlayedFn>>,
    /// Asks Bloxkeep to rejoin the account's server; the answer is why it can't, or None.
    pub rejoin: RwLock<Option<RejoinFn>>,
    /// A Discord alert and an activity entry about the account (macro, text, the window's picture as PNG or None).
    pub alert: RwLock<Option<AlertFn>>,
    /// Recording: every run waits.
    pub recording: AtomicBool,
    /// For the checks: random timing and offsets.
    pub random: Mutex<Rng>,
    /// A saved macro by its id (a copy), for Run a macro steps; None when it's gone.
    pub find_macro: RwLock<Option<FindMacroFn>>,
    /// The clipboard, files and web addresses (Read and Write steps).
    pub outside: RwLock<Arc<dyn IMacroOutside>>,
}

/// A window's turn: brought forward only once there is input for it.
pub struct Turn {
    input: Mutex<Option<Arc<dyn IMacroTurn>>>,
    pub window: MacroWindow,
    pub run: RunRef,
    pub started: Stamp,
    /// The picture for this step (taken once per look).
    picture: Mutex<Option<Arc<Picture>>>,
}

impl Turn {
    pub fn input(&self) -> Option<Arc<dyn IMacroTurn>> {
        lk(&self.input).clone()
    }

    /// Lets go of everything the run holds, now (a restart carries nothing over).
    fn let_go(&self) {
        let held = {
            let mut r = lk(&self.run);
            r.held_as.clear();
            std::mem::take(&mut r.held)
        };
        if let Some(input) = self.input() {
            for h in &held {
                press_held(&*input, h, true);
            }
        }
    }

    /// Everything let go and the PC given back now, with no wait (release_all).
    fn abort(&self) {
        let Some(input) = self.input() else { return };
        self.let_go();
        *lk(&self.input) = None;
        input.abort(); // a press halfway, the pointer, the windows
    }
}

impl MacroPlayer {
    pub fn new(desk: Arc<dyn IMacroDesk>) -> Arc<MacroPlayer> {
        Arc::new(MacroPlayer {
            desk,
            state: Mutex::new(State::default()),
            loop_thread: Mutex::new(None),
            changed: RwLock::new(None),
            played: RwLock::new(None),
            rejoin: RwLock::new(None),
            alert: RwLock::new(None),
            recording: AtomicBool::new(false),
            random: Mutex::new(Rng::new()),
            find_macro: RwLock::new(None),
            outside: RwLock::new(Arc::new(MacroOutside)),
        })
    }

    /// The player's state (variables, texts, feature records) for the app to read.
    pub fn state(&self) -> MutexGuard<'_, State> {
        lk(&self.state)
    }

    fn st(&self) -> MutexGuard<'_, State> {
        lk(&self.state)
    }

    fn changed(&self) {
        let f = self.changed.read().unwrap_or_else(|e| e.into_inner()).clone();
        if let Some(f) = f {
            f();
        }
    }

    fn outside(&self) -> Arc<dyn IMacroOutside> {
        self.outside.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn delay(&self, ms: i32, ct: &Cancel) -> Result<(), Halt> {
        self.desk.delay(ms, ct).map_err(|_| Halt::Cancelled)
    }

    /// Runs, finished ones too (the last of each macro and account), in the order queued.
    pub fn runs(&self) -> Vec<RunRef> {
        self.st().runs.clone()
    }

    pub fn active(&self) -> bool {
        self.st().runs.iter().any(|r| !lk(r).finished())
    }

    /// Queues the macro on these accounts; one already running there is left alone, except the editor's Try it of it,
    /// which the saved macro takes over from (Run, its hotkey or trigger did nothing, with no word, before). Returns how
    /// many were queued.
    pub fn queue(self: &Arc<Self>, r#macro: Macro, accounts: &[Guid], trial: bool) -> Res<i32> {
        let mut m = r#macro;
        let mut origins: HashMap<String, Vec<i32>> = HashMap::new();
        if macro_rules::all_steps(&m).any(|s| s.kind == "call") {
            m = macro_rules::copy(&m);
            let id = m.id;
            let (steps, o) = self.expand(&m.steps, &[id]);
            m.steps = steps;
            origins.insert(String::new(), o);
            let mut expanded = Vec::new();
            for f in &m.features {
                expanded.push(self.expand(&f.steps, &[id]));
            }
            for (f, (steps, o)) in m.features.iter_mut().zip(expanded) {
                f.steps = steps;
                origins.insert(f.id.clone(), o);
            }
        }
        let jumps = macro_rules::jumps(&m.steps)?;
        let mut feature_jumps = HashMap::new();
        for f in &m.features {
            if feature_jumps.insert(f.id.clone(), macro_rules::jumps(&f.steps)?).is_some() {
                return Err(Fail::other("An item with the same key has already been added."));
            }
        }
        let safe_ends = safe_ends(&m.steps);
        let recover_at = m.steps.iter().position(|st| st.kind == "recover").map_or(-1, |i| i as i32);
        let m = Arc::new(m);
        let now = self.desk.now();
        let mut queued = 0;
        let mut taken_over: Vec<RunRef> = Vec::new();
        let (mut asked, mut busy, mut already, mut quiet_queued) = (0, 0, 0, 0);
        let start_loop;
        {
            let mut st = self.st();
            let mut seen = HashSet::new();
            for &account in accounts {
                if !seen.insert(account) {
                    continue;
                }
                asked += 1;
                // One macro on a window at a time: two would take turns on it, one's clicks in the middle of the other's.
                if busy_with_in(&st.runs, account, m.id).is_some() {
                    busy += 1;
                    continue;
                }
                let going = st.runs.iter().position(|r| {
                    let g = lk(r);
                    !g.finished() && g.r#macro.id == m.id && g.account_id == account
                });
                if let Some(at) = going {
                    let is_trial = lk(&st.runs[at]).trial;
                    if is_trial && !trial {
                        // One run of a macro per account in the list (the page shows that one): Try it's goes.
                        taken_over.push(st.runs.remove(at));
                    } else {
                        already += 1;
                        continue;
                    }
                }
                let quiet = st.runs.iter().any(|r| {
                    let g = lk(r);
                    g.finished() && g.r#macro.id == m.id && g.account_id == account && g.status == "done" && g.reason.is_none()
                });
                if quiet {
                    quiet_queued += 1;
                }
                st.runs.retain(|r| {
                    let g = lk(r);
                    !(g.finished() && g.r#macro.id == m.id && g.account_id == account)
                });
                st.runs.push(Arc::new(Mutex::new(MacroRun {
                    r#macro: m.clone(),
                    account_id: account,
                    jumps: jumps.clone(),
                    trial,
                    status: "waiting".into(),
                    reason: None,
                    pc: 0,
                    loops: 0,
                    started_at: now,
                    ends_at: Some(now.plus_secs(m.max_minutes as f64 * 60.0)),
                    resume_at: now,
                    deadline: None,
                    last_frame: None,
                    still_since: None,
                    passes: HashMap::new(),
                    again: -1,
                    held: Vec::new(),
                    held_as: HashMap::new(),
                    wait_left: None,
                    pid: 0,
                    focus_misses: 0,
                    recover_at,
                    recovering: false,
                    miss_since: None,
                    paused_at: None,
                    restarts: 0,
                    section: None,
                    feature_jumps: feature_jumps.clone(),
                    safe_ends: safe_ends.clone(),
                    due: VecDeque::new(),
                    return_to: 0,
                    main_passes: None,
                    resumed: false,
                    origins: origins.clone(),
                    owner: None,
                    last_web_at: min_stamp(),
                    cancel: Cancel::new(),
                    steps_run: 0,
                    logged_pause: None,
                    quiet,
                    noted: HashSet::new(),
                    last_score: None,
                })));
                queued += 1;
            }
            while st.runs.len() > MAX_KEPT {
                let Some(old) = st.runs.iter().position(|r| lk(r).finished()) else { break };
                st.runs.remove(old);
            }
            start_loop = queued > 0 && !st.loop_running;
            if start_loop {
                st.loop_running = true;
            }
        }
        // Stopped once the list is as it will be: what the try holds is let go as its turn ends.
        for run in &taken_over {
            self.finish(run, "stopped", Some("byUser"));
        }
        let mut queue_note = format!("queued on {queued} of {asked} accounts{}", if trial { " (Try it)" } else { "" });
        if busy > 0 {
            queue_note.push_str(&format!(", {busy} busy with another macro"));
        }
        if already > 0 {
            queue_note.push_str(&format!(", {already} already running it"));
        }
        {
            let mut st = self.st();
            // A repeat that finds it still running would write the same line every period.
            let last = st.queue_noted.get(&m.id).cloned().flatten();
            if queued > quiet_queued || last.as_deref() != Some(queue_note.as_str()) {
                log::note(&format!("Macro {}: {queue_note}.", &m.id.to_string()[..8]));
            }
            st.queue_noted.insert(m.id, if queued > 0 { None } else { Some(queue_note) });
        }
        if queued > 0 {
            self.changed();
            if start_loop {
                let me = self.clone();
                let handle = match crate::net::Named::new("Bloxkeep macros".into()).spawn(move || me.run_loop()) {
                    Ok(h) => h,
                    Err(e) => {
                        // No loop after all: the next run may start one.
                        self.st().loop_running = false;
                        return Err(Fail::from(e));
                    }
                };
                *lk(&self.loop_thread) = Some(handle);
            }
        }
        Ok(queued)
    }

    /// Another macro going on (or waiting) on the account; None when none.
    pub fn busy_with(&self, account: Guid, macro_id: Guid) -> Option<Arc<Macro>> {
        busy_with_in(&self.st().runs, account, macro_id)
    }

    /// The loop running the queue, for the checks to wait on (Idle).
    pub fn idle(&self) {
        let handle = lk(&self.loop_thread).take();
        if let Some(h) = handle {
            let _ = h.join();
        }
    }

    /// Stops one macro's runs (all when None), on one account when given, only the editor's Try it (whatever id it
    /// got) when `trial_only`; what they hold is let go as their turn ends.
    pub fn stop(&self, macro_id: Option<Guid>, reason: &str, account_id: Option<Guid>, trial_only: bool) -> i32 {
        let targets: Vec<RunRef> = self
            .st()
            .runs
            .iter()
            .filter(|r| {
                let g = lk(r);
                !g.finished()
                    && (macro_id.is_none() || Some(g.r#macro.id) == macro_id || trial_only)
                    && (account_id.is_none() || Some(g.account_id) == account_id)
                    && (!trial_only || g.trial)
            })
            .cloned()
            .collect();
        for run in &targets {
            self.finish(run, "stopped", Some(reason));
        }
        if !targets.is_empty() {
            self.changed();
        }
        targets.len() as i32
    }

    /// Bloxkeep closing: what the turn going on holds (keys and buttons held, a press or click halfway) let go and the
    /// PC given back before this returns, then every run stopped.
    pub fn release_all(&self, why: &str) {
        let turn = self.st().turn.clone();
        log::note(&format!(
            "Macro: letting go of everything ({why}){}.",
            match &turn {
                None => String::new(),
                Some(t) => format!(", a turn on window {} was going on", t.window.pid),
            }
        ));
        // Stopped first: the turn, if it goes on a moment, never brings a window forward again.
        self.stop(None, why, None, false);
        if let Some(t) = turn {
            t.abort();
        }
    }

    /// The vault went: runs stopped, variables forgotten, finished runs too.
    pub fn clear(&self, reason: &str) {
        self.stop(None, reason, None, false);
        {
            let mut st = self.st();
            st.vars.clear();
            st.texts.clear();
            st.features.clear();
            st.runs.clear();
        }
        self.changed();
    }

    fn finish(&self, run: &RunRef, status: &str, reason: Option<&str>) {
        let now = self.desk.now();
        let mut r = lk(run);
        if r.finished() {
            return;
        }
        r.status = status.into();
        r.reason = reason.map(Into::into);
        r.ends_at = Some(now);
        r.cancel.cancel();
        if r.quiet && status == "done" && reason.is_none() {
            return;
        }
        log::note(&format!(
            "Macro {}: {status}{} after {} steps, at step {}{}, window {}.",
            r.tag(),
            reason.map_or(String::new(), |x| format!(" ({x})")),
            r.steps_run,
            r.shown_pc() + 1,
            r.section.as_ref().map_or(String::new(), |f| format!(" of feature {f}")),
            r.pid
        ));
    }

    fn set(&self, run: &RunRef, status: &str, reason: Option<&str>) {
        let now = self.desk.now();
        {
            let mut r = lk(run);
            // Stopped meanwhile (Stop, Bloxkeep closing): a late status would bring a finished run back.
            if r.finished() || r.status == status && r.reason.as_deref() == reason {
                return;
            }
            // Each pause reason once in a row: the person at the PC would write one every turn.
            if status == "paused" && reason != r.logged_pause.as_deref() {
                r.logged_pause = reason.map(Into::into);
                log::note(&format!("Macro {}: paused ({}) at step {}.", r.tag(), reason.unwrap_or(""), r.shown_pc() + 1));
            }
            // Time held back isn't time stuck.
            if status == "paused" {
                if r.paused_at.is_none() {
                    r.paused_at = Some(now);
                }
            } else if let Some(paused) = r.paused_at {
                if let Some(since) = r.miss_since {
                    r.miss_since = Some(since.plus_secs(now.secs_since(paused)));
                }
                r.paused_at = None;
            }
            r.status = status.into();
            r.reason = reason.map(Into::into);
        }
        self.changed();
    }

    fn live(&self) -> Vec<RunRef> {
        self.st().runs.iter().filter(|r| !lk(r).finished()).cloned().collect()
    }

    fn run_loop(self: Arc<Self>) {
        if program::catch_unwind(AssertUnwindSafe(|| self.run_loop_())).is_ok() {
            return;
        }
        // A panic outside a turn: the loop is gone, so its runs end (a run left "waiting" would never play, and the
        // flag left set would keep every later run from starting one).
        let live = {
            let mut st = self.st();
            st.loop_running = false;
            st.turn = None;
            st.runs.iter().filter(|r| !lk(r).finished()).cloned().collect::<Vec<_>>()
        };
        for run in &live {
            self.finish(run, "failed", None);
        }
        self.changed();
    }

    fn run_loop_(&self) {
        let none = Cancel::new();
        loop {
            {
                let mut st = self.st();
                if !st.runs.iter().any(|r| !lk(r).finished()) {
                    st.loop_running = false;
                    break;
                }
            }
            let now = self.desk.now();
            for run in self.live() {
                let over = lk(&run).ends_at.is_some_and(|e| e <= now);
                if over {
                    self.finish(&run, "stopped", Some("maxTime"));
                    self.changed();
                }
            }
            if let Some(held) = self.hold() {
                for run in self.live() {
                    self.set(&run, "paused", Some(&held));
                }
                let _ = self.desk.delay(500, &none);
                continue;
            }
            // Free again: runs held for the PC's sake wait for their turn.
            for run in self.live() {
                let free = {
                    let r = lk(&run);
                    r.status == "paused" && matches!(r.reason.as_deref(), Some("personActive" | "locked" | "fullscreen" | "recording"))
                };
                if free {
                    self.set(&run, "waiting", None);
                }
            }
            let Some(ready) = self.next(now) else {
                let soonest = self.live().iter().map(|r| lk(r).resume_at).min().unwrap_or(now);
                let _ = self.desk.delay(ms_since(soonest, now).clamp(10.0, 250.0) as i32, &none);
                continue;
            };
            match self.turn_(&ready) {
                // Stopped while waiting for its turn.
                Ok(()) | Err(Halt::Cancelled) => {}
                Err(Halt::Fail(f)) => {
                    log::write(&f);
                    self.finish(&ready, "failed", None);
                }
                Err(other) => {
                    log::write(&format!("{other:?}"));
                    self.finish(&ready, "failed", None);
                }
            }
            self.changed();
            let _ = self.desk.delay(TURN_GAP, &none);
        }
        self.changed();
    }

    /// Why every run waits now: recording, then the desktop's reasons. A full-screen app holds them back five minutes
    /// at most: the person isn't at the PC (that would be "personActive"), so it is likely a video left playing.
    fn hold(&self) -> Option<String> {
        if self.recording.load(Ordering::SeqCst) {
            return Some("recording".into());
        }
        let held = self.desk.hold();
        let now = self.desk.now();
        let mut st = self.st();
        if held.as_deref() != Some("fullscreen") {
            st.full_screen_since = None;
            return held;
        }
        let since = *st.full_screen_since.get_or_insert(now);
        if ms_since(now, since) < ms(FULL_SCREEN_WAIT) {
            st.full_screen_noted = false;
            return held;
        }
        if !st.full_screen_noted {
            st.full_screen_noted = true;
            log::note("Macro: a full-screen app has been in front 5 minutes; macros play anyway.");
        }
        None
    }

    /// The next run due, in rotation.
    fn next(&self, now: Stamp) -> Option<RunRef> {
        let mut st = self.st();
        let live: Vec<RunRef> = st.runs.iter().filter(|r| !lk(r).finished()).cloned().collect();
        for i in 0..live.len() {
            let run = &live[(st.next + i) % live.len()];
            if lk(run).resume_at <= now {
                st.next = (st.next + i + 1) % live.len().max(1);
                return Some(run.clone());
            }
        }
        None
    }

    fn others_ready(&self, run: &RunRef) -> bool {
        let now = self.desk.now();
        self.st().runs.iter().any(|r| {
            !Arc::ptr_eq(r, run) && {
                let g = lk(r);
                !g.finished() && g.resume_at <= now
            }
        })
    }

    /// One window's turn.
    fn turn_(&self, run: &RunRef) -> Result<(), Halt> {
        let account = lk(run).account_id;
        let Some(window) = self.desk.window(account) else {
            self.finish(run, "failed", Some("windowGone"));
            return Ok(());
        };
        if window.elevated {
            self.finish(run, "failed", Some("elevated"));
            return Ok(());
        }
        if !window.in_game {
            self.set(run, "paused", Some("notInGame"));
            lk(run).resume_at = self.desk.now().plus_secs(2.0);
            return Ok(());
        }
        {
            let mut r = lk(run);
            if r.pid != 0 && r.pid != window.pid {
                log::note(&format!("Macro {}: its window changed from {} to {}; it plays again from the top.", r.tag(), r.pid, window.pid));
                start_over(&mut r);
            } else if r.pid == 0 && !r.quiet {
                log::note(&format!(
                    "Macro {}: first turn, window {} ({}x{}, fps limit {}).",
                    r.tag(),
                    window.pid,
                    window.client.width,
                    window.client.height,
                    window.fps
                ));
            }
            r.pid = window.pid;
        }
        let ct = lk(run).cancel.clone();
        let lease = match input_turn::try_enter("macro") {
            Some(l) => l,
            None => {
                self.set(run, "paused", Some("busy"));
                input_turn::enter("macro", Some(&ct)).map_err(|_| Halt::Cancelled)?
            }
        };
        let turn = Arc::new(Turn { input: Mutex::new(None), window, run: run.clone(), started: self.desk.now(), picture: Mutex::new(None) });
        self.st().turn = Some(turn.clone());
        // A panic in a step still ends the turn: keys let go, Bloxkeep's windows clickable again, the PC handed on.
        let body = match program::catch_unwind(AssertUnwindSafe(|| self.turn_body(&turn, &ct))) {
            Ok(body) => body,
            Err(_) => Err(Halt::Fail(Fail::other(format!("Macro {}: a step panicked.", lk(run).tag())))),
        };
        // Handed on even when giving the window back failed: every nudge, picture and hotkey waits for it.
        if program::catch_unwind(AssertUnwindSafe(|| self.end_turn(&turn))).is_err() {
            // The desk's own turn gives the PC back as it goes (its Drop); here only what this one keeps is let go of.
            lk(&turn.input).take();
        }
        lease.dispose();
        drop(lease);
        {
            let mut st = self.st();
            if st.turn.as_ref().is_some_and(|t| Arc::ptr_eq(t, &turn)) {
                st.turn = None;
            }
        }
        match body {
            // Stopped (by the person, a lock, the window going): what it held was let go above.
            Err(Halt::Cancelled) => Ok(()),
            other => other,
        }
    }

    fn turn_body(&self, turn: &Turn, ct: &Cancel) -> Result<(), Halt> {
        let run = &turn.run;
        let recovering = lk(run).recovering;
        let reason = if recovering {
            Some("recovering")
        } else if self.ime(run) {
            Some("imeOn")
        } else {
            None
        };
        self.set(run, "running", reason);
        // Keys it held when its last turn ended (let go then) are held again before anything counts: a wait with W held
        // is a walk, not standing still.
        let holding = !lk(run).held.is_empty();
        if holding && self.front(turn, ct)?.is_none() {
            return Ok(());
        }
        let started = self.desk.now();
        let mut steps = -1;
        loop {
            steps += 1;
            if ct.is_cancelled() {
                return Err(Halt::Cancelled);
            }
            let now = self.desk.now();
            let over = lk(run).ends_at.is_some_and(|e| e <= now);
            if over {
                self.finish(run, "stopped", Some("maxTime")); // its longest time, even in the middle of a turn
                return Ok(());
            }
            let (feature_done, pc, limit, recovering) = {
                let r = lk(run);
                // A feature ends at its own last step: the main steps' count (or their If stuck part) would cut a longer
                // one short in its middle.
                let limit = if r.section.is_some() {
                    r.steps().len() as i32
                } else if r.recovering || r.recover_at < 0 {
                    r.r#macro.steps.len() as i32
                } else {
                    r.recover_at
                };
                (r.section.is_some() && r.pc >= r.steps().len() as i32, r.pc, limit, r.recovering)
            };
            if feature_done {
                self.feature_ended(run, None);
                continue;
            }
            if pc >= limit {
                if recovering {
                    // Unstuck: from the top.
                    {
                        let mut r = lk(run);
                        r.recovering = false;
                        r.pc = 0;
                    }
                    self.set(run, "running", None);
                    continue;
                }
                // The end of the main steps: the features on play first.
                if self.safe_point(run) {
                    continue;
                }
                self.finish(run, "done", None);
                return Ok(());
            }
            if self.interrupted(turn).is_some() {
                return Ok(());
            }
            // A feature switched off on the panel: it stops here, letting go of what it holds.
            let feature_off = {
                let r = lk(run);
                r.feature().is_some_and(|f| !switch_on(&r.r#macro, f))
            };
            if feature_off {
                turn.let_go();
                self.feature_ended(run, None);
                continue;
            }
            let raw = {
                let mut r = lk(run);
                let raw = r.steps()[r.pc as usize].clone();
                r.owner = raw.owner.clone();
                raw
            };
            let step = match macro_values::resolve_step(&raw, &mut |name: &str| self.value(run, name)) {
                Ok(s) => s.into_owned(),
                Err(_missing) => {
                    self.finish(run, "failed", Some("missingValue"));
                    return Ok(());
                }
            };
            lk(run).steps_run += 1;
            let then = match self.step(turn, &step, ct) {
                Ok(t) => t,
                Err(Halt::Missing) => {
                    self.finish(run, "failed", Some("missingValue"));
                    return Ok(());
                }
                Err(Halt::Expr) => {
                    self.finish(run, "failed", Some("badFormula"));
                    return Ok(());
                }
                Err(Halt::Covered) => {
                    // Another app's window over the place: no click into it. Tried again in a moment.
                    self.set(run, "paused", Some("covered"));
                    lk(run).resume_at = self.desk.now().plus_secs(2.0);
                    return Ok(());
                }
                Err(e) => return Err(e),
            };
            let finished = lk(run).finished();
            if then == Then::Over || finished {
                return Ok(());
            }
            if then == Then::EndTurn {
                return Ok(());
            }
            let now = self.desk.now();
            let held_empty = lk(run).held.is_empty();
            if steps >= TURN_STEPS || (ms_since(now, started) >= ms(TURN_SHARE) && held_empty && self.others_ready(run)) {
                return Ok(()); // its share is used up: the others go, it carries on next time round
            }
        }
    }

    /// Why the turn can't go on now: the run's longest time is up (Over), the person touched the PC, the window left the
    /// front, or anti-AFK has waited for the PC while this turn ran past AFK_BUDGET (EndTurn: what it holds is let go as
    /// the turn ends, and held again next turn); None: it goes on. Asked before every step and inside every step that
    /// keeps the window a while.
    fn interrupted(&self, turn: &Turn) -> Option<Then> {
        let run = &turn.run;
        let now = self.desk.now();
        let over = lk(run).ends_at.is_some_and(|e| e <= now);
        if over {
            self.finish(run, "stopped", Some("maxTime"));
            return Some(Then::Over);
        }
        if let Some(input) = turn.input() {
            if input.person_input() {
                self.set(run, "paused", Some("personActive"));
                return Some(Then::EndTurn);
            }
            if !input.still_front() {
                self.set(run, "paused", Some("focusRefused"));
                lk(run).resume_at = self.desk.now().plus_secs(2.0);
                return Some(Then::EndTurn);
            }
        }
        if ms_since(self.desk.now(), turn.started) >= ms(AFK_BUDGET) && input_turn::waits("afk") {
            return Some(self.end_now(run)); // the nudge goes, then this run carries on where it was
        }
        None
    }

    fn feature_record<'a>(st: &'a mut State, r: &MacroRun, feature: &str) -> &'a mut FeatureRecord {
        st.features.entry((r.r#macro.id, r.account_id, feature.to_string())).or_default()
    }

    /// A feature due now: switched on, and not played within its every_ms.
    fn due(&self, st: &mut State, r: &MacroRun, f: &MacroFeature) -> bool {
        if !switch_on(&r.r#macro, f) {
            return false;
        }
        let Some(every) = f.every_ms else { return true };
        let Some(last) = Self::feature_record(st, r, &f.id).last_ran else { return true };
        ms_since(self.desk.now(), last) >= every as f64
    }

    /// A safe point of the main steps (their end, or the end of a pass of a repeat inside no other block): the features
    /// due play now, whole, in the list's order, then the main steps go on from here. Not while a key is held (they
    /// would play with it down), nor in the If stuck part. True when one starts.
    fn safe_point(&self, run: &RunRef) -> bool {
        let started = {
            let mut st = self.st();
            let mut r = lk(run);
            if r.resumed {
                r.resumed = false; // back from them: this step plays on
                return false;
            }
            if r.recovering || !r.held.is_empty() || r.r#macro.features.is_empty() {
                return false;
            }
            let m = r.r#macro.clone();
            for f in &m.features {
                if self.due(&mut st, &r, f) {
                    r.due.push_back(f.id.clone());
                }
            }
            let Some(first) = r.due.pop_front() else { return false };
            r.return_to = r.pc;
            r.main_passes = Some(r.passes.clone());
            start_feature(&mut r, &first);
            true
        };
        if started {
            self.changed();
        }
        started
    }

    /// A feature played through, switched off, or skipped (`failed`: why, at the step it stopped): the next one due
    /// plays, else the main steps go on where they were.
    fn feature_ended(&self, run: &RunRef, failed: Option<&str>) {
        let now = self.desk.now();
        {
            let mut st = self.st();
            let mut r = lk(run);
            let section = r.section.clone().unwrap_or_default();
            let failed_step = if failed.is_none() { 0 } else { r.pc };
            let record = Self::feature_record(&mut st, &r, &section);
            record.last_ran = Some(now);
            record.failed = failed.map(Into::into);
            record.failed_step = failed_step;
            if let Some(next) = r.due.pop_front() {
                start_feature(&mut r, &next);
            } else {
                r.section = None;
                r.pc = r.return_to;
                r.passes.clear();
                if let Some(main) = r.main_passes.take() {
                    for (at, passes) in main {
                        r.passes.insert(at, passes);
                    }
                }
                r.again = -1;
                clear_wait(&mut r);
                r.resumed = true;
            }
        }
        self.changed();
    }

    fn ime(&self, run: &RunRef) -> bool {
        let m = lk(run).r#macro.clone();
        macro_rules::all_steps(&m).any(|s| s.kind == "type" || matches!(s.kind.as_str(), "key" | "down") && macro_keys::telex_rewrites(s.key.as_deref().unwrap_or("")))
            && self.desk.ime_on()
    }

    /// The window in front, or None when Windows wouldn't (the run waits and tries again).
    fn front(&self, turn: &Turn, ct: &Cancel) -> Result<Option<Arc<dyn IMacroTurn>>, Halt> {
        if let Some(input) = turn.input() {
            return Ok(Some(input));
        }
        if ct.is_cancelled() {
            return Err(Halt::Cancelled); // stopped (Bloxkeep closing): no window comes forward
        }
        let Some(input) = self.desk.begin_turn(&turn.window, ct)? else {
            let misses = {
                let mut r = lk(&turn.run);
                r.focus_misses += 1;
                r.focus_misses
            };
            self.set(&turn.run, "paused", Some("focusRefused"));
            // 2 s, 4 s… up to 30 s between tries.
            let secs = 30.min(2 << (misses - 1).clamp(0, 4));
            lk(&turn.run).resume_at = self.desk.now().plus_secs(secs as f64);
            return Ok(None);
        };
        let held = {
            let mut r = lk(&turn.run);
            r.focus_misses = 0;
            r.held.clone()
        };
        *lk(&turn.input) = Some(input.clone());
        // Keys and buttons held when its last turn ended are held again.
        for h in &held {
            press_held(&*input, h, false);
        }
        Ok(Some(input))
    }

    fn picture(&self, turn: &Turn) -> Option<Arc<Picture>> {
        *lk(&turn.picture) = None;
        let input = turn.input();
        let picture = self.desk.picture(&turn.window, input.as_deref()).map(Arc::new);
        *lk(&turn.picture) = picture.clone();
        if picture.is_none() {
            let mut r = lk(&turn.run);
            let text = format!("couldn't take a picture of window {} at step {}.", turn.window.pid, r.shown_pc() + 1);
            note_once(&mut r, "picture", &text);
        }
        picture
    }

    fn end_turn(&self, turn: &Turn) {
        *lk(&turn.picture) = None;
        let Some(input) = lk(&turn.input).take() else { return };
        // Let go of everything: a key left down would go on "held" in whatever window comes in front.
        let held = lk(&turn.run).held.clone();
        for h in &held {
            press_held(&*input, h, true);
        }
        {
            let mut r = lk(&turn.run);
            if r.finished() {
                r.held.clear();
                r.held_as.clear();
            }
        }
        input.dispose();
        let account = lk(&turn.run).account_id;
        let played = self.played.read().unwrap_or_else(|e| e.into_inner()).clone();
        if let Some(f) = played {
            f(account, turn.window.pid);
        }
    }

    fn jitter(&self, ms: i32, jitter: i32) -> i32 {
        if jitter <= 0 { ms } else { (ms + lk(&self.random).next(-jitter, jitter + 1)).max(0) }
    }

    fn deadline_or(&self, run: &RunRef, timeout_ms: i32) {
        let now = self.desk.now();
        let mut r = lk(run);
        if r.deadline.is_none() {
            r.deadline = Some(after_ms(now, timeout_ms as f64));
        }
    }

    fn step(&self, turn: &Turn, s: &MacroStep, ct: &Cancel) -> Result<Then, Halt> {
        let run = &turn.run;
        let m = lk(run).r#macro.clone();
        let frame = frame(&turn.window);
        match s.kind.as_str() {
            "key" => {
                let Some(input) = self.front(turn, ct)? else { return Ok(Then::EndTurn) };
                let key = key_of(s.key.as_deref())?;
                input.key(key, false);
                let waited = self.delay(hold_for(self.jitter(s.ms.unwrap_or(60), m.jitter_ms), turn.window.fps), ct);
                input.key(key, true);
                waited?;
                self.delay(frame, ct)?;
            }
            "down" | "up" => {
                let Some(input) = self.front(turn, ct)? else { return Ok(Then::EndTurn) };
                let up = s.kind == "up";
                // Release lets go of what its Hold pressed: a setting changed on the panel meanwhile names another key.
                let named = {
                    let r = lk(run);
                    macro_rules::key_named(&r.steps()[r.pc as usize])
                };
                let mut key = s.key.clone().unwrap_or_default();
                if up && let Some(pressed) = lk(run).held_as.remove(&named) {
                    key = pressed;
                }
                if let Some(mouse) = macro_rules::mouse_key(&key) {
                    // A mouse button held where the pointer is: it must be the game's window there.
                    if !up && !input.clear(input.cursor_at()) {
                        return Err(Halt::Covered);
                    }
                    let changed = held_change(&mut lk(run).held, &key, up);
                    if changed {
                        input.button(mouse, up);
                    }
                } else {
                    let changed = held_change(&mut lk(run).held, &key, up);
                    if changed {
                        input.key(key_of(Some(&key))?, up);
                    }
                }
                if !up {
                    lk(run).held_as.insert(named, key);
                }
                self.delay(frame, ct)?;
            }
            "click" => {
                let Some(input) = self.front(turn, ct)? else { return Ok(Then::EndTurn) };
                let at = macro_geometry::client_point(input.client(), need(&s.x)?, need(&s.y)?);
                self.click(&*input, turn, at, s.button.as_deref().unwrap_or("left"), s.double == Some(true), ct, s.hold_ms.unwrap_or(0))?;
            }
            "clickImage" => {
                self.deadline_or(run, s.timeout_ms.unwrap_or(10_000));
                let found = self.find_image(turn, &need(&s.image)?, s.region.as_ref(), s.threshold.unwrap_or(0.9));
                let Some((rect, w, h)) = found else { return self.not_yet(turn, "notFound", LOOK_EVERY, ct, false) };
                let Some(input) = self.front(turn, ct)? else { return Ok(Then::EndTurn) };
                let client = input.client();
                // The picture's pixels to the client's (the same unless the window changed size since).
                let centre = Point {
                    x: client.x + ((rect.x as f64 + rect.width as f64 / 2.0) * client.width as f64 / w as f64) as i32,
                    y: client.y + ((rect.y as f64 + rect.height as f64 / 2.0) * client.height as f64 / h as f64) as i32,
                };
                lk(run).deadline = None;
                self.click(&*input, turn, centre, s.button.as_deref().unwrap_or("left"), false, ct, 0)?;
            }
            "camera" => {
                let Some(input) = self.front(turn, ct)? else { return Ok(Then::EndTurn) };
                self.camera(&*input, turn, s.dx.unwrap_or(0), s.dy.unwrap_or(0), s.ms.unwrap_or(200), s.button.as_deref() == Some("right"), ct)?;
            }
            "scroll" => {
                let Some(input) = self.front(turn, ct)? else { return Ok(Then::EndTurn) };
                let centre = macro_geometry::client_point(input.client(), 0.5, 0.5);
                if !input.clear(centre) {
                    return Err(Halt::Covered);
                }
                input.move_to(centre);
                self.delay(frame, ct)?;
                let notches = s.notches.unwrap_or(0);
                for _ in 0..notches.abs() {
                    input.wheel(notches.signum());
                    self.delay(40.max(frame), ct)?;
                }
            }
            "move" => {
                let Some(input) = self.front(turn, ct)? else { return Ok(Then::EndTurn) };
                let at = self.offset(macro_geometry::client_point(input.client(), need(&s.x)?, need(&s.y)?), m.jitter_px);
                input.move_to(at);
                self.delay(frame, ct)?;
            }
            "wait" => {
                // What is left of one cut short, else the whole of it.
                let left = lk(run).wait_left.take();
                let ms = match left {
                    Some(l) => l,
                    None => self.jitter(s.ms.unwrap_or(0), m.jitter_ms),
                };
                // A long wait gives the window back, unless keys are held (walking for ten seconds stays in front).
                let held_empty = lk(run).held.is_empty();
                if ms as f64 > ms_of(LONG_WAIT) && held_empty {
                    let now = self.desk.now();
                    {
                        let mut r = lk(run);
                        r.pc += 1;
                        r.resume_at = after_ms(now, ms as f64);
                    }
                    self.set(run, "waiting", None);
                    return Ok(Then::EndTurn);
                }
                // In front: in pieces, each asking whether the turn may go on. Cut short, the rest is waited next turn,
                // with the keys held again first.
                let until = after_ms(self.desk.now(), ms as f64);
                while self.desk.now() < until {
                    if let Some(then) = self.interrupted(turn) {
                        if then != Then::Over {
                            lk(run).wait_left = Some(ms_since(until, self.desk.now()).ceil() as i32);
                        }
                        return Ok(then);
                    }
                    self.delay((WAIT_SLICE as f64).min(ms_since(until, self.desk.now()).ceil()) as i32, ct)?;
                }
                lk(run).pc += 1;
                return Ok(Then::Next);
            }
            "waitImage" => {
                self.deadline_or(run, s.timeout_ms.unwrap_or(10_000));
                let seen = self.find_image(turn, &need(&s.image)?, s.region.as_ref(), s.threshold.unwrap_or(0.9)).is_some();
                if seen == (s.gone == Some(true)) {
                    return self.not_yet(turn, "notFound", LOOK_EVERY, ct, false);
                }
            }
            "waitText" => {
                self.deadline_or(run, s.timeout_ms.unwrap_or(10_000));
                let seen = self.find_text(turn, s.text.as_deref(), s.words.as_deref(), &s.extra, s.region.as_ref())?;
                if seen == (s.gone == Some(true)) {
                    return self.not_yet(turn, "timeout", READ_EVERY, ct, false);
                }
            }
            "waitColour" => {
                self.deadline_or(run, s.timeout_ms.unwrap_or(10_000));
                let seen = match self.picture(turn) {
                    None => false,
                    Some(picture) => match s.region.as_ref() {
                        Some(area) => {
                            let area = macro_vision::area(Some(area), picture.width, picture.height);
                            !macro_follow::blobs(&Pixels::of(&picture, Some(area)), &need(&s.colour)?, s.tolerance.unwrap_or(24), 2)?.is_empty()
                        }
                        None => macro_vision::colour_at(&picture, need(&s.x)?, need(&s.y)?, &need(&s.colour)?, s.tolerance.unwrap_or(24)),
                    },
                };
                if seen == (s.gone == Some(true)) {
                    return self.not_yet(turn, "timeout", LOOK_EVERY, ct, false);
                }
            }
            "waitStill" => {
                self.deadline_or(run, s.timeout_ms.unwrap_or(10_000));
                let mut done = false;
                if let Some(picture) = self.picture(turn) {
                    let gray = macro_vision::to_gray(&picture, None);
                    let frame_now = macro_vision::signature(&gray, macro_vision::area(s.region.as_ref(), gray.w, gray.h));
                    let now = self.desk.now();
                    let mut r = lk(run);
                    let still = r.last_frame.as_ref().is_some_and(|last| macro_vision::difference(last, &frame_now) <= macro_vision::STILL_DIFFERENCE);
                    if still {
                        if r.still_since.is_none() {
                            r.still_since = Some(now);
                        }
                    } else {
                        r.still_since = None;
                    }
                    r.last_frame = Some(frame_now);
                    if r.still_since.is_some_and(|since| ms_since(now, since) >= s.ms.unwrap_or(500) as f64) {
                        r.last_frame = None;
                        r.still_since = None;
                        done = true;
                    }
                }
                if !done {
                    return self.not_yet(turn, "timeout", STILL_EVERY, ct, true);
                }
            }
            "if" => {
                let test = s.test.as_ref().ok_or_else(nre)?;
                let yes = self.test(turn, test)?;
                if self.stuck(run) {
                    return self.failed(turn, "stuck");
                }
                // No: to the step after its else (or its end).
                let mut r = lk(run);
                r.pc = if yes { r.pc + 1 } else { r.jump(r.pc) + 1 };
                return Ok(Then::Next);
            }
            "else" => {
                // The if's own steps are done: past its end.
                let mut r = lk(run);
                r.pc = r.jump(r.pc) + 1;
                return Ok(Then::Next);
            }
            "loop" => {
                let mut r = lk(run);
                let start = r.pc;
                if r.again != start {
                    r.passes.insert(start, 0);
                }
                r.again = -1;
                if let Some(times) = s.times.filter(|t| *t > 0)
                    && r.passes.get(&start).copied().unwrap_or(0) >= times
                {
                    r.passes.remove(&start);
                    r.pc = r.jump(start) + 1;
                    return Ok(Then::Next);
                }
            }
            "loopUntil" => {
                let (start, maxed) = {
                    let mut r = lk(run);
                    let start = r.pc;
                    if r.again != start {
                        r.passes.insert(start, 0);
                    }
                    r.again = -1;
                    (start, s.max.is_some_and(|max| max > 0 && r.passes.get(&start).copied().unwrap_or(0) >= max))
                };
                let met = if maxed { true } else { self.test(turn, s.test.as_ref().ok_or_else(nre)?)? };
                if self.stuck(run) {
                    return self.failed(turn, "stuck");
                }
                if met {
                    let mut r = lk(run);
                    r.passes.remove(&start);
                    r.pc = r.jump(start) + 1;
                    return Ok(Then::Next);
                }
            }
            "endLoop" => {
                // The end of a pass of a repeat inside no other block: the features on play first.
                let (in_main, safe) = {
                    let r = lk(run);
                    (r.section.is_none(), r.safe_ends.contains(&r.pc))
                };
                if in_main && safe && self.safe_point(run) {
                    return Ok(Then::Next);
                }
                {
                    let mut r = lk(run);
                    let start = r.jump(r.pc);
                    *r.passes.entry(start).or_insert(0) += 1;
                    r.loops += 1;
                    r.again = start;
                    r.pc = start;
                }
                self.changed();
                // Round the loop: a turn that has played a while lets the others go first.
                let long = ms_since(self.desk.now(), turn.started) >= ms(LONG_WAIT);
                let held_empty = lk(run).held.is_empty();
                return Ok(if long && held_empty && self.others_ready(run) { self.end_now(run) } else { Then::Next });
            }
            "break" => {
                let mut r = lk(run);
                let end = r.jump(r.pc);
                let start = r.jump(end);
                r.passes.remove(&start);
                r.pc = end + 1;
                return Ok(Then::Next);
            }
            "endIf" => {}
            "type" => {
                let Some(input) = self.front(turn, ct)? else { return Ok(Then::EndTurn) };
                let text = self.alert_text(run, &need(&s.text)?);
                self.type_text(&*input, turn, &text, ct)?;
            }
            "setVar" | "addVar" => {
                if s.kind == "setVar"
                    && let Some(text) = &s.text
                {
                    let value = self.alert_text(run, text);
                    self.set_text(run, &need(&s.name)?, value);
                } else {
                    let by = match &s.expr {
                        Some(expr) => macro_expr::eval(expr, &mut |name: &str| self.number_of(run, name), None)?,
                        None => need(&s.value)?,
                    };
                    let name = need(&s.name)?;
                    let value = if s.kind == "setVar" { by } else { self.number_of(run, &name)? + by };
                    self.set_number(run, &name, value);
                }
            }
            "fail" => {
                self.finish(run, "failed", Some(s.text.as_deref().unwrap_or("missingMacro")));
                return Ok(Then::Over);
            }
            "write" => {
                let text = self.alert_text(run, &need(&s.text)?);
                let wrote: Res<()> = match s.to.as_deref() {
                    Some("clipboard") => self.outside().copy(&text),
                    Some("file") => match &s.file_name {
                        Some(name) => self.outside().append_file(name, &text),
                        None => Err(Fail::other("Object reference not set to an instance of an object.")),
                    },
                    Some("web") => match &s.url {
                        Some(url) => match self.web(run, url, Some(&text), ct) {
                            Ok(None) => {
                                let mut r = lk(run);
                                let key = format!("write{}:{}", r.section.as_deref().unwrap_or(""), r.pc);
                                let note = format!("write step {}: the web address didn't take it; the macro goes on.", r.shown_pc() + 1);
                                note_once(&mut r, &key, &note);
                                Ok(())
                            }
                            Ok(Some(_)) => Ok(()),
                            Err(f) => Err(f),
                        },
                        None => Err(Fail::other("Object reference not set to an instance of an object.")),
                    },
                    _ => Ok(()),
                };
                if let Err(f) = wrote {
                    // A clipboard another app holds, a file open elsewhere: noted, and the macro goes on.
                    let f = outside_halt(f)?;
                    let mut r = lk(run);
                    let key = format!("write{}:{}:{}", r.section.as_deref().unwrap_or(""), r.pc, type_name(&f));
                    let note = format!("write step {} to {}: {}; the macro goes on.", r.shown_pc() + 1, s.to.as_deref().unwrap_or(""), type_name(&f));
                    note_once(&mut r, &key, &note);
                }
            }
            "read" => {
                self.deadline_or(run, s.timeout_ms.unwrap_or(10_000));
                let screen = matches!(s.from.as_deref(), None | Some("screen"));
                let got: Res<Option<String>> = match s.from.as_deref() {
                    Some("clipboard") => self.outside().clipboard(),
                    Some("file") => match &s.file_name {
                        Some(name) => self.outside().read_file(name),
                        None => Err(Fail::other("Object reference not set to an instance of an object.")),
                    },
                    Some("web") => match &s.url {
                        Some(url) => self.web(run, url, None, ct),
                        None => Err(Fail::other("Object reference not set to an instance of an object.")),
                    },
                    _ => self.read_area(turn, s.region.as_ref()),
                };
                let words = match got {
                    Ok(w) => w,
                    Err(f) => {
                        let f = outside_halt(f)?;
                        let mut r = lk(run);
                        let key = format!("read{}:{}:{}", r.section.as_deref().unwrap_or(""), r.pc, type_name(&f));
                        let note = format!("read step {} from {}: {}.", r.shown_pc() + 1, s.from.as_deref().unwrap_or(""), type_name(&f));
                        note_once(&mut r, &key, &note);
                        None
                    }
                };
                if words.is_none() && s.from.as_deref() == Some("file") {
                    let mut r = lk(run);
                    let key = format!("nofile{}:{}", r.section.as_deref().unwrap_or(""), r.pc);
                    let note = format!("read step {}: its file isn't in Documents\\Bloxkeep.", r.shown_pc() + 1);
                    note_once(&mut r, &key, &note);
                }
                let again = if screen {
                    READ_EVERY
                } else if s.from.as_deref() == Some("web") {
                    3000
                } else {
                    1000
                };
                if s.mode.as_deref() == Some("text") {
                    let Some(w) = words.filter(|w| !w.trim().is_empty()) else { return self.not_yet(turn, "timeout", again, ct, false) };
                    let kept = if w.encode_utf16().count() > 200 { utf16_prefix(&w, 200).trim().to_string() } else { w.trim().to_string() };
                    self.set_text(run, &need(&s.name)?, kept);
                } else {
                    // Only the length: what was read may be a name.
                    if let Some(w) = &words
                        && !w.is_empty()
                        && macro_expr::number_in(w).is_none()
                    {
                        let mut r = lk(run);
                        let key = format!("nonumber{}:{}", r.section.as_deref().unwrap_or(""), r.pc);
                        let note = format!(
                            "read step {} from {} read {} characters with no number in them.",
                            r.shown_pc() + 1,
                            s.from.as_deref().unwrap_or("screen"),
                            w.encode_utf16().count()
                        );
                        note_once(&mut r, &key, &note);
                    }
                    let Some(n) = words.as_deref().and_then(macro_expr::number_in) else { return self.not_yet(turn, "timeout", again, ct, false) };
                    self.set_number(run, &need(&s.name)?, n);
                }
            }
            "rejoin" => {
                let account = lk(run).account_id;
                let rejoin = self.rejoin.read().unwrap_or_else(|e| e.into_inner()).clone();
                let refused = rejoin.and_then(|f| f(account));
                if refused.is_some() {
                    let r = lk(run);
                    log::note(&format!("Macro {}: its rejoin step {} was refused.", r.tag(), r.shown_pc() + 1));
                }
                self.finish(run, if refused.is_none() { "done" } else { "failed" }, None);
                return Ok(Then::Over);
            }
            "alert" => {
                let alert = self.alert.read().unwrap_or_else(|e| e.into_inner()).clone();
                if let Some(f) = alert {
                    let text = self.alert_text(run, &need(&s.text)?);
                    let png = if s.picture == Some(true) { self.png(turn)? } else { None };
                    let account = lk(run).account_id;
                    f(account, &m, &text, png);
                }
            }
            "clickColour" => return self.click_colour(turn, s, ct),
            "follow" => return self.follow(turn, s, ct),
            "stop" => {
                self.finish(run, "done", None);
                return Ok(Then::Over);
            }
            _ => {}
        }
        if matches!(s.kind.as_str(), "waitImage" | "waitText" | "waitColour" | "waitStill" | "clickImage") || (s.kind == "read" && matches!(s.from.as_deref(), None | Some("screen"))) {
            seen(&mut lk(run));
        }
        let mut r = lk(run);
        // A wait met: the next wait gets its own timeout, not what is left of this one's.
        r.deadline = None;
        r.pc += 1;
        Ok(Then::Next)
    }

    /// Run a macro steps put in place: the other macro's own steps (not its If stuck part), its settings and areas read
    /// now. One that's gone, runs a macro already running here, or goes too deep becomes a step that fails the run.
    fn expand(&self, steps: &[MacroStep], chain: &[Guid]) -> (Vec<MacroStep>, Vec<i32>) {
        let fail = |why: &str| MacroStep { kind: "fail".into(), text: Some(why.into()), ..Default::default() };
        let mut result = Vec::new();
        let mut origin = Vec::new();
        for (i, s) in steps.iter().enumerate() {
            let inner = if s.kind == "call" { self.inner(s, chain) } else { vec![s.clone()] };
            origin.extend(std::iter::repeat_n(i as i32, inner.len()));
            result.extend(inner);
            if result.len() > MAX_EXPANDED {
                return (vec![fail("missingMacro")], vec![i as i32]);
            }
        }
        (result, origin)
    }

    fn inner(&self, call: &MacroStep, chain: &[Guid]) -> Vec<MacroStep> {
        let fail = || vec![MacroStep { kind: "fail".into(), text: Some("missingMacro".into()), ..Default::default() }];
        let find = self.find_macro.read().unwrap_or_else(|e| e.into_inner()).clone();
        let Some(callee) = find.and_then(|f| f(call.macro_id.unwrap_or(Guid::nil()))) else { return fail() };
        if chain.contains(&callee.id) || chain.len() > macro_rules::MAX_CALL_DEPTH as usize {
            return fail();
        }
        let end = callee.steps.iter().position(|s| s.kind == "recover").unwrap_or(callee.steps.len());
        let id = callee.id;
        let callee = Arc::new(callee);
        let own: Vec<MacroStep> = callee.steps[..end]
            .iter()
            .map(|s| {
                let mut mine = s.clone();
                mine.owner = Some(callee.clone());
                mine
            })
            .collect();
        let mut deeper = chain.to_vec();
        deeper.push(id);
        self.expand(&own, &deeper).0
    }

    /// Ends the turn here; the run is due again at once (after the others).
    fn end_now(&self, run: &RunRef) -> Then {
        lk(run).resume_at = self.desk.now();
        Then::EndTurn
    }

    /// A wait on the picture not met yet: looked at again in a moment (with keys held, without giving the window back),
    /// or, past its time, the macro's onFail.
    fn not_yet(&self, turn: &Turn, reason: &str, again: i32, ct: &Cancel, keep_frame: bool) -> Result<Then, Halt> {
        let run = &turn.run;
        let reason = if self.missed(run) { "stuck" } else { reason };
        let now = self.desk.now();
        let over = {
            let mut r = lk(run);
            if reason == "stuck" || r.deadline.is_some_and(|d| now >= d) {
                r.deadline = None;
                r.last_frame = None;
                r.still_since = None;
                true
            } else {
                false
            }
        };
        if over {
            return self.failed(turn, reason);
        }
        let holding = {
            let mut r = lk(run);
            if !keep_frame {
                r.last_frame = None;
            }
            !r.held.is_empty()
        };
        if holding {
            self.delay(again, ct)?;
            return Ok(Then::Next); // the same step again
        }
        lk(run).resume_at = after_ms(self.desk.now(), again as f64);
        Ok(Then::EndTurn)
    }

    /// A wait timed out, an image wasn't there, or it is stuck: the macro's onFail. In the If stuck part a step that
    /// finds nothing is passed over (it does what it can), unless that part is stuck too.
    fn failed(&self, turn: &Turn, reason: &str) -> Result<Then, Halt> {
        let run = &turn.run;
        let (m, recovering, feature_skips) = {
            let mut r = lk(run);
            let m = r.r#macro.clone();
            // Once per step and reason: a step that skips on fail would write one every pass.
            let kind = kind_at(&r);
            let feature_skips = r.feature().is_some_and(|f| f.on_fail == "skip");
            let key = format!("fail{}:{}:{reason}", r.section.as_deref().unwrap_or(""), r.pc);
            let text = format!(
                "step {} ({kind}{}) failed ({reason}){}{}; on fail: {}, restarts {}.",
                r.shown_pc() + 1,
                r.section.as_ref().map_or(String::new(), |f| format!(", feature {f}")),
                match r.last_score {
                    Some(score) if matches!(kind.as_str(), "clickImage" | "waitImage") => format!(", best picture match {score:.2}"),
                    _ => String::new(),
                },
                if r.recovering { " in its If stuck part" } else { "" },
                if feature_skips { "skip the feature" } else { m.on_fail.as_str() },
                r.restarts
            );
            note_once(&mut r, &key, &text);
            (m, r.recovering, feature_skips)
        };
        if recovering && reason != "stuck" {
            lk(run).pc += 1;
            return Ok(Then::Next);
        }
        // A feature that skips: not this time, back to the main steps (it plays again when it is due).
        if feature_skips && reason != "stuck" {
            turn.let_go();
            self.feature_ended(run, Some(reason));
            return Ok(Then::Next);
        }
        match m.on_fail.as_str() {
            "skip" if reason != "stuck" => {
                lk(run).pc += 1;
                return Ok(Then::Next);
            }
            "skip" | "restart" => return Ok(self.restart(turn)),
            "rejoin" => {
                let account = lk(run).account_id;
                let rejoin = self.rejoin.read().unwrap_or_else(|e| e.into_inner()).clone();
                if rejoin.and_then(|f| f(account)).is_some() {
                    log::note(&format!("Macro {}: its rejoin on fail was refused.", lk(run).tag()));
                }
            }
            "alert" => {
                let alert = self.alert.read().unwrap_or_else(|e| e.into_inner()).clone();
                if let Some(f) = alert {
                    let pc = lk(run).pc;
                    let text = match reason {
                        "stuck" => l::f("Macro {0} stopped: it found nothing for {1} minutes.", &[&m.name, &m.stuck_minutes.unwrap_or(0)]),
                        "notFound" => l::f("Macro {0} stopped: step {1} didn't find its picture.", &[&m.name, &(pc + 1)]),
                        _ => l::f("Macro {0} stopped: step {1} waited too long.", &[&m.name, &(pc + 1)]),
                    };
                    let png = self.png(turn)?;
                    let account = lk(run).account_id;
                    f(account, &m, &text, png);
                }
            }
            _ => {}
        }
        self.finish(run, "failed", Some(reason));
        Ok(Then::Over)
    }

    /// Back to the top, through the If stuck part first when there is one. After MAX_RESTARTS in a row with nothing
    /// seen in between, it gives up.
    fn restart(&self, turn: &Turn) -> Then {
        let run = &turn.run;
        let restarts = {
            let mut r = lk(run);
            r.restarts += 1;
            r.restarts
        };
        if restarts > MAX_RESTARTS {
            self.finish(run, "failed", Some("stuck"));
            return Then::Over;
        }
        turn.let_go();
        let recovering = {
            let mut r = lk(run);
            leave_features(&mut r);
            r.passes.clear();
            r.again = -1;
            r.wait_left = None;
            r.deadline = None;
            r.last_frame = None;
            r.still_since = None;
            r.miss_since = None;
            r.recovering = r.recover_at >= 0;
            r.pc = r.recover_at + 1; // 0 without one
            r.recovering
        };
        self.set(run, "running", if recovering { Some("recovering") } else { None });
        self.end_now(run)
    }

    /// What a step looks for wasn't seen; true when that makes it stuck.
    fn missed(&self, run: &RunRef) -> bool {
        let now = self.desk.now();
        {
            let mut r = lk(run);
            if r.miss_since.is_none() {
                r.miss_since = Some(now);
            }
        }
        self.stuck(run)
    }

    fn stuck(&self, run: &RunRef) -> bool {
        let now = self.desk.now();
        let r = lk(run);
        match (r.r#macro.stuck_minutes, r.miss_since) {
            (Some(minutes), Some(since)) if minutes > 0 => ms_since(now, since) >= minutes as f64 * 60_000.0,
            _ => false,
        }
    }

    /// The window's picture as PNG, for an alert; None when it can't be pictured.
    fn png(&self, turn: &Turn) -> Result<Option<Vec<u8>>, Halt> {
        let Some(picture) = self.picture(turn) else { return Ok(None) };
        Ok(Some(png_of(&picture)?))
    }

    fn find_image(&self, turn: &Turn, image: &str, region: Option<&MacroRect>, threshold: f64) -> Option<(Rect, i32, i32)> {
        let picture = self.picture(turn)?;
        // Cut from a window of another width: scaled to this one's.
        let recorded = lk(&turn.run).r#macro.recorded_size.clone();
        let scale = match recorded {
            Some(size) if size.w > 0 => picture.width as f64 / size.w as f64,
            _ => 1.0,
        };
        let gray = macro_vision::to_gray(&picture, None);
        let result = macro_vision::template(image, scale).and_then(|template| macro_vision::find(&gray, &template, macro_vision::area(region, gray.w, gray.h)));
        lk(&turn.run).last_score = result.map(|r| r.0);
        match result {
            Some((score, found)) if score >= threshold => Some((found, picture.width, picture.height)),
            _ => None,
        }
    }

    /// The text read in the area, or any of the words (a list the player ticks: the whole list, from its setting, keeps
    /// a longer name read from counting as a shorter one). True when found.
    fn find_text(&self, turn: &Turn, text: Option<&str>, words: Option<&[String]>, extra: &crate::core::base::Extra, region: Option<&MacroRect>) -> Result<bool, Halt> {
        let Some(picture) = self.picture(turn) else { return Ok(false) };
        let Some(read) = self.desk.read(&picture, macro_vision::area(region, picture.width, picture.height))? else {
            let mut r = lk(&turn.run);
            let note = format!("step {} looks for text, but Windows can't read text on this PC.", r.shown_pc() + 1);
            note_once(&mut r, "ocr", &note);
            return Ok(false);
        };
        let m = lk(&turn.run).r#macro.clone();
        let all: Vec<String> = macro_values::ref_of(extra, "words")
            .and_then(|name| m.options.iter().find(|o| o.name == name).and_then(|o| o.choices.clone()))
            .unwrap_or_default();
        for lines in &read {
            let found = match words {
                Some(w) => macro_ocr::locate_any(lines, w, &all).is_some(),
                None => macro_ocr::locate(lines, text.ok_or_else(nre)?).is_some(),
            };
            if found {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// A web step: at most one a second for each run.
    fn web(&self, run: &RunRef, url: &str, post: Option<&str>, ct: &Cancel) -> Res<Option<String>> {
        let last = lk(run).last_web_at;
        let wait = ms_since(after_ms(last, 1000.0), self.desk.now());
        if wait > 0.0 {
            self.desk.delay(wait as i32, ct)?;
        }
        lk(run).last_web_at = self.desk.now();
        self.outside().web(url, post, ct)
    }

    /// The words read in the area, its lines joined by spaces; None when Windows can't read text here.
    fn read_area(&self, turn: &Turn, region: Option<&MacroRect>) -> Res<Option<String>> {
        let Some(picture) = self.picture(turn) else { return Ok(None) };
        let read = self.desk.read(&picture, macro_vision::area(region, picture.width, picture.height))?;
        if read.is_none() {
            let mut r = lk(&turn.run);
            let note = format!("step {} reads text, but Windows can't read text on this PC.", r.shown_pc() + 1);
            note_once(&mut r, "ocr", &note);
        }
        let Some(found) = read.and_then(|r| r.into_iter().find(|lines| !lines.is_empty())) else { return Ok(None) };
        Ok(Some(found.iter().map(|l| l.text()).collect::<Vec<_>>().join(" ").trim().to_string()))
    }

    /// A name's number for a formula: a variable the macro sets itself is 0 before it is first set; a text that holds a
    /// number counts as that number.
    fn number_of(&self, run: &RunRef, name: &str) -> Result<f64, Halt> {
        let Some(v) = self.value(run, name) else {
            let m = lk(run).r#macro.clone();
            return if sets_var(&m, Some(name)) { Ok(0.0) } else { Err(Halt::Missing) };
        };
        match &v {
            Json::Number(n) => Ok(n.as_f64().unwrap_or(0.0)),
            Json::Bool(true) => Ok(1.0),
            Json::Bool(false) => Ok(0.0),
            Json::String(s) if macro_expr::number_in(s).is_some() => Ok(macro_expr::number_in(s).unwrap_or(0.0)),
            _ => Err(MacroExprError(l::f("“{0}” isn't a number.", &[&name])).into()),
        }
    }

    fn set_number(&self, run: &RunRef, name: &str, value: f64) {
        let account = lk(run).account_id;
        {
            let mut st = self.st();
            st.vars.entry(account).or_default().insert(name.to_string(), round_to(value, 9));
            if let Some(t) = st.texts.get_mut(&account) {
                t.remove(name);
            }
        }
        self.changed();
    }

    fn set_text(&self, run: &RunRef, name: &str, value: String) {
        let account = lk(run).account_id;
        {
            let mut st = self.st();
            st.texts.entry(account).or_default().insert(name.to_string(), value);
            if let Some(v) = st.vars.get_mut(&account) {
                v.remove(name);
            }
        }
        self.changed();
    }

    /// An if's or loopUntil's question, on the window's picture now or the account's variables.
    fn test(&self, turn: &Turn, t: &MacroTest) -> Result<bool, Halt> {
        let run = &turn.run;
        let yes = match t.kind.as_str() {
            "image" => self.find_image(turn, &need(&t.image)?, t.region.as_ref(), t.threshold.unwrap_or(0.9)).is_some(),
            "text" => self.find_text(turn, t.text.as_deref(), t.words.as_deref(), &t.extra, t.region.as_ref())?,
            "colour" => match self.picture(turn) {
                Some(picture) => macro_vision::colour_at(&picture, need(&t.x)?, need(&t.y)?, &need(&t.colour)?, t.tolerance.unwrap_or(24)),
                None => false,
            },
            "var" => {
                let m = lk(run).r#macro.clone();
                let set_by_macro = sets_var(&m, t.name.as_deref());
                macro_values::var_test(t, &mut |name: &str| self.value(run, name), set_by_macro)?
            }
            _ => false,
        };
        // A question isn't an expectation: a "no" isn't counted towards stuck, a "seen" is progress.
        if yes && t.kind != "var" {
            seen(&mut lk(run));
        }
        Ok(if t.kind == "var" { yes } else { yes != (t.not == Some(true)) })
    }

    /// A value a run reads by name: the account's variable first, then the macro's option ("o1"); an area by "@a1".
    pub fn value(&self, run: &RunRef, name: &str) -> Option<Json> {
        let (account, owner) = {
            let r = lk(run);
            (r.account_id, r.owner.clone().unwrap_or_else(|| r.r#macro.clone()))
        };
        let st = self.st();
        if let Some(text) = st.texts.get(&account).and_then(|t| t.get(name)) {
            return Some(Json::String(text.clone()));
        }
        value_in(&owner, st.vars.get(&account), name)
    }

    /// An alert's text with "{name}" put in: a variable or a setting (a counter's number, a choice's word).
    pub fn alert_text(&self, run: &RunRef, text: &str) -> String {
        static NAME_IN_BRACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{([A-Za-z_][A-Za-z0-9_]{0,31})\}").expect("alert pattern"));
        NAME_IN_BRACES
            .replace_all(text, |c: &regex::Captures| match self.value(run, &c[1]) {
                Some(Json::Number(n)) => format_0_hash(n.as_f64().unwrap_or(0.0)),
                Some(Json::String(s)) => s,
                _ => c[0].to_string(),
            })
            .into_owned()
    }

    /// A real-time step keeps the window in front as long as it runs: the others wait meanwhile.
    fn others_wait(&self, run: &RunRef) {
        let others: Vec<RunRef> = self
            .st()
            .runs
            .iter()
            .filter(|r| {
                !Arc::ptr_eq(r, run) && {
                    let g = lk(r);
                    !g.finished() && g.status != "paused"
                }
            })
            .cloned()
            .collect();
        for other in &others {
            self.set(other, "waiting", Some("busy"));
        }
        self.changed();
    }

    /// clickColour: a patch of the colour anywhere in the area is clicked in its middle; with repeatUntilGoneMs, again
    /// each time one shows up, until none has shown for that long (buttons popping up at random places). With
    /// eachAfterMs, every patch is clicked once, when it has shown that long (rings to hit as they come).
    fn click_colour(&self, turn: &Turn, s: &MacroStep, ct: &Cancel) -> Result<Then, Halt> {
        if let Some(after) = s.each_after_ms {
            return self.click_each(turn, s, after, ct);
        }
        let run = &turn.run;
        let colour = need(&s.colour)?;
        self.deadline_or(run, s.timeout_ms.unwrap_or(10_000));
        let every = (frame(&turn.window) * 2).max(50);
        let repeat = s.repeat_until_gone_ms.is_some_and(|r| r > 0);
        let mut clicked = false;
        let mut last_seen = self.desk.now();
        let gave_up_at = self.give_up_at(s);
        if repeat {
            self.others_wait(run);
        }
        loop {
            if ct.is_cancelled() {
                return Err(Halt::Cancelled);
            }
            if let Some(stop) = self.interrupted(turn) {
                return Ok(stop);
            }
            if self.desk.now() >= gave_up_at {
                lk(run).deadline = None;
                return self.failed(turn, "timeout");
            }
            let mut hit: Option<Rect> = None;
            let (mut w, mut h) = (0, 0);
            if let Some(look) = self.look(turn, s.region.as_ref()) {
                (w, h) = (look.width, look.height);
                let area = look.area;
                if let Some(first) = macro_follow::blobs(&Pixels::of(&look.picture, Some(look.part)), &colour, s.tolerance.unwrap_or(24), s.min_size.unwrap_or(4))?.first() {
                    hit = Some(Rect { x: first.x + area.x, y: first.y + area.y, ..*first });
                }
            }
            if let Some(b) = hit {
                let Some(input) = self.front(turn, ct)? else { return Ok(Then::EndTurn) };
                let client = input.client();
                let centre = Point {
                    x: client.x + ((b.x as f64 + b.width as f64 / 2.0) * client.width as f64 / w as f64) as i32,
                    y: client.y + ((b.y as f64 + b.height as f64 / 2.0) * client.height as f64 / h as f64) as i32,
                };
                self.click(&*input, turn, centre, s.button.as_deref().unwrap_or("left"), false, ct, 0)?;
                clicked = true;
                last_seen = self.desk.now();
                lk(run).deadline = Some(after_ms(self.desk.now(), s.timeout_ms.unwrap_or(10_000) as f64));
                if !repeat {
                    break;
                }
            } else if clicked && ms_since(self.desk.now(), last_seen) >= s.repeat_until_gone_ms.unwrap_or(0) as f64 {
                break; // it stopped coming back
            } else if !clicked {
                if !repeat {
                    return self.not_yet(turn, "notFound", LOOK_EVERY, ct, false);
                }
                let missed = self.missed(run);
                let past = lk(run).deadline.is_some_and(|d| self.desk.now() >= d);
                if missed || past {
                    lk(run).deadline = None;
                    let reason = if self.stuck(run) { "stuck" } else { "notFound" };
                    return self.failed(turn, reason);
                }
            }
            self.delay(every, ct)?;
        }
        lk(run).deadline = None;
        let mut r = lk(run);
        seen(&mut r);
        r.pc += 1;
        Ok(Then::Next)
    }

    /// What a real-time step looks at: once the window is in front, only its area, copied quickly (as follow does: a
    /// whole PrintWindow every other frame is far slower); before that, or when the copy fails, the whole picture.
    fn look(&self, turn: &Turn, region: Option<&MacroRect>) -> Option<Look> {
        if let Some(input) = turn.input() {
            let client = input.client();
            let area = macro_vision::area(region, client.width, client.height);
            if let Some(shot) = self.desk.area(&turn.window, &*input, area) {
                let part = Rect::new(0, 0, shot.width, shot.height);
                return Some(Look { picture: Arc::new(shot), part, area, width: client.width, height: client.height });
            }
        }
        let picture = self.picture(turn)?;
        let area = macro_vision::area(region, picture.width, picture.height);
        Some(Look { part: area, area, width: picture.width, height: picture.height, picture })
    }

    /// When a real-time step that clicks as things come gives up: REAL_TIME_CAP after it began, or its timeout when
    /// that is longer.
    fn give_up_at(&self, s: &MacroStep) -> Stamp {
        after_ms(self.desk.now(), (s.timeout_ms.unwrap_or(10_000) as f64).max(ms(REAL_TIME_CAP)))
    }

    /// clickColour with eachAfterMs: the patches are followed from picture to picture (one counts as the same while its
    /// middle stays within its size); each is clicked once, when it has shown that long, and forgotten when gone. Ends
    /// when nothing new has come for repeatUntilGoneMs after a click, or at the timeout with none clicked.
    fn click_each(&self, turn: &Turn, s: &MacroStep, after: i32, ct: &Cancel) -> Result<Then, Halt> {
        let run = &turn.run;
        let colour = need(&s.colour)?;
        self.deadline_or(run, s.timeout_ms.unwrap_or(10_000));
        let every = (frame(&turn.window) * 2).max(30);
        let mut spots: Vec<Spot> = Vec::new();
        let mut clicked = false;
        let mut last_new = self.desk.now();
        let gave_up_at = self.give_up_at(s);
        self.others_wait(run);
        loop {
            if ct.is_cancelled() {
                return Err(Halt::Cancelled);
            }
            if let Some(stop) = self.interrupted(turn) {
                return Ok(stop);
            }
            if self.desk.now() >= gave_up_at {
                lk(run).deadline = None;
                return self.failed(turn, "timeout");
            }
            if let Some(look) = self.look(turn, s.region.as_ref()) {
                let (w, h) = (look.width, look.height);
                let area = look.area;
                let mut now_spots: Vec<Spot> = Vec::new();
                let mut taken: HashSet<usize> = HashSet::new();
                for b in macro_follow::blobs(&Pixels::of(&look.picture, Some(look.part)), &colour, s.tolerance.unwrap_or(24), s.min_size.unwrap_or(4))? {
                    let centre = ((area.x as f32 + b.x as f32 + b.width as f32 / 2.0), (area.y as f32 + b.y as f32 + b.height as f32 / 2.0));
                    let size = (b.width.max(b.height) as f32).max(8.0);
                    let same = spots.iter().enumerate().position(|(k, p)| {
                        !taken.contains(&k) && (p.centre.0 - centre.0).abs() <= size.max(p.size) && (p.centre.1 - centre.1).abs() <= size.max(p.size)
                    });
                    match same {
                        None => {
                            last_new = self.desk.now();
                            now_spots.push(Spot { centre, size, since: self.desk.now(), clicked: false });
                        }
                        Some(k) => {
                            taken.insert(k);
                            now_spots.push(Spot { centre, size, ..spots[k] });
                        }
                    }
                }
                spots = now_spots; // the ones gone are forgotten
                for k in 0..spots.len() {
                    if spots[k].clicked || ms_since(self.desk.now(), spots[k].since) < after as f64 {
                        continue;
                    }
                    let Some(input) = self.front(turn, ct)? else { return Ok(Then::EndTurn) };
                    let client = input.client();
                    let at = Point {
                        x: client.x + (spots[k].centre.0 as f64 * client.width as f64 / w as f64) as i32,
                        y: client.y + (spots[k].centre.1 as f64 * client.height as f64 / h as f64) as i32,
                    };
                    self.click(&*input, turn, at, s.button.as_deref().unwrap_or("left"), false, ct, 0)?;
                    spots[k].clicked = true;
                    clicked = true;
                    last_new = self.desk.now();
                    lk(run).deadline = Some(after_ms(self.desk.now(), s.timeout_ms.unwrap_or(10_000) as f64));
                }
            }
            if clicked && ms_since(self.desk.now(), last_new) >= s.repeat_until_gone_ms.unwrap_or(1000) as f64 && spots.iter().all(|p| p.clicked) {
                break; // none has come for a while
            }
            if !clicked {
                let missed = self.missed(run);
                let past = lk(run).deadline.is_some_and(|d| self.desk.now() >= d);
                if missed || past {
                    lk(run).deadline = None;
                    let reason = if self.stuck(run) { "stuck" } else { "notFound" };
                    return self.failed(turn, reason);
                }
            }
            self.delay(every, ct)?;
        }
        lk(run).deadline = None;
        let mut r = lk(run);
        seen(&mut r);
        r.pc += 1;
        Ok(Then::Next)
    }

    /// follow: keeps a marker on a moving target in a bar (a reel, a charge), looking at the area many times a second
    /// with the window in front the whole time. On/off holds the button while the marker is behind the target; smooth
    /// presses in short pulses sized by distance and speed. Ends when the target has been gone for untilGoneMs, or at
    /// its timeout (then the macro's onFail). Whatever it holds is let go at the end.
    fn follow(&self, turn: &Turn, s: &MacroStep, ct: &Cancel) -> Result<Then, Halt> {
        let run = &turn.run;
        let Some(input) = self.front(turn, ct)? else { return Ok(Then::EndTurn) };
        let marker = macro_follow::marker(s.marker.as_ref(), &|name: &str| self.value(run, name))?;
        self.others_wait(run);
        let button = s.button.clone().unwrap_or_else(|| "left".into());
        let key: Option<&'static MacroKey> = if button == "key" { Some(key_of(s.key.as_deref())?) } else { None };
        if key.is_none() {
            // The press goes where the pointer is: the middle of the bar's area, inside the game.
            let client = input.client();
            let area = macro_vision::area(s.region.as_ref(), client.width, client.height);
            let at = Point { x: client.x + area.x + area.width / 2, y: client.y + area.y + area.height / 2 };
            if !input.clear(at) {
                return Err(Halt::Covered);
            }
            input.move_to(at);
        }
        let mut held = false;
        let ended = self.follow_loop(turn, s, ct, &*input, &marker, key, &button, &mut held);
        if held {
            press(&*input, key, &button, false);
        }
        match ended? {
            Some(then) => Ok(then),
            None => {
                let mut r = lk(run);
                seen(&mut r);
                r.pc += 1;
                Ok(Then::Next)
            }
        }
    }

    /// The follow step's frames; None when the target went (the step is done), else how the turn goes on. `held` says
    /// whether the press is down, for the caller to let go of whatever happens.
    #[allow(clippy::too_many_arguments)]
    fn follow_loop(
        &self,
        turn: &Turn,
        s: &MacroStep,
        ct: &Cancel,
        input: &dyn IMacroTurn,
        marker: &macro_follow::MacroMarker,
        key: Option<&MacroKey>,
        button: &str,
        held: &mut bool,
    ) -> Result<Option<Then>, Halt> {
        let target_colour = need(&s.target)?;
        let frame_ms = 4.max(1000 / s.fps.unwrap_or(60).clamp(15, 240));
        let deadline = after_ms(self.desk.now(), s.timeout_ms.unwrap_or(10_000) as f64);
        let mut gone_since: Option<Stamp> = None;
        let mut seen_once = false;
        let mut pd = macro_follow::Pd::new(s.kp.unwrap_or(1.0), s.kd.unwrap_or(0.4), 1.0);
        let mut predictor = macro_follow::Predictor::new();
        loop {
            if ct.is_cancelled() {
                return Err(Halt::Cancelled);
            }
            let mut pulsed = 0;
            // The person, the window leaving the front, anti-AFK's turn: the step starts over next turn. Its longest time: over.
            if let Some(stop) = self.interrupted(turn) {
                return Ok(Some(stop));
            }
            let client = input.client();
            let area = macro_vision::area(s.region.as_ref(), client.width, client.height);
            let shot = self.desk.area(&turn.window, input, area);
            let sight = match &shot {
                None => macro_follow::FollowSight::default(),
                Some(p) => macro_follow::look(&Pixels::of(p, None), &target_colour, marker, s.tolerance.unwrap_or(24))?,
            };
            match sight.target {
                None => {
                    let now = self.desk.now();
                    let since = *gone_since.get_or_insert(now);
                    if *held {
                        press(input, key, button, false);
                        *held = false;
                    }
                    // Gone after it was there: the minigame is over. Never there: still waiting for it, up to the timeout.
                    if seen_once && ms_since(now, since) >= s.until_gone_ms.unwrap_or(1000) as f64 {
                        return Ok(None);
                    }
                }
                Some(target) => {
                    seen_once = true;
                    gone_since = None;
                    if let Some(at) = sight.marker {
                        let ahead = if s.predict == Some(true) { predictor.ahead(at) } else { at };
                        if s.mode.as_deref() == Some("smooth") {
                            if pd.width() != area.width as f64 {
                                pd = macro_follow::Pd::new(s.kp.unwrap_or(1.0), s.kd.unwrap_or(0.4), area.width as f64);
                            }
                            let press_ms = pd.press_ms(target, ahead, frame_ms as f64);
                            if press_ms >= 1.0 {
                                press(input, key, button, true);
                                *held = true;
                                self.delay(press_ms.round_ties_even() as i32, ct)?;
                                press(input, key, button, false);
                                *held = false;
                                pulsed = press_ms.round_ties_even() as i32;
                            }
                        } else if let Some(down) = macro_follow::on_off(ahead, target, macro_follow::DEAD_ZONE)
                            && down != *held
                        {
                            press(input, key, button, down);
                            *held = down;
                        }
                    } else if *held {
                        press(input, key, button, false); // the marker isn't seen: nothing to steer by
                        *held = false;
                    }
                }
            }
            if self.desk.now() >= deadline {
                if *held {
                    press(input, key, button, false);
                }
                *held = false;
                return self.failed(turn, if seen_once { "timeout" } else { "notFound" }).map(Some);
            }
            self.delay(1.max(frame_ms - pulsed), ct)?;
        }
    }

    fn offset(&self, p: Point, px: i32) -> Point {
        if px <= 0 {
            return p;
        }
        let mut random = lk(&self.random);
        let x = p.x + random.next(-px, px + 1);
        let y = p.y + random.next(-px, px + 1);
        Point { x, y }
    }

    /// A wait inside a step that keeps the window (a long press), in WAIT_SLICE pieces each asking whether the turn may
    /// go on; Cancelled when it may not (the step plays again next turn). A wait of one piece is a plain delay.
    fn wait_in_front(&self, turn: &Turn, ms: i32, ct: &Cancel) -> Result<(), Halt> {
        let until = after_ms(self.desk.now(), ms as f64);
        loop {
            let left = ms_since(until, self.desk.now()).ceil();
            if left <= 0.0 {
                return Ok(());
            }
            self.delay((WAIT_SLICE as f64).min(left) as i32, ct)?;
            if ms_since(until, self.desk.now()) > 0.0 && self.interrupted(turn).is_some() {
                return Err(Halt::Cancelled);
            }
        }
    }

    /// `hold_ms`: held this long (a charge); 0: a plain click.
    #[allow(clippy::too_many_arguments)]
    fn click(&self, input: &dyn IMacroTurn, turn: &Turn, at: Point, button: &str, twice: bool, ct: &Cancel, hold_ms: i32) -> Result<(), Halt> {
        let (jitter_px, jitter_ms) = {
            let r = lk(&turn.run);
            (r.r#macro.jitter_px, r.r#macro.jitter_ms)
        };
        let frame = frame(&turn.window);
        let at = self.offset(at, jitter_px);
        if !input.clear(at) {
            return Err(Halt::Covered);
        }
        input.move_to(at);
        self.delay(frame, ct)?; // the game sees the pointer arrive before the press
        for i in 0..(if twice { 2 } else { 1 }) {
            if i > 0 {
                self.delay(60.max(frame), ct)?;
            }
            input.button(button, false);
            let waited = self.wait_in_front(turn, hold_for(self.jitter(if hold_ms > 0 { hold_ms } else { 60 }, jitter_ms), turn.window.fps), ct);
            input.button(button, true);
            waited?;
        }
        self.delay(frame, ct)
    }

    /// The camera turned by a relative mouse move spread over `ms`, about one piece a frame, with the right button held
    /// (Roblox's camera drag) unless the game locks the pointer itself.
    #[allow(clippy::too_many_arguments)]
    fn camera(&self, input: &dyn IMacroTurn, turn: &Turn, dx: i32, dy: i32, ms: i32, drag: bool, ct: &Cancel) -> Result<(), Halt> {
        let frame = frame(&turn.window);
        let centre = macro_geometry::client_point(input.client(), 0.5, 0.5);
        if !input.clear(centre) {
            return Err(Halt::Covered);
        }
        input.move_to(centre);
        self.delay(frame, ct)?;
        if drag {
            input.button("right", false);
            held_change(&mut lk(&turn.run).held, "right", false);
            self.delay(frame.max(30), ct)?;
        }
        let pieces = 1.max(ms / frame.max(8));
        let mut moved = Ok(());
        for (x, y) in macro_geometry::pieces(dx, dy, pieces) {
            input.move_by(x, y);
            moved = self.delay(1.max(ms / pieces), ct);
            if moved.is_err() {
                break;
            }
        }
        if drag {
            input.button("right", true);
            held_change(&mut lk(&turn.run).held, "right", true);
        }
        moved?;
        self.delay(frame, ct)
    }

    /// Chat: "/" opens it, the text, Enter sends it. Letters and signs as keys (Shift where needed); any other character
    /// (Vietnamese letters) typed as itself.
    fn type_text(&self, input: &dyn IMacroTurn, turn: &Turn, text: &str, ct: &Cancel) -> Result<(), Halt> {
        let hold = hold_for(30, turn.window.fps);
        let frame = frame(&turn.window);
        let tap = |key: &MacroKey, shift: bool| -> Result<(), Halt> {
            let shift_key = key_of(Some("Shift"))?;
            if shift {
                input.key(shift_key, false);
            }
            input.key(key, false);
            let waited = self.delay(hold.min(40), ct);
            input.key(key, true);
            if shift {
                input.key(shift_key, true);
            }
            waited?;
            self.delay(15.max(frame / 2), ct)
        };
        tap(key_of(Some("Slash"))?, false)?;
        self.delay(150.max(frame * 3), ct)?; // the chat box opening
        for (i, c) in text.chars().enumerate() {
            // A long text takes seconds: the person, the window leaving the front or anti-AFK's turn stop it between
            // letters (the step types it again next turn) rather than typing on into whatever is in front.
            if i > 0 && i % 8 == 0 && self.interrupted(turn).is_some() {
                return Err(Halt::Cancelled);
            }
            match macro_keys::for_char(c) {
                Some((k, shift)) => tap(k, shift)?,
                None => {
                    input.char(c, false); // down and up together, per UTF-16 half
                    self.delay(15.max(frame / 2), ct)?;
                }
            }
        }
        self.delay(60.max(frame * 2), ct)?;
        tap(key_of(Some("Enter"))?, false)
    }
}

/// Another macro going on (or waiting) on the account; None when none.
fn busy_with_in(runs: &[RunRef], account: Guid, macro_id: Guid) -> Option<Arc<Macro>> {
    runs.iter().find_map(|r| {
        let g = lk(r);
        (!g.finished() && g.account_id == account && g.r#macro.id != macro_id).then(|| g.r#macro.clone())
    })
}

/// The "End repeat"s of the main steps' repeats inside no other block, before the If stuck part.
pub fn safe_ends(steps: &[MacroStep]) -> HashSet<i32> {
    let mut ends = HashSet::new();
    let mut depth = 0;
    for (i, s) in steps.iter().enumerate() {
        if s.kind == "recover" {
            break;
        }
        match s.kind.as_str() {
            "if" | "loop" | "loopUntil" => depth += 1,
            "endIf" => depth -= 1,
            "endLoop" => {
                // Inside no other block: nothing else was open.
                if depth == 1 {
                    ends.insert(i as i32);
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    ends
}

/// error.log: a note written once per run for a condition that would repeat every look.
fn note_once(run: &mut MacroRun, key: &str, text: &str) {
    if run.noted.insert(key.to_string()) {
        log::note(&format!("Macro {}: {text}", run.tag()));
    }
}

fn kind_at(run: &MacroRun) -> String {
    if run.pc >= 0 && (run.pc as usize) < run.steps().len() { run.steps()[run.pc as usize].kind.clone() } else { "end".into() }
}

/// Adds (down) or takes away (up) a held key; true when that changed the list (HashSet.Add / Remove).
fn held_change(held: &mut Vec<String>, key: &str, up: bool) -> bool {
    match (up, held.iter().position(|k| k == key)) {
        (false, None) => {
            held.push(key.to_string());
            true
        }
        (true, Some(at)) => {
            held.remove(at);
            true
        }
        _ => false,
    }
}

/// The follow step's press: its key, or its mouse button.
fn press(input: &dyn IMacroTurn, key: Option<&MacroKey>, button: &str, down: bool) {
    match key {
        Some(k) => input.key(k, !down),
        None => input.button(button, !down),
    }
}

/// Its window was replaced since its last turn (a rejoin, a crash: another process): a new server is a new start, so it
/// plays from the top, holding nothing (what it held went with the old window, let go as that turn ended).
fn start_over(run: &mut MacroRun) {
    leave_features(run);
    run.held.clear();
    run.held_as.clear();
    run.pc = 0;
    run.passes.clear();
    run.again = -1;
    run.wait_left = None;
    run.deadline = None;
    run.last_frame = None;
    run.still_since = None;
    run.miss_since = None;
    run.recovering = false;
    run.restarts = 0;
}

/// Back to the main steps, with no feature due (a restart, a new window).
fn leave_features(run: &mut MacroRun) {
    run.section = None;
    run.due.clear();
    run.resumed = false;
    run.main_passes = None;
}

fn start_feature(run: &mut MacroRun, id: &str) {
    run.section = Some(id.to_string());
    run.pc = 0;
    run.passes.clear();
    run.again = -1;
    clear_wait(run);
}

fn clear_wait(run: &mut MacroRun) {
    run.wait_left = None;
    run.deadline = None;
    run.last_frame = None;
    run.still_since = None;
}

/// What its steps look for was seen: not stuck.
fn seen(run: &mut MacroRun) {
    run.miss_since = None;
    if !run.recovering {
        run.restarts = 0;
    }
}

/// A feature's switch is on.
pub fn switch_on(m: &Macro, f: &MacroFeature) -> bool {
    m.options.iter().find(|o| o.name == f.option).is_some_and(|o| matches!(macro_options::value_of(o), Json::Number(n) if n.as_f64().unwrap_or(0.0) != 0.0))
}

fn sets_var(m: &Macro, name: Option<&str>) -> bool {
    macro_rules::all_steps(m).any(|s| matches!(s.kind.as_str(), "setVar" | "addVar" | "read") && s.name.as_deref() == name)
}

/// A value by name in a macro: an area by "@a1", a variable, then an option.
pub fn value_in(m: &Macro, vars: Option<&HashMap<String, f64>>, name: &str) -> Option<Json> {
    if let Some(area) = name.strip_prefix('@') {
        // The page's shape (camelCase).
        return m.areas.iter().find(|a| a.name == area).map(|found| serde_json::json!({ "x": found.rect.x, "y": found.rect.y, "w": found.rect.w, "h": found.rect.h }));
    }
    if let Some(v) = vars.and_then(|v| v.get(name)) {
        return Some(Json::from(*v));
    }
    m.options.iter().find(|o| o.name == name).map(macro_options::value_of)
}

/// A number as .NET's "0.##" writes it (invariant culture).
fn format_0_hash(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0; // away from zero on a tie, as .NET's custom formats
    let text = format!("{r:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    text.to_string()
}

fn utf16_prefix(text: &str, max: usize) -> String {
    let mut used = 0;
    let mut out = String::new();
    for c in text.chars() {
        used += c.len_utf16();
        if used > max {
            break;
        }
        out.push(c);
    }
    out
}

pub fn compare(a: f64, op: &str, b: f64) -> bool {
    match op {
        "<" => a < b,
        "<=" => a <= b,
        "=" => (a - b).abs() < 1e-9,
        ">=" => a >= b,
        ">" => a > b,
        "!=" => (a - b).abs() >= 1e-9,
        _ => false,
    }
}

/// A frame of the window's frame rate limit (60 fps when not known).
fn frame(w: &MacroWindow) -> i32 {
    if w.fps > 0 { (1000.0 / w.fps as f64).ceil() as i32 } else { 17 }
}

fn ms_of(d: Duration) -> f64 {
    ms(d)
}

/// How long a key or button is held: at least two frames (a game reads its input once a frame, and a press shorter
/// than one can fall between two) and never under 60 ms.
pub fn hold_for(ms: i32, fps: i32) -> i32 {
    ms.max(60.max(if fps > 0 { (2000.0 / fps as f64).ceil() as i32 } else { 0 }))
}

/// A real-time step's look: `part` of `picture` holds the step's `area` of a window `width`×`height` (the client's
/// pixels, or the whole picture's).
struct Look {
    picture: Arc<Picture>,
    part: Rect,
    area: Rect,
    width: i32,
    height: i32,
}

/// A patch being watched: where its middle is (picture pixels), since when, and whether it was clicked.
#[derive(Clone, Copy, Debug)]
struct Spot {
    centre: (f32, f32),
    size: f32,
    since: Stamp,
    clicked: bool,
}

/// Roughly how long one round over `windows` windows takes for this macro: bringing each forward, and its steps up to
/// its first long wait.
pub fn sweep_seconds(m: &Macro, windows: i32, fps: i32) -> f64 {
    let mut turn = 0.5; // forward and back
    let frame = if fps > 0 { (1000.0 / fps as f64).ceil() as i32 } else { 17 };
    for s in &m.steps {
        let ms = match s.kind.as_str() {
            "key" => (hold_for(s.ms.unwrap_or(60), fps) + frame) as f64,
            "down" | "up" | "move" | "setVar" | "addVar" => frame as f64,
            "read" => 300.0,
            "click" => (frame * 2 + hold_for(60, fps) * if s.double == Some(true) { 2 } else { 1 }) as f64,
            "clickImage" => (300 + frame * 2 + hold_for(60, fps)) as f64,
            "camera" => (s.ms.unwrap_or(200) + frame * 3) as f64,
            "scroll" => (s.notches.unwrap_or(0).abs() * 40.max(frame) + frame) as f64,
            "type" => (300 + s.text.as_ref().map_or(0, |t| t.encode_utf16().count() as i32) * 60) as f64,
            "wait" => s.ms.unwrap_or(0) as f64,
            _ => 0.0,
        };
        if matches!(s.kind.as_str(), "waitImage" | "waitText" | "waitColour" | "waitStill") || (s.kind == "wait" && ms > ms_of(LONG_WAIT)) {
            break;
        }
        turn += ms / 1000.0;
        if turn > TURN_SHARE.as_secs_f64() {
            turn = TURN_SHARE.as_secs_f64();
            break;
        }
    }
    round_to(windows as f64 * turn, 1)
}

/// Where on the screen a place in a window is, and how SendInput names it.
pub mod macro_geometry {
    use crate::core::win_kit::{Point, Rect};

    /// A place (0..1 of the client area) in screen pixels, kept inside the client.
    pub fn client_point(client: Rect, x: f64, y: f64) -> Point {
        Point {
            x: client.x + ((x * client.width as f64).round_ties_even() as i32).clamp(0, 0.max(client.width - 1)),
            y: client.y + ((y * client.height as f64).round_ties_even() as i32).clamp(0, 0.max(client.height - 1)),
        }
    }

    /// A screen pixel as SendInput's absolute coordinates over the whole desktop (MOUSEEVENTF_VIRTUALDESK): 0..65535 from
    /// the left of the leftmost screen to the right of the rightmost, which may start left of or above the main screen.
    pub fn absolute(screen: Point, desktop: Rect) -> (i32, i32) {
        (
            ((screen.x - desktop.x) as f64 * 65535.0 / 1.max(desktop.width - 1) as f64).round_ties_even() as i32,
            ((screen.y - desktop.y) as f64 * 65535.0 / 1.max(desktop.height - 1) as f64).round_ties_even() as i32,
        )
    }

    /// A move split into `count` pieces that add up to it exactly.
    pub fn pieces(dx: i32, dy: i32, count: i32) -> Vec<(i32, i32)> {
        let count = count.max(1);
        let (mut done_x, mut done_y) = (0, 0);
        let mut out = Vec::new();
        for i in 1..=count {
            let x = (dx as f64 * i as f64 / count as f64).round_ties_even() as i32;
            let y = (dy as f64 * i as f64 / count as f64).round_ties_even() as i32;
            if x - done_x != 0 || y - done_y != 0 {
                out.push((x - done_x, y - done_y));
            }
            (done_x, done_y) = (x, y);
        }
        out
    }
}
