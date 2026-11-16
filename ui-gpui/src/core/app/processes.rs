//! The process check every second: Roblox's windows found, matched to the launches that opened them, and timed (the PC's
//! memory: memory.rs). (AppController.Processes.cs)
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    panic::AssertUnwindSafe,
    rc::Rc,
    time::{Duration, Instant},
};

use super::{
    App,
    controller::{COMMIT_MARGIN_MB, HAND_OVER_WINDOW, MB, PENDING_TIMEOUT, PendingLaunch, ROBLOX_VERSION_INTERVAL, TRAY_SWEEP, UPDATE_CHECK_INTERVAL},
    games::GAME_RETRY,
    launching::{LATE_TIMEOUT, add_launch, closing_has, get_by, has_launch, log_of, may_be_launch_of, min_stamp, pending, pending_count, running, running_pids, secs, set_by, set_seen_alive, vault_now},
};
use crate::core::{
    anti_afk,
    base::{Fail, Guid, Res, Stamp},
    game_target::GameTarget,
    log, multi_instance, prefs,
    roblox_input::RobloxInput,
    roblox_log_watcher::ClientState,
    roblox_game_settings, roblox_flags,
    roblox_processes::{self, PlayerProcess},
    rt,
    system_memory::{self, MemoryStatus},
    win_kit,
    window_size_keeper::WindowSizeKeeper,
};

/// How often Windows' process list is read while nothing of Roblox's runs and the window is in the tray.
pub const IDLE_SCAN_INTERVAL: Duration = Duration::from_secs(15);
/// The check runs on the core's thread, so Bloxkeep answers nothing meanwhile. When all is well it takes a few ms.
pub const SLOW_TICK: Duration = Duration::from_millis(100);
pub const TICK_ERROR_REPEAT: Duration = Duration::from_secs(60 * 60);
/// The main window shown but none of Bloxkeep's in front this long (left open behind the games): its figures are sent
/// at most every UNFOCUSED_METRICS_EVERY, and Roblox's CPU and GPU read every 15 s, not 3 (RobloxUsage).
pub const UNFOCUSED_AFTER: Duration = Duration::from_secs(10);
pub const UNFOCUSED_METRICS_EVERY: Duration = Duration::from_secs(5);
/// After this long with none of Bloxkeep's windows in front, the main page is asked to hold less memory
/// (set_page_light); in front again, it is back to normal at the next tick.
pub const PAGE_LIGHT_AFTER: Duration = Duration::from_secs(3 * 60);

pub struct ProcessesState {
    pub last_scan: Cell<Stamp>,
    pub last_scan_found_none: Cell<bool>,
    pub last_players: RefCell<Option<(Vec<PlayerProcess>, Stamp)>>,
    /// None between checks.
    pub tick_parts: RefCell<Option<Vec<(String, Duration)>>>,
    pub slow_tick: Cell<Option<(Stamp, Duration, i32)>>,
    pub tick_error: RefCell<Option<(String, Stamp, i32)>>,
    pub unfocused_since: Cell<Option<Stamp>>,
    pub metrics_sent_at: Cell<Stamp>,
    pub page_light: Cell<bool>,
    /// The main page asked to hold less memory (true) or as usual (false); the shell's, on the window's thread.
    pub set_page_light: RefCell<Option<Box<dyn Fn(bool)>>>,
    pub window_sizes: RefCell<WindowSizeKeeper>,
    /// The PlaceLauncher link a client was started with (roblox_processes::place_launcher_url_of); tests give their own.
    pub launch_url_of: RefCell<Option<Rc<dyn Fn(i32, i64) -> Option<String>>>>,
}

impl Default for ProcessesState {
    fn default() -> ProcessesState {
        ProcessesState {
            last_scan: Cell::new(min_stamp()),
            last_scan_found_none: Cell::new(false),
            last_players: RefCell::new(None),
            tick_parts: RefCell::new(None),
            slow_tick: Cell::new(None),
            tick_error: RefCell::new(None),
            unfocused_since: Cell::new(None),
            metrics_sent_at: Cell::new(min_stamp()),
            page_light: Cell::new(false),
            set_page_light: RefCell::new(None),
            window_sizes: RefCell::new(WindowSizeKeeper::new()),
            launch_url_of: RefCell::new(None),
        }
    }
}

/// A time moved back by a clock jump; one never set (the default) stays as it is.
pub fn moved_back(at: Stamp, back: Duration) -> Stamp {
    if at > min_stamp().plus_secs(secs(back)) { at.plus_secs(-secs(back)) } else { at }
}

/// The game an adopted window is rejoined into: only when its own start asked for a public server (a browser's Play:
/// PlaceLauncher's RequestGame or RequestGameJob, no private server code) of the place its log says it is in, and no
/// game was picked in the window since. Roblox's log doesn't say whether a server is private or reserved (none of the
/// owner's logs did, checked 2026-11-03), so one started on Roblox's home screen, from a private server, or moved to
/// another place gets none: a drop there isn't rejoined, rather than rejoined into a public server of that place.
pub fn public_target_of(launch_url: Option<&str>, state: Option<&ClientState>) -> Option<GameTarget> {
    let launch_url = launch_url.filter(|u| !u.is_empty())?;
    let state = state?;
    let place = state.place_id?;
    if state.chosen_place.is_some() || state.person_started {
        return None;
    }
    // Keys without regard to case, the first of each kept.
    let mut query: HashMap<String, String> = HashMap::new();
    let start = launch_url.find('?').map(|i| i + 1).unwrap_or(0);
    for pair in launch_url[start..].split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            query.entry(key.to_lowercase()).or_insert_with(|| value.to_string());
        }
    }
    let public_server = matches!(query.get("request").map(String::as_str), Some("RequestGame") | Some("RequestGameJob"))
        && !query.contains_key("accesscode")
        && !query.contains_key("linkcode")
        && !query.contains_key("privateserverlinkcode");
    // NumberStyles.None: digits only.
    let asked = query.get("placeid").filter(|v| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit())).and_then(|v| v.parse::<i64>().ok());
    if public_server && asked == Some(place) { Some(GameTarget { place_id: place, ..Default::default() }) } else { None }
}

/// Runs the job of a check, catching a panic as the C#'s catch did any exception.
fn guarded(job: impl FnOnce() -> Res<()>) -> Result<(), (String, String)> {
    match crate::core::program::catch_unwind(AssertUnwindSafe(job)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(f)) => Err((crate::core::roblox_api::fail_type_name(&f), f.message().to_string())),
        Err(panic) => {
            let text = panic.downcast_ref::<String>().cloned().or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
            Err(("panic".into(), text))
        }
    }
}

impl App {
    /// Always, unless in the tray with no client found, nothing opening, closing or waiting to rejoin, and the last
    /// read under 15 s old. A Roblox started by hand meanwhile is noticed at the next read.
    fn scan_due(&self, now: Stamp) -> bool {
        let last = self.processes.last_scan.get();
        !(self.controller.background.get()
            && self.processes.last_scan_found_none.get()
            && self.controller.running.borrow().is_empty()
            && pending_count(self) == 0
            && self.launching.late.borrow().is_empty()
            && self.controller.rejoin_queue.borrow().is_empty()
            && !self.controller.rejoining.get()
            && self.controller.launch_cts.borrow().is_none()
            && now >= self.controller.sweep_background_until.get()
            && self.controller.tray_sweep.borrow().is_empty()
            && self.rejoin.closing_for_rejoin.borrow().is_empty()
            && now >= last
            && now.secs_since(last) < secs(IDLE_SCAN_INTERVAL))
    }

    pub fn on_proc_timer(self: &Rc<Self>) {
        if self.controller.disposed.get() {
            return;
        }
        // Set up on the first tick, once the message loop runs: its window gets Windows' input messages.
        if !self.anti_afk.roblox_input_tried.get() {
            self.anti_afk.roblox_input_tried.set(true);
            // The C# caught Win32Exception and InvalidOperationException here (anti-AFK counts input as before);
            // RobloxInput::new doesn't fail: it reports `working() == false` instead.
            let input = RobloxInput::new();
            let working = input.working();
            *self.anti_afk.roblox_input.borrow_mut() = Some(input);
            if working {
                // PORT: C# kept the method group `_robloxInput.ReachedAt`; here a closure that reads the input held by the App.
                *self.anti_afk.roblox_input_at.borrow_mut() = Some(Box::new(|pid: i32| {
                    let mut at = None;
                    App::with(|app| at = app.anti_afk.roblox_input.borrow().as_ref().and_then(|i| i.reached_at(pid)));
                    at
                }));
            }
        }
        let start = Instant::now();
        *self.processes.tick_parts.borrow_mut() = Some(Vec::new());
        // A timer callback must never take the app down.
        if let Err((kind, message)) = guarded(|| {
            self.track_processes();
            Ok(())
        }) {
            self.log_tick_error(&kind, &message);
            // Only the first half (the process list, clients matched to launches) is left to fail the whole check: the save
            // still goes, but no rejoin, which would open windows nothing then matches. Already saved: nothing to do.
            if vault_now(self).is_some() {
                self.step("PersistIfDue(DateTime.UtcNow)", || {
                    self.persist_if_due(Stamp::utc_now());
                    Ok(())
                });
            }
        }
        let parts = self.processes.tick_parts.borrow_mut().take().unwrap_or_default();
        self.note_tick_time(start.elapsed(), parts);
    }

    /// The clock was set back: periodic work is due now, and deadlines and start times move back by as much.
    pub(crate) fn move_clock_back(&self, now: Stamp, back: Duration) {
        if self.persist.save_due.get().is_some() {
            self.persist.save_due.set(Some(now));
        }
        self.controller.next_presence.set(now);
        self.updates.next_roblox_version_check.set(now);
        self.updates.next_update_check.set(now);
        self.updates.next_log_clean.set(now);
        let c = &self.controller;
        c.sweep_background_until.set(moved_back(c.sweep_background_until.get(), back));
        c.resumed_at.set(moved_back(c.resumed_at.get(), back));
        self.relaunch.started.set(moved_back(self.relaunch.started.get(), back));
        c.last_session_start.set(moved_back(c.last_session_start.get(), back));
        c.roblox_settings_written_at.set(moved_back(c.roblox_settings_written_at.get(), back));
        let mut launches = pending(self);
        launches.extend(self.launching.late.borrow().iter().cloned());
        launches.extend(running(self).into_iter().map(|(_, r)| r.1));
        for launch in launches {
            launch.started_at.set(moved_back(launch.started_at.get(), back));
        }
        for until in c.tray_sweep.borrow_mut().values_mut() {
            *until = moved_back(*until, back);
        }
        for seen in c.pending_seen_alive.borrow_mut().values_mut() {
            *seen = moved_back(*seen, back);
        }
        for r in c.rejoin_queue.borrow_mut().iter_mut() {
            r.due = moved_back(r.due, back);
        }
        // Anti-AFK's: left ahead, a window's idle time read as none until the clock caught up again, and one set back
        // more than about 8 minutes got no nudge before Roblox's 20-minute kick (review, 2026-10-30). Roblox's log times
        // can't move: LastInput passes over one ahead of now.
        for at in self.anti_afk.last_input.borrow_mut().values_mut() {
            *at = moved_back(*at, back);
        }
        for at in self.anti_afk.nudge_retry_at.borrow_mut().values_mut() {
            *at = moved_back(*at, back);
        }
        self.health.sign_in_check_retry_at.set(moved_back(self.health.sign_in_check_retry_at.get(), back));
        self.move_discord_back(back);
        self.memory.auto_ram.borrow_mut().clear(); // starts over: its times would hold it back by as much
        let mut p = prefs::lock();
        if let Some(checked_at) = p.last_sign_in_check {
            p.last_sign_in_check = Some(moved_back(checked_at, back)); // written with the next prefs change; a restart clamps it anyway
        }
    }

    pub fn track_processes(self: &Rc<Self>) {
        let now = Stamp::utc_now();
        let last_tick = self.controller.last_tick.get();
        if now < last_tick {
            self.move_clock_back(now, Duration::from_secs_f64(last_tick.secs_since(now)));
        }
        self.note_check_gap(now, last_tick);
        self.controller.last_tick.set(now);
        self.step("NoteFocus(now)", || {
            self.note_focus(now);
            Ok(())
        });
        // The process list is the costly part of a tick (a few ms, whatever runs).
        let scan = self.scan_due(now);
        let mut part = Instant::now();
        let mut players: Vec<PlayerProcess> = if scan { roblox_processes::get_players() } else { Vec::new() };
        self.took("process list", part);
        let scanned: Vec<PlayerProcess> = players.clone();
        if scan {
            self.processes.last_scan.set(now);
        }
        let mut pids: HashSet<i32> = players.iter().map(|p| p.pid).collect();
        part = Instant::now();
        let windows: HashMap<i32, Vec<isize>> = roblox_processes::visible_windows(&pids.iter().copied().collect::<Vec<_>>());
        self.took("window list", part);
        let has_window = |pid: i32| windows.get(&pid).is_some_and(|list| !list.is_empty());
        let tree: RefCell<Option<HashMap<i32, Vec<i32>>>> = RefCell::new(None);
        let children_of = |parent: i32| -> Vec<i32> {
            let mut t = tree.borrow_mut();
            t.get_or_insert_with(|| roblox_processes::player_children(&scanned)).get(&parent).cloned().unwrap_or_default()
        };
        let parent_of = |child: i32| -> i32 {
            let mut t = tree.borrow_mut();
            t.get_or_insert_with(|| roblox_processes::player_children(&scanned)).iter().find(|(_, v)| v.contains(&child)).map(|(k, _)| *k).unwrap_or(0)
        };
        // Roblox's copy in the notification area (roblox_processes::is_tray_copy) is not a client. Once a window of ours
        // closed, or after Close all, it is ended: it would take over the next launch and open nothing.
        part = Instant::now();
        let trays: HashSet<i32> = players.iter().filter(|p| !has_window(p.pid) && roblox_processes::is_tray_copy(p.pid, p.created)).map(|p| p.pid).collect();
        let sweeping = now < self.controller.sweep_background_until.get() || self.controller.tray_sweep.borrow().values().any(|until| now <= *until);
        if sweeping {
            for &pid in &trays {
                roblox_processes::kill(pid);
                self.forget_client_start(pid);
            }
        }
        pids.retain(|p| !trays.contains(p));
        players.retain(|p| !trays.contains(&p.pid));
        // Raw input only while a Roblox window may need it (anti-AFK, macros, the recorder): none running, none taken.
        if scan && let Some(input) = self.anti_afk.roblox_input.borrow_mut().as_mut() {
            input.listen(!pids.is_empty());
        }
        self.release_home_session_if_idle(scan, &scanned, &trays, now);
        self.note_client_starts(&pids, now, &parent_of);
        // The browser tracker id on a client's command line says which launch it is (see claim_windows).
        let created_of: HashMap<i32, i64> = scanned.iter().map(|p| (p.pid, p.created)).collect();
        let tracker_of = |pid: i32| roblox_processes::launch_tracker_of(pid, created_of.get(&pid).copied().unwrap_or(0));
        let mut changed = false;

        for (id, info) in running(self) {
            if pids.contains(&info.0) {
                continue;
            }
            self.controller.running.borrow_mut().shift_remove(&id);
            // A scheduled stop closing it is done: another preset with this account opened later doesn't wait 20 s for it
            // (2026-10-30 review: only the same preset's next run cleared it).
            let stopping = self.schedule.stop_closing.borrow_mut().remove(&id);
            self.persist_soon(); // the open clients on disk (see persist)
            // Read before the log watcher drops the pid below, with the log's last lines (a leave logged right before quitting).
            let mut log = self.ended_state(info.0);
            // Roblox's own words on why it ended (roblox_words.rs), unless Bloxkeep closed it: the person's Close, a
            // schedule's stop, a restart, or a rejoin of a drop it already wrote the reason of.
            if !stopping
                && !self.controller.user_closed.borrow().contains(&id)
                && !self.presets.restarts.borrow().contains_key(&id)
                && !self.rejoin.closing_for_rejoin.borrow().contains(&info.0)
            {
                let last = log.as_ref().map(|l| match l.reason {
                    Some(code) => format!("{}, code {code}", l.status),
                    None => l.status.clone(),
                });
                self.note_roblox_lines(info.0, &format!("ended, last state {}", last.as_deref().unwrap_or("none")));
            }
            if self.controller.logs.borrow().out_of_memory_at(info.0).is_some() {
                self.rejoin.ran_out_of_memory.borrow_mut().insert(id);
            }
            // Closed by Roblox because another client opened: no hand-over, no auto-rejoin. Only a window that closed in
            // order can be that: its log says it shut down, or it left its notification-area copy behind.
            let in_order = self.controller.logs.borrow().closed_in_order_at(info.0).is_some() || children_of(info.0).iter().any(|c| trays.contains(c));
            let closed_by_roblox = self.check_closed_by_roblox(id, info.0, info.1.started_at.get(), now, in_order, &children_of);
            // No log, but a notification-area copy left behind: Roblox starts that only when its window is closed (X or
            // Bloxkeep's close), never on a drop or crash. So a leave by choice, not a drop to rejoin.
            if log.is_none()
                && !closed_by_roblox
                && !self.rejoin.closing_for_rejoin.borrow().contains(&info.0)
                && children_of(info.0).iter().any(|c| trays.contains(c))
            {
                let t = info.1.target.as_ref();
                log = Some(ClientState::new("left", t.map(|t| t.place_id), t.and_then(|t| t.job_id.clone()), None, now));
            }
            let ended_job = log.as_ref().and_then(|l| l.job_id.clone()).or(info.1.target.as_ref().and_then(|t| t.job_id.clone()));
            let ended_place = log.as_ref().and_then(|l| l.place_id).or(info.1.target.as_ref().map(|t| t.place_id));
            self.state.ended_here.borrow_mut().insert(id, (ended_job, ended_place, now));
            // Hidden: the window auto-rejoin opens for it is hidden too, once in its game (rejoin_from_logs).
            if roblox_processes::forget_hidden(info.0) {
                self.rejoin.hide_when_in.borrow_mut().insert(id);
            }
            self.end_play(id, log.as_ref(), now);
            self.refresh_presence_soon(Duration::from_secs(8));
            // The first process may only bootstrap and hand over to a child: wait for that only when a new, unclaimed
            // client runs. One whose log shows a join was the real one: its windowless child is the tray copy.
            let claimed_now = running_pids(self);
            let tray_copies: Vec<i32> = if log.is_none() { Vec::new() } else { children_of(info.0).into_iter().filter(|p| !has_window(*p)).collect() };
            if log.is_some() {
                self.controller.tray_sweep.borrow_mut().insert(info.0, now.plus_secs(secs(TRAY_SWEEP)));
            }
            // Roblox's updater started since the launch is one too: an outdated client hands the launch to it.
            let hand_over = now.secs_since(info.1.started_at.get()) < secs(HAND_OVER_WINDOW)
                && !self.controller.user_closed.borrow().contains(&id)
                && !closed_by_roblox
                && (pids.iter().any(|p| {
                    !info.1.before.contains(p) && !claimed_now.contains(p) && !tray_copies.contains(p) && may_be_launch_of(tracker_of(*p).as_deref(), &info.1)
                }) || (self.may_have_handed_to_updater(info.0) && roblox_processes::updater_running_since(info.1.started_at.get())));
            if hand_over {
                self.controller.pending.borrow_mut().push(info.1.clone());
                set_seen_alive(self, &info.1, now);
                set_by(&mut self.launching.handed_over.borrow_mut(), &info.1, (info.0, log));
            } else {
                // Ended on its own (no leave, disconnect or sign-out in its log, not closed by anyone): what it left behind
                // goes to error.log, off this thread. A disconnect is skipped: a server's shutdown ends many at once.
                if !stopping
                    && !in_order
                    && !closed_by_roblox
                    && !self.controller.user_closed.borrow().contains(&id)
                    && !self.presets.restarts.borrow().contains_key(&id)
                    && !self.rejoin.closing_for_rejoin.borrow().contains(&info.0)
                    && !log.as_ref().is_some_and(|l| matches!(l.status.as_str(), "left" | "disconnected" | "signedout"))
                {
                    let pid = info.0;
                    let ran_for = Duration::from_secs_f64(now.secs_since(info.1.started_at.get()).max(0.0));
                    let last = log.as_ref().map(|l| l.status.clone()).unwrap_or_else(|| "no log".into());
                    std::thread::spawn(move || {
                        let found = crate::core::roblox_crash_info::describe(pid, ran_for);
                        log::note(&format!(
                            "Window {pid} ended on its own (last log: {last}): {}.",
                            found.as_deref().unwrap_or("no crash report, no Windows error record")
                        ));
                    });
                }
                self.on_session_ended(id, &info.1, log);
            }
            changed = true;
        }

        let updating = |launch: &PendingLaunch| roblox_processes::updater_running_since(launch.started_at.get());
        changed |= self.expire_pending(now, &updating);
        changed |= self.claim_windows(&pids, &has_window, &children_of, &tracker_of, &updating, now);
        {
            let still = pending(self);
            self.controller.pending_seen_alive.borrow_mut().retain(|l, _| has_launch(&still, &l.0));
        }

        // The tray copy left by a closed client: ended while windowless, never one a launch could be waiting for.
        let sweeps: Vec<(i32, Stamp)> = self.controller.tray_sweep.borrow().iter().map(|(k, v)| (*k, *v)).collect();
        for (parent, until) in sweeps {
            if now > until {
                self.controller.tray_sweep.borrow_mut().remove(&parent);
                continue;
            }
            let tracked = running_pids(self);
            for child in children_of(parent) {
                if !pids.contains(&child) || has_window(child) || tracked.contains(&child) || pending(self).iter().any(|l| !l.before.contains(&child)) {
                    continue;
                }
                roblox_processes::kill(child);
                pids.remove(&child);
                self.forget_client_start(child);
            }
        }
        self.took("clients started, ended and matched to launches", part);
        players.retain(|p| pids.contains(&p.pid));
        if scan {
            self.step("SplitCores(pids)", || {
                self.split_cores(&pids);
                Ok(())
            });
        }
        if scan {
            self.processes.last_scan_found_none.set(pids.is_empty());
            *self.processes.last_players.borrow_mut() = Some((players.clone(), now)); // the monitor tab's tiles use it (monitor_clients)
        }

        self.rejoin.closing_for_rejoin.borrow_mut().retain(|pid| pids.contains(pid));
        // From here on each job on its own (step): one that throws (bad data in one session or macro) would otherwise
        // stop the save, auto-rejoin, schedules and auto-lock for as long as it kept throwing (audit R1).
        // Once a client has started (its window is up), the file goes back to the person's own settings.
        let quiet = self.controller.launch_cts.borrow().is_none()
            && pending_count(self) == 0
            && !self.controller.rejoining.get()
            && now.secs_since(self.controller.roblox_settings_written_at.get()) > 10.0;
        if self.controller.roblox_settings_pending.get() && quiet {
            self.step("RobloxGameSettings.Restore(final)", || {
                let last = pids.is_empty();
                roblox_game_settings::restore(last);
                if last {
                    self.controller.roblox_settings_pending.set(false);
                }
                Ok(())
            });
        }
        // Light AFK's flags: read by the windows they were for, so the file is the person's own again.
        if self.controller.roblox_flags_pending.get() && quiet {
            self.step("RobloxFlags.Restore()", || {
                roblox_flags::restore();
                self.controller.roblox_flags_pending.set(false);
                Ok(())
            });
        }
        // Every client's log: the ones Bloxkeep opened for their status, all of them for anti-AFK.
        self.step("Roblox logs", || {
            let list: Vec<i32> = pids.iter().copied().collect();
            let polled = self.controller.logs.borrow_mut().poll(&list);
            changed |= polled && !self.controller.running.borrow().is_empty();
            Ok(())
        });
        if scan {
            self.step("AdoptWindowsOpenedElsewhere(players)", || {
                changed |= self.adopt_windows_opened_elsewhere(&players);
                Ok(())
            });
        }
        self.step("NameLoggedPlacesAsync(unnamed)", || {
            let mut unnamed: Vec<i64> = Vec::new();
            for (_, r) in running(self) {
                let id = log_of(self, r.0).and_then(|l| l.place_id).unwrap_or(0);
                let asked_lately = self.controller.game_fetched_at.borrow().get(&id).is_some_and(|asked| now.secs_since(*asked) < secs(GAME_RETRY));
                if id > 0 && !self.has_game(id, false) && !asked_lately && !unnamed.contains(&id) {
                    unnamed.push(id);
                }
            }
            if !unnamed.is_empty() {
                let app = self.clone();
                rt::spawn(async move {
                    let _ = app.name_logged_places(unnamed).await;
                });
            }
            Ok(())
        });
        self.step("RejoinFromLogs", || {
            self.rejoin_from_logs();
            Ok(())
        });
        self.step("ForgetLiftedGameBans", || {
            self.forget_lifted_game_bans();
            Ok(())
        });
        self.step("ForgetSettledRejoins(now)", || {
            self.forget_settled_rejoins(now);
            Ok(())
        });
        self.step("TrackPlay(now)", || {
            self.track_play(now);
            Ok(())
        });
        self.step("RunAntiAfk(now, windows)", || {
            self.run_anti_afk(now, &windows);
            Ok(())
        });
        self.step("MinimiseAfkWindows(windows)", || {
            self.minimise_afk_windows(&windows);
            Ok(())
        });
        // Before Maximise: a rejoin going back isn't maximised.
        self.step("KeepRejoinPlaces(now, windows)", || {
            self.keep_rejoin_places(now, &windows);
            Ok(())
        });
        self.step("MaximiseNewWindows(now, windows)", || {
            self.maximise_new_windows(now, &windows);
            Ok(())
        });
        self.step("KeepWindowSizes(now, windows)", || {
            self.keep_window_sizes(now, &windows);
            Ok(())
        });
        self.step("TickMacros(now)", || {
            self.tick_macros(now);
            Ok(())
        });
        self.step("TickPanels()", || {
            self.tick_panels();
            Ok(())
        });

        if vault_now(self).is_some() && self.vault(|d| d.settings.rename_windows).unwrap_or(false) {
            self.step("window titles", || {
                for (id, info) in running(self) {
                    // With its process id, so a window can be matched to Task Manager and to its Roblox log.
                    // (This tick's window list, as the C# did: not a desktop walk of its own for each client.)
                    if let Some(account) = self.find_account(id)
                        && let Some(list) = windows.get(&info.0).filter(|list| !list.is_empty())
                    {
                        roblox_processes::set_windows_title(list, &format!("Roblox - {} (PID {})", self.name_text(&account), info.0));
                    }
                }
                Ok(())
            });
        }
        if scan {
            self.step("ApplyWindowIcons(pids, windows)", || {
                self.apply_window_icons(&pids, &windows);
                Ok(())
            });
            self.step("ApplyTaskbarGroups(pids, windows)", || {
                self.apply_taskbar_groups(&pids, &windows);
                Ok(())
            });
        }

        if vault_now(self).is_some() && self.vault(|d| d.settings.multi_instance).unwrap_or(false) && scan && pids.is_empty() {
            self.step("MultiInstance.Enable()", || {
                self.apply_watch_names();
                if !multi_instance::armed() || multi_instance::stale() {
                    multi_instance::enable();
                }
                Ok(())
            });
        }
        if multi_instance::armed() != self.controller.last_armed.get() {
            self.controller.last_armed.set(multi_instance::armed());
            changed = true;
        }
        if changed {
            self.step("PushStateIfChanged", || {
                self.push_state_if_changed();
                Ok(())
            });
        }

        let active = !pids.is_empty() || pending_count(self) > 0 || !self.controller.rejoin_queue.borrow().is_empty() || now < self.controller.sweep_background_until.get();
        let interval: u64 = if self.controller.background.get() {
            5000
        } else if active {
            1500
        } else {
            4000
        };
        // PORT: the WinForms timer's Interval; the core's tick loop reads this before each wait.
        if self.controller.proc_interval_ms.get() != interval {
            self.controller.proc_interval_ms.set(interval);
        }

        if now >= self.updates.next_log_clean.get() {
            self.updates.next_log_clean.set(now.plus_secs(24.0 * 3600.0));
            std::thread::spawn(|| {
                let removed = crate::core::roblox_log_watcher::clean_old_logs(std::time::SystemTime::now());
                if removed > 0 {
                    log::note(&format!("Roblox logs: {removed} older than {} days deleted.", crate::core::roblox_log_watcher::KEEP_LOGS.as_secs() / 86400));
                }
            });
        }
        if now >= self.updates.next_roblox_version_check.get() && !self.controller.background.get() {
            self.updates.next_roblox_version_check.set(now.plus_secs(secs(ROBLOX_VERSION_INTERVAL)));
            rt::spawn({
                let app = self.clone();
                async move {
                    let _ = app.refresh_latest_roblox().await;
                }
            });
            rt::spawn({
                let app = self.clone();
                async move {
                    let _ = app.refresh_roblox_watch().await;
                }
            });
        }
        // Independent jobs: one that fails doesn't stop the rest, least of all the save and auto-rejoin.
        self.step("TickRobloxUpdates(now, ingame)", || {
            let in_game = pids.iter().filter(|pid| log_of(self, **pid).is_some_and(|l| l.status == "ingame")).count() as i32;
            self.tick_roblox_updates(now, in_game);
            Ok(())
        });

        let updater = self.updates.updater.borrow().clone();
        if prefs::lock().check_updates && updater.supported() && now >= self.updates.next_update_check.get() {
            self.updates.next_update_check.set(now.plus_secs(secs(UPDATE_CHECK_INTERVAL)));
            let u = updater.clone();
            rt::spawn(async move {
                rt::blocking(move || u.check()).await;
            });
        }
        self.step("TickLatest(now)", || {
            self.tick_latest(now); // a new version, from Bloxkeep's bot: the check above brought forward
            Ok(())
        });
        self.step("RelaunchIfQuiet(now)", || {
            self.relaunch_if_quiet(now); // a downloaded update, installed once nobody is using the app
            Ok(())
        });
        self.step("SayUpdateFailed", || {
            self.say_update_failed(); // one that didn't install, said once something can show it
            Ok(())
        });

        self.step("CheckSchedules", || {
            self.check_schedules(); // also while locked: a run due then is reported as missed
            Ok(())
        });
        self.step("TickDiscordPresence(now)", || {
            self.tick_discord_presence(now); // Settings › Discord › Show on my Discord profile, locked or not
            Ok(())
        });
        self.step("TickOnline(now)", || {
            self.tick_online(now); // Settings › Bloxkeep › Add me to the online count, locked or not
            Ok(())
        });
        self.step("TickRemote", || {
            self.tick_remote(); // Settings › Discord › Control from Discord, locked or not (locked, it says only that)
            Ok(())
        });
        self.step("TickFeeds", || {
            self.tick_feeds(); // the Game news tab, locked or not (nothing personal comes or goes)
            Ok(())
        });
        // Also while locked: a window that ends soon after the commit limit ran out ran out of memory.
        let mut memory = MemoryStatus::default();
        self.step("SystemMemory.Read()", || {
            memory = system_memory::read();
            if memory.commit_limit_mb > 0 && memory.commit_room_mb() < COMMIT_MARGIN_MB {
                self.roblox_updates.memory_short_at.set(Some(now));
            }
            Ok(())
        });
        if vault_now(self).is_none() {
            return;
        }
        self.step("CheckAutoLock(AntiAfk.IdleTime(), DateTime.UtcNow)", || {
            self.check_auto_lock(anti_afk::idle_time(), Stamp::utc_now());
            Ok(())
        });
        self.step("PersistIfDue(now)", || {
            self.persist_if_due(now);
            Ok(())
        });
        self.step("EmitMetrics, GuardResources", || {
            self.note_client_memory(&players);
            self.emit_metrics(&players, &windows, &memory)?;
            self.guard_resources(&players, &memory, now);
            self.limit_resting_ram(&players, &windows, now, &memory);
            self.empty_resting_ram_once(&players, &windows, now);
            Ok(())
        });
        if scan {
            self.step("NoteClientBuilds, AutoUpdateRoblox", || {
                self.note_client_builds(&players);
                self.auto_update_roblox(&players);
                Ok(())
            });
        }
        self.step("TickUpdateReopen(now)", || {
            self.tick_update_reopen(now);
            Ok(())
        });
        self.step("RunDueRejoin(now)", || {
            self.run_due_rejoin(now);
            Ok(())
        });
        self.step("RestartDueClients(now)", || {
            self.restart_due_clients(now);
            Ok(())
        });
        self.step("HopCrowdedClients(now)", || {
            self.hop_crowded_clients(now);
            Ok(())
        });
        self.step("RunSignInCheckIfDue(now)", || {
            self.run_sign_in_check_if_due(now);
            Ok(())
        });
        self.step("NoteGameHoursIfDue(now)", || {
            self.note_game_hours_if_due(now);
            Ok(())
        });
        self.step("SaveRenewedSignIns", || {
            self.save_renewed_sign_ins(); // a cookie Roblox renewed in an answer goes to its account
            Ok(())
        });
        self.step("RefreshRobuxIfDue(now)", || {
            self.refresh_robux_if_due(now);
            Ok(())
        });
        self.step("TickDiscord(now, players)", || {
            self.tick_discord(now, &players);
            Ok(())
        });
        if !self.controller.presence_busy.get() && now >= self.controller.next_presence.get() {
            let app = self.clone();
            rt::spawn(async move {
                let _ = app.refresh_presence().await;
            });
        }
    }

    /// Its log says it handed its launch to Roblox's updater, or isn't followed (a client that quits within two seconds
    /// may go before its log is found). One updating in the background went on into its game: the updater opens nothing
    /// for it, so its end is its own, whatever else is updating (the owner's logs, 2026-10-30).
    pub(crate) fn may_have_handed_to_updater(&self, pid: i32) -> bool {
        let logs = self.controller.logs.borrow();
        !logs.follows(pid) || logs.update_hand_off_at(pid).is_some()
    }

    /// Launches whose window didn't come within PENDING_TIMEOUT stop waiting.
    /// `updating`: see roblox_processes::updater_running_since.
    pub(crate) fn expire_pending(&self, now: Stamp, updating: &dyn Fn(&PendingLaunch) -> bool) -> bool {
        let mut changed = false;
        let expired: Vec<Rc<PendingLaunch>> = pending(self).into_iter().filter(|p| now.secs_since(p.started_at.get()) > secs(PENDING_TIMEOUT)).collect();
        for launch in expired {
            let handed = get_by(&self.launching.handed_over.borrow(), &launch).is_some();
            let closing = closing_has(self, &launch);
            // Handed to Roblox's updater, still at it (a slow link): the updated client is yet to come. Dropped at 2 min, the
            // launch was rejoined at once and a second outdated client opened for the account while the first update ran,
            // and the updated window was then nobody's (review, 2026-10-30). LATE_TIMEOUT more at most.
            if handed && !closing && now.secs_since(launch.started_at.get()) <= secs(PENDING_TIMEOUT) + secs(LATE_TIMEOUT) && updating(&launch) {
                continue;
            }
            // No window yet (Roblox updating on a slow link, its launcher stuck): one that comes later is still this
            // launch's, not the next account's (see `late`).
            let wait_longer = !closing && !handed;
            self.drop_pending(&launch, wait_longer);
            if wait_longer {
                let mut late = self.launching.late.borrow_mut();
                late.retain(|l| l.account_id != launch.account_id);
                add_launch(&mut late, &launch);
            }
            changed = true;
        }
        changed
    }

    /// A failure is logged (log_tick_error) and the next job runs; timed and named by its code.
    pub fn step(&self, what: &str, job: impl FnOnce() -> Res<()>) {
        let start = Instant::now();
        if let Err((kind, message)) = guarded(job) {
            self.log_tick_error(&kind, &message);
        }
        self.took(what, start);
    }

    /// `since`: an Instant.
    pub fn took(&self, what: &str, since: Instant) {
        if let Some(parts) = self.processes.tick_parts.borrow_mut().as_mut() {
            parts.push((what.to_string(), since.elapsed()));
        }
    }

    /// Written once an hour, and sooner when twice as slow as the slowest written that hour, so the worst one is in the
    /// log with its parts (named by the code: no account data).
    pub(crate) fn note_tick_time(&self, took: Duration, parts: Vec<(String, Duration)>) {
        if took < SLOW_TICK {
            return;
        }
        let now = Stamp::utc_now();
        let mut since = now;
        let previous = self.processes.slow_tick.get();
        if let Some(last) = previous
            && now >= last.0
            && now.secs_since(last.0) < secs(TICK_ERROR_REPEAT)
        {
            if took < last.1 * 2 {
                self.processes.slow_tick.set(Some((last.0, last.1, last.2 + 1)));
                return;
            }
            since = last.0; // the same hour: the next one written must take twice as long again
        }
        fn name(what: &str) -> &str {
            what.strip_prefix("() => ").unwrap_or(what)
        }
        let mut sorted = parts.clone();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        let slowest: Vec<String> = sorted.iter().take(4).map(|p| format!("{} {:.0} ms", name(&p.0), p.1.as_secs_f64() * 1000.0)).collect();
        let sum: Duration = parts.iter().map(|p| p.1).sum();
        let rest = took.as_secs_f64() - sum.as_secs_f64();
        let unwritten = match previous {
            Some(p) if p.2 > 0 => format!(" {} slow check(s) came since the last one written.", p.2),
            _ => String::new(),
        };
        let clients = self.processes.last_players.borrow().as_ref().map(|p| p.0.len()).unwrap_or(0);
        log::note(&format!(
            "Slow process check: {:.0} ms, {clients} Roblox client(s). Slowest parts: {}; the rest {:.0} ms.{unwritten}",
            took.as_secs_f64() * 1000.0,
            slowest.join(", "),
            rest * 1000.0
        ));
        self.processes.slow_tick.set(Some((since, took, 0)));
    }

    /// Once, not every tick (every 1.5 s would rewrite error.log every few minutes): a repeat is counted and written
    /// once an hour. `kind` and `message`: the failure's type and text.
    fn log_tick_error(&self, kind: &str, message: &str) {
        // PORT: no stack trace's first line to tell two failures apart by place: the type and text alone are the key.
        let key = format!("{kind}|{message}|");
        let now = Stamp::utc_now();
        let last = self.processes.tick_error.borrow().clone();
        if let Some((last_key, logged, repeats)) = &last
            && *last_key == key
            && now >= *logged
            && now.secs_since(*logged) < secs(TICK_ERROR_REPEAT)
        {
            *self.processes.tick_error.borrow_mut() = Some((last_key.clone(), *logged, repeats + 1));
            return;
        }
        if let Some((_, _, repeats)) = last
            && repeats > 0
        {
            log::note(&format!("The process check's last logged error came {repeats} more time(s) after it was written."));
        }
        *self.processes.tick_error.borrow_mut() = Some((key, now, 0));
        log::write(&Fail::other(format!("{kind}: {message}")));
    }

    /// Each tick: whether a window of Bloxkeep's (the main one, a panel, the tray's, a dialog) is in front.
    fn note_focus(&self, now: Stamp) {
        if roblox_processes::foreground_pid() == std::process::id() as i32 {
            self.processes.unfocused_since.set(None);
        } else if self.processes.unfocused_since.get().is_none_or(|s| s > now) {
            self.processes.unfocused_since.set(Some(now));
        }
        let light = !self.controller.background.get() && self.unfocused_for(now) >= secs(PAGE_LIGHT_AFTER);
        if light != self.processes.page_light.get() {
            self.processes.page_light.set(light);
            if let Some(f) = self.processes.set_page_light.borrow().as_ref() {
                f(light);
            }
        }
    }

    /// In seconds.
    fn unfocused_for(&self, now: Stamp) -> f64 {
        match self.processes.unfocused_since.get() {
            Some(since) if now >= since => now.secs_since(since),
            _ => 0.0,
        }
    }

    /// Live RAM per client and for the PC, sent apart from the state so the page can patch numbers in place.
    fn emit_metrics(&self, players: &[PlayerProcess], windows: &HashMap<i32, Vec<isize>>, memory: &MemoryStatus) -> Res<()> {
        // A sleeping page gets the figures of the moment it wakes, not a queue of old ones: none are made for it meanwhile.
        if !self.controller.page_attached.get() || self.auto_lock.screen_locked.get() || self.controller.page_asleep.get() {
            return Ok(());
        }
        let now = Stamp::utc_now();
        let unfocused = self.unfocused_for(now) >= secs(UNFOCUSED_AFTER);
        // Behind a game for a while: the sidebar's figures less often (see UNFOCUSED_AFTER); figures owed (None) go at once.
        let sent = self.processes.metrics_sent_at.get();
        if unfocused && self.controller.last_metrics.borrow().is_some() && now >= sent && now.secs_since(sent) < secs(UNFOCUSED_METRICS_EVERY) {
            return Ok(());
        }
        let working_sets: HashMap<i32, i64> = players.iter().map(|p| (p.pid, p.ram())).collect();
        let keys: Vec<i32> = working_sets.keys().copied().collect();
        let (cpu, gpu) = {
            let usage = self.controller.roblox_usage.borrow();
            usage.refresh(&keys, unfocused);
            (usage.cpu_pct(), usage.gpu_pct())
        };
        let accounts: Vec<serde_json::Value> = running(self)
            .into_iter()
            .map(|(id, r)| serde_json::json!({ "id": id, "ramMb": working_sets.get(&r.0).copied().unwrap_or(0) / MB }))
            .collect();
        // About how many windows fit on this PC, from what these take (capacity.rs).
        let filetime_now = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0) + 11_644_473_600) * 10_000_000;
        let fit = crate::core::capacity::tick(players, memory, cpu, gpu, filetime_now);
        let json = serde_json::json!({
            "accounts": accounts,
            "playRevision": self.stats.play_revision.get(),
            "fit": fit,
            "robloxCount": players.len(),
            // The monitor tab's tiles: clients with a visible window (see monitor_clients), from this tick's window pass.
            "monitorCount": players.iter().filter(|p| windows.get(&p.pid).is_some_and(|l| !l.is_empty())).count(),
            "totalMb": players.iter().map(|p| p.ram()).sum::<i64>() / MB,
            "systemMb": memory.total_mb,
            "availableMb": memory.available_mb / 128 * 128, // coarse, so idle jitter doesn't repaint the page
            "usedPct": memory.load_percent,
            "cpuPct": cpu,
            "gpuPct": gpu,
        })
        .to_string();
        if self.controller.last_metrics.borrow().as_deref() == Some(json.as_str()) {
            return Ok(());
        }
        *self.controller.last_metrics.borrow_mut() = Some(json.clone());
        self.processes.metrics_sent_at.set(now);
        self.post(&format!("{{\"type\":\"event\",\"name\":\"metrics\",\"data\":{json}}}"));
        Ok(())
    }

    /// A window leaving full screen comes back at the size it had. A window shown unseen for a moment is left out.
    fn keep_window_sizes(&self, now: Stamp, windows: &HashMap<i32, Vec<isize>>) {
        let alive: Vec<i32> = windows.keys().copied().collect();
        self.processes.window_sizes.borrow_mut().forget(&alive);
        if self.windows_in_turn() {
            return;
        }
        for (pid, list) in windows {
            let window = roblox_processes::main_window_of(list);
            if window != 0
                && let Some(rect) = roblox_processes::window_rect(window)
            {
                let back = self.processes.window_sizes.borrow_mut().look(
                    *pid,
                    rect,
                    roblox_processes::is_fullscreen(window, rect),
                    roblox_processes::is_plain_window(window),
                    now,
                );
                if let Some(back) = back {
                    roblox_processes::place_window(window, back);
                }
            }
        }
    }

    /// A window opened outside Bloxkeep (from Roblox's own app, a browser's Play) that joined a game signed in as one of the
    /// vault's accounts becomes that account's, as if Bloxkeep had opened it: its card shows it, and macros, Edit on game,
    /// the panel and closing can use it, and auto-rejoin when its server is known to be public (public_target_of).
    /// Not while a launch could still claim it, nor while windows are being closed (Close all's sweep, the monitor tab's
    /// Close) or a separate sign-in window opens; never a second window for an account that has one, nor one whose user id
    /// two accounts share (a Global and a Roblox VNG account: the log can't say which).
    pub(crate) fn adopt_windows_opened_elsewhere(&self, players: &[PlayerProcess]) -> bool {
        if vault_now(self).is_none()
            || self.controller.launch_cts.borrow().is_some()
            || pending_count(self) > 0
            || self.controller.rejoining.get()
            || self.tray.opening_home.get()
            || Stamp::utc_now() < self.controller.sweep_background_until.get()
        {
            return false;
        }
        let mut separate: Option<HashSet<i32>> = None;
        let mut adopted = false;
        for p in players {
            let Some(user) = self.controller.logs.borrow().user_id_of(p.pid) else {
                continue;
            };
            if running_pids(self).contains(&p.pid) || roblox_processes::is_closing(p.pid) {
                continue;
            }
            let named: Vec<Guid> = self.vault(|d| d.accounts.iter().filter(|a| a.user_id == user).take(2).map(|a| a.id).collect()).unwrap_or_default();
            let [account_id] = named.as_slice() else {
                continue;
            };
            let account_id = *account_id;
            if self.controller.running.borrow().contains_key(&account_id) {
                continue;
            }
            // A separate sign-in window keeps its own sign-in: auto-rejoin would reopen it as a normal client. Unsure which
            // windows those are: none is adopted until the next check.
            let home = self.tray.home_session.borrow().is_some();
            if home {
                if separate.is_none() {
                    let found = self.tray.home_session.borrow_mut().as_mut().map(|h| h.find_clients(players, None));
                    let Some((ours, uncertain)) = found else {
                        continue;
                    };
                    if uncertain {
                        break;
                    }
                    separate = Some(ours.into_iter().collect());
                }
                if separate.as_ref().is_some_and(|s| s.contains(&p.pid)) {
                    continue;
                }
            }
            let since = if p.created > 0 { win_kit::filetime_to_stamp(p.created) } else { Stamp::utc_now() };
            let url_of = self.processes.launch_url_of.borrow().clone();
            let url = match url_of {
                Some(f) => f(p.pid, p.created),
                None => roblox_processes::place_launcher_url_of(p.pid, p.created),
            };
            let target = public_target_of(url.as_deref(), log_of(self, p.pid).as_ref());
            let launch = Rc::new(PendingLaunch {
                account_id,
                before: HashSet::new(),
                started_at: Cell::new(since),
                target,
                rejoin: false,
                preset_id: None,
                tracker: None,
                open_retries: 0,
                fps_limit: 0,
                home_screen: false,
            });
            self.controller.running.borrow_mut().insert(account_id, (p.pid, launch));
            // Not one Bloxkeep opened: Settings' Maximise leaves it as the person has it.
            self.presets.maximised.borrow_mut().insert(p.pid, "ingame".to_string());
            adopted = true;
        }
        if adopted {
            self.persist_soon(); // as claim_windows: not encrypted and written on the core's thread in the check
        }
        adopted
    }
}
