// AppController.Memory.cs: the PC's memory watched: Roblox windows' RAM priority, resting windows' RAM emptied, and the
// RAM limit per window.
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    future::Future,
    sync::{Arc, Condvar, Mutex, mpsc},
    time::Duration,
};

use futures::channel::oneshot;

use super::App;
use super::controller::{COMMIT_MARGIN_MB, DEFAULT_CLIENT_COMMIT_MB, MB, MEMORY_WARNING_INTERVAL, MEMORY_WARNING_LOAD};
use super::presets::secs;
use crate::core::{
    base::{Guid, Stamp},
    l, log,
    models::stamp_min,
    roblox_processes::{self, PlayerProcess},
    rt,
    system_memory::MemoryStatus,
};

pub const EMPTY_RAM_RESTED: Duration = Duration::from_secs(60);

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
    /// Windows Windows refused a limit for, said once each in the log.
    pub ram_limit_refused: RefCell<HashSet<i32>>,
    /// Clients given a RAM limit, and the limit (-1: one found already limited, by a Bloxkeep that closed before lifting it).
    pub ram_limited: RefCell<HashMap<i32, i64>>,
    pub ram_limits_adopted: Cell<bool>,
    pub ram_limit_turn: RefCell<Option<RamTurn>>,
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
        if !self.memory.memory_priority_adopted.get() && !players.is_empty() {
            self.memory.memory_priority_adopted.set(true);
            let front = roblox_processes::foreground_pid();
            let mains = self.main_pids();
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

        // Only touch a window when its role changes (it went to the background, or came to the front).
        let foreground = roblox_processes::foreground_pid();
        let main = self.main_pids(); // the window played on is never Save RAM's
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
        // stuttered, a window grabbed to drag most of all (the owner's video, 2026-10-21). Resting ones: empty_resting_ram_once.
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
        // A window shown unseen for a moment (anti-AFK's nudge, /pc screenshot) goes on resting.
        if self.windows_in_turn() {
            return;
        }
        // PORT: `_vault!` threw when locked; the caller runs it only while unlocked.
        let Ok(trim_background) = self.vault(|d| d.settings.trim_background) else { return };
        if !trim_background {
            self.memory.resting_since.borrow_mut().clear();
            self.memory.ram_emptied.borrow_mut().clear();
            return;
        }
        let front = roblox_processes::foreground_pid();
        let main = self.main_pids();
        for p in players {
            let window = windows.get(&p.pid).map_or(0, |list| roblox_processes::main_window_of(list));
            if p.pid == front || main.contains(&p.pid) || window == 0 || !(roblox_processes::is_minimised(window) || roblox_processes::is_hidden(window)) {
                self.memory.resting_since.borrow_mut().remove(&p.pid);
                self.memory.ram_emptied.borrow_mut().remove(&p.pid);
                continue;
            }
            let added = {
                let mut resting = self.memory.resting_since.borrow_mut();
                if resting.contains_key(&p.pid) {
                    false
                } else {
                    resting.insert(p.pid, now);
                    true
                }
            };
            let since = self.memory.resting_since.borrow().get(&p.pid).copied().unwrap_or(now);
            if !added && now.secs_since(since) >= secs(EMPTY_RAM_RESTED) && self.memory.ram_emptied.borrow_mut().insert(p.pid) {
                self.empty_ram_in_turn(p.pid, p.working_set / MB);
            }
        }
    }

    /// RAM limit for resting windows: a Roblox window that isn't drawing (minimized, or hidden by Bloxkeep) keeps at most
    /// the chosen RAM (or Auto's, `RAM_LIMIT_AUTO`), the rest waiting in compressed memory or virtual memory, so more
    /// accounts fit on one PC (a resting game uses little of what it holds: about 1 of its 3 GB, measured 2026-10-29).
    /// Lifted once the window shows, comes to the front, or the option is off. Not for one in view: it still draws, and
    /// limited it took twice the CPU. Every limit set, changed and lifted is noted in the log (pid and MB only), so a
    /// window seen above its limit can be told apart from one that wasn't limited then (the owner saw 2 GB, 2026-10-30).
    pub fn limit_resting_ram(&self, players: &[PlayerProcess], windows: &HashMap<i32, Vec<isize>>, now: Stamp, memory: &MemoryStatus) {
        let alive = |pid: &i32| players.iter().any(|p| p.pid == *pid);
        self.memory.ram_limited.borrow_mut().retain(|pid, _| alive(pid));
        self.memory.auto_ram.borrow_mut().retain(|pid, _| alive(pid));
        self.memory.ram_limit_refused.borrow_mut().retain(|pid| alive(pid));
        if !self.memory.ram_limits_adopted.get() {
            self.memory.ram_limits_adopted.set(true);
            for p in players.iter().filter(|p| roblox_processes::has_ram_limit(p.pid)) {
                self.memory.ram_limited.borrow_mut().insert(p.pid, -1);
            }
        }
        // A window shown unseen for a moment (anti-AFK's nudge, /pc screenshot) is left as it is meanwhile.
        if self.windows_in_turn() {
            return;
        }
        // PORT: `_vault!` threw when locked; the caller runs it only while unlocked.
        let Ok(setting) = self.vault(|d| d.settings.ram_limit_mb as i64) else { return };
        if setting != RAM_LIMIT_AUTO as i64 {
            self.memory.auto_ram.borrow_mut().clear(); // back on Auto later, it waits and measures again
        }
        let front = roblox_processes::foreground_pid();
        let main = self.main_pids();
        for p in players {
            let window = windows.get(&p.pid).map_or(0, |list| roblox_processes::main_window_of(list));
            let resting = setting != 0
                && p.pid != front
                && !main.contains(&p.pid)
                && window != 0
                && (roblox_processes::is_minimised(window) || roblox_processes::is_hidden(window));
            let want: Option<i64> = if !resting {
                None
            } else if setting == RAM_LIMIT_AUTO as i64 {
                self.auto_limit(p, now, memory)
            } else {
                Some(setting)
            };
            let had = self.memory.ram_limited.borrow().get(&p.pid).copied();
            if !resting {
                self.memory.auto_ram.borrow_mut().remove(&p.pid);
            }
            match (want, had) {
                (Some(limit), had) if had != Some(limit) => {
                    self.memory.ram_limited.borrow_mut().insert(p.pid, limit);
                    self.set_ram_limit(p.pid, limit, if setting == RAM_LIMIT_AUTO as i64 { "auto" } else { "chosen" }, p.ram() / MB);
                }
                (None, Some(_)) => {
                    // C#: `_ = LimitRamInTurn(p.Pid, null)`: asked in turn, its answer not awaited.
                    drop(self.limit_ram_in_turn(p.pid, None));
                    self.memory.ram_limited.borrow_mut().remove(&p.pid);
                    log::note(&format!(
                        "RAM limit: lifted for Roblox window {} ({}).",
                        p.pid,
                        if resting { "auto left it alone" } else { "shown, in front, or the option off" }
                    ));
                }
                _ => {}
            }
        }
    }

    /// Off the interface's thread and one at a time, in the order asked: Windows takes the RAM back before it answers, and
    /// five windows minimized together held the app still for 1.5 s (the owner's log, 2026-11-01). The ask is queued at
    /// once (so asks keep their order); the future only waits for its answer.
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

    /// A refused limit is forgotten, so the next look asks again.
    pub fn set_ram_limit(&self, pid: i32, limit: i64, why: &'static str, holding_mb: i64) {
        let answer = self.limit_ram_in_turn(pid, Some(limit));
        rt::spawn(async move {
            let ok = answer.await;
            App::with(|app| {
                if ok {
                    log::note(&format!("RAM limit: Roblox window {pid} limited to {limit} MB ({why}), holding {holding_mb} MB."));
                } else {
                    let now = app.memory.ram_limited.borrow().get(&pid).copied();
                    if now == Some(limit) {
                        app.memory.ram_limited.borrow_mut().remove(&pid);
                    }
                    if app.memory.ram_limit_refused.borrow_mut().insert(pid) {
                        log::note(&format!("RAM limit: Windows refused a limit for Roblox window {pid}."));
                    }
                }
            });
        });
    }

    /// Auto's limit for a resting window now (MB), or None for none: not before it rested `AUTO_RAM_RESTED`, nor below
    /// `AUTO_RAM_LOAD` % of RAM used (a limit already set stays), nor while it is left alone after needing more twice. Its
    /// fault rates (median of the last five looks, so one garbage collection's burst is no reason) move it.
    pub fn auto_limit(&self, p: &PlayerProcess, now: Stamp, memory: &MemoryStatus) -> Option<i64> {
        let mut all = self.memory.auto_ram.borrow_mut();
        let a = all.entry(p.pid).or_insert_with(|| AutoRam {
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
        if p.page_faults > 0 && a.faults >= 0 && now > a.last_look {
            let seconds = now.secs_since(a.last_look);
            let (soft, hard) = (0.max(p.page_faults - a.faults) as f64 / seconds, 0.max(p.hard_faults - a.hard_faults) as f64 / seconds);
            // Faulting while it holds well under its limit: Windows took its RAM (another program needed more than the PC
            // has), which says nothing of the limit, and a higher one gives nothing back. Not counted, so not raised for it.
            let starved = soft > AUTO_RAM_STARVED || hard > AUTO_RAM_HARD_STARVED;
            if !starved || a.cap_mb == 0 || p.working_set >= a.cap_mb * MB * 9 / 10 {
                a.rates.push((soft, hard));
            }
            if a.rates.len() > 5 {
                a.rates.remove(0);
            }
        }
        (a.faults, a.hard_faults, a.last_look) = (p.page_faults, p.hard_faults, now);
        if let Some(until) = a.left_alone_until {
            if now < until {
                return None;
            }
            a.left_alone_until = None;
            a.raises = 0;
            a.cap_mb = 0;
        }
        if a.cap_mb == 0 {
            if now.secs_since(a.resting_since) < secs(AUTO_RAM_RESTED) || memory.load_percent < AUTO_RAM_LOAD {
                return None;
            }
            a.cap_mb = AUTO_RAM_FLOOR_MB.max(p.working_set / MB * 3 / 4 / 64 * 64); // what's shared counts towards a limit too
            a.changed_at = now;
            a.rates.clear();
            return Some(a.cap_mb);
        }
        if a.rates.len() >= 3 {
            let soft = median(a.rates.iter().map(|r| r.0));
            let hard = median(a.rates.iter().map(|r| r.1));
            if soft > AUTO_RAM_STARVED || hard > AUTO_RAM_HARD_STARVED {
                a.raises += 1;
                if a.raises >= 3 {
                    a.left_alone_until = Some(now.plus_secs(secs(AUTO_RAM_LEFT_ALONE)));
                    log::note(&format!(
                        "RAM limit: Roblox window {} needs its RAM ({soft:.0} faults/s, {hard:.0} from disk); left alone for {:.0} min.",
                        p.pid,
                        secs(AUTO_RAM_LEFT_ALONE) / 60.0
                    ));
                    return None;
                }
                a.cap_mb = a.cap_mb * 3 / 2 / 64 * 64;
                a.changed_at = now.plus_secs(secs(AUTO_RAM_RAISED_HOLD) - secs(AUTO_RAM_STEP)); // held a while before it may come down again
                a.rates.clear();
            } else if soft < AUTO_RAM_CALM
                && hard < 1.0
                && now.secs_since(a.changed_at) >= secs(AUTO_RAM_STEP)
                && a.cap_mb > AUTO_RAM_FLOOR_MB
                && memory.load_percent >= AUTO_RAM_LOAD
            {
                a.cap_mb = AUTO_RAM_FLOOR_MB.max(a.cap_mb * 3 / 4 / 64 * 64);
                a.changed_at = now;
                a.raises = 0; // calm again: an asset load an hour ago is no reason to give up on it
                a.rates.clear();
            }
        }
        Some(a.cap_mb)
    }

    /// Every window Bloxkeep lowered gets its normal RAM priority back (the option turned off, Bloxkeep closing).
    pub fn give_memory_priority_back(&self) {
        let pids: Vec<i32> = self.memory.low_memory_priority.borrow_mut().drain().collect();
        for pid in pids {
            roblox_processes::set_memory_priority(pid, false);
        }
    }

    /// Every client's RAM limit lifted (Bloxkeep closing: none is left limited without it).
    pub fn lift_ram_limits(&self) {
        let pids: Vec<i32> = self.memory.ram_limited.borrow().keys().copied().collect();
        for pid in pids {
            drop(self.limit_ram_in_turn(pid, None));
        }
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
