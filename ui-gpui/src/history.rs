// The play history: what the accounts' stays in games add up to. The stays themselves are still kept by the C# app
// (in the vault, written as it watches the Roblox windows); it hands them over raw ("playSessions"), and the sums are
// made here. The first is the games played in the last 90 days, which was the app's PlayedGames
// (RobloxManager/AppController.Wiki.cs); the app keeps its own for the old web page, so one table of cases holds
// both to the same answers: Tests/played-games-cases.json, run by the C# suite "played-games" and the test below.
use std::collections::HashMap;

use serde_json::Value;
use crate::val::int;

/// A stay of an account in a game (times in milliseconds since 1970).
#[derive(Debug, Clone, Default)]
pub struct Session {
    /// Whose it is (the app's id for the account), the name the account had then (none while names are hidden), why
    /// the stay ended ("left", "disconnected", "closed"…) and Roblox's code for a disconnect.
    pub account: String,
    pub label: Option<String>,
    pub reason: String,
    pub code: Option<i64>,
    pub place: i64,
    pub universe: i64,
    pub game: String,
    pub start: i64,
    pub end: Option<i64>,
    /// When the window was last seen still playing: the end of a stay nobody saw end.
    pub seen: Option<i64>,
}

/// A game the app has looked up: its universe, name and start place.
#[derive(Debug, Clone, Default)]
pub struct Known {
    pub universe: i64,
    pub name: String,
    pub root: i64,
}

/// A game played: its name, a place to open it by, the hours in it and when its latest stay began.
#[derive(Debug, Clone, PartialEq)]
pub struct Played {
    pub name: String,
    pub place: i64,
    pub hours: f64,
    pub last: i64,
}

/// How far back the history is counted.
pub const HISTORY_MS: i64 = 90 * 24 * 3600 * 1000;

/// The app's "playSessions" answer, read: the stays, the games it knows, and its clock.
pub fn read(v: &Value) -> (Vec<Session>, HashMap<i64, Known>, i64) {
    let sessions = v
        .get("sessions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|s| Session {
            account: s.get("account").and_then(Value::as_str).unwrap_or_default().to_string(),
            label: s.get("label").and_then(Value::as_str).map(str::to_string),
            reason: s.get("reason").and_then(Value::as_str).unwrap_or_default().to_string(),
            code: s.get("code").and_then(Value::as_i64),
            place: int(s, "place"),
            universe: int(s, "universe"),
            game: s.get("game").and_then(Value::as_str).unwrap_or_default().to_string(),
            start: int(s, "start"),
            end: s.get("end").and_then(Value::as_i64),
            seen: s.get("seen").and_then(Value::as_i64),
        })
        .collect();
    let known = v
        .get("games")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(place, g)| Some((place.parse().ok()?, Known { universe: int(g, "universe"), name: g.get("name").and_then(Value::as_str).unwrap_or_default().to_string(), root: int(g, "root") })))
        .collect();
    (sessions, known, int(v, "now"))
}

/// The games played in the 90 days before `now`, most played first, thirty at most. Stays are of one game when they
/// share a universe: a stay's own, else the one another stay of its place has, else the one the app knows for the
/// place, else the place stands alone. A game nothing names is left out.
#[cfg(test)]
pub fn played(sessions: &[Session], known: &HashMap<i64, Known>, now: i64) -> Vec<Played> {
    let mut games = played_in_order(sessions, known, now, |a, b| b.hours.total_cmp(&a.hours));
    for g in &mut games {
        g.hours = (g.hours * 10.).round_ties_even() / 10.;
    }
    games
}

/// Recently started games for Home and Keep playing. Sort before applying the history limit.
pub fn recently_played(sessions: &[Session], known: &HashMap<i64, Known>, now: i64) -> Vec<Played> {
    played_in_order(sessions, known, now, |a, b| b.last.cmp(&a.last))
}

fn played_in_order(
    sessions: &[Session], known: &HashMap<i64, Known>, now: i64,
    sort: impl FnMut(&Played, &Played) -> std::cmp::Ordering,
) -> Vec<Played> {
    let since = now - HISTORY_MS;
    // (Of every stay, not only the 90 days'; the last one of a place tells.)
    let mut universe_of: HashMap<i64, i64> = HashMap::new();
    for s in sessions.iter().filter(|s| s.universe > 0) {
        universe_of.insert(s.place, s.universe);
    }
    let key = |s: &Session| -> i64 {
        if s.universe > 0 {
            s.universe
        } else if let Some(u) = universe_of.get(&s.place).copied().filter(|u| *u > 0) {
            u
        } else if let Some(u) = known.get(&s.place).map(|g| g.universe).filter(|u| *u > 0) {
            u
        } else {
            -s.place
        }
    };
    let ended = |s: &Session| s.end.or(s.seen).unwrap_or(now);
    // The games in the order they first come, each with its stays in theirs (the app's sort keeps that order on a tie).
    let mut groups: Vec<(i64, Vec<&Session>)> = Vec::new();
    for s in sessions.iter().filter(|s| s.place > 0 && ended(s) > since) {
        let k = key(s);
        match groups.iter_mut().find(|g| g.0 == k) {
            Some(g) => g.1.push(s),
            None => groups.push((k, vec![s])),
        }
    }
    // The latest by its start (the first of them on a tie).
    fn latest<'a>(stays: impl Iterator<Item = &'a Session>) -> Option<&'a Session> {
        let mut best: Option<&Session> = None;
        for s in stays {
            if best.is_none_or(|b| s.start > b.start) {
                best = Some(s);
            }
        }
        best
    }
    let mut out: Vec<Played> = groups
        .iter()
        .filter_map(|(_, stays)| {
            let last = latest(stays.iter().copied())?;
            let name = latest(stays.iter().copied().filter(|s| !s.game.is_empty())).map(|s| s.game.clone()).or_else(|| known.get(&last.place).map(|g| g.name.clone())).unwrap_or_default();
            let place = stays.iter().filter_map(|s| known.get(&s.place).map(|g| g.root)).find(|p| *p > 0).unwrap_or(last.place);
            let hours: f64 = stays.iter().map(|s| ((ended(s) - s.start.max(since)) as f64 / 3_600_000.).max(0.)).sum();
            (!name.is_empty()).then_some(Played { name, place, hours, last: last.start })
        })
        .collect();
    out.sort_by(sort);
    out.truncate(30);
    out
}

/// An account's stays in a game over 7 days, summed.
#[derive(Debug, Clone, PartialEq)]
pub struct Stay {
    pub account: String,
    pub seconds: f64,
    /// Kicked by the game (267) or for idling 20 minutes (278); `idle` when one was for idling.
    pub kicks: i64,
    pub idle: bool,
    /// Its other disconnects.
    pub drops: i64,
    /// In it now.
    pub now: bool,
    /// When it last was (now, while it is).
    pub last: i64,
    /// The name its last stay kept.
    pub label: Option<String>,
}

/// Roblox's disconnect codes that are a kick: by the game, for idling.
const KICKS: [i64; 2] = [267, 278];
const IDLE: i64 = 278;

/// The accounts that played a game in the 7 days before `now` (any of its places), most played first: the app's
/// GameAccountsOf. A stay is of the game when it is at `place`, or when the game's `universe` is known and is the
/// stay's (its own, else the one another stay of its place has, else the one the app knows for the place).
pub fn game_accounts(sessions: &[Session], known: &HashMap<i64, Known>, place: i64, universe: i64, now: i64) -> Vec<Stay> {
    let since = now - 7 * 24 * 3600 * 1000;
    let mut universe_of: HashMap<i64, i64> = HashMap::new();
    for s in sessions.iter().filter(|s| s.universe > 0) {
        universe_of.insert(s.place, s.universe);
    }
    let of = |s: &Session| -> i64 {
        if s.universe > 0 {
            s.universe
        } else if let Some(u) = universe_of.get(&s.place).copied().filter(|u| *u > 0) {
            u
        } else {
            known.get(&s.place).map(|g| g.universe).unwrap_or(0)
        }
    };
    // (A stay still open ends now here: when it was last seen is the play history's business, not this one's.)
    let ended = |s: &Session| s.end.unwrap_or(now);
    let mut groups: Vec<(&str, Vec<&Session>)> = Vec::new();
    for s in sessions.iter().filter(|s| (s.place == place || (universe > 0 && of(s) == universe)) && ended(s) > since) {
        match groups.iter_mut().find(|g| g.0 == s.account) {
            Some(g) => g.1.push(s),
            None => groups.push((&s.account, vec![s])),
        }
    }
    let mut out: Vec<Stay> = groups
        .into_iter()
        .map(|(account, stays)| {
            let dropped: Vec<&&Session> = stays.iter().filter(|s| s.reason == "disconnected" && s.end.is_some_and(|e| e > since)).collect();
            let kicks = dropped.iter().filter(|s| s.code.is_some_and(|c| KICKS.contains(&c))).count() as i64;
            Stay {
                account: account.to_string(),
                seconds: stays.iter().map(|s| ((ended(s) - s.start.max(since)) as f64 / 1000.).max(0.)).sum(),
                kicks,
                idle: dropped.iter().any(|s| s.code == Some(IDLE)),
                drops: dropped.len() as i64 - kicks,
                now: stays.iter().any(|s| s.end.is_none()),
                last: stays.iter().map(|s| ended(s)).max().unwrap_or(now),
                label: stays.last().and_then(|s| s.label.clone()),
            }
        })
        .collect();
    out.sort_by(|a, b| b.seconds.total_cmp(&a.seconds));
    out
}

/// A game that banned an account, by the place the ban names.
#[derive(Debug, Clone)]
pub struct Ban {
    pub account: String,
    pub place: i64,
}

/// The bans in the app's "playSessions" answer.
pub fn bans(v: &Value) -> Vec<Ban> {
    v.get("bans").and_then(Value::as_array).into_iter().flatten().map(|b| Ban { account: b.get("account").and_then(Value::as_str).unwrap_or_default().to_string(), place: int(b, "place") }).collect()
}

/// Whether a ban is of this game: the same place, or places the app knows to be of one universe (its SameGame).
pub fn same_game(known: &HashMap<i64, Known>, a: i64, b: i64) -> bool {
    a == b || known.get(&a).map(|g| g.universe).filter(|u| *u > 0).is_some_and(|u| known.get(&b).map(|g| g.universe) == Some(u))
}

/// The universe of a place as the stays and the looked-up games tell it (0: nobody knows).
pub fn universe_of(sessions: &[Session], known: &HashMap<i64, Known>, place: i64) -> i64 {
    sessions.iter().rev().find(|s| s.place == place && s.universe > 0).map(|s| s.universe).or_else(|| known.get(&place).map(|g| g.universe).filter(|u| *u > 0)).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table both this and the C# app are held to for a game's accounts (Tests/game-accounts-cases.json; the C#
    /// suite is "game-accounts"). Its times are minutes from its `now`.
    #[test]
    fn game_accounts_agree_with_the_app() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../Tests/game-accounts-cases.json");
        let table: Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("the cases file")).expect("the cases as JSON");
        let now = table["now"].as_i64().expect("now");
        let at = |m: &Value| m.as_f64().map(|m| now + (m * 60_000.) as i64);
        let cases = table["cases"].as_array().expect("cases");
        assert!(cases.len() >= 10, "the table lost its cases");
        for case in cases {
            let what = case["what"].as_str().unwrap_or_default();
            let sessions: Vec<Session> = case["sessions"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| Session {
                    account: s["account"].as_str().unwrap().into(),
                    label: s["label"].as_str().map(str::to_string),
                    reason: s["reason"].as_str().unwrap_or_default().into(),
                    code: s["code"].as_i64(),
                    place: int(s, "place"),
                    universe: int(s, "universe"),
                    game: String::new(),
                    start: at(&s["start"]).unwrap(),
                    end: at(&s["end"]),
                    seen: None,
                })
                .collect();
            let known: HashMap<i64, Known> = case["games"]
                .as_object()
                .into_iter()
                .flatten()
                .map(|(p, g)| (p.parse().unwrap(), Known { universe: int(g, "universe"), name: g["name"].as_str().unwrap_or_default().into(), root: int(g, "root") }))
                .collect();
            let got = game_accounts(&sessions, &known, int(case, "place"), int(case, "universe"), now);
            let want = case["expect"].as_array().unwrap();
            assert_eq!(got.len(), want.len(), "{what}: {got:?}");
            for (g, w) in got.iter().zip(want) {
                assert_eq!(g.account, w["account"].as_str().unwrap(), "{what}");
                assert!((g.seconds - w["seconds"].as_f64().unwrap()).abs() < 1e-6, "{what}: {} s", g.seconds);
                assert_eq!((g.kicks, g.idle, g.drops, g.now), (int(w, "kicks"), w["idle"].as_bool().unwrap(), int(w, "drops"), w["now"].as_bool().unwrap()), "{what}");
                assert_eq!(Some(g.last), at(&w["last"]), "{what}");
                assert_eq!(g.label.as_deref(), w["label"].as_str(), "{what}");
            }
        }
    }

    /// The table both this and the C# app are held to (Tests/played-games-cases.json; the C# suite is "played-games").
    /// Its times are minutes from its `now` (before it when below zero).
    #[test]
    fn agrees_with_the_app() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../Tests/played-games-cases.json");
        let table: Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("the cases file")).expect("the cases as JSON");
        let now = table["now"].as_i64().expect("now");
        let at = |m: &Value| m.as_f64().map(|m| now + (m * 60_000.) as i64);
        let cases = table["cases"].as_array().expect("cases");
        assert!(cases.len() >= 12, "the table lost its cases");
        for case in cases {
            let what = case["what"].as_str().unwrap_or_default();
            let sessions: Vec<Session> = case["sessions"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| Session { place: int(s, "place"), universe: int(s, "universe"), game: s["game"].as_str().unwrap_or_default().into(), start: at(&s["start"]).unwrap(), end: at(&s["end"]), seen: at(&s["seen"]), ..Default::default() })
                .collect();
            let known: HashMap<i64, Known> = case["games"]
                .as_object()
                .into_iter()
                .flatten()
                .map(|(p, g)| (p.parse().unwrap(), Known { universe: int(g, "universe"), name: g["name"].as_str().unwrap_or_default().into(), root: int(g, "root") }))
                .collect();
            let got = played(&sessions, &known, now);
            let want = case["expect"].as_array().unwrap();
            assert_eq!(got.len(), want.len(), "{what}: {got:?}");
            for (g, w) in got.iter().zip(want) {
                assert_eq!(g.name, w["name"].as_str().unwrap(), "{what}");
                assert_eq!(g.place, int(w, "place"), "{what}: {}", g.name);
                assert!((g.hours - w["hours"].as_f64().unwrap()).abs() < 1e-9, "{what}: {} hours {}", g.name, g.hours);
                assert_eq!(Some(g.last), at(&w["last"]), "{what}: {}", g.name);
            }
        }
    }
}
