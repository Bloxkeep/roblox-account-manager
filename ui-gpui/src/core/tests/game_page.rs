// Tests/Bloxkeep.Tests/GamePageSuite.cs: a game's page (app/game_page.rs, game_hours.rs), against a stand-in Roblox: its
// figures (likes from the votes, the game's own last update over the universe's, nothing without a cookie but what anyone
// sees); your accounts in it over 7 days, any of its places; the hours it's busy, noted and kept; a game Roblox doesn't know.
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::{call, unlock, user_error, window::quiet_app as app, Checks, PASSWORD};
use crate::core::{
    base::Stamp,
    game_hours,
    models::{Account, PlaySession},
    roblox_api::{self, HttpRequest, HttpResponse},
    vault::Vault,
};

const PLACE: i64 = 2753915549;
const OTHER_PLACE: i64 = 4442272183;
const UNIVERSE: i64 = 994732206;
const OTHER_GAME: i64 = 1;

pub async fn run(c: &mut Checks, dir: &str) {
    game_hours::reset();

    // ── the hours ──
    // An hour far from now: the page notes the count it just saw at this hour, which mustn't land on the one checked.
    let at = (Stamp::now().local().hour as usize + 12) % 24;
    let next = (at + 1) % 24;
    let noon = Stamp::from_local(2026, 10, 29, at as u32, 0, 0).to_utc();
    game_hours::note(UNIVERSE, 1000, noon);
    game_hours::note(UNIVERSE, 2000, noon.plus_secs(86400.0));
    game_hours::note(UNIVERSE, 0, noon.plus_secs(3600.0)); // nothing said: not noted
    let hours = game_hours::of(UNIVERSE);
    c.check(
        "an hour's figure leans on the newest (1000, then 2000: 1300), other hours unknown",
        hours.as_ref().is_some_and(|h| h.len() == 24 && h[at] == Some(1300) && h[next].is_none() && h.iter().filter(|x| x.is_some()).count() == 1),
        &hours,
    );
    game_hours::save();
    game_hours::reset();
    c.check(
        "…kept in the data folder",
        game_hours::of(UNIVERSE).and_then(|h| h[at]) == Some(1300) && std::path::Path::new(dir).join("game-hours.json").is_file(),
        game_hours::of(UNIVERSE),
    );
    for g in 100..145i64 {
        game_hours::note(g, 5, noon.plus_secs(2.0 * 86400.0 + g as f64 * 60.0));
    }
    c.check(
        "…for the 40 games noted last",
        game_hours::of(UNIVERSE).is_none() && game_hours::of(144).is_some() && game_hours::of(104).is_none() && game_hours::of(105).is_some(),
        (),
    );
    game_hours::note(UNIVERSE, 1300, noon);

    // ── the accounts ──
    let now = Stamp::utc_now();
    let seed = Vault::create(PASSWORD, None).expect("vault");
    let farm = Account { username: "Farm".into(), user_id: 4001, cookie: "_TEST_gp_A".into(), cookie_valid: Some(true), ..Default::default() };
    let main = Account { username: "Main".into(), user_id: 4002, cookie: "_TEST_gp_B".into(), cookie_valid: Some(true), ..Default::default() };
    let stay = |a: &Account, place: i64, universe: i64, hours_ago: f64, length: f64, end: Option<&str>, code: Option<i32>| PlaySession {
        account_id: a.id,
        account_label: Some(a.username.clone()),
        place_id: place,
        universe_id: universe,
        game_name: "Blox Fruits".into(),
        started_at: now.plus_secs(-hours_ago * 3600.0),
        ended_at: end.map(|_| now.plus_secs((-hours_ago + length) * 3600.0)),
        end_reason: end.map(str::to_string),
        reason_code: code,
        ..Default::default()
    };
    {
        let mut d = seed.data();
        d.accounts.push(farm.clone());
        d.accounts.push(main.clone());
        d.play_sessions.extend([
            stay(&farm, PLACE, UNIVERSE, 30.0, 10.0, Some("disconnected"), Some(278)), // 10 h, kicked for idling
            stay(&farm, OTHER_PLACE, 0, 20.0, 2.0, Some("disconnected"), Some(277)),   // another place of the game: 2 h, a drop
            stay(&farm, OTHER_PLACE, UNIVERSE, 15.0, 1.0, Some("left"), None),         // 1 h
            stay(&main, PLACE, UNIVERSE, 9.0 * 24.0, 5.0, Some("left"), None),         // more than 7 days ago
            stay(&main, PLACE, UNIVERSE, 50.0, 0.5, Some("disconnected"), Some(267)),  // 30 min, kicked by the game
            stay(&main, 999, OTHER_GAME, 3.0, 2.0, Some("left"), None),                // another game
        ]);
    }
    seed.save().expect("saved");
    drop(seed);

    // ── Roblox's figures ── (set before unlocking, so nothing the unlock starts reaches the real Roblox)
    let asked: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let log = asked.clone();
    *roblox_api::STAND_IN.write().unwrap() = Some(Box::new(move |req: &HttpRequest| {
        let url = req.url.clone();
        let signed_in = req.headers_named("Cookie").any(|v| v.contains("_TEST_gp_"));
        let host = url::Url::parse(&url).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_default();
        log.lock().unwrap().push(format!("{}{host}", if signed_in { "+" } else { "-" }));
        let body = if url.contains("/universes/v1/places/123/") {
            r#"{"universeId":0}"#.to_string()
        } else if url.contains("/universes/v1/places/") {
            format!(r#"{{"universeId":{UNIVERSE}}}"#)
        } else if url.contains("develop.roblox.com") {
            format!(
                r#"{{"data":[{{"id":{UNIVERSE},"name":"Blox Fruits","description":"Become a master swordsman.","rootPlaceId":{PLACE},"creatorName":"Gamer Robot Inc","created":"2019-01-15T09:29:19.417Z","updated":"2026-09-21T18:36:21.337Z"}}]}}"#
            )
        } else if url.contains("/v1/games/votes") {
            format!(r#"{{"data":[{{"id":{UNIVERSE},"upVotes":900,"downVotes":100}}]}}"#)
        } else if url.contains("games.roblox.com/v1/games") {
            if signed_in {
                format!(
                    r#"{{"data":[{{"id":{UNIVERSE},"playing":412873,"visits":48216000000,"favoritedCount":8700000,"genre":"Adventure","updated":"2026-10-28T10:00:00Z"}}]}}"#
                )
            } else {
                r#"{"data":[{"id":0,"playing":0,"visits":0}]}"#.to_string()
            }
        } else if url.contains("thumbnails") {
            format!(r#"{{"data":[{{"targetId":{UNIVERSE},"state":"Completed","imageUrl":"https://tr.rbxcdn.com/icon.png"}}]}}"#)
        } else {
            "{}".to_string()
        };
        Ok(HttpResponse { status: 200, headers: Vec::new(), body: body.into_bytes() })
    }));

    let (app, _said) = app();
    unlock(&app).await;
    // In it now (1 h so far): added once unlocked, or the unlock ends it (no client of it is running here).
    let _ = app.vault_mut(|d| d.play_sessions.push(stay(&farm, PLACE, UNIVERSE, 1.0, 0.0, None, None)));
    let rows = app.game_accounts(PLACE, UNIVERSE, now).unwrap_or_default();
    let row = |r: &Value| {
        format!(
            "{} {} k{} d{} {}",
            r["name"].as_str().unwrap_or(""),
            r["minutes"].as_i64().unwrap_or(-1),
            r["kicks"].as_i64().unwrap_or(-1),
            r["drops"].as_i64().unwrap_or(-1),
            r["now"].as_str().unwrap_or("")
        )
    };
    let shown: Vec<String> = rows.iter().map(row).collect();
    c.check(
        "your accounts in it over 7 days, any of its places, most played first: time, kicks, drops, now",
        rows.len() == 2 && row(&rows[0]) == "Farm 840 k1 d1 ingame" && row(&rows[1]) == "Main 30 k1 d0 ",
        shown.join(" | "),
    );
    c.check(
        "…an idle kick said as AFK, a kick by the game not",
        rows.len() == 2 && rows[0]["kickWhy"] == json!("AFK") && rows[1]["kickWhy"].is_null(),
        (),
    );
    c.check(
        "…when one not in it now last played",
        rows.len() == 2 && rows[1]["lastPlayed"].is_string() && rows[0]["lastPlayed"].is_null(),
        (),
    );

    asked.lock().unwrap().clear();
    let page = call(&app, "gameDetails", json!({ "placeId": PLACE })).await;
    let page = match page {
        Ok(v) => v,
        Err(e) => json!({ "error": e.to_string() }),
    };
    c.check(
        "Roblox's figures: name, maker, players, visits, likes from the votes (90 %), genre",
        page["name"] == json!("Blox Fruits")
            && page["creator"] == json!("Gamer Robot Inc")
            && page["playing"].as_i64() == Some(412873)
            && page["visits"].as_i64() == Some(48216000000)
            && page["likes"].as_i64() == Some(90)
            && page["genre"] == json!("Adventure"),
        page.to_string(),
    );
    let updated = page["updated"].as_str().and_then(Stamp::parse);
    c.check(
        "…the game's own last update (signed in) over the universe's",
        updated == Stamp::parse("2026-10-28T10:00:00Z"),
        page["updated"].to_string(),
    );
    let asked_now = asked.lock().unwrap().clone();
    c.check(
        "…the player count asked with an account's sign-in (Roblox hides it otherwise), the rest without",
        asked_now.iter().any(|a| a == "+games.roblox.com") && asked_now.iter().any(|a| a == "-develop.roblox.com"),
        asked_now.join(","),
    );
    let this_hour = Stamp::now().local().hour as usize;
    let hours = page["hours"].as_array().cloned().unwrap_or_default();
    c.check(
        "…its hours, with the count just seen noted too",
        hours.len() == 24 && hours[at].as_i64() == Some(1300) && hours[this_hour].is_number() && page["accounts"].as_array().is_some_and(|a| a.len() == 2),
        page["hours"].to_string(),
    );
    let unknown = call(&app, "gameDetails", json!({ "placeId": 123 })).await;
    c.check(
        "a Place ID Roblox doesn't know: said so",
        user_error(&unknown).as_deref() == Some("Roblox has no game with this Place ID."),
        user_error(&unknown),
    );
    c.check(
        "an invalid one for its roblox.com page: refused, nothing opened",
        user_error(&call(&app, "openGamePage", json!({ "placeId": 0 })).await).is_some(),
        (),
    );
    c.check(
        "a wiki for no name: nothing opened",
        matches!(call(&app, "openGameWiki", json!({ "name": " " })).await, Ok(Value::Null)),
        (),
    );
    game_hours::reset();
}
