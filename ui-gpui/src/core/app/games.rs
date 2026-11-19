// AppController.Games.cs: the game box: what a link, Place ID or name stands for, and the games kept (recent, pinned, saved).
use crate::threads::Hold;
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Mutex,
    time::Duration,
};

use serde_json::{Value, json};

use super::App;
use super::presets::{same_vault, secs};
use crate::core::{
    base::{Cancel, Fail, Guid, Res, Stamp},
    game_target::GameTarget,
    l, log,
    models::{Account, RecentGame},
    roblox_api::{self, GameInfo, OwnedServer, PrivateServer, RobloxUser, ServerInfo, realm},
    rt,
};

pub const GAME_RETRY: Duration = Duration::from_secs(2 * 60);
/// The longest a place Roblox keeps answering without (unpublished, private) waits before it is asked again.
pub const MAX_GAME_RETRY: Duration = Duration::from_secs(3600);
pub const MAX_CACHED_GAMES: usize = 1000;
/// Roblox gives a few server lists a minute (RobloxApi.ServerListLeft): switching order back and forth reuses what's loaded.
pub const SERVER_LIST_LIFETIME: Duration = Duration::from_secs(60);
pub const FRIEND_SERVER_LIFETIME: Duration = Duration::from_secs(45);
/// Who each account's friends are changes rarely; where they are is asked afresh.
pub const FRIEND_IDS_LIFETIME: Duration = Duration::from_secs(10 * 60);

/// A friend found in a game: who, where, and which of your accounts saw them (named when shown, so hiding names applies).
#[derive(Clone, Debug)]
pub struct FriendFound {
    pub user_id: i64,
    pub name: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub job_id: String,
    pub place_id: i64,
    pub via: Guid,
}

/// A private server found for this game, with the accounts that can join it (each asked with its own cookie).
#[derive(Clone, Debug)]
pub struct PrivateFound {
    pub server: PrivateServer,
    pub via: Vec<Guid>,
}

#[derive(Default)]
pub struct GamesState {
    /// Lookups in a row that Roblox answered without the place: each one doubles the wait before the next.
    pub game_misses: RefCell<HashMap<i64, i32>>,
    /// The last game lookup failed: written once, then once more when one goes through.
    pub games_failing: Cell<bool>,
    pub server_pages: RefCell<HashMap<(i64, bool, String), (Vec<ServerInfo>, Option<String>, Stamp)>>,
    /// Kept for a moment per game and per accounts asked (another selection asks other accounts).
    pub friend_servers: RefCell<HashMap<(i64, String), (Vec<FriendFound>, i32, Stamp)>>,
    pub friend_ids: RefCell<HashMap<i64, (Vec<i64>, Stamp)>>,
    pub private_servers: RefCell<HashMap<(i64, String), (Vec<PrivateFound>, i32, bool, Stamp)>>,
    /// The private servers the accounts own (each with its owner's account), how many accounts were asked and how
    /// many of them couldn't be read, kept for a minute.
    pub owned_servers: RefCell<Option<(Vec<(OwnedServer, Guid)>, i32, i32, Stamp)>>,
    /// What the last reading of them came to: written to error.log when it changes, not on every reading.
    pub owned_said: RefCell<String>,
}

/// A public server as the page reads it (camelCase, as the controller's JSON writes ServerInfo).
pub fn server_json(s: &ServerInfo) -> Value {
    json!({ "jobId": s.job_id, "playing": s.playing, "maxPlayers": s.max_players, "ping": s.ping, "fps": s.fps })
}

impl App {
    /// Whether the cache can answer for a place; `fresh` wants a live player count.
    pub fn has_game(&self, place_id: i64, fresh: bool) -> bool {
        let games = self.controller.games.borrow();
        let Some(cached) = games.get(&place_id) else {
            return false;
        };
        if cached.is_some() && !fresh {
            return true;
        }
        let fetched = self.controller.game_fetched_at.borrow().get(&place_id).copied();
        let age = fetched.map_or(f64::MAX, |at| Stamp::utc_now().secs_since(at));
        age < secs(self.game_retry_for(place_id, cached.as_ref(), fresh))
    }

    /// How long a lookup holds: two minutes; for a place Roblox answered without (asked by every status check while a
    /// client is in it) 2, 4, 8 … up to 60 minutes. A lookup the person asked for (fresh) waits two minutes at most.
    pub fn game_retry_for(&self, place_id: i64, cached: Option<&GameInfo>, fresh: bool) -> Duration {
        if !fresh && cached.is_none() {
            if let Some(misses) = self.games.game_misses.borrow().get(&place_id).copied() {
                let shift = (misses - 1).min(6).max(0) as u32;
                let ticks = (GAME_RETRY.as_nanos() as u64).checked_shl(shift).unwrap_or(u64::MAX);
                return Duration::from_nanos(ticks.min(MAX_GAME_RETRY.as_nanos() as u64));
            }
        }
        GAME_RETRY
    }

    /// Fills the game cache in one batch; places whose universe isn't known from presence are looked up first.
    pub async fn load_games(self: &Rc<Self>, places: Vec<(i64, i64)>, fresh: bool) -> Res<()> {
        let mut wanted: Vec<(i64, i64)> = Vec::new();
        for p in places {
            if p.0 > 0 && !self.has_game(p.0, fresh) && !wanted.iter().any(|w| w.0 == p.0) {
                wanted.push(p);
            }
        }
        if wanted.is_empty() {
            return Ok(());
        }
        let now = Stamp::utc_now();
        if self.controller.games.borrow().len() > MAX_CACHED_GAMES {
            let mut ids: Vec<i64> = self.controller.games.borrow().keys().copied().collect();
            let fetched = self.controller.game_fetched_at.borrow().clone();
            ids.sort_by_key(|id| fetched.get(id).copied().map_or(i64::MIN, |s| s.unix_ms()));
            let drop_count = self.controller.games.borrow().len() - MAX_CACHED_GAMES / 2;
            for old in ids.into_iter().take(drop_count) {
                self.controller.games.borrow_mut().remove(&old);
                self.controller.game_fetched_at.borrow_mut().remove(&old);
                self.games.game_misses.borrow_mut().remove(&old);
            }
        }
        for (place_id, _) in &wanted {
            self.controller.game_fetched_at.borrow_mut().insert(*place_id, now);
        }
        // Roblox has no batch lookup from place to universe, so the unknown ones are asked a few at a time.
        let mut known: Vec<(i64, i64)> = Vec::new();
        let mut unknown: Vec<i64> = Vec::new();
        {
            let games = self.controller.games.borrow();
            for w in &wanted {
                let k = if w.1 > 0 { w.1 } else { games.get(&w.0).cloned().flatten().map_or(0, |g| g.universe_id) };
                if k > 0 {
                    known.push((w.0, k));
                } else {
                    unknown.push(w.0);
                }
            }
        }
        let cookie = self.any_global_cookie();
        let looked: Res<(Vec<(i64, i64)>, HashMap<i64, GameInfo>)> = rt::blocking(move || {
            let ct = Cancel::new();
            // Four at a time (SemaphoreSlim(4)): each thread takes the next place until none is left.
            let queue = Mutex::new(unknown.into_iter());
            let answers: Mutex<Vec<(i64, i64)>> = Mutex::new(Vec::new());
            let failure: Mutex<Option<Fail>> = Mutex::new(None);
            crate::threads::scope(|scope| {
                for _ in 0..4 {
                    scope.spawn(|| {
                        loop {
                            let next = queue.hold().next();
                            let Some(place) = next else {
                                break;
                            };
                            match roblox_api::get_universe_id(place, &ct) {
                                Ok(universe) => answers.hold().push((place, universe)),
                                Err(e) => {
                                    let mut f = failure.hold();
                                    if f.is_none() {
                                        *f = Some(e);
                                    }
                                }
                            }
                        }
                    });
                }
            });
            if let Some(f) = failure.into_inner().ok().flatten() {
                return Err(f);
            }
            let mut universe_of = known;
            universe_of.extend(answers.into_inner().unwrap_or_default());
            let games = roblox_api::get_games(universe_of.iter().map(|u| u.1).collect::<Vec<_>>(), cookie.as_deref(), &ct)?;
            Ok((universe_of, games))
        })
        .await;
        match looked {
            Ok((universe_of, games)) => {
                for (place_id, universe_id) in universe_of {
                    // A missing answer (a request failed) keeps what was known before instead of forgetting the name.
                    let before = self.controller.games.borrow().get(&place_id).cloned().flatten();
                    let next = match games.get(&universe_id) {
                        Some(info) => {
                            let mut info = info.clone();
                            info.place_id = place_id;
                            info.icon_url = info.icon_url.or_else(|| before.as_ref().and_then(|b| b.icon_url.clone()));
                            Some(info)
                        }
                        None => before,
                    };
                    let missing = next.is_none();
                    self.controller.games.borrow_mut().insert(place_id, next);
                    if missing {
                        *self.games.game_misses.borrow_mut().entry(place_id).or_insert(0) += 1;
                    } else {
                        self.games.game_misses.borrow_mut().remove(&place_id);
                    }
                }
                if self.games.games_failing.get() {
                    log::note("Games: lookup recovered.");
                }
                self.games.games_failing.set(false);
                Ok(())
            }
            Err(ex) if roblox_api::is_expected_error(&ex) => {
                if !self.games.games_failing.get() {
                    log::note(&format!(
                        "Games: lookup of {} place(s) failed ({}); names stay unknown, asked again in two minutes.",
                        wanted.len(),
                        App::log_kind_of(&ex)
                    ));
                }
                self.games.games_failing.set(true);
                let mut games = self.controller.games.borrow_mut();
                for (place_id, _) in &wanted {
                    games.entry(*place_id).or_insert(None);
                }
                Ok(())
            }
            Err(ex) => Err(ex),
        }
    }

    /// A sign-in for roblox.com's own services (a Roblox VNG one only works there), for calls any account can make.
    pub fn any_global_cookie(&self) -> Option<String> {
        self.vault(|d| {
            d.accounts
                .iter()
                .find(|a| a.cookie_valid != Some(false) && !a.cookie.is_empty() && a.realm == realm::GLOBAL)
                .map(|a| a.cookie.clone())
        })
        .ok()
        .flatten()
    }

    pub async fn get_game_info(self: &Rc<Self>, place_id: i64, fresh: bool) -> Res<Option<GameInfo>> {
        self.load_games(vec![(place_id, 0)], fresh).await?;
        Ok(self.controller.games.borrow().get(&place_id).cloned().flatten())
    }

    /// A page of a game's public servers, kept a minute. left: how many more Roblox would give now (-1: unknown). When
    /// Roblox says to wait, busy with its seconds (retryIn); the window retries by itself then.
    pub async fn servers(self: &Rc<Self>, place_id: i64, fewest_first: bool, cursor: Option<String>) -> Res<Value> {
        if place_id <= 0 {
            return Err(Fail::user(l::t("Invalid Place ID.")));
        }
        let key = (place_id, fewest_first, cursor.clone().unwrap_or_default());
        if let Some((servers, next, at)) = self.games.server_pages.borrow().get(&key) {
            if at.age_secs() < secs(SERVER_LIST_LIFETIME) {
                return Ok(json!({ "servers": servers.iter().map(server_json).collect::<Vec<_>>(), "next": next, "busy": false, "left": roblox_api::server_list_left() }));
            }
        }
        let answer = rt::blocking(move || roblox_api::get_servers(place_id, fewest_first, cursor.as_deref(), &Cancel::new())).await;
        match answer {
            Ok((servers, next)) => {
                {
                    let mut pages = self.games.server_pages.borrow_mut();
                    if pages.len() > 40 {
                        pages.clear();
                    }
                    pages.insert(key, (servers.clone(), next.clone(), Stamp::utc_now()));
                }
                Ok(json!({ "servers": servers.iter().map(server_json).collect::<Vec<_>>(), "next": next, "busy": false, "left": roblox_api::server_list_left() }))
            }
            Err(ex) if ex.rate_limited() => {
                // Roblox's Retry-After when it said, else 20 s (as the C#).
                let retry_in = ex.retry_after().unwrap_or(Duration::from_secs(20)).as_secs_f64().ceil() as i32;
                log::note(&format!("Servers: Roblox rate-limited the server list; the page waits {retry_in} s."));
                Ok(json!({ "servers": [], "next": Value::Null, "busy": true, "retryIn": retry_in, "left": 0 }))
            }
            Err(ex) => Err(ex),
        }
    }

    /// The game's public servers (two pages, fullest first) with the country and city each runs in (server_regions.rs),
    /// for the game page's country pick beside Play. Where a server is comes from one signed-in account's join answer
    /// (the picked ones first), at most 60 servers, two at a time; a server found once is known for the run. `ask`: how
    /// many to ask about this time: the page asks a few, shows them, and asks for the rest ("more") while they show
    /// (the owner, 2026-11-19: "lâu mới hiện server": every one of 60 was asked before the menu showed any).
    pub async fn server_regions(self: &Rc<Self>, place_id: i64, preferred: &[Guid], ask: usize) -> Res<Value> {
        self.held_vault().ok_or_else(|| Fail::user(l::t("Bloxkeep is locked.")))?;
        if place_id <= 0 {
            return Err(Fail::user(l::t("Invalid Place ID.")));
        }
        let mut viewers: Vec<Account> = self.vault(|d| {
            d.accounts.iter().filter(|a| a.user_id > 0 && a.cookie_valid != Some(false) && !a.cookie.is_empty() && a.realm == realm::GLOBAL).cloned().collect()
        })?;
        viewers.sort_by_key(|a| if preferred.contains(&a.id) { 0 } else { 1 });
        let Some(viewer) = viewers.into_iter().next() else {
            return Err(Fail::user(l::t("Add an account first: Roblox says where a server is only to a signed-in account.")));
        };
        let first = self.servers(place_id, false, None).await?;
        if first.get("busy").and_then(Value::as_bool) == Some(true) {
            return Ok(json!({ "busy": true, "retryIn": first.get("retryIn").cloned().unwrap_or(json!(20)) }));
        }
        let list = |v: &Value| v.get("servers").and_then(Value::as_array).cloned().unwrap_or_default();
        let mut rows = list(&first);
        if let Some(next) = first.get("next").and_then(Value::as_str).filter(|n| !n.is_empty()).map(str::to_string)
            && let Ok(more) = self.servers(place_id, false, Some(next)).await
        {
            for r in list(&more) {
                if !rows.iter().any(|o| o.get("jobId") == r.get("jobId")) {
                    rows.push(r);
                }
            }
        }
        // A full server only answers "full" (gamejoin status 22) with no address, so it isn't asked.
        let full = |r: &Value| {
            let max = r.get("maxPlayers").and_then(Value::as_i64).unwrap_or(0);
            max > 0 && r.get("playing").and_then(Value::as_i64).unwrap_or(0) >= max
        };
        let pending: Vec<String> = rows
            .iter()
            .filter(|r| !full(r))
            .filter_map(|r| r.get("jobId").and_then(Value::as_str))
            .filter(|j| !j.is_empty())
            .take(60)
            .filter(|j| crate::core::server_regions::known(j).is_none() && !crate::core::server_regions::tried(j))
            .map(str::to_string)
            .collect();
        let jobs: Vec<String> = pending.iter().take(ask.max(1)).cloned().collect();
        let more = pending.len() - jobs.len();
        let stopped = if jobs.is_empty() { None } else { ask_where(viewer.cookie.clone(), place_id, jobs).await };
        if let Some(why) = &stopped {
            log::write(&format!("Server regions: stopped asking where servers are for place {place_id}: {why}"));
        }
        let blocked = stopped.as_deref().is_some_and(roblox_api::is_region_blocked);
        // Each server with its place, then the countries: how many servers and players in each, most servers first.
        let mut regions: Vec<(String, Vec<String>, i64, i64)> = Vec::new();
        let mut unknown = 0;
        for row in rows.iter_mut() {
            let job = row.get("jobId").and_then(Value::as_str).unwrap_or_default().to_string();
            match crate::core::server_regions::known(&job) {
                Some((cc, city)) => {
                    let playing = row.get("playing").and_then(Value::as_i64).unwrap_or(0);
                    row["cc"] = json!(cc);
                    row["city"] = json!(city);
                    match regions.iter_mut().find(|r| r.0 == cc) {
                        Some(r) => {
                            if !r.1.contains(&city) {
                                r.1.push(city);
                            }
                            r.2 += 1;
                            r.3 += playing;
                        }
                        None => regions.push((cc, vec![city], 1, playing)),
                    }
                }
                None if !full(row) && crate::core::server_regions::tried(&job) => unknown += 1,
                None => {}
            }
        }
        regions.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));
        Ok(json!({
            "servers": rows,
            "regions": regions.iter().map(|(cc, cities, n, players)| json!({ "cc": cc, "cities": cities, "servers": n, "players": players })).collect::<Vec<_>>(),
            "unknown": if blocked { 0 } else { unknown },
            "stopped": stopped.is_some() && !blocked,
            "blocked": blocked,
            "more": if stopped.is_some() { 0 } else { more },
        }))
    }

    /// The Servers tab's servers (`jobs`, the ones on screen): each one's country and city, game version and start,
    /// asked of gamejoin for those not known yet (at most 40 a time). `newest`: the highest version seen, so a server
    /// still on an older one can be told apart.
    pub async fn server_facts(self: &Rc<Self>, place_id: i64, jobs: Vec<String>, preferred: &[Guid]) -> Res<Value> {
        self.held_vault().ok_or_else(|| Fail::user(l::t("Bloxkeep is locked.")))?;
        if place_id <= 0 {
            return Err(Fail::user(l::t("Invalid Place ID.")));
        }
        let mut viewers: Vec<Account> = self.vault(|d| {
            d.accounts.iter().filter(|a| a.user_id > 0 && a.cookie_valid != Some(false) && !a.cookie.is_empty() && a.realm == realm::GLOBAL).cloned().collect()
        })?;
        viewers.sort_by_key(|a| if preferred.contains(&a.id) { 0 } else { 1 });
        let Some(viewer) = viewers.into_iter().next() else {
            return Err(Fail::user(l::t("Add an account first: Roblox says where a server is only to a signed-in account.")));
        };
        let ask: Vec<String> = jobs.iter().filter(|j| !j.is_empty() && crate::core::server_regions::facts(j).is_none()).take(40).cloned().collect();
        let stopped = if ask.is_empty() { None } else { ask_where(viewer.cookie.clone(), place_id, ask).await };
        if let Some(why) = &stopped {
            log::write(&format!("Server facts: stopped asking for place {place_id}: {why}"));
        }
        let blocked = stopped.as_deref().is_some_and(roblox_api::is_region_blocked);
        let rows: Vec<Value> = jobs
            .iter()
            .filter_map(|j| {
                let (version, started) = crate::core::server_regions::facts(j)?;
                let at = crate::core::server_regions::known(j);
                Some(json!({ "jobId": j, "version": version, "started": started, "cc": at.as_ref().map(|a| a.0.clone()), "city": at.map(|a| a.1) }))
            })
            .collect();
        let newest = rows.iter().filter_map(|r| r.get("version").and_then(Value::as_i64)).max().unwrap_or(0);
        Ok(json!({ "facts": rows, "newest": newest, "blocked": blocked, "stopped": stopped.is_some() && !blocked }))
    }

    pub async fn friend_ids(self: &Rc<Self>, user_id: i64) -> Res<Vec<i64>> {
        if let Some((ids, at)) = self.games.friend_ids.borrow().get(&user_id) {
            if at.age_secs() < secs(FRIEND_IDS_LIFETIME) {
                return Ok(ids.clone());
            }
        }
        let ids = rt::blocking(move || roblox_api::get_friend_ids(user_id, &Cancel::new())).await?;
        self.games.friend_ids.borrow_mut().insert(user_id, (ids.clone(), Stamp::utc_now()));
        Ok(ids)
    }

    /// Friends of your accounts playing this game, with their server. Each account asks with its own cookie: Roblox gives
    /// the server only when that friend's privacy lets them join. At most 3 accounts (selected first) to keep requests few.
    pub async fn friend_servers(self: &Rc<Self>, place_id: i64, preferred: &[Guid]) -> Res<Value> {
        let vault = self.held_vault().ok_or_else(|| Fail::user(l::t("Bloxkeep is locked.")))?;
        if place_id <= 0 {
            return Err(Fail::user(l::t("Invalid Place ID.")));
        }
        let (mine, mut viewers): (HashSet<i64>, Vec<Account>) = self.vault(|d| {
            (
                d.accounts.iter().map(|a| a.user_id).collect(),
                d.accounts.iter().filter(|a| a.user_id > 0 && a.cookie_valid != Some(false) && !a.cookie.is_empty() && a.realm == realm::GLOBAL).cloned().collect(),
            )
        })?;
        viewers.sort_by_key(|a| if preferred.contains(&a.id) { 0 } else { 1 });
        viewers.truncate(3);
        let key = (place_id, viewers.iter().map(|v| v.id.to_string()).collect::<Vec<_>>().join(","));
        if let Some((friends, asked, at)) = self.games.friend_servers.borrow().get(&key) {
            if at.age_secs() < secs(FRIEND_SERVER_LIFETIME) {
                return Ok(self.friend_servers_dto(friends, *asked));
            }
        }
        // Each viewer's friends (from the cache when fresh), then where they are, side by side.
        let cached: Vec<Option<Vec<i64>>> = viewers
            .iter()
            .map(|v| self.games.friend_ids.borrow().get(&v.user_id).filter(|(_, at)| at.age_secs() < secs(FRIEND_IDS_LIFETIME)).map(|(ids, _)| ids.clone()))
            .collect();
        let asks: Vec<(Account, Option<Vec<i64>>)> = viewers.iter().cloned().zip(cached).collect();
        // Per viewer: (its friends' ids freshly read, the friends found in this game, or how it failed)
        type Answer = (Guid, i64, Option<Vec<i64>>, Res<Vec<(i64, String, i64)>>);
        let answers: Vec<Answer> = rt::blocking(move || {
            let ct = Cancel::new();
            crate::threads::scope(|scope| {
                let handles: Vec<_> = asks
                    .iter()
                    .map(|(viewer, cached)| {
                        let ct = &ct;
                        let mine = &mine;
                        scope.spawn(move || {
                            let mut fresh: Option<Vec<i64>> = None;
                            let found = (|| -> Res<Vec<(i64, String, i64)>> {
                                let ids = match cached {
                                    Some(ids) => ids.clone(),
                                    None => {
                                        let ids = roblox_api::get_friend_ids(viewer.user_id, ct)?;
                                        fresh = Some(ids.clone());
                                        ids
                                    }
                                };
                                let friends: Vec<i64> = ids.into_iter().filter(|id| !mine.contains(id)).collect();
                                if friends.is_empty() {
                                    return Ok(Vec::new());
                                }
                                let mut found = Vec::new();
                                for (id, p) in roblox_api::get_presence(Some(&viewer.cookie), &friends, realm::GLOBAL, ct)? {
                                    if let Some(job) = p.job_id.clone().filter(|j| !j.is_empty()) {
                                        if p.r#type == 2 && (p.place_id == Some(place_id) || p.root_place_id == Some(place_id)) {
                                            found.push((id, job, p.place_id.unwrap_or(place_id)));
                                        }
                                    }
                                }
                                Ok(found)
                            })();
                            (viewer.id, viewer.user_id, fresh, found)
                        })
                    })
                    .collect();
                handles.into_iter().filter_map(|h| h.join().ok()).collect()
            })
        })
        .await;
        let mut found: Vec<(i64, (String, i64, Guid))> = Vec::new();
        let mut friend_fails = 0;
        let mut friend_kind: Option<String> = None;
        for (via, user_id, fresh, answer) in answers {
            if let Some(ids) = fresh {
                self.games.friend_ids.borrow_mut().insert(user_id, (ids, Stamp::utc_now()));
            }
            match answer {
                Ok(list) => {
                    for (id, job, place) in list {
                        if !found.iter().any(|f| f.0 == id) {
                            found.push((id, (job, place, via)));
                        }
                    }
                }
                // That account's friends are just left out.
                Err(ex) if roblox_api::is_expected_error(&ex) => {
                    friend_fails += 1;
                    friend_kind.get_or_insert_with(|| App::log_kind_of(&ex));
                }
                Err(ex) => return Err(ex),
            }
        }
        if friend_fails > 0 {
            log::note(&format!(
                "Servers: friends of {friend_fails} of {} account(s) not read ({}); left out.",
                viewers.len(),
                friend_kind.unwrap_or_default()
            ));
        }
        let ids: Vec<i64> = found.iter().map(|f| f.0).collect();
        // Names and pictures are extras: without them the friends still show (by id) and can be joined.
        let mut users: HashMap<i64, RobloxUser> = HashMap::new();
        let mut faces: HashMap<i64, String> = HashMap::new();
        if !ids.is_empty() {
            let asked = ids.clone();
            let extras: Res<(HashMap<i64, RobloxUser>, HashMap<i64, String>)> = rt::blocking(move || {
                let ct = Cancel::new();
                crate::threads::scope(|scope| {
                    let named = scope.spawn(|| roblox_api::get_users(asked.clone(), &ct, false));
                    let pictured = scope.spawn(|| roblox_api::get_headshot_urls(asked.clone(), &ct));
                    let users = named.join().unwrap_or_else(|_| Err(Fail::other("names thread died")))?;
                    let faces = pictured.join().unwrap_or_else(|_| Err(Fail::other("pictures thread died")))?;
                    Ok((users, faces))
                })
            })
            .await;
            match extras {
                Ok((u, f)) => {
                    users = u;
                    faces = f;
                }
                Err(ex) if roblox_api::is_expected_error(&ex) => {
                    log::note(&format!("Servers: friends' names and pictures not loaded ({}); shown without them.", App::log_kind_of(&ex)));
                }
                Err(ex) => return Err(ex),
            }
        }
        let mut friends: Vec<FriendFound> = found
            .into_iter()
            // The server belongs to this place, which can be another place of the game than the one asked.
            .map(|(id, (job, place, via))| FriendFound {
                user_id: id,
                name: users.get(&id).map(|u| u.name.clone()).unwrap_or_default(),
                display_name: users.get(&id).map(|u| u.display_name.clone()).unwrap_or_default(),
                avatar_url: faces.get(&id).cloned(),
                job_id: job,
                place_id: place,
                via,
            })
            .collect();
        friends.sort_by_key(|f| f.display_name.to_lowercase());
        if !same_vault(&self.held_vault(), &Some(vault)) {
            return Err(Fail::user(l::t("Bloxkeep was locked.")));
        }
        {
            let mut kept = self.games.friend_servers.borrow_mut();
            if kept.len() > 20 {
                kept.clear();
            }
            kept.insert(key, (friends.clone(), viewers.len() as i32, Stamp::utc_now()));
        }
        Ok(self.friend_servers_dto(&friends, viewers.len() as i32))
    }

    /// The friends as the page gets them, with the account that saw each named as it reads now.
    pub fn friend_servers_dto(&self, friends: &[FriendFound], asked: i32) -> Value {
        json!({
            "asked": asked,
            "friends": friends.iter().map(|f| json!({
                "userId": f.user_id,
                "name": f.name,
                "displayName": f.display_name,
                "avatarUrl": f.avatar_url,
                "jobId": f.job_id,
                "placeId": f.place_id,
                "via": self.find_account(f.via).map(|a| self.name_text(&a)).unwrap_or_default(),
            })).collect::<Vec<_>>(),
        })
    }

    /// The game's private servers your accounts can join, for Browse servers. Roblox answers each account with its own
    /// list, so the selected accounts are asked (up to 6; none selected, 3 others), and each server lists who can join it.
    pub async fn private_servers(self: &Rc<Self>, place_id: i64, selected: &[Guid], fresh: bool) -> Res<Value> {
        let vault = self.held_vault().ok_or_else(|| Fail::user(l::t("Bloxkeep is locked.")))?;
        if place_id <= 0 {
            return Err(Fail::user(l::t("Invalid Place ID.")));
        }
        let usable: Vec<Account> = self.vault(|d| {
            d.accounts.iter().filter(|a| a.user_id > 0 && a.cookie_valid != Some(false) && !a.cookie.is_empty() && a.realm == realm::GLOBAL).cloned().collect()
        })?;
        let mut viewers: Vec<Account> = usable.iter().filter(|a| selected.contains(&a.id)).take(6).cloned().collect();
        if viewers.is_empty() {
            viewers = usable.iter().take(3).cloned().collect();
        }
        let key = (place_id, viewers.iter().map(|v| v.id.to_string()).collect::<Vec<_>>().join(","));
        if !fresh
            && let Some((servers, asked, failed, at)) = self.games.private_servers.borrow().get(&key)
            && at.age_secs() < secs(FRIEND_SERVER_LIFETIME)
        {
            return Ok(self.private_servers_dto(servers, *asked, *failed));
        }
        let asks: Vec<(Guid, String)> = viewers.iter().map(|v| (v.id, v.cookie.clone())).collect();
        let answers: Vec<(Guid, Res<Vec<PrivateServer>>)> = rt::blocking(move || {
            let ct = Cancel::new();
            crate::threads::scope(|scope| {
                let handles: Vec<_> = asks
                    .iter()
                    .map(|(id, cookie)| {
                        let ct = &ct;
                        scope.spawn(move || (*id, roblox_api::get_private_servers(cookie, place_id, ct)))
                    })
                    .collect();
                handles.into_iter().filter_map(|h| h.join().ok()).collect()
            })
        })
        .await;
        // By access code, ignoring case, in the order they were found.
        let mut found: Vec<PrivateFound> = Vec::new();
        let mut failed = 0;
        let mut failed_kind: Option<String> = None;
        for (viewer, answer) in answers {
            match answer {
                Ok(list) => {
                    for s in list {
                        match found.iter_mut().find(|f| f.server.access_code.to_lowercase() == s.access_code.to_lowercase()) {
                            Some(f) => f.via.push(viewer),
                            None => found.push(PrivateFound { server: s, via: vec![viewer] }),
                        }
                    }
                }
                Err(ex) if roblox_api::is_expected_error(&ex) => {
                    failed += 1; // that account's servers are just left out
                    failed_kind.get_or_insert_with(|| App::log_kind_of(&ex));
                }
                Err(ex) => return Err(ex),
            }
        }
        if failed > 0 {
            log::note(&format!("Private servers: {failed} of {} account(s) not read ({}).", viewers.len(), failed_kind.unwrap_or_default()));
        }
        if !same_vault(&self.held_vault(), &Some(vault)) {
            return Err(Fail::user(l::t("Bloxkeep was locked.")));
        }
        // Those every account asked can join first, then the fuller ones (people are playing there).
        let mut servers = found;
        servers.sort_by(|a, b| {
            b.via
                .len()
                .cmp(&a.via.len())
                .then(b.server.playing.cmp(&a.server.playing))
                .then(crate::core::win_kit::culture_cmp(&a.server.name, &b.server.name))
        });
        let all_failed = !viewers.is_empty() && failed == viewers.len();
        {
            let mut kept = self.games.private_servers.borrow_mut();
            if kept.len() > 20 {
                kept.clear();
            }
            if !all_failed {
                kept.insert(key, (servers.clone(), viewers.len() as i32, false, Stamp::utc_now()));
            }
        }
        Ok(self.private_servers_dto(&servers, viewers.len() as i32, all_failed))
    }

    /// Every private server the app's accounts own, for the Roblox tab's "Private servers": Roblox lists them one
    /// account at a time (Inventory › Private Servers), so each account is asked with its own sign-in, six at a time,
    /// and the answers are put together by game. `fresh` reads again (the page's refresh).
    pub async fn owned_servers(self: &Rc<Self>, fresh: bool) -> Res<Value> {
        let vault = self.held_vault().ok_or_else(|| Fail::user(l::t("Bloxkeep is locked.")))?;
        if !fresh {
            if let Some((servers, asked, failed, at)) = &*self.games.owned_servers.borrow() {
                if at.age_secs() < secs(FRIEND_SERVER_LIFETIME) {
                    return Ok(self.owned_servers_dto(servers, *asked, *failed));
                }
            }
        }
        let owners: Vec<Account> = self.vault(|d| {
            d.accounts.iter().filter(|a| a.user_id > 0 && a.cookie_valid != Some(false) && !a.cookie.is_empty() && a.realm == realm::GLOBAL).cloned().collect()
        })?;
        let asks: Vec<(Guid, i64, String)> = owners.iter().map(|a| (a.id, a.user_id, a.cookie.clone())).collect();
        let answers: Vec<(Guid, i64, Res<Vec<OwnedServer>>)> = rt::blocking(move || {
            let ct = Cancel::new();
            let mut all = Vec::new();
            for group in asks.chunks(6) {
                let part: Vec<_> = crate::threads::scope(|scope| {
                    let handles: Vec<_> = group
                        .iter()
                        .map(|(id, user, cookie)| {
                            let ct = &ct;
                            scope.spawn(move || (*id, *user, roblox_api::get_owned_servers(cookie, ct)))
                        })
                        .collect();
                    handles.into_iter().filter_map(|h| h.join().ok()).collect()
                });
                all.extend(part);
            }
            all
        })
        .await;
        let mut found: Vec<(OwnedServer, Guid)> = Vec::new();
        let (mut failed, mut failed_kind, mut others) = (0, None::<String>, 0);
        for (account, user, answer) in answers {
            match answer {
                Ok(list) => {
                    for s in list {
                        // (Its own only: a server someone else owns and lent it is not "mine", whatever the tab gave.)
                        if s.owner_id != 0 && s.owner_id != user {
                            others += 1;
                        } else if !found.iter().any(|f| f.0.id == s.id) {
                            found.push((s, account));
                        }
                    }
                }
                Err(ex) if roblox_api::is_expected_error(&ex) => {
                    failed += 1;
                    failed_kind.get_or_insert_with(|| App::log_kind_of(&ex));
                }
                Err(ex) => return Err(ex),
            }
        }
        if !same_vault(&self.held_vault(), &Some(vault)) {
            return Err(Fail::user(l::t("Bloxkeep was locked.")));
        }
        // By game, then the ones still running, then by name.
        found.sort_by(|a, b| {
            crate::core::win_kit::culture_cmp(&a.0.game, &b.0.game)
                .then(a.0.universe_id.cmp(&b.0.universe_id))
                .then(b.0.active.cmp(&a.0.active))
                .then(crate::core::win_kit::culture_cmp(&a.0.name, &b.0.name))
        });
        let said = format!(
            "Owned private servers: {} read from {} account(s){}{}.",
            found.len(),
            owners.len(),
            if failed > 0 { format!(", {failed} not read ({})", failed_kind.unwrap_or_default()) } else { String::new() },
            if others > 0 { format!(", {others} of other owners left out") } else { String::new() }
        );
        if *self.games.owned_said.borrow() != said {
            log::note(&said);
            *self.games.owned_said.borrow_mut() = said;
        }
        let (asked, failed) = (owners.len() as i32, failed as i32);
        // (All failed: not kept, so the next look asks again.)
        *self.games.owned_servers.borrow_mut() = (asked == 0 || failed < asked).then(|| (found.clone(), asked, failed, Stamp::utc_now()));
        Ok(self.owned_servers_dto(&found, asked, failed))
    }

    pub fn owned_servers_dto(&self, servers: &[(OwnedServer, Guid)], asked: i32, failed: i32) -> Value {
        json!({
            "asked": asked,
            "failed": failed,
            "servers": servers.iter().map(|(s, account)| json!({
                "id": s.id,
                "name": s.name,
                "placeId": s.place_id,
                "universeId": s.universe_id,
                "game": s.game,
                "active": s.active,
                "price": s.price,
                "expires": s.expires,
                "renews": s.renews,
                "account": self.find_account(*account).map(|a| self.name_text(&a)).unwrap_or_default(),
                "accountId": account.to_string(),
            })).collect::<Vec<_>>(),
        })
    }

    pub fn private_servers_dto(&self, servers: &[PrivateFound], asked: i32, failed: bool) -> Value {
        json!({
            "asked": asked,
            "failed": failed,
            "servers": servers.iter().map(|f| json!({
                "id": f.server.id,
                "name": f.server.name,
                "owner": if !f.server.owner_display_name.is_empty() { &f.server.owner_display_name } else { &f.server.owner_name },
                "ownerName": f.server.owner_name,
                "accessCode": f.server.access_code,
                "playing": f.server.playing,
                "maxPlayers": f.server.max_players,
                "via": f.via.iter().map(|id| self.find_account(*id).map(|a| self.name_text(&a)).unwrap_or_default()).filter(|n| !n.is_empty()).collect::<Vec<_>>(),
                "viaIds": f.via.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        })
    }

    pub async fn resolve_game(self: &Rc<Self>, input: &str) -> Res<Value> {
        let target = match GameTarget::try_parse(input, "") {
            Ok(t) => t,
            Err(error) => return Ok(json!({ "ok": false, "error": error })),
        };
        if target.share_code.is_some() {
            return Ok(json!({ "ok": true, "kind": "share" }));
        }
        let info = self.get_game_info(target.place_id, true).await?;
        let kind = if target.is_private() {
            "private"
        } else if target.job_id.is_some() {
            "server"
        } else {
            "game"
        };
        Ok(json!({
            "ok": true,
            "kind": kind,
            "placeId": target.place_id,
            "name": info.as_ref().map(|i| i.name.clone()),
            "creator": info.as_ref().map(|i| i.creator.clone()),
            "playing": info.as_ref().map(|i| i.playing),
            "iconUrl": info.as_ref().and_then(|i| i.icon_url.clone()),
        }))
    }

    pub fn remember_game(&self, place_id: i64) -> Res<()> {
        let info = self.controller.games.borrow().get(&place_id).cloned().flatten();
        self.vault_mut(|d| {
            let recent = &mut d.settings.recent_games;
            recent.retain(|g| g.place_id != place_id);
            recent.insert(
                0,
                RecentGame {
                    place_id,
                    universe_id: info.as_ref().map_or(0, |i| i.universe_id),
                    name: info.as_ref().map(|i| i.name.clone()).unwrap_or_default(),
                    ..RecentGame::default()
                },
            );
            if recent.len() > 8 {
                recent.truncate(8);
            }
        })
    }

    pub async fn load_recent_games(self: &Rc<Self>) -> Res<()> {
        let Some(vault) = self.held_vault() else {
            return Ok(());
        };
        let places: Vec<(i64, i64)> = self.vault(|d| d.settings.recent_games.iter().map(|g| (g.place_id, g.universe_id)).collect())?;
        self.load_games(places, false).await?;
        if !same_vault(&self.held_vault(), &Some(vault)) {
            return Ok(());
        }
        let games = self.controller.games.borrow().clone();
        let changed = self.vault_mut(|d| {
            let mut changed = false;
            for game in d.settings.recent_games.iter_mut() {
                let Some(info) = games.get(&game.place_id).cloned().flatten().filter(|i| !i.name.is_empty()) else {
                    continue;
                };
                if info.name != game.name || game.universe_id != info.universe_id {
                    game.name = info.name.clone();
                    game.universe_id = info.universe_id;
                    changed = true;
                }
            }
            changed
        })?;
        if changed {
            self.persist();
        }
        self.push_state();
        Ok(())
    }
}

/// Asks gamejoin where each of `jobs` runs, as Roblox's own client asks (two at a time with a pause; Roblox's "too many"
/// waited out a few times), and keeps each one's place and facts (server_regions.rs). Why it stopped early, if it did:
/// Roblox's 429 for good, or the game blocked where the account is (roblox_api::is_region_blocked).
async fn ask_where(cookie: String, place_id: i64, jobs: Vec<String>) -> Option<String> {
    rt::blocking(move || {
        let ct = Cancel::new();
        let queue = Mutex::new(jobs.into_iter());
        let stop: Mutex<Option<String>> = Mutex::new(None);
        // Two at a time with a pause between, and Roblox's "too many" (HTTP 429) waited out a few times before giving
        // up: six at once with no pause were refused at once and none of 99 servers was placed (the owner's log,
        // 2026-11-18). What was placed is kept either way.
        let waits = std::sync::atomic::AtomicU32::new(0);
        // Why a server got no place, counted for the log: none of 98 was placed and the log said nothing (the
        // owner, 2026-11-19: "ko biết do roblox hay do app").
        let missed: Mutex<HashMap<String, u32>> = Mutex::new(HashMap::new());
        crate::threads::scope(|scope| {
            for _ in 0..2 {
                let (queue, stop, cookie, ct, waits, missed) = (&queue, &stop, &cookie, &ct, &waits, &missed);
                scope.spawn(move || {
                    'jobs: loop {
                        if stop.lock().unwrap_or_else(|p| p.into_inner()).is_some() {
                            break;
                        }
                        let Some(job) = queue.lock().unwrap_or_else(|p| p.into_inner()).next() else { break };
                        crate::core::server_regions::mark_tried(&job);
                        loop {
                            match roblox_api::server_address(cookie, place_id, &job, ct) {
                                Ok(Ok(join)) => {
                                    crate::core::server_regions::keep_facts(&job, join.version, join.started_ms);
                                    match crate::core::server_regions::region_of(&join.address) {
                                        Some(at) => crate::core::server_regions::keep(&job, at),
                                        None => *missed.lock().unwrap_or_else(|p| p.into_inner()).entry("an address no table or ipinfo.io places".into()).or_insert(0) += 1,
                                    }
                                }
                                Ok(Err(why)) if roblox_api::is_region_blocked(&why) => {
                                    // (Every server of the game answers the same: not asked further.)
                                    *stop.lock().unwrap_or_else(|p| p.into_inner()) = Some(why);
                                    break 'jobs;
                                }
                                Ok(Err(why)) => *missed.lock().unwrap_or_else(|p| p.into_inner()).entry(why).or_insert(0) += 1,
                                Err(e) if e.to_string().contains("HTTP 429") && waits.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 4 => {
                                    std::thread::sleep(Duration::from_secs(6));
                                    continue;
                                }
                                Err(e) => {
                                    *stop.lock().unwrap_or_else(|p| p.into_inner()) = Some(e.to_string());
                                    break 'jobs;
                                }
                            }
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(250));
                    }
                });
            }
        });
        let missed = missed.into_inner().unwrap_or_else(|p| p.into_inner());
        if !missed.is_empty() {
            let mut why: Vec<(String, u32)> = missed.into_iter().collect();
            why.sort_by(|a, b| b.1.cmp(&a.1));
            let said: Vec<String> = why.iter().map(|(w, n)| format!("{n} × {w}")).collect();
            log::write(&format!("Server regions: place {place_id}: servers with no place: {}", said.join("; ")));
        }
        stop.into_inner().unwrap_or_else(|p| p.into_inner())
    })
    .await
}
