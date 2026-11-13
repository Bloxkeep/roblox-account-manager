// RobloxApi.Lookups.cs: public lookups: users, pictures, games, servers and presence.
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use serde_json::{Value, json};

use super::{
    GameDetailsInfo, GameInfo, GameSummary, HTTP_REQUEST_EXCEPTION, HttpResponse, Presence, RobloxApiException, RobloxUser, ServerInfo, as_array,
    get_f64, get_i64, get_prop, get_str, is_expected_error, is_fail_kind, lock, parse_assume_utc, parse_json, post_with_csrf, rate_limited, realm,
    refused, request, send, try_i32, try_i64, try_prop,
};
use crate::core::base::{Cancel, Fail, Res, Stamp, new_guid};
use crate::core::l;
use crate::core::roblox_installs::ClientVersion;

pub(super) fn get_json(url: &str, ct: &Cancel, cookie: Option<&str>) -> Res<Option<Value>> {
    let req = request("GET", url, cookie, false, realm::GLOBAL);
    let res = send(&req, ct)?;
    if res.is_success() { Ok(Some(parse_json(&res.text())?)) } else { Ok(None) }
}

fn is_roblox_cdn(url: Option<&str>) -> bool {
    match url.and_then(|u| url::Url::parse(u).ok()) {
        Some(uri) => uri.scheme() == "https" && uri.host_str().is_some_and(|h| super::ends_with_ci(h, ".rbxcdn.com")),
        None => false,
    }
}

/// The ids above 0, each once, in their order, in pieces of `size`.
fn chunks(ids: impl IntoIterator<Item = i64>, size: usize) -> Vec<Vec<i64>> {
    let mut seen = Vec::new();
    for id in ids {
        if id > 0 && !seen.contains(&id) {
            seen.push(id);
        }
    }
    seen.chunks(size).map(<[i64]>::to_vec).collect()
}

fn joined(ids: &[i64], sep: &str) -> String {
    ids.iter().map(i64::to_string).collect::<Vec<_>>().join(sep)
}

/// The Windows client Roblox currently serves, i.e. what a launcher updates to (no cookie involved).
pub fn get_latest_client_version(ct: &Cancel) -> Res<Option<ClientVersion>> {
    let Some(doc) = get_json("https://clientsettingscdn.roblox.com/v2/client-version/WindowsPlayer", ct, None)? else {
        return Ok(None);
    };
    let text = |name: &str| -> Res<Option<String>> {
        Ok(match try_prop(&doc, name)? {
            Some(Value::String(v)) => Some(v.clone()),
            _ => None,
        })
    };
    Ok(match text("version")?.filter(|v| !v.is_empty()) {
        Some(version) => Some(ClientVersion { version, folder: text("clientVersionUpload")?.unwrap_or_default(), source: None, exe: None }),
        None => None,
    })
}

/// Public avatar headshot URLs (no cookie involved); missing ones are left out.
pub fn get_headshot_urls(user_ids: impl IntoIterator<Item = i64>, ct: &Cancel) -> Res<HashMap<i64, String>> {
    let mut result = HashMap::new();
    for chunk in chunks(user_ids, 100) {
        let url = format!("https://thumbnails.roblox.com/v1/users/avatar-headshot?userIds={}&size=150x150&format=Png&isCircular=false", joined(&chunk, ","));
        let Some(doc) = get_json(&url, ct, None)? else { continue };
        for item in as_array(get_prop(&doc, "data")?)? {
            if get_str(get_prop(item, "state")?)?.as_deref() == Some("Completed")
                && let Some(url) = get_str(get_prop(item, "imageUrl")?)?
                && is_roblox_cdn(Some(&url))
            {
                result.insert(get_i64(get_prop(item, "targetId")?)?, url);
            }
        }
    }
    Ok(result)
}

/// A user's friends, ids only (Roblox leaves the names empty here; see `get_users`).
pub fn get_friend_ids(user_id: i64, ct: &Cancel) -> Res<Vec<i64>> {
    let doc = get_json(&format!("https://friends.roblox.com/v1/users/{user_id}/friends"), ct, None)?;
    let Some(doc) = doc else { return Ok(Vec::new()) };
    let Some(Value::Array(data)) = try_prop(&doc, "data")? else { return Ok(Vec::new()) };
    let mut ids = Vec::new();
    for f in data {
        let id = match try_prop(f, "id")? {
            Some(id) => try_i64(id)?.unwrap_or(0),
            None => 0,
        };
        if id > 0 {
            ids.push(id);
        }
    }
    Ok(ids)
}

fn read_user(u: &Value) -> Res<RobloxUser> {
    Ok(RobloxUser {
        id: get_i64(get_prop(u, "id")?)?,
        name: get_str(get_prop(u, "name")?)?.unwrap_or_default(),
        display_name: get_str(get_prop(u, "displayName")?)?.unwrap_or_default(),
    })
}

/// The user with this username (not display name); None when nobody has it or it is banned (Roblox answers
/// an empty "data" list for an unknown name, checked 2026-10-14).
pub fn get_user_by_name(username: &str, ct: &Cancel) -> Res<Option<RobloxUser>> {
    let mut req = request("POST", "https://users.roblox.com/v1/usernames/users", None, false, realm::GLOBAL);
    req.set_json_body(&json!({ "usernames": [username], "excludeBannedUsers": true }).to_string());
    let res = send(&req, ct)?;
    let body = res.text();
    if res.status == 429 {
        return Err(rate_limited());
    }
    if !res.is_success() {
        return Err(refused("Username lookup", res.status, &body, false));
    }
    let doc = parse_json(&body)?;
    if let Some(u) = as_array(get_prop(&doc, "data")?)?.first() {
        return Ok(Some(read_user(u)?));
    }
    Ok(None)
}

/// Names for many users at once, 100 per request (no cookie involved); banned users are left out.
/// `strict`: a refused request fails (rate limit included) instead of leaving its users out: for a lookup
/// where "not there" must mean that Roblox knows nobody by that id.
pub fn get_users(user_ids: impl IntoIterator<Item = i64>, ct: &Cancel, strict: bool) -> Res<HashMap<i64, RobloxUser>> {
    let mut result = HashMap::new();
    for chunk in chunks(user_ids, 100) {
        let mut req = request("POST", "https://users.roblox.com/v1/users", None, false, realm::GLOBAL);
        req.set_json_body(&json!({ "userIds": chunk, "excludeBannedUsers": true }).to_string());
        let res = send(&req, ct)?;
        if strict && res.status == 429 {
            return Err(rate_limited());
        }
        if strict && !res.is_success() {
            return Err(refused("User lookup", res.status, &res.text(), false));
        }
        if !res.is_success() {
            continue;
        }
        let doc = parse_json(&res.text())?;
        for u in as_array(get_prop(&doc, "data")?)? {
            let user = read_user(u)?;
            result.insert(user.id, user);
        }
    }
    Ok(result)
}

/// The public profile's "isBanned" (users.roblox.com/v1/users/{id}, no cookie; confirmed 2026-10-14). None
/// when it couldn't tell: no such user, or an answer without the field.
pub fn get_banned(user_id: i64, realm: &str, ct: &Cancel) -> Res<Option<bool>> {
    let req = request("GET", &format!("{}/v1/users/{user_id}", realm::api(realm, "users")), None, false, realm::GLOBAL);
    let res = send(&req, ct)?;
    if res.status == 429 {
        return Err(rate_limited());
    }
    if !res.is_success() {
        return Ok(None);
    }
    let doc = parse_json(&res.text())?;
    Ok(match &doc {
        Value::Object(o) => match o.get("isBanned") {
            Some(Value::Bool(b)) => Some(*b),
            _ => None,
        },
        _ => None,
    })
}

/// The universe (experience) a place belongs to, or 0 when Roblox doesn't know the place.
pub fn get_universe_id(place_id: i64, ct: &Cancel) -> Res<i64> {
    let doc = get_json(&format!("https://apis.roblox.com/universes/v1/places/{place_id}/universe"), ct, None)?;
    Ok(match &doc {
        Some(doc) => match try_prop(doc, "universeId")? {
            Some(u @ Value::Number(_)) => get_i64(u)?,
            _ => 0,
        },
        None => 0,
    })
}

/// Game details for previews: name, creator, live player count and icon.
pub fn get_game_info(place_id: i64, cookie: Option<&str>, ct: &Cancel) -> Res<Option<GameInfo>> {
    let universe_id = get_universe_id(place_id, ct)?;
    if universe_id == 0 {
        return Ok(None);
    }
    let mut games = get_games([universe_id], cookie, ct)?;
    Ok(games.remove(&universe_id).map(|info| GameInfo { place_id, ..info }))
}

/// Many games' details, keyed by universe, three requests per 50 games. Since 2026 games.roblox.com hides titles
/// from anonymous callers, so names come from develop.roblox.com and the player count needs a cookie. Games without
/// a name are left out (and asked again later).
pub fn get_games(universe_ids: impl IntoIterator<Item = i64>, cookie: Option<&str>, ct: &Cancel) -> Res<HashMap<i64, GameInfo>> {
    let mut result = HashMap::new();
    for chunk in chunks(universe_ids, 50) {
        let (details, counts, icons) = std::thread::scope(|s| {
            let details = s.spawn(|| get_universe_details(&chunk, ct));
            let counts = s.spawn(|| get_player_counts(&chunk, cookie, ct));
            let icons = s.spawn(|| get_game_icons(chunk.iter().copied(), ct));
            (joined_result(details.join()), joined_result(counts.join()), joined_result(icons.join()))
        });
        let details = details?;
        // The names are what counts: a player count or icon that timed out or came back garbled is left blank instead
        // of losing the names too, which the game list then remembered as unknown for 2 minutes (review, 2026-10-30).
        let first = counts.as_ref().err().or(icons.as_ref().err());
        if let Some(ex) = first
            && (ct.is_cancelled() || !is_expected_error(ex))
        {
            return Err(ex.clone());
        }
        for (id, (name, creator, root_place_id)) in details {
            result.insert(
                id,
                GameInfo {
                    place_id: root_place_id,
                    universe_id: id,
                    name,
                    creator,
                    playing: counts.as_ref().ok().and_then(|c| c.get(&id).copied()).unwrap_or(0),
                    icon_url: icons.as_ref().ok().and_then(|i| i.get(&id).cloned()),
                },
            );
        }
    }
    Ok(result)
}

/// A thread's answer, a panic in it as a failure.
pub(crate) fn joined_result<T>(r: std::thread::Result<Res<T>>) -> Res<T> {
    r.unwrap_or_else(|_| Err(Fail::other("a lookup thread panicked")))
}

fn get_universe_details(universe_ids: &[i64], ct: &Cancel) -> Res<HashMap<i64, (String, String, i64)>> {
    let mut result = HashMap::new();
    let query = universe_ids.iter().map(|id| format!("ids={id}")).collect::<Vec<_>>().join("&");
    let Some(doc) = get_json(&format!("https://develop.roblox.com/v1/universes/multiget?{query}"), ct, None)? else {
        return Ok(result);
    };
    let Some(Value::Array(data)) = try_prop(&doc, "data")? else { return Ok(result) };
    for item in data {
        let text = |name: &str| -> Res<String> {
            Ok(match try_prop(item, name)? {
                Some(Value::String(v)) => v.clone(),
                _ => String::new(),
            })
        };
        let num = |name: &str| -> Res<i64> {
            Ok(match try_prop(item, name)? {
                Some(v @ Value::Number(_)) => get_i64(v)?,
                _ => 0,
            })
        };
        if num("id")? > 0 && !text("name")?.is_empty() {
            result.insert(num("id")?, (text("name")?, text("creatorName")?, num("rootPlaceId")?));
        }
    }
    Ok(result)
}

fn get_player_counts(universe_ids: &[i64], cookie: Option<&str>, ct: &Cancel) -> Res<HashMap<i64, i64>> {
    let mut result = HashMap::new();
    let Some(doc) = get_json(&format!("https://games.roblox.com/v1/games?universeIds={}", joined(universe_ids, ",")), ct, cookie)? else {
        return Ok(result);
    };
    let Some(Value::Array(data)) = try_prop(&doc, "data")? else { return Ok(result) };
    for item in data {
        if let Some(id @ Value::Number(_)) = try_prop(item, "id")?
            && let Some(p @ Value::Number(_)) = try_prop(item, "playing")?
        {
            result.insert(get_i64(id)?, get_i64(p)?);
        }
    }
    Ok(result)
}

/// A game's page (AppController.GamePage.cs): its name, maker and description from develop.roblox.com (shown to
/// anyone), its players, visits, favourites, genre and last update from games.roblox.com (blank without a cookie
/// since 2026; the universe's own "updated" is used then), and its likes from the votes (shown to anyone). Checked
/// against Blox Fruits on 2026-10-29. None when Roblox doesn't know the universe.
pub fn get_game_details(universe_id: i64, cookie: Option<&str>, ct: &Cancel) -> Res<Option<GameDetailsInfo>> {
    let (universe, game, votes) = std::thread::scope(|s| {
        let universe = s.spawn(|| get_json(&format!("https://develop.roblox.com/v1/universes/multiget?ids={universe_id}"), ct, None));
        let game = s.spawn(|| get_json(&format!("https://games.roblox.com/v1/games?universeIds={universe_id}"), ct, cookie));
        let votes = s.spawn(|| get_json(&format!("https://games.roblox.com/v1/games/votes?universeIds={universe_id}"), ct, None));
        (joined_result(universe.join()), joined_result(game.join()), joined_result(votes.join()))
    });
    let universe = universe?;
    // Only the universe is needed; the figures are shown as unknown (a network failure only: anything else goes on up).
    if let Some(ex) = game.as_ref().err().or(votes.as_ref().err())
        && !is_fail_kind(ex, HTTP_REQUEST_EXCEPTION)
    {
        return Err(ex.clone());
    }
    fn first(doc: Option<&Value>) -> Option<&Value> {
        match doc?.get("data") {
            Some(Value::Array(data)) if !data.is_empty() => Some(&data[0]),
            _ => None,
        }
    }
    fn text(e: Option<&Value>, name: &str) -> Res<Option<String>> {
        let Some(o) = e else { return Ok(None) };
        Ok(match try_prop(o, name)? {
            Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        })
    }
    fn num(e: Option<&Value>, name: &str) -> Res<i64> {
        let Some(o) = e else { return Ok(0) };
        Ok(match try_prop(o, name)? {
            Some(v @ Value::Number(_)) => get_i64(v)?,
            _ => 0,
        })
    }
    fn when(e: Option<&Value>, name: &str) -> Res<Option<Stamp>> {
        Ok(text(e, name)?.and_then(|s| parse_assume_utc(&s)).filter(|d| d.utc().year > 2000))
    }
    // C#'s root.TryGetProperty("data") on a document that isn't an object throws.
    let object_or_fail = |doc: &Option<Value>| -> Res<()> {
        match doc {
            Some(d) => try_prop(d, "data").map(|_| ()),
            None => Ok(()),
        }
    };
    object_or_fail(&universe)?;
    let game = game.unwrap_or(None);
    let votes = votes.unwrap_or(None);
    object_or_fail(&game)?;
    object_or_fail(&votes)?;
    let u = first(universe.as_ref());
    let Some(name) = text(u, "name")? else { return Ok(None) };
    // Without a cookie games.roblox.com answers with id 0 and blanks for everything.
    let g = match first(game.as_ref()) {
        Some(gg) if num(Some(gg), "id")? == universe_id => Some(gg),
        _ => None,
    };
    let v = first(votes.as_ref());
    let (up, down) = (num(v, "upVotes")?, num(v, "downVotes")?);
    Ok(Some(GameDetailsInfo {
        name,
        creator: text(u, "creatorName")?.unwrap_or_default(),
        description: text(u, "description")?,
        genre: text(g, "genre")?,
        playing: num(g, "playing")?,
        visits: num(g, "visits")?,
        favorites: num(g, "favoritedCount")?,
        likes_percent: if up + down > 0 { Some((100.0 * up as f64 / (up + down) as f64).round_ties_even() as i32) } else { None },
        created: when(u, "created")?,
        updated: match when(g, "updated")? {
            Some(d) => Some(d),
            None => when(u, "updated")?,
        },
        root_place_id: num(u, "rootPlaceId")?,
        up_votes: up,
        down_votes: down,
        max_players: num(g, "maxPlayers")?,
    }))
}

/// Roblox's own game search, the same one the website's search box uses.
pub fn search_games(query: &str, ct: &Cancel) -> Res<Vec<GameSummary>> {
    let url = format!("https://apis.roblox.com/search-api/omni-search?searchQuery={}&sessionId={}&pageType=all", super::escape_data_string(query), new_guid());
    let doc = get_json(&url, ct, None)?;
    let mut result = Vec::new();
    let Some(doc) = doc else { return Ok(result) };
    let Some(Value::Array(groups)) = try_prop(&doc, "searchResults")? else { return Ok(result) };
    for group in groups {
        let is_game = match try_prop(group, "contentGroupType")? {
            Some(t) => get_str(t)?.as_deref() == Some("Game"),
            None => false,
        };
        if is_game && let Some(Value::Array(contents)) = try_prop(group, "contents")? {
            for game in contents {
                if let Some(summary) = read_game(game, "universeId", "rootPlaceId")? {
                    result.push(summary);
                }
            }
        }
    }
    Ok(result)
}

/// Games an account marked as favourite on Roblox (public list).
pub fn get_favorite_games(user_id: i64, ct: &Cancel) -> Res<Vec<GameSummary>> {
    let doc = get_json(&format!("https://games.roblox.com/v2/users/{user_id}/favorite/games?limit=25&sortOrder=Desc"), ct, None)?;
    let mut result = Vec::new();
    let Some(doc) = doc else { return Ok(result) };
    let Some(Value::Array(data)) = try_prop(&doc, "data")? else { return Ok(result) };
    for game in data {
        let place_id = match try_prop(game, "rootPlace")? {
            Some(root @ Value::Object(_)) => match try_prop(root, "id")? {
                Some(rid) => try_i64(rid)?.unwrap_or(0),
                None => 0,
            },
            _ => 0,
        };
        let universe_id = match try_prop(game, "id")? {
            Some(uid) => try_i64(uid)?.unwrap_or(0),
            None => 0,
        };
        let name = match try_prop(game, "name")? {
            Some(n) => get_str(n)?.unwrap_or_default(),
            None => String::new(),
        };
        if place_id > 0 && !name.is_empty() {
            result.push(GameSummary { place_id, universe_id, name, playing: 0 });
        }
    }
    Ok(result)
}

/// The first game list on Roblox's home/charts page. Roblox only fills it for signed-in requests.
pub fn get_popular_games(cookie: Option<&str>, ct: &Cancel) -> Res<Vec<GameSummary>> {
    let session = new_guid().to_string();
    let mut token = String::new();
    for _page in 0..4 {
        let url = format!(
            "https://apis.roblox.com/explore-api/v1/get-sorts?sessionId={session}&device=computer&country=all{}",
            if token.is_empty() { String::new() } else { format!("&sortsPageToken={}", super::escape_data_string(&token)) }
        );
        let Some(doc) = get_json(&url, ct, cookie)? else { break };
        let Some(Value::Array(sorts)) = try_prop(&doc, "sorts")? else { break };
        for sort in sorts {
            let Some(Value::Array(games)) = try_prop(sort, "games")? else { continue };
            if games.is_empty() {
                continue;
            }
            let mut result = Vec::new();
            for g in games {
                if let Some(summary) = read_game(g, "universeId", "rootPlaceId")? {
                    result.push(summary);
                }
            }
            if !result.is_empty() {
                return Ok(result);
            }
        }
        token = match try_prop(&doc, "nextSortsPageToken")? {
            Some(next) => get_str(next)?.unwrap_or_default(),
            None => String::new(),
        };
        if token.is_empty() {
            break;
        }
    }
    Ok(Vec::new())
}

fn read_game(game: &Value, universe_field: &str, place_field: &str) -> Res<Option<GameSummary>> {
    let num = |name: &str| -> Res<i64> {
        Ok(match try_prop(game, name)? {
            Some(v @ Value::Number(_)) => try_i64(v)?.unwrap_or(0),
            _ => 0,
        })
    };
    let name = match try_prop(game, "name")? {
        Some(Value::String(n)) => n.clone(),
        _ => String::new(),
    };
    let place_id = num(place_field)?;
    Ok(if place_id > 0 && !name.is_empty() { Some(GameSummary { place_id, universe_id: num(universe_field)?, name, playing: num("playerCount")? }) } else { None })
}

// Roblox allows one connection only a few server lists a minute ("x-ratelimit-limit: 3, 3;w=60", October 2026),
// whatever the game. What it said is kept so a request it would refuse isn't sent.
struct ServerListGate {
    left: i32,
    reset: Option<Stamp>,
}

static SERVER_LIST_GATE: Mutex<ServerListGate> = Mutex::new(ServerListGate { left: -1, reset: None });

/// How many more server lists Roblox takes before its minute is up (-1: not known yet).
pub fn server_list_left() -> i32 {
    let gate = lock(&SERVER_LIST_GATE);
    match gate.reset {
        Some(reset) if Stamp::utc_now() < reset => gate.left,
        _ => -1,
    }
}

fn rate_limited_for(wait: Duration) -> Fail {
    RobloxApiException {
        rate_limited: true,
        retry_after: Some(wait),
        status: 429,
        ..RobloxApiException::new(l::t("Roblox is rate-limiting requests; try again shortly."))
    }
    .into()
}

/// A page of a game's public servers, busiest or emptiest first, without full ones.
pub fn get_servers(place_id: i64, fewest_first: bool, cursor: Option<&str>, ct: &Cancel) -> Res<(Vec<ServerInfo>, Option<String>)> {
    {
        let gate = lock(&SERVER_LIST_GATE);
        if let Some(reset) = gate.reset {
            let wait = reset.secs_since(Stamp::utc_now());
            if gate.left == 0 && wait > 0.0 {
                return Err(rate_limited_for(Duration::from_secs_f64(wait)));
            }
        }
    }
    let url = format!(
        "https://games.roblox.com/v1/games/{place_id}/servers/Public?sortOrder={}&limit=50&excludeFullGames=true{}",
        if fewest_first { "Asc" } else { "Desc" },
        match cursor.filter(|c| !c.is_empty()) {
            Some(c) => format!("&cursor={}", super::escape_data_string(c)),
            None => String::new(),
        }
    );
    let req = request("GET", &url, None, false, realm::GLOBAL);
    let res = send(&req, ct)?;
    let left = first_number(&res, "x-ratelimit-remaining");
    let reset = first_number(&res, "x-ratelimit-reset").or_else(|| retry_after_secs(&res));
    if res.status == 429 {
        let wait = Duration::from_secs(reset.unwrap_or(20).clamp(1, 120) as u64);
        {
            let mut gate = lock(&SERVER_LIST_GATE);
            gate.left = 0;
            gate.reset = Some(Stamp::utc_now().plus_secs(wait.as_secs_f64()));
        }
        return Err(rate_limited_for(wait));
    }
    if let (Some(n), Some(r)) = (left, reset) {
        let mut gate = lock(&SERVER_LIST_GATE);
        gate.left = n;
        gate.reset = Some(Stamp::utc_now().plus_secs(r.clamp(0, 120) as f64));
    }
    // A failed answer is said, not read as "no servers": the server window kept that empty list a minute after a one-off
    // 500, and a preset told "no server has room" (review, 2026-10-30).
    if !res.is_success() {
        return Err(RobloxApiException { status: res.status, server_error: res.status >= 500, ..RobloxApiException::new(l::t("Couldn't load the server list. Try again in a moment.")) }
            .into());
    }
    let doc = parse_json(&res.text())?;
    let next = match try_prop(&doc, "nextPageCursor")? {
        Some(Value::String(c)) => Some(c.clone()),
        _ => None,
    };
    Ok((read_servers(&doc)?, next))
}

/// The page's servers without empty ones: a server whose last player left is closing, and joining it ends on
/// "This experience has ended…". Seen 2026-10-17: the emptiest-first list sometimes started with one that had no
/// player count but still a ping, the very server a party of 5 was sent to.
pub(crate) fn read_servers(root: &Value) -> Res<Vec<ServerInfo>> {
    let mut servers = Vec::new();
    let Some(Value::Array(data)) = try_prop(root, "data")? else { return Ok(servers) };
    for s in data {
        let int = |name: &str| -> Res<i32> {
            Ok(match try_prop(s, name)? {
                Some(v @ Value::Number(_)) => try_i32(v)?.unwrap_or(0),
                _ => 0,
            })
        };
        let num = |name: &str| -> Res<f64> {
            Ok(match try_prop(s, name)? {
                Some(v @ Value::Number(_)) => get_f64(v)?,
                _ => 0.0,
            })
        };
        if let Some(id) = try_prop(s, "id")?
            && let Some(job_id) = get_str(id)?.filter(|j| !j.is_empty())
            && int("playing")? > 0
        {
            servers.push(ServerInfo {
                job_id,
                playing: int("playing")?,
                max_players: int("maxPlayers")?,
                ping: num("ping")?.round_ties_even() as i32,
                fps: num("fps")?.round_ties_even(),
            });
        }
    }
    Ok(servers)
}

/// The first number of a rate-limit header ("2" of "2", "3" of "3, 3;w=60"), or None.
fn first_number(res: &HttpResponse, header: &str) -> Option<i32> {
    let v = res.headers_named(header).next()?;
    v.split(',').next().unwrap_or("").trim().parse::<i32>().ok()
}

/// Retry-After as a number of seconds (a date there gives none), as res.Headers.RetryAfter?.Delta.
fn retry_after_secs(res: &HttpResponse) -> Option<i32> {
    let v = res.headers_named("Retry-After").next()?.trim();
    v.parse::<i64>().ok().filter(|s| *s >= 0).map(|s| s.min(i32::MAX as i64) as i32)
}

/// Each game's first wide picture (the ones on its Roblox page, 768×432), for many games at once. WebP: a sixth of the
/// PNG's bytes (33 KB against 183 KB, measured 2026-11-13), and Roblox's picture servers are slow the first time a
/// picture is asked for (about 1.4 s a PNG from Vietnam).
pub fn get_game_covers(universe_ids: impl IntoIterator<Item = i64>, ct: &Cancel) -> Res<HashMap<i64, String>> {
    let mut result = HashMap::new();
    for chunk in chunks(universe_ids, 50) {
        let url = format!(
            "https://thumbnails.roblox.com/v1/games/multiget/thumbnails?universeIds={}&countPerUniverse=1&defaults=true&size=768x432&format=Webp&isCircular=false",
            joined(&chunk, ",")
        );
        let Some(doc) = get_json(&url, ct, None)? else { continue };
        let Some(Value::Array(data)) = try_prop(&doc, "data")? else { continue };
        for game in data {
            if let Some(Value::Array(shots)) = try_prop(game, "thumbnails")?
                && !shots.is_empty()
                && let Some(state) = try_prop(&shots[0], "state")?
                && get_str(state)?.as_deref() == Some("Completed")
                && let Some(url) = get_str(get_prop(&shots[0], "imageUrl")?)?
                && is_roblox_cdn(Some(&url))
            {
                result.insert(get_i64(get_prop(game, "universeId")?)?, url);
            }
        }
    }
    Ok(result)
}

/// Square game icons for many games at once.
pub fn get_game_icons(universe_ids: impl IntoIterator<Item = i64>, ct: &Cancel) -> Res<HashMap<i64, String>> {
    let mut result = HashMap::new();
    for chunk in chunks(universe_ids, 100) {
        let url = format!(
            "https://thumbnails.roblox.com/v1/games/icons?universeIds={}&returnPolicy=PlaceHolder&size=150x150&format=Webp&isCircular=false",
            joined(&chunk, ",")
        );
        let Some(doc) = get_json(&url, ct, None)? else { continue };
        let Some(Value::Array(data)) = try_prop(&doc, "data")? else { continue };
        for item in data {
            if let Some(url) = get_str(get_prop(item, "imageUrl")?)?
                && is_roblox_cdn(Some(&url))
            {
                result.insert(get_i64(get_prop(item, "targetId")?)?, url);
            }
        }
    }
    Ok(result)
}

/// Online / in-game status. With a cookie Roblox also reveals the game for that account and for anyone whose
/// privacy settings allow it.
pub fn get_presence(cookie: Option<&str>, user_ids: &[i64], realm: &str, ct: &Cancel) -> Res<HashMap<i64, Presence>> {
    let mut result = HashMap::new();
    // 100 ids per request (Roblox's limit); the requests go out together.
    let pages: Vec<Res<String>> = std::thread::scope(|s| {
        let handles: Vec<_> = chunks(user_ids.iter().copied(), 100)
            .into_iter()
            .map(|chunk| {
                s.spawn(move || -> Res<String> {
                    let json = json!({ "userIds": chunk }).to_string();
                    let res = post_with_csrf(&format!("{}/v1/presence/users", realm::api(realm, "presence")), cookie.unwrap_or(""), &json, None, ct, realm)?;
                    if res.status == 429 {
                        return Err(rate_limited());
                    }
                    // A failed page would read as "everyone offline / unknown"; fail the whole check so the last answer stays.
                    // 401 with a cookie: that sign-in is dead (flagged, so the caller can mark it and ask with another).
                    if !res.is_success() {
                        return Err(RobloxApiException {
                            status: res.status,
                            cookie_invalid: res.status == 401 && cookie.is_some_and(|c| !c.is_empty()),
                            ..RobloxApiException::new(l::f("Roblox refused the request (HTTP {0}). Try again in a moment.", &[&res.status]))
                        }
                        .into());
                    }
                    Ok(res.text())
                })
            })
            .collect();
        handles.into_iter().map(|h| joined_result(h.join())).collect()
    });
    // Task.WhenAll: the first failure, in the requests' order.
    let pages = pages.into_iter().collect::<Res<Vec<String>>>()?;
    for page in pages {
        let doc = parse_json(&page)?;
        for p in as_array(get_prop(&doc, "userPresences")?)? {
            let num = |name: &str| -> Res<Option<i64>> {
                Ok(match try_prop(p, name)? {
                    Some(v @ Value::Number(_)) => Some(get_i64(v)?),
                    _ => None,
                })
            };
            let text = |name: &str| -> Res<Option<String>> {
                Ok(match try_prop(p, name)? {
                    Some(Value::String(v)) => Some(v.clone()),
                    _ => None,
                })
            };
            let last_online = text("lastOnline")?.and_then(|s| Stamp::parse(&s));
            result.insert(
                get_i64(get_prop(p, "userId")?)?,
                Presence {
                    r#type: num("userPresenceType")?.unwrap_or(0) as i32,
                    location: text("lastLocation")?,
                    place_id: num("placeId")?,
                    root_place_id: num("rootPlaceId")?,
                    job_id: text("gameId")?,
                    last_online,
                    universe_id: num("universeId")?,
                },
            );
        }
    }
    Ok(result)
}
