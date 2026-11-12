// AppController.Schedule.cs: scheduled presets and the launch order of accounts. A preset can run at a time on chosen
// weekdays (or once, on a date) and close what it opened after some hours or at a time. Checked once per local wall-clock
// minute: a DST change or a hand-set clock only skips or repeats a minute; a preset runs at most once per scheduled minute
// and never twice within a day of uptime. Minutes missed while Bloxkeep wasn't running aren't caught up. Nobody is asked
// anything: a launch needing a confirmation (Multi-Roblox off, low RAM) is skipped and said.
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    rc::Rc,
    sync::{Arc, atomic::{AtomicU64, Ordering}},
    time::Duration,
};

use serde_json::{Value, json};

use super::App;
use super::controller::{bool_of, str_of};
use super::presets::{same_vault, secs, short};
use crate::core::{
    base::{Fail, Guid, Kind, Res, Stamp},
    clock, l, log,
    models::{Account, LaunchPreset, PresetSchedule, ScheduledRunState, VaultData},
    roblox_api, rt,
    vault::Vault,
};

/// A preset that just ran can't run again before this much uptime (a day minus the biggest clock shift).
pub const MIN_SCHEDULE_GAP: Duration = Duration::from_secs(22 * 3600);
pub const MAX_STOP_HOURS: f64 = 168.0;
pub const SCHEDULE_LAUNCH_WAIT: Duration = Duration::from_secs(5 * 60);
pub const SCHEDULE_STOP_WAIT: Duration = Duration::from_secs(20);

/// A moment as this PC's clock writes it, without a zone: C#'s local DateTime, which adds, compares and subtracts by
/// the wall clock (a DST change is not a moment's difference to it). Seconds since 1970-01-01 00:00 on the wall.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Wall(i64);

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + if m <= 2 { 1 } else { 0 }, m, d)
}

impl Wall {
    /// Clock.Now().
    pub fn now() -> Wall {
        Wall::of(clock::now())
    }
    /// The wall-clock reading of a moment: by the offset it was written with when it has one (a swapped test clock
    /// walking across a DST change), else this PC's.
    pub fn of(at: Stamp) -> Wall {
        let c = match at.kind() {
            Kind::Local(m) => at.plus_secs(m as f64 * 60.0).utc(),
            _ => at.local(),
        };
        Wall::from_parts(c.year as i64, c.month as i64, c.day as i64, c.hour as i64, c.minute as i64, c.second as i64)
    }
    pub fn from_parts(y: i64, m: i64, d: i64, h: i64, min: i64, s: i64) -> Wall {
        Wall(days_from_civil(y, m, d) * 86400 + h * 3600 + min * 60 + s)
    }
    /// (year, month, day, hour, minute, second)
    pub fn parts(self) -> (i64, i64, i64, i64, i64, i64) {
        let (days, rest) = (self.0.div_euclid(86400), self.0.rem_euclid(86400));
        let (y, m, d) = civil_from_days(days);
        (y, m, d, rest / 3600, rest % 3600 / 60, rest % 60)
    }
    /// DayOfWeek: 0 Sunday … 6 Saturday.
    pub fn day_of_week(self) -> i32 {
        (self.0.div_euclid(86400) + 4).rem_euclid(7) as i32
    }
    /// .Date: midnight of its day.
    pub fn date(self) -> Wall {
        Wall(self.0.div_euclid(86400) * 86400)
    }
    /// The start of its minute.
    pub fn minute(self) -> Wall {
        Wall(self.0.div_euclid(60) * 60)
    }
    pub fn add_days(self, days: i64) -> Wall {
        Wall(self.0 + days * 86400)
    }
    pub fn add_secs(self, s: i64) -> Wall {
        Wall(self.0 + s)
    }
    /// `(self - earlier).TotalSeconds`.
    pub fn secs_since(self, earlier: Wall) -> i64 {
        self.0 - earlier.0
    }
    /// "yyyy-MM-dd HH:mm".
    pub fn minute_key(self) -> String {
        let (y, m, d, h, min, _) = self.parts();
        format!("{y:04}-{m:02}-{d:02} {h:02}:{min:02}")
    }
    /// "yyyy-MM-dd".
    pub fn ymd(self) -> String {
        let (y, m, d, ..) = self.parts();
        format!("{y:04}-{m:02}-{d:02}")
    }
    /// "HH:mm".
    pub fn hh_mm(self) -> String {
        let (.., h, min, _) = self.parts();
        format!("{h:02}:{min:02}")
    }
    /// The moment it stands for on this PC's clock (a local DateTime as the page gets it).
    pub fn to_stamp(self) -> Stamp {
        let (y, m, d, h, min, s) = self.parts();
        Stamp::from_local(y as i32, m as u32, d as u32, h as u32, min as u32, s as u32)
    }
}

static RUN_KEYS: AtomicU64 = AtomicU64::new(1);

/// A scheduled run whose stop is still to come: it closes only the accounts the run opened.
#[derive(Clone, Debug)]
pub struct ScheduledRun {
    /// Which run this is (C# told them apart by reference).
    pub key: u64,
    pub preset_id: Guid,
    /// The file of the vault whose preset this is: runs of another vault, still armed after a switch, aren't
    /// written into the vault open now (2026-10-30 review).
    pub vault: String,
    pub name: String,
    pub opened: Vec<Guid>,
    /// Uptime (Environment.TickCount64, ms) to stop at, for "after N hours".
    pub stop_at_tick: Option<i64>,
    /// Local minute ("yyyy-MM-dd HH:mm") to stop at, for "at a time".
    pub stop_at_minute: Option<String>,
}

impl ScheduledRun {
    pub(crate) fn new(preset_id: Guid, vault: String, name: String, opened: Vec<Guid>) -> ScheduledRun {
        ScheduledRun { key: RUN_KEYS.fetch_add(1, Ordering::Relaxed), preset_id, vault, name, opened, stop_at_tick: None, stop_at_minute: None }
    }
}

pub struct ScheduleState {
    pub scheduled_runs: RefCell<Vec<ScheduledRun>>,
    /// Presets whose run stopped while the vault was locked, so the vault still lists it (see resume_schedules).
    pub stopped_while_locked: RefCell<HashSet<Guid>>,
    pub scheduled_start_tick: RefCell<std::collections::HashMap<Guid, i64>>,
    pub last_schedule_minute: RefCell<String>,
    /// The local time of the last new-minute check; None after an unlock (the lock's own report covers that gap).
    pub last_schedule_check: Cell<Option<Wall>>,
    /// Runs due while the vault is locked (noted when it locked): each is reported once as missed.
    pub locked_schedules: RefCell<Vec<(Guid, String, String)>>,
    /// Preset and minute of each run reported as missed while locked, so the unlock doesn't report it again.
    pub missed_reported: RefCell<HashSet<(Guid, String)>>,
    /// Since when runs could have been missed: the app's start, then each lock.
    pub locked_since: Cell<Wall>,
    /// Accounts a scheduled stop is closing right now: a start in the same minute waits for them to go.
    pub stop_closing: RefCell<HashSet<Guid>>,
}

impl Default for ScheduleState {
    fn default() -> ScheduleState {
        ScheduleState {
            scheduled_runs: RefCell::default(),
            stopped_while_locked: RefCell::default(),
            scheduled_start_tick: RefCell::default(),
            last_schedule_minute: RefCell::default(),
            last_schedule_check: Cell::new(None),
            locked_schedules: RefCell::default(),
            missed_reported: RefCell::default(),
            locked_since: Cell::new(Wall::now()),
            stop_closing: RefCell::default(),
        }
    }
}

pub fn of_vault(run: &ScheduledRun, vault: &Vault) -> bool {
    run.vault.to_lowercase() == vault.file_path().to_string_lossy().to_lowercase()
}

pub fn minute_key(local: Wall) -> String {
    local.minute_key()
}

/// "HH:mm" as TimeSpan.TryParseExact(@"hh\:mm") reads it: the seconds into the day.
pub fn try_time(text: &str) -> Option<i64> {
    let (h, m) = text.split_once(':')?;
    if h.len() != 2 || m.len() != 2 || !h.bytes().all(|b| b.is_ascii_digit()) || !m.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (h, m): (i64, i64) = (h.parse().ok()?, m.parse().ok()?);
    if h > 23 || m > 59 {
        return None;
    }
    Some(h * 3600 + m * 60)
}

/// "yyyy-MM-dd" as DateTime.TryParseExact reads it: midnight of that day.
pub fn try_date(text: &str) -> Option<Wall> {
    let mut parts = text.split('-');
    let (y, m, d) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || y.len() != 4 || m.len() != 2 || d.len() != 2 {
        return None;
    }
    if ![y, m, d].iter().all(|p| p.bytes().all(|b| b.is_ascii_digit())) {
        return None;
    }
    let (y, m, d): (i64, i64, i64) = (y.parse().ok()?, m.parse().ok()?, d.parse().ok()?);
    if y < 1 || !(1..=12).contains(&m) || d < 1 {
        return None;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][(m - 1) as usize];
    if d > days {
        return None;
    }
    Some(Wall::from_parts(y, m, d, 0, 0, 0))
}

/// The form's "schedule" object checked; none when the page sent null (no schedule set up), the old one when it sent nothing.
pub fn parse_schedule(a: &Value, old: Option<&PresetSchedule>) -> Res<Option<PresetSchedule>> {
    match a.get("schedule") {
        Some(Value::Null) => return Ok(None),
        Some(Value::Object(_)) => {}
        _ => return Ok(old.cloned()),
    }
    let sc = &a["schedule"];
    let mut days: Vec<i32> = Vec::new();
    if let Some(list) = sc.get("days").and_then(|d| d.as_array()) {
        for d in list {
            if let Some(n) = d.as_i64().filter(|n| (0..=6).contains(n)) {
                if !days.contains(&(n as i32)) {
                    days.push(n as i32);
                }
            }
        }
        days.sort();
    }
    let stop = str_of(sc, "stop");
    let mut s = PresetSchedule {
        enabled: bool_of(sc, "enabled"),
        time: str_of(sc, "time").trim().to_string(),
        date: str_of(sc, "date").trim().to_string(),
        days,
        stop: if stop == "after" || stop == "at" { stop } else { "never".to_string() },
        stop_hours: match sc.get("stopHours") {
            Some(v) if v.is_number() => v.as_f64().unwrap_or(0.0).clamp(0.0, MAX_STOP_HOURS),
            _ => 0.0,
        },
        stop_time: str_of(sc, "stopTime").trim().to_string(),
        last_run: old.map(|o| o.last_run.clone()).unwrap_or_default(),
        ..PresetSchedule::default()
    };
    if !s.date.is_empty() {
        s.days.clear(); // once, on that date
    }
    if !s.enabled {
        return Ok(Some(s)); // kept as typed, for when it is turned on again
    }
    if try_time(&s.time).is_none() {
        return Err(Fail::user(l::t("Enter the start time as HH:mm.")));
    }
    if !s.date.is_empty() {
        if try_date(&s.date).is_none() {
            return Err(Fail::user(l::t("Pick the date to run on.")));
        }
        if next_run(Some(&s)).is_none() {
            // Saved again as it was (the preset renamed, say) after its time went by: over, so off, not in the way.
            if old.is_some_and(|o| o.enabled && o.date == s.date && o.time == s.time) {
                s.enabled = false;
                return Ok(Some(s));
            }
            return Err(Fail::user(l::t("That date and time have already passed.")));
        }
    } else if s.days.is_empty() {
        return Err(Fail::user(l::t("Pick at least one weekday, or a date.")));
    }
    if s.stop == "after" && s.stop_hours <= 0.0 {
        return Err(Fail::user(l::f("Enter how many hours to play (up to {0}).", &[&MAX_STOP_HOURS])));
    }
    if s.stop == "at" && try_time(&s.stop_time).is_none() {
        return Err(Fail::user(l::t("Enter the stop time as HH:mm.")));
    }
    Ok(Some(s))
}

pub fn schedule_dto(s: Option<&PresetSchedule>) -> Value {
    match s {
        None => Value::Null,
        Some(s) => json!({ "enabled": s.enabled, "time": s.time, "days": s.days, "date": s.date, "stop": s.stop, "stopHours": s.stop_hours, "stopTime": s.stop_time }),
    }
}

/// The next minute this schedule starts at (local time), or None when it never will.
pub fn next_run(s: Option<&PresetSchedule>) -> Option<Wall> {
    let s = s.filter(|s| s.enabled)?;
    let time = try_time(&s.time)?;
    let now = Wall::now();
    let this_minute = now.minute();
    if !s.date.is_empty() {
        let date = try_date(&s.date)?;
        let at = date.add_secs(time);
        return if at >= this_minute && s.last_run != minute_key(at) { Some(at) } else { None };
    }
    for d in 0..=7 {
        let at = now.date().add_days(d).add_secs(time);
        if s.days.contains(&at.day_of_week()) && at >= this_minute && s.last_run != minute_key(at) {
            return Some(at);
        }
    }
    None
}

pub fn due_at(s: &PresetSchedule, local: Wall) -> bool {
    s.time == local.hh_mm() && if !s.date.is_empty() { s.date == local.ymd() } else { s.days.contains(&local.day_of_week()) }
}

impl App {
    /// A schedule was saved or switched: the current minute is looked at again so one set for it still runs (LastRun
    /// prevents twice); a planned stop goes with a schedule switched off or set to never stop, since nothing shows it any more.
    pub fn schedule_changed(&self, preset_id: Guid, schedule: Option<&PresetSchedule>) {
        *self.schedule.last_schedule_minute.borrow_mut() = String::new();
        let keeps_stop = schedule.is_some_and(|s| s.enabled && s.stop != "never");
        if !keeps_stop {
            let removed = {
                let mut runs = self.schedule.scheduled_runs.borrow_mut();
                let before = runs.len();
                runs.retain(|r| r.preset_id != preset_id);
                before - runs.len()
            };
            if removed > 0 {
                self.note_scheduled_runs(); // saved with the preset
            }
        }
    }

    /// Every process-timer tick: stops that are due, then (once per wall-clock minute) starts that are due.
    pub fn check_schedules(self: &Rc<Self>) {
        let local = Wall::now();
        let minute = minute_key(local);
        let tick = clock::tick();
        // A stop whose minute passed while the PC slept still happens, late rather than never.
        let due_stops: Vec<u64> = self
            .schedule
            .scheduled_runs
            .borrow()
            .iter()
            .filter(|r| match r.stop_at_tick {
                Some(at) => tick >= at,
                None => r.stop_at_minute.as_deref().is_none_or(|m| minute.as_str() >= m),
            })
            .map(|r| r.key)
            .collect();
        for key in due_stops {
            let run = {
                let mut runs = self.schedule.scheduled_runs.borrow_mut();
                runs.iter().position(|r| r.key == key).map(|i| runs.remove(i))
            };
            if let Some(run) = run {
                self.stop_scheduled_run(run);
            }
        }
        if minute == *self.schedule.last_schedule_minute.borrow() {
            return;
        }
        *self.schedule.last_schedule_minute.borrow_mut() = minute.clone();
        let last_check = self.schedule.last_schedule_check.get();
        self.schedule.last_schedule_check.set(Some(local));
        let Some(vault) = self.held_vault() else {
            // Said once its minute is over: an unlock within the minute still runs it (and nothing is reported).
            let missed: Vec<(Guid, String, String)> = self.schedule.locked_schedules.borrow().iter().filter(|m| m.2.as_str() < minute.as_str()).cloned().collect();
            for (id, name, at) in missed {
                log::note(&format!("Schedule: preset {} missed {}, Bloxkeep locked.", short(&id), &at[11..]));
                self.schedule.missed_reported.borrow_mut().insert((id, at.clone()));
                self.notify(&l::f("{0} didn't run at {1}: Bloxkeep was locked.", &[&name, &&at[11..]]), "warn", "scheduleMissed", id, true, None);
            }
            self.schedule.locked_schedules.borrow_mut().retain(|m| m.2.as_str() >= minute.as_str());
            return;
        };
        self.schedule.locked_schedules.borrow_mut().clear();
        // The timer didn't tick for a while (the PC slept, or hung): a run due in between is said to be missed, once.
        if let Some(last_check) = last_check {
            if local.secs_since(last_check) > 90 {
                log::note(&format!("Schedule: no check from {} to {} (the PC slept or hung).", last_check.hh_mm(), local.hh_mm()));
                self.report_schedule_gap(&vault, last_check, local);
            }
        }
        // A one-off whose time went by unseen (PC off, Bloxkeep closed): over, as if it had run, not left on with no next run.
        let over = self
            .vault_mut(|d| {
                let mut over = false;
                for preset in d.settings.presets.iter_mut() {
                    let id = preset.id;
                    if let Some(once) = preset.schedule.as_mut() {
                        if once.enabled && !once.date.is_empty() && !due_at(once, local) && next_run(Some(once)).is_none() {
                            log::note(&format!("Schedule: preset {}'s one-off at {} {} went by unseen; switched off.", short(&id), once.date, once.time));
                            once.enabled = false;
                            over = true;
                        }
                    }
                }
                over
            })
            .unwrap_or(false);
        if over {
            self.persist_soon();
            self.push_state();
        }
        let gap_ms = MIN_SCHEDULE_GAP.as_millis() as i64;
        let due: Vec<LaunchPreset> = self
            .vault_mut(|d| {
                let mut due = Vec::new();
                for preset in d.settings.presets.iter_mut() {
                    let id = preset.id;
                    let Some(s) = preset.schedule.as_mut() else {
                        continue;
                    };
                    if !s.enabled || !due_at(s, local) || s.last_run == minute {
                        continue;
                    }
                    let last = self.schedule.scheduled_start_tick.borrow().get(&id).copied();
                    if let Some(last) = last {
                        if tick - last < gap_ms {
                            log::note(&format!(
                                "Schedule: preset {} due at {} skipped, it ran {} min ago (clock set back).",
                                short(&id),
                                &minute[11..],
                                (tick - last) / 60_000
                            ));
                            continue; // the clock was set back: it ran already
                        }
                    }
                    s.last_run = minute.clone();
                    self.schedule.scheduled_start_tick.borrow_mut().insert(id, tick);
                    if !s.date.is_empty() {
                        s.enabled = false; // a one-off has had its turn
                    }
                    due.push(preset.clone());
                }
                due
            })
            .unwrap_or_default();
        if due.is_empty() {
            return;
        }
        log::note(&format!(
            "Schedule: {} preset(s) due at {} ({}).",
            due.len(),
            &minute[11..],
            due.iter().map(|p| short(&p.id)).collect::<Vec<_>>().join(", ")
        ));
        self.persist_soon();
        self.push_state(); // the next run time changed
        rt::spawn({
            let app = self.clone();
            async move { app.run_scheduled(due).await }
        });
    }

    /// Presets due the same minute run one after another: a second launch can't start while one is going.
    pub async fn run_scheduled(self: &Rc<Self>, due: Vec<LaunchPreset>) {
        for preset in due {
            if let Err(ex) = self.run_scheduled_one(preset.clone()).await {
                // Said, not only logged (ErrorText logs it): nobody may be at the PC to notice a run that didn't happen.
                log::note(&format!("Schedule: preset {} didn't run ({}).", short(&preset.id), App::launch_fail_kind(&ex)));
                self.notify(&l::f("Scheduled preset {0} didn't run: {1}", &[&preset.name, &App::error_text(&ex)]), "warn", "scheduleFail", preset.id, true, None);
            }
        }
    }

    pub fn open_or_opening(&self) -> HashSet<Guid> {
        let mut set: HashSet<Guid> = self.controller.running.borrow().keys().copied().collect();
        set.extend(self.controller.pending.borrow().iter().map(|p| p.account_id));
        set
    }

    pub async fn run_scheduled_one(self: &Rc<Self>, preset: LaunchPreset) -> Res<()> {
        let vault = self.held_vault();
        let name = preset.name.clone();
        let started_at = Wall::now(); // the scheduled minute, give or take: a stop "at" counts from here, not from when the launch ends
        let tag = format!("Schedule: preset {}", short(&preset.id));
        if self.controller.launch_cts.borrow().is_some() {
            log::note(&format!("{tag} waits for a launch under way ({} min at most).", SCHEDULE_LAUNCH_WAIT.as_secs() / 60));
        }
        // A manual launch under way would refuse this one ("already launching"): it is waited for, within reason.
        let until = clock::utc_now().plus_secs(secs(SCHEDULE_LAUNCH_WAIT));
        while self.controller.launch_cts.borrow().is_some() && clock::utc_now() < until && same_vault(&self.held_vault(), &vault) {
            rt::sleep(Duration::from_millis(1000)).await;
        }
        let closing = |app: &App| -> bool {
            let stop_closing = app.schedule.stop_closing.borrow();
            let running = app.controller.running.borrow();
            preset.account_ids.iter().any(|id| stop_closing.contains(id) && running.contains_key(id))
        };
        if closing(self) {
            log::note(&format!("{tag} waits for its stop to finish closing its windows."));
        }
        // This preset's stop firing this same minute is still closing its clients: wait, so the start reopens rather than skips them.
        let until = clock::utc_now().plus_secs(secs(SCHEDULE_STOP_WAIT));
        while closing(self) && clock::utc_now() < until && same_vault(&self.held_vault(), &vault) {
            rt::sleep(Duration::from_millis(500)).await;
        }
        {
            let mut stop_closing = self.schedule.stop_closing.borrow_mut();
            for id in &preset.account_ids {
                stop_closing.remove(id);
            }
        }
        if !same_vault(&self.held_vault(), &vault) {
            log::note(&format!("{tag} didn't run, Bloxkeep was locked meanwhile."));
            return Ok(());
        }
        // The newest run owns this preset's stop: a pending one (daily 20:00, stop after 24 h) is taken over with its still-open
        // accounts, so the old stop can't close everything minutes after the new start.
        let taken: Vec<ScheduledRun> = {
            let mut runs = self.schedule.scheduled_runs.borrow_mut();
            let taken = runs.iter().filter(|r| r.preset_id == preset.id).cloned().collect();
            runs.retain(|r| r.preset_id != preset.id);
            taken
        };
        let before = self.open_or_opening();
        let mut started = false;
        let outcome: Res<()> = async {
            // A warning answered "Don't ask again" is left out by the launch itself, so it doesn't stop this run either.
            let result = match self.start_preset(preset.id, false).await {
                Ok(r) => r,
                Err(ex) if matches!(ex, Fail::User(_) | Fail::Roblox { .. }) || roblox_api::is_expected_error(&ex) => {
                    let busy = self.controller.launch_cts.borrow().is_some();
                    log::note(&format!("{tag} didn't run ({}{}).", App::launch_fail_kind(&ex), if busy { ", a launch still under way" } else { "" }));
                    self.notify(&l::f("Scheduled preset {0} didn't run: {1}", &[&name, &App::error_text(&ex)]), "warn", "scheduleFail", preset.id, true, None);
                    return Ok(());
                }
                Err(ex) => return Err(ex),
            };
            if result.needs_confirm {
                log::note(&format!("{tag} didn't run, it would need a confirmation ({}).", result.asks.clone().unwrap_or_default().join(", ")));
                // The question Play would ask (Multi-Roblox not ready, low RAM) isn't answered for the person.
                let why = result.message.clone().unwrap_or_default().replace("\n\n", " ");
                self.notify(&l::f("Scheduled preset {0} didn't run: {1}", &[&name, &why]), "warn", "scheduleFail", preset.id, true, None);
                return Ok(());
            }
            if result.cancelled {
                log::note(&format!("{tag} stopped (Stop pressed or Bloxkeep locked) after {} opened.", result.launched));
                return Ok(()); // Stop was pressed (or Bloxkeep locked): the person is there and saw it
            }
            started = true;
            let errors = result.errors.clone().unwrap_or_default();
            log::note(&format!(
                "{tag} started: {} opened, {} not opened, {} already open.",
                result.launched,
                errors.len(),
                result.skipped.as_ref().map_or(0, |s| s.len())
            ));
            if result.launched > 0 {
                self.notify(
                    &l::p(result.launched as i64, "Scheduled preset {0} started: {1} account opened.", "Scheduled preset {0} started: {1} accounts opened.", &[&name, &result.launched]),
                    "ok",
                    "schedule",
                    preset.id,
                    true,
                    None,
                );
            } else if errors.is_empty() {
                self.notify(&l::f("Scheduled preset {0}: its accounts were already open.", &[&name]), "info", "schedule", preset.id, true, None);
            }
            // Its own type: the "started" notice a moment earlier would otherwise hold this one back for five minutes.
            if !errors.is_empty() {
                self.notify(
                    &l::p(
                        errors.len() as i64,
                        "Scheduled preset {0}: {1} account didn't open. {2}",
                        "Scheduled preset {0}: {1} accounts didn't open. {2}",
                        &[&name, &errors.len(), &errors[0]],
                    ),
                    "warn",
                    "scheduleFail",
                    preset.id,
                    true,
                    None,
                );
            }
            Ok(())
        }
        .await;
        // However the launch ended, what it opened gets its stop; a start that didn't happen leaves the old stop in place.
        // The preset as it is now (C# held the live object); deleted or locked meanwhile, as it was.
        let live = self.preset_for(Some(preset.id)).unwrap_or(preset);
        self.arm_scheduled_stop(&live, vault, started_at, before, taken, started);
        outcome
    }

    /// The stop of a run that just started: closes only what this run (and the run it took over) opened and is still open.
    pub fn arm_scheduled_stop(&self, preset: &LaunchPreset, vault: Option<Arc<Vault>>, started_at: Wall, before: HashSet<Guid>, taken: Vec<ScheduledRun>, started: bool) {
        let open = |id: &Guid| -> bool { self.controller.running.borrow().contains_key(id) || self.controller.pending.borrow().iter().any(|p| p.account_id == *id) };
        let fresh: Vec<Guid> = preset.account_ids.iter().filter(|id| !before.contains(id) && open(id)).copied().collect();
        if !started && fresh.is_empty() {
            if !taken.is_empty() {
                log::note(&format!("Schedule: preset {} didn't start; its earlier stop stays.", short(&preset.id)));
            }
            self.schedule.scheduled_runs.borrow_mut().extend(taken);
            return;
        }
        let mut opened: Vec<Guid> = Vec::new();
        for id in fresh.iter().copied().chain(taken.iter().flat_map(|r| r.opened.iter().copied()).filter(|id| open(id))) {
            if !opened.contains(&id) {
                opened.push(id);
            }
        }
        // Locked meanwhile: kept in memory, written at the next unlock; another vault unlocked meanwhile has other accounts.
        let now_vault = self.held_vault();
        let other_vault = now_vault.is_some() && !same_vault(&now_vault, &vault);
        let schedule = preset.schedule.clone();
        let (Some(vault), Some(s)) = (vault.clone(), schedule.clone()) else {
            if preset.schedule.as_ref().is_some_and(|s| s.stop != "never") {
                log::note(&format!(
                    "Schedule: preset {}'s stop not set ({}).",
                    short(&preset.id),
                    if opened.is_empty() { "nothing it opened is open" } else if vault.is_none() { "locked" } else { "another vault is open" }
                ));
            }
            return;
        };
        if s.stop == "never" || opened.is_empty() || other_vault {
            if s.stop != "never" {
                log::note(&format!(
                    "Schedule: preset {}'s stop not set ({}).",
                    short(&preset.id),
                    if opened.is_empty() { "nothing it opened is open" } else { "another vault is open" }
                ));
            }
            return;
        }
        let mut run = ScheduledRun::new(preset.id, vault.file_path().to_string_lossy().to_string(), preset.name.clone(), opened.clone());
        if s.stop == "after" {
            run.stop_at_tick = Some(clock::tick() + (s.stop_hours * 3_600_000.0) as i64);
        } else if let (true, Some(time)) = (s.stop == "at", try_time(&s.stop_time)) {
            let mut at = started_at.date().add_secs(time);
            if at <= started_at {
                at = at.add_days(1); // a stop at or before the start time means the next day (22:00 → 02:00)
            }
            run.stop_at_minute = Some(minute_key(at));
        } else {
            return;
        }
        let when = match &run.stop_at_minute {
            Some(stop_minute) => format!("at {}", &stop_minute[11..]),
            None => format!("after {} h", s.stop_hours),
        };
        self.schedule.scheduled_runs.borrow_mut().push(run);
        log::note(&format!(
            "Schedule: preset {}'s stop set for {} account(s), {when}{}.",
            short(&preset.id),
            opened.len(),
            if !taken.is_empty() { ", taking over the earlier run's" } else { "" }
        ));
        if self.note_scheduled_runs() {
            self.persist_soon();
        }
    }

    /// `run` is already out of the list.
    pub fn stop_scheduled_run(&self, run: ScheduledRun) {
        // Locked (or another vault open), its vault can't be told; the next unlock mustn't arm this stop again from what it still holds.
        let open = self.held_vault();
        let ours = open.as_ref().is_some_and(|v| of_vault(&run, v));
        if !ours {
            self.schedule.stopped_while_locked.borrow_mut().insert(run.preset_id);
        }
        if self.note_scheduled_runs() {
            self.persist_soon();
        }
        // A client waiting to rejoin (auto-rejoin's wait can be minutes long) or to restart is called off too, as is one whose
        // window is late (it would come after the stop and stay) or whose rejoin is under way.
        let ids: Vec<Guid> = run
            .opened
            .iter()
            .filter(|id| {
                self.controller.running.borrow().contains_key(*id)
                    || self.controller.pending.borrow().iter().chain(self.launching.late.borrow().iter()).any(|p| p.account_id == **id)
                    || self.controller.rejoin_queue.borrow().iter().any(|r| r.account_id == **id)
                    || self.rejoin.rejoin_in_flight.borrow().as_ref().is_some_and(|r| r.account_id == **id)
            })
            .copied()
            .collect();
        log::note(&format!(
            "Schedule: preset {}'s stop, {} of {} account(s) it opened still open or coming back{}.",
            short(&run.preset_id),
            ids.len(),
            run.opened.len(),
            if !ours { ", its vault not open" } else { "" }
        ));
        if ids.is_empty() {
            return; // closed by hand meanwhile
        }
        self.schedule.stop_closing.borrow_mut().extend(ids.iter().copied());
        self.close_accounts(&ids, false); // gracefully, and marked as closed on purpose: no auto-rejoin
        self.notify(
            &l::p(ids.len() as i64, "Scheduled preset {0} finished: closing {1} account.", "Scheduled preset {0} finished: closing {1} accounts.", &[&run.name, &ids.len()]),
            "info",
            "scheduleStop",
            run.preset_id,
            true,
            None,
        );
        self.push_state();
    }

    /// The account's client is gone for good (closed by the person, a drop not rejoined, never showed): no scheduled stop
    /// owns it, so a client opened by hand later isn't closed by one. Through an auto-rejoin the stop keeps it.
    pub fn forget_scheduled_open(&self, account_id: Guid) {
        let mut changed = false;
        for run in self.schedule.scheduled_runs.borrow_mut().iter_mut() {
            if let Some(i) = run.opened.iter().position(|id| *id == account_id) {
                run.opened.remove(i);
                changed = true;
            }
        }
        if changed && self.note_scheduled_runs() {
            self.persist_soon();
        }
    }

    /// Writes runs with a stop still to come into the vault, so an exit, update or crash doesn't leave their clients open
    /// for good: the next unlock arms them again (see resume_schedules). True when the list changed, i.e. the vault needs saving.
    pub fn note_scheduled_runs(&self) -> bool {
        let Some(vault) = self.held_vault() else {
            return false;
        };
        let tick = clock::tick();
        let now = clock::utc_now();
        let runs: Vec<ScheduledRunState> = self
            .schedule
            .scheduled_runs
            .borrow()
            .iter()
            .filter(|r| of_vault(r, &vault))
            .map(|r| ScheduledRunState {
                preset_id: r.preset_id,
                name: r.name.clone(),
                opened: r.opened.clone(),
                stop_at: r.stop_at_tick.map(|at| now.plus_secs((at - tick) as f64 / 1000.0)),
                stop_at_minute: r.stop_at_minute.clone(),
                ..ScheduledRunState::default()
            })
            .collect();
        self.vault_mut(|d| {
            let noted = &d.scheduled_runs;
            let same = noted.len() == runs.len()
                && noted.iter().zip(runs.iter()).all(|(a, b)| {
                    a.preset_id == b.preset_id
                        && a.stop_at_minute == b.stop_at_minute
                        && a.opened == b.opened
                        && a.stop_at.is_none() == b.stop_at.is_none()
                        // An "after" stop is written as a time; one re-read on each save would move it by milliseconds.
                        && match (a.stop_at, b.stop_at) {
                            (Some(x), Some(y)) => x.secs_since(y).abs() < 5.0,
                            _ => true,
                        }
                });
            if same {
                return false;
            }
            d.scheduled_runs = runs;
            true
        })
        .unwrap_or(false)
    }

    /// After unlocking: the runs the last app run wrote are armed again (their clients were adopted by AdoptOpenSessions),
    /// even if their preset was deleted; a stop that fired while locked wins. The current minute may run again, and runs missed
    /// since the app started or the vault locked are reported once.
    pub fn resume_schedules(&self, vault: &Arc<Vault>) {
        let tick = clock::tick();
        let now = clock::utc_now();
        let (saved_runs, presets, accounts): (Vec<ScheduledRunState>, Vec<(Guid, String)>, HashSet<Guid>) = {
            let d = vault.data();
            (d.scheduled_runs.clone(), d.settings.presets.iter().map(|p| (p.id, p.name.clone())).collect(), d.accounts.iter().map(|a| a.id).collect())
        };
        for saved in &saved_runs {
            let armed = self.schedule.scheduled_runs.borrow().iter().any(|r| r.preset_id == saved.preset_id && of_vault(r, vault));
            if armed || self.schedule.stopped_while_locked.borrow().contains(&saved.preset_id) {
                continue;
            }
            if saved.stop_at.is_none() && saved.stop_at_minute.is_none() {
                continue;
            }
            let name = presets
                .iter()
                .find(|p| p.0 == saved.preset_id)
                .map(|p| p.1.clone())
                .unwrap_or_else(|| if !saved.name.is_empty() { saved.name.clone() } else { l::t("(deleted)") });
            let mut run = ScheduledRun::new(
                saved.preset_id,
                vault.file_path().to_string_lossy().to_string(),
                name,
                saved.opened.iter().filter(|id| accounts.contains(id)).copied().collect(),
            );
            run.stop_at_tick = saved.stop_at.map(|at| tick + (at.secs_since(now) * 1000.0).max(0.0) as i64);
            run.stop_at_minute = saved.stop_at_minute.clone();
            self.schedule.scheduled_runs.borrow_mut().push(run);
        }
        let armed = self.schedule.scheduled_runs.borrow().iter().filter(|r| of_vault(r, vault)).count();
        if armed > 0 {
            log::note(&format!("Schedule: {armed} saved stop(s) armed again after the unlock."));
        }
        // Only this vault's: a stop of another vault's run that fired meanwhile is for that vault's next unlock.
        {
            let mut stopped = self.schedule.stopped_while_locked.borrow_mut();
            for r in &saved_runs {
                stopped.remove(&r.preset_id);
            }
        }
        if self.note_scheduled_runs() {
            self.persist_soon();
        }
        // The minute spent on the lock screen counted as seen: without this the run due now is skipped (LastRun prevents twice).
        *self.schedule.last_schedule_minute.borrow_mut() = String::new();
        self.schedule.last_schedule_check.set(None);
        self.report_missed_schedules(vault);
    }

    /// Runs due strictly between two checks (the current minute runs as usual), each preset's latest said once.
    pub fn report_schedule_gap(&self, vault: &Arc<Vault>, from: Wall, to: Wall) {
        let from_minute = from.minute();
        let to_minute = to.minute();
        let presets: Vec<LaunchPreset> = vault.data().settings.presets.clone();
        for preset in presets {
            let Some(s) = preset.schedule.as_ref().filter(|s| s.enabled) else {
                continue;
            };
            let Some(time) = try_time(&s.time) else {
                continue;
            };
            for d in 0..=7 {
                let at = to_minute.date().add_days(-d).add_secs(time);
                if at <= from_minute {
                    break;
                }
                if at >= to_minute || !due_at(s, at) || s.last_run == minute_key(at) {
                    continue;
                }
                log::note(&format!("Schedule: preset {} missed {}, the PC was asleep or busy.", short(&preset.id), at.hh_mm()));
                self.notify(&l::f("{0} didn't run at {1}: the PC was asleep or busy.", &[&preset.name, &at.hh_mm()]), "warn", "scheduleMissed", preset.id, true, None);
                break;
            }
        }
    }

    /// Runs due between the app's start (or the lock) and now that didn't happen, each said once.
    pub fn report_missed_schedules(&self, vault: &Arc<Vault>) {
        let now = Wall::now();
        let this_minute = now.minute();
        let locked_since = self.schedule.locked_since.get();
        let presets: Vec<LaunchPreset> = vault.data().settings.presets.clone();
        for preset in presets {
            let Some(s) = preset.schedule.as_ref().filter(|s| s.enabled) else {
                continue;
            };
            let Some(time) = try_time(&s.time) else {
                continue;
            };
            // Only the latest due minute, and only one that is already over: this minute's run is still to come.
            for d in 0..=1 {
                let at = now.date().add_days(-d).add_secs(time);
                if at >= this_minute || !due_at(s, at) {
                    continue;
                }
                let minute = minute_key(at);
                if at >= locked_since && s.last_run != minute && self.schedule.missed_reported.borrow_mut().insert((preset.id, minute.clone())) {
                    log::note(&format!("Schedule: preset {} missed {}, Bloxkeep was locked.", short(&preset.id), &minute[11..]));
                    self.notify(&l::f("{0} didn't run at {1}: Bloxkeep was locked.", &[&preset.name, &at.hh_mm()]), "warn", "scheduleMissed", preset.id, true, None);
                }
                break;
            }
        }
        self.schedule.missed_reported.borrow_mut().clear();
    }

    /// Before the vault locks: the runs due next, so each can be reported as missed while it stays locked.
    pub fn remember_schedules_for_lock(&self) {
        self.schedule.locked_since.set(Wall::now());
        let list: Vec<(Guid, String, String)> = self
            .vault(|d| {
                d.settings
                    .presets
                    .iter()
                    .filter_map(|p| next_run(p.schedule.as_ref()).map(|next| (p.id, p.name.clone(), minute_key(next))))
                    .collect()
            })
            .unwrap_or_default();
        *self.schedule.locked_schedules.borrow_mut() = list;
    }

    // ───────────────────────────── launch order ─────────────────────────────

    /// Every account id in launch order: the saved order first, then accounts added since, as listed.
    pub fn effective_launch_order(d: &VaultData) -> Vec<Guid> {
        let known: HashSet<Guid> = d.accounts.iter().map(|a| a.id).collect();
        let mut order: Vec<Guid> = Vec::new();
        for id in &d.settings.launch_order {
            if known.contains(id) && !order.contains(id) {
                order.push(*id);
            }
        }
        let mut seen: HashSet<Guid> = order.iter().copied().collect();
        order.extend(d.accounts.iter().map(|a| a.id).filter(|id| seen.insert(*id)));
        order
    }

    pub fn launch_order_dto(&self) -> Vec<Guid> {
        self.vault(App::effective_launch_order).unwrap_or_default()
    }

    /// The chosen accounts in the global launch order (Play with several selected).
    pub fn in_launch_order(&self, ids: &[Guid]) -> Vec<Account> {
        let wanted: HashSet<Guid> = ids.iter().copied().collect();
        let order = self.vault(App::effective_launch_order).unwrap_or_default();
        order.into_iter().filter(|id| wanted.contains(id)).filter_map(|id| self.find_account(id)).collect()
    }

    pub fn set_launch_order(&self, ids: &[Guid]) -> Res<Value> {
        self.vault_mut(|d| {
            let known: HashSet<Guid> = d.accounts.iter().map(|a| a.id).collect();
            let mut order: Vec<Guid> = ids.iter().filter(|id| known.contains(id)).copied().collect();
            let rest: Vec<Guid> = App::effective_launch_order(d).into_iter().filter(|id| !order.contains(id)).collect();
            order.extend(rest); // anything the page didn't list stays last
            d.settings.launch_order = order;
        })?;
        self.persist();
        self.push_state();
        Ok(Value::Null)
    }
}
