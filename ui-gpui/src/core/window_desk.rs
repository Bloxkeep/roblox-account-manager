//! The one place that acts on the show state and the place of Bloxkeep's Roblox windows for the rules that run on the
//! app's tick: "When a window joins a game" (minimise), Settings' Maximise, a rejoin's window going back where its
//! account's last one stood, and a window coming back from full screen at its size. Once a tick it reads each window
//! as Windows has it (one foreground read, the tick's window map), asks window_plan.rs what it should be, and carries
//! out the difference on a pool thread, inside the input turn, one window after another: each ask is waited for
//! (briefly) and what Windows then shows is what is remembered. A window lent to a borrower is left alone, it alone.
//!
//! The asks of one tick are taken in at the start of the next; while they are still under way the tick passes.
//!
//! An Arrange is the desk's too (`desk_arrange`): launching.rs says who takes which tile on which screen, and the desk
//! carries it out at once, in one job inside the input turn, every step read back, and remembers each window's tile
//! (window_plan.rs `Memory::tile`). A window it couldn't place then (lent, gone down meanwhile, sized back by Roblox)
//! takes its tile, or keeps it as where it opens, on a later tick.
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use futures::channel::oneshot;

use crate::core::{
    anti_afk,
    app::{
        App,
        presets::{AFK_PROFILE, MAXIMISE_WITHOUT_LOG},
    },
    base::{Guid, Res, Stamp},
    input_turn, live_monitor, log,
    roblox_processes as rp, rt,
    win_kit::{self, Rect, Size},
    window_loan,
    window_plan::{self, Facts, Game, Last, Memory, Seen, Show, Stage, Step, Wanted, in_tile},
    window_report,
};

/// How long one ask is waited for before it is taken as not come about (it is looked at again on the next ticks).
const ASK_WAIT: Duration = Duration::from_secs(1);

/// One thing asked of a window, in the order they are carried out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ask {
    /// Maximised back to an ordinary window (before it is placed).
    Unmaximise,
    Place(Rect),
    Minimise,
    /// Minimised: its place as an ordinary window for when it opens.
    OpensAt(Rect),
    /// Minimised: it opens maximised.
    OpensMaximised,
    Maximise,
    /// Into the tile an Arrange gave it, as Arrange places windows.
    Tile(Rect),
    /// Minimised: its tile is where it opens.
    TileOpensAt(Rect),
}

/// The asks a window's `Wanted` comes to, given how it is now.
pub fn asks_of(w: &Wanted, seen: &Seen) -> Vec<Ask> {
    let mut asks = Vec::new();
    if let Some(r) = w.place {
        if seen.maximised {
            asks.push(Ask::Unmaximise);
        }
        asks.push(if w.place_why == "tile" { Ask::Tile(r) } else { Ask::Place(r) });
    }
    if w.show == Some(Show::Minimise) {
        asks.push(Ask::Minimise);
    }
    if let Some(r) = w.opens_at {
        asks.push(if w.place_why == "tile" { Ask::TileOpensAt(r) } else { Ask::OpensAt(r) });
    }
    if w.opens_maximised {
        asks.push(Ask::OpensMaximised);
    }
    if w.show == Some(Show::Maximise) {
        asks.push(Ask::Maximise);
    }
    asks
}

/// One window's asks of a tick, and its plan (None: only its size kept, a window that may not be Bloxkeep's).
struct Job {
    pid: i32,
    window: isize,
    asks: Vec<Ask>,
    wanted: Option<Wanted>,
}

/// Asks sent off and not taken in yet. The answer: per job, per ask, whether Windows showed it; None when nothing was
/// asked (the input turn was someone else's, or an Arrange came in between).
struct Pending {
    answer: oneshot::Receiver<Option<Vec<Vec<bool>>>>,
    jobs: Vec<Job>,
    epoch: u64,
    now: Stamp,
}

/// The desk's memory, on the app's thread.
#[derive(Default)]
pub struct Desk {
    /// What was asked of each window and how far it got (by process id).
    memory: RefCell<HashMap<i32, Memory>>,
    pending: RefCell<Option<Pending>>,
    /// The tick passed last and the clients it had (the rules' entry points share one pass a tick).
    last_pass: RefCell<Option<(Stamp, Vec<i32>)>>,
    /// Counted up by every Arrange: asks planned before one aren't carried out after it.
    epoch: Arc<AtomicU64>,
}

fn on_a_screen(place: Rect) -> bool {
    win_kit::all_screens().iter().any(|s| {
        let a = s.working_area;
        a.x < place.x + place.width && place.x < a.x + a.width && a.y < place.y + place.height && place.y < a.y + a.height
    })
}

/// The window as Windows has it now (no message is sent to it).
fn seen_of(h: isize, front: bool) -> Seen {
    if !rp::exists(h) {
        return Seen::default();
    }
    let minimised = rp::is_minimised(h);
    Seen {
        exists: true,
        minimised,
        maximised: rp::is_maximised(h),
        hidden: rp::is_hidden(h),
        caption: rp::has_caption(h),
        rect: rp::window_rect(h),
        resting: if minimised { rp::resting_place(h) } else { None },
        opens_maximised: minimised && rp::opens_maximised(h),
        front,
        hung: rp::is_hung_window(h),
    }
}

fn game_of(state: Option<&crate::core::roblox_log_watcher::ClientState>) -> Game {
    match state {
        None => Game::Unknown,
        Some(s) if s.status == "ingame" => Game::InGame { at: s.at },
        Some(s) if s.status == "joining" => Game::Joining { at: s.at },
        Some(_) => Game::Out,
    }
}

/// Until `done` holds, `ASK_WAIT` at most.
fn wait_until(done: impl Fn() -> bool) -> bool {
    let until = Instant::now() + ASK_WAIT;
    loop {
        if done() {
            return true;
        }
        if Instant::now() >= until {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// One ask, and whether Windows then shows it. Blocking: on the desk's pool thread only.
fn carry(h: isize, ask: Ask) -> bool {
    match ask {
        Ask::Unmaximise => !rp::is_maximised(h) || (rp::unmaximise_quietly(h) && wait_until(|| !rp::is_maximised(h))),
        Ask::Place(r) => rp::place_window(h, r) && wait_until(|| rp::window_rect(h) == Some(r)),
        Ask::Minimise => rp::is_minimised(h) || (rp::minimise(h) && wait_until(|| rp::is_minimised(h))),
        Ask::OpensAt(r) => rp::resting_place(h) == Some(r) || (rp::restore_to(h, r) && rp::resting_place(h) == Some(r)),
        Ask::OpensMaximised => rp::is_minimised(h) && (rp::opens_maximised(h) || (rp::restore_maximised(h) && rp::opens_maximised(h))),
        Ask::Maximise => rp::is_maximised(h) || (rp::maximise(h) && wait_until(|| rp::is_maximised(h))),
        Ask::Tile(r) => rp::place_in_tile(h, r) && wait_until(|| !rp::is_minimised(h) && rp::window_rect(h).is_some_and(|now| in_tile(now, r))),
        Ask::TileOpensAt(r) => rests_in(h, r),
    }
}

/// A minimised window opens in `tile` (asked when it didn't already), grown by Roblox's unseen borders as an open one
/// stands in its tile. Blocking.
fn rests_in(h: isize, tile: Rect) -> bool {
    let kept = || rp::is_minimised(h) && rp::resting_place(h).is_some_and(|p| in_tile(p, tile));
    kept() || (rp::restore_to(h, rp::over_borders(tile)) && kept())
}

/// A window's asks in turn; one that didn't come about stops those that need it (a place after an unmaximise, what a
/// minimised window opens to after its minimise). A window gone, frozen, or lent meanwhile: none.
fn carry_all(h: isize, asks: &[Ask]) -> Vec<bool> {
    let mut held = Vec::with_capacity(asks.len());
    if !rp::exists(h) || window_loan::is_lent(h) || rp::is_hung_window(h) {
        held.resize(asks.len(), false);
        return held;
    }
    let mut open_ok = true;
    let mut down_ok = true;
    for ask in asks {
        let ok = match ask {
            Ask::Place(_) | Ask::Tile(_) if !open_ok => false,
            Ask::OpensAt(_) | Ask::TileOpensAt(_) | Ask::OpensMaximised if !down_ok => false,
            _ => carry(h, *ask),
        };
        match ask {
            Ask::Unmaximise => open_ok = ok,
            Ask::Minimise => down_ok = ok,
            _ => {}
        }
        held.push(ok);
    }
    held
}

/// What the log says when an ask came about (None: nothing worth a line).
fn held_line(pid: i32, ask: Ask, w: &Wanted) -> Option<String> {
    let late = format!(" (not in its game after {} s)", window_plan::GO_BACK_WITHOUT_GAME.as_secs());
    let size = |r: Rect| format!("{}×{}", r.width, r.height);
    Some(match ask {
        Ask::Minimise => match w.why {
            "rejoin" => format!("Rejoin place: window {pid} minimised again, as the account's last window was."),
            "rejoin-late" => format!("Rejoin place: window {pid} minimised again, as the account's last window was{late}."),
            why => format!("{}: window {pid} minimised, in its game.", window_plan::label(why)),
        },
        Ask::Maximise => format!(
            "Maximise: window {pid} maximised ({}).",
            if w.why == "ingame" { "in its game".to_string() } else { format!("up {} s", MAXIMISE_WITHOUT_LOG.as_secs()) }
        ),
        Ask::OpensMaximised => format!("Maximise: window {pid} left minimised; it opens maximised."),
        Ask::Place(r) => match w.place_why {
            "rejoin-again" => format!("Rejoin place: window {pid} moved again after going back; put back once more at {}.", size(r)),
            "rejoin-late" => format!("Rejoin place: window {pid} put back at {}, where the account's last window was{late}.", size(r)),
            _ => format!("Rejoin place: window {pid} put back at {}, where the account's last window was.", size(r)),
        },
        Ask::OpensAt(r) => format!("Rejoin place: window {pid} minimised; it opens again at {}, where the account's last window was.", size(r)),
        Ask::Tile(r) => format!("Arrange: window {pid} put in its tile ({}) after the Arrange.", size(r)),
        Ask::TileOpensAt(r) => format!("Arrange: window {pid} minimised; it opens in its tile ({}).", size(r)),
        Ask::Unmaximise => return None,
    })
}

/// What the log says when an ask didn't come about.
fn missed_line(pid: i32, ask: Ask, w: &Wanted) -> Option<String> {
    let (what, why) = match ask {
        Ask::Minimise => ("minimise", w.why),
        Ask::Maximise => ("maximise", w.why),
        Ask::OpensMaximised => ("keep opening maximised", "open"),
        Ask::Place(_) => ("take its place", w.place_why),
        Ask::OpensAt(_) => ("keep the place it opens at", w.place_why),
        Ask::Unmaximise => ("come out of maximised", w.place_why),
        Ask::Tile(_) => ("take its tile", w.place_why),
        Ask::TileOpensAt(_) => ("keep its tile as the place it opens at", w.place_why),
    };
    Some(format!("Window desk: window {pid} didn't {what} ({why}); looked at again on the next ticks."))
}

// ───────── Arrange ─────────

/// How the tiled windows come over other apps after an Arrange.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lift {
    /// Left in the PC's order.
    No,
    /// Over every other app, the keyboard left where it is (after a launch). Left in the PC's order, the tiles behind
    /// Bloxkeep or another app stayed hidden: 4 accounts opened showed 2 tiles ("2 in place; 45420 under Potassium;
    /// 45628 under Potassium", a real error.log). Not given the keyboard: whatever the person types in keeps it.
    Quietly,
    /// Over every other app, the last one taking the keyboard: clicked with Bloxkeep in view, or the hotkey. Stacked
    /// under Bloxkeep's window instead, they stayed under whatever covered it ("0 in place; 10904 under Discord").
    Keyboard,
}

/// An Arrange as decided (launching.rs `arrange_windows`): who takes which tile, on which screen.
#[derive(Clone, Debug)]
pub struct Arrangement {
    /// Every window it is about (process id, window), with a tile or not (hidden, minimised after a launch): for its log
    /// line.
    pub windows: Vec<(i32, isize)>,
    /// The tiles' windows in order; a window of 0 keeps its tile empty (a window still to come, or one not tiled now
    /// while a launch keeps every tile in its place).
    pub slots: Vec<(i32, isize)>,
    pub area: Rect,
    pub layout: String,
    pub custom: Size,
    /// Minimised windows to open first: a clicked or hotkey Arrange's. An automatic one opens none.
    pub open: Vec<isize>,
    /// Clicked or by the hotkey: the join rule then leaves its windows as the person has them.
    pub clicked: bool,
    pub lift: Lift,
}

/// What became of a tile's window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Took {
    /// An empty tile, or one whose window was hidden by the time the Arrange ran (a hidden window takes none).
    Empty,
    /// Left alone: lent to a borrower, not answering, or gone. It is given its tile on a later tick.
    Skipped,
    /// Open and seen in its place.
    Placed,
    /// Minimised (kept so, or gone down meanwhile): its tile is where it opens.
    Resting,
    /// Asked and not seen there: given it again on a later tick.
    Missed,
}

/// An Arrange carried out.
#[derive(Clone, Debug)]
pub struct Arranged {
    /// Windows with a tile.
    pub done: i32,
    /// Windows it was about.
    pub windows: usize,
    pub layout: String,
    /// Their states before ("3 normal, 1 maximized"; empty for none), what it did that the result doesn't show, and
    /// where they ended (window_report): its log line.
    pub before: String,
    pub note: String,
    pub placed: String,
    /// Each tile in order: process id, window, the place given (its tile, or its spot when the windows were laid over
    /// each other), and what became of it.
    pub took: Vec<(i32, isize, Rect, Took)>,
}

/// Who takes which tile. `order`: every window in tile order, (0, 0) keeping a tile for a window still to come. A
/// hidden window (Bloxkeep's own hide) takes none: it would be placed unseen, a gap among the others. After a launch
/// (`auto`) a minimised window takes none either, unless the join rule keeps it minimised (`resting`: its tile is then
/// where it opens), and none is opened; clicked or by the hotkey, the minimised ones are opened (the second list).
/// While tiles are kept for windows to come, every tile keeps its place and a window not tiled now leaves its tile
/// empty; otherwise the others close up. A lent window keeps its tile: it is placed once it is back.
pub fn choose_tiles(order: &[(i32, isize)], auto: bool, resting: &[i32]) -> (Vec<(i32, isize)>, Vec<isize>) {
    let planned = order.iter().any(|s| s.0 == 0);
    let slots: Vec<(i32, isize)> = order
        .iter()
        .map(|&(pid, h)| {
            let takes = h != 0 && rp::exists(h) && !rp::is_hidden(h) && !(auto && rp::is_minimised(h) && !resting.contains(&pid));
            (pid, if takes { h } else { 0 })
        })
        .filter(|&(_, h)| planned || h != 0)
        .collect();
    let open = if auto { Vec::new() } else { slots.iter().map(|s| s.1).filter(|&h| h != 0 && rp::is_minimised(h)).collect() };
    (slots, open)
}

/// An Arrange, one step after another, each read back: the windows asked to open come up (and maximised ones become
/// ordinary windows), the open ones go onto the screen and into their tiles, and each is then seen in its place or
/// found gone down meanwhile (the join rule or the person put it away): that one is left down, its place given as
/// where it opens, as for those that stayed minimised. Never opens a window it wasn't asked to. Blocking: on a pool
/// thread, inside the input turn.
fn lay_out(a: &Arrangement) -> Arranged {
    let mut note = String::new();
    let before = window_report::states(&a.windows.iter().map(|w| w.1).filter(|&h| h != 0).collect::<Vec<_>>());
    let n = a.slots.len();
    let h = |i: usize| a.slots[i].1;
    let grid = rp::tiles(n as i32, a.area, &a.layout, a.custom);
    let mut given: Vec<Rect> = grid.every.clone();
    let mut took: Vec<Took> = (0..n)
        .map(|i| {
            let w = h(i);
            if w == 0 || rp::is_hidden(w) {
                Took::Empty
            } else if !rp::exists(w) || window_loan::is_lent(w) || rp::is_hung_window(w) {
                Took::Skipped
            } else {
                Took::Missed
            }
        })
        .collect();
    let live: Vec<usize> = (0..n).filter(|&i| took[i] == Took::Missed).collect();
    // Up first, each restore finished before the place is given, or it undoes the new place (a fixed 250 ms wait was
    // too short for busy windows: of 6 Roblox windows on a real PC one went to the screen asked for).
    let (mut asked, mut opening) = (Vec::new(), 0);
    for &i in &live {
        let w = h(i);
        if rp::is_minimised(w) {
            if a.open.contains(&w) && rp::open_window(w) {
                asked.push(w);
                opening += 1;
            }
        } else if rp::is_maximised(w) {
            // Quietly after a launch: the keyboard stays where it is.
            let up = if a.clicked { rp::open_window(w) } else { rp::unmaximise_quietly(w) };
            if up {
                asked.push(w);
            }
        }
    }
    if !asked.is_empty() {
        rp::until_restored(&asked);
    }
    if opening > 0 {
        note.push_str(&format!("{opening} opened first; "));
    }
    // The open ones are placed; the minimised ones (kept so, or not opening) keep their tile as where they open.
    let open_now: Vec<usize> = live.iter().copied().filter(|&i| !rp::is_minimised(h(i))).collect();
    let windows: Vec<isize> = open_now.iter().map(|&i| h(i)).collect();
    rp::onto_screen_first(&windows, a.area, &mut note);
    let tiles: Vec<Rect> = open_now.iter().map(|&i| grid.every[i]).collect();
    for (k, place) in rp::place_tiles(&windows, &tiles, &grid, a.area, &mut note).into_iter().enumerate() {
        given[open_now[k]] = place;
    }
    let placed_or_down = |i: usize| rp::is_minimised(h(i)) || rp::window_rect(h(i)).is_some_and(|r| in_tile(r, given[i]));
    let until = Instant::now() + ASK_WAIT;
    while Instant::now() < until && !open_now.iter().all(|&i| placed_or_down(i)) {
        std::thread::sleep(Duration::from_millis(20));
    }
    let (mut resting, mut went_down, mut missed) = (0, 0, 0);
    for &i in &live {
        let w = h(i);
        took[i] = if !rp::exists(w) {
            Took::Skipped
        } else if rp::is_minimised(w) {
            if open_now.contains(&i) {
                went_down += 1;
            } else {
                resting += 1;
            }
            if rests_in(w, given[i]) { Took::Resting } else { Took::Missed }
        } else if !rp::is_maximised(w) && rp::window_rect(w).is_some_and(|r| in_tile(r, given[i])) {
            Took::Placed
        } else {
            Took::Missed
        };
        if took[i] == Took::Missed {
            missed += 1;
        }
    }
    if resting > 0 {
        note.push_str(&format!("{resting} kept minimised, each given its tile to open at; "));
    }
    if went_down > 0 {
        note.push_str(&format!("{went_down} went down meanwhile, left minimised with its tile to open at; "));
    }
    let skipped = took.iter().filter(|t| **t == Took::Skipped).count();
    if skipped > 0 {
        note.push_str(&format!("{skipped} left alone (lent or not answering), given their tiles later; "));
    }
    if missed > 0 {
        note.push_str(&format!("{missed} not in their tiles yet, given them again on the next ticks; "));
    }
    let up: Vec<isize> = windows.iter().copied().filter(|&w| rp::exists(w) && !rp::is_minimised(w)).collect();
    if !up.is_empty() {
        match a.lift {
            Lift::Quietly => {
                rp::lift_quietly(&up);
                note.push_str("lifted over other apps, the keyboard left where it was; ");
            }
            Lift::Keyboard => {
                rp::lift(&up);
                note.push_str("lifted over other apps; ");
            }
            Lift::No => {}
        }
    }
    Arranged {
        done: a.slots.iter().filter(|s| s.1 != 0).count() as i32,
        windows: a.windows.len(),
        layout: a.layout.clone(),
        before,
        note,
        placed: window_report::placed(&a.windows, a.area),
        took: (0..n).map(|i| (a.slots[i].0, a.slots[i].1, given[i], took[i])).collect(),
    }
}

impl App {
    /// One pass of the window desk for this tick (the tick's "WindowDesk" step). The old rules' entry points
    /// (`minimise_afk_windows`, `keep_rejoin_places`, `maximise_new_windows`) come here too: of the calls with the same
    /// moment and the same clients, the first runs it.
    pub fn window_desk(&self, now: Stamp, windows: &HashMap<i32, Vec<isize>>) {
        let desk = &self.presets.desk;
        let mut pids: Vec<i32> = windows.keys().copied().collect();
        pids.sort_unstable();
        if desk.last_pass.borrow().as_ref().is_some_and(|(at, had)| *at == now && *had == pids) {
            return;
        }
        *desk.last_pass.borrow_mut() = Some((now, pids));
        self.presets.afk_minimised.borrow_mut().retain(|pid| windows.contains_key(pid));
        self.presets.maximised.borrow_mut().retain(|pid, _| windows.contains_key(pid));
        desk.memory.borrow_mut().retain(|pid, _| windows.contains_key(pid));
        let alive: Vec<i32> = windows.keys().copied().collect();
        self.processes.window_sizes.borrow_mut().forget(&alive);
        if !self.desk_take_in() {
            return; // the last tick's asks still under way
        }
        // A nudge, a picture or a macro's turn under way: each puts its window back as it found it.
        if self.windows_in_turn() {
            return;
        }
        let front = rp::foreground_pid();
        let mut jobs: Vec<Job> = Vec::new();
        if self.vault_ref().is_some() {
            self.desk_plan(now, windows, front, &mut jobs);
        }
        self.desk_sizes(now, windows, &mut jobs);
        if jobs.is_empty() {
            return;
        }
        let (tell, answer) = oneshot::channel();
        let work: Vec<(isize, Vec<Ask>)> = jobs.iter().map(|j| (j.window, j.asks.clone())).collect();
        let epoch = desk.epoch.clone();
        let at = epoch.load(Ordering::SeqCst);
        crate::threads::run(move || {
            let turn = input_turn::try_enter("desk");
            let done = match &turn {
                Some(_) if epoch.load(Ordering::SeqCst) == at => Some(work.iter().map(|(h, asks)| carry_all(*h, asks)).collect()),
                _ => None,
            };
            // Answered before the turn is handed on, so whoever takes it next finds the answer there.
            let _ = tell.send(done);
            drop(turn);
        });
        *desk.pending.borrow_mut() = Some(Pending { answer, jobs, epoch: at, now });
    }

    /// Takes in the last tick's asks, if answered. False while they are still under way.
    fn desk_take_in(&self) -> bool {
        let desk = &self.presets.desk;
        let Some(mut pending) = desk.pending.borrow_mut().take() else { return true };
        match pending.answer.try_recv() {
            Ok(None) => {
                *desk.pending.borrow_mut() = Some(pending);
                false
            }
            Ok(Some(done)) => {
                if pending.epoch == desk.epoch.load(Ordering::SeqCst)
                    && let Some(done) = done
                {
                    self.desk_commit(pending.jobs, done, pending.now);
                }
                true
            }
            // The pool thread went away without an answer: nothing is known to have come about.
            Err(_) => true,
        }
    }

    /// Writes down what came about: a step asked becomes done only where Windows showed it.
    fn desk_commit(&self, jobs: Vec<Job>, done: Vec<Vec<bool>>, now: Stamp) {
        for (job, held) in jobs.into_iter().zip(done) {
            let Some(w) = job.wanted else { continue }; // a size kept: nothing to remember
            let mut m = w.memory;
            for (ask, ok) in job.asks.iter().zip(held) {
                let line = if ok { held_line(job.pid, *ask, &w) } else { missed_line(job.pid, *ask, &w) };
                if let Some(line) = line {
                    log::note(&line);
                }
                match (ask, ok) {
                    (Ask::Minimise, true) => m.put_away = Step::Done { at: now },
                    (Ask::Maximise | Ask::OpensMaximised, true) => m.maximised = Step::Done { at: now },
                    (Ask::Tile(_) | Ask::TileOpensAt(_), true) => m.tile_given = Step::Done { at: now },
                    _ => {}
                }
            }
            for note in &w.notes {
                log::note(note);
            }
            self.desk_remember(job.pid, m);
        }
    }

    fn desk_remember(&self, pid: i32, m: Memory) {
        let mut memory = self.presets.desk.memory.borrow_mut();
        let Some(slot) = memory.get_mut(&pid) else { return }; // ended meanwhile
        *slot = m;
        // (Kept for the parts that read or seed them: a session picked up again at unlock, a window adopted.)
        if m.put_away.settled() {
            self.presets.afk_minimised.borrow_mut().insert(pid);
        }
        if m.maximised.settled() && m.maximised_for != Stage::None {
            self.presets.maximised.borrow_mut().insert(pid, if m.maximised_for == Stage::InGame { "ingame" } else { "shown" }.into());
        }
    }

    /// Bloxkeep's own windows (the vault's accounts' clients): each one's facts, plan and asks.
    fn desk_plan(&self, now: Stamp, windows: &HashMap<i32, Vec<isize>>, front: i32, jobs: &mut Vec<Job>) {
        let desk = &self.presets.desk;
        let person_active = anti_afk::person_active();
        let started = self.relaunch.started.get();
        let main = self.main_pids();
        let runs: Vec<(Guid, i32, bool, Option<Guid>, Stamp)> =
            self.controller.running.borrow().iter().map(|(id, (pid, l))| (*id, *pid, l.rejoin, l.preset_id, l.started_at.get())).collect();
        for (id, pid, rejoin, preset_id, started_at) in runs {
            let Some(list) = windows.get(&pid).filter(|l| !l.is_empty()) else { continue };
            let h = rp::main_window_of(list);
            if h == 0 {
                continue;
            }
            let preset = self.preset_for(preset_id);
            let afk_profile = preset.as_ref().is_some_and(|p| p.profile == AFK_PROFILE);
            let (on_join, maximise_setting) = self.vault(|d| (App::on_join_for(preset.as_ref(), &d.settings), d.settings.maximize_on_open)).unwrap_or(("", false));
            let is_main = main.contains(&pid);
            let game = game_of(self.controller.logs.borrow().state_of(pid).as_ref());
            let mut memory = *desk.memory.borrow_mut().entry(pid).or_insert_with(|| Memory::new(now));
            // Seeds from outside the desk: a light-AFK window minimised before Bloxkeep closed (unlock), a window
            // opened elsewhere and adopted (Settings' Maximise leaves it as the person has it).
            if memory.put_away == Step::Never && self.presets.afk_minimised.borrow().contains(&pid) {
                memory.put_away = Step::Done { at: now };
            }
            if self.presets.maximised.borrow().get(&pid).is_some_and(|s| s == "ingame") && !(memory.maximised_for == Stage::InGame && memory.maximised.settled()) {
                memory.maximised_for = Stage::InGame;
                memory.maximised = Step::LeftUp("not opened by Bloxkeep");
            }
            let last = if rejoin && !memory.rejoin_judged {
                self.rejoin_place.last.borrow().get(&id).map(|l| Last { place: l.place.filter(|p| on_a_screen(*p)), minimised: l.minimised })
            } else {
                None
            };
            let facts = Facts {
                pid,
                now,
                main: is_main,
                afk_profile,
                rejoin,
                opened_this_run: started_at >= started,
                on_join_minimise: on_join == "minimise",
                maximise_on_open: window_plan::maximise_on_open(maximise_setting, preset.as_ref().and_then(|p| p.arrange), afk_profile, is_main),
                game,
                seen: seen_of(h, front == pid),
                person_active,
                lent: window_loan::is_lent(h),
                watched: live_monitor::watching(pid),
                sharp: live_monitor::sharp(pid),
                last,
                memory,
            };
            let wanted = window_plan::plan(&facts);
            self.desk_note(id, &facts, &wanted.memory);
            self.desk_take(h, &facts, wanted, jobs);
        }
        // Clients that aren't the accounts' (opened outside Bloxkeep): no rule of theirs runs, only a tile an Arrange
        // gave them and they aren't in yet.
        let ours: HashSet<i32> = self.controller.running.borrow().values().map(|r| r.0).collect();
        for (pid, list) in windows {
            if ours.contains(pid) {
                continue;
            }
            let Some(memory) = desk.memory.borrow().get(pid).copied().filter(|m| m.tile.is_some() && !m.tile_given.settled()) else { continue };
            let h = rp::main_window_of(list);
            if h == 0 {
                continue;
            }
            let facts = Facts {
                pid: *pid,
                now,
                main: false,
                afk_profile: false,
                rejoin: false,
                opened_this_run: false,
                on_join_minimise: false,
                maximise_on_open: false,
                game: Game::Unknown,
                seen: seen_of(h, front == *pid),
                person_active,
                lent: window_loan::is_lent(h),
                watched: live_monitor::watching(*pid),
                sharp: live_monitor::sharp(*pid),
                last: None,
                memory,
            };
            let wanted = window_plan::plan(&facts);
            self.desk_take(h, &facts, wanted, jobs);
        }
    }

    /// A window's plan for this tick: its asks to carry out, or what was decided without one remembered now.
    fn desk_take(&self, h: isize, f: &Facts, wanted: Wanted, jobs: &mut Vec<Job>) {
        if wanted.asks() {
            jobs.push(Job { pid: f.pid, window: h, asks: asks_of(&wanted, &f.seen), wanted: Some(wanted) });
        } else {
            for note in &wanted.notes {
                log::note(note);
            }
            self.desk_remember(f.pid, wanted.memory);
        }
    }

    /// Notes where and how the account's window stands, for its next rejoin: its place as an ordinary window (where it
    /// opens, while minimised), or none when maximised or full screen. Not while it is lent or made big for a live
    /// view, not a rejoin's window still going back (or not judged yet), not one Bloxkeep hid.
    fn desk_note(&self, id: Guid, f: &Facts, m: &Memory) {
        if !f.seen.exists || f.lent || f.sharp || f.seen.hidden || m.back.is_some() || (f.rejoin && !m.rejoin_judged) {
            return;
        }
        let mut last = self.rejoin_place.last.borrow_mut();
        if f.seen.minimised {
            last.insert(id, Last { place: f.seen.resting, minimised: true });
            return;
        }
        // (Still to be minimised again: that is what the account's window is.)
        let still_to_minimise = m.rejoin_minimise && !m.put_away.settled() && last.get(&id).is_some_and(|l| l.minimised);
        last.insert(id, Last { place: if f.seen.plain() { f.seen.rect } else { None }, minimised: still_to_minimise });
    }

    /// A window leaving full screen comes back at the size it had (every Roblox window, Bloxkeep's or not). Not one
    /// lent or made big for a live view: that size isn't the window's own.
    fn desk_sizes(&self, now: Stamp, windows: &HashMap<i32, Vec<isize>>, jobs: &mut Vec<Job>) {
        for (pid, list) in windows {
            if jobs.iter().any(|j| j.pid == *pid) {
                continue; // placed or shown by a rule this tick: its size is learned on the next
            }
            let h = rp::main_window_of(list);
            if h == 0 || window_loan::is_lent(h) || live_monitor::sharp(*pid) {
                continue;
            }
            let Some(rect) = rp::window_rect(h) else { continue };
            let back = self.processes.window_sizes.borrow_mut().look(*pid, rect, rp::is_fullscreen(h, rect), rp::is_plain_window(h), now);
            if let Some(back) = back {
                jobs.push(Job { pid: *pid, window: h, asks: vec![Ask::Place(back)], wanted: None });
            }
        }
    }

    /// An Arrange. `decide` says who takes which tile once the input turn is Bloxkeep's: after a nudge, a picture, a
    /// macro's turn or another Arrange under way (each puts its windows back first; two Arranges go one after the
    /// other, so no window is left between two tiles). It is then carried out at once, in one job on a pool thread, and
    /// each window's tile is remembered: one it couldn't place takes its tile, or keeps it as where it opens, on a later
    /// tick. The window-size keeper forgets the sizes it knew (the tiles are the windows' sizes now), rejoins' windows
    /// still going back stay in their new tiles, and the tick's asks planned before it and not carried out yet are
    /// dropped.
    pub async fn desk_arrange(self: &Rc<Self>, decide: impl FnOnce(&App) -> Arrangement) -> Res<Arranged> {
        let turn = rt::blocking(|| input_turn::enter("arrange", None)).await?;
        let a = decide(self);
        self.processes.window_sizes.borrow_mut().clear();
        let _ = self.desk_take_in();
        self.presets.desk.epoch.fetch_add(1, Ordering::SeqCst);
        let clicked = a.clicked;
        let laid = rt::blocking(move || lay_out(&a)).await;
        self.desk_tiled(&laid, clicked);
        drop(turn);
        Ok(laid)
    }

    /// An Arrange's tiles take the place of the last one's: each window it tiled remembers its tile and whether it was
    /// seen there; every other window has none now. Any Arrange ends rejoins' windows going back; a clicked or hotkey
    /// one leaves its windows to the person as far as the join rule goes (`window_plan::arranged`).
    fn desk_tiled(&self, laid: &Arranged, clicked: bool) {
        let desk = &self.presets.desk;
        // (The tick's clock, as the rest of the memory has it.)
        let now = desk.last_pass.borrow().as_ref().map(|p| p.0).unwrap_or_else(Stamp::utc_now);
        let mut memory = desk.memory.borrow_mut();
        for m in memory.values_mut() {
            window_plan::arranged(m, false);
            window_plan::given_tile(m, None, false, now);
        }
        for &(pid, h, place, took) in &laid.took {
            if pid == 0 {
                continue;
            }
            let m = memory.entry(pid).or_insert_with(|| Memory::new(now));
            window_plan::arranged(m, clicked);
            let seen = matches!(took, Took::Placed | Took::Resting);
            window_plan::given_tile(m, (h != 0 && took != Took::Empty).then_some(place), seen, now);
        }
    }

    /// What the desk remembers of a window (for the checks).
    pub(crate) fn desk_memory(&self, pid: i32) -> Option<Memory> {
        self.presets.desk.memory.borrow().get(&pid).copied()
    }

    /// A rejoin's window going back to its account's place.
    pub fn desk_going_back(&self, pid: i32) -> bool {
        self.presets.desk.memory.borrow().get(&pid).is_some_and(|m| m.back.is_some())
    }
}
