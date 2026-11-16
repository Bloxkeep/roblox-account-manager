// AppController.Stats.cs.
//! Play-time statistics: one `PlaySession` per stay in a game by a client Bloxkeep opened (or picked up), kept 90 days.
//! A stay starts when the log says it is joining a server (or at launch, with no log) and ends when the log says the
//! person left or was dropped, or the client quits; a teleport between a game's places is the same stay.
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Duration;

use indexmap::IndexMap;
use serde_json::{Value, json};

use crate::core::app::App;
use crate::core::base::{Cancel, Guid, Res, Stamp};
use crate::core::models::PlaySession;
use crate::core::roblox_api::GameInfo;
use crate::core::roblox_log_watcher::ClientState;
use crate::core::{l, log, rt};

pub const PLAY_HISTORY: Duration = Duration::from_secs(90 * 86400);
pub const MAX_PLAY_SESSIONS: usize = 4000;
/// The last month's stays are kept past `MAX_PLAY_SESSIONS` (the 30-day view needs them all), up to this many.
pub const MAX_RECENT_PLAY_SESSIONS: usize = 20000;
pub const RECENT_PLAY: Duration = Duration::from_secs(31 * 86400);
pub const MAX_TOP_GAMES: usize = 8;
pub const MAX_TIMELINE_ACCOUNTS: usize = 30;
/// A client still without a log after this long is counted from its launch, as playing what it was opened with.
pub const NO_LOG_GRACE: Duration = Duration::from_secs(45);
/// Open stays are written this often, so a crash of Bloxkeep (or of the PC) loses minutes, not hours. Each
/// write is the whole vault (about 400 KB): 15 minutes, not 5, cuts those writes to a third (owner, 2026-11-03).
pub const PLAY_SAVE_INTERVAL: Duration = Duration::from_secs(15 * 60);
/// Process checks come every few seconds; a gap this long means the PC was asleep in between.
pub const SLEEP_GAP: Duration = Duration::from_secs(30);
pub const PLAY_ICON_RETRY: Duration = Duration::from_secs(10 * 60);
pub const STREAK_DAYS: i32 = 60;

#[derive(Default)]
pub struct StatsState {
    pub play_revision: Cell<u64>,
    /// The open stay of each account with a client here (copies of the vault's list entries: `update_play` writes both).
    pub playing: RefCell<HashMap<Guid, PlaySession>>,
    /// Clients that quit while Bloxkeep was locked: how their stay ended, applied at the next unlock.
    pub ended_while_locked: RefCell<HashMap<Guid, (String, Option<i32>, Stamp)>>,
    pub last_play_save_tick: Cell<i64>,
    /// When the last process check ran (`tick_count`, which counts on through a sleep).
    pub last_check_tick: Cell<i64>,
    /// When Windows said the PC is going to sleep (0: awake): a check squeezed in before it stops would begin a
    /// stay spanning the sleep. Lapses on its own, in case the sleep was called off.
    pub asleep_tick: Cell<i64>,
    /// The PC first went to sleep at this time while Bloxkeep was locked: stays still open then end there.
    pub slept_while_locked: Cell<Option<Stamp>>,
    /// When icons were last asked for, by game (universe): one Roblox didn't give isn't asked at every refresh.
    pub play_icon_asked: RefCell<HashMap<i64, Stamp>>,
}

/// Environment.TickCount64: milliseconds since Windows started, counting on through a sleep.
pub fn tick_count() -> i64 {
    // SAFETY: GetTickCount64 takes nothing and only reads the system's counter.
    unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() as i64 }
}

fn ms(d: Duration) -> i64 {
    d.as_millis() as i64
}

/// The local midnight `add_days` days from `at`'s local day (DateTime.Date.AddDays, then as a moment).
pub(crate) fn local_day(at: Stamp, add_days: i64) -> Stamp {
    local_day_hour(at, add_days, 0)
}

/// `hour` o'clock (may be 24 or more: the next day's) on the local day `add_days` days from `at`'s.
pub(crate) fn local_day_hour(at: Stamp, add_days: i64, hour: u32) -> Stamp {
    let c = at.local();
    let noon = Stamp::from_local(c.year, c.month, c.day, 12, 0, 0).plus_secs(add_days as f64 * 86400.0).local();
    Stamp::from_local(noon.year, noon.month, noon.day, hour, 0, 0)
}

/// Whole local days from `first` (a local midnight) to `at`'s local day.
fn days_between(first: Stamp, at: Stamp) -> i64 {
    (local_day(at, 0).secs_since(first) / 86400.0).round() as i64
}

/// Two copies of the same stay (the vault's and `playing`'s).
pub(crate) fn same_stay(a: &PlaySession, b: &PlaySession) -> bool {
    a.account_id == b.account_id && a.pid == b.pid && a.started_at == b.started_at && a.place_id == b.place_id
}

impl App {
    /// The vault is open (C#'s `_vault is not null`).
    pub(crate) fn has_open_vault(&self) -> bool {
        self.vault(|_| ()).is_ok()
    }

    /// `_games.GetValueOrDefault(placeId)`: the place as looked up, if it was.
    pub(crate) fn known_game(&self, place_id: i64) -> Option<GameInfo> {
        self.controller.games.borrow().get(&place_id).cloned().flatten()
    }

    /// Changes a stay in the vault's list and in `playing` alike (C# held the one object in both).
    fn update_play(&self, stay: &PlaySession, f: impl Fn(&mut PlaySession)) {
        let _ = self.vault_mut(|d| {
            if let Some(s) = d.play_sessions.iter_mut().rev().find(|s| same_stay(s, stay)) {
                f(s);
            }
        });
        if let Some(open) = self.stats.playing.borrow_mut().get_mut(&stay.account_id).filter(|o| same_stay(o, stay)) {
            f(open);
        }
    }

    /// Each check, first: one long after the last means the PC slept, which Windows doesn't always announce (its
    /// word may come after this check). Open stays end at the last check as "sleep"; new ones start no earlier than now.
    pub fn note_check_gap(&self, now: Stamp, last_check: Stamp) {
        let tick = tick_count();
        let last = self.stats.last_check_tick.get();
        let slept = last != 0 && tick - last >= ms(SLEEP_GAP);
        self.stats.last_check_tick.set(tick);
        if !slept {
            return;
        }
        self.stats.asleep_tick.set(0);
        if self.controller.resumed_at.get() < now {
            self.controller.resumed_at.set(now);
        }
        if !self.has_open_vault() {
            if self.stats.slept_while_locked.get().is_none() {
                self.stats.slept_while_locked.set(Some(last_check));
            }
        } else if self.suspend_play(last_check) {
            self.persist_soon();
        }
    }

    pub fn track_play(&self, now: Stamp) {
        if !self.has_open_vault() {
            return;
        }
        let asleep = self.stats.asleep_tick.get();
        if asleep != 0 && tick_count() - asleep < ms(SLEEP_GAP) {
            return;
        }
        let mut changed = false;
        let running: Vec<(Guid, i32, std::rc::Rc<crate::core::app::controller::PendingLaunch>)> =
            self.controller.running.borrow().iter().map(|(id, (pid, launch))| (*id, *pid, launch.clone())).collect();
        let resumed_at = self.controller.resumed_at.get();
        let started = self.relaunch.started.get();
        for (id, pid, launch) in running {
            // Another vault's client, still open after a switch: its stays are that vault's, not this one's (2026-10-30
            // review: they filled this vault's statistics and Discord summary with unknown accounts).
            if self.find_account(id).is_none() {
                continue;
            }
            let log = self.controller.logs.borrow().state_of(pid);
            let mut open = self.stats.playing.borrow().get(&id).cloned();
            if let Some(o) = &open {
                // written with the next save, so a crash of Bloxkeep still leaves an end time
                self.update_play(o, |s| s.last_seen_at = Some(now));
            }
            // A client in its game before the PC slept (its stay ended at the sleep) begins a new stay at the wake.
            let since_wake = |at: Stamp| if at < resumed_at { resumed_at } else { at };
            let Some(log) = log else {
                // No log (yet): after a while, playing what it was opened with since the launch. A client open before Bloxkeep started
                // has its log looked for first: its stay may be open again already, or have ended and mustn't be counted twice.
                let before = launch.started_at.get() < started;
                let waited = if before {
                    now.secs_since(started) >= crate::core::roblox_log_watcher::search_limit().as_secs_f64()
                } else {
                    now.secs_since(launch.started_at.get()) >= NO_LOG_GRACE.as_secs_f64()
                };
                if open.is_none() && waited {
                    if let Some(target) = launch.target.as_ref().filter(|t| t.place_id > 0) {
                        self.start_play(id, pid, target.place_id, since_wake(if before { started } else { launch.started_at.get() }), now);
                        changed = true;
                    }
                }
                continue;
            };
            if matches!(log.status.as_str(), "left" | "disconnected" | "signedout") {
                if let Some(o) = open {
                    self.close_play(&o, &log.status, log.reason, log.at);
                    self.stats.playing.borrow_mut().remove(&id);
                    changed = true;
                }
                continue;
            }
            let Some(place) = log.place_id.filter(|p| *p > 0) else { continue };
            // A teleport across games is a new stay, known once the new place is looked up (the process check asks for it);
            // until then the stay goes on rather than split for a place of the same game.
            if let Some(o) = open.clone() {
                if o.place_id != place && !self.same_game(o.place_id, place) {
                    let asked = self.controller.game_fetched_at.borrow().get(&place).copied();
                    if !self.has_game(place, false) && asked.is_some_and(|asked| now.secs_since(asked) < crate::core::app::games::GAME_RETRY.as_secs_f64()) {
                        continue;
                    }
                    self.close_play(&o, "left", None, log.at);
                    self.stats.playing.borrow_mut().remove(&id);
                    open = None;
                    changed = true;
                }
            }
            match open {
                None => {
                    self.start_play(id, pid, place, since_wake(log.at), now);
                    changed = true;
                }
                Some(o) => {
                    if o.game_name.is_empty() {
                        if let Some(named) = self.known_game(o.place_id).filter(|g| !g.name.is_empty()) {
                            // The place was looked up after the stay began (a game moved the player to a place not seen before).
                            self.update_play(&o, |s| {
                                s.game_name = named.name.clone();
                                s.universe_id = named.universe_id;
                            });
                            self.bump_play_revision();
                            changed = true;
                        }
                    }
                }
            }
        }
        let tick = tick_count();
        if !self.stats.playing.borrow().is_empty() && tick - self.stats.last_play_save_tick.get() >= ms(PLAY_SAVE_INTERVAL) {
            self.stats.last_play_save_tick.set(tick);
            changed = true; // the last-seen times above reach the file (cheap: stays don't count for backups)
        }
        if changed {
            self.persist_soon();
        }
    }

    /// The PC is going to sleep: every open stay ends now as "sleep". Hours asleep aren't play time and the wake's
    /// disconnect isn't a kick; a client still in its game then begins a new stay.
    /// `at`: when the PC went to sleep: now, or the last process check before a sleep noticed afterwards.
    /// Returns true when there is something to save.
    pub fn suspend_play(&self, at: Stamp) -> bool {
        let open: Vec<PlaySession> = self.stats.playing.borrow().values().cloned().collect();
        if open.is_empty() {
            return false;
        }
        for stay in &open {
            self.close_play(stay, "sleep", None, at);
        }
        self.stats.playing.borrow_mut().clear();
        true
    }

    pub fn start_play(&self, account_id: Guid, pid: i32, place_id: i64, at: Stamp, now: Stamp) {
        let info = self.known_game(place_id);
        let preset_id = self.controller.running.borrow().get(&account_id).and_then(|(_, launch)| launch.preset_id);
        let session = PlaySession {
            account_id,
            account_label: self.find_account(account_id).map(|a| a.label()),
            pid,
            place_id,
            universe_id: info.as_ref().map_or(0, |i| i.universe_id),
            game_name: info.as_ref().map(|i| i.name.clone()).unwrap_or_default(),
            started_at: if at > now { now } else { at },
            last_seen_at: Some(now),
            preset_id,
            ..PlaySession::default()
        };
        let started_at = session.started_at;
        self.stats.playing.borrow_mut().insert(account_id, session.clone());
        let _ = self.vault_mut(|d| {
            d.play_sessions.push(session);
            prune_play(&mut d.play_sessions, now);
        });
        self.bump_play_revision();
        self.note_activity("joined", account_id, Some(place_id), None, Some(started_at), None, None); // Statistics › Activity
    }

    /// Ends a stay; one its log ended (a leave, a drop, a sign-out) is noted in Statistics › Activity too.
    pub fn close_play(&self, session: &PlaySession, reason: &str, code: Option<i32>, at: Stamp) {
        let ended = if at < session.started_at { session.started_at } else { at };
        self.update_play(session, |s| {
            s.ended_at = Some(ended);
            s.end_reason = Some(reason.to_string());
            s.reason_code = code;
        });
        self.bump_play_revision();
        if matches!(reason, "left" | "disconnected" | "signedout") {
            let kind = if reason == "signedout" { "signedOut" } else { reason };
            self.note_activity(kind, session.account_id, Some(session.place_id), code, Some(ended), None, None);
        }
    }

    fn bump_play_revision(&self) {
        self.stats.play_revision.set(self.stats.play_revision.get().wrapping_add(1));
    }

    /// The client quit: its stay ends as its log says (a leave, a drop), else as closed. While locked the end is
    /// kept for the next unlock.
    pub fn end_play(&self, account_id: Guid, last_log: Option<&ClientState>, now: Stamp) {
        let (reason, code, at) = match last_log {
            Some(l) if matches!(l.status.as_str(), "left" | "disconnected" | "signedout") => {
                (if l.status == "signedout" { "left".to_string() } else { l.status.clone() }, l.reason, l.at)
            }
            _ => ("closed".to_string(), None, now),
        };
        if !self.has_open_vault() {
            self.stats.ended_while_locked.borrow_mut().insert(account_id, (reason, code, at));
            return;
        }
        let Some(open) = self.stats.playing.borrow_mut().remove(&account_id) else { return };
        // `playing` no longer holds it: the vault's copy is changed by `update_play` alone.
        self.close_play(&open, &reason, code, at);
        self.persist_soon();
    }

    /// Before the vault goes away (lock, exit, an update's restart): open stays are left open with their last-seen
    /// time, so a client still running after a restart goes on with its stay (see `resume_play_sessions`).
    /// Returns true when there is something to save.
    pub fn note_play_sessions(&self) -> bool {
        let open: Vec<PlaySession> = self.stats.playing.borrow().values().cloned().collect();
        if open.is_empty() {
            return false;
        }
        let now = Stamp::utc_now();
        for stay in &open {
            self.update_play(stay, |s| s.last_seen_at = Some(now));
        }
        true
    }

    /// After unlocking: a stay left open goes on when its client is still here, the same process (see
    /// AdoptOpenSessions); otherwise it ended while Bloxkeep was away, at its last-seen time. If the PC slept
    /// while locked, a stay still going then ended at that sleep.
    pub fn resume_play_sessions(&self) {
        self.stats.playing.borrow_mut().clear();
        let open: Vec<PlaySession> = self.vault(|d| d.play_sessions.iter().filter(|s| s.ended_at.is_none()).cloned().collect()).unwrap_or_default();
        let mut changed = false;
        let slept_while_locked = self.stats.slept_while_locked.get();
        for session in open {
            let ended = self.stats.ended_while_locked.borrow().get(&session.account_id).cloned();
            let same = self.controller.running.borrow().get(&session.account_id).is_some_and(|(pid, _)| *pid == session.pid);
            match slept_while_locked {
                Some(slept) if slept > session.started_at && (if let Some(e) = &ended { e.2 > slept } else { same }) => {
                    self.close_play(&session, "sleep", None, slept);
                }
                _ => {
                    if let Some((reason, code, at)) = ended {
                        self.close_play(&session, &reason, code, at);
                    } else if same && !self.stats.playing.borrow().contains_key(&session.account_id) {
                        self.stats.playing.borrow_mut().insert(session.account_id, session);
                        continue;
                    } else {
                        let at = session.last_seen_at.unwrap_or(session.started_at);
                        self.close_play(&session, "appClosed", None, at);
                    }
                }
            }
            changed = true;
        }
        self.stats.ended_while_locked.borrow_mut().clear();
        self.stats.slept_while_locked.set(None);
        let now = Stamp::utc_now();
        changed |= self.vault_mut(|d| prune_play(&mut d.play_sessions, now)).unwrap_or(false);
        if changed {
            self.persist_soon();
        }
    }

    /// `play_stats` for the page. Games in the period that no other view loaded have no icon yet: those
    /// are asked for first, one request per 100 games.
    pub async fn play_stats_async(self: &Rc<Self>, days: i32, game: Option<String>) -> Res<Value> {
        let (sessions, presets) = self.vault(|d| (d.play_sessions.clone(), d.settings.presets.clone()))?;
        let now = Stamp::utc_now();
        // A day more than the period: today's figures start at local midnight, up to a day before now.
        let since = now.plus_secs(-((if days == 7 || days == 30 { days } else { 1 }) as f64 + 1.0) * 86400.0);
        let mut seen = HashSet::new();
        let places: Vec<(i64, i64)> = sessions
            .iter()
            .filter(|s| s.ended_at.is_none_or(|ended| ended >= since))
            .map(|s| (s.place_id, s.universe_id))
            .chain(presets.iter().filter_map(App::preset_place_id).map(|place| (place, 0i64)))
            .filter(|p| p.0 > 0 && self.icon_of(p.0, p.1).is_none())
            .filter(|p| seen.insert(p.0))
            .collect();
        if places.is_empty() {
            return self.play_stats(days, None, None, false, game.as_deref());
        }
        // Places whose game isn't known yet: looked up whole (name and icon).
        let unknown: Vec<(i64, i64)> = places.iter().filter(|p| p.1 == 0 && self.known_game(p.0).is_none()).take(40).copied().collect();
        let _ = self.load_games(unknown, false).await;
        if self.stats.play_icon_asked.borrow().len() > crate::core::app::games::MAX_CACHED_GAMES as usize {
            self.stats.play_icon_asked.borrow_mut().clear();
        }
        let mut universes: Vec<i64> = Vec::new();
        for p in &places {
            let u = if p.1 > 0 { p.1 } else { self.known_game(p.0).map_or(0, |g| g.universe_id) };
            let asked_lately = self.stats.play_icon_asked.borrow().get(&u).is_some_and(|at| now.secs_since(*at) < PLAY_ICON_RETRY.as_secs_f64());
            if u > 0 && !self.game_picker.icon_by_universe.borrow().contains_key(&u) && !asked_lately && !universes.contains(&u) {
                universes.push(u);
            }
            if universes.len() == 100 {
                break;
            }
        }
        if !universes.is_empty() {
            for u in &universes {
                self.stats.play_icon_asked.borrow_mut().insert(*u, now);
            }
            let asked = universes.clone();
            match rt::blocking(move || crate::core::roblox_api::get_game_icons(asked, &Cancel::new())).await {
                Ok(icons) => {
                    let mut by_universe = self.game_picker.icon_by_universe.borrow_mut();
                    for (id, url) in icons {
                        by_universe.insert(id, url);
                    }
                }
                Err(ex) if crate::core::roblox_api::is_expected_error(&ex) => {
                    // The rows keep their placeholder until the next try.
                    log::note(&format!(
                        "Statistics: icons of {} game(s) not loaded ({}).",
                        universes.len(),
                        App::log_kind_of(&ex)
                    ));
                }
                Err(ex) => return Err(ex),
            }
        }
        self.play_stats(days, None, None, false, game.as_deref())
    }

    /// Today's stays of every account, for the strip on its card: seconds from local midnight, and 1 where one
    /// ended cut off. Asked for by the Accounts tab every minute while it shows.
    pub fn today_strips(&self) -> Res<Value> {
        let sessions = self.vault(|d| d.play_sessions.clone())?;
        let now = Stamp::utc_now();
        let since = local_day(now, 0).to_utc();
        let mut rows: IndexMap<String, Vec<[i32; 3]>> = IndexMap::new();
        for s in &sessions {
            let end = match s.ended_at {
                Some(ended) if ended < now => ended,
                _ => now,
            };
            let from = if s.started_at > since { s.started_at } else { since };
            if end <= from {
                continue;
            }
            let cut = if s.end_reason.as_deref() == Some("disconnected") && s.ended_at.is_some() { 1 } else { 0 };
            rows.entry(s.account_id.to_string()).or_default().push([from.secs_since(since) as i32, end.secs_since(since) as i32, cut]);
        }
        Ok(json!({ "since": since, "now": now, "rows": rows }))
    }

    /// A game's icon from what is known: its place's lookup, or the icon fetched for its universe.
    pub fn icon_of(&self, place_id: i64, universe_id: i64) -> Option<String> {
        let info = self.known_game(place_id);
        if let Some(icon) = info.as_ref().and_then(|g| g.icon_url.clone()) {
            return Some(icon);
        }
        let universe = if universe_id > 0 { universe_id } else { info.map_or(0, |g| g.universe_id) };
        self.game_picker.icon_by_universe.borrow().get(&universe).cloned()
    }

    /// The page's figures: today (by hour) or the last 7 or 30 days (by day), in local time. A stay counts for its
    /// part inside the period, one still going up to now.
    /// `only`: the accounts to count (a Discord summary for chosen accounts); None for all.
    /// `until`: the period ends here (UTC) instead of now: a Discord summary sent late covers its own day.
    /// `last24_hours`: one day is the 24 hours before the end rather than the calendar day (a Discord summary).
    /// `game`: only this game's stays (its key, as the page has it: u + universe id, or p + place id).
    pub fn play_stats(&self, days: i32, only: Option<&HashSet<Guid>>, until: Option<Stamp>, last24_hours: bool, game: Option<&str>) -> Res<Value> {
        let sessions = self.vault(|d| d.play_sessions.clone())?;
        let days = if days == 7 || days == 30 { days } else { 1 };
        let hourly = days == 1;
        let utc_now = Stamp::utc_now();
        let now = match until {
            Some(stop) if stop < utc_now => stop,
            _ => utc_now,
        };
        let rolling = hourly && last24_hours;
        let since = if rolling {
            now.plus_secs(-86400.0)
        } else if hourly {
            local_day(now, 0).to_utc()
        } else {
            local_day(now, 1 - days as i64).to_utc()
        };
        let count = if hourly { 24 } else { days as usize };
        let starts: Vec<Stamp> = (0..count)
            .map(|i| {
                if rolling {
                    since.plus_secs(i as f64 * 3600.0)
                } else if hourly {
                    local_day_hour(now, 0, i as u32).to_utc()
                } else {
                    local_day(now, i as i64 + 1 - days as i64).to_utc()
                }
            })
            .collect();
        let mut bucket_seconds = vec![0.0f64; starts.len()];

        // (seconds, sessions, kicks, label)
        let mut by_account: IndexMap<Guid, (f64, i32, i32, Option<String>)> = IndexMap::new();
        // (place id, universe id, name, seconds, sessions)
        let mut by_game: IndexMap<String, (i64, i64, String, f64, i32)> = IndexMap::new();
        // A stay begun before its place was looked up has no universe id; one learned later puts it with the rest of its game.
        let mut universe_of: HashMap<i64, i64> = HashMap::new();
        for s in &sessions {
            if s.universe_id > 0 {
                universe_of.insert(s.place_id, s.universe_id);
            }
        }
        let universe_of_stay = |s: &PlaySession| -> i64 {
            if s.universe_id > 0 {
                return s.universe_id;
            }
            match universe_of.get(&s.place_id) {
                Some(u) if *u > 0 => *u,
                _ => self.known_game(s.place_id).map_or(0, |g| g.universe_id),
            }
        };
        let key_of = |s: &PlaySession| -> String {
            let u = universe_of_stay(s);
            if u > 0 { format!("u{u}") } else { format!("p{}", s.place_id) }
        };
        let counted_fn = |s: &PlaySession| only.is_none_or(|o| o.contains(&s.account_id)) && game.is_none_or(|g| key_of(s) == g);
        let mut total = 0.0f64;
        let (mut session_count, mut kicks) = (0i32, 0i32);
        // By the preset whose window it was (None: opened otherwise, or before 1.0.5 noted it, or a preset deleted since).
        let mut by_preset: IndexMap<Guid, (f64, i32, i32, HashSet<Guid>)> = IndexMap::new();
        let mut no_preset: (f64, i32, i32, HashSet<Guid>) = (0.0, 0, 0, HashSet::new());
        let mut longest: (f64, Option<PlaySession>) = (0.0, None);
        let mut segments: HashMap<Guid, Vec<Value>> = HashMap::new();
        // 7 and 30 days: windows running in each hour of each day (hours of play in it), for the page's hour map.
        let first_day = local_day(now, 1 - days as i64);
        let mut heat: Option<Vec<Vec<f64>>> = if hourly { None } else { Some((0..days).map(|_| vec![0.0; 24]).collect()) };
        // Each account's play per bucket (hour or day), for the line on its card.
        let mut account_buckets: HashMap<Guid, Vec<f64>> = HashMap::new();
        // Each account's time per game, for its most played one.
        let mut account_games: IndexMap<(Guid, String), f64> = IndexMap::new();
        for s in &sessions {
            if only.is_some_and(|o| !o.contains(&s.account_id)) {
                continue;
            }
            // After the period's end (a late summary): not in it.
            if until.is_some() && s.started_at >= now {
                continue;
            }
            let end = match s.ended_at {
                Some(ended) if ended < now => ended,
                _ => now,
            };
            let from = if s.started_at > since { s.started_at } else { since };
            if end <= from && s.started_at < since {
                continue;
            }
            let seconds = end.secs_since(from).max(0.0);
            // Every game goes on the map, the one picked or not (to pick another); the rest counts the picked one only.
            let key = key_of(s);
            let had = by_game.get(&key).cloned().unwrap_or((0, 0, String::new(), 0.0, 0));
            let name = if !s.game_name.is_empty() { s.game_name.clone() } else { had.2.clone() };
            by_game.insert(key.clone(), (if had.0 == 0 { s.place_id } else { had.0 }, universe_of_stay(s), name, had.3 + seconds, had.4 + 1));
            if game.is_some_and(|g| key != g) {
                continue;
            }
            let kicked = s.end_reason.as_deref() == Some("disconnected") && s.ended_at.is_some_and(|e| e >= since && e <= now);
            total += seconds;
            session_count += 1;
            if kicked {
                kicks += 1;
            }
            let acc = by_account.get(&s.account_id).cloned().unwrap_or((0.0, 0, 0, None));
            // The newest name recorded for it (the list is oldest first), for an account deleted since.
            by_account.insert(s.account_id, (acc.0 + seconds, acc.1 + 1, acc.2 + kicked as i32, s.account_label.clone().or(acc.3)));
            match s.preset_id.filter(|id| self.preset_for(Some(*id)).is_some()) {
                Some(preset_id) => {
                    let p = by_preset.entry(preset_id).or_insert((0.0, 0, 0, HashSet::new()));
                    p.3.insert(s.account_id);
                    p.0 += seconds;
                    p.1 += 1;
                    p.2 += kicked as i32;
                }
                None => {
                    no_preset.3.insert(s.account_id);
                    no_preset.0 += seconds;
                    no_preset.1 += 1;
                    no_preset.2 += kicked as i32;
                }
            }
            if seconds > longest.0 {
                longest = (seconds, Some(s.clone()));
            }
            *account_games.entry((s.account_id, key.clone())).or_insert(0.0) += seconds;
            if seconds > 0.0 {
                let name = if !s.game_name.is_empty() { Some(s.game_name.clone()) } else { self.known_game(s.place_id).map(|g| g.name) };
                segments.entry(s.account_id).or_default().push(json!({
                    "from": from.to_utc(),
                    "to": end.to_utc(),
                    "game": key,
                    "name": name,
                    "reason": if kicked { Some(crate::core::play_losses::kind_of(s.reason_code)) } else { None },
                }));
            }
            if let Some(heat) = heat.as_mut() {
                if seconds > 0.0 {
                    add_heat(heat, first_day, from, end);
                }
            }
            if seconds > 0.0 {
                for i in 0..starts.len() {
                    let bucket_end = if i + 1 < starts.len() { starts[i + 1] } else { now };
                    let a = if from > starts[i] { from } else { starts[i] };
                    let b = if end < bucket_end { end } else { bucket_end };
                    if b > a {
                        bucket_seconds[i] += b.secs_since(a);
                        account_buckets.entry(s.account_id).or_insert_with(|| vec![0.0; starts.len()])[i] += b.secs_since(a);
                    }
                }
            }
        }

        let game_name_of = |key: &str| -> String {
            let g = &by_game[key];
            if !g.2.is_empty() {
                return g.2.clone();
            }
            match self.known_game(g.0).map(|i| i.name).filter(|n| !n.is_empty()) {
                Some(cached) => cached,
                None => l::f("Place {0}", &[&g.0]),
            }
        };
        let game_icon_of = |key: &str| -> Option<String> { self.icon_of(by_game[key].0, by_game[key].1) };
        let top_game_of = |account: Guid| -> Value {
            let mut own: Vec<(&String, f64)> = account_games.iter().filter(|e| e.0.0 == account).map(|e| (&e.0.1, *e.1)).collect();
            own.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            match own.first() {
                Some((key, _)) => json!({ "key": key, "name": game_name_of(key), "iconUrl": game_icon_of(key) }),
                None => Value::Null,
            }
        };

        // The period before, as far into it as this one has gone (today against yesterday up to this time), when the
        // records reach back that far.
        let previous_since = since.plus_secs(-(if hourly { 1.0 } else { days as f64 }) * 86400.0);
        let previous_end = previous_since.plus_secs(now.secs_since(since));
        let mut previous = Value::Null;
        let counted: Vec<PlaySession> = sessions.iter().filter(|s| counted_fn(s)).cloned().collect();
        let streaks = full_day_streaks(&sessions, now);
        if counted.iter().map(|s| s.started_at).min().is_some_and(|first| first <= previous_since) {
            let mut previous_seconds = 0.0;
            let mut previous_kicks = 0;
            for s in &counted {
                let a = if s.started_at > previous_since { s.started_at } else { previous_since };
                let b = match s.ended_at {
                    Some(ended) if ended < previous_end => ended,
                    _ => previous_end,
                };
                if s.started_at < previous_end && b > a {
                    previous_seconds += b.secs_since(a);
                }
                if s.end_reason.as_deref() == Some("disconnected") && s.ended_at.is_some_and(|e| e >= previous_since && e < previous_end) {
                    previous_kicks += 1;
                }
            }
            previous = json!({ "totalSeconds": previous_seconds as i64, "kicks": previous_kicks });
        }

        let mut accounts_sorted: Vec<(&Guid, &(f64, i32, i32, Option<String>))> = by_account.iter().collect();
        accounts_sorted.sort_by(|a, b| b.1.0.partial_cmp(&a.1.0).unwrap_or(std::cmp::Ordering::Equal));
        let avatar_urls = self.controller.avatar_urls.borrow().clone();
        let accounts: Vec<Value> = accounts_sorted
            .iter()
            .map(|(id, v)| {
                let account = self.find_account(**id);
                json!({
                    "id": id,
                    "label": account.as_ref().map(|a| a.label()).or_else(|| v.3.clone()),
                    "deleted": account.is_none(),
                    "username": account.as_ref().map(|a| a.username.clone()),
                    "avatarUrl": account.as_ref().and_then(|a| avatar_urls.get(&a.user_id).cloned()),
                    "seconds": v.0 as i64,
                    "sessions": v.1,
                    "kicks": v.2,
                    "topGame": top_game_of(**id),
                    "buckets": account_buckets.get(*id).map(|own| own.iter().map(|x| *x as i64).collect::<Vec<i64>>()),
                    "streak": streaks.get(*id).copied().unwrap_or(0),
                })
            })
            .collect();
        let timeline: Vec<Value> = accounts_sorted
            .iter()
            .filter(|a| segments.contains_key(a.0))
            .take(MAX_TIMELINE_ACCOUNTS)
            .map(|(id, _)| json!({ "id": id, "segments": segments[&**id] }))
            .collect();
        let losses: Vec<Value> = crate::core::play_losses::of(&counted, since, now)
            .into_iter()
            .map(|loss| json!({ "kind": loss.kind, "times": loss.times, "seconds": loss.seconds, "accounts": loss.accounts }))
            .collect();
        let mut games_sorted: Vec<(&String, &(i64, i64, String, f64, i32))> = by_game.iter().collect();
        games_sorted.sort_by(|a, b| b.1.3.partial_cmp(&a.1.3).unwrap_or(std::cmp::Ordering::Equal));
        let games: Vec<Value> = games_sorted
            .iter()
            .take(MAX_TOP_GAMES)
            .map(|(key, g)| {
                json!({
                    "key": key,
                    "placeId": g.0,
                    "name": game_name_of(key),
                    "iconUrl": self.icon_of(g.0, g.1),
                    "seconds": g.3 as i64,
                    "sessions": g.4,
                })
            })
            .collect();
        let mut icons = serde_json::Map::new();
        for key in by_game.keys() {
            if let Some(url) = game_icon_of(key) {
                icons.insert(key.clone(), Value::String(url));
            }
        }
        let mut presets_sorted: Vec<(&Guid, &(f64, i32, i32, HashSet<Guid>))> = by_preset.iter().collect();
        presets_sorted.sort_by(|a, b| b.1.0.partial_cmp(&a.1.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut presets: Vec<Value> = presets_sorted
            .iter()
            .filter_map(|(id, p)| {
                let preset = self.preset_for(Some(**id))?;
                Some(json!({
                    "id": id, "name": preset.name,
                    "iconUrl": App::preset_place_id(&preset).and_then(|place| self.icon_of(place, 0)),
                    "seconds": p.0 as i64, "sessions": p.1, "kicks": p.2, "accounts": p.3.len(),
                }))
            })
            .collect();
        if no_preset.1 > 0 {
            presets.push(json!({
                "id": Guid::nil(), "name": "", "iconUrl": Value::Null, "seconds": no_preset.0 as i64, "sessions": no_preset.1,
                "kicks": no_preset.2, "accounts": no_preset.3.len(),
            }));
        }
        let longest = match longest {
            (seconds, Some(stay)) => json!({
                "seconds": seconds as i64,
                "account": stay.account_id,
                "label": self.find_account(stay.account_id).map(|a| a.label()).or(stay.account_label.clone()),
                "game": if !stay.game_name.is_empty() { Some(stay.game_name.clone()) } else { self.known_game(stay.place_id).map(|g| g.name) },
            }),
            _ => Value::Null,
        };
        let ends = if rolling { now } else { local_day(now, 1).to_utc() };
        Ok(json!({
            "days": days,
            "hourly": hourly,
            "since": since.to_utc(),
            "previous": previous,
            "icons": icons,
            "ends": ends.to_utc(),
            "recorded": !sessions.is_empty(),
            "totalSeconds": total as i64,
            // Every game's, for the map's shares while one game is picked (the total is then that game's).
            "gamesSeconds": by_game.values().map(|g| g.3).sum::<f64>() as i64,
            "sessions": session_count,
            "kicks": kicks,
            "accounts": accounts,
            "timeline": timeline,
            "losses": losses,
            "games": games,
            "buckets": starts.iter().enumerate().map(|(i, start)| json!({ "start": start.to_utc(), "seconds": bucket_seconds[i] as i64 })).collect::<Vec<_>>(),
            "heat": heat.map(|h| h.iter().map(|day| day.iter().map(|x| (x * 100.0).round() / 100.0).collect::<Vec<f64>>()).collect::<Vec<_>>()),
            "presets": presets,
            "longest": longest,
        }))
    }
}

/// Stays older than 90 days go, and the oldest finished ones when the list is long: first those older than a
/// month, so the 30-day view stays whole (accounts rejoining all day make thousands a week).
pub fn prune_play(list: &mut Vec<PlaySession>, now: Stamp) -> bool {
    let cutoff = now.plus_secs(-PLAY_HISTORY.as_secs_f64());
    let before = list.len();
    list.retain(|s| s.ended_at.unwrap_or(now) >= cutoff);
    let mut removed = before - list.len();
    fn trim(list: &mut Vec<PlaySession>, max: usize, may: impl Fn(&PlaySession) -> bool) -> usize {
        if list.len() <= max {
            return 0;
        }
        let mut oldest: Vec<usize> = (0..list.len()).filter(|i| list[*i].ended_at.is_some() && may(&list[*i])).collect();
        oldest.sort_by_key(|i| list[*i].started_at);
        let gone: HashSet<usize> = oldest.into_iter().take(list.len() - max).collect();
        let mut i = 0;
        list.retain(|_| {
            let keep = !gone.contains(&i);
            i += 1;
            keep
        });
        gone.len()
    }
    let recent = now.plus_secs(-RECENT_PLAY.as_secs_f64());
    removed += trim(list, MAX_PLAY_SESSIONS, |s| s.ended_at.is_some_and(|e| e < recent));
    removed += trim(list, MAX_RECENT_PLAY_SESSIONS, |_| true);
    removed > 0
}

/// A stay's hours into the hour map: each local hour it covers gets its share (an hour fully played adds 1).
/// `first_day`: the map's first local midnight.
pub fn add_heat(heat: &mut [Vec<f64>], first_day: Stamp, from_utc: Stamp, to_utc: Stamp) {
    let mut at = from_utc;
    let end = to_utc;
    while at < end {
        let hour = at.local().hour;
        let next = local_day_hour(at, 0, hour + 1);
        let stop = if next < end { next } else { end };
        let day = days_between(first_day, at);
        if day >= 0 && (day as usize) < heat.len() {
            heat[day as usize][hour as usize] += stop.secs_since(at) / 3600.0;
        }
        // PORT: .NET's local DateTime always steps forward an hour; moments here go through this PC's offset, so a
        // step that wouldn't move (a clock change) ends the loop rather than spin.
        if stop <= at {
            break;
        }
        at = stop;
    }
}

/// Each account's days in a row, up to yesterday, it played nearly all of (23 hours or more), looking back
/// at most `STREAK_DAYS` days. Today isn't over, so it doesn't count yet.
pub fn full_day_streaks(sessions: &[PlaySession], now_utc: Stamp) -> HashMap<Guid, i32> {
    let first = local_day(now_utc, -(STREAK_DAYS as i64));
    let first_utc = first.to_utc();
    let mut played: IndexMap<Guid, Vec<Vec<f64>>> = IndexMap::new();
    for s in sessions {
        let end = match s.ended_at {
            Some(ended) if ended < now_utc => ended,
            _ => now_utc,
        };
        if end <= first_utc {
            continue;
        }
        let hours = played.entry(s.account_id).or_insert_with(|| (0..STREAK_DAYS).map(|_| vec![0.0; 24]).collect());
        // From the first counted day only: a stay open for weeks (an AFK farm) would otherwise walk every hour
        // before it, on each Statistics refresh, only to throw those hours away.
        let start = if s.started_at > first_utc { s.started_at } else { first_utc };
        add_heat(hours, first, start, end);
    }
    played
        .into_iter()
        .map(|(id, days)| {
            let mut n = 0;
            let mut day = STREAK_DAYS - 1;
            while day >= 0 && days[day as usize].iter().sum::<f64>() >= 23.0 {
                n += 1;
                day -= 1;
            }
            (id, n)
        })
        .collect()
}
