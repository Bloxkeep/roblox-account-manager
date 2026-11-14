//! "Switch to another server": each window in a public server reopens into a random other server of its game with room;
//! accounts of one game switched together stay together. Private servers, share links and follows are left as they are.
//! Also: a preset's accounts each in a public server of its own, and windows moved off a server that got crowded
//! (Settings › Staying in game, AppSettings::hop_above_players). (AppController.ServerHop.cs)
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use serde_json::{Value, json};

use super::{
    App,
    launching::{EachServer, Run, launch_fail_kind, min_stamp, running, same_vault, secs, vault_now, require_vault},
    presets::random_below,
};
use crate::core::{
    base::{Cancel, Fail, Guid, Res, Stamp},
    game_target::GameTarget,
    l, log,
    roblox_api::{self, ServerInfo},
    roblox_log_watcher::ClientState,
    rt,
    vault::Vault,
};

/// One of the accounts' windows in a public server: (account, its run, its log).
pub type Playing = (Guid, Run, ClientState);

pub const HOP_TIMEOUT: Duration = Duration::from_secs(3 * 60);
/// The choices Settings offers for AppSettings::hop_above_players (0 = never).
pub const HOP_ABOVE_CHOICES: [i32; 9] = [0, 1, 2, 3, 5, 8, 10, 15, 20];
/// How often the servers are counted: Roblox's server list allows only a few reads a minute.
pub const CROWD_CHECK_EVERY: Duration = Duration::from_secs(2 * 60);
/// A window just in is left alone this long: its server may not be in Roblox's list yet, and one that keeps finding
/// crowded servers doesn't hop without a pause.
pub const CROWD_GRACE: Duration = Duration::from_secs(90);
/// Pages of 50 read, emptiest first, before a server not seen yet counts as unknown.
pub const CROWD_PAGES: i32 = 4;

pub struct ServerHopState {
    /// Accounts on their way to another server, and since when: their reopening goes there, not to another account's
    /// server. Taken by that reopening (rejoin); one that never comes (the window closed for good, everything closed)
    /// counts no more after HOP_TIMEOUT, so /pc switch isn't refused for ever (2026-10-30 review).
    pub hopping: RefCell<HashMap<Guid, Stamp>>,
    pub next_crowd_check: Cell<Stamp>,
    pub crowd_busy: Cell<bool>,
    /// Games told to have no quiet server, and when: said again only after a while.
    pub no_quiet_server_said: RefCell<HashMap<i64, Stamp>>,
    /// The last failure of the crowd check that was logged (None after a good read).
    pub crowd_fail_noted: RefCell<Option<String>>,
}

impl Default for ServerHopState {
    fn default() -> ServerHopState {
        ServerHopState {
            hopping: RefCell::new(HashMap::new()),
            next_crowd_check: Cell::new(min_stamp()),
            crowd_busy: Cell::new(false),
            no_quiet_server_said: RefCell::new(HashMap::new()),
            crowd_fail_noted: RefCell::new(None),
        }
    }
}

/// A job id as a key compared without regard to case (StringComparer.OrdinalIgnoreCase).
fn job_key(job: &str) -> String {
    job.to_lowercase()
}

/// Groups by key in first-seen order (LINQ's GroupBy).
fn group_by<K: PartialEq + Clone, T: Clone>(items: &[T], key: impl Fn(&T) -> K) -> Vec<(K, Vec<T>)> {
    let mut groups: Vec<(K, Vec<T>)> = Vec::new();
    for item in items {
        let k = key(item);
        match groups.iter_mut().find(|(g, _)| *g == k) {
            Some((_, list)) => list.push(item.clone()),
            None => groups.push((k, vec![item.clone()])),
        }
    }
    groups
}

/// The next server for an account of its own: one with room that none of `taken` (lower-cased job ids) is in, the list's
/// first (read emptiest or fullest first) or, with "random", any.
pub fn server_apart<'a>(servers: &'a [ServerInfo], taken: &HashSet<String>, order: &str, random: impl Fn(usize) -> usize) -> Option<&'a ServerInfo> {
    let fits: Vec<&ServerInfo> = servers.iter().filter(|s| s.max_players - s.playing >= 1 && !taken.contains(&job_key(&s.job_id))).collect();
    if fits.is_empty() {
        None
    } else if order == "random" {
        Some(fits[random(fits.len())])
    } else {
        Some(fits[0])
    }
}

/// Which of the accounts' servers have more than `above` other players, from Roblox's list read emptiest first (`ours`:
/// how many of the accounts are in each, keyed by lower-cased job id). One in the list counts its players less the
/// accounts'. One not in it is fuller than the list went, when the list went past that many; when the list ended, it is
/// full (Roblox leaves full servers out). Otherwise nothing is known of it and it stays.
pub fn crowded_servers(seen: &[ServerInfo], ended: bool, ours: &[(String, i32)], above: i32) -> HashSet<String> {
    let mut crowded = HashSet::new();
    let max = seen.iter().map(|s| s.max_players).max().unwrap_or(0);
    for (job, count) in ours {
        let server = seen.iter().find(|s| job_key(&s.job_id) == job_key(job));
        let others: Option<i32> = if let Some(server) = server {
            Some(server.playing - count)
        } else if seen.iter().any(|s| s.playing - count > above) {
            Some(above + 1)
        } else if ended && max > 0 {
            Some(max - count)
        } else {
            None
        };
        if others.is_some_and(|o| o > above) {
            crowded.insert(job_key(job));
        }
    }
    crowded
}

/// A server with room where, once in, at most `above` others play, none of `avoid` (lower-cased); any of them.
pub fn quiet_server<'a>(seen: &'a [ServerInfo], above: i32, avoid: &HashSet<String>, random: impl Fn(usize) -> usize) -> Option<&'a ServerInfo> {
    let fits: Vec<&ServerInfo> = seen.iter().filter(|s| s.playing <= above && s.max_players - s.playing >= 1 && !avoid.contains(&job_key(&s.job_id))).collect();
    if fits.is_empty() { None } else { Some(fits[random(fits.len())]) }
}

impl App {
    /// Some account is still on its way to another server.
    pub fn hopping(&self) -> bool {
        let now = Stamp::utc_now();
        self.server_hop.hopping.borrow_mut().retain(|_, since| !(now.secs_since(*since) > secs(HOP_TIMEOUT) || now < *since));
        !self.server_hop.hopping.borrow().is_empty()
    }

    /// The accounts' windows in a public server of a game, with that server.
    fn in_public_servers(&self, ids: &[Guid]) -> Vec<Playing> {
        let runs: HashMap<Guid, Run> = running(self).into_iter().collect();
        ids.iter()
            .filter_map(|id| runs.get(id).map(|run| (*id, run.clone(), self.controller.logs.borrow().state_of(run.0))))
            .filter(|(_, run, log)| {
                run.1.target.as_ref().is_some_and(|t| !t.is_private() && t.share_code.is_none() && t.follow.is_none())
                    && log.as_ref().is_some_and(|l| l.status == "ingame" && l.place_id.is_some() && l.job_id.as_deref().is_some_and(|j| !j.is_empty()))
            })
            .map(|(id, run, log)| (id, run, log.expect("filtered")))
            .collect()
    }

    pub async fn switch_server(self: &Rc<Self>, ids: &[Guid]) -> Res<Value> {
        let vault = require_vault(self)?;
        let playing = self.in_public_servers(ids);
        if playing.is_empty() {
            return Err(Fail::user(l::t("Only accounts playing in a public server can switch to another one.")));
        }
        let mut moved = 0;
        for (place, game) in group_by(&playing, |w| w.2.place_id.unwrap_or(0)) {
            let leaving: HashSet<String> = game.iter().filter_map(|w| w.2.job_id.clone()).collect();
            let servers = match rt::blocking(move || roblox_api::get_servers(place, false, None, &Cancel::new())).await {
                Ok((servers, _)) => servers,
                Err(ex) if ex.rate_limited() => {
                    log::note(&format!("Switch server: place {place}'s server list refused (rate-limited)."));
                    return Err(Fail::user(l::t("Roblox won't give the server list right now. Try again in a minute.")));
                }
                Err(ex) => return Err(ex),
            };
            // Locked while Roblox answered: nothing of that vault's is touched any more.
            if !same_vault(self, &vault) {
                break;
            }
            let count = game.len() as i32;
            // HashSet<string>.Contains: ordinal, case and all.
            let fits: Vec<&ServerInfo> = servers.iter().filter(|s| !leaving.contains(&s.job_id) && s.max_players - s.playing >= count).collect();
            if fits.is_empty() {
                log::note(&format!("Switch server: no other server of place {place} has room for {count} ({} read).", servers.len()));
                self.toast(&l::f("No other server of {0} has room right now.", &[&self.hop_game_name(place)]), "warn");
                continue;
            }
            let chosen = fits[random_below(fits.len())];
            log::note(&format!(
                "Switch server: {count} window(s) of place {place} to a server with {}/{} ({} with room).",
                chosen.playing,
                chosen.max_players,
                fits.len()
            ));
            for (id, run, _) in &game {
                if let Some(why) = self.sign_in_refused_now(*id, &Cancel::new()).await {
                    let n = self.find_account(*id).map(|a| self.name_text(&a)).unwrap_or_default();
                    self.toast(&l::f("{0} stays where it is: {1}", &[&n, &why]), "warn");
                    continue;
                }
                if self.move_to_server(*id, run, place, &chosen.job_id) {
                    moved += 1;
                }
            }
        }
        if moved > 0 {
            let text = if moved == 1 {
                match self.find_account(playing[0].0) {
                    Some(account) => l::f("{0} is moving to another server…", &[&self.name(&account)]),
                    // PORT: FindAccount(..)! threw a NullReferenceException when the account was gone meanwhile.
                    None => l::f("{0} accounts are moving to another server…", &[&moved]),
                }
            } else {
                l::f("{0} accounts are moving to another server…", &[&moved])
            };
            self.toast(&text, "ok");
        }
        self.push_state();
        Ok(json!(moved))
    }

    /// Before a working window is closed to open it elsewhere: a sign-in ticket is asked for (and dropped), so an account
    /// Roblox would refuse now (signed out, a moderation notice, no connection) keeps its window instead of losing it for
    /// nothing. RoALT checks the same before closing. None when Roblox signed it in; else why not, logged.
    pub async fn sign_in_refused_now(&self, id: Guid, ct: &Cancel) -> Option<String> {
        let Some(account) = self.find_account(id) else { return Some(l::t("Account not found.")) };
        let (cookie, realm, c) = (account.cookie.clone(), account.realm.clone(), ct.clone());
        match rt::blocking(move || roblox_api::get_auth_ticket(&cookie, &c, &realm)).await {
            Ok(_) => None,
            Err(ex) if !ct.is_cancelled() && roblox_api::is_expected_error(&ex) => {
                log::note(&format!(
                    "Account {}'s window stays open: a new sign-in was refused ({}), so it isn't closed to reopen.",
                    &id.to_string()[..8],
                    launch_fail_kind(&ex)
                ));
                Some(App::error_text(&ex))
            }
            // The C# lets anything else end the switch, closing nothing: the window stays here too, never closed on a
            // sign-in nobody saw work.
            Err(ex) => {
                log::note(&format!("Account {}'s window stays open: the sign-in check failed unexpectedly ({ex}).", &id.to_string()[..8]));
                Some(App::error_text(&ex))
            }
        }
    }

    /// Closes the window to reopen it in that server (auto-rejoin's path); false when it closed or was replaced meanwhile,
    /// leaving nothing to move and no mark that nothing takes.
    fn move_to_server(&self, id: Guid, run: &Run, place_id: i64, job_id: &str) -> bool {
        let current = self.controller.running.borrow().get(&id).map(|r| r.0);
        if current != Some(run.0) {
            return false;
        }
        self.server_hop.hopping.borrow_mut().insert(id, Stamp::utc_now());
        self.rejoin.join_refused.borrow_mut().retain(|l| !Rc::ptr_eq(l, &run.1));
        self.presets.restarts.borrow_mut().insert(id, (GameTarget::new(place_id, Some(job_id.to_string()), None, None), Stamp::utc_now()));
        self.rejoin.closing_for_rejoin.borrow_mut().insert(run.0);
        self.close_for_reopen(run.0, self.in_game_now(run.0));
        true
    }

    /// GameName(placeId) of the C# (renamed: discord_alerts.rs has its own game_name).
    fn hop_game_name(&self, place_id: i64) -> String {
        let name = self.controller.games.borrow().get(&place_id).and_then(|g| g.as_ref().map(|g| g.name.clone()));
        match name {
            Some(name) if !name.is_empty() => name,
            _ => l::t("this game"),
        }
    }

    // ---- Each account in a server of its own ----

    /// For a launch: a public server for each account as it opens, none shared with another of the accounts (the ones
    /// already playing that game included). `order` is "fewest", "most" or "random". Roblox's list is read once, at the
    /// first account; when it runs out, or can't be read, Roblox picks for the rest.
    pub fn servers_apart(self: &Rc<Self>, order: &str) -> EachServer {
        let servers: Rc<RefCell<Option<Vec<ServerInfo>>>> = Rc::new(RefCell::new(None));
        let taken: Rc<RefCell<HashSet<String>>> = Rc::new(RefCell::new(HashSet::new()));
        let said = Rc::new(Cell::new(false));
        let order = order.to_string();
        let app = self.clone();
        Rc::new(move |target: GameTarget, ct: Cancel| {
            let (servers, taken, said, order, app) = (servers.clone(), taken.clone(), said.clone(), order.clone(), app.clone());
            Box::pin(async move {
                if servers.borrow().is_none() {
                    app.progress(Some(&l::t("Finding a server…")));
                    let mut list: Vec<ServerInfo> = Vec::new();
                    let fewest = order == "fewest";
                    let place = target.place_id;
                    let c = ct.clone();
                    let first = rt::blocking(move || roblox_api::get_servers(place, fewest, None, &c)).await;
                    let read: Res<()> = match first {
                        Ok((first, next)) => {
                            list.extend(first);
                            match next {
                                Some(next) => {
                                    let c = ct.clone();
                                    match rt::blocking(move || roblox_api::get_servers(place, fewest, Some(&next), &c)).await {
                                        Ok((more, _)) => {
                                            list.extend(more);
                                            Ok(())
                                        }
                                        Err(ex) => Err(ex),
                                    }
                                }
                                None => Ok(()),
                            }
                        }
                        Err(ex) => Err(ex),
                    };
                    match read {
                        Ok(()) => {}
                        // Whatever loaded is used, else Roblox picks.
                        Err(ex) if !ct.is_cancelled() && roblox_api::is_expected_error(&ex) => {
                            log::note(&format!(
                                "Servers apart: place {place}'s server list not fully read ({}), {} loaded.",
                                launch_fail_kind(&ex),
                                list.len()
                            ));
                        }
                        Err(ex) => {
                            *servers.borrow_mut() = Some(list); // `servers` was set before the read in the C#
                            return Err(ex);
                        }
                    }
                    for (_, run) in running(&app) {
                        if let Some(log) = app.controller.logs.borrow().state_of(run.0)
                            && let (Some(p), Some(job)) = (log.place_id, log.job_id)
                            && p == target.place_id
                        {
                            taken.borrow_mut().insert(job_key(&job));
                        }
                    }
                    log::note(&format!(
                        "Servers apart ({order}): {} server(s) of place {place} read, {} already taken by open windows.",
                        list.len(),
                        taken.borrow().len()
                    ));
                    *servers.borrow_mut() = Some(list);
                }
                let chosen = {
                    let list = servers.borrow();
                    let list = list.as_deref().unwrap_or(&[]);
                    server_apart(list, &taken.borrow(), &order, random_below).map(|s| s.job_id.clone())
                };
                if let Some(job) = chosen {
                    taken.borrow_mut().insert(job_key(&job));
                    return Ok(GameTarget { job_id: Some(job), ..target });
                }
                if !said.get() {
                    said.set(true);
                    log::note(&format!("Servers apart: ran out of servers with room after {} taken; Roblox picks for the rest.", taken.borrow().len()));
                    app.toast(&l::t("Not enough servers with room were found for every account to have its own: Roblox picks for the rest."), "warn");
                }
                Ok(GameTarget { job_id: None, ..target })
            }) as futures::future::LocalBoxFuture<'static, Res<GameTarget>>
        })
    }

    // ---- Off a crowded server ----

    /// Every CROWD_CHECK_EVERY, when Settings asks for it: the servers of the windows in public servers are counted from
    /// Roblox's list, and a window whose server has more than that many other players moves to one with that many or
    /// fewer, apart from the other accounts (as Switch server does for one). Left until later while a launch, rejoin,
    /// restart or switch is under way.
    pub fn hop_crowded_clients(self: &Rc<Self>, now: Stamp) {
        let Some(vault) = vault_now(self) else {
            return;
        };
        if self.server_hop.crowd_busy.get() || now < self.server_hop.next_crowd_check.get() {
            return;
        }
        let above = self.vault(|d| d.settings.hop_above_players).unwrap_or(0);
        if above <= 0 {
            return;
        }
        if self.controller.launch_cts.borrow().is_some()
            || self.controller.rejoining.get()
            || !self.presets.restarts.borrow().is_empty()
            || self.hopping()
            || self.settling()
            || self.roblox_updates.update_reopen_since.get().is_some()
        {
            return;
        }
        self.server_hop.next_crowd_check.set(now.plus_secs(secs(CROWD_CHECK_EVERY)));
        // Not a preset's windows sent to a server on purpose: its own Job ID, or the fullest server ("most").
        let ids: Vec<Guid> = self.controller.running.borrow().keys().copied().collect();
        let playing: Vec<Playing> = self
            .in_public_servers(&ids)
            .into_iter()
            .filter(|w| {
                now.secs_since(w.2.at) >= secs(CROWD_GRACE)
                    && !self.rejoin.closing_for_rejoin.borrow().contains(&w.1.0)
                    && !self.preset_for(w.1.1.preset_id).is_some_and(|p| p.server == "job" || p.server == "most")
            })
            .collect();
        if playing.is_empty() {
            return;
        }
        self.server_hop.crowd_busy.set(true);
        let app = self.clone();
        rt::spawn(async move { app.hop_crowded(vault, playing, above).await });
    }

    async fn hop_crowded(self: Rc<Self>, vault: Arc<Vault>, playing: Vec<Playing>, above: i32) {
        let run: Res<()> = async {
            for (place, game) in group_by(&playing, |w| w.2.place_id.unwrap_or(0)) {
                // The accounts' windows in each server of this game: they don't count as the others.
                let ours: Vec<(String, i32)> =
                    group_by(&game, |w| job_key(w.2.job_id.as_deref().unwrap_or(""))).into_iter().map(|(k, g)| (k, g.len() as i32)).collect();
                let most = above + ours.iter().map(|o| o.1).max().unwrap_or(0);
                let mut seen: Vec<ServerInfo> = Vec::new();
                let mut ended = false;
                let mut cursor: Option<String> = None;
                for _ in 0..CROWD_PAGES {
                    let c = cursor.clone();
                    let (servers, next) = rt::blocking(move || roblox_api::get_servers(place, true, c.as_deref(), &Cancel::new())).await?;
                    let last_playing = servers.last().map(|s| s.playing);
                    seen.extend(servers);
                    cursor = next.clone();
                    if next.is_none() {
                        ended = true;
                        break;
                    }
                    // Read emptiest first: past this many players, a server of theirs not seen yet is the more crowded.
                    if last_playing.is_some_and(|p| p > most) {
                        break;
                    }
                }
                if !same_vault(&self, &vault) {
                    return Ok(());
                }
                let crowded = crowded_servers(&seen, ended, &ours, above);
                *self.server_hop.crowd_fail_noted.borrow_mut() = None; // read fine: a later failure is said again
                if crowded.is_empty() {
                    continue;
                }
                let mut avoid: HashSet<String> = ours.iter().map(|o| o.0.clone()).collect();
                let mut moved = 0;
                for (id, run, _) in game.iter().filter(|w| crowded.contains(&job_key(w.2.job_id.as_deref().unwrap_or("")))) {
                    let Some(quiet) = quiet_server(&seen, above, &avoid, random_below).map(|s| s.job_id.clone()) else {
                        let said = self.server_hop.no_quiet_server_said.borrow().get(&place).copied();
                        if said.is_none_or(|said| Stamp::utc_now().secs_since(said) > 15.0 * 60.0) {
                            self.server_hop.no_quiet_server_said.borrow_mut().insert(place, Stamp::utc_now());
                            log::note(&format!(
                                "Crowded servers: place {place} has no server with {above} or fewer players ({} read); windows stay.",
                                seen.len()
                            ));
                            self.toast(
                                &l::f(
                                    "No server of {0} has {1} or fewer players right now. Bloxkeep looks again in a few minutes.",
                                    &[&self.hop_game_name(place), &above],
                                ),
                                "warn",
                            );
                        }
                        break;
                    };
                    avoid.insert(job_key(&quiet));
                    if self.sign_in_refused_now(*id, &Cancel::new()).await.is_some() {
                        continue;
                    }
                    if self.move_to_server(*id, run, place, &quiet) {
                        moved += 1;
                    }
                }
                if moved > 0 {
                    log::note(&format!(
                        "Crowded servers: place {place}, {} server(s) above {above} other players; {moved} window(s) moved to quieter ones.",
                        crowded.len()
                    ));
                    self.toast(
                        &l::p(
                            moved as i64,
                            "{0} account's server got crowded, so it moves to a quieter one…",
                            "{0} accounts' servers got crowded, so they move to quieter ones…",
                            &[&moved],
                        ),
                        "ok",
                    );
                }
            }
            Ok(())
        }
        .await;
        match run {
            Ok(()) => {}
            // Rate-limited or offline: counted again next time. Logged when the kind changes, not every 2 minutes.
            Err(ex) if roblox_api::is_expected_error(&ex) => {
                let kind = launch_fail_kind(&ex);
                if self.server_hop.crowd_fail_noted.borrow().as_deref() != Some(kind.as_str()) {
                    *self.server_hop.crowd_fail_noted.borrow_mut() = Some(kind.clone());
                    log::note(&format!("Crowded servers: the server list couldn't be read ({kind}); checked again later."));
                }
            }
            // Out of a task nobody awaited (`_ = HopCrowdedAsync(…)`): it went unseen in the C#.
            Err(_) => {}
        }
        // finally
        self.server_hop.crowd_busy.set(false);
        self.push_state();
    }
}
