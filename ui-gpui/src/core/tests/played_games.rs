// Tests/Bloxkeep.Tests/PlayedGamesSuite.cs: the games played in the last 90 days, summed from the stays
// (App::played_games_of), against the table Tests/played-games-cases.json. The table's times are minutes from its `now`.
use std::collections::HashMap;

use serde_json::Value;

use super::Checks;
use crate::core::app::App;
use crate::core::app::wiki::KnownGame;
use crate::core::base::Stamp;
use crate::core::models::PlaySession;

/// A cases table of Tests/, or None when it isn't there.
pub(crate) fn cases(name: &str) -> Option<Value> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("Tests").join(name);
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

pub(crate) fn known_games(x: &Value) -> HashMap<i64, KnownGame> {
    x["games"]
        .as_object()
        .map(|o| {
            o.iter()
                .filter_map(|(k, g)| {
                    Some((
                        k.parse().ok()?,
                        KnownGame { universe: g["universe"].as_i64().unwrap_or(0), name: g["name"].as_str().unwrap_or("").to_string(), root: g["root"].as_i64().unwrap_or(0) },
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

pub async fn run(c: &mut Checks, _dir: &str) {
    let clock = 10 * 3_600_000_i64;
    let mut stays: Vec<_> = (1..=35).map(|place| crate::history::Session {
        place, game: format!("Older game {place}"), start: clock - 7_200_000,
        end: Some(clock - 3_600_000), ..Default::default()
    }).collect();
    stays.push(crate::history::Session {
        place: 99, game: "Newest game".into(), start: clock - 60_000, end: Some(clock), ..Default::default()
    });
    let recent = crate::history::recently_played(&stays, &HashMap::new(), clock);
    c.check("a brief recent game appears ahead of older long sessions", recent.len() == 30 && recent[0].place == 99, ());
    c.check("a one-minute session keeps its unrounded duration in the game library",
        recent.first().is_some_and(|game| (game.hours * 60. - 1.).abs() < 0.0001), ());

    let doc = cases("played-games-cases.json");
    c.check("The table of cases is found", doc.is_some(), "");
    let Some(doc) = doc else { return };
    let now = Stamp::from_unix_ms(doc["now"].as_i64().unwrap_or(0));
    let at = |minutes: &Value| now.plus_secs(minutes.as_f64().unwrap_or(0.0) * 60.0);
    let at_or_none = |x: &Value, k: &str| x.get(k).filter(|v| v.is_number()).map(at);
    let mut n = 0;
    for x in doc["cases"].as_array().cloned().unwrap_or_default() {
        n += 1;
        let sessions: Vec<PlaySession> = x["sessions"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|s| PlaySession {
                place_id: s["place"].as_i64().unwrap_or(0),
                universe_id: s["universe"].as_i64().unwrap_or(0),
                game_name: s["game"].as_str().unwrap_or("").to_string(),
                started_at: at(&s["start"]),
                ended_at: at_or_none(s, "end"),
                last_seen_at: at_or_none(s, "seen"),
                ..Default::default()
            })
            .collect();
        let known = known_games(&x);
        let got = App::played_games_of(&sessions, &|p| known.get(&p).cloned(), now);
        let want = x["expect"].as_array().cloned().unwrap_or_default();
        let same = got.len() == want.len()
            && got.iter().zip(&want).all(|(g, w)| {
                Some(g.name.as_str()) == w["name"].as_str()
                    && Some(g.place) == w["place"].as_i64()
                    && (g.hours - w["hours"].as_f64().unwrap_or(f64::NAN)).abs() < 1e-9
                    && g.last == at(&w["last"])
            });
        c.check(
            x["what"].as_str().unwrap_or(""),
            same,
            got.iter().map(|g| format!("{} place {} {} h", g.name, g.place, g.hours)).collect::<Vec<_>>().join("; "),
        );
    }
    c.check("The table has its cases", n >= 12, n);
}
