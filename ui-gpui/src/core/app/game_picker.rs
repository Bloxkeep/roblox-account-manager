// AppController.GamePicker.cs: the game box's lists: recent, pinned, favourites, popular, and search.
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
    time::Duration,
};

use serde_json::{Map, Value, json};

use super::App;
use super::games::MAX_CACHED_GAMES;
use super::presets::{same_vault, secs};
use crate::core::{
    base::{Cancel, Fail, Res, Stamp},
    l, log,
    models::RecentGame,
    roblox_api::{self, GameSummary, realm},
    rt,
};

pub const GAME_LIST_LIFETIME: Duration = Duration::from_secs(10 * 60);

#[derive(Default)]
pub struct GamePickerState {
    pub icon_by_universe: RefCell<HashMap<i64, String>>,
    pub favorite_games: RefCell<Option<(Vec<GameSummary>, Stamp)>>,
    pub popular_games: RefCell<Option<(Vec<GameSummary>, Stamp)>>,
}

impl App {
    /// Icons not known yet, in one request per 100 games.
    pub async fn fetch_icons(self: &Rc<Self>, games: &[GameSummary]) -> Res<()> {
        if self.game_picker.icon_by_universe.borrow().len() > MAX_CACHED_GAMES {
            self.game_picker.icon_by_universe.borrow_mut().clear(); // a link per game ever shown; the ones on screen are asked for again
        }
        let mut missing: Vec<i64> = Vec::new();
        {
            let known = self.game_picker.icon_by_universe.borrow();
            for g in games {
                if g.universe_id > 0 && !known.contains_key(&g.universe_id) && !missing.contains(&g.universe_id) {
                    missing.push(g.universe_id);
                }
            }
        }
        if missing.is_empty() {
            return Ok(());
        }
        let asked = missing.clone();
        match rt::blocking(move || roblox_api::get_game_icons(asked, &Cancel::new())).await {
            Ok(icons) => self.game_picker.icon_by_universe.borrow_mut().extend(icons),
            // Rows still work without icons.
            Err(ex) if roblox_api::is_expected_error(&ex) => {
                log::note(&format!("Game picker: icons of {} game(s) not loaded ({}).", missing.len(), App::log_kind_of(&ex)));
            }
            Err(ex) => return Err(ex),
        }
        Ok(())
    }

    /// `fetch_icons`: false when the caller already requested the icons.
    pub async fn game_rows(self: &Rc<Self>, games: &[GameSummary], fetch_icons: bool) -> Res<Vec<Value>> {
        let mut list: Vec<GameSummary> = Vec::new();
        for g in games {
            if !list.iter().any(|x| x.place_id == g.place_id) {
                list.push(g.clone());
            }
        }
        if fetch_icons {
            self.fetch_icons(&list).await?;
        }
        let pinned: HashSet<i64> = self.vault(|d| d.settings.pinned_games.iter().map(|p| p.place_id).collect()).unwrap_or_default();
        let icons = self.game_picker.icon_by_universe.borrow();
        Ok(list
            .iter()
            .map(|g| {
                json!({
                    "placeId": g.place_id,
                    "universeId": g.universe_id,
                    "name": g.name,
                    "playing": g.playing,
                    "iconUrl": icons.get(&g.universe_id),
                    "pinned": pinned.contains(&g.place_id),
                })
            })
            .collect())
    }

    pub async fn search_games(self: &Rc<Self>, query: &str) -> Res<Value> {
        let query = query.trim().to_string();
        if query.is_empty() {
            return Ok(json!([]));
        }
        let found = rt::blocking(move || roblox_api::search_games(&query, &Cancel::new())).await?;
        let first: Vec<GameSummary> = found.into_iter().take(12).collect();
        Ok(Value::Array(self.game_rows(&first, true).await?))
    }

    pub async fn game_icons(self: &Rc<Self>, a: &Value) -> Res<Value> {
        let mut ids: Vec<i64> = Vec::new();
        if let Some(arr) = a.get("universeIds").and_then(|v| v.as_array()) {
            for x in arr {
                let v = x.as_i64().unwrap_or(0);
                if v > 0 && !ids.contains(&v) {
                    ids.push(v);
                }
                if ids.len() >= 50 {
                    break;
                }
            }
        }
        if self.game_picker.icon_by_universe.borrow().len() > MAX_CACHED_GAMES {
            self.game_picker.icon_by_universe.borrow_mut().clear(); // a link per game ever shown; the ones on screen are asked for again
        }
        let missing: Vec<i64> = {
            let known = self.game_picker.icon_by_universe.borrow();
            ids.iter().filter(|id| !known.contains_key(id)).copied().collect()
        };
        if !missing.is_empty() {
            let asked = missing.clone();
            match rt::blocking(move || roblox_api::get_game_icons(asked, &Cancel::new())).await {
                Ok(icons) => self.game_picker.icon_by_universe.borrow_mut().extend(icons),
                // The rows keep their placeholder.
                Err(ex) if roblox_api::is_expected_error(&ex) => {
                    log::note(&format!("Game picker: icons of {} game(s) not loaded ({}).", missing.len(), App::log_kind_of(&ex)));
                }
                Err(ex) => return Err(ex),
            }
        }
        let known = self.game_picker.icon_by_universe.borrow();
        let mut out = Map::new();
        for id in ids {
            if let Some(url) = known.get(&id) {
                out.insert(id.to_string(), json!(url));
            }
        }
        Ok(Value::Object(out))
    }

    /// Recent and pinned games plus, cached for a while, the accounts' Roblox favourites and the popular chart.
    pub async fn game_lists(self: &Rc<Self>) -> Res<Value> {
        let vault = self.held_vault().ok_or_else(|| Fail::user(l::t("Bloxkeep is locked.")))?;
        // Roblox's own accounts only: a Roblox VNG user id is a stranger's on roblox.com (review, 2026-10-30).
        let users: Vec<i64> = self.vault(|d| d.accounts.iter().filter(|a| a.user_id > 0 && a.realm == realm::GLOBAL).take(5).map(|a| a.user_id).collect())?;
        let now = Stamp::utc_now();
        let fav_kept = self.game_picker.favorite_games.borrow().clone().filter(|(_, at)| now.secs_since(*at) <= secs(GAME_LIST_LIFETIME)).map(|(g, _)| g);
        let pop_kept = self.game_picker.popular_games.borrow().clone().filter(|(_, at)| now.secs_since(*at) <= secs(GAME_LIST_LIFETIME)).map(|(g, _)| g);
        let fav_reused = fav_kept.is_some();
        let pop_reused = pop_kept.is_some();
        let cookie = self.any_global_cookie();
        // Both lists side by side; an expected failure is noted and gives an empty list, any other fails the request.
        // The two bools: that list came back empty because its reads failed.
        type Lists = (Vec<GameSummary>, Vec<GameSummary>, Vec<String>, bool, bool);
        let lists: Res<Lists> = rt::blocking(move || {
            let ct = Cancel::new();
            crate::threads::scope(|scope| {
                let fav = scope.spawn(|| -> Res<(Vec<GameSummary>, Vec<String>)> {
                    if let Some(g) = fav_kept {
                        return Ok((g, Vec::new()));
                    }
                    let lists: Vec<Res<(Vec<GameSummary>, Option<String>)>> = crate::threads::scope(|inner| {
                        let handles: Vec<_> = users
                            .iter()
                            .map(|user| {
                                let ct = &ct;
                                inner.spawn(move || match roblox_api::get_favorite_games(*user, ct) {
                                    Ok(list) => Ok((list, None)),
                                    Err(ex) if roblox_api::is_expected_error(&ex) => {
                                        Ok((Vec::new(), Some(format!("Game picker: an account's favorites not loaded ({}).", App::log_kind_of(&ex)))))
                                    }
                                    Err(ex) => Err(ex),
                                })
                            })
                            .collect();
                        handles.into_iter().map(|h| h.join().unwrap_or_else(|_| Err(Fail::other("favorites thread died")))).collect()
                    });
                    let mut all: Vec<GameSummary> = Vec::new();
                    let mut notes = Vec::new();
                    for answer in lists {
                        let (list, note) = answer?;
                        notes.extend(note);
                        for g in list {
                            if !all.iter().any(|x| x.place_id == g.place_id) {
                                all.push(g);
                            }
                        }
                    }
                    all.truncate(30);
                    Ok((all, notes))
                });
                let pop = scope.spawn(|| -> Res<(Vec<GameSummary>, Option<String>)> {
                    match pop_kept {
                        Some(g) => Ok((g, None)),
                        None => match roblox_api::get_popular_games(cookie.as_deref(), &ct) {
                            Ok(list) => Ok((list.into_iter().take(30).collect(), None)),
                            Err(ex) if roblox_api::is_expected_error(&ex) => {
                                Ok((Vec::new(), Some(format!("Game picker: popular games not loaded ({}).", App::log_kind_of(&ex)))))
                            }
                            Err(ex) => Err(ex),
                        },
                    }
                });
                let fav = fav.join().unwrap_or_else(|_| Err(Fail::other("favorites thread died")));
                let pop = pop.join().unwrap_or_else(|_| Err(Fail::other("popular thread died")));
                let (favorites, mut notes) = fav?;
                let (popular, note) = pop?;
                // Any account's read failed, even with others' games in: a part list isn't kept for the whole lifetime
                // (one account's favourites standing in for five).
                let fav_failed = !notes.is_empty();
                let pop_failed = popular.is_empty() && note.is_some();
                notes.extend(note);
                Ok((favorites, popular, notes, fav_failed, pop_failed))
            })
        })
        .await;
        let (favorites, popular, notes, fav_failed, pop_failed) = lists?;
        for note in notes {
            log::note(&note);
        }
        // Locked (and maybe another vault opened) meanwhile: these were this vault's accounts' favourites, so not kept for the
        // next one (review, 2026-10-30).
        if !same_vault(&self.held_vault(), &Some(vault)) {
            return Err(Fail::user(l::t("Bloxkeep was locked.")));
        }
        // An empty list from failed reads (no connection for a moment) isn't kept: the next look tries again rather than
        // showing nothing for the whole lifetime.
        {
            if !fav_failed {
                let fav_at = if fav_reused { self.game_picker.favorite_games.borrow().as_ref().map_or(now, |f| f.1) } else { now };
                *self.game_picker.favorite_games.borrow_mut() = Some((favorites.clone(), fav_at));
            }
            if !pop_failed {
                let pop_at = if pop_reused { self.game_picker.popular_games.borrow().as_ref().map_or(now, |p| p.1) } else { now };
                *self.game_picker.popular_games.borrow_mut() = Some((popular.clone(), pop_at));
            }
        }

        let (recent_saved, pinned_saved): (Vec<RecentGame>, Vec<RecentGame>) = self.vault(|d| (d.settings.recent_games.clone(), d.settings.pinned_games.clone()))?;
        let recent: Vec<GameSummary> = recent_saved
            .iter()
            .map(|g| GameSummary {
                place_id: g.place_id,
                universe_id: g.universe_id,
                name: if !g.name.is_empty() { g.name.clone() } else { format!("Place {}", g.place_id) },
                playing: 0,
            })
            .collect();
        let pinned: Vec<GameSummary> =
            pinned_saved.iter().map(|g| GameSummary { place_id: g.place_id, universe_id: g.universe_id, name: g.name.clone(), playing: 0 }).collect();
        let all: Vec<GameSummary> = recent.iter().chain(pinned.iter()).chain(favorites.iter()).chain(popular.iter()).cloned().collect();
        self.fetch_icons(&all).await?;
        Ok(json!({
            "recent": self.game_rows(&recent, true).await?,
            "pinned": self.game_rows(&pinned, true).await?,
            "favorites": self.game_rows(&favorites, true).await?,
            "popular": self.game_rows(&popular, true).await?,
        }))
    }

    pub fn toggle_pin(&self, place_id: i64, universe_id: i64, name: &str) -> Res<Value> {
        let now_pinned = self.vault_mut(|d| {
            let pinned = &mut d.settings.pinned_games;
            let before = pinned.len();
            pinned.retain(|g| g.place_id != place_id);
            let now_pinned = pinned.len() == before;
            if now_pinned && place_id > 0 {
                pinned.insert(0, RecentGame { place_id, universe_id, name: name.trim().to_string(), ..RecentGame::default() });
                if pinned.len() > 30 {
                    pinned.truncate(30);
                }
            }
            now_pinned
        })?;
        self.persist();
        self.push_state();
        Ok(json!(now_pinned))
    }

    pub fn remove_recent(&self, place_id: i64) -> Res<Value> {
        self.vault_mut(|d| d.settings.recent_games.retain(|g| g.place_id != place_id))?;
        self.persist();
        self.push_state();
        Ok(Value::Null)
    }
}
