// AppController.Presets.cs: launch presets: saved accounts, game and server kind, run from the dock or by schedule
// (schedule.rs). Servers other than "any" and a fixed Job ID are picked when the preset runs, so a scheduled run needs
// no page.
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use serde::Serialize;
use serde_json::{Value, json};

use super::App;
// The server picker a preset hands the launch (LaunchAccountsAsync's pickServer).
use super::launching::PickServer;
use super::controller::{id_of, ids_of, long_of, opt_bool, opt_str, str_of};
use super::schedule::{next_run, parse_schedule, schedule_dto};
use crate::core::{
    anti_afk,
    base::{Cancel, Fail, Guid, Res, Stamp, new_guid},
    discord_webhook,
    game_target::{FollowedPlayer, GameTarget},
    input_turn, l, log,
    models::{Account, AppSettings, LaunchPreset, PresetSchedule},
    multi_instance,
    roblox_api::{self, Presence, RobloxUser, ServerInfo},
    roblox_log_watcher, roblox_processes, rt,
    vault::Vault,
};

/// What a launch did. `need_confirm`: nothing was opened yet: `message` says what to look at before opening anyway.
/// `notice`: something the person should know that isn't an error (no server had room). `skipped`: accounts left as
/// they were because they were already open where asked. `asks`: with need_confirm, the warning kinds in the message:
/// the page offers each one's fix, and "Don't ask again" for skippable ones.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub needs_confirm: bool,
    pub message: Option<String>,
    pub launched: i32,
    pub errors: Option<Vec<String>>,
    pub cancelled: bool,
    pub notice: Option<String>,
    pub skipped: Option<Vec<String>>,
    pub asks: Option<Vec<String>>,
    pub refused: Option<Vec<Guid>>,
}

pub const MAX_PRESETS: usize = 50;
pub const MAX_PRESET_NAME: usize = 60;
/// "each…": every account to a public server of its own, emptiest, fullest or any first (see server_hop.rs).
pub const PRESET_SERVERS: [&str; 10] = ["any", "most", "fewest", "ping", "random", "eachFewest", "eachMost", "eachRandom", "job", "follow"];
/// `LaunchPreset::profile` for the light AFK setup (RobloxGameSettings.LightAfk).
pub const AFK_PROFILE: &str = "afk";
/// The choices the form offers for `LaunchPreset::restart_hours` (0 = never).
pub const RESTART_CHOICES: [i32; 4] = [2, 4, 6, 12];
/// Roblox gives its window its size as it finishes starting up: a window not in a game yet is left this long first.
pub const MAXIMISE_WITHOUT_LOG: Duration = Duration::from_secs(10);
/// At least this long after the last client started, so a preset's windows (opened together) aren't all reopened at once.
pub const RESTART_SPACING: Duration = Duration::from_secs(60);
/// A client asked to close for a restart that is still there after this is left alone (tried again later).
pub const RESTART_CLOSE_TIMEOUT: Duration = Duration::from_secs(60);
/// The choices of Settings › Staying in game › Restart a heavy window, in MB (0 = never).
pub const RESTART_ABOVE_CHOICES: [i32; 5] = [0, 3072, 4096, 6144, 8192];
/// A window above that much for this long is restarted: not for a moment's peak (a teleport, a big map loading).
pub const HEAVY_FOR: Duration = Duration::from_secs(3 * 60);


#[derive(Default)]
pub struct PresetsState {
    /// Windows of a light-AFK preset already minimised once in their game (by process id).
    pub afk_minimised: RefCell<HashSet<i32>>,
    /// Windows maximised for Settings' "Maximise" (by process id), and at which point of starting up.
    pub maximised: RefCell<HashMap<i32, String>>,
    /// When each window Bloxkeep opened was first seen (by process id): one whose client writes no log is maximised a while after.
    pub window_first_seen: RefCell<HashMap<i32, Stamp>>,
    /// Restarts under way: the account's client is closing, to open again into this target (see QueueRejoin).
    pub restarts: RefCell<HashMap<Guid, (GameTarget, Stamp)>>,
    /// Since when each client has been above Restart a heavy window's choice.
    pub heavy_since: RefCell<HashMap<i32, Stamp>>,
    /// Windows whose due restart was logged as held back (once each).
    pub restart_held_noted: RefCell<HashSet<i32>>,
}

/// The first 8 characters of an id, as the log writes it (`id.ToString()[..8]`).
pub(super) fn short(id: &Guid) -> String {
    id.to_string()[..8].to_string()
}

pub(super) fn secs(d: Duration) -> f64 {
    d.as_secs_f64()
}

/// Two looks at the open vault are the same vault (C#'s `_vault == vault`).
pub(super) fn same_vault(a: &Option<Arc<Vault>>, b: &Option<Arc<Vault>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

/// A number below `n` (Random.Shared.Next(n)).
pub(super) fn random_below(n: usize) -> usize {
    use std::hash::{BuildHasher, Hasher};
    if n == 0 {
        return 0;
    }
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
    (h.finish() % n as u64) as usize
}

/// UTF-16 units, as C#'s string.Length counts them.
pub(super) fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

impl App {
    /// The vault open now (C#'s `_vault`), to tell later whether it is still the same one.
    pub(super) fn held_vault(&self) -> Option<Arc<Vault>> {
        self.controller.vault.borrow().clone()
    }

    pub(crate) fn preset_dto(&self, p: &LaunchPreset) -> Value {
        let icon_url = App::preset_place_id(p).and_then(|place| self.controller.games.borrow().get(&place).cloned().flatten().and_then(|g| g.icon_url));
        json!({
            "id": p.id,
            "name": p.name,
            "accountIds": p.account_ids,
            "placeInput": p.place_input,
            "gameName": p.game_name,
            "server": p.server,
            "jobId": p.job_id,
            "followPlayer": p.follow_player,
            "delaySeconds": p.delay_seconds,
            "schedule": schedule_dto(p.schedule.as_ref()),
            "nextRun": next_run(p.schedule.as_ref()).map(|w| w.to_stamp()),
            // Only from what is already known: the Presets tab asks for the missing ones (presetIcons).
            "iconUrl": icon_url,
            "lastRun": p.last_run_at,
            "antiAfk": p.anti_afk,
            "autoRejoin": p.auto_rejoin,
            "rejoinWith": p.rejoin_with,
            "profile": p.profile,
            "restartHours": p.restart_hours,
            "arrange": p.arrange,
            // The link stays in the app, like Settings' one: the page only gets its masked form.
            "discord": if !p.discord_webhook.is_empty() { Some(discord_webhook::mask(&p.discord_webhook)) } else { None },
            // Its windows open now (or being opened again by auto-rejoin), for the Presets tab's Close.
            "running": self.preset_windows(p.id).len(),
        })
    }

    /// Also a window still to come after its wait (`late`) and a rejoin under way: Close stops them too.
    pub(crate) fn preset_windows(&self, preset_id: Guid) -> Vec<Guid> {
        let mut ids: Vec<Guid> = Vec::new();
        for (id, (_, launch)) in self.controller.running.borrow().iter() {
            if launch.preset_id == Some(preset_id) {
                ids.push(*id);
            }
        }
        for l in self.controller.pending.borrow().iter().chain(self.launching.late.borrow().iter()) {
            if l.preset_id == Some(preset_id) {
                ids.push(l.account_id);
            }
        }
        for r in self.controller.rejoin_queue.borrow().iter() {
            if r.preset_id == Some(preset_id) {
                ids.push(r.account_id);
            }
        }
        if let Some(under_way) = self.rejoin.rejoin_in_flight.borrow().as_ref() {
            if under_way.preset_id == Some(preset_id) {
                ids.push(under_way.account_id);
            }
        }
        let mut seen = HashSet::new();
        ids.retain(|id| seen.insert(*id));
        ids
    }

    /// The Presets tab's Close: the windows this preset opened, gracefully and on purpose (no auto-rejoin).
    pub fn close_preset_windows(&self, preset_id: Guid) -> Res<Value> {
        self.vault(|_| ())?;
        let ids = self.preset_windows(preset_id);
        log::note(&format!("Preset {}: Close, {} account(s) open or opening.", short(&preset_id), ids.len()));
        Ok(if ids.is_empty() { json!(0) } else { self.close_accounts(&ids, false) })
    }

    /// The game's place id, or None without a game or for a share link (its place is only known when it opens).
    pub fn preset_place_id(p: &LaunchPreset) -> Option<i64> {
        if p.place_input.is_empty() {
            return None;
        }
        match GameTarget::try_parse(&p.place_input, "") {
            Ok(target) if target.share_code.is_none() && target.place_id > 0 => Some(target.place_id),
            _ => None,
        }
    }

    pub async fn save_preset(self: &Rc<Self>, a: &Value) -> Res<Value> {
        let vault = self.held_vault().ok_or_else(|| Fail::user(l::t("Bloxkeep is locked.")))?;
        let mut name = str_of(a, "name").trim().to_string();
        if name.is_empty() {
            return Err(Fail::user(l::t("Give the preset a name.")));
        }
        if utf16_len(&name) > MAX_PRESET_NAME {
            name = App::cut_text(&name, MAX_PRESET_NAME).trim_end().to_string();
        }
        let known: HashSet<Guid> = self.vault(|d| d.accounts.iter().map(|acc| acc.id).collect())?;
        let ids: Vec<Guid> = ids_of(a).into_iter().filter(|id| known.contains(id)).collect();
        if ids.is_empty() {
            return Err(Fail::user(l::t("Select at least one account.")));
        }
        let place = str_of(a, "placeInput").trim().to_string();
        // Without a game there's no server to choose; following a player needs none (their game is used).
        let requested = str_of(a, "server");
        let server = if requested == "follow" {
            "follow".to_string()
        } else if place.is_empty() {
            "any".to_string()
        } else if PRESET_SERVERS.contains(&requested.as_str()) {
            requested.clone()
        } else {
            "any".to_string()
        };
        let job = if server == "job" { str_of(a, "jobId").trim().to_string() } else { String::new() };
        if server == "job" && job.is_empty() {
            return Err(Fail::user(l::t("Enter the server's Job ID.")));
        }
        // "Rejoin into": only with a game, and not for a preset that follows a player (its rejoins follow that player).
        let mut rejoin_with = str_of(a, "rejoinWith").trim().to_string();
        let with_ok = rejoin_with == "any" || Guid::parse_str(&rejoin_with).is_ok_and(|with| ids.contains(&with));
        if place.is_empty() || server == "follow" || !with_ok {
            rejoin_with = String::new();
        }
        let delay = if a.get("delaySeconds").is_some_and(|v| v.is_number()) { Some(long_of(a, "delaySeconds").clamp(0, 120) as i32) } else { None };
        let profile = if str_of(a, "profile") == AFK_PROFILE { AFK_PROFILE.to_string() } else { String::new() };
        // A restart opens the same game again (or follows again); a preset without a game has nothing to restart.
        let restart_hours =
            if (!place.is_empty() || server == "follow") && RESTART_CHOICES.contains(&(long_of(a, "restartHours") as i32)) { long_of(a, "restartHours") as i32 } else { 0 };
        let form_id = id_of(a);
        let old_schedule = self.vault(|d| d.settings.presets.iter().find(|p| p.id == form_id).and_then(|p| p.schedule.clone()))?;
        let mut schedule = parse_schedule(a, old_schedule.as_ref())?;
        // Its own Discord channel: absent keeps the one saved, "" goes back to Settings' one, a link replaces it.
        let mut discord = opt_str(a, "discordWebhook").map(|d| d.trim().to_string());
        if let Some(d) = discord.clone().filter(|d| !d.is_empty()) {
            discord = Some(discord_webhook::try_normalize(Some(&d)).ok_or_else(|| {
                Fail::user(l::t(
                    "That isn't a Discord webhook link. It looks like https://discord.com/api/webhooks/… (Channel settings › Integrations › Webhooks › Copy Webhook URL).",
                ))
            })?);
        }

        // Checked now, so a typo shows while saving rather than when the preset runs.
        let mut followed: Option<RobloxUser> = None;
        if server == "follow" {
            let who = str_of(a, "followPlayer").trim().trim_start_matches('@').trim().to_string();
            followed = Some(self.look_up_player(&who).await?.ok_or_else(|| Fail::user(l::f("Roblox has no player called {0}.", &[&who])))?);
        }
        let mut game_name = String::new();
        if !place.is_empty() {
            let target = GameTarget::try_parse(&place, &job).map_err(Fail::user)?;
            if target.share_code.is_none() {
                if let Some(info) = self.get_game_info(target.place_id, false).await? {
                    game_name = info.name;
                }
            }
        }
        if !same_vault(&self.held_vault(), &Some(vault.clone())) {
            return Err(Fail::user(l::t("Bloxkeep was locked.")));
        }

        let live_schedule = self.vault(|d| d.settings.presets.iter().find(|p| p.id == form_id).map(|p| p.schedule.clone()))?;
        App::catch_up_schedule(schedule.as_mut(), live_schedule.clone().flatten().as_ref());
        let anti_afk = opt_bool(a, "antiAfk");
        let auto_rejoin = opt_bool(a, "autoRejoin");
        let arrange = opt_bool(a, "arrange");
        let schedule_for_save = schedule.clone();
        // (preset id, its Discord link changed, its schedule changed today)
        let saved: Res<(Guid, bool, bool)> = self.vault_mut(|d| {
            let presets = &mut d.settings.presets;
            let index = match presets.iter().position(|p| p.id == form_id) {
                Some(i) => i,
                None => {
                    if presets.len() >= MAX_PRESETS {
                        return Err(Fail::user(l::f("You can save up to {0} presets.", &[&MAX_PRESETS])));
                    }
                    presets.push(LaunchPreset::default());
                    presets.len() - 1
                }
            };
            let preset = &mut presets[index];
            preset.name = name.clone();
            preset.account_ids = ids.clone();
            preset.place_input = place.clone();
            preset.game_name = game_name.clone();
            preset.server = server.clone();
            preset.job_id = job.clone();
            preset.follow_player = followed.as_ref().map(|f| f.name.clone()).unwrap_or_default();
            preset.follow_user_id = followed.as_ref().map(|f| f.id).unwrap_or(0);
            preset.delay_seconds = delay;
            preset.anti_afk = anti_afk;
            preset.auto_rejoin = auto_rejoin;
            preset.rejoin_with = rejoin_with.clone();
            preset.profile = profile.clone();
            preset.restart_hours = restart_hours;
            preset.arrange = arrange;
            let mut route_changed = false;
            if let Some(discord) = discord.as_ref() {
                if *discord != preset.discord_webhook {
                    preset.discord_webhook = discord.clone();
                    route_changed = true;
                }
            }
            // A schedule changed today (another time, re-enabled) may run today: the "not twice within a day" guard
            // against a clock set back is for the schedule as it was.
            let old = preset.schedule.as_ref();
            let changed = match (old, schedule_for_save.as_ref()) {
                (_, None) => false,
                (None, Some(_)) => true,
                (Some(old), Some(s)) => old.enabled != s.enabled || old.time != s.time || old.date != s.date || old.days != s.days,
            };
            preset.schedule = schedule_for_save.clone();
            Ok((preset.id, route_changed, changed))
        })?;
        let (preset_id, route_changed, schedule_changed_today) = saved?;
        if route_changed {
            self.discord_route_changed(preset_id); // what waited for the old link isn't sent to it
        }
        if schedule_changed_today {
            self.schedule.scheduled_start_tick.borrow_mut().remove(&preset_id);
        }
        self.schedule_changed(preset_id, schedule.as_ref());
        self.persist();
        self.push_state();
        Ok(json!(preset_id))
    }

    /// The form's schedule, read before the player and game lookups (seconds), brought up to date with a run that
    /// happened meanwhile: saved as read, a one-off due that minute was on again with no last run and ran twice (2026-10-30
    /// review); a weekly one ran again after the save.
    pub fn catch_up_schedule(parsed: Option<&mut PresetSchedule>, live: Option<&PresetSchedule>) {
        let (Some(parsed), Some(live)) = (parsed, live) else {
            return;
        };
        if live.last_run == parsed.last_run {
            return;
        }
        parsed.last_run = live.last_run.clone();
        // A one-off that just had its turn is over, as the run left it.
        if parsed.enabled && !parsed.date.is_empty() && !live.enabled && live.date == parsed.date && live.time == parsed.time {
            parsed.enabled = false;
        }
    }

    /// At most `max` UTF-16 units, cut between whole text elements (never inside an emoji or accent).
    pub fn cut_text(text: &str, max: usize) -> String {
        use unicode_segmentation::UnicodeSegmentation;
        if utf16_len(text) <= max {
            return text.to_string();
        }
        let mut length = 0;
        let mut bytes = 0;
        for element in text.graphemes(true) {
            let units = utf16_len(element);
            if length + units > max {
                break;
            }
            length += units;
            bytes += element.len();
        }
        text[..bytes].to_string()
    }

    pub fn delete_preset(&self, id: Guid) -> Res<Value> {
        self.vault_mut(|d| d.settings.presets.retain(|p| p.id != id))?;
        self.persist();
        self.push_state();
        Ok(Value::Null)
    }

    /// A copy right after the original (and its earlier copies), named "… (copy)" or numbered. Its schedule comes along
    /// off, so the two don't start the same accounts at the same minute.
    pub fn duplicate_preset(&self, id: Guid) -> Res<Value> {
        let copied: Res<Guid> = self.vault_mut(|d| {
            let presets = &mut d.settings.presets;
            let index = presets.iter().position(|p| p.id == id).ok_or_else(|| Fail::user(l::t("Preset not found.")))?;
            if presets.len() >= MAX_PRESETS {
                return Err(Fail::user(l::f("You can save up to {0} presets.", &[&MAX_PRESETS])));
            }
            let source = presets[index].clone();
            let names: HashSet<String> = presets.iter().map(|p| p.name.to_lowercase()).collect();
            let mut name = String::new();
            let mut copy_names: HashSet<String> = HashSet::new();
            for n in 1..=MAX_PRESETS + 1 {
                let suffix = if n == 1 { l::t(" (copy)") } else { l::f(" (copy {0})", &[&n]) };
                // The original's name is shortened, not the suffix, so a 60-character name still reads as a copy.
                let candidate = App::cut_text(&source.name, MAX_PRESET_NAME.saturating_sub(utf16_len(&suffix))).trim_end().to_string() + &suffix;
                copy_names.insert(candidate.to_lowercase());
                if name.is_empty() && !names.contains(&candidate.to_lowercase()) {
                    name = candidate;
                }
            }
            // After the copies already made of it, so "(copy 2)" comes under "(copy)", not above it.
            let mut at = index + 1;
            while at < presets.len() && copy_names.contains(&presets[at].name.to_lowercase()) {
                at += 1;
            }
            // Everything the original has (a followed player too), as its own preset that hasn't run yet.
            let mut copy = source.json_clone()?;
            copy.id = new_guid();
            copy.name = name;
            copy.last_run_at = None;
            if let Some(s) = copy.schedule.as_mut() {
                s.enabled = false;
                s.last_run = String::new();
            }
            let copy_id = copy.id;
            presets.insert(at, copy);
            Ok(copy_id)
        })?;
        let copy_id = copied?;
        self.persist();
        self.push_state();
        Ok(json!(copy_id))
    }

    /// The dock menu and the Presets tab list presets in this order; any the page didn't send stay, last.
    pub fn set_preset_order(&self, ids: &[Guid]) -> Res<Value> {
        self.vault_mut(|d| {
            let presets = std::mem::take(&mut d.settings.presets);
            let mut order: Vec<LaunchPreset> = Vec::new();
            let mut taken: HashSet<Guid> = HashSet::new();
            for id in ids {
                if taken.contains(id) {
                    continue;
                }
                if let Some(p) = presets.iter().find(|p| p.id == *id) {
                    taken.insert(*id);
                    order.push(p.clone());
                }
            }
            for p in presets {
                if !taken.contains(&p.id) {
                    order.push(p);
                }
            }
            d.settings.presets = order;
        })?;
        self.persist();
        self.push_state();
        Ok(Value::Null)
    }

    /// The Presets tab's schedule switch; turning it on checks it like the form (a passed one-off can't be turned on).
    pub fn set_preset_schedule(&self, id: Guid, enabled: bool) -> Res<Value> {
        let preset = self.vault(|d| d.settings.presets.iter().find(|p| p.id == id).cloned())?.ok_or_else(|| Fail::user(l::t("Preset not found.")))?;
        let Some(s) = preset.schedule.clone() else {
            return Err(Fail::user(l::t("This preset has no schedule. Edit it to add one.")));
        };
        if s.enabled != enabled {
            log::note(&format!("Preset {}: schedule switched {}.", short(&id), if enabled { "on" } else { "off" }));
            let form = json!({
                "schedule": { "enabled": enabled, "time": s.time, "days": s.days, "date": s.date, "stop": s.stop, "stopHours": s.stop_hours, "stopTime": s.stop_time },
            });
            let parsed = parse_schedule(&form, Some(&s))?;
            let stored = parsed.clone();
            self.vault_mut(|d| {
                if let Some(p) = d.settings.presets.iter_mut().find(|p| p.id == id) {
                    p.schedule = stored;
                }
            })?;
            self.schedule.scheduled_start_tick.borrow_mut().remove(&id); // as in the form: a schedule turned on again may run today
            self.schedule_changed(id, parsed.as_ref());
            self.persist();
        }
        self.push_state();
        Ok(Value::Null)
    }

    /// Looks up the games of presets whose icon isn't known yet (the Presets tab shows them), then pushes the state.
    pub async fn preset_icons(self: &Rc<Self>) -> Res<Value> {
        let place_ids: Vec<i64> = self.vault(|d| d.settings.presets.iter().filter_map(App::preset_place_id).collect())?;
        let mut places: Vec<(i64, i64)> = Vec::new();
        {
            let games = self.controller.games.borrow();
            for id in place_ids {
                if games.get(&id).cloned().flatten().and_then(|g| g.icon_url).is_none() && !places.iter().any(|p| p.0 == id) {
                    places.push((id, 0));
                }
            }
        }
        if places.is_empty() {
            return Ok(Value::Null);
        }
        self.load_games(places, false).await?;
        if self.held_vault().is_some() {
            self.push_state_if_changed();
        }
        Ok(Value::Null)
    }

    /// The page's command: the same run, typed for the dispatcher (the schedule calls `start_preset`).
    pub async fn run_preset(self: &Rc<Self>, id: Guid, force: bool) -> Res<Value> {
        let result = self.start_preset(id, force).await?;
        Ok(serde_json::to_value(result)?)
    }

    /// Opens the preset's remaining accounts in order through the dock's Play launch (same waits, warnings, Stop, progress).
    pub async fn start_preset(self: &Rc<Self>, id: Guid, force: bool) -> Res<LaunchResult> {
        let vault = self.held_vault().ok_or_else(|| Fail::user(l::t("Bloxkeep is locked.")))?;
        let (preset, launch_delay) = self.vault(|d| (d.settings.presets.iter().find(|p| p.id == id).cloned(), d.settings.launch_delay_seconds))?;
        let preset = preset.ok_or_else(|| Fail::user(l::t("Preset not found.")))?;
        let mut distinct: Vec<Guid> = Vec::new();
        for a in &preset.account_ids {
            if !distinct.contains(a) {
                distinct.push(*a);
            }
        }
        let accounts: Vec<Account> = distinct.iter().filter_map(|a| self.find_account(*a)).collect();
        let tag = format!("Preset {}", short(&id));
        let deleted = if distinct.len() > accounts.len() { format!(" ({} deleted)", distinct.len() - accounts.len()) } else { String::new() };
        let busy = self.controller.launch_cts.borrow().is_some() || self.tray.opening_home.get();
        log::note(&format!(
            "{tag}: run, {} account(s){deleted}, server {}{}{}.",
            accounts.len(),
            preset.server,
            if force { ", confirmed" } else { "" },
            if busy { ", but a launch is under way" } else { "" }
        ));
        if accounts.is_empty() {
            return Err(Fail::user(l::t("This preset has no accounts left.")));
        }
        self.check_can_launch(&accounts)?;
        // A preset that follows a player takes the launch before looking for them (network requests): a Play or a scheduled run
        // meanwhile is told one is under way, and Stop or Close all stops this one. Taken only once the player was found, two
        // launches ran side by side and Stop stopped only the newer.
        let mut slot: Option<Cancel> = None;
        let run: Res<LaunchResult> = async {
            let mut target: Option<GameTarget> = None;
            if preset.server == "follow" {
                let s = Cancel::new();
                *self.controller.launch_cts.borrow_mut() = Some(s.clone());
                slot = Some(s.clone());
                let found = self.preset_follow_target(vault.clone(), &preset, &accounts, &s).await;
                let stopped = match &found {
                    Err(e) => roblox_api::is_fail_kind(e, roblox_api::TASK_CANCELED_EXCEPTION) && s.is_cancelled(),
                    Ok(_) => s.is_cancelled(),
                };
                if stopped {
                    log::note(&format!("{tag}: stopped while looking for its player."));
                    self.progress(None);
                    self.push_state();
                    return Ok(LaunchResult { cancelled: true, ..Default::default() });
                }
                target = Some(found?);
            } else if !preset.place_input.is_empty() {
                let job = if preset.server == "job" { preset.job_id.as_str() } else { "" };
                target = Some(GameTarget::try_parse(&preset.place_input, job).map_err(Fail::user)?);
            }

            // A link that already names a server (or a private one) keeps it.
            let server = preset.server.clone();
            let plain = target.as_ref().is_some_and(|t| t.job_id.is_none() && t.link_code.is_none() && t.share_code.is_none() && t.access_code.is_none());
            let pick = plain && matches!(server.as_str(), "most" | "fewest" | "ping" | "random");
            let notice: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
            let apart = plain && server.starts_with("each");
            let pick_server: Option<PickServer> = if pick {
                let app = self.clone();
                let server = server.clone();
                let notice = notice.clone();
                Some(Box::new(move |t: GameTarget, party: i32, ct: Cancel| -> futures::future::LocalBoxFuture<'static, Res<GameTarget>> {
                    let app = app.clone();
                    let server = server.clone();
                    let notice = notice.clone();
                    Box::pin(async move {
                        let (picked, note) = app.pick_server(t, &server, party, &ct).await?;
                        *notice.borrow_mut() = note;
                        Ok(picked)
                    })
                }))
            } else {
                None
            };
            let each = if apart { Some(self.servers_apart(&server[4..].to_lowercase())) } else { None };
            let mut result = self
                .launch_accounts(vault.clone(), accounts.clone(), target, force, preset.delay_seconds.unwrap_or(launch_delay), pick_server, Some(preset.id), each, slot.clone())
                .await?;
            let notice_now = notice.borrow().clone();
            let what = if result.needs_confirm {
                format!("held for a confirmation ({})", result.asks.clone().unwrap_or_default().join(", "))
            } else {
                format!(
                    "{} opened, {} not opened, {} skipped",
                    result.launched,
                    result.errors.as_ref().map_or(0, |e| e.len()),
                    result.skipped.as_ref().map_or(0, |s| s.len())
                )
            };
            log::note(&format!(
                "{tag}: {what}{}{}.",
                if result.cancelled { ", stopped" } else { "" },
                if notice_now.is_some() { ", Roblox picks the server" } else { "" }
            ));
            // Shown on the Presets tab; a run that opened nothing (all already open, or all failed) doesn't count.
            if result.launched > 0 && same_vault(&self.held_vault(), &Some(vault.clone())) {
                let _ = self.vault_mut(|d| {
                    if let Some(p) = d.settings.presets.iter_mut().find(|p| p.id == id) {
                        p.last_run_at = Some(Stamp::utc_now());
                    }
                });
                self.persist_soon();
                self.push_state();
            }
            result.notice = notice_now;
            Ok(result)
        }
        .await;
        // The launch has given it back by now, unless it refused before starting (or the player wasn't found).
        if let Some(slot) = slot {
            let mine = self.controller.launch_cts.borrow().as_ref().is_some_and(|c| c.same(&slot));
            if mine {
                *self.controller.launch_cts.borrow_mut() = None;
            }
        }
        run
    }

    /// Where a follow-a-player preset opens: the player's server now, as Roblox shows it to the preset's accounts (failing
    /// them, the other signed-in ones); rejoins follow too. When Roblox won't show one (offline, website, Studio, hidden), the
    /// preset doesn't run and says why.
    pub async fn preset_follow_target(self: &Rc<Self>, vault: Arc<Vault>, preset: &LaunchPreset, accounts: &[Account], ct: &Cancel) -> Res<GameTarget> {
        if preset.follow_user_id <= 0 {
            return Err(Fail::user(l::t("This preset joins a player who isn't saved with it. Edit the preset and type the player's name again.")));
        }
        let player = if !preset.follow_player.is_empty() { preset.follow_player.clone() } else { preset.follow_user_id.to_string() };
        let mine: HashSet<Guid> = accounts.iter().map(|a| a.id).collect();
        let others: Vec<Account> = self.vault(|d| d.accounts.iter().filter(|o| !mine.contains(&o.id)).cloned().collect())?;
        let viewers: Vec<Account> = accounts.iter().cloned().chain(others).filter(App::can_ask_presence).collect();
        if viewers.is_empty() {
            return Err(Fail::user(l::t("None of the selected accounts is signed in, so Bloxkeep can't ask Roblox where this player is. Sign them in again first.")));
        }
        self.progress(Some(&l::f("Looking for {0}…", &[&player])));
        let (seen, failed): (Option<Presence>, Option<Fail>) = self.where_player_is(preset.follow_user_id, viewers, vault, ct).await;
        let looked = match (&seen, &failed) {
            (None, Some(f)) => format!("not looked up ({})", App::launch_fail_kind(f)),
            (None, None) => "offline".to_string(),
            (Some(s), _) => App::player_status(s).to_string(),
        };
        let runs_not = seen.as_ref().is_none_or(|s| App::player_status(s) != "found");
        log::note(&format!("Preset {}: followed player {looked}{}.", short(&preset.id), if runs_not { ", so it doesn't run" } else { "" }));
        if seen.is_none() {
            if let Some(failed) = failed {
                // What the C#'s WherePlayerIsAsync threw (called off meanwhile, anything unexpected) goes on as it is: a Stop
                // while it looked must read as stopped, not as Roblox's failure.
                if ct.is_cancelled() || !roblox_api::is_expected_error(&failed) {
                    return Err(failed);
                }
                return Err(Fail::user(if failed.cookie_invalid() {
                    l::t("Roblox didn't accept the selected accounts' sign-in. Check their sign-in, then try again.")
                } else {
                    App::error_text(&failed)
                }));
            }
        }
        let status = seen.as_ref().map_or("offline", App::player_status);
        match status {
            "found" => {
                let seen = seen.expect("found has a presence");
                let mut target = GameTarget::new(seen.place_id.expect("found has a place"), seen.job_id.clone(), None, None);
                target.follow = Some(FollowedPlayer { user_id: preset.follow_user_id, name: player });
                Ok(target)
            }
            "hidden" => Err(Fail::user(l::f(
                "{0} is in a game, but Roblox doesn't show which server to your accounts (their privacy settings), so this preset can't join them.",
                &[&player],
            ))),
            "studio" => Err(Fail::user(l::f("{0} is in Roblox Studio, not in a game, so this preset can't join them yet.", &[&player]))),
            "website" => Err(Fail::user(l::f("{0} is on the Roblox website, not in a game, so this preset can't join them yet.", &[&player]))),
            _ => Err(Fail::user(l::f("{0} is offline, or hides being online, so this preset can't join them yet.", &[&player]))),
        }
    }

    /// A public server with room for all accounts: fullest or emptiest (Roblox's order), lowest ping over two pages, or
    /// random. With none, or no answer, Roblox picks as usual.
    pub async fn pick_server(self: &Rc<Self>, target: GameTarget, choice: &str, party: i32, ct: &Cancel) -> Res<(GameTarget, Option<String>)> {
        if party == 0 {
            return Ok((target, None)); // every account is already open; nothing will join
        }
        self.progress(Some(&l::t("Finding a server…")));
        let place = target.place_id;
        let choice_s = choice.to_string();
        let token = ct.clone();
        // (what loaded, how it failed)
        let (servers, failure): (Vec<ServerInfo>, Option<Fail>) = rt::blocking(move || {
            let mut servers: Vec<ServerInfo> = Vec::new();
            let mut go = || -> Res<()> {
                let (first, next) = roblox_api::get_servers(place, choice_s == "fewest", None, &token)?;
                servers.extend(first);
                if choice_s == "ping" {
                    if let Some(next) = next {
                        servers.extend(roblox_api::get_servers(place, false, Some(&next), &token)?.0);
                    }
                }
                Ok(())
            };
            let r = go();
            (servers, r.err())
        })
        .await;
        let mut failed = false;
        if let Some(ex) = failure {
            if !ct.is_cancelled() && roblox_api::is_expected_error(&ex) {
                // Rate-limited or offline: whatever loaded is used, else Roblox picks.
                log::note(&format!(
                    "Preset server ({choice}): place {place}'s server list not fully read ({}), {} loaded.",
                    App::launch_fail_kind(&ex),
                    servers.len()
                ));
                failed = true;
            } else {
                return Err(ex);
            }
        }
        let fits: Vec<&ServerInfo> = servers.iter().filter(|s| s.max_players - s.playing >= party).collect();
        let chosen: Option<&ServerInfo> = match choice {
            "ping" => fits.iter().filter(|s| s.ping > 0).min_by_key(|s| s.ping).copied().or_else(|| fits.first().copied()),
            "random" => {
                if !fits.is_empty() {
                    Some(fits[random_below(fits.len())])
                } else {
                    None
                }
            }
            _ => fits.first().copied(),
        };
        let tail = match chosen {
            Some(c) => format!("; chose one with {}/{}{}.", c.playing, c.max_players, if c.ping > 0 { format!(", ping {}", c.ping) } else { String::new() }),
            None => "; Roblox picks.".to_string(),
        };
        log::note(&format!("Preset server ({choice}): {} of {} server(s) of place {place} have room for {party}{tail}", fits.len(), servers.len()));
        if let Some(chosen) = chosen {
            let mut picked = target.clone();
            picked.job_id = Some(chosen.job_id.clone());
            return Ok((picked, None));
        }
        // Nothing loaded says nothing about room in the servers: said as what happened.
        let note = if failed && servers.is_empty() {
            l::t("Couldn't load the server list, so Roblox picks a server.")
        } else {
            l::f("No server has room for {0} accounts right now, so Roblox picks one.", &[&party])
        };
        self.toast(&note, "warn");
        Ok((target, Some(note)))
    }

    // ───────── options of the windows a preset opened ─────────
    // A client remembers the preset that opened it (PendingLaunch.preset_id, kept through rejoins, restarts and saved sessions).
    // Options are read from the preset as it is now, so edits apply at once; a deleted preset or Play follows the usual settings.

    pub fn preset_for(&self, id: Option<Guid>) -> Option<LaunchPreset> {
        let preset = id?;
        self.vault(|d| d.settings.presets.iter().find(|p| p.id == preset).cloned()).ok().flatten()
    }

    pub fn auto_rejoin_for(preset: Option<&LaunchPreset>, account: &Account) -> bool {
        preset.and_then(|p| p.auto_rejoin).unwrap_or(account.auto_rejoin)
    }

    pub fn anti_afk_for(preset: Option<&LaunchPreset>, settings: &AppSettings) -> bool {
        preset.and_then(|p| p.anti_afk).unwrap_or(settings.anti_afk)
    }

    pub fn rejoin_on(&self, account: &Account, preset_id: Option<Guid>) -> bool {
        App::auto_rejoin_for(self.preset_for(preset_id).as_ref(), account)
    }

    /// A preset's "Rejoin into" (`LaunchPreset::rejoin_with`): the server its chosen account, or any other window it opened,
    /// plays in now per that window's log. Servers Roblox calls full, ended or closed to this account, or a game that banned
    /// it, are passed over; none left: None, and the rejoin goes to any server of its game.
    pub async fn teammate_target(self: &Rc<Self>, account: &Account, request: &super::controller::RejoinRequest, ct: &Cancel) -> Res<Option<GameTarget>> {
        let Some(preset) = self.preset_for(request.preset_id).filter(|p| !p.rejoin_with.is_empty()) else {
            return Ok(None);
        };
        if request.target.is_private() || request.target.share_code.is_some() {
            return Ok(None);
        }
        let only: Option<Guid> = Guid::parse_str(&preset.rejoin_with).ok();
        let mut runs: Vec<(Stamp, i32)> = self
            .controller
            .running
            .borrow()
            .iter()
            .filter(|(id, (_, launch))| **id != account.id && if let Some(one) = only { **id == one } else { launch.preset_id == Some(preset.id) })
            .map(|(_, (pid, launch))| (launch.started_at.get(), *pid))
            .collect();
        runs.sort_by(|a, b| a.0.cmp(&b.0));
        let mut servers: Vec<GameTarget> = Vec::new();
        for (_, pid) in runs {
            let state = self.controller.logs.borrow().state_of(pid);
            if let Some(s) = state {
                if s.status == "ingame" && s.place_id.is_some() && s.job_id.as_ref().is_some_and(|j| !j.is_empty()) {
                    let t = GameTarget::new(s.place_id.unwrap_or_default(), s.job_id.clone(), None, None);
                    if !servers.contains(&t) {
                        servers.push(t);
                    }
                }
            }
        }
        for target in &servers {
            if self.game_ban_of(account, Some(target.place_id)).is_some() {
                continue;
            }
            let answer = self.check_join(account, target, "Rejoining another account's server", ct).await;
            ct.check()?;
            if !answer.as_ref().is_some_and(|a| a.full() || a.ended() || a.refused()) {
                return Ok(Some(target.clone()));
            }
        }
        if !servers.is_empty() {
            log::note(&format!(
                "Rejoin into: none of {} other account server(s) takes account {}; any server of its game.",
                servers.len(),
                short(&account.id)
            ));
        }
        Ok(None)
    }

    /// Anti-AFK for a Roblox window here: the preset's that opened it, else Settings (also windows opened outside Bloxkeep).
    pub fn anti_afk_on(&self, pid: i32) -> bool {
        let Ok(settings) = self.vault(|d| d.settings.clone()) else {
            return false;
        };
        let preset_id = self.controller.running.borrow().values().find(|(p, _)| *p == pid).map(|(_, launch)| launch.preset_id);
        match preset_id {
            Some(preset_id) => App::anti_afk_for(self.preset_for(preset_id).as_ref(), &settings),
            None => settings.anti_afk,
        }
    }

    /// A light-AFK preset's window is minimised once in its game (Roblox draws less then). Only once, so a window brought
    /// back up stays up; one in use at that moment is left alone.
    pub fn minimise_afk_windows(&self, windows: &HashMap<i32, Vec<isize>>) {
        self.presets.afk_minimised.borrow_mut().retain(|pid| windows.contains_key(pid));
        if self.held_vault().is_none() || input_turn::busy() {
            return;
        }
        let mut foreground: Option<i32> = None;
        let runs: Vec<(i32, Option<Guid>)> = self.controller.running.borrow().values().map(|(pid, launch)| (*pid, launch.preset_id)).collect();
        for (pid, preset_id) in runs {
            let in_game = self.controller.logs.borrow().state_of(pid).is_some_and(|s| s.status == "ingame");
            let list = windows.get(&pid);
            if self.presets.afk_minimised.borrow().contains(&pid)
                || !in_game
                || list.is_none_or(|l| l.is_empty())
                || self.preset_for(preset_id).map(|p| p.profile) != Some(AFK_PROFILE.to_string())
            {
                continue;
            }
            self.presets.afk_minimised.borrow_mut().insert(pid);
            if *foreground.get_or_insert_with(roblox_processes::foreground_pid) == pid && anti_afk::person_active() {
                log::note(&format!("Light AFK: window {pid} in its game left up, in use."));
                continue;
            }
            log::note(&format!("Light AFK: window {pid} minimised, in its game."));
            roblox_processes::minimise(roblox_processes::main_window_of(list.expect("checked above")));
        }
    }

    /// Settings' "Maximise" for windows Bloxkeep opened this run: once up a few seconds or in a game, and again on joining if
    /// Roblox resized it. Maximised, not fullscreen, so windows don't fight over Roblox's one fullscreen switch. Not light-AFK or
    /// tiled presets' windows, nor one the person minimised; not while they're busy in another window (Windows brings a maximised
    /// one to the front).
    pub fn maximise_new_windows(&self, now: Stamp, windows: &HashMap<i32, Vec<isize>>) {
        self.presets.maximised.borrow_mut().retain(|pid, _| windows.contains_key(pid));
        self.presets.window_first_seen.borrow_mut().retain(|pid, _| windows.contains_key(pid));
        if !self.vault(|d| d.settings.maximize_on_open).unwrap_or(false) || input_turn::busy() {
            return;
        }
        let started = self.relaunch.started.get();
        let mut foreground: Option<i32> = None;
        let runs: Vec<(i32, Stamp, Option<Guid>)> =
            self.controller.running.borrow().values().map(|(pid, launch)| (*pid, launch.started_at.get(), launch.preset_id)).collect();
        for (pid, started_at, preset_id) in runs {
            // Only windows opened since Bloxkeep started: ones picked up after a restart were sized by the person already.
            let Some(list) = windows.get(&pid).filter(|l| !l.is_empty()) else {
                continue;
            };
            if started_at < started {
                continue;
            }
            if let Some(preset) = self.preset_for(preset_id) {
                if preset.profile == AFK_PROFILE || preset.arrange == Some(true) {
                    continue;
                }
            }
            let seen = *self.presets.window_first_seen.borrow_mut().entry(pid).or_insert(now);
            // In a game, or up a while (Roblox's home screen, still loading, or a client that writes no log).
            let in_game = self.controller.logs.borrow().state_of(pid).is_some_and(|s| s.status == "ingame");
            let step = if in_game {
                "ingame"
            } else if now.secs_since(seen) >= secs(MAXIMISE_WITHOUT_LOG) {
                "shown"
            } else {
                ""
            };
            let done = self.presets.maximised.borrow().get(&pid).cloned();
            if step.is_empty() || done.is_some_and(|done| done == step || done == "ingame") {
                continue;
            }
            if *foreground.get_or_insert_with(roblox_processes::foreground_pid) != pid && anti_afk::person_active() {
                continue; // the next tick: not over what the person is doing
            }
            let main = roblox_processes::main_window_of(list);
            // Put away while it was loading: it stays away (maximised, it also lost its RAM limit for resting windows).
            if roblox_processes::is_minimised(main) || roblox_processes::is_hidden(main) {
                log::note(&format!(
                    "Maximise: window {pid} left as it is, {} while loading.",
                    if roblox_processes::is_hidden(main) { "hidden" } else { "minimised" }
                ));
                self.presets.maximised.borrow_mut().insert(pid, "ingame".into());
                continue;
            }
            log::note(&format!("Maximise: window {pid} maximised ({}).", if step == "ingame" { "in its game" } else { "up 10 s" }));
            roblox_processes::maximise(main);
            self.presets.maximised.borrow_mut().insert(pid, step.into());
        }
    }

    /// A preset's "restart every N hours": a client in its game that ran that long is closed and reopened through auto-rejoin's
    /// path, one at a time (menu or error screens are auto-rejoin's). A window in use while the person is at the PC waits; one merely
    /// in front of an idle PC is restarted. A client that couldn't be reopened (Multi-Roblox not ready with other windows, a signed-out
    /// or banned account) is left running.
    pub fn restart_due_clients(&self, now: Stamp) {
        const MB: i64 = 1024 * 1024;
        let overdue: Vec<Guid> = self
            .presets
            .restarts
            .borrow()
            .iter()
            .filter(|(_, (_, at))| now.secs_since(*at) > secs(RESTART_CLOSE_TIMEOUT) || now < *at)
            .map(|(id, _)| *id)
            .collect();
        for id in overdue {
            log::note(&format!(
                "Reopen (restart, switch or update): account {}'s window didn't close within {} s; left running.",
                short(&id),
                RESTART_CLOSE_TIMEOUT.as_secs()
            ));
            self.presets.restarts.borrow_mut().remove(&id);
        }
        let running_pids: HashSet<i32> = self.controller.running.borrow().values().map(|(pid, _)| *pid).collect();
        self.presets.restart_held_noted.borrow_mut().retain(|pid| running_pids.contains(pid));
        let heavy_mb = self.vault(|d| d.settings.restart_above_mb as i64).unwrap_or(0);
        let commits: HashMap<i32, i64> = self.mini.client_commit.borrow().clone();
        self.presets.heavy_since.borrow_mut().retain(|pid, _| !(heavy_mb <= 0 || commits.get(pid).copied().unwrap_or(0) / MB <= heavy_mb));
        if heavy_mb > 0 {
            let mut heavy = self.presets.heavy_since.borrow_mut();
            for (pid, commit) in &commits {
                if commit / MB > heavy_mb {
                    heavy.entry(*pid).or_insert(now);
                }
            }
        }
        let held = self.held_vault().is_none()
            || self.controller.running.borrow().is_empty()
            || !self.presets.restarts.borrow().is_empty()
            || input_turn::busy()
            || self.settling()
            || self.controller.rejoining.get()
            || self.controller.launch_cts.borrow().is_some()
            || self.roblox_updates.update_reopen_since.get().is_some()
            || !self.controller.pending.borrow().is_empty()
            || self.controller.rejoin_queue.borrow().iter().any(|r| r.restart)
            || now < self.controller.sweep_background_until.get()
            || now.secs_since(self.controller.last_session_start.get()) < secs(RESTART_SPACING);
        if held {
            return;
        }
        let started = self.relaunch.started.get();
        let mut foreground: Option<i32> = None;
        let mut main: Option<HashSet<i32>> = None;
        let mut runs: Vec<(Guid, i32, Rc<super::controller::PendingLaunch>)> =
            self.controller.running.borrow().iter().map(|(id, (pid, launch))| (*id, *pid, launch.clone())).collect();
        runs.sort_by(|a, b| a.2.started_at.get().cmp(&b.2.started_at.get()));
        for (id, pid, launch) in runs {
            let preset = self.preset_for(launch.preset_id);
            let launch_started = launch.started_at.get();
            let due = preset.as_ref().is_some_and(|p| p.restart_hours > 0 && now.secs_since(launch_started) >= p.restart_hours as f64 * 3600.0);
            // Too heavy for long enough; never the account the person plays (MAIN).
            let heavy = !due
                && self.presets.heavy_since.borrow().get(&pid).is_some_and(|since| now.secs_since(*since) >= secs(HEAVY_FOR))
                && !main.get_or_insert_with(|| self.main_pids()).contains(&pid);
            let state = self.controller.logs.borrow().state_of(pid);
            if !(due || heavy) || launch.target.is_none() || self.rejoin.closing_for_rejoin.borrow().contains(&pid) || state.as_ref().is_some_and(|s| s.status != "ingame") {
                continue;
            }
            let Some(account) = self.find_account(id) else {
                continue;
            };
            // Open before Bloxkeep started and its log not found yet: it may be on an error screen, auto-rejoin's.
            if state.is_none() && launch_started < started && now.secs_since(started) < secs(roblox_log_watcher::search_limit()) {
                continue;
            }
            if *foreground.get_or_insert_with(roblox_processes::foreground_pid) == pid && anti_afk::person_active() {
                continue;
            }
            // Auto-rejoin's path would refuse to open it again (see RejoinAsync): it would stay closed.
            let running_count = self.controller.running.borrow().len();
            if account.cookie_valid == Some(false) || account.banned == Some(true) || (!multi_instance::armed() && running_count > 1) {
                if self.presets.restart_held_noted.borrow_mut().insert(pid) {
                    log::note(&format!(
                        "Restart: window {pid} due ({}) but left running: {}.",
                        if heavy { "heavy" } else { "its preset's hours" },
                        if account.cookie_valid == Some(false) {
                            "signed out"
                        } else if account.banned == Some(true) {
                            "banned on Roblox"
                        } else {
                            "Multi-Roblox not ready"
                        }
                    ));
                }
                continue;
            }
            let commit = self.mini.client_commit.borrow().get(&pid).copied().unwrap_or(0);
            log::note(&format!(
                "Restart: closing window {pid} to reopen it, {}.",
                if heavy {
                    format!("{} MB for {} min", commit / MB, HEAVY_FOR.as_secs() / 60)
                } else {
                    format!("every {} h", preset.as_ref().map_or(0, |p| p.restart_hours))
                }
            ));
            let target = self.restart_target(pid, &launch);
            self.presets.restarts.borrow_mut().insert(id, (target, now));
            self.rejoin.closing_for_rejoin.borrow_mut().insert(pid); // Bloxkeep's own close: neither a drop nor closed by Roblox
            if heavy {
                self.presets.heavy_since.borrow_mut().remove(&pid);
                let gb = (commit as f64 / (1024 * MB) as f64 * 10.0).round() / 10.0;
                self.notify(&l::f("Restarting {0}'s Roblox: it used {1} GB of RAM…", &[&self.name(&account), &gb]), "info", "restartHeavy", id, true, None);
            } else {
                let hours = preset.as_ref().map_or(0, |p| p.restart_hours);
                self.toast(&l::f("Restarting {0}'s Roblox (its preset restarts it every {1} h)…", &[&self.name(&account), &hours]), "info");
            }
            roblox_processes::close(pid, self.in_game_now(pid));
            return;
        }
    }

    /// Where a restart goes: the client's current server while it's the game it was opened into (a preset's accounts stay
    /// together), else the same game or private server. A game picked in the window since is the one it goes back into.
    pub fn restart_target(&self, pid: i32, launch: &super::controller::PendingLaunch) -> GameTarget {
        let mut target = launch.target.clone().expect("a restart has a target");
        let state = self.controller.logs.borrow().state_of(pid);
        if let Some(chosen) = state.as_ref().and_then(|s| s.chosen_place) {
            if chosen != target.place_id {
                let follow = target.follow.clone();
                target = GameTarget::new(chosen, None, None, None);
                target.follow = follow;
            }
        }
        let mut out = target.clone();
        match state {
            Some(s) if !target.is_private() && s.status == "ingame" && s.job_id.is_some() && s.place_id == Some(target.place_id) => out.job_id = s.job_id,
            _ => out.job_id = None,
        }
        out
    }
}
