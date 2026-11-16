// AppController.GamePage.cs: a game's page (ui/src/GamePage.tsx, opened from its name under the game box): Roblox's
// figures for it, your accounts in it over the last 7 days (from Statistics' stays), the hours it is quiet (GameHours,
// noted every half hour while Bloxkeep is open), its wiki (a web search) and its roblox.com page.
use std::{
    cell::Cell,
    collections::HashMap,
    rc::Rc,
    time::Duration,
};

use serde_json::{Value, json};

use super::App;
use super::presets::secs;
use super::wiki::KnownGame;
use crate::core::{
    base::{Cancel, Fail, Guid, Res, Stamp},
    game_hours, l, prefs,
    models::PlaySession,
    roblox_api, rt,
};

pub const GAME_HOURS_EVERY: Duration = Duration::from_secs(30 * 60);
/// The games whose player count is noted: those being played, then the newest in the game box's list.
pub const GAME_HOURS_RECENT: usize = 3;
/// Roblox's disconnect codes that are a kick: by the game (267), for idling 20 minutes (278).
pub const KICK_CODES: [i32; 2] = [267, 278];
pub const IDLE_KICK_CODE: i32 = 278;

#[derive(Default)]
pub struct GamePageState {
    /// None: never (default(DateTime)).
    pub game_hours_at: Cell<Option<Stamp>>,
    pub game_hours_busy: Cell<bool>,
}

/// An account's stays in a game over 7 days, summed: how long, kicks (by the game or for idling) and other
/// drops, whether it is in it now, when it last was, and the name its last stay kept.
#[derive(Clone, Debug, PartialEq)]
pub struct GameStay {
    pub id: Guid,
    pub seconds: f64,
    pub kicks: i32,
    pub idle: bool,
    pub drops: i32,
    pub now: bool,
    pub last: Stamp,
    pub label: Option<String>,
}

impl App {
    pub async fn game_community(&self, group: i64, id: Guid, action: &str) -> Res<Value> {
        self.vault(|_| ())?;
        if group <= 0 {
            return Err(Fail::user(l::t("Invalid Roblox community.")));
        }
        let viewer = if id.is_nil() { None } else {
            let account = self.account(id)?;
            (!account.cookie.is_empty() && account.user_id > 0 && account.cookie_valid != Some(false)
                && account.realm == roblox_api::realm::GLOBAL).then_some(account)
        };
        let change = match action {
            "" => None,
            "join" => Some(true),
            "leave" => Some(false),
            _ => return Err(Fail::user(l::t("Invalid Roblox community."))),
        };
        if change.is_some() && viewer.is_none() {
            return Err(Fail::user(l::t("Select a signed-in Roblox Global account.")));
        }
        rt::blocking(move || {
            let ct = Cancel::new();
            let viewer = viewer.as_ref().map(|a| (a.user_id, a.cookie.as_str()));
            if let (Some(join), Some((user, cookie))) = (change, viewer) {
                roblox_api::community::change_group(group, user, cookie, join, &ct)
            } else {
                roblox_api::community::read_group(group, viewer, &ct)
            }
        }).await
    }

    pub async fn game_social_links(&self, universe: i64, account: Guid) -> Res<Value> {
        self.vault(|_| ())?;
        let Some(account) = self.find_account(account).filter(|a| {
            universe > 0 && a.realm.is_empty() && a.cookie_valid != Some(false) && !a.cookie.is_empty()
        }) else { return Ok(json!([])) };
        let links = rt::blocking(move || roblox_api::get_game_social_links(universe, &account.cookie, &Cancel::new())).await?;
        Ok(json!(links))
    }

    pub async fn game_details(self: &Rc<Self>, place_id: i64) -> Res<Value> {
        self.vault(|_| ())?;
        if place_id <= 0 {
            return Err(Fail::user(l::t("Invalid Place ID.")));
        }
        let mut universe = self.controller.games.borrow().get(&place_id).cloned().flatten().map_or(0, |g| g.universe_id);
        if universe == 0 {
            universe = rt::blocking(move || roblox_api::get_universe_id(place_id, &Cancel::new())).await?;
        }
        let cookie = self.any_global_cookie();
        let details = if universe > 0 { rt::blocking(move || roblox_api::get_game_details(universe, cookie.as_deref(), &Cancel::new())).await? } else { None };
        let Some(details) = details else {
            return Err(Fail::user(l::t("Roblox has no game with this Place ID.")));
        };
        if details.playing > 0 {
            game_hours::note(universe, details.playing, Stamp::utc_now());
            game_hours::save();
        }
        let icon: Option<String> = match rt::blocking(move || roblox_api::get_game_icons(vec![universe], &Cancel::new())).await {
            Ok(icons) => icons.get(&universe).cloned(),
            // The page shows a plain tile instead.
            Err(ex) if roblox_api::is_expected_error(&ex) => None,
            Err(ex) => return Err(ex),
        };
        Ok(json!({
            "placeId": place_id, "universe": universe, "name": details.name, "creator": details.creator, "genre": details.genre, "iconUrl": icon,
            "creatorGroupId": details.creator_group_id, "creatorVerified": details.creator_verified, "rootPlaceId": details.root_place_id,
            "description": details.description, "playing": details.playing, "visits": details.visits, "favorites": details.favorites,
            "likes": details.likes_percent, "created": details.created, "updated": details.updated,
            "upVotes": details.up_votes, "downVotes": details.down_votes, "maxPlayers": details.max_players,
            "accounts": self.game_accounts(place_id, universe, Stamp::utc_now())?,
            "hours": game_hours::of(universe),
        }))
    }

    /// The accounts that played this game in the 7 days before `now` (any of its places), most played first: how long,
    /// kicks (by the game or for idling) and other drops, and whether it's in it now or banned from it.
    pub fn game_accounts(&self, place_id: i64, universe: i64, now: Stamp) -> Res<Vec<Value>> {
        let sessions: Vec<PlaySession> = self.vault(|d| d.play_sessions.clone())?;
        let known = |place: i64| self.known_game_of(place);
        let hide = prefs::lock().hide_names;
        Ok(App::game_accounts_of(&sessions, &known, place_id, universe, now)
            .into_iter()
            .map(|r| {
                let account = self.find_account(r.id);
                // Hide account names applies here too; a deleted account goes by the name it had.
                let name = match &account {
                    Some(a) => self.name_text(a),
                    None if hide => "–".to_string(),
                    None => r.label.clone().unwrap_or_else(|| "–".to_string()),
                };
                let state = if r.now {
                    "ingame"
                } else if account.as_ref().is_some_and(|a| self.game_ban_of(a, Some(place_id)).is_some()) {
                    "banned"
                } else {
                    ""
                };
                json!({
                    "id": r.id, "name": name, "minutes": (r.seconds / 60.0).round_ties_even() as i32, "kicks": r.kicks,
                    "kickWhy": if r.idle { Some("AFK") } else { None }, "drops": r.drops,
                    "now": state,
                    "lastPlayed": if r.now { None } else { Some(r.last) },
                })
            })
            .collect())
    }

    /// The sum behind `game_accounts`, apart from the accounts' names and bans: most played first. The GPUI interface makes
    /// the same sum itself from the raw stays (ui-gpui/src/history.rs, since 2026-11-11); one table of cases,
    /// Tests/game-accounts-cases.json, holds both to the same answers (suite "game-accounts" here, a test there).
    pub fn game_accounts_of(sessions: &[PlaySession], known: &dyn Fn(i64) -> Option<KnownGame>, place_id: i64, universe: i64, now: Stamp) -> Vec<GameStay> {
        let since = now.plus_secs(-7.0 * 86400.0);
        // A stay begun before its place was looked up has no universe: its place tells, from another stay or the cache.
        let mut universe_of: HashMap<i64, i64> = HashMap::new();
        for s in sessions.iter().filter(|s| s.universe_id > 0) {
            universe_of.insert(s.place_id, s.universe_id);
        }
        let in_game = |s: &PlaySession| -> bool {
            if s.place_id == place_id {
                return true;
            }
            if universe <= 0 {
                return false;
            }
            let u = if s.universe_id > 0 {
                s.universe_id
            } else {
                match universe_of.get(&s.place_id).copied() {
                    Some(u) if u > 0 => u,
                    _ => known(s.place_id).map_or(0, |k| k.universe),
                }
            };
            u == universe
        };
        // Grouped by account, in the order they first come.
        let mut groups: Vec<(Guid, Vec<&PlaySession>)> = Vec::new();
        for s in sessions.iter().filter(|s| in_game(s)).filter(|s| s.ended_at.unwrap_or(now) > since) {
            match groups.iter_mut().find(|g| g.0 == s.account_id) {
                Some(g) => g.1.push(s),
                None => groups.push((s.account_id, vec![s])),
            }
        }
        let mut rows: Vec<GameStay> = Vec::new();
        for (account, group) in groups {
            let seconds: f64 = group.iter().map(|s| s.ended_at.unwrap_or(now).secs_since(if s.started_at > since { s.started_at } else { since }).max(0.0)).sum();
            let ended: Vec<&&PlaySession> = group.iter().filter(|s| s.end_reason.as_deref() == Some("disconnected") && s.ended_at.is_some_and(|e| e > since)).collect();
            let kicks = ended.iter().filter(|s| s.reason_code.is_some_and(|c| KICK_CODES.contains(&c))).count() as i32;
            rows.push(GameStay {
                id: account,
                seconds,
                kicks,
                idle: ended.iter().any(|s| s.reason_code == Some(IDLE_KICK_CODE)),
                drops: ended.len() as i32 - kicks,
                now: group.iter().any(|s| s.ended_at.is_none()),
                last: group.iter().map(|s| s.ended_at.unwrap_or(now)).max().unwrap_or(now),
                label: group.last().and_then(|s| s.account_label.clone()),
            });
        }
        rows.sort_by(|a, b| b.seconds.partial_cmp(&a.seconds).unwrap_or(std::cmp::Ordering::Equal));
        rows
    }

    /// Its wiki: a web search for it, since each game's wiki lives on a site of its own.
    pub fn open_game_wiki(name: &str) -> Res<Value> {
        if name.trim().is_empty() {
            return Ok(Value::Null);
        }
        App::open_url(&format!("https://www.google.com/search?q={}", roblox_api::escape_data_string(&format!("{} Roblox wiki", name.trim()))))
    }

    pub fn open_game_page(place_id: i64) -> Res<Value> {
        if place_id <= 0 {
            return Err(Fail::user(l::t("Invalid Place ID.")));
        }
        App::open_url(&format!("https://www.roblox.com/games/{place_id}"))
    }

    /// Every half hour: the player counts of the games being played and the newest few in the game box's list.
    pub fn note_game_hours_if_due(self: &Rc<Self>, now: Stamp) {
        if self.game_page.game_hours_busy.get() || self.game_page.game_hours_at.get().is_some_and(|at| now.secs_since(at) < secs(GAME_HOURS_EVERY)) {
            return;
        }
        if self.held_vault().is_none() {
            return;
        }
        let Some(cookie) = self.any_global_cookie() else {
            return;
        };
        self.game_page.game_hours_at.set(Some(now));
        let universes: Vec<i64> = self
            .vault(|d| {
                let mut list: Vec<i64> = Vec::new();
                for id in d
                    .play_sessions
                    .iter()
                    .filter(|s| s.ended_at.is_none() && s.universe_id > 0)
                    .map(|s| s.universe_id)
                    .chain(d.settings.recent_games.iter().take(GAME_HOURS_RECENT).map(|g| g.universe_id))
                {
                    if id > 0 && !list.contains(&id) {
                        list.push(id);
                    }
                }
                list
            })
            .unwrap_or_default();
        if universes.is_empty() {
            return;
        }
        self.game_page.game_hours_busy.set(true);
        rt::spawn({
            let app = self.clone();
            async move { app.note_game_hours(universes, cookie).await }
        });
    }

    pub async fn note_game_hours(self: &Rc<Self>, universes: Vec<i64>, cookie: String) {
        let answer = rt::blocking(move || -> Res<()> {
            let games = roblox_api::get_games(universes, Some(&cookie), &Cancel::new())?;
            let at = Stamp::utc_now();
            for (id, game) in games {
                game_hours::note(id, game.playing, at);
            }
            game_hours::save();
            Ok(())
        })
        .await;
        // An expected failure: asked again in half an hour.
        if let Err(ex) = answer {
            if !roblox_api::is_expected_error(&ex) {
                // PORT: an unexpected failure ended the fire-and-forget task unobserved in C#; here it is logged.
                crate::core::log::write(&ex);
            }
        }
        self.game_page.game_hours_busy.set(false);
    }
}
