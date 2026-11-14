// GameHours.cs: how many play a game at each hour of the day.
//! How many play a game at each hour of the day (this PC's time), for its page's "When it is quiet": while Bloxkeep is
//! open it notes the player count Roblox gives for the games in use (see AppController.GamePage.cs), each hour's figure
//! an average that leans on the newest (so a game's new season shows within days). Only Roblox's public figures, kept
//! in the data folder (game-hours.json), for the 40 games noted last.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::core::base::Stamp;
use crate::core::roblox_api::lock;
use crate::core::{durable_file, vault};

const MAX_GAMES: usize = 40;
/// A new figure's weight in its hour's average.
const NEW_WEIGHT: f64 = 0.3;

fn default_hours() -> Option<Vec<Option<f64>>> {
    Some(vec![None; 24])
}

/// DateTime's default (0001-01-01).
fn min_stamp() -> Stamp {
    Stamp::from_unix_secs(-62_135_596_800)
}

/// Hours missing: 24 empty (the C#'s initialiser); null: the game is dropped on reading.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Game {
    #[serde(default = "default_hours")]
    hours: Option<Vec<Option<f64>>>,
    #[serde(default = "min_stamp")]
    noted: Stamp,
}

impl Default for Game {
    fn default() -> Self {
        Game { hours: default_hours(), noted: min_stamp() }
    }
}

static GAMES: Mutex<Option<HashMap<i64, Game>>> = Mutex::new(None);

fn file_path() -> PathBuf {
    vault::data_dir().join("game-hours.json")
}

fn games(slot: &mut Option<HashMap<i64, Game>>) -> &mut HashMap<i64, Game> {
    if slot.is_none() {
        let path = file_path();
        let read: Option<HashMap<i64, Option<Game>>> =
            if path.is_file() { crate::net::saved::<Option<HashMap<i64, Option<Game>>>>("Game hours", &path).flatten() } else { None };
        // A file from another version, or edited by hand: each game with its 24 hours, no other figure.
        let games: HashMap<i64, Game> =
            read.unwrap_or_default().into_iter().filter_map(|(id, g)| g.filter(|g| g.hours.as_ref().is_some_and(|h| h.len() == 24)).map(|g| (id, g))).collect();
        *slot = Some(games);
    }
    slot.as_mut().unwrap()
}

/// The players `playing` a game had at `at` (local time is taken from it).
pub fn note(universe_id: i64, playing: i64, at: Stamp) {
    if universe_id <= 0 || playing <= 0 {
        return;
    }
    let mut slot = lock(&GAMES);
    let games = games(&mut slot);
    if !games.contains_key(&universe_id) {
        if games.len() >= MAX_GAMES
            && let Some(oldest) = games.iter().min_by_key(|(_, g)| g.noted).map(|(id, _)| *id)
        {
            games.remove(&oldest);
        }
        games.insert(universe_id, Game::default());
    }
    let game = games.get_mut(&universe_id).unwrap();
    let hour = at.local().hour as usize;
    let hours = game.hours.get_or_insert_with(|| vec![None; 24]);
    let playing = playing as f64;
    hours[hour] = Some(match hours[hour] {
        Some(was) => was + NEW_WEIGHT * (playing - was),
        None => playing,
    });
    game.noted = at;
}

/// The 24 hours' figures (None: never noted at that hour), or None for a game never noted.
pub fn of(universe_id: i64) -> Option<Vec<Option<i64>>> {
    let mut slot = lock(&GAMES);
    let g = games(&mut slot).get(&universe_id)?;
    Some(g.hours.as_ref()?.iter().map(|h| h.map(|v| v.round_ties_even() as i64)).collect())
}

pub fn save() {
    let slot = lock(&GAMES);
    let Some(games) = slot.as_ref() else { return };
    if let Err(e) = std::fs::create_dir_all(vault::data_dir()) {
        crate::net::wrote("Game hours", &file_path(), Err(e));
        return;
    }
    if let Ok(json) = serde_json::to_string(games) {
        crate::net::wrote("Game hours", &file_path(), durable_file::write_text(file_path(), &json).map_err(|e| format!("{e:?}")));
    }
}

/// Forgets what was read (the checks use another data folder).
pub(crate) fn reset() {
    *lock(&GAMES) = None;
}
