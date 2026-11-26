// AppController.Memory.cs: the PC's memory watched: Roblox windows' RAM priority, resting windows' RAM emptied, and the
// RAM limit per window.
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    future::Future,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicU32, Ordering},
        mpsc,
    },
    time::Duration,
};

use futures::channel::oneshot;

use super::App;
use super::controller::{COMMIT_MARGIN_MB, DEFAULT_CLIENT_COMMIT_MB, MB, MEMORY_WARNING_INTERVAL, MEMORY_WARNING_LOAD};
use super::presets::secs;
use crate::core::{
    base::{Guid, Stamp},
    cpu_cap, l, log,
    models::stamp_min,
    roblox_log_watcher::ClientState,
    roblox_processes::{self, PlayerProcess},
    rt,
    system_memory::MemoryStatus,
};

pub const EMPTY_RAM_RESTED: Duration = Duration::from_secs(60);

/// Auto clean above a RAM level runs at most this often: a PC that stays above it after a clean isn't cleaned over and
/// over.
pub const AUTO_CLEAN_GAP: Duration = Duration::from_secs(10 * 60);
/// An auto clean above the level that freed less says nothing (only the log has it). A timed one never says anything.
const AUTO_CLEAN_SAID_MB: u64 = 200;

/// A window moving to a server (a teleport, a join) loads the new place and needs its whole RAM back for a while: 4
/// minimized windows held at 512 MB froze 17–27 s in a teleport and all 4 dropped with "Disconnected from game,
/// please reconnect" (a real log; back at 1.4–1.5 GB each after the rejoin). From the teleport's start until
/// `SETTLE_AFTER_JOIN` after the new server is reached its limit is `MOVING_LIMIT_MB` (or the chosen one, if higher) and
/// it isn't emptied; a join that hangs longer than `SETTLE_LONGEST` is left to the chosen limit again.
/// Its own time, not the 30 s a window is given before it is minimised (`presets::ON_JOIN_LOADING`): tied to that, a
/// window's memory came down only in the second it was minimised, as if the limit needed that. 12 s: a game's own
/// scripts began 12 s after the join line in a real log, the place loaded by then.
pub const SETTLE_AFTER_JOIN: Duration = Duration::from_secs(12);
pub const SETTLE_LONGEST: Duration = Duration::from_secs(5 * 60);
/// A moving window's limit: room for the new place, but not all the RAM it likes. Above the 1.4–1.5 GB those 4 windows
/// held after joining, the most measured there.
pub const MOVING_LIMIT_MB: i64 = 2048;

/// Whether a window is moving to a server, or reached one under `SETTLE_AFTER_JOIN` ago (see that).
pub fn settling(state: Option<&ClientState>, now: Stamp) -> bool {
    Game::of(state).settling(now)
}

/// A window's game as its Roblox log has it (roblox_log_watcher's `ClientState`), as far as its memory cares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Game {
    /// Its log says nothing yet (still opening, or not followed).
    Unknown,
    Joining { at: Stamp },
    InGame { at: Stamp, teleport: Option<Stamp> },
    Left,
    Disconnected,
    SignedOut,
}

impl Game {
    pub fn of(state: Option<&ClientState>) -> Game {
        let Some(s) = state else { return Game::Unknown };
        match s.status.as_str() {
            "joining" => Game::Joining { at: s.at },
            "ingame" => Game::InGame { at: s.at, teleport: s.teleport_started },
            "disconnected" => Game::Disconnected,
            "signedout" => Game::SignedOut,
            _ => Game::Left,
        }
    }

    /// Moving to a server, or reached one under `SETTLE_AFTER_JOIN` ago.
    pub fn settling(self, now: Stamp) -> bool {
        let recent = |at: Stamp, within: Duration| now.secs_since(at) < secs(within);
        match self {
            Game::Joining { at } => recent(at, SETTLE_LONGEST),
            Game::InGame { at, teleport } => recent(at, SETTLE_AFTER_JOIN) || teleport.is_some_and(|t| recent(t, SETTLE_LONGEST)),
            _ => false,
        }
    }
}

/// Settings › RAM limit as one window's memory reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RamSetting {
    Off,
    Auto,
    Mb(i64),
}

impl RamSetting {
    pub fn of(ram_limit_mb: i64) -> RamSetting {
        match ram_limit_mb {
            0 => RamSetting::Off,
            mb if mb == RAM_LIMIT_AUTO as i64 => RamSetting::Auto,
            mb => RamSetting::Mb(mb),
        }
    }

    /// The MB chosen; nothing for Off and Auto (a moving window's limit takes the highest of it and the others).
    fn chosen_mb(self) -> i64 {
        if let RamSetting::Mb(mb) = self { mb } else { 0 }
    }
}

/// What Bloxkeep asked of one window's memory and what Windows said, kept per process and dropped when it ends.
#[derive(Clone, Debug, Default)]
pub struct RamMemory {
    /// An ask still on its way through the RAM turn: `Some(None)` a lift, `Some(Some(mb))` a limit.
    pub asked: Option<Option<i64>>,
    /// The limit Windows last said yes to (None: none). `Some(-1)`: one found set without an ask of this run (a Bloxkeep
    /// that closed before lifting it, or a lift Windows refused before a lock).
    pub held: Option<i64>,
    /// Windows said no to the last ask; it is asked again.
    pub refused: bool,
    pub auto: Option<AutoRam>,
    /// Save RAM: since when it has rested, and whether it was emptied this rest.
    pub resting_since: Option<Stamp>,
    pub emptied: bool,
}

impl RamMemory {
    /// The limit it has once the asks on their way are answered yes.
    pub fn limit(&self) -> Option<i64> {
        self.asked.unwrap_or(self.held)
    }
}

/// One window as the RAM limit and Save RAM see it, read once per tick.
#[derive(Clone, Debug)]
pub struct RamFacts {
    pub pid: i32,
    pub now: Stamp,
    pub setting: RamSetting,
    /// "Limit open windows too" (`ram_limit_open`).
    pub open_too: bool,
    /// Save RAM (`trim_background`).
    pub save_ram: bool,
    pub front: bool,
    pub main: bool,
    pub has_window: bool,
    pub minimised: bool,
    /// Hidden by Bloxkeep.
    pub hidden: bool,
    /// Lent for a moment (a nudge, a macro's turn, a picture): left as it is.
    pub lent: bool,
    pub game: Game,
    /// Bytes, and its page faults so far (all, and those read from disk).
    pub working_set: i64,
    pub page_faults: i64,
    pub hard_faults: i64,
    /// How much of the PC's RAM is in use, in %.
    pub load_percent: i32,
    pub memory: RamMemory,
    /// Settings › CPU limit for resting windows, in % of the PC's whole CPU (0: off).
    pub cpu_percent: i32,
    pub cpu: CpuMemory,
}

/// The limit wanted for a window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RamCap {
    /// Not touched (the window is lent).
    Leave,
    /// No limit.
    Lift,
    Mb(i64),
}

#[derive(Clone, Debug)]
pub struct RamWanted {
    pub cap: RamCap,
    /// What to ask Windows now: the cap where it differs from what was asked or held (`Some(None)`: a lift).
    pub ask: Option<Option<i64>>,
    /// The word for the log line: "moving to a server", "auto" or "chosen" for a limit; why, for a lift.
    pub why: &'static str,
    /// Save RAM: empty its RAM now.
    pub empty_once: bool,
    /// As it is to be remembered (with `ask` as asked).
    pub memory: RamMemory,
    /// A line Auto has for the log (it gave up on a window for a while).
    pub note: Option<String>,
}

/// The RAM limit and Save RAM's emptying for one window: what `limit_resting_ram` and `empty_resting_ram_once` carry out.
/// No Win32 and no clock of its own: the same facts give the same answer.
///
/// Limited when not in front, not a main account's, it has a window, and it is minimized or hidden (or, with "Limit open
/// windows too", in view with its log read). While it moves to a server its limit is `MOVING_LIMIT_MB` (or the chosen
/// one, or Auto's, if higher). Anything else lifts what Bloxkeep set. A lent window keeps what it has. Save RAM empties a
/// window once a rest, after it rested `EMPTY_RAM_RESTED` minimized or hidden, not in front, not a main account's, and
/// not moving.
pub fn ram_plan(f: &RamFacts) -> RamWanted {
    let mut m = f.memory.clone();
    // A window lent for a moment (anti-AFK's nudge, /pc screenshot) is left as it is meanwhile: its limit, Auto's
    // measures, and its rest, which goes on.
    if f.lent {
        return RamWanted { cap: RamCap::Leave, ask: None, why: "", empty_once: false, memory: m, note: None };
    }
    let auto_on = f.setting == RamSetting::Auto;
    if !auto_on {
        m.auto = None; // back on Auto later, it waits and measures again
    }

    // Save RAM's one emptying a rest.
    let mut empty_once = false;
    if !f.save_ram {
        (m.resting_since, m.emptied) = (None, false);
    } else if f.front || f.main || !f.has_window || !(f.minimised || f.hidden) || f.game.settling(f.now) {
        (m.resting_since, m.emptied) = (None, false);
    } else {
        match m.resting_since {
            None => m.resting_since = Some(f.now),
            Some(since) => {
                if f.now.secs_since(since) >= secs(EMPTY_RAM_RESTED) && !m.emptied {
                    (m.emptied, empty_once) = (true, true);
                }
            }
        }
    }

    let put_away = f.setting != RamSetting::Off
        && !f.front
        && !f.main
        && f.has_window
        // "Limit open windows too": one in view as well. Not one still opening: its log says nothing yet, so it isn't
        // "moving", and it was held at the chosen limit for the seconds it was starting up in.
        && (f.minimised || f.hidden || (f.open_too && f.game != Game::Unknown));
    let moving = put_away && f.game.settling(f.now);
    let mut note = None;
    let (cap, why) = if moving {
        // Auto's own limit, raised for a game that needed more, stays if it is higher. Auto's limit is kept for after the
        // move, its faults counted afresh: loading a place faults a lot, which says nothing of the limit.
        let auto = m.auto.as_ref().map_or(0, |a| a.cap_mb);
        if let Some(a) = m.auto.as_mut() {
            (a.faults, a.hard_faults) = (-1, -1);
            a.rates.clear();
        }
        (RamCap::Mb(MOVING_LIMIT_MB.max(f.setting.chosen_mb()).max(auto)), "moving to a server")
    } else if !put_away {
        m.auto = None; // shown, in front, a main account's: Auto starts over, waiting its minute
        (RamCap::Lift, "shown, in front, or the option off")
    } else if auto_on {
        let (cap, said) = auto_cap(&mut m.auto, f);
        note = said;
        match cap {
            Some(mb) => (RamCap::Mb(mb), "auto"),
            None => (RamCap::Lift, "auto left it alone"),
        }
    } else {
        (RamCap::Mb(f.setting.chosen_mb()), "chosen")
    };
    let ask = match cap {
        RamCap::Leave => None,
        RamCap::Lift => m.limit().is_some().then_some(None),
        RamCap::Mb(mb) => (m.limit() != Some(mb)).then_some(Some(mb)),
    };
    if ask.is_some() {
        m.asked = ask;
    }
    RamWanted { cap, ask, why, empty_once, memory: m, note }
}

/// What Bloxkeep asked of one window's CPU limit and what Windows said, as the plan reads it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CpuMemory {
    /// An ask still on its way through the RAM turn: `Some(None)` a lift, `Some(Some(percent))` a limit.
    pub asked: Option<Option<i32>>,
    /// The limit Windows last said yes to (None: none).
    pub held: Option<i32>,
    /// Windows said no to the last ask; it is asked again.
    pub refused: bool,
}

impl CpuMemory {
    /// The limit it has once the asks on their way are answered yes.
    pub fn limit(&self) -> Option<i32> {
        self.asked.unwrap_or(self.held)
    }
}

/// The CPU limit wanted for a window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CpuCap {
    /// Not touched (the window is lent).
    Leave,
    /// No limit.
    Lift,
    Percent(i32),
}

#[derive(Clone, Debug)]
pub struct CpuWanted {
    pub cap: CpuCap,
    /// What to ask Windows now: the cap where it differs from what was asked or held (`Some(None)`: a lift).
    pub ask: Option<Option<i32>>,
    /// The word for the log line.
    pub why: &'static str,
    /// As it is to be remembered (with `ask` as asked).
    pub memory: CpuMemory,
}

/// The CPU limit for one window: what `limit_resting_cpu` carries out. No Win32 and no clock of its own.
///
/// The same windows as the RAM limit's chosen number: limited when not in front, not a main account's, it has a
/// window, and it is minimized or hidden (or, with "Limit open windows too", in view with its log read). A window
/// moving to a server has no CPU limit at all: loading the place needs the CPU, and a starved join or teleport drops.
/// Anything else lifts what Bloxkeep set. A lent window keeps what it has.
pub fn cpu_plan(f: &RamFacts) -> CpuWanted {
    let mut m = f.cpu.clone();
    if f.lent {
        return CpuWanted { cap: CpuCap::Leave, ask: None, why: "", memory: m };
    }
    let put_away = !f.front && !f.main && f.has_window && (f.minimised || f.hidden || (f.open_too && f.game != Game::Unknown));
    let (cap, why) = if f.cpu_percent <= 0 {
        (CpuCap::Lift, "the option off")
    } else if !put_away {
        (CpuCap::Lift, "shown, in front, or a main account's")
    } else if f.game.settling(f.now) {
        (CpuCap::Lift, "moving to a server")
    } else {
        (CpuCap::Percent(f.cpu_percent), "chosen")
    };
    let ask = match cap {
        CpuCap::Leave => None,
        CpuCap::Lift => m.limit().is_some().then_some(None),
        CpuCap::Percent(p) => (m.limit() != Some(p)).then_some(Some(p)),
    };
    if ask.is_some() {
        m.asked = ask;
    }
    CpuWanted { cap, ask, why, memory: m }
}

/// The choices of Settings › Staying in game › CPU limit for resting windows, in % of the PC's whole CPU (0 = off).
pub const CPU_LIMIT_CHOICES: [i32; 10] = [0, 1, 2, 3, 5, 10, 15, 20, 30, 50];

/// Auto's limit for a resting window now (MB), or None for none: not before it rested `AUTO_RAM_RESTED`, nor below
/// `AUTO_RAM_LOAD` % of RAM used (a limit already set stays), nor while it is left alone after needing more twice. Its
/// fault rates (median of the last five looks, so one garbage collection's burst is no reason) move it. The second value
/// is a line for the log.
pub fn auto_cap(auto: &mut Option<AutoRam>, f: &RamFacts) -> (Option<i64>, Option<String>) {
    let now = f.now;
    let a = auto.get_or_insert_with(|| AutoRam {
        cap_mb: 0,
        changed_at: stamp_min(),
        resting_since: now,
        raises: 0,
        left_alone_until: None,
        faults: -1,
        hard_faults: -1,
        last_look: now,
        rates: Vec::new(),
    });
    if f.page_faults > 0 && a.faults >= 0 && now > a.last_look {
        let seconds = now.secs_since(a.last_look);
        let (soft, hard) = (0.max(f.page_faults - a.faults) as f64 / seconds, 0.max(f.hard_faults - a.hard_faults) as f64 / seconds);
        // Faulting while it holds well under its limit: Windows took its RAM (another program needed more than the PC
        // has), which says nothing of the limit, and a higher one gives nothing back. Not counted, so not raised for it.
        let starved = soft > AUTO_RAM_STARVED || hard > AUTO_RAM_HARD_STARVED;
        if !starved || a.cap_mb == 0 || f.working_set >= a.cap_mb * MB * 9 / 10 {
            a.rates.push((soft, hard));
        }
        if a.rates.len() > 5 {
            a.rates.remove(0);
        }
    }
    (a.faults, a.hard_faults, a.last_look) = (f.page_faults, f.hard_faults, now);
    if let Some(until) = a.left_alone_until {
        if now < until {
            return (None, None);
        }
        a.left_alone_until = None;
        a.raises = 0;
        a.cap_mb = 0;
    }
    if a.cap_mb == 0 {
        if now.secs_since(a.resting_since) < secs(AUTO_RAM_RESTED) || f.load_percent < AUTO_RAM_LOAD {
            return (None, None);
        }
        a.cap_mb = AUTO_RAM_FLOOR_MB.max(f.working_set / MB * 3 / 4 / 64 * 64); // what's shared counts towards a limit too
        a.changed_at = now;
        a.rates.clear();
        return (Some(a.cap_mb), None);
    }
    if a.rates.len() >= 3 {
        let soft = median(a.rates.iter().map(|r| r.0));
        let hard = median(a.rates.iter().map(|r| r.1));
        if soft > AUTO_RAM_STARVED || hard > AUTO_RAM_HARD_STARVED {
            a.raises += 1;
            if a.raises >= 3 {
                a.left_alone_until = Some(now.plus_secs(secs(AUTO_RAM_LEFT_ALONE)));
                let said = format!(
                    "RAM limit: Roblox window {} needs its RAM ({soft:.0} faults/s, {hard:.0} from disk); left alone for {:.0} min.",
                    f.pid,
                    secs(AUTO_RAM_LEFT_ALONE) / 60.0
                );
                return (None, Some(said));
            }
            a.cap_mb = a.cap_mb * 3 / 2 / 64 * 64;
            a.changed_at = now.plus_secs(secs(AUTO_RAM_RAISED_HOLD) - secs(AUTO_RAM_STEP)); // held a while before it may come down again
            a.rates.clear();
        } else if soft < AUTO_RAM_CALM
            && hard < 1.0
            && now.secs_since(a.changed_at) >= secs(AUTO_RAM_STEP)
            && a.cap_mb > AUTO_RAM_FLOOR_MB
            && f.load_percent >= AUTO_RAM_LOAD
        {
            a.cap_mb = AUTO_RAM_FLOOR_MB.max(a.cap_mb * 3 / 4 / 64 * 64);
            a.changed_at = now;
            a.raises = 0; // calm again: an asset load an hour ago is no reason to give up on it
            a.rates.clear();
        }
    }
    (Some(a.cap_mb), None)
}

/// The choices of Settings › Staying in game › RAM limit for resting windows, in MB (0 = off).
pub const RAM_LIMIT_CHOICES: [i32; 7] = [0, RAM_LIMIT_AUTO, 512, 768, 1024, 1536, 2048];

/// "Auto": each resting window's limit found for its own game (2026-10-30, two designs measured on a stand-in game: a
/// limit above what a game keeps touching costs nothing, one below it makes the game fetch its own pages back for ever,
/// ~430k faults/s and 5 fps). Starts at 3/4 of what the window holds, comes down a quarter at a time while it barely
/// faults, goes up half again when it faults a lot, and a window that needed it twice is left alone a while. Only once the
/// PC has used `AUTO_RAM_LOAD` % of its RAM: below that a limit frees RAM nobody needs.
pub const RAM_LIMIT_AUTO: i32 = -1;
pub const AUTO_RAM_LOAD: i32 = 60;
pub const AUTO_RAM_FLOOR_MB: i64 = 768;
pub const AUTO_RAM_RESTED: Duration = Duration::from_secs(60);
pub const AUTO_RAM_STEP: Duration = Duration::from_secs(60);
pub const AUTO_RAM_RAISED_HOLD: Duration = Duration::from_secs(10 * 60);
pub const AUTO_RAM_LEFT_ALONE: Duration = Duration::from_secs(30 * 60);
/// Faults a second (median of the last few looks): under this the limit may come down, over the next it goes up. A game
/// at its own pace faulted under 700/s on the stand-in; one short of what it touches, hundreds of thousands.
pub const AUTO_RAM_CALM: f64 = 5_000.0;
pub const AUTO_RAM_STARVED: f64 = 25_000.0;
pub const AUTO_RAM_HARD_STARVED: f64 = 50.0;

/// Auto's state for a window: its limit, since when, raises, fault counts last seen, recent rates.
#[derive(Clone, Debug)]
pub struct AutoRam {
    pub cap_mb: i64,
    pub changed_at: Stamp,
    pub resting_since: Stamp,
    pub raises: i32,
    pub left_alone_until: Option<Stamp>,
    pub faults: i64,
    pub hard_faults: i64,
    pub last_look: Stamp,
    pub rates: Vec<(f64, f64)>,
}

type RamJob = Box<dyn FnOnce() + Send>;

/// The C#'s `_ramLimitTurn` (a chain of ContinueWith): one worker thread taking the asks in order, and how many are
/// still to go (LiftRamLimits waits for them).
pub struct RamTurn {
    jobs: mpsc::Sender<RamJob>,
    left: Arc<(Mutex<usize>, Condvar)>,
}

impl RamTurn {
    fn start() -> RamTurn {
        let (jobs, inbox) = mpsc::channel::<RamJob>();
        let _ = crate::threads::own("ram-turn", move || {
            for job in inbox {
                // One that panics (shown as an unexpected error) doesn't end the turn for the asks after it.
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
            }
        });
        RamTurn { jobs, left: Arc::new((Mutex::new(0), Condvar::new())) }
    }
}

#[derive(Default)]
pub struct MemoryState {
    pub low_memory_priority: RefCell<HashSet<i32>>,
    /// Every Roblox window open at the first look has had its RAM priority set (see guard_resources).
    pub memory_priority_adopted: Cell<bool>,
    /// Since when each window has rested, and the ones already emptied this rest (see empty_resting_ram_once).
    pub resting_since: RefCell<HashMap<i32, Stamp>>,
    pub ram_emptied: RefCell<HashSet<i32>>,
    pub auto_ram: RefCell<HashMap<i32, AutoRam>>,
    /// Windows Windows refused a limit for, and a lift for, said once each in the log.
    pub ram_limit_refused: RefCell<HashSet<i32>>,
    pub ram_lift_refused: RefCell<HashSet<i32>>,
    /// Clients under a RAM limit, and the limit (-1: one found already limited): rebuilt from `ram_asks` after every change
    /// there, for the parts that only ask whether a window is limited.
    pub ram_limited: RefCell<HashMap<i32, i64>>,
    /// What was asked of each client's RAM limit and what Windows said yes to: the record the limit pass goes by.
    pub ram_asks: RefCell<HashMap<i32, RamAsk>>,
    /// Numbers the asks, so an answer is told from a newer ask's.
    pub ram_ask_serial: Cell<u64>,
    /// Clients the limit pass has looked at: one seen for the first time with a limit nobody here asked for is adopted.
    pub ram_seen: RefCell<HashSet<i32>>,
    /// The tick's one look at the front window's process and the main accounts' (see `ram_look`).
    pub look: RefCell<Option<(Stamp, i32, HashSet<i32>)>>,
    pub ram_limit_turn: RefCell<Option<RamTurn>>,
    /// What was asked of each client's CPU limit and what Windows said yes to.
    pub cpu_asks: RefCell<HashMap<i32, CpuAsk>>,
    pub cpu_ask_serial: Cell<u64>,
    /// Windows Windows refused a CPU limit for, and a lift for, said once each in the log.
    pub cpu_limit_refused: RefCell<HashSet<i32>>,
    pub cpu_lift_refused: RefCell<HashSet<i32>>,
    /// When the jobs of ended clients were last closed (cpu_cap::tidy).
    pub cpu_tidied: Cell<Option<Stamp>>,
}

/// One client's CPU limit as asked and as Windows answered (the shape of `RamAsk`).
#[derive(Clone, Debug, Default)]
pub struct CpuAsk {
    /// The last ask still on its way (`Some(None)`: a lift), and its number.
    pub asked: Option<Option<i32>>,
    pub serial: u64,
    /// The limit Windows last said yes to, in %.
    pub held: Option<i32>,
    pub refused: bool,
    /// Answers to asks numbered up to this one came before this record and say nothing of it.
    pub from: u64,
}

impl CpuAsk {
    /// The limit the client is under now, or will be once its ask is answered yes (a lift on its way: still the old one).
    pub fn limited_to(&self) -> Option<i32> {
        self.asked.flatten().or(self.held)
    }
}

/// How often the jobs of ended clients are looked for and closed.
const CPU_TIDY_EVERY: Duration = Duration::from_secs(60);

/// One client's RAM limit as asked and as Windows answered.
#[derive(Clone, Debug, Default)]
pub struct RamAsk {
    /// The last ask still on its way (`Some(None)`: a lift), and its number.
    pub asked: Option<Option<i64>>,
    pub serial: u64,
    /// The limit Windows last said yes to (`Some(-1)`: one found set).
    pub held: Option<i64>,
    pub refused: bool,
    /// Answers to asks numbered up to this one came before this record and say nothing of it.
    pub from: u64,
}

impl RamAsk {
    /// The limit the client is under now, or will be once its ask is answered yes (a lift on its way: still the old one).
    pub fn limited_to(&self) -> Option<i64> {
        self.asked.flatten().or(self.held)
    }
}

pub fn median(values: impl Iterator<Item = f64>) -> f64 {
    let mut sorted: Vec<f64> = values.collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if sorted.is_empty() { 0.0 } else { sorted[sorted.len() / 2] }
}

impl App {
    /// Opening `to_open` more windows would leave less than the margin of the commit room: Play's "virtual memory is almost
    /// full" warning.
    pub fn virtual_memory_short(memory: &MemoryStatus, to_open: i32, per_client_commit_mb: i64) -> bool {
        to_open > 0 && memory.commit_limit_mb > 0 && memory.commit_room_mb() - to_open as i64 * per_client_commit_mb < COMMIT_MARGIN_MB
    }

    /// The average of those running, else `DEFAULT_CLIENT_COMMIT_MB`.
    pub fn client_commit_mb(players: &[PlayerProcess]) -> i64 {
        let known = players.iter().filter(|p| p.private > 0).count() as i64;
        if known > 0 {
            (DEFAULT_CLIENT_COMMIT_MB / 2).max(players.iter().filter(|p| p.private > 0).map(|p| p.private).sum::<i64>() / known / MB)
        } else {
            DEFAULT_CLIENT_COMMIT_MB
        }
    }

    /// Another Roblox wouldn't fit in the commit room, or RAM is nearly all used. The main window then closes its browser
    /// sooner in the background.
    pub fn memory_tight(memory: &MemoryStatus) -> bool {
        (memory.commit_limit_mb > 0 && memory.commit_room_mb() < DEFAULT_CLIENT_COMMIT_MB + COMMIT_MARGIN_MB) || memory.load_percent >= MEMORY_WARNING_LOAD
    }

    /// Warns when the PC runs low on RAM. With "save RAM on background windows", windows you aren't playing get a low memory
    /// priority: Windows takes RAM from them first, only when it runs short.
    pub fn guard_resources(&self, players: &[PlayerProcess], memory: &MemoryStatus, now: Stamp) {
        // PORT: `_vault!.Data.Settings` threw when locked; the caller runs it only while unlocked, so locked does nothing.
        let Ok((memory_warnings, trim_background)) = self.vault(|d| (d.settings.memory_warnings, d.settings.trim_background)) else { return };
        self.auto_clean(memory, now);
        let warned_lately = now.secs_since(self.controller.last_memory_warning.get()) <= secs(MEMORY_WARNING_INTERVAL);
        if memory_warnings && !players.is_empty() && memory.load_percent >= MEMORY_WARNING_LOAD && !warned_lately {
            self.controller.last_memory_warning.set(now);
            self.notify(&l::f("Your PC is using {0}% of its RAM. Close some Roblox windows if things get slow.", &[&memory.load_percent]), "warn", "ram", Guid::nil(), true, None);
        }
        // RAM may be fine while the commit limit is nearly used up: a Roblox window that needs more then crashes.
        else if memory_warnings && !players.is_empty() && memory.commit_limit_mb > 0 && memory.commit_room_mb() < COMMIT_MARGIN_MB && !warned_lately {
            self.controller.last_memory_warning.set(now);
            self.notify(
                &l::t("RAM is free, but Windows' virtual memory is almost full, so a Roblox window may crash. Close other apps or increase virtual memory."),
                "warn",
                "ram",
                Guid::nil(),
                true,
                None,
            );
        }

        self.memory.low_memory_priority.borrow_mut().retain(|pid| players.iter().any(|p| p.pid == *pid));
        // The first look at the windows: a Bloxkeep that crashed (or was ended) left its background windows at low priority
        // with no list of them, so one in front, or every one with the option off, stayed low (2026-10-30 review). Each gets
        // what it should have, once.
        let (foreground, main) = self.ram_look(now);
        if !self.memory.memory_priority_adopted.get() && !players.is_empty() {
            self.memory.memory_priority_adopted.set(true);
            let (front, mains) = (foreground, &main);
            for p in players {
                let low = trim_background && p.pid != front && !mains.contains(&p.pid);
                if roblox_processes::set_memory_priority(p.pid, low) && low {
                    self.memory.low_memory_priority.borrow_mut().insert(p.pid);
                }
            }
        }
        if !trim_background {
            self.give_memory_priority_back();
            return;
        }
        if players.is_empty() {
            return;
        }

        // Only touch a window when its role changes (it went to the background, or came to the front). The window played
        // on (a main account's) is never Save RAM's.
        let mut refused = false;
        for p in players {
            let background = p.pid != foreground && !main.contains(&p.pid);
            if background == self.memory.low_memory_priority.borrow().contains(&p.pid) {
                continue;
            }
            if !roblox_processes::set_memory_priority(p.pid, background) {
                refused |= !roblox_processes::ended(p.pid); // one that closed since the scan had nothing to change
                continue;
            }
            if background {
                self.memory.low_memory_priority.borrow_mut().insert(p.pid);
            } else {
                self.memory.low_memory_priority.borrow_mut().remove(&p.pid);
            }
        }

        // No emptying the RAM of windows in view: a background game needed it all back at once, and with 5 windows some
        // stuttered, a window grabbed to drag most of all. Resting ones: empty_resting_ram_once.
        if refused && !self.controller.trim_refused_shown.get() {
            self.controller.trim_refused_shown.set(true);
            // Nothing the person can do about it: noted, not shown.
            log::note("Save RAM: Windows refused to lower a Roblox window's memory priority; it runs as usual.");
        }
    }

    /// Save RAM: a window that has rested a minute (minimized, or hidden by Bloxkeep) has its RAM emptied, once each rest:
    /// what the game doesn't touch stays out of RAM and the rest comes back. Not again and again, and not a window in view
    /// (both stuttered, 2026-10-21); the rest of the time only the low priority of guard_resources applies. Noted in the
    /// log (pid and MB only), to tell from a real game what it gave. Not yet tried on real Roblox (2026-10-31).
    pub fn empty_resting_ram_once(&self, players: &[PlayerProcess], windows: &HashMap<i32, Vec<isize>>, now: Stamp) {
        let alive = |pid: &i32| players.iter().any(|p| p.pid == *pid);
        self.memory.resting_since.borrow_mut().retain(|pid, _| alive(pid));
        self.memory.ram_emptied.borrow_mut().retain(|pid| alive(pid));
        // PORT: `_vault!` threw when locked; the caller runs it only while unlocked.
        let Ok((setting, open_too, trim_background)) = self.vault(|d| (d.settings.ram_limit_mb as i64, d.settings.ram_limit_open, d.settings.trim_background)) else {
            return;
        };
        if !trim_background {
            self.memory.resting_since.borrow_mut().clear();
            self.memory.ram_emptied.borrow_mut().clear();
            return;
        }
        // Only the rest is taken from the plan here; the limit is limit_resting_ram's.
        for (p, f) in players.iter().zip(self.ram_facts(players, windows, now, None, RamSetting::of(setting), open_too, trim_background, 0)) {
            let w = ram_plan(&f);
            match w.memory.resting_since {
                Some(since) => self.memory.resting_since.borrow_mut().insert(p.pid, since),
                None => self.memory.resting_since.borrow_mut().remove(&p.pid),
            };
            if w.memory.emptied {
                self.memory.ram_emptied.borrow_mut().insert(p.pid);
            } else {
                self.memory.ram_emptied.borrow_mut().remove(&p.pid);
            }
            if w.empty_once {
                self.empty_ram_in_turn(p.pid, p.working_set / MB);
            }
        }
    }

    /// The front window's process and the main accounts' processes as the tick at `now` read them: read once, so Save
    /// RAM's priority, the RAM limit and the emptying all decide from the same look. A call with another `now` reads again.
    pub fn ram_look(&self, now: Stamp) -> (i32, HashSet<i32>) {
        if let Some((at, front, mains)) = self.memory.look.borrow().as_ref() {
            if *at == now {
                return (*front, mains.clone());
            }
        }
        let (front, mains) = (roblox_processes::foreground_pid(), self.main_pids());
        *self.memory.look.borrow_mut() = Some((now, front, mains.clone()));
        (front, mains)
    }

    /// Each client's facts for `ram_plan`, from the tick's one look at the front window and the main accounts.
    /// `memory` None: the PC's RAM load isn't needed (counted as 0).
    #[allow(clippy::too_many_arguments)]
    fn ram_facts(
        &self,
        players: &[PlayerProcess],
        windows: &HashMap<i32, Vec<isize>>,
        now: Stamp,
        memory: Option<&MemoryStatus>,
        setting: RamSetting,
        open_too: bool,
        save_ram: bool,
        cpu_percent: i32,
    ) -> Vec<RamFacts> {
        let (front, mains) = self.ram_look(now);
        let logs = self.controller.logs.borrow();
        players
            .iter()
            .map(|p| {
                let window = windows.get(&p.pid).map_or(0, |list| roblox_processes::main_window_of(list));
                let has_window = window != 0;
                RamFacts {
                    pid: p.pid,
                    now,
                    setting,
                    open_too,
                    save_ram,
                    front: p.pid == front,
                    main: mains.contains(&p.pid),
                    has_window,
                    minimised: has_window && roblox_processes::is_minimised(window),
                    hidden: has_window && roblox_processes::is_hidden(window),
                    lent: has_window && crate::core::window_loan::is_lent(window),
                    game: Game::of(logs.state_of(p.pid).as_ref()),
                    working_set: p.working_set,
                    page_faults: p.page_faults,
                    hard_faults: p.hard_faults,
                    load_percent: memory.map_or(0, |m| m.load_percent),
                    memory: self.ram_memory(p.pid),
                    cpu_percent,
                    cpu: self.cpu_memory(p.pid),
                }
            })
            .collect()
    }

    /// One client's memory as the plan reads it, from the records each part keeps.
    fn ram_memory(&self, pid: i32) -> RamMemory {
        let ask = self.memory.ram_asks.borrow().get(&pid).cloned().unwrap_or_default();
        RamMemory {
            asked: ask.asked,
            held: ask.held,
            refused: ask.refused,
            auto: self.memory.auto_ram.borrow().get(&pid).cloned(),
            resting_since: self.memory.resting_since.borrow().get(&pid).copied(),
            emptied: self.memory.ram_emptied.borrow().contains(&pid),
        }
    }

    /// One client's CPU limit as the plan reads it.
    fn cpu_memory(&self, pid: i32) -> CpuMemory {
        let ask = self.memory.cpu_asks.borrow().get(&pid).cloned().unwrap_or_default();
        CpuMemory { asked: ask.asked, held: ask.held, refused: ask.refused }
    }

    /// `ram_limited` made again from the record (`ram_asks`), so a part that took an entry out of it (a lift of its own)
    /// doesn't hide a limit Windows still holds.
    fn sync_ram_limited(&self) {
        let asks = self.memory.ram_asks.borrow();
        let mut limited = self.memory.ram_limited.borrow_mut();
        limited.clear();
        limited.extend(asks.iter().filter_map(|(pid, a)| a.limited_to().map(|mb| (*pid, mb))));
    }

    /// A client seen for the first time that has a hard RAM limit nobody here asked for (a Bloxkeep that closed before
    /// lifting it, a lift Windows refused before a lock) is taken on as limited, so the pass lifts or keeps it as it should.
    fn adopt_ram_limits(&self, players: &[PlayerProcess]) {
        for p in players {
            if !self.memory.ram_seen.borrow_mut().insert(p.pid) || self.memory.ram_asks.borrow().contains_key(&p.pid) {
                continue;
            }
            if roblox_processes::has_ram_limit(p.pid) {
                let from = self.memory.ram_ask_serial.get();
                self.memory.ram_asks.borrow_mut().insert(p.pid, RamAsk { held: Some(-1), from, ..Default::default() });
            }
        }
    }

    /// RAM limit for resting windows: a Roblox window that isn't drawing (minimized, or hidden by Bloxkeep) keeps at most the
    /// chosen RAM (or Auto's, `RAM_LIMIT_AUTO`), the rest waiting in compressed memory or virtual memory, so more accounts fit on
    /// one PC (a resting game uses little of what it holds: about 1 of its 3 GB, measured 2026-10-29). Lifted once the window
    /// shows, comes to the front, or the option is off. Not for one in view: it still draws, and limited it took twice the CPU;
    /// unless "Limit open windows too" (`ram_limit_open`) asks for that, the window in front and a main account's still left out.
    /// Every limit set, changed and lifted is noted in the log (pid and MB only), so a window seen above its limit can be told
    /// apart from one that wasn't limited then (a user saw 2 GB).
    pub fn limit_resting_ram(&self, players: &[PlayerProcess], windows: &HashMap<i32, Vec<isize>>, now: Stamp, memory: &MemoryStatus) {
        let alive = |pid: &i32| players.iter().any(|p| p.pid == *pid);
        self.memory.ram_asks.borrow_mut().retain(|pid, _| alive(pid));
        self.memory.auto_ram.borrow_mut().retain(|pid, _| alive(pid));
        self.memory.ram_limit_refused.borrow_mut().retain(|pid| alive(pid));
        self.memory.ram_lift_refused.borrow_mut().retain(|pid| alive(pid));
        self.memory.ram_seen.borrow_mut().retain(|pid| alive(pid));
        self.memory.cpu_asks.borrow_mut().retain(|pid, _| alive(pid));
        self.memory.cpu_limit_refused.borrow_mut().retain(|pid| alive(pid));
        self.memory.cpu_lift_refused.borrow_mut().retain(|pid| alive(pid));
        self.adopt_ram_limits(players);
        self.sync_ram_limited();
        // A limit held as set is read back from Windows: a process put in a job (the CPU limit's) loses its hard limit
        // without a word (its maximum still reads the same, the flag is off), and the record went on saying "held" while
        // the window grew past it. One found gone is asked again by the pass below.
        let lost: Vec<i32> = self
            .memory
            .ram_asks
            .borrow()
            .iter()
            .filter(|(_, a)| a.asked.is_none() && a.held.is_some_and(|mb| mb > 0))
            .map(|(pid, _)| *pid)
            .filter(|pid| !roblox_processes::has_ram_limit(*pid) && !roblox_processes::ended(*pid))
            .collect();
        for pid in lost {
            if let Some(a) = self.memory.ram_asks.borrow_mut().get_mut(&pid) {
                a.held = None;
            }
            log::note(&format!("RAM limit: Roblox window {pid} had lost its limit (Windows drops it when a CPU limit is set); set again."));
        }
        self.sync_ram_limited();
        // PORT: `_vault!` threw when locked; the caller runs it only while unlocked.
        let Ok((setting, open_too, trim_background, cpu_percent)) =
            self.vault(|d| (d.settings.ram_limit_mb as i64, d.settings.ram_limit_open, d.settings.trim_background, d.settings.cpu_limit_percent))
        else {
            return;
        };
        let setting = RamSetting::of(setting);
        if setting != RamSetting::Auto {
            self.memory.auto_ram.borrow_mut().clear(); // back on Auto later, it waits and measures again
        }
        let facts = self.ram_facts(players, windows, now, Some(memory), setting, open_too, trim_background, cpu_percent);
        // Only the limit is taken from the plan here; the rest is empty_resting_ram_once's.
        for (p, f) in players.iter().zip(&facts) {
            let w = ram_plan(f);
            if let Some(note) = &w.note {
                log::note(note);
            }
            match w.memory.auto {
                Some(a) => self.memory.auto_ram.borrow_mut().insert(p.pid, a),
                None => self.memory.auto_ram.borrow_mut().remove(&p.pid),
            };
            // (Not of a window Bloxkeep is closing: it is on its way out.)
            if let Some(limit) = w.ask.filter(|_| !roblox_processes::is_closing(p.pid)) {
                self.ask_ram_limit(p.pid, limit, w.why, p.ram() / MB);
            }
        }
        self.sync_ram_limited();
        self.limit_resting_cpu(&facts, now);
    }

    /// CPU limit for resting windows: a Roblox window that isn't drawing (minimized, or hidden by Bloxkeep; or open, with
    /// "Limit open windows too") may use at most the chosen share of the PC's whole CPU, held by a job object's hard cap
    /// (cpu_cap.rs), so more accounts run on one PC. Decided by `cpu_plan` from the facts the RAM limit read this tick, and
    /// asked through the same turn and the same checked path. Not for a window Bloxkeep is closing for a rejoin.
    fn limit_resting_cpu(&self, facts: &[RamFacts], now: Stamp) {
        for f in facts {
            let w = cpu_plan(f);
            let Some(limit) = w.ask else { continue };
            if roblox_processes::is_closing(f.pid) || limit.is_some() && self.close_desk.own_close(f.pid) {
                continue;
            }
            self.ask_cpu_limit(f.pid, limit, w.why);
        }
        // The jobs of clients that ended are closed now and then (a job is kept for as long as its process runs).
        if self.memory.cpu_tidied.get().is_none_or(|at| now.secs_since(at) >= secs(CPU_TIDY_EVERY)) {
            self.memory.cpu_tidied.set(Some(now));
            let _ = self.in_ram_turn(|| {
                cpu_cap::tidy();
                true
            });
        }
    }

    /// A CPU limit (`Some(percent)`) or a lift (None) asked of Windows in the RAM turn, remembered as held only once
    /// Windows said yes; a refusal leaves the record as it was, so the next pass asks again, and is said once per window
    /// in the log with Windows' error code. One that closed meanwhile had nothing to change. `why` is the word for the
    /// log; empty: no line for a lift.
    pub fn ask_cpu_limit(&self, pid: i32, limit: Option<i32>, why: &'static str) {
        let serial = self.memory.cpu_ask_serial.get() + 1;
        self.memory.cpu_ask_serial.set(serial);
        {
            let mut asks = self.memory.cpu_asks.borrow_mut();
            let ask = asks.entry(pid).or_insert_with(|| CpuAsk { from: serial - 1, ..Default::default() });
            ask.asked = Some(limit);
            ask.serial = serial;
        }
        let code = Arc::new(AtomicU32::new(0));
        let error = code.clone();
        let answer = self.in_ram_turn(move || match cpu_cap::set(pid, limit) {
            Ok(()) => true,
            Err(e) => {
                error.store(e, Ordering::Relaxed);
                false
            }
        });
        rt::spawn(async move {
            let ok = answer.await;
            let code = code.load(Ordering::Relaxed);
            App::with(|app| app.cpu_limit_answered(pid, limit, serial, ok, code, why));
        });
    }

    fn cpu_limit_answered(&self, pid: i32, limit: Option<i32>, serial: u64, ok: bool, code: u32, why: &'static str) {
        {
            let mut asks = self.memory.cpu_asks.borrow_mut();
            // Answers come in the order asked; one to an ask from before the record was made says nothing of it.
            if let Some(ask) = asks.get_mut(&pid).filter(|a| serial > a.from) {
                if ok {
                    ask.held = limit;
                }
                ask.refused = !ok;
                if ask.serial == serial {
                    ask.asked = None;
                }
            }
        }
        match (ok, limit) {
            (true, Some(percent)) => log::note(&format!("CPU limit: Roblox window {pid} limited to {percent} % ({why}).")),
            (true, None) => {
                if !why.is_empty() {
                    log::note(&format!("CPU limit: lifted for Roblox window {pid} ({why})."));
                }
            }
            (false, _) if roblox_processes::ended(pid) => {}
            (false, Some(_)) => {
                if self.memory.cpu_limit_refused.borrow_mut().insert(pid) {
                    log::note(&format!("CPU limit: Windows refused a limit for Roblox window {pid} (error {code})."));
                }
            }
            (false, None) => {
                if self.memory.cpu_lift_refused.borrow_mut().insert(pid) {
                    log::note(&format!("CPU limit: Windows refused to lift the limit of Roblox window {pid} (error {code})."));
                }
            }
        }
    }

    /// The client's CPU limit lifted at once (it came to the front, it became a main account's), not at the next pass. A
    /// lift already on its way isn't asked twice; one with no limit has nothing to lift.
    pub fn lift_cpu_limit_now(&self, pid: i32, why: &'static str) {
        let limited = pid != 0 && self.memory.cpu_asks.borrow().get(&pid).is_some_and(|a| a.limited_to().is_some() && a.asked != Some(None));
        if limited {
            self.ask_cpu_limit(pid, None, why);
        }
    }

    /// Every CPU limit Bloxkeep set switched off (Bloxkeep closing or locking): a job outlives Bloxkeep while its process
    /// runs, and a Bloxkeep started later cannot find it again. Every job cpu_cap.rs holds is switched off, whatever the
    /// record says; the record goes, so the pass after an unlock decides every client afresh (the jobs are kept and used
    /// again). Waits, as the RAM limit's lift does, for the asks still in the turn.
    pub fn lift_cpu_limits(&self) {
        let answer = self.in_ram_turn(|| {
            let refused = cpu_cap::lift_all();
            for (pid, code) in &refused {
                if !roblox_processes::ended(*pid) {
                    log::note(&format!("CPU limit: Windows refused to lift the limit of Roblox window {pid} (error {code})."));
                }
            }
            refused.is_empty()
        });
        drop(answer); // the job is queued; its answer is only for the log, written on the turn
        self.memory.cpu_asks.borrow_mut().clear();
        self.memory.cpu_limit_refused.borrow_mut().clear();
        self.memory.cpu_lift_refused.borrow_mut().clear();
        self.wait_ram_turn();
    }

    /// Up to 5 s for the asks still waiting in the RAM turn: one asked just before mustn't land after Bloxkeep is gone.
    fn wait_ram_turn(&self) {
        let left = self.memory.ram_limit_turn.borrow().as_ref().map(|t| t.left.clone());
        if let Some(left) = left {
            let (lock, done) = &*left;
            let n = lock.lock().unwrap_or_else(|e| e.into_inner());
            let _ = done.wait_timeout_while(n, Duration::from_secs(5), |n| *n > 0);
        }
    }

    /// A RAM limit (`Some(mb)`) or a lift (None) asked of Windows in the RAM turn, remembered as held only once Windows
    /// said yes; a refusal leaves the record as it was, so the next pass asks again, and is said once per window in the
    /// log. One that closed meanwhile had nothing to change. `why` is the word for the log; empty: no line for a lift.
    pub fn ask_ram_limit(&self, pid: i32, limit: Option<i64>, why: &'static str, holding_mb: i64) {
        let serial = self.memory.ram_ask_serial.get() + 1;
        self.memory.ram_ask_serial.set(serial);
        {
            let mut asks = self.memory.ram_asks.borrow_mut();
            let ask = asks.entry(pid).or_insert_with(|| RamAsk { from: serial - 1, ..Default::default() });
            ask.asked = Some(limit);
            ask.serial = serial;
        }
        self.sync_ram_limited();
        let answer = self.limit_ram_in_turn(pid, limit);
        rt::spawn(async move {
            let ok = answer.await;
            App::with(|app| app.ram_limit_answered(pid, limit, serial, ok, why, holding_mb));
        });
    }

    fn ram_limit_answered(&self, pid: i32, limit: Option<i64>, serial: u64, ok: bool, why: &'static str, holding_mb: i64) {
        {
            let mut asks = self.memory.ram_asks.borrow_mut();
            // Answers come in the order asked, so a yes is what Windows has now; one to an ask from before the record was
            // made (the record dropped at a lock, say) says nothing of it.
            if let Some(ask) = asks.get_mut(&pid).filter(|a| serial > a.from) {
                if ok {
                    ask.held = limit;
                }
                ask.refused = !ok;
                if ask.serial == serial {
                    ask.asked = None;
                }
            }
        }
        self.sync_ram_limited();
        match (ok, limit) {
            (true, Some(mb)) => log::note(&format!("RAM limit: Roblox window {pid} limited to {mb} MB ({why}), holding {holding_mb} MB.")),
            (true, None) => {
                if !why.is_empty() {
                    log::note(&format!("RAM limit: lifted for Roblox window {pid} ({why})."));
                }
            }
            (false, _) if roblox_processes::ended(pid) => {}
            (false, Some(_)) => {
                if self.memory.ram_limit_refused.borrow_mut().insert(pid) {
                    log::note(&format!("RAM limit: Windows refused a limit for Roblox window {pid}."));
                }
            }
            (false, None) => {
                if self.memory.ram_lift_refused.borrow_mut().insert(pid) {
                    log::note(&format!("RAM limit: Windows refused to lift the limit of Roblox window {pid}."));
                }
            }
        }
    }

    /// Off the interface's thread and one at a time, in the order asked: Windows takes the RAM back before it answers, and
    /// five windows minimized together held the app still for 1.5 s. The ask is queued at once (so asks keep their order);
    /// the future only waits for its answer.
    pub fn in_ram_turn(&self, ask: impl FnOnce() -> bool + Send + 'static) -> impl Future<Output = bool> + 'static {
        let mut turn = self.memory.ram_limit_turn.borrow_mut();
        let turn = turn.get_or_insert_with(RamTurn::start);
        let (tx, rx) = oneshot::channel::<bool>();
        let left = turn.left.clone();
        *left.0.lock().unwrap_or_else(|e| e.into_inner()) += 1;
        let job: RamJob = Box::new(move || {
            // Counted down even when the ask panics: LiftRamLimits would wait its whole 5 s on exit for it.
            struct Done(Arc<(Mutex<usize>, Condvar)>);
            impl Drop for Done {
                fn drop(&mut self) {
                    let mut n = self.0.0.lock().unwrap_or_else(|e| e.into_inner());
                    *n = n.saturating_sub(1);
                    self.0.1.notify_all();
                }
            }
            let _done = Done(left);
            let answer = ask();
            let _ = tx.send(answer);
        });
        if turn.jobs.send(job).is_err() {
            // The worker is gone (it never stops on its own): nothing was asked.
            *turn.left.0.lock().unwrap_or_else(|e| e.into_inner()) -= 1;
        }
        async move { rx.await.unwrap_or(false) }
    }

    pub fn limit_ram_in_turn(&self, pid: i32, mb: Option<i64>) -> impl Future<Output = bool> + 'static {
        self.in_ram_turn(move || roblox_processes::limit_ram(pid, mb))
    }

    pub fn empty_ram_in_turn(&self, pid: i32, held_mb: i64) {
        let answer = self.in_ram_turn(move || roblox_processes::empty_ram(pid));
        rt::spawn(async move {
            if answer.await {
                log::note(&format!("Save RAM: Roblox window {pid} emptied once, it held {held_mb} MB."));
            }
        });
    }

    /// Auto clean RAM (clean_ram.rs), as Mem Reduct cleans by itself: above the RAM level picked, at most every
    /// `AUTO_CLEAN_GAP`, and every so many minutes if that is picked too.
    fn auto_clean(&self, memory: &MemoryStatus, now: Stamp) {
        let Ok((at, every, areas)) = self.vault(|d| (d.settings.auto_clean_at, d.settings.auto_clean_every, d.settings.clean_areas)) else { return };
        let c = &self.controller;
        if every <= 0 {
            c.auto_clean_since.set(stamp_min());
        } else if c.auto_clean_since.get() == stamp_min() {
            c.auto_clean_since.set(now);
        }
        let since_last = now.secs_since(c.last_auto_clean.get());
        let above = at > 0 && memory.load_percent >= at && since_last >= secs(AUTO_CLEAN_GAP);
        let timed = every > 0
            && now.secs_since(c.auto_clean_since.get()) >= secs(Duration::from_secs(every as u64 * 60))
            && since_last >= secs(Duration::from_secs(every as u64 * 60));
        if !(above || timed) || crate::clean_ram::running() {
            return;
        }
        c.last_auto_clean.set(now);
        let load = memory.load_percent;
        let got = crate::threads::call(move || crate::clean_ram::auto_clean(areas));
        rt::spawn(async move {
            let got = got.await.ok().and_then(|r| r.ok());
            App::with(|app| app.auto_cleaned(load, above, got));
        });
    }

    /// An auto clean is over: a toast when one above the level freed enough; both turned off when the task is gone
    /// (Windows would have to ask again, which only Settings does).
    fn auto_cleaned(&self, load: i32, above: bool, got: Option<crate::clean_ram::Cleaned>) {
        use crate::clean_ram::Cleaned;
        match got {
            Some(Cleaned::Freed(bytes)) => {
                let mb = bytes / MB as u64;
                log::note(&format!("Auto clean RAM ({}): RAM at {load}%, {mb} MB freed.", if above { "above the level" } else { "timed" }));
                if above && mb >= AUTO_CLEAN_SAID_MB {
                    self.toast(&l::f("RAM was at {0}%. Bloxkeep cleaned it and freed {1}.", &[&load, &crate::lang::cur().mb(mb as f64)]), "info");
                }
            }
            Some(Cleaned::Gone) => {
                log::note("Auto clean RAM: its task is gone; turned off.");
                let _ = self.vault_mut(|d| {
                    d.settings.auto_clean_at = 0;
                    d.settings.auto_clean_every = 0;
                });
                self.persist_soon();
                self.push_state();
                self.toast(&l::t("Auto clean RAM is off: its task in Task Scheduler is gone. Turn it on again in Settings › RAM & CPU."), "warn");
            }
            Some(Cleaned::Failed(why)) => log::note(&format!("Auto clean RAM failed: {why}.")),
            Some(Cleaned::Refused) | None => {}
        }
    }

    /// Every window Bloxkeep lowered gets its normal RAM priority back (the option turned off, Bloxkeep closing).
    pub fn give_memory_priority_back(&self) {
        let pids: Vec<i32> = self.memory.low_memory_priority.borrow_mut().drain().collect();
        for pid in pids {
            roblox_processes::set_memory_priority(pid, false);
        }
    }

    /// Every client's RAM limit lifted (Bloxkeep closing or locking: none is left limited without it). The record goes
    /// with it, and what was seen: the next pass after an unlock looks at every client afresh, so a lift Windows refused
    /// here is found by its limit and asked again.
    pub fn lift_ram_limits(&self) {
        let mut pids: HashSet<i32> = self.memory.ram_limited.borrow().keys().copied().collect();
        pids.extend(self.memory.ram_asks.borrow().iter().filter(|(_, a)| a.limited_to().is_some()).map(|(pid, _)| *pid));
        for pid in pids {
            self.ask_ram_limit(pid, None, "", 0);
        }
        self.memory.ram_asks.borrow_mut().clear();
        self.memory.ram_seen.borrow_mut().clear();
        self.memory.ram_limited.borrow_mut().clear();
        // One asked just before mustn't land after Bloxkeep is gone: up to 5 s for the asks still waiting.
        let left = self.memory.ram_limit_turn.borrow().as_ref().map(|t| t.left.clone());
        if let Some(left) = left {
            let (lock, done) = &*left;
            let n = lock.lock().unwrap_or_else(|e| e.into_inner());
            let _ = done.wait_timeout_while(n, Duration::from_secs(5), |n| *n > 0);
        }
    }
}
