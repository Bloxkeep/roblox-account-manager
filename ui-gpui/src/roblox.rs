// Roblox's own services, called from here (the core moves to Rust bit by bit, the owner's call on 2026-11-09): the
// charts (explore-api: the rows on roblox.com/charts and their games), the game search (search-api: the site's search
// box) and the games' icons (thumbnails). The charts' games come only to a signed-in account: the app hands one
// account's sign-in over its pipe (robloxSignIn), kept in memory only and sent to roblox.com alone.
//
// An account's friends (friends, presence, users, thumbnails) are asked with that account's own sign-in: Roblox
// refuses the list to a request that isn't signed in (403, checked 2026-11-10), and names a friend's server only to an
// account that friend lets join.
//
// Each call blocks, so it runs on a thread of its own and answers through a channel the window awaits.
use crate::threads::Hold;
use std::{collections::HashMap, io::Read, time::Duration};

use futures::channel::oneshot;
use serde_json::Value;
use crate::val::int;

/// A game in a row or a search: what the card shows.
#[derive(Clone, Debug)]
pub struct Game {
    pub universe: i64,
    pub place: i64,
    pub name: String,
    pub playing: i64,
    pub up: i64,
    pub down: i64,
}

/// A row of the charts (Top Trending, Up-and-Coming…), with what get-sort-content needs to fill it.
#[derive(Clone, Debug)]
pub struct Sort {
    pub id: String,
    pub name: String,
    target: Option<i64>,
    kind: Option<i64>,
    treatment: Option<String>,
}

/// The charts' rows when get-sorts names none (it names only the filters to a request that isn't signed in): the ones
/// roblox.com/charts shows, checked 2026-11-09.
const CHARTS: [(&str, &str); 5] = [
    ("top-trending", "Top Trending"),
    ("up-and-coming", "Up-and-Coming"),
    ("top-playing-now", "Top Playing Now"),
    ("fun-with-friends", "Fun with Friends"),
    ("top-revisited", "Top Revisited"),
];

/// One for every ask: its connections to Roblox stay open between them (a new one each time cost a quarter of every
/// ask, measured 2026-11-11: 0.33 s against 0.25 s).
fn agent() -> ureq::Agent {
    static AGENT: std::sync::OnceLock<ureq::Agent> = std::sync::OnceLock::new();
    AGENT.get_or_init(|| crate::net::builder().timeout(Duration::from_secs(20)).user_agent("Bloxkeep").max_idle_connections(64).max_idle_connections_per_host(12).build()).clone()
}

/// Two asks of Roblox at once. Each ask takes Roblox a quarter of a second whatever it is, so a page that asks ten
/// things one after another makes its reader wait ten times that; what doesn't need another's answer is asked together.
pub fn both<A: Send, B: Send>(a: impl FnOnce() -> A + Send, b: impl FnOnce() -> B + Send) -> (A, B) {
    crate::threads::scope(|s| {
        let a = s.spawn(a);
        let b = b();
        (a.join().expect("an ask of Roblox panicked"), b)
    })
}

/// The same ask for each of several things, all at once, answers in their order.
fn each<T: Send, R: Send>(things: Vec<T>, ask: impl Fn(T) -> R + Sync) -> Vec<R> {
    if things.len() < 2 {
        return things.into_iter().map(ask).collect();
    }
    crate::threads::scope(|s| {
        let ask = &ask;
        let going: Vec<_> = things.into_iter().map(|t| s.spawn(move || ask(t))).collect();
        going.into_iter().map(|g| g.join().expect("an ask of Roblox panicked")).collect()
    })
}

/// The Cookie header of a signed-in ask: the account's sign-in, and a browser tracker id as roblox.com's own pages
/// carry (the RBXEventTrackerV2 cookie's "browserid"). Roblox's settings calls refuse an ask without one: 400 "Request
/// Context BrowserTrackerID is missing or invalid" (the owner's error.log, 2026-11-15). One id for the whole run, as
/// one browser has one for all the accounts signed in to it.
pub(crate) fn signed(cookie: &str) -> String {
    static TRACKER: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    let id = TRACKER.get_or_init(|| {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
        100_000_000_000 + (now ^ ((std::process::id() as u64) << 20)) % 899_999_999_999
    });
    format!(".ROBLOSECURITY={cookie}; RBXEventTrackerV2=&browserid={id}")
}

fn get(url: &str, cookie: Option<&str>) -> Result<Value, String> {
    let out = get_once(url, cookie);
    // (One that got no answer is in error.log already, with its real error.)
    if let Err(why) = &out {
        if why != crate::net::UNREACHABLE {
            crate::net::note_failed("Roblox tab", url, why);
        }
    }
    out
}

fn get_once(url: &str, cookie: Option<&str>) -> Result<Value, String> {
    let mut req = agent().get(url).set("Accept", "application/json");
    if let Some(c) = cookie.filter(|c| !c.is_empty()) {
        req = req.set("Cookie", &signed(c));
    }
    match req.call() {
        Ok(r) => {
            let mut text = String::new();
            r.into_reader().take(8 << 20).read_to_string(&mut text).map_err(|e| e.to_string())?;
            serde_json::from_str(&text).map_err(|e| e.to_string())
        }
        Err(ureq::Error::Status(code, r)) => Err(refused(url, code, r)),
        Err(e) => Err(crate::net::unreachable("Roblox tab", url, &e.to_string())),
    }
}

/// A call Roblox refused: "Roblox {code}" for the page, and Roblox's own words (its errors[0].message, or the start of
/// the answer) in error.log, so a 400 says why without asking anyone for more.
fn refused(url: &str, code: u16, r: ureq::Response) -> String {
    let mut body = String::new();
    let _ = r.into_reader().take(4096).read_to_string(&mut body);
    let said = serde_json::from_str::<Value>(&body)
        .ok()
        .and_then(|v| v.pointer("/errors/0/message").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_else(|| body.chars().filter(|c| !c.is_control()).take(160).collect());
    crate::net::note_failed("Roblox said", url, &format!("{code}: {}", if said.trim().is_empty() { "no reason given" } else { said.trim() }));
    format!("Roblox {code}")
}

fn enc(s: &str) -> String {
    percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string()
}

fn session() -> String {
    // Any id will do: Roblox ties a page's calls together by it.
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("{:08x}-{:04x}-4{:03x}-a{:03x}-{:012x}", (n >> 32) as u32, (n >> 16) as u16, (n >> 4) as u16 & 0xfff, n as u16 & 0xfff, (n as u64) & 0xffff_ffff_ffff)
}

fn game_of(v: &Value) -> Option<Game> {
    let name = v.get("name").and_then(Value::as_str).unwrap_or_default().trim().to_string();
    let place = int(v, "rootPlaceId");
    (place > 0 && !name.is_empty()).then(|| Game {
        universe: int(v, "universeId"),
        place,
        name,
        playing: int(v, "playerCount"),
        up: int(v, "totalUpVotes"),
        down: int(v, "totalDownVotes"),
    })
}

/// The charts' rows, every page of them (as roblox.com asks: up to 25).
pub fn sorts(cookie: Option<&str>) -> Result<Vec<Sort>, String> {
    #[cfg(feature = "shot")]
    if let Some(f) = fixture() {
        return Ok(f.0);
    }
    let s = session();
    let mut out: Vec<Sort> = Vec::new();
    let mut token = String::new();
    for _ in 0..25 {
        let mut url = format!("https://apis.roblox.com/explore-api/v1/get-sorts?sessionId={s}&device=computer&country=all");
        if !token.is_empty() {
            url += &format!("&sortsPageToken={}", enc(&token));
        }
        let d = get(&url, cookie)?;
        for v in d.get("sorts").and_then(Value::as_array).into_iter().flatten() {
            let id = v.get("sortId").and_then(Value::as_str).unwrap_or_default();
            if v.get("contentType").and_then(Value::as_str) != Some("Games") || id.is_empty() || out.iter().any(|x| x.id == id) {
                continue;
            }
            out.push(Sort {
                id: id.into(),
                name: v.get("sortDisplayName").and_then(Value::as_str).unwrap_or(id).into(),
                target: v.get("gameSetTargetId").and_then(Value::as_i64),
                kind: v.get("gameSetTypeId").and_then(Value::as_i64),
                treatment: v.get("treatmentType").and_then(Value::as_str).map(str::to_string),
            });
        }
        token = d.get("nextSortsPageToken").and_then(Value::as_str).unwrap_or_default().to_string();
        if token.is_empty() {
            break;
        }
    }
    if out.is_empty() {
        out = CHARTS.iter().map(|(id, name)| Sort { id: (*id).into(), name: (*name).into(), target: None, kind: None, treatment: None }).collect();
    }
    Ok(out)
}

/// A row's games, all of them (the games get-sorts carries are only its first few).
pub fn sort_games(cookie: Option<&str>, sort: &Sort) -> Result<Vec<Game>, String> {
    #[cfg(feature = "shot")]
    if let Some(f) = fixture() {
        return Ok(f.1.get(&sort.id).cloned().unwrap_or_default());
    }
    let s = session();
    let mut url = format!(
        "https://apis.roblox.com/explore-api/v1/get-sort-content?sessionId={s}&discoverPageSessionInfo={s}&sortId={}&age=all&country=all&device=computer&page=chartsPage&position=1&cpuCores=8&maxResolution=1920x1080",
        enc(&sort.id)
    );
    if let Some(t) = sort.target {
        url += &format!("&gameSetTargetId={t}");
    }
    if let Some(k) = sort.kind {
        url += &format!("&gameSetTypeId={k}");
    }
    if let Some(t) = &sort.treatment {
        url += &format!("&treatmentType={}", enc(t));
    }
    // Roblox gives a chart a page at a time (nextPageToken, as its own Charts page loads more on scroll): up to four
    // pages, about a hundred games, so the chart runs on down the page (owner 2026-11-17: "hiện nhiều hơn").
    let mut games: Vec<Game> = Vec::new();
    let mut token: Option<String> = None;
    for _ in 0..4 {
        let page = match &token {
            Some(t) => format!("{url}&pageToken={}", enc(t)),
            None => url.clone(),
        };
        let d = match get(&page, cookie) {
            Ok(d) => d,
            // (A later page failing keeps what came.)
            Err(e) if games.is_empty() => return Err(e),
            Err(_) => break,
        };
        let before = games.len();
        for g in d.get("games").and_then(Value::as_array).into_iter().flatten().filter_map(game_of) {
            if !games.iter().any(|x| x.universe == g.universe) {
                games.push(g);
            }
        }
        token = d.get("nextPageToken").and_then(Value::as_str).filter(|t| !t.is_empty() && *t != "e30=").map(str::to_string);
        if token.is_none() || games.len() == before || games.len() >= 100 {
            break;
        }
    }
    Ok(games)
}

/// Roblox's game search, the one on roblox.com (no sign-in needed).
pub fn search(query: &str) -> Result<Vec<Game>, String> {
    let url = format!("https://apis.roblox.com/search-api/omni-search?searchQuery={}&sessionId={}&pageType=all", enc(query), session());
    let d = get(&url, None)?;
    Ok(d.get("searchResults")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|g| g.get("contentGroupType").and_then(Value::as_str) == Some("Game"))
        .flat_map(|g| g.get("contents").and_then(Value::as_array).cloned().unwrap_or_default())
        .filter_map(|v| game_of(&v))
        .collect())
}

#[derive(Default)]
pub struct HomeGames {
    pub continued: Vec<Game>,
    pub recommended: Vec<Game>,
}

fn home_universes(d: &Value, topic: i64, name: &str) -> Vec<i64> {
    let mut seen = std::collections::HashSet::new();
    d
        .get("sorts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|s| int(s, "topicId") == topic || text_of(s, "topic").eq_ignore_ascii_case(name))
        .flat_map(|s| s.get("recommendationList").and_then(Value::as_array).into_iter().flatten())
        .filter(|i| i.get("contentType").and_then(Value::as_str) == Some("Game"))
        .filter_map(|i| i.get("contentId").and_then(Value::as_i64))
        .filter(|u| *u > 0 && seen.insert(*u))
        .filter(|u| d["contentMetadata"]["Game"][u.to_string()]["isSponsored"].as_bool() != Some(true))
        .collect()
}

/// The account's two personalized Home rows in the order Roblox returns them.
pub fn home_games(cookie: &str) -> Result<HomeGames, String> {
    let d = post("https://apis.roblox.com/discovery-api/omni-recommendation", &serde_json::json!({ "pageType": "Home", "sessionId": session() }), Some(cookie))?;
    let continued = home_universes(&d, 100_000_003, "Continue");
    let mut recommended = home_universes(&d, 100_000_000, "Recommended for you");
    recommended.truncate(72);
    let mut ids: Vec<_> = continued.iter().chain(&recommended).copied().collect();
    ids.sort_unstable();
    ids.dedup();
    let mut by: HashMap<i64, Game> = ids.iter().filter_map(|&id| {
        let game = game_of(&d["contentMetadata"]["Game"][id.to_string()])?;
        (game.universe == id).then_some((id, game))
    }).collect();
    let missing: Vec<_> = ids.into_iter().filter(|id| !by.contains_key(id)).collect();
    for chunk in missing.chunks(50) {
        let ids: Vec<String> = chunk.iter().map(i64::to_string).collect();
        let g = get(&format!("https://games.roblox.com/v1/games?universeIds={}", ids.join(",")), None)?;
        for v in g.get("data").and_then(Value::as_array).into_iter().flatten() {
            let (universe, place, name) = (int(v, "id"), int(v, "rootPlaceId"), text_of(v, "name"));
            if place > 0 && !name.is_empty() {
                by.insert(universe, Game { universe, place, name, playing: int(v, "playing"), up: 0, down: 0 });
            }
        }
    }
    let collect = |ids: Vec<i64>| ids.iter().filter_map(|u| by.get(u).cloned()).collect();
    Ok(HomeGames { continued: collect(continued), recommended: collect(recommended) })
}

fn text_of(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or_default().trim().to_string()
}

/// The games' square icons by universe, 100 a call. WebP, as the covers (a sixth of a PNG's bytes).
pub fn icons(universes: &[i64]) -> HashMap<i64, String> {
    let (mut out, missing) = pics::known("icons-webp", universes);
    let mut learnt = HashMap::new();
    for chunk in missing.chunks(100) {
        let ids: Vec<String> = chunk.iter().map(i64::to_string).collect();
        let url = format!("https://thumbnails.roblox.com/v1/games/icons?universeIds={}&returnPolicy=PlaceHolder&size=150x150&format=Webp&isCircular=false", ids.join(","));
        let Ok(d) = get(&url, None) else { continue };
        for v in d.get("data").and_then(Value::as_array).into_iter().flatten() {
            if let Some(u) = v.get("imageUrl").and_then(Value::as_str).filter(|u| !u.is_empty()) {
                learnt.insert(int(v, "targetId"), u.to_string());
            }
        }
    }
    pics::learn("icons-webp", &learnt);
    out.extend(learnt);
    out
}

/// A game's pictures that Roblox shows: those its moderation passed ("Completed"). One it blocked comes as Roblox's
/// grey "unapproved" stand-in on a white ground (`…/UnapprovedImage/…`): Ryujin's first and last of six, the owner's
/// game page, 2026-11-19 ("ảnh game này lỗi à"). Pending and failed ones are left out with them.
fn shown_thumbnails(v: &Value) -> impl Iterator<Item = &Value> {
    v.get("thumbnails").and_then(Value::as_array).into_iter().flatten().filter(|t| {
        t.get("state").and_then(Value::as_str) == Some("Completed") && t.get("imageUrl").and_then(Value::as_str).is_some_and(|u| !u.is_empty() && !u.contains("/UnapprovedImage/"))
    })
}

/// The games' wide pictures (16:9) by universe, in one of Roblox's sizes ("480x270", "768x432"), 50 a call. They need
/// no sign-in (checked 2026-11-11). WebP: 33 KB a 480×270 against the PNG's 183 KB, and Roblox's picture servers take
/// about 1.4 s for a PNG nobody near has asked for lately (measured from Vietnam, 2026-11-13); 0.8 s for the WebP.
pub fn covers(universes: &[i64], size: &str) -> HashMap<i64, String> {
    // (Kept by a name of its own since blocked pictures are left out: the ones learnt before may be the stand-in.)
    let kind = format!("covers-shown-{size}");
    let (mut out, missing) = pics::known(&kind, universes);
    let mut learnt = HashMap::new();
    for chunk in missing.chunks(50) {
        let ids: Vec<String> = chunk.iter().map(i64::to_string).collect();
        let url = format!("https://thumbnails.roblox.com/v1/games/multiget/thumbnails?universeIds={}&countPerUniverse=3&defaults=true&size={size}&format=Webp&isCircular=false", ids.join(","));
        let Ok(d) = get(&url, None) else { continue };
        for v in d.get("data").and_then(Value::as_array).into_iter().flatten() {
            let pic = shown_thumbnails(v).next().and_then(|t| t.get("imageUrl")).and_then(Value::as_str);
            if let Some(u) = pic {
                learnt.insert(int(v, "universeId"), u.to_string());
            }
        }
    }
    pics::learn(&kind, &learnt);
    out.extend(learnt);
    out
}

/// A game's own pictures (the ones its Roblox page turns through), wide, each once. No sign-in needed.
pub fn shots(universe: i64) -> Vec<String> {
    shots_with_assets(universe).into_iter().map(|(u, _)| u).collect()
}

/// `shots`, each with the id of the picture as it was uploaded (0 when Roblox names none): what `uploaded_picture`
/// fetches at its own size.
fn shots_with_assets(universe: i64) -> Vec<(String, i64)> {
    let url = format!("https://thumbnails.roblox.com/v1/games/multiget/thumbnails?universeIds={universe}&countPerUniverse=10&defaults=true&size=768x432&format=Webp&isCircular=false");
    let Ok(d) = get(&url, None) else { return Vec::new() };
    let mut out: Vec<(String, i64)> = Vec::new();
    for t in d.pointer("/data/0").into_iter().flat_map(shown_thumbnails) {
        let pic = t.get("imageUrl").and_then(Value::as_str).unwrap_or_default();
        if !pic.is_empty() && !out.iter().any(|(u, _)| u == pic) {
            out.push((pic.to_string(), int(t, "targetId").max(0)));
        }
    }
    out
}

/// The games' first wide pictures (768×432) by universe, each with the id of the picture as uploaded (0: none named).
fn covers_with_assets(universes: &[i64]) -> HashMap<i64, (String, i64)> {
    let mut out = HashMap::new();
    for chunk in universes.chunks(50) {
        let ids: Vec<String> = chunk.iter().map(i64::to_string).collect();
        let url = format!("https://thumbnails.roblox.com/v1/games/multiget/thumbnails?universeIds={}&countPerUniverse=3&defaults=true&size=768x432&format=Webp&isCircular=false", ids.join(","));
        let Ok(d) = get(&url, None) else { continue };
        for v in d.get("data").and_then(Value::as_array).into_iter().flatten() {
            let Some(t) = shown_thumbnails(v).next() else { continue };
            let pic = t.get("imageUrl").and_then(Value::as_str).unwrap_or_default();
            if !pic.is_empty() {
                out.insert(int(v, "universeId"), (pic.to_string(), int(t, "targetId").max(0)));
            }
        }
    }
    out
}

/// A game's gameplay clip, when its page has one: the asset of the first "GamePreviewVideo" among its media
/// (games.roblox.com/v1/games/{universe}/media, read 2026-11-13: type 86, the asset's id in `imageId`; Adopt Me has one,
/// most games none).
pub fn preview_video(universe: i64) -> Option<i64> {
    let d = get(&format!("https://games.roblox.com/v1/games/{universe}/media"), None).ok()?;
    d.get("data")?
        .as_array()?
        .iter()
        .find(|m| m.get("assetTypeId").and_then(Value::as_i64) == Some(86) && m.get("approved").and_then(Value::as_bool) != Some(false))?
        .get("imageId")?
        .as_i64()
        .filter(|id| *id > 0)
}

/// The gameplay clip of the game at `place`, its file on this PC (see `video_file`), when it has one.
pub fn place_clip(place: i64, dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let universe = int(&get(&format!("https://apis.roblox.com/universes/v1/places/{place}/universe"), None).ok()?, "universeId");
    if universe <= 0 {
        return None;
    }
    video_file(preview_video(universe)?, dir)
}

/// The clip's file on this PC, fetched the first time, without a sign-in. Roblox's own page plays a clip as HLS, its
/// best picture 1280×720: that one is kept, its parts as `{asset}-N.webm` and the file handed back `{asset}.parts`,
/// their names a line each (video.rs plays them in turn). The plain file its asset delivery names (`{asset}.webm`) is
/// only 640×360 (measured 2026-11-18 on three kept clips: VP9 at 640×360), soft on the Games tab's big tile; it is
/// what plays when Roblox gives no HLS for the clip. The folder keeps the newest clips up to CLIPS_MAX.
pub fn video_file(asset: i64, dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let file = dir.join(format!("{asset}.webm"));
    let list = dir.join(format!("{asset}.parts"));
    if let Ok(names) = std::fs::read_to_string(&list)
        && names.lines().all(|n| dir.join(n).metadata().is_ok_and(|m| m.len() > 0))
    {
        return Some(list);
    }
    std::fs::create_dir_all(dir).ok()?;
    if let Some(kept) = hls_clip(asset, dir, &list) {
        // (The small one kept by an earlier version goes.)
        let _ = std::fs::remove_file(&file);
        sweep_clips(dir, &kept);
        return Some(kept);
    }
    if file.metadata().is_ok_and(|m| m.len() > 0) {
        return Some(file);
    }
    let d = get(&format!("https://assetdelivery.roblox.com/v2/assetId/{asset}"), None).ok()?;
    let at = d.pointer("/locations/0/location")?.as_str()?;
    if !at.starts_with("https://") {
        return None;
    }
    let bytes = fetch(at, 64 << 20)?;
    std::fs::create_dir_all(dir).ok()?;
    let kept = if is_webm(&bytes) {
        keep(dir, &file, &bytes)?;
        file
    } else if bytes.starts_with(b"#EXTM3U") {
        hls_clip(asset, dir, &list)?
    } else {
        // (An error page isn't kept.)
        return None;
    };
    sweep_clips(dir, &kept);
    Some(kept)
}

/// A WebM starts with Matroska's mark.
fn is_webm(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x1a, 0x45, 0xdf, 0xa3])
}

fn fetch(url: &str, most: u64) -> Option<Vec<u8>> {
    fetch_as("Game clip", url, most)
}

/// A file from Roblox's servers, `most` bytes of it at most; a failure is in error.log under `what`.
fn fetch_as(what: &str, url: &str, most: u64) -> Option<Vec<u8>> {
    let r = match agent().get(url).timeout(Duration::from_secs(180)).call() {
        Ok(r) => r,
        Err(e) => {
            crate::net::note_failed(what, url, &e.to_string());
            return None;
        }
    };
    let mut bytes = Vec::new();
    if let Err(e) = r.into_reader().take(most).read_to_end(&mut bytes) {
        crate::net::note_failed(what, url, &e.to_string());
        return None;
    }
    Some(bytes)
}

/// The banner of the game at `place` as it was uploaded, its file on this PC (`dir`/{asset}.img), fetched the first
/// time. Roblox's picture servers give a game's wide picture at 768×432 at most ("The requested size is invalid" to
/// any more, asked 2026-11-18); asset delivery gives the picture itself without a sign-in, 1024×576 (three games
/// asked that day): a third more dots each way, for Home's hero, where the 768 one was blown up and soft (the owner:
/// "banner phải nét, ko mờ").
pub fn place_banner(place: i64, dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let universe = int(&get(&format!("https://apis.roblox.com/universes/v1/places/{place}/universe"), None).ok()?, "universeId");
    if universe <= 0 {
        return None;
    }
    let d = get(&format!("https://thumbnails.roblox.com/v1/games/multiget/thumbnails?universeIds={universe}&countPerUniverse=3&defaults=true&size=768x432&format=Webp&isCircular=false"), None).ok()?;
    let asset = shown_thumbnails(d.pointer("/data/0")?).next()?.get("targetId").and_then(Value::as_i64).filter(|id| *id > 0)?;
    uploaded_picture(asset, dir)
}

/// A game picture as it was uploaded (its asset id, from the picture servers' `targetId`), its file on this PC
/// (`dir`/{asset}.img), fetched the first time from asset delivery, which needs no sign-in: 1024×576 where the picture
/// servers stop at 768×432. Kept with the others in `dir` (Home's hero and the profiles' banners), the oldest going
/// past 150 MB (a picture is about 1 MB).
pub fn uploaded_picture(asset: i64, dir: &std::path::Path) -> Option<std::path::PathBuf> {
    if asset <= 0 {
        return None;
    }
    let file = dir.join(format!("{asset}.img"));
    if file.metadata().is_ok_and(|m| m.len() > 0) {
        return Some(file);
    }
    let d = get(&format!("https://assetdelivery.roblox.com/v2/assetId/{asset}"), None).ok()?;
    let at = d.pointer("/locations/0/location")?.as_str()?;
    if !at.starts_with("https://") {
        return None;
    }
    let bytes = fetch_as("Game banner", at, 16 << 20)?;
    if !crate::images::is_picture(&bytes) {
        crate::net::note_failed("Game banner", "https://assetdelivery.roblox.com/v2/assetId", &format!("what came isn't a picture ({} bytes)", bytes.len()));
        return None;
    }
    std::fs::create_dir_all(dir).ok()?;
    keep(dir, &file, &bytes)?;
    sweep_kept(dir, &file, 150 << 20);
    Some(file)
}

/// An emote's motion file (its KeyframeSequence, a binary Roblox file, rbxm.rs): the emote's own asset holds an
/// Animation whose AnimationId names it. Asset delivery gives both without a sign-in (measured 2026-11-20: the Salute
/// emote, 3360689775, and its motion, 10714389988: 89 keyframes of the R15 joints).
pub fn emote_motion(emote: i64) -> Result<Vec<u8>, String> {
    let one = |asset: i64| -> Result<Vec<u8>, String> {
        let d = get(&format!("https://assetdelivery.roblox.com/v2/assetId/{asset}"), None)?;
        let at = d.pointer("/locations/0/location").and_then(|v| v.as_str()).filter(|a| a.starts_with("https://")).ok_or("Roblox gave no file")?;
        fetch_as("Emote", at, 8 << 20).ok_or_else(|| "the file didn't come".to_string())
    };
    let first = one(emote)?;
    let doc = crate::rbxm::read(&first)?;
    if !doc.all("KeyframeSequence").is_empty() {
        return Ok(first);
    }
    let motion = doc.all("Animation").into_iter().find_map(|a| doc.text(a, "AnimationId").and_then(crate::rbxm::asset_id)).ok_or("no animation in it")?;
    one(motion)
}

/// Written whole or not at all.
fn keep(dir: &std::path::Path, file: &std::path::Path, bytes: &[u8]) -> Option<()> {
    // A temp name of its own: two fetches of the same clip at once (Home and Games, a hover again) wrote one temp file,
    // and the loser's rename failed with the clip there all along.
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let part = dir.join(format!("{}.{}-{n}.part", file.file_name()?.to_string_lossy(), std::process::id()));
    std::fs::write(&part, bytes).ok()?;
    if std::fs::rename(&part, file).is_ok() {
        return Some(());
    }
    let _ = std::fs::remove_file(&part);
    file.is_file().then_some(())
}

/// An HLS clip's parts, fetched as roblox.com's own game page does (seen 2026-11-13): asking asset delivery for the
/// "hls" representation gives a signed link on sc*.rbxcdn.com, and every playlist and part under it takes that same
/// signature. The plain `/v2/assetId` link (hls-segments.rbxcdn.com, unsigned) answers 403 to everyone. Each part is a
/// whole WebM of about 6 s; the rendition kept is the tallest Roblox lists (1280×720 today; the owner, 2026-11-18: the highest there is).
fn hls_clip(asset: i64, dir: &std::path::Path, list: &std::path::Path) -> Option<std::path::PathBuf> {
    // [{"format":"hls","majorVersion":"1","fidelity":"main"}], as the page sends it.
    const HLS: &str = "W3siZm9ybWF0IjoiaGxzIiwibWFqb3JWZXJzaW9uIjoiMSIsImZpZGVsaXR5IjoibWFpbiJ9XQ%3D%3D";
    let d = get(&format!("https://assetdelivery.roblox.com/v2/asset?Id={asset}&ContentRepresentationPriorityList={HLS}"), None).ok()?;
    let at = d.pointer("/locations/0/location")?.as_str()?;
    let (head, sign) = at.split_once('?')?;
    let base = head.rsplit_once('/')?.0;
    if !base.starts_with("https://") {
        return None;
    }
    let text = |url: &str| fetch(url, 1 << 20).and_then(|b| String::from_utf8(b).ok());
    let master = text(at)?;
    // #EXT-X-STREAM-INF:…,RESOLUTION=1280x720,… then its playlist on the next line.
    let mut best: Option<(u32, &str)> = None;
    let mut lines = master.lines();
    while let Some(line) = lines.next() {
        let Some(info) = line.strip_prefix("#EXT-X-STREAM-INF:") else { continue };
        let Some(uri) = lines.next() else { break };
        let tall = info
            .split(',')
            .find_map(|p| p.strip_prefix("RESOLUTION="))
            .and_then(|r| r.split_once('x'))
            .and_then(|(_, h)| h.parse::<u32>().ok())
            .unwrap_or(0);
        if best.is_none_or(|(b, _)| tall > b) {
            best = Some((tall, uri));
        }
    }
    let rendition = best?.1.split('?').next()?;
    let folder = rendition.rsplit_once('/').map_or("", |(f, _)| f);
    let playlist = text(&format!("{base}/{rendition}?{sign}"))?;
    let parts: Vec<&str> = playlist.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).filter_map(|l| l.split('?').next()).collect();
    if parts.is_empty() || parts.len() > 64 {
        return None;
    }
    let mut names = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        let url = if folder.is_empty() { format!("{base}/{part}?{sign}") } else { format!("{base}/{folder}/{part}?{sign}") };
        let bytes = fetch(&url, 32 << 20)?;
        if !is_webm(&bytes) {
            return None;
        }
        let name = format!("{asset}-{i}.webm");
        keep(dir, &dir.join(&name), &bytes)?;
        names.push(name);
    }
    keep(dir, list, names.join("\n").as_bytes())?;
    Some(list.to_path_buf())
}

/// The oldest clips go once the folder is past its size (never the one just fetched, nor its parts).
fn sweep_clips(dir: &std::path::Path, fresh: &std::path::Path) {
    sweep_kept(dir, fresh, 300 << 20)
}

/// The oldest files of a folder of kept ones go once it is past `most` bytes (never `fresh`, nor its parts).
fn sweep_kept(dir: &std::path::Path, fresh: &std::path::Path, most: u64) {
    let stem = fresh.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let mine = |p: &std::path::Path| p == fresh || p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(&format!("{stem}-")));
    let mut kept: Vec<(std::time::SystemTime, u64, std::path::PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            Some((m.modified().ok()?, m.len(), e.path()))
        })
        .collect();
    kept.sort_unstable_by(|a, b| b.0.cmp(&a.0));
    let mut total = 0u64;
    for (_, len, path) in kept {
        total += len;
        if total > most && !mine(&path) {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// The avatar as a thing with depth: Roblox's own model of it (what its site turns with three.js), for mesh.rs to draw.
pub struct Solid {
    pub obj: String,
    pub mtl: String,
    /// The MTL's pictures, by the names it gives them.
    pub pictures: HashMap<String, Vec<u8>>,
    /// Where Roblox's camera stands, and the model's box.
    pub eye: [f32; 3],
    pub min: [f32; 3],
    pub max: [f32; 3],
    /// Roblox's address of this model: another look has another.
    pub file: String,
}

/// A file of Roblox's picture servers, by its hash. Which of the eight servers holds it is reckoned from the hash, as
/// Roblox's own page does (DevForum, "Using Roblox's avatar-3d API…", read 2026-11-11); should that reckoning have
/// changed, the others are tried.
fn cdn(hash: &str) -> Option<Vec<u8>> {
    if hash.is_empty() {
        return None;
    }
    // (A file named by its hash is the same file for ever: kept in the cache folder, the next look at the avatar reads
    // it here instead of from Roblox. Only a plain hash names a file.)
    // (Roblox's names may carry dashes and underscores ("30DAY-…"): with letters and digits only, nothing was ever kept,
    // the owner's cache had no avatar-3d folder, 2026-11-17.)
    let kept = KEEP
        .get()
        .cloned()
        .flatten()
        .filter(|_| hash.len() <= 96 && hash.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .map(|d| d.join("avatar-3d").join(hash));
    if let Some(bytes) = kept.as_ref().and_then(|p| std::fs::read(p).ok()).filter(|b| !b.is_empty()) {
        return Some(bytes);
    }
    let got = cdn_fetch(hash);
    if let (Some(p), Some(b)) = (kept, got.as_ref()) {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&p, b);
    }
    got
}

fn cdn_fetch(hash: &str) -> Option<Vec<u8>> {
    let first = hash.bytes().take(38).fold(31u32, |i, b| i ^ b as u32) % 8;
    let found = std::iter::once(first).chain((0..8).filter(|n| *n != first)).find_map(|n| {
        let r = agent().get(&format!("https://t{n}.rbxcdn.com/{hash}")).call().ok()?;
        let mut out = Vec::new();
        r.into_reader().take(32 << 20).read_to_end(&mut out).ok()?;
        (!out.is_empty()).then_some(out)
    });
    if found.is_none() {
        crate::net::note_failed("Avatar 3D", "https://t?.rbxcdn.com/[file]", "none of Roblox's 8 servers gave it");
    }
    found
}

/// A user's avatar as a model. Roblox answers only a signed-in request (403 to any other, checked 2026-11-11), so the
/// answer's shape is the DevForum's, not one seen here with a real sign-in: { state, imageUrl } and, at that address,
/// { camera: { position }, aabb: { min, max }, obj, mtl, textures }. A model Roblox is still making (after a change)
/// is waited for: with `after` (the address of the model before a change) up to 45 s for another than that one, as
/// Roblox answers Pending while it makes the new one and, for a moment, still the one before (one took 10.7 s on the
/// owner's PC, 2026-11-17); without it 12 s.
pub fn solid(cookie: &str, user: i64, after: Option<&str>) -> Result<Solid, String> {
    // BLOXKEEP_SHOT_MODEL=dir: a hidden shot's model, from model.obj, model.mtl and the pictures beside them (every
    // other file there, by its name: Roblox's own have no ending).
    #[cfg(feature = "shot")]
    if let Ok(dir) = std::env::var("BLOXKEEP_SHOT_MODEL") {
        let dir = std::path::Path::new(&dir);
        let read = |n: &str| std::fs::read_to_string(dir.join(n)).map_err(|e| e.to_string());
        let mut pictures = HashMap::new();
        for f in std::fs::read_dir(dir).map_err(|e| e.to_string())?.flatten() {
            let name = f.file_name().to_string_lossy().to_string();
            if name != "model.obj" && name != "model.mtl" {
                pictures.insert(name, std::fs::read(f.path()).map_err(|e| e.to_string())?);
            }
        }
        return Ok(Solid { obj: read("model.obj")?, mtl: read("model.mtl")?, pictures, eye: [0.; 3], min: [0.; 3], max: [0.; 3], file: String::new() });
    }
    let began = std::time::Instant::now();
    let patience = Duration::from_secs(if after.is_some() { 45 } else { 12 });
    let asked = "https://thumbnails.roblox.com/v1/users/avatar-3d";
    let (mut file, mut before) = (String::new(), String::new());
    let state = loop {
        let d = get(&format!("{asked}?userId={user}"), Some(cookie))?;
        let state = text(&d, "state");
        if state == "Completed" {
            let url = text(&d, "imageUrl");
            if after != Some(url.as_str()) {
                file = url;
                break state;
            }
            before = url;
        } else if state != "Pending" {
            // (Error, Blocked, InReview…: Roblox makes none.)
            break state;
        }
        if began.elapsed() >= patience {
            break state;
        }
        std::thread::sleep(Duration::from_millis(1000));
    };
    // (No other model came: the change made none, and the one before stands.)
    if file.is_empty() {
        file = before;
    }
    if file.is_empty() {
        crate::net::note_failed("Avatar 3D", asked, &format!("Roblox gave no model in {:.0} s; its last answer: {}", began.elapsed().as_secs_f32(), if state.is_empty() { "none" } else { &state }));
        return Err(String::new());
    }
    let d = get(&file, None)?;
    let xyz = |path: &str| -> [f32; 3] {
        let at = |k: &str| d.pointer(&format!("{path}/{k}")).and_then(Value::as_f64).unwrap_or(0.) as f32;
        [at("x"), at("y"), at("z")]
    };
    let (obj, mtl) = both(|| cdn(&text(&d, "obj")), || cdn(&text(&d, "mtl")));
    // The newest model's file names, beside the kept files: a look that went wrong (R15 drawn apart from R6, owner
    // 2026-11-17) can be read back from the very files Roblox gave.
    if let Some(dir) = KEEP.get().cloned().flatten() {
        let _ = std::fs::write(dir.join("avatar-3d").join("last.json"), d.to_string());
    }
    let obj = String::from_utf8_lossy(&obj.ok_or("obj")?).to_string();
    let mtl = String::from_utf8_lossy(&mtl.ok_or("mtl")?).to_string();
    // The pictures the materials name (each name is the picture's hash).
    let mut names: Vec<_> = mtl.lines().filter_map(|line| {
        let mut words = line.split_whitespace();
        if !matches!(words.next(), Some("map_Kd" | "map_d")) {
            return None;
        }
        words.last().and_then(|path| path.rsplit(['/', '\\']).next()).map(str::to_string)
    }).collect();
    names.sort();
    names.dedup();
    let pictures: HashMap<String, Vec<u8>> = each(names, |n| cdn(&n).map(|b| (n, b))).into_iter().flatten().collect();
    // The shape can load even when Roblox failed to serve the clothing or face atlas.
    if mtl.lines().filter_map(|line| {
        let mut words = line.split_whitespace();
        if words.next()? != "map_Kd" { return None; }
        words.last()?.rsplit(['/', '\\']).next()
    }).any(|name| !pictures.contains_key(name)) {
        return Err("Avatar texture unavailable".into());
    }
    // (How long it took, when it was slow: error.log says where the time went without asking anyone.)
    let took = began.elapsed().as_secs_f32();
    if took > 4. {
        crate::net::note_worked(&format!("Avatar 3D: slow ({took:.1} s; {} pictures).", pictures.len()));
    }
    Ok(Solid { obj, mtl, pictures, eye: xyz("/camera/position"), min: xyz("/aabb/min"), max: xyz("/aabb/max"), file })
}

// ───────────────────────── moved here from the C# app ─────────────────────────
// Reads of Roblox the C# app made for the interface (its gameLooks and searchGames commands), now made here: they
// need nothing the app holds (no vault, no window), only Roblox's public answers.

/// Where what Roblox said may be kept between runs: the app's cache folder (--cache, from GpuiHost), none in a hidden
/// shot or the mock.
static KEEP: std::sync::OnceLock<Option<std::path::PathBuf>> = std::sync::OnceLock::new();

pub fn keep_in(dir: Option<std::path::PathBuf>) {
    let _ = KEEP.set(dir);
}

/// A game's pictures, and when Roblox was asked for them (seconds since 1970).
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
struct GameLook {
    icon: String,
    cover: String,
    at: u64,
}

/// A week, as the C# app kept them (WikiLook.cs): the Games tab opens with its pictures at once.
const LOOKS_KEPT: u64 = 7 * 24 * 3600;

fn looks_book() -> &'static std::sync::Mutex<HashMap<i64, GameLook>> {
    static BOOK: std::sync::OnceLock<std::sync::Mutex<HashMap<i64, GameLook>>> = std::sync::OnceLock::new();
    BOOK.get_or_init(|| {
        // What was kept last time, when there is a place to keep it and it reads.
        let kept = KEEP.get().cloned().flatten().and_then(|d| std::fs::read_to_string(d.join("ui-game-looks.json")).ok()).and_then(|t| serde_json::from_str::<HashMap<i64, GameLook>>(&t).ok());
        std::sync::Mutex::new(kept.unwrap_or_default())
    })
}

fn now_secs() -> u64 {
    crate::time::now_secs().max(0) as u64
}

/// How each game is drawn, by its place: its square icon and its wide picture (the C# app's gameLooks, without the
/// colour it took from the icon, which this interface doesn't use). Kept a week, in the app's cache folder when there
/// is one; a game Roblox says nothing of is left out, and asked for again the next time.
pub fn looks(places: &[i64]) -> HashMap<i64, Value> {
    let now = now_secs();
    let missing: Vec<i64> = {
        let book = looks_book().hold();
        places.iter().copied().filter(|p| *p > 0 && !book.get(p).is_some_and(|l| now.saturating_sub(l.at) < LOOKS_KEPT)).collect()
    };
    if !missing.is_empty() {
        // Each place's universe (a dozen asks at a time), then the pictures of them all in two asks.
        let mut pairs: Vec<(i64, i64)> = Vec::new();
        for chunk in missing.chunks(12) {
            pairs.extend(each(chunk.to_vec(), |p| (p, get(&format!("https://apis.roblox.com/universes/v1/places/{p}/universe"), None).map(|d| int(&d, "universeId")).unwrap_or(0))));
        }
        let ids: Vec<i64> = pairs.iter().map(|x| x.1).filter(|u| *u > 0).collect();
        let (small, wide) = both(|| icons(&ids), || covers(&ids, "768x432"));
        let mut book = looks_book().hold();
        for (place, universe) in pairs {
            let (icon, cover) = (small.get(&universe).cloned().unwrap_or_default(), wide.get(&universe).cloned().unwrap_or_default());
            if !icon.is_empty() || !cover.is_empty() {
                book.insert(place, GameLook { icon, cover, at: now });
            }
        }
        // Out of date ones go, so the book doesn't grow with every game ever seen.
        book.retain(|_, l| now.saturating_sub(l.at) < LOOKS_KEPT);
        // Kept for the next run (through a file of its own beside the C# app's, written whole then put in place).
        if let Some(dir) = KEEP.get().cloned().flatten() {
            if let Ok(text) = serde_json::to_string(&*book) {
                let _ = std::fs::create_dir_all(&dir);
                let temp = dir.join("ui-game-looks.json.tmp");
                let r = std::fs::write(&temp, text).and_then(|_| std::fs::rename(&temp, dir.join("ui-game-looks.json")));
                crate::net::wrote("Game pictures list", &dir.join("ui-game-looks.json"), r);
            }
        }
    }
    let book = looks_book().hold();
    places.iter().filter_map(|p| book.get(p).map(|l| (*p, serde_json::json!({ "icon": l.icon, "cover": l.cover })))).collect()
}

/// What a game box holds, looked up (the C# app's resolveGame): refused with why ("errorKey", words.json), or a share
/// link, or the place with its name, maker, players now and icon as far as Roblox gives them. Roblox hides titles and
/// player counts from a request that isn't signed in, so names come from develop.roblox.com and the count needs a
/// sign-in (none: no count).
pub fn resolve(input: &str, cookie: Option<&str>) -> Value {
    let t = match crate::target::parse(input, "") {
        Ok(t) => t,
        Err(why) => return serde_json::json!({ "ok": false, "errorKey": why.key() }),
    };
    if t.share.is_some() {
        return serde_json::json!({ "ok": true, "kind": "share" });
    }
    let kind = if t.is_private() { "private" } else if t.job.is_some() { "server" } else { "game" };
    let universe = get(&format!("https://apis.roblox.com/universes/v1/places/{}/universe", t.place), None).map(|d| int(&d, "universeId")).unwrap_or(0);
    let mut out = serde_json::json!({ "ok": true, "kind": kind, "placeId": t.place, "name": null, "creator": null, "playing": null, "iconUrl": null });
    if universe > 0 {
        let (named, (count, pics)) = both(
            || get(&format!("https://develop.roblox.com/v1/universes/multiget?ids={universe}"), None),
            || both(|| get(&format!("https://games.roblox.com/v1/games?universeIds={universe}"), cookie), || icons(&[universe])),
        );
        let first = |d: Result<Value, String>| d.ok().and_then(|d| d.get("data").and_then(Value::as_array).and_then(|l| l.first().cloned()));
        // (A game Roblox names nothing for is left blank, as the app left it out.)
        if let Some(g) = first(named).filter(|g| !text(g, "name").is_empty()) {
            out["name"] = text(&g, "name").into();
            out["creator"] = text(&g, "creatorName").into();
            out["playing"] = first(count).map(|c| int(&c, "playing")).unwrap_or(0).into();
            out["iconUrl"] = pics.get(&universe).cloned().map(Value::from).unwrap_or(Value::Null);
        }
    }
    out
}

/// Games by name for a box that picks one (the C# app's searchGames): Roblox's own search, the first twelve, one row
/// a place, each with its icon. Fewer than two letters find nothing.
pub fn search_rows(query: &str) -> Vec<Value> {
    let query = query.trim();
    if query.chars().count() < 2 {
        return Vec::new();
    }
    let mut seen = std::collections::HashSet::new();
    let found: Vec<Game> = search(query).unwrap_or_default().into_iter().filter(|g| seen.insert(g.place)).take(12).collect();
    let pics = icons(&found.iter().map(|g| g.universe).filter(|u| *u > 0).collect::<Vec<_>>());
    found
        .into_iter()
        .map(|g| serde_json::json!({ "placeId": g.place, "universeId": g.universe, "name": g.name, "playing": g.playing, "iconUrl": pics.get(&g.universe).cloned().unwrap_or_default() }))
        .collect()
}

fn post(url: &str, body: &Value, cookie: Option<&str>) -> Result<Value, String> {
    send("POST", url, body, cookie)
}

/// What the account has done with a game: favourited it, and voted it up (true) or down (None: neither). As Roblox's
/// game page asks (games.roblox.com, both answer only a sign-in: 401 without, checked 2026-11-18).
/// The third: whether Roblox takes a vote from the account at all (`canVote`; false with reasonForNotVoteable set when
/// it hasn't played the game: a vote then answers 403 "Play game required", the owner's log 2026-11-19).
pub fn game_mine(cookie: &str, universe: i64) -> Result<(Option<bool>, Option<bool>, Option<bool>), String> {
    let base = format!("https://games.roblox.com/v1/games/{universe}");
    let (fav, vote) = both(|| get(&format!("{base}/favorites"), Some(cookie)), || get(&format!("{base}/votes/user"), Some(cookie)));
    let vote = vote.ok();
    Ok((
        fav.ok().and_then(|d| d.get("isFavorited").and_then(Value::as_bool)),
        vote.as_ref().and_then(|d| d.get("userVote").and_then(Value::as_bool)),
        vote.as_ref().and_then(|d| d.get("canVote").and_then(Value::as_bool)),
    ))
}

/// Favourites the game or takes it out: the call Roblox's game page makes (its GameDetails script, read 2026-11-18).
pub fn set_favorite(cookie: &str, universe: i64, on: bool) -> Result<(), String> {
    post_ok(&format!("https://games.roblox.com/v1/games/{universe}/favorites"), &serde_json::json!({ "isFavorited": on }), Some(cookie))
}

/// Votes the game up or down (games.roblox.com's user-votes; it answers "XSRF token invalid" to a bare call, so it is
/// there, but its body is from Roblox's published games API, not seen with a real sign-in yet).
pub fn set_vote(cookie: &str, universe: i64, up: bool) -> Result<(), String> {
    send_text("PATCH", &format!("https://games.roblox.com/v1/games/{universe}/user-votes"), &serde_json::json!({ "vote": up }), Some(cookie)).map(|_| ())
}

/// Takes the account's vote on a game back. Roblox's own game page does it by voting again with "null": a second click
/// on the thumb already chosen posts /voting/vote?assetId=<place>&vote=null (its GameDetails script, read 2026-11-20).
/// That call first; www.roblox.com is out of reach in some countries (Vietnam), so the games API's user-votes with a
/// null vote after it. What Roblox then holds is read back by the caller, so a wrong answer can't stay on screen.
pub fn clear_vote(cookie: &str, universe: i64, place: i64) -> Result<(), String> {
    post_ok(&format!("https://www.roblox.com/voting/vote?assetId={place}&vote=null"), &serde_json::json!({}), Some(cookie)).or_else(|first| {
        send_text("PATCH", &format!("https://games.roblox.com/v1/games/{universe}/user-votes"), &serde_json::json!({ "vote": null }), Some(cookie)).map(|_| ()).map_err(|_| first)
    })
}

/// Tells Roblox the account is on its site now, as roblox.com's own pages do (their PresenceRegistration script, read
/// 2026-11-18: this POST with these three fields every 30 s while the person is active; the interval and "enabled for
/// everyone" from apis.roblox.com/guac-v2/v1/bundles/user-heartbeats). It is what makes an account show online on the
/// website; the older presence.roblox.com register-app-presence is gone (404). `session` stands for the site's
/// session id (its cookie's `sessionid`, a UUID; the site sends "unknown" without one).
pub fn pulse(cookie: &str, session: &str) -> Result<(), String> {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    let body = serde_json::json!({
        "clientSideTimestampEpochMs": now,
        "sessionInfo": { "sessionId": session },
        "locationInfo": { "websiteLocationInfo": { "url": "https://www.roblox.com/home" } },
    });
    post_ok("https://apis.roblox.com/user-heartbeats-api/pulse", &body, Some(cookie))
}

/// A POST whose answer is only its status: an empty or non-JSON 2xx is done (some of Roblox's answer with nothing).
fn post_ok(url: &str, body: &Value, cookie: Option<&str>) -> Result<(), String> {
    send_text("POST", url, body, cookie).map(|_| ())
}

/// A write to Roblox (POST, DELETE…), with the token it asks for.
fn send(method: &str, url: &str, body: &Value, cookie: Option<&str>) -> Result<Value, String> {
    serde_json::from_str(&send_text(method, url, body, cookie)?).map_err(|e| e.to_string())
}

/// The same write, its answer as it came (some of Roblox's answer with nothing).
fn send_text(method: &str, url: &str, body: &Value, cookie: Option<&str>) -> Result<String, String> {
    let out = send_text_once(method, url, body, cookie);
    if let Err(why) = &out {
        if why != crate::net::UNREACHABLE {
            crate::net::note_failed(&format!("Roblox tab ({method})"), url, why);
        }
    }
    out
}

fn send_text_once(method: &str, url: &str, body: &Value, cookie: Option<&str>) -> Result<String, String> {
    let send = |token: Option<&str>| {
        let mut req = agent().request(method, url).set("Accept", "application/json").set("Content-Type", "application/json");
        if let Some(c) = cookie.filter(|c| !c.is_empty()) {
            req = req.set("Cookie", &signed(c));
        }
        if let Some(t) = token {
            req = req.set("x-csrf-token", t);
        }
        req.send_string(&body.to_string())
    };
    let res = match send(None) {
        // A signed-in POST is refused once with the token it wants; asked again with it.
        Err(ureq::Error::Status(403, r)) => match r.header("x-csrf-token").map(str::to_string) {
            Some(t) => send(Some(&t)),
            None => return Err(refused(url, 403, r)),
        },
        other => other,
    };
    match res {
        Ok(r) => {
            let mut text = String::new();
            r.into_reader().take(8 << 20).read_to_string(&mut text).map_err(|e| e.to_string())?;
            Ok(text)
        }
        Err(ureq::Error::Status(code, r)) => Err(refused(url, code, r)),
        Err(e) => Err(crate::net::unreachable("Roblox tab", url, &e.to_string())),
    }
}

/// A friend as the Friends page shows them: who, and where Roblox says they are (`kind`: 0 offline, 1 online, 2 in a
/// game, 3 in Studio). The game and its server are empty when the friend's privacy hides them from the asking account.
#[derive(Clone, Debug, Default)]
pub struct Friend {
    pub id: i64,
    pub name: String,
    pub display: String,
    pub face: String,
    pub kind: i64,
    pub game: String,
    pub game_icon: String,
    /// The game's wide picture (16:9), for the lane friends in it stand in.
    pub game_cover: String,
    pub place: i64,
    pub universe: i64,
    pub job: String,
    /// What this PC has kept of them (friend_shifts): when it last saw them online (0: never yet), and the game it has
    /// seen them in most. Roblox shows neither.
    pub seen: i64,
    pub usual: String,
    /// In a game: since when this PC has seen them in it (0: not in one). Roblox doesn't say when they joined.
    pub since: i64,
    /// A friend request: when it was sent (unix seconds, 0: not known).
    pub sent: i64,
}

/// When a request was sent: its friendRequest.sentAt ("2024-03-24T02:25:33.095Z"), as Roblox's own friends
/// store keeps it.
fn sent_at(v: &Value) -> i64 {
    v.pointer("/friendRequest/sentAt").and_then(Value::as_str).and_then(crate::time::iso_secs).unwrap_or(0)
}

fn text(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or_default().trim().to_string()
}

/// The ids of a user's friends: the paged list (Roblox's advice since accounts can have 1,000), else the old whole one.
fn friend_ids(cookie: &str, user: i64) -> Result<Vec<i64>, String> {
    let mut out: Vec<i64> = Vec::new();
    let mut cursor = String::new();
    for page in 0..20 {
        let mut url = format!("https://friends.roblox.com/v1/users/{user}/friends/find?userSort=1&limit=50");
        if !cursor.is_empty() {
            url += &format!("&cursor={}", enc(&cursor));
        }
        let d = match get(&url, Some(cookie)) {
            Ok(d) => d,
            Err(_) if page == 0 => {
                let d = get(&format!("https://friends.roblox.com/v1/users/{user}/friends"), Some(cookie))?;
                return Ok(d.get("data").and_then(Value::as_array).into_iter().flatten().map(|v| int(v, "id")).filter(|i| *i > 0).collect());
            }
            Err(e) => return Err(e),
        };
        out.extend(d.get("PageItems").and_then(Value::as_array).into_iter().flatten().map(|v| int(v, "id")).filter(|i| *i > 0));
        cursor = text(&d, "NextCursor");
        if cursor.is_empty() {
            break;
        }
    }
    out.dedup();
    Ok(out)
}

/// The square faces of users, 100 a call.
fn faces(users: &[i64]) -> HashMap<i64, String> {
    let (mut out, missing) = pics::known("faces", users);
    let got = each(missing.chunks(100).collect(), |chunk: &[i64]| {
        let ids: Vec<String> = chunk.iter().map(i64::to_string).collect();
        get(&format!("https://thumbnails.roblox.com/v1/users/avatar-headshot?userIds={}&size=150x150&format=Png&isCircular=false", ids.join(",")), None)
    });
    let mut learnt = HashMap::new();
    for d in got.into_iter().flatten() {
        // Only a face Roblox has made: a moderated one ("Blocked") comes as Roblox's crossed-out picture, which read as
        // a broken image on a card; without it the card shows the friend's first letter.
        for v in d.get("data").and_then(Value::as_array).into_iter().flatten().filter(|v| v.get("state").and_then(Value::as_str).is_none_or(|s| s == "Completed")) {
            if let Some(u) = v.get("imageUrl").and_then(Value::as_str).filter(|u| !u.is_empty()) {
                learnt.insert(int(v, "targetId"), u.to_string());
            }
        }
    }
    pics::learn("faces", &learnt);
    out.extend(learnt);
    out
}

/// An account's friends with where each is now, as that account may see it. The list and where they are must come;
/// the names, faces and game icons are extras (a friend without them still shows, by id).
pub fn friends(cookie: &str, user: i64) -> Result<Vec<Friend>, String> {
    #[cfg(feature = "shot")]
    if let Some(list) = friends_fixture() {
        return Ok(dress(list));
    }
    let ids = friend_ids(cookie, user)?;
    let mut list: Vec<Friend> = ids.iter().map(|id| Friend { id: *id, ..Default::default() }).collect();
    // Where they are, their names and their faces: asked together, a hundred friends an ask.
    let hundreds = || ids.chunks(100).collect::<Vec<&[i64]>>();
    let ((places, names), pictures) = both(
        || {
            both(
                || each(hundreds(), |chunk| post("https://presence.roblox.com/v1/presence/users", &serde_json::json!({ "userIds": chunk }), Some(cookie))),
                || each(hundreds(), |chunk| post("https://users.roblox.com/v1/users", &serde_json::json!({ "userIds": chunk, "excludeBannedUsers": false }), None)),
            )
        },
        || faces(&ids),
    );
    for d in places {
        put_places(&mut list, &d?);
    }
    for d in names.into_iter().flatten() {
        for v in d.get("data").and_then(Value::as_array).into_iter().flatten() {
            let id = int(v, "id");
            if let Some(f) = list.iter_mut().find(|f| f.id == id) {
                f.name = text(v, "name");
                f.display = text(v, "displayName");
            }
        }
    }
    for f in &mut list {
        if let Some(u) = pictures.get(&f.id) {
            f.face = u.clone();
        }
    }
    Ok(dress(list))
}

/// Where presence.roblox.com says each of the people is (an answer to its users ask), into the list.
fn put_places(list: &mut [Friend], d: &Value) {
    for v in d.get("userPresences").and_then(Value::as_array).into_iter().flatten() {
        let id = int(v, "userId");
        if let Some(f) = list.iter_mut().find(|f| f.id == id) {
            f.kind = int(v, "userPresenceType");
            f.game = if f.kind == 2 { text(v, "lastLocation") } else { String::new() };
            f.place = if int(v, "placeId") > 0 { int(v, "placeId") } else { int(v, "rootPlaceId") };
            f.universe = int(v, "universeId");
            f.job = text(v, "gameId");
        }
    }
}

/// Where Roblox says each of these people is now, as the signed-in account may see it (their privacy can hide the
/// game: in one, with none named), a hundred an ask.
fn place_people(cookie: &str, list: &mut [Friend]) -> Result<(), String> {
    let ids: Vec<i64> = list.iter().map(|f| f.id).collect();
    for d in each(ids.chunks(100).collect::<Vec<&[i64]>>(), |chunk| post("https://presence.roblox.com/v1/presence/users", &serde_json::json!({ "userIds": chunk }), Some(cookie))) {
        put_places(list, &d?);
    }
    Ok(())
}

/// Where one person is now, for their profile's line: online, in a game (which, with its icon) or in Studio, as the
/// signed-in account may see it. BLOXKEEP_SHOT_WHERE=kind|game|universeId|placeId: a hidden shot's answer.
pub fn whereabouts(cookie: &str, user: i64) -> Result<Friend, String> {
    let mut f = Friend { id: user, ..Default::default() };
    #[cfg(feature = "shot")]
    let shot = std::env::var("BLOXKEEP_SHOT_WHERE").ok();
    #[cfg(not(feature = "shot"))]
    let shot: Option<String> = None;
    if let Some(v) = shot {
        let p: Vec<&str> = v.split('|').collect();
        let num = |i: usize| p.get(i).and_then(|s| s.trim().parse::<i64>().ok()).unwrap_or(0);
        f.kind = num(0);
        f.game = p.get(1).map(|s| s.to_string()).unwrap_or_default();
        f.universe = num(2);
        f.place = num(3);
    } else {
        place_people(cookie, std::slice::from_mut(&mut f))?;
    }
    if f.universe > 0 {
        if let Some(u) = icons(&[f.universe]).remove(&f.universe) {
            f.game_icon = u;
        }
    }
    Ok(f)
}

/// The friend requests waiting for the signed-in account, newest first (the first 50).
pub fn requests(cookie: &str) -> Result<Vec<Friend>, String> {
    #[cfg(feature = "shot")]
    if let Some(list) = fixture_people("requests") {
        return Ok(dress(list));
    }
    let d = get("https://friends.roblox.com/v1/my/friends/requests?limit=50&friendRequestSort=1", Some(cookie))?;
    let list = d
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|v| int(v, "id") > 0)
        .map(|v| Friend { id: int(v, "id"), name: text(v, "name"), display: text(v, "displayName"), sent: sent_at(v), ..Default::default() })
        .collect();
    Ok(dress(list))
}

/// The people who follow a user ("followers") or whom the user follows ("followings"), newest first (the first 50).
/// Roblox gives these lists only to a signed-in request.
pub fn follows(cookie: &str, user: i64, which: &str) -> Result<Vec<Friend>, String> {
    #[cfg(feature = "shot")]
    if let Some(list) = fixture_people(which) {
        return Ok(dress(list));
    }
    let d = get(&format!("https://friends.roblox.com/v1/users/{user}/{which}?limit=50&sortOrder=Desc"), Some(cookie))?;
    let mut list: Vec<Friend> = d
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|v| int(v, "id") > 0)
        .map(|v| Friend { id: int(v, "id"), name: text(v, "name"), display: text(v, "displayName"), ..Default::default() })
        .collect();
    // (Where each is, as the Friends page shows it; without it they still show.)
    if let Err(e) = place_people(cookie, &mut list) {
        crate::net::note_worked(&format!("Roblox tab: where the {which} are not given by Roblox ({e})."));
    }
    Ok(dress(list))
}

/// Whether the signed-in account follows `user` (friends.roblox.com's following-exists, as Roblox's own API
/// description gives it). BLOXKEEP_SHOT_FOLLOW=1|0: a hidden shot's answer.
pub fn following(cookie: &str, user: i64) -> Result<bool, String> {
    #[cfg(feature = "shot")]
    if let Ok(v) = std::env::var("BLOXKEEP_SHOT_FOLLOW") {
        return Ok(v == "1");
    }
    let d = send("POST", "https://friends.roblox.com/v1/user/following-exists", &serde_json::json!({ "targetUserIds": [user] }), Some(cookie))?;
    Ok(d.get("followings").and_then(Value::as_array).into_iter().flatten().any(|f| int(f, "userId") == user && f.get("isFollowing").and_then(Value::as_bool) == Some(true)))
}

/// One of a game's events, with all Roblox's game page shows for it: its picture, its kind ("New content", "Challenge"),
/// its name and the line under it, the longer words, when it starts and ends (seconds), how many said they are going
/// and whether the account did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GameEvent {
    pub id: String,
    pub title: String,
    pub sub: String,
    pub about: String,
    pub kind: String,
    pub start: i64,
    pub end: i64,
    pub going: bool,
    /// The event's own picture (its first thumbnail, by rank), else the game's.
    pub media: i64,
    pub pic: String,
    /// "Going" on Roblox's counter; None when it couldn't be read.
    pub count: Option<i64>,
}

/// A game's events that are on or still to come, soonest first, as Roblox's game page lists them: virtual-events v2's
/// experience-events (what RoSeal asks; RoValra asks v1 with the same fields), else v3's game-events. Then each one's
/// picture (thumbnails' assets, the game's own when it has none) and its going count (v1's rsvps/counters).
/// BLOXKEEP_SHOT_EVENTS=file.json: a hidden shot's list (pictures and counts still asked).
pub fn game_events(cookie: &str, universe: i64) -> Result<Vec<GameEvent>, String> {
    let now = crate::time::now_secs();
    #[cfg(feature = "shot")]
    if let Ok(path) = std::env::var("BLOXKEEP_SHOT_EVENTS") {
        let d: Value = serde_json::from_str(&std::fs::read_to_string(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let mut list = read_events(&d, now);
        event_pictures(&mut list, universe);
        // (A shot's going counts: "goingCount" on its items.)
        for e in &mut list {
            e.count = d.get("data").and_then(Value::as_array).into_iter().flatten().find(|v| text(v, "id") == e.id).and_then(|v| v.get("goingCount")).and_then(Value::as_i64);
        }
        return Ok(list);
    }
    let v2 = format!("https://apis.roblox.com/virtual-events/v2/universes/{universe}/experience-events?endsAfter={}&visibility=public&limit=25", enc(&crate::time::iso(now)));
    let d = match get(&v2, Some(cookie)) {
        Ok(d) => d,
        Err(_) => get(&format!("https://apis.roblox.com/virtual-events/v3/universes/{universe}/game-events?pageSize=20"), Some(cookie))?,
    };
    let mut list = read_events(&d, now);
    event_pictures(&mut list, universe);
    let ids: Vec<String> = list.iter().take(9).map(|e| e.id.clone()).collect();
    let counts: HashMap<String, i64> = each(ids, |id| {
        let n = get(&format!("https://apis.roblox.com/virtual-events/v1/virtual-events/{id}/rsvps/counters"), Some(cookie)).ok()?.pointer("/counters/going")?.as_i64()?;
        Some((id, n))
    })
    .into_iter()
    .flatten()
    .collect();
    for e in &mut list {
        e.count = counts.get(&e.id).copied();
    }
    crate::net::note_worked(&format!("Game events read ({} on or to come, {} with a picture of their own, {} counts).", list.len(), list.iter().filter(|e| e.media > 0).count(), counts.len()));
    Ok(list)
}

/// Each event's picture: its own (thumbnails' assets at 768x432, 420x420 when Roblox refuses that size), else the
/// game's first.
fn event_pictures(list: &mut [GameEvent], universe: i64) {
    let ids: Vec<String> = list.iter().filter(|e| e.media > 0).map(|e| e.media.to_string()).collect();
    let mut got: HashMap<i64, String> = HashMap::new();
    if !ids.is_empty() {
        for size in ["768x432", "420x420"] {
            let url = format!("https://thumbnails.roblox.com/v1/assets?assetIds={}&returnPolicy=PlaceHolder&size={size}&format=Webp&isCircular=false", ids.join(","));
            let Ok(d) = get(&url, None) else { continue };
            for t in d.get("data").and_then(Value::as_array).into_iter().flatten() {
                let pic = text(t, "imageUrl");
                if !pic.is_empty() && text(t, "state") == "Completed" {
                    got.insert(int(t, "targetId"), pic);
                }
            }
            break;
        }
    }
    let game = if list.iter().any(|e| !got.contains_key(&e.media)) { shots(universe).into_iter().next().unwrap_or_default() } else { String::new() };
    for e in list.iter_mut() {
        e.pic = got.get(&e.media).cloned().unwrap_or_else(|| game.clone());
    }
}

/// Going to an event or not (v1's rsvps, as RoValra's Notify me sends it).
pub fn set_going(cookie: &str, id: &str, going: bool) -> Result<(), String> {
    let url = format!("https://apis.roblox.com/virtual-events/v1/virtual-events/{id}/rsvps");
    send_text("POST", &url, &serde_json::json!({ "rsvpStatus": if going { "going" } else { "notGoing" } }), Some(cookie)).map(|_| ())
}

/// Roblox's list as the page shows it: an event that is over, or not for everyone (private, cancelled, held by
/// moderation), is left out; its name in the account's language when Roblox gave one. Reads v2's shape (data,
/// eventTime.startUtc, eventVisibility, eventStatus, eventCategories, thumbnails) and v3's (gameEvents, startTime).
pub fn read_events(d: &Value, now: i64) -> Vec<GameEvent> {
    let pick = |v: &Value, own: &str, plain: &str| Some(text(v, own)).filter(|t| !t.is_empty()).unwrap_or_else(|| text(v, plain));
    let first_by_rank = |v: &Value, list: &str, field: &str| -> Option<Value> {
        let mut all: Vec<&Value> = v.get(list).and_then(Value::as_array)?.iter().collect();
        all.sort_by_key(|x| int(x, "rank"));
        all.first().and_then(|x| x.get(field)).cloned()
    };
    let mut out: Vec<GameEvent> = d
        .get("data")
        .or_else(|| d.get("gameEvents"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|v| matches!(pick(v, "eventVisibility", "visibility").as_str(), "" | "public"))
        .filter(|v| matches!(text(v, "eventStatus").as_str(), "" | "active"))
        .filter_map(|v| {
            let time = |utc: &str, plain: &str| v.pointer(&format!("/eventTime/{utc}")).and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| text(v, plain));
            let start = crate::time::iso_secs(&time("startUtc", "startTime"))?;
            let end = crate::time::iso_secs(&time("endUtc", "endTime")).unwrap_or(start);
            let title = pick(v, "displayTitle", "title");
            (end > now && !title.is_empty()).then(|| GameEvent {
                id: text(v, "id"),
                title,
                sub: pick(v, "displaySubtitle", "subtitle"),
                about: pick(v, "displayDescription", "description"),
                kind: first_by_rank(v, "eventCategories", "category").and_then(|c| c.as_str().map(str::to_string)).unwrap_or_default(),
                start,
                end,
                going: text(v, "userRsvpStatus") == "going",
                media: first_by_rank(v, "thumbnails", "mediaId").and_then(|m| m.as_i64().or_else(|| m.as_str().and_then(|s| s.parse().ok()))).unwrap_or(0),
                pic: String::new(),
                count: None,
            })
        })
        .collect();
    out.sort_by_key(|e| e.start);
    out
}

/// Roblox's people search, the one on roblox.com (no sign-in needed): the first dozen.
pub fn people(query: &str) -> Result<Vec<Friend>, String> {
    let url = format!("https://apis.roblox.com/search-api/omni-search?verticalType=user&searchQuery={}&sessionId={}&pageType=all", enc(query), session());
    let d = get(&url, None)?;
    let list = d
        .get("searchResults")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|g| g.get("contentGroupType").and_then(Value::as_str) == Some("User"))
        .flat_map(|g| g.get("contents").and_then(Value::as_array).cloned().unwrap_or_default())
        .filter(|v| int(v, "contentId") > 0)
        .take(12)
        .map(|v| Friend { id: int(&v, "contentId"), name: text(&v, "username"), display: text(&v, "displayName"), ..Default::default() })
        .collect();
    Ok(dress(list))
}

/// The faces and the game icons, and a name for one Roblox named none.
fn dress(mut list: Vec<Friend>) -> Vec<Friend> {
    // Names: Roblox's requests, followers and followings lists now give only ids ("name" and "displayName" empty: the
    // owner's Requests showed numbers and a bare "@", 2026-11-15), so they are asked of users.roblox.com, a hundred an
    // ask, as the friends list already did.
    let nameless: Vec<i64> = list.iter().filter(|f| f.name.is_empty()).map(|f| f.id).collect();
    if !nameless.is_empty() {
        let chunks: Vec<&[i64]> = nameless.chunks(100).collect();
        let mut named = 0;
        for d in each(chunks, |chunk| post("https://users.roblox.com/v1/users", &serde_json::json!({ "userIds": chunk, "excludeBannedUsers": false }), None)).into_iter().flatten() {
            for v in d.get("data").and_then(Value::as_array).into_iter().flatten() {
                let id = int(v, "id");
                if let Some(f) = list.iter_mut().find(|f| f.id == id) {
                    f.name = text(v, "name");
                    f.display = text(v, "displayName");
                    named += 1;
                }
            }
        }
        if named < nameless.len() {
            crate::net::note_worked(&format!("Roblox tab: {} of {} people's names not given by Roblox.", nameless.len() - named, nameless.len()));
        }
    }
    let want: Vec<i64> = list.iter().filter(|f| f.face.is_empty()).map(|f| f.id).collect();
    let got = faces(&want);
    let mut games: Vec<i64> = list.iter().filter(|f| f.universe > 0 && f.game_icon.is_empty()).map(|f| f.universe).collect();
    games.sort_unstable();
    games.dedup();
    let (pics, wide) = both(|| icons(&games), || covers(&games, "480x270"));
    for f in &mut list {
        if let Some(u) = got.get(&f.id) {
            f.face = u.clone();
        }
        if let Some(u) = pics.get(&f.universe) {
            f.game_icon = u.clone();
        }
        if let Some(u) = wide.get(&f.universe) {
            f.game_cover = u.clone();
        }
        if f.display.is_empty() {
            f.display = if f.name.is_empty() { f.id.to_string() } else { f.name.clone() };
        }
    }
    list
}

/// BLOXKEEP_SHOT_FRIENDS=file.json: a hidden shot's friends and requests, { friends: [{ id, name, displayName, kind, game,
/// placeId, universeId, jobId }], requests: [the same] }, since Roblox gives them only to a signed-in account.
#[cfg(feature = "shot")]
pub fn friends_fixture() -> Option<Vec<Friend>> {
    fixture_people("friends")
}

#[cfg(feature = "shot")]
fn fixture_people(key: &str) -> Option<Vec<Friend>> {
    let path = std::env::var("BLOXKEEP_SHOT_FRIENDS").ok()?;
    let d: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    Some(
        d.get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|v| Friend {
                id: int(v, "id"),
                name: text(v, "name"),
                display: text(v, "displayName"),
                kind: int(v, "kind"),
                game: text(v, "game"),
                place: int(v, "placeId"),
                universe: int(v, "universeId"),
                job: text(v, "jobId"),
                game_icon: text(v, "gameIcon"),
                // (In a game for a while: a shot's own spread of times, by id.)
                sent: sent_at(v),
                since: if int(v, "kind") == 2 { crate::time::now_secs() - 60 * (int(v, "id").rem_euclid(150) + 1) } else { 0 },
                ..Default::default()
            })
            .collect(),
    )
}

/// Something worn: its picture and name, and the fitting Roblox keeps with it (sent back as it came).
#[derive(Clone, Debug, Default)]
pub struct Item {
    pub id: i64,
    pub name: String,
    pub pic: String,
    /// Roblox's number for the kind of thing (8 a hat, 11 a shirt…).
    kind: i64,
    meta: Option<Value>,
}

/// What a worn thing is worth: Robux, whether it is a Limited, and the bundle it is a part of (0: none), whose price
/// this then is.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Price {
    pub robux: i64,
    pub limited: bool,
    pub pack: i64,
}

/// What things are worth, by id: None when Roblox has no price for one (never sold, or given away and off sale).
pub type Worth = HashMap<i64, Option<Price>>;

/// Prices read since the app opened: an avatar read again (after a change, or by another tab) asks only for what is new.
fn price_book() -> &'static std::sync::Mutex<Worth> {
    static BOOK: std::sync::OnceLock<std::sync::Mutex<Worth>> = std::sync::OnceLock::new();
    BOOK.get_or_init(Default::default)
}

/// The most bundles looked up in one read (a call each).
const PACKS: usize = 16;

/// What worn things are worth (no sign-in needed), which Roblox doesn't add up. A thing on sale: its price. A Limited:
/// what it has sold for lately (Roblox's recent average price), not what resellers ask. A thing with no price of its
/// own (a body part, an animation): the price of the cheapest bundle it comes in, which the things worn from it share.
pub fn worth(ids: &[i64]) -> Result<Worth, String> {
    let mut book: Worth = price_book().hold().clone();
    let ask: Vec<i64> = ids.iter().copied().filter(|id| !book.contains_key(id)).collect();
    let mut loose: Vec<i64> = Vec::new();
    for part in ask.chunks(100) {
        let items: Vec<Value> = part.iter().map(|id| serde_json::json!({ "itemType": "Asset", "id": id })).collect();
        let d = post("https://catalog.roblox.com/v1/catalog/items/details", &serde_json::json!({ "items": items }), None)?;
        for v in d.get("data").and_then(Value::as_array).into_iter().flatten() {
            let id = int(v, "id");
            let limited = v.get("itemRestrictions").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).any(|r| r.starts_with("Limited"));
            let price = v.get("price").and_then(Value::as_i64);
            let status = v.get("priceStatus").and_then(Value::as_str).unwrap_or_default();
            let off = status == "Off Sale" || v.get("isOffSale").and_then(Value::as_bool) == Some(true);
            let one = |robux| Some(Price { robux, limited, pack: 0 });
            let got = if limited {
                let sold = get(&format!("https://economy.roblox.com/v1/assets/{id}/resale-data"), None).ok().map(|d| int(&d, "recentAveragePrice")).filter(|p| *p > 0);
                sold.or(price.filter(|p| *p > 0)).and_then(one)
            } else {
                match price {
                    Some(p) if p > 0 => one(p),
                    Some(_) if status == "Free" || !off => one(0),
                    Some(_) => None,
                    None => {
                        loose.push(id);
                        continue;
                    }
                }
            };
            book.insert(id, got);
        }
    }
    for (n, id) in loose.iter().enumerate() {
        // Another part of a bundle already found brought this one's price with it.
        if book.contains_key(id) {
            continue;
        }
        // Past the most, or Roblox not answering: left unknown this time, asked again the next.
        if n >= PACKS {
            break;
        }
        let Ok(d) = get(&format!("https://catalog.roblox.com/v1/assets/{id}/bundles?limit=10"), None) else { continue };
        let best = d
            .get("data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|b| b.pointer("/product/priceInRobux").and_then(Value::as_i64).filter(|p| *p > 0).map(|p| (p, b)))
            .min_by_key(|(p, _)| *p);
        match best {
            Some((robux, b)) => {
                let price = Some(Price { robux, limited: false, pack: int(b, "id") });
                book.insert(*id, price);
                for it in b.get("items").and_then(Value::as_array).into_iter().flatten() {
                    let part = int(it, "id");
                    // A part looked up before with no bundle of its own to price it (a mood) takes this one's.
                    if text(it, "type") == "Asset" && loose.contains(&part) && !matches!(book.get(&part), Some(Some(_))) {
                        book.insert(part, price);
                    }
                }
            }
            None => {
                book.insert(*id, None);
            }
        }
    }
    let out: Worth = ids.iter().filter_map(|id| book.get(id).map(|p| (*id, *p))).collect();
    price_book().hold().extend(book);
    Ok(out)
}

/// Something owned, on a shelf of the inventory: `kind` is Roblox's name for what it is ("Hat", "ShirtAccessory").
#[derive(Clone, Debug, Default)]
pub struct Owned {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub pic: String,
}

/// The equipped background comes from Roblox's profile, separately from the avatar's worn assets.
pub fn profile_background(user: i64) -> Result<Option<Owned>, String> {
    let profile = post(
        "https://apis.roblox.com/profile-platform-api/v1/profiles/get",
        &serde_json::json!({
            "profileType": "User",
            "profileId": user.to_string(),
            "components": [{ "component": "ProfileBackground" }],
        }),
        None,
    )?;
    let components = profile.get("components").and_then(Value::as_object).ok_or("Missing profile components")?;
    let Some(id) = components.get("ProfileBackground").and_then(|v| v.get("assetId")).and_then(Value::as_i64).filter(|id| *id > 0) else {
        return Ok(None);
    };
    let pic = thumbs("assets?size=768x432&assetIds=", &[id]).remove(&id).ok_or("Background image unavailable")?;
    Ok(Some(Owned { id, pic, ..Owned::default() }))
}

/// The inventory's shelves, as the avatar editor groups them: a key, and the kinds asked for together.
pub const SHELVES: [(&str, &str); 6] = [
    ("acc", "Hat,HairAccessory,FaceAccessory,NeckAccessory,ShoulderAccessory,FrontAccessory,BackAccessory,WaistAccessory"),
    ("clothes", "Shirt,Pants,TShirt,TShirtAccessory,ShirtAccessory,PantsAccessory,JacketAccessory,SweaterAccessory,ShortsAccessory,LeftShoeAccessory,RightShoeAccessory,DressSkirtAccessory"),
    ("body", "DynamicHead,Torso,RightArm,LeftArm,LeftLeg,RightLeg"),
    ("anim", "IdleAnimation,WalkAnimation,RunAnimation,JumpAnimation,FallAnimation,ClimbAnimation,SwimAnimation,MoodAnimation"),
    ("emote", "EmoteAnimation"),
    ("background", "AvatarBackground"),
];

/// A page (50) of what a user owns of those kinds, newest first, and the mark for the next page (empty: the last).
/// The sign-in lets an account see its own when its inventory is private.
pub fn inventory(cookie: &str, user: i64, kinds: &str, cursor: &str) -> Result<(Vec<Owned>, String), String> {
    let mut url = format!("https://inventory.roblox.com/v2/users/{user}/inventory?assetTypes={kinds}&limit=50&sortOrder=Desc");
    if !cursor.is_empty() {
        url += &format!("&cursor={}", enc(cursor));
    }
    let d = get(&url, Some(cookie))?;
    let mut list: Vec<Owned> = d
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|v| int(v, "assetId") > 0)
        .map(|v| Owned { id: int(v, "assetId"), name: text(v, "name"), kind: text(v, "assetType"), pic: String::new() })
        .collect();
    let ids: Vec<i64> = list.iter().map(|o| o.id).collect();
    let size = if kinds == "AvatarBackground" { "768x432" } else { "150x150" };
    let got = thumbs(&format!("assets?size={size}&assetIds="), &ids);
    for o in &mut list {
        o.pic = got.get(&o.id).cloned().unwrap_or_default();
    }
    Ok((list, text(&d, "nextPageCursor")))
}

/// The inventory's shelves past the avatar's, as Roblox's own inventory page lists them (its categories, RoSeal's copy
/// of them, 2026-11-15: the owner found "a heap of shelves" missing): classic faces and heads, then what creators keep.
pub const LIBRARY: [(&str, &str); 7] = [
    ("inv.faces", "Face,Head"),
    ("inv.audio", "Audio"),
    ("inv.decals", "Decal"),
    ("inv.models", "Model"),
    ("inv.plugins", "Plugin"),
    ("inv.meshes", "MeshPart"),
    ("inv.videos", "Video"),
];

/// A page (50) of the bundles a user owns (bodies, animation packs, shoes, heads), newest first: catalog's
/// /v1/users/{id}/bundles, as Roblox's inventory asks it.
pub fn bundles_page(cookie: &str, user: i64, cursor: &str) -> Result<(Vec<Owned>, String), String> {
    let mut url = format!("https://catalog.roblox.com/v1/users/{user}/bundles?limit=50&sortOrder=Desc");
    if !cursor.is_empty() {
        url += &format!("&cursor={}", enc(cursor));
    }
    let d = get(&url, Some(cookie))?;
    let mut list: Vec<Owned> = d
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|v| int(v, "id") > 0)
        .map(|v| Owned { id: int(v, "id"), name: text(v, "name").trim().to_string(), kind: text(v, "bundleType"), pic: String::new() })
        .collect();
    let ids: Vec<i64> = list.iter().map(|o| o.id).collect();
    let got = thumbs("bundles/thumbnails?size=150x150&bundleIds=", &ids);
    for o in &mut list {
        o.pic = got.get(&o.id).cloned().unwrap_or_default();
    }
    Ok((list, text(&d, "nextPageCursor")))
}

/// A page (100) of the game passes a user owns: apis' game-passes/v1/users/{id}/game-passes, which pages by the last
/// pass's id (the mark for the next page; empty when the page wasn't full). Each pass's picture is its icon's.
pub fn passes_page(cookie: &str, user: i64, after: &str) -> Result<(Vec<Owned>, String), String> {
    let mut url = format!("https://apis.roblox.com/game-passes/v1/users/{user}/game-passes?count=100");
    if !after.is_empty() {
        url += &format!("&exclusiveStartId={}", enc(after));
    }
    let d = get(&url, Some(cookie))?;
    let rows: Vec<&Value> = d.get("gamePasses").and_then(Value::as_array).into_iter().flatten().filter(|v| int(v, "gamePassId") > 0).collect();
    let next = if rows.len() >= 100 { rows.last().map(|v| int(v, "gamePassId").to_string()).unwrap_or_default() } else { String::new() };
    let icons: Vec<i64> = rows.iter().map(|v| int(v, "iconAssetId")).filter(|i| *i > 0).collect();
    let got = thumbs("assets?size=150x150&assetIds=", &icons);
    let list = rows
        .into_iter()
        .map(|v| Owned { id: int(v, "gamePassId"), name: text(v, "name").trim().to_string(), kind: "GamePass".into(), pic: got.get(&int(v, "iconAssetId")).cloned().unwrap_or_default() })
        .collect();
    Ok((list, next))
}

/// A page (50) of the badges a user has won, newest first, and the mark for the next page (empty: the last): the
/// inventory's Badges shelf (badges.roblox.com's /v1/users/{id}/badges, with the sign-in so a private inventory answers
/// its owner).
pub fn badges_page(cookie: &str, user: i64, cursor: &str) -> Result<(Vec<Owned>, String), String> {
    let mut url = format!("https://badges.roblox.com/v1/users/{user}/badges?limit=50&sortOrder=Desc");
    if !cursor.is_empty() {
        url += &format!("&cursor={}", enc(cursor));
    }
    let d = get(&url, Some(cookie))?;
    let mut list: Vec<Owned> =
        d.get("data").and_then(Value::as_array).into_iter().flatten().filter(|v| int(v, "id") > 0).map(|v| Owned { id: int(v, "id"), name: text(v, "name").trim().to_string(), kind: "Badge".into(), pic: String::new() }).collect();
    let ids: Vec<i64> = list.iter().map(|o| o.id).collect();
    let got = thumbs("badges/icons?size=150x150&badgeIds=", &ids);
    for o in &mut list {
        o.pic = got.get(&o.id).cloned().unwrap_or_default();
    }
    Ok((list, text(&d, "nextPageCursor")))
}

/// Where layered clothing sits when it is put on (Roblox wants an order with each): shoes under trousers, a jacket
/// over a shirt. The numbers only say which is over which.
fn layer(kind: &str) -> Option<i64> {
    Some(match kind {
        "LeftShoeAccessory" | "RightShoeAccessory" => 3,
        "PantsAccessory" => 4,
        "ShortsAccessory" => 5,
        "DressSkirtAccessory" => 6,
        "TShirtAccessory" => 7,
        "ShirtAccessory" => 8,
        "SweaterAccessory" => 9,
        "JacketAccessory" => 10,
        _ => return None,
    })
}

/// The signed-in account puts one owned thing on. Roblox allows so many of a kind (avatar-rules: one shirt, three
/// hats…): when that kind is full, the one worn longest makes room. Then the whole list is set again.
pub fn put_on(cookie: &str, user: i64, asset: i64, kind: &str) -> Result<(), String> {
    // An emote isn't worn: it goes on the wheel.
    if kind == "EmoteAnimation" {
        return put_emote(cookie, user, asset);
    }
    let rules = get("https://avatar.roblox.com/v1/avatar-rules", None)?;
    let plain = |s: &str| s.replace([' ', '-'], "");
    let (kind_id, most) = rules
        .get("wearableAssetTypes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|t| plain(&text(t, "name")) == kind)
        .map(|t| (int(t, "id"), int(t, "maxNumber").max(1)))
        .ok_or("Roblox")?;
    let d = get(&format!("https://avatar.roblox.com/v4/avatar/users/{user}"), Some(cookie))?;
    // (No model in the answer: setting the list from nothing would take everything off.)
    let mut items = items_of(d.get("avatarModel").filter(|m| m.is_object()).ok_or("Roblox")?);
    if items.iter().any(|i| i.id == asset) {
        return Ok(());
    }
    while items.iter().filter(|i| i.kind == kind_id).count() as i64 >= most {
        let at = items.iter().position(|i| i.kind == kind_id).ok_or("Roblox")?;
        items.remove(at);
    }
    items.push(Item { id: asset, kind: kind_id, meta: layer(kind).map(|o| serde_json::json!({ "order": o, "version": 1 })), ..Default::default() });
    worn(&post("https://avatar.roblox.com/v2/avatar/set-wearing-assets", &serde_json::json!({ "assets": wear_list(&items) }), Some(cookie))?)
}

#[derive(Clone, Debug, Default)]
pub struct Outfit {
    pub id: i64,
    pub name: String,
    pub pic: String,
}

/// An account's avatar as Roblox shows it to anyone: the full picture, the body kind (R6, R15), what it wears and
/// the outfits it has saved.
#[derive(Clone, Debug, Default)]
pub struct Avatar {
    pub pic: String,
    pub rig: String,
    pub items: Vec<Item>,
    pub outfits: Vec<Outfit>,
    /// The body: its sizes (SCALES' order), the head's colour (hex), and from Roblox's avatar rules the least and most
    /// each size may be and the skin colours offered.
    pub scales: [f64; 5],
    pub colour: String,
    /// Every body part's colour (hex), the head's first: what a colour change repaints at once.
    pub body: Vec<String>,
    pub ranges: [(f64, f64); 5],
    pub palette: Vec<String>,
    /// The avatar's history kept on this PC, newest first, and which of them is the avatar as it is now.
    pub looks: Vec<Look>,
    pub now: usize,
    /// The emotes on its wheel (eight places at most), by place.
    pub emotes: Vec<Emote>,
}

/// An emote on the wheel: its place there (1 to 8).
#[derive(Clone, Debug, Default)]
pub struct Emote {
    pub id: i64,
    pub name: String,
    pub slot: i64,
    pub pic: String,
}

/// The wheel's places.
pub const EMOTES: i64 = 8;

/// The emotes on a user's wheel (Roblox shows them to anyone), without their pictures.
fn emotes_of(user: i64, cookie: Option<&str>) -> Result<Vec<Emote>, String> {
    let d = get(&format!("https://avatar.roblox.com/v2/avatar/users/{user}/avatar"), cookie)?;
    let mut list: Vec<Emote> = d
        .get("emotes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|v| int(v, "assetId") > 0)
        .map(|v| Emote { id: int(v, "assetId"), name: text(v, "assetName"), slot: int(v, "position"), pic: String::new() })
        .collect();
    list.sort_by_key(|e| e.slot);
    Ok(list)
}

/// The signed-in account puts an emote on its wheel, in the first free place; a full wheel is said, not overwritten.
fn put_emote(cookie: &str, user: i64, asset: i64) -> Result<(), String> {
    let have = emotes_of(user, Some(cookie))?;
    let slot = (1..=EMOTES).find(|s| !have.iter().any(|e| e.slot == *s)).ok_or(format!("{EMOTES}/{EMOTES}"))?;
    let d = post(&format!("https://avatar.roblox.com/v1/emotes/{asset}/{slot}"), &serde_json::json!({}), Some(cookie))?;
    if d.get("success").and_then(Value::as_bool) == Some(false) { Err("Roblox".into()) } else { Ok(()) }
}

/// The signed-in account takes the emote off a place of its wheel.
pub fn clear_emote(cookie: &str, slot: i64) -> Result<(), String> {
    let d = send("DELETE", &format!("https://avatar.roblox.com/v1/emotes/{slot}"), &serde_json::json!({}), Some(cookie))?;
    if d.get("success").and_then(Value::as_bool) == Some(false) { Err("Roblox".into()) } else { Ok(()) }
}

/// A look the avatar had, kept on this PC: when it was first seen, its picture (a file) and everything Roblox said
/// of it then (kind, sizes, colours, what it wore), so it can be put on again. Roblox keeps no such history: a change
/// there loses what was worn before unless it was saved as an outfit first.
#[derive(Clone, Debug, Default)]
pub struct Look {
    pub at: i64,
    pub pic: String,
    model: Value,
}

/// How many looks an account keeps; the oldest goes when another comes.
const LOOKS: usize = 40;

/// What Bloxkeep keeps of Roblox on this PC, that Roblox itself doesn't keep (an avatar's looks, the changes among an
/// account's friends): a folder each under this PC's Bloxkeep data (BLOXKEEP_KEPT: elsewhere, for a hidden shot).
pub fn kept(what: &str) -> std::path::PathBuf {
    std::env::var("BLOXKEEP_KEPT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into())).join("Bloxkeep"))
        .join(what)
}

/// Where an account's looks are kept.
fn looks_dir(user: i64) -> std::path::PathBuf {
    kept("avatar-history").join(user.to_string())
}

/// Something that changed among an account's friends between two reads of the list: a friend who is "new", one who is
/// "gone" (no longer a friend: either side ended it, or their account went) or one "renamed" (`was`: the name before).
/// Roblox tells an account none of these.
#[derive(Clone, Debug, Default)]
pub struct Shift {
    pub at: i64,
    pub kind: String,
    pub id: i64,
    pub name: String,
    pub display: String,
    pub was: String,
    pub face: String,
}

/// How many changes an account keeps.
const SHIFTS: usize = 100;

/// The friends just read are held against the ones known on this PC (a file an account), and what differs is kept:
/// returns the changes, newest first. The first read only learns who the friends are. A friend missing from one read
/// isn't gone yet (Roblox's list comes short now and then): one missing from two reads running is.
///
/// Each read also notes who is online and in which game, so a friend carries when this PC last saw them online and the
/// game it has seen them in most (a game counts once in five minutes, however often the list is read).
pub fn friend_shifts(user: i64, now: &mut [Friend]) -> Vec<Shift> {
    let dir = kept("friends-history");
    let path = dir.join(format!("{user}.json"));
    let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    // One read of an account's list at a time holds the file (a tab put away and shown again can start a second while
    // the first is still on its way): read, changed and written whole, else one's changes would be lost to the other's.
    let lock = friends_file_lock(user);
    let held = lock.lock().unwrap_or_else(|e| e.into_inner());
    let old: Option<Value> = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok());
    let mut known: serde_json::Map<String, Value> = old.as_ref().and_then(|o| o.get("known")).and_then(Value::as_object).cloned().unwrap_or_default();
    let mut missing: serde_json::Map<String, Value> = old.as_ref().and_then(|o| o.get("missing")).and_then(Value::as_object).cloned().unwrap_or_default();
    let mut shifts: Vec<Value> = old.as_ref().and_then(|o| o.get("shifts")).and_then(Value::as_array).cloned().unwrap_or_default();
    let mut seen: serde_json::Map<String, Value> = old.as_ref().and_then(|o| o.get("seen")).and_then(Value::as_object).cloned().unwrap_or_default();
    for f in now.iter_mut() {
        let key = f.id.to_string();
        let mut rec = seen.get(&key).filter(|r| r.is_object()).cloned().unwrap_or_else(|| serde_json::json!({}));
        if f.kind != 0 {
            rec["at"] = serde_json::json!(at);
        }
        if f.kind == 2 && !f.game.is_empty() && at - int(&rec, "gat") >= 300 {
            // (A spoilt file's "g" that isn't a list of games starts again: indexing into it would panic.)
            if !rec["g"].is_object() {
                rec["g"] = serde_json::json!({});
            }
            let n = rec["g"][f.game.as_str()].as_i64().unwrap_or(0);
            rec["g"][f.game.as_str()] = serde_json::json!(n + 1);
            rec["gat"] = serde_json::json!(at);
        }
        // How long in this game, from the first read that saw them in it: a read that sees them elsewhere, or none for
        // an hour (Bloxkeep was closed: they may have left and come back), starts it again.
        let game_key = if f.universe > 0 { f.universe } else { f.place };
        if f.kind == 2 && game_key > 0 {
            if int(&rec, "u") != game_key || at - int(&rec, "uat") > 3600 || int(&rec, "since") == 0 {
                rec["u"] = serde_json::json!(game_key);
                rec["since"] = serde_json::json!(at);
            }
            rec["uat"] = serde_json::json!(at);
            f.since = int(&rec, "since");
        } else if let Some(o) = rec.as_object_mut() {
            o.remove("u");
            o.remove("uat");
            o.remove("since");
        }
        f.seen = int(&rec, "at");
        f.usual = rec.get("g").and_then(Value::as_object).into_iter().flatten().max_by_key(|(_, n)| n.as_i64().unwrap_or(0)).map(|(g, _)| g.clone()).unwrap_or_default();
        if rec.as_object().is_some_and(|o| !o.is_empty()) {
            seen.insert(key, rec);
        }
    }
    let first = old.is_none();
    let mut fresh: Vec<Value> = Vec::new();
    let entry = |kind: &str, id: i64, name: &str, display: &str, was: &str| serde_json::json!({ "at": at, "kind": kind, "id": id, "name": name, "display": display, "was": was });
    for f in now.iter() {
        let key = f.id.to_string();
        missing.remove(&key);
        match known.get(&key) {
            None if !first => fresh.push(entry("new", f.id, &f.name, &f.display, "")),
            // A name is compared only when Roblox named the friend this time (the names are an extra that may not come).
            Some(k) if !f.name.is_empty() && !text(k, "display").is_empty() && text(k, "display") != f.display => fresh.push(entry("renamed", f.id, &f.name, &f.display, &text(k, "display"))),
            Some(k) if !f.name.is_empty() && !text(k, "name").is_empty() && text(k, "name") != f.name => fresh.push(entry("renamed", f.id, &f.name, &f.display, &format!("@{}", text(k, "name")))),
            _ => {}
        }
        if !f.name.is_empty() || !known.contains_key(&key) {
            known.insert(key, serde_json::json!({ "name": f.name, "display": f.display }));
        }
    }
    let here: std::collections::HashSet<String> = now.iter().map(|f| f.id.to_string()).collect();
    let absent: Vec<String> = known.keys().filter(|k| !here.contains(*k)).cloned().collect();
    for key in absent {
        if missing.contains_key(&key) {
            let k = known.remove(&key).unwrap_or(Value::Null);
            missing.remove(&key);
            seen.remove(&key);
            fresh.push(entry("gone", key.parse().unwrap_or(0), &text(&k, "name"), &text(&k, "display"), ""));
        } else {
            missing.insert(key, serde_json::json!(at));
        }
    }
    fresh.extend(shifts.drain(..));
    fresh.truncate(SHIFTS);
    let _ = std::fs::create_dir_all(&dir);
    // Written beside it and moved over it: a crash (or the PC going off) mid-write never leaves half a file.
    let tmp = dir.join(format!("{user}.json.tmp"));
    if std::fs::write(&tmp, serde_json::json!({ "known": known, "missing": missing, "shifts": fresh, "seen": seen }).to_string()).is_ok() && std::fs::rename(&tmp, &path).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    drop(held);
    let ids: Vec<i64> = fresh.iter().map(|v| int(v, "id")).filter(|i| *i > 0).collect();
    let got = faces(&ids);
    fresh
        .iter()
        .map(|v| Shift { at: int(v, "at"), kind: text(v, "kind"), id: int(v, "id"), name: text(v, "name"), display: text(v, "display"), was: text(v, "was"), face: got.get(&int(v, "id")).cloned().unwrap_or_default() })
        .collect()
}

/// The lock an account's friends file is changed under (friend_shifts), one an account.
fn friends_file_lock(user: i64) -> std::sync::Arc<std::sync::Mutex<()>> {
    static LOCKS: std::sync::OnceLock<std::sync::Mutex<HashMap<i64, std::sync::Arc<std::sync::Mutex<()>>>>> = std::sync::OnceLock::new();
    LOCKS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner()).entry(user).or_default().clone()
}

/// What makes two looks the same one: its sizes and colours, and what is worn with its fitting. Not R6 or R15: the
/// same clothes on the other body are the same look (each switch was kept as another, the owner's "lịch sử lặp"); the
/// look keeps the body it was last worn with.
fn look_mark(model: &Value) -> String {
    let mut worn: Vec<String> = model.get("assets").and_then(Value::as_array).into_iter().flatten().map(|a| format!("{}{}", int(a, "id"), a.get("meta").filter(|m| !m.is_null()).map(Value::to_string).unwrap_or_default())).collect();
    worn.sort();
    format!("{}|{}|{}", model.get("scales").map(Value::to_string).unwrap_or_default(), model.get("bodyColors").map(Value::to_string).unwrap_or_default(), worn.join(","))
}

impl Avatar {
    /// What names the avatar as it is now among models kept (pages/avatar.rs): its body (R6, R15) and its look.
    pub fn key(&self) -> Option<String> {
        let model = &self.looks.get(self.now)?.model;
        Some(format!("{}|{}", text(model, "playerAvatarType"), look_mark(model)))
    }
}

/// An account's kept looks, newest first, one of each (the newest of looks kept twice before R6 and R15 were one);
/// and whether some were twice.
fn read_looks(user: i64) -> (Vec<Look>, bool) {
    let dir = looks_dir(user);
    let list: Vec<Value> = std::fs::read_to_string(dir.join("index.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let mut seen = std::collections::HashSet::new();
    let mut twice = false;
    let looks = list
        .iter()
        .map(|v| Look { at: int(v, "at"), pic: dir.join(text(v, "pic")).to_string_lossy().replace('\\', "/"), model: v.get("model").cloned().unwrap_or(Value::Null) })
        .filter(|l| l.model.is_object())
        .filter(|l| {
            let first = seen.insert(look_mark(&l.model));
            if !first {
                twice = true;
                let _ = std::fs::remove_file(&l.pic);
            }
            first
        })
        .collect();
    (looks, twice)
}

fn write_looks(user: i64, looks: &[Look]) {
    let dir = looks_dir(user);
    let index: Vec<Value> = looks.iter().map(|l| serde_json::json!({ "at": l.at, "pic": std::path::Path::new(&l.pic).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), "model": l.model })).collect();
    crate::net::wrote("Avatar looks list", &dir.join("index.json"), std::fs::write(dir.join("index.json"), serde_json::to_string(&index).unwrap_or_default()));
}

/// The avatar as it was just read joins its history when it is a look not kept yet (so one made in Roblox's own app
/// is kept too, the next time Bloxkeep reads the avatar). Returns the history, newest first, and where this look is
/// in it.
fn keep_look(user: i64, model: &Value, pic_url: &str) -> (Vec<Look>, usize) {
    let (mut looks, twice) = read_looks(user);
    let mark = look_mark(model);
    if let Some(i) = looks.iter().position(|l| look_mark(&l.model) == mark) {
        // (Worn now on the other body: it keeps that one, to be worn back as it was.)
        let other_body = text(&looks[i].model, "playerAvatarType") != text(model, "playerAvatarType");
        if other_body {
            looks[i].model = model.clone();
        }
        if other_body || twice {
            write_looks(user, &looks);
        }
        return (looks, i);
    }
    // No picture yet (Roblox is still drawing it): kept the next time, with one.
    let bytes = (!pic_url.is_empty()).then(|| agent().get(pic_url).call().ok()).flatten().and_then(|r| {
        let mut b = Vec::new();
        r.into_reader().take(8 << 20).read_to_end(&mut b).ok().map(|_| b)
    });
    let Some(bytes) = bytes.filter(|b| !b.is_empty()) else { return (looks, usize::MAX) };
    let dir = looks_dir(user);
    let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let name = format!("{at}.png");
    if std::fs::create_dir_all(&dir).is_err() || std::fs::write(dir.join(&name), bytes).is_err() {
        return (looks, usize::MAX);
    }
    looks.insert(0, Look { at, pic: dir.join(&name).to_string_lossy().replace('\\', "/"), model: model.clone() });
    for old in looks.drain(LOOKS.min(looks.len())..) {
        let _ = std::fs::remove_file(&old.pic);
    }
    write_looks(user, &looks);
    (looks, 0)
}

/// The signed-in account's avatar becomes what a model says (a kept look, a saved outfit): the kind of body, its sizes
/// and colours, then what it wears.
fn become_model(cookie: &str, rig: &str, scales: Option<&Value>, colours: Option<&Value>, items: &[Item]) -> Result<(), String> {
    let kind = match rig {
        "R6" => Some(1),
        "R15" => Some(3),
        _ => None,
    };
    if let Some(r) = kind {
        post("https://avatar.roblox.com/v1/avatar/set-player-avatar-type", &serde_json::json!({ "playerAvatarType": r }), Some(cookie))?;
    }
    if let Some(sc) = scales.filter(|v| v.is_object()) {
        post("https://avatar.roblox.com/v1/avatar/set-scales", sc, Some(cookie))?;
    }
    if let Some(b) = colours.filter(|v| v.is_object()) {
        let part = |k: &str| text(b, k).trim_start_matches('#').to_string();
        let all = serde_json::json!({
            "headColor3": part("headColor"), "torsoColor3": part("torsoColor"),
            "rightArmColor3": part("rightArmColor"), "leftArmColor3": part("leftArmColor"),
            "rightLegColor3": part("rightLegColor"), "leftLegColor3": part("leftLegColor"),
        });
        post("https://avatar.roblox.com/v2/avatar/set-body-colors", &all, Some(cookie))?;
    }
    worn(&post("https://avatar.roblox.com/v2/avatar/set-wearing-assets", &serde_json::json!({ "assets": wear_list(items) }), Some(cookie))?)
}

/// The signed-in account goes back to a look of its history.
pub fn wear_look(cookie: &str, look: &Look) -> Result<(), String> {
    let m = &look.model;
    become_model(cookie, &text(m, "playerAvatarType"), m.get("scales"), m.get("bodyColors"), &items_of(m))
}

/// The body's sizes as Roblox names them, in the order the page shows them.
pub const SCALES: [&str; 5] = ["height", "width", "head", "proportion", "bodyType"];

/// Roblox's answer to a change of the body: refused when it says so.
fn done(d: &Value) -> Result<(), String> {
    if d.get("success").and_then(Value::as_bool) == Some(false) { Err("Roblox".into()) } else { Ok(()) }
}

/// The signed-in account's body becomes R15 or R6.
pub fn set_rig(cookie: &str, r15: bool) -> Result<(), String> {
    done(&post("https://avatar.roblox.com/v1/avatar/set-player-avatar-type", &serde_json::json!({ "playerAvatarType": if r15 { 3 } else { 1 } }), Some(cookie))?)
}

/// The signed-in account's skin, one colour all over (as the avatar editor's skin tones).
pub fn set_colour(cookie: &str, hex: &str) -> Result<(), String> {
    let h = hex.trim_start_matches('#');
    let all = serde_json::json!({ "headColor3": h, "torsoColor3": h, "rightArmColor3": h, "leftArmColor3": h, "rightLegColor3": h, "leftLegColor3": h });
    done(&post("https://avatar.roblox.com/v2/avatar/set-body-colors", &all, Some(cookie))?)
}

/// One of the signed-in account's sizes changes; the others are sent back as they are now.
pub fn set_scale(cookie: &str, user: i64, which: usize, value: f64) -> Result<(), String> {
    let d = get(&format!("https://avatar.roblox.com/v4/avatar/users/{user}"), Some(cookie))?;
    let mut scales = d.pointer("/avatarModel/scales").filter(|v| v.is_object()).cloned().ok_or("Roblox")?;
    scales[SCALES[which.min(4)]] = serde_json::json!((value * 100.).round() / 100.);
    done(&post("https://avatar.roblox.com/v1/avatar/set-scales", &scales, Some(cookie))?)
}

/// Pictures by id from one of the thumbnail lists, 100 a call; one still being drawn is asked for again a few times.
fn thumbs(path: &str, ids: &[i64]) -> HashMap<i64, String> {
    thumbs_from(path, ids, true)
}

/// `thumbs`, but `kept: false` asks Roblox even for an address kept on this PC (an avatar just changed: its old
/// picture was still trusted for two hours).
fn thumbs_from(path: &str, ids: &[i64], kept: bool) -> HashMap<i64, String> {
    let (mut out, missing) = if kept { pics::known(path, ids) } else { (HashMap::new(), ids.to_vec()) };
    let had: std::collections::HashSet<i64> = out.keys().copied().collect();
    for chunk in missing.chunks(100) {
        let list: Vec<String> = chunk.iter().map(i64::to_string).collect();
        let url = format!("https://thumbnails.roblox.com/v1/{path}{}&format=Png&isCircular=false", list.join(","));
        for round in 0..4 {
            let Ok(d) = get(&url, None) else { break };
            let mut pending = false;
            for v in d.get("data").and_then(Value::as_array).into_iter().flatten() {
                match v.get("imageUrl").and_then(Value::as_str).filter(|u| !u.is_empty()) {
                    Some(u) => {
                        out.insert(int(v, "targetId"), u.to_string());
                    }
                    None => pending |= v.get("state").and_then(Value::as_str) == Some("Pending"),
                }
            }
            if !pending || round == 3 {
                break;
            }
            std::thread::sleep(Duration::from_millis(1500));
        }
    }
    let learnt: HashMap<i64, String> = out.iter().filter(|(k, _)| !had.contains(*k)).map(|(k, v)| (*k, v.clone())).collect();
    pics::learn(path, &learnt);
    out
}

/// The pictures' addresses Roblox gave by id, kept on this PC a while (an item's for a week, a game's for a day, a
/// face's for two hours: it changes with the avatar), so a page opened again asks Roblox only for what is new. Asking
/// for an address costs a quarter of a second at least, and a picture still being drawn a second and a half more.
mod pics {
    use std::{
        collections::HashMap,
        sync::{LazyLock, Mutex},
        time::{Duration, Instant},
    };

    use serde_json::{Value, json};

    struct Known {
        /// "kind|id" → (address, when it was learnt, seconds since 1970).
        map: HashMap<String, (String, i64)>,
        saved: Instant,
        dirty: bool,
    }

    static KNOWN: LazyLock<Mutex<Known>> = LazyLock::new(|| {
        let mut map = HashMap::new();
        if let Ok(t) = std::fs::read_to_string(file())
            && let Ok(Value::Object(o)) = serde_json::from_str::<Value>(&t)
        {
            for (k, v) in o {
                if let (Some(u), Some(at)) = (v.get(0).and_then(Value::as_str), v.get(1).and_then(Value::as_i64)) {
                    map.insert(k, (u.to_string(), at));
                }
            }
        }
        Mutex::new(Known { map, saved: Instant::now() - Duration::from_secs(60), dirty: false })
    });

    fn file() -> std::path::PathBuf {
        super::kept("cache").join("picture-addresses.json")
    }

    use crate::time::now_secs as now;

    /// How long an address of this kind is trusted.
    fn life(kind: &str) -> i64 {
        if kind.starts_with("faces") || kind.starts_with("users/") {
            2 * 3600
        } else if kind.starts_with("icons") || kind.starts_with("covers") || kind.starts_with("groups/") {
            24 * 3600
        } else {
            7 * 24 * 3600
        }
    }

    /// The addresses kept for these ids, and the ids still to ask Roblox for.
    pub fn known(kind: &str, ids: &[i64]) -> (HashMap<i64, String>, Vec<i64>) {
        let (mut out, mut missing) = (HashMap::new(), Vec::new());
        let oldest = now() - life(kind);
        let k = KNOWN.lock().unwrap_or_else(|e| e.into_inner());
        for id in ids {
            match k.map.get(&format!("{kind}|{id}")) {
                Some((u, at)) if *at >= oldest => {
                    out.insert(*id, u.clone());
                }
                _ => missing.push(*id),
            }
        }
        (out, missing)
    }

    /// Addresses just given by Roblox; written to the PC at most every few seconds.
    pub fn learn(kind: &str, got: &HashMap<i64, String>) {
        let mut k = KNOWN.lock().unwrap_or_else(|e| e.into_inner());
        if !got.is_empty() {
            let at = now();
            for (id, u) in got {
                k.map.insert(format!("{kind}|{id}"), (u.clone(), at));
            }
            k.dirty = true;
        }
        if !k.dirty || k.saved.elapsed() < Duration::from_secs(3) {
            return;
        }
        // Older than any kind is trusted: dropped; and the oldest once there are too many.
        let cutoff = now() - 7 * 24 * 3600;
        k.map.retain(|_, (_, at)| *at >= cutoff);
        if k.map.len() > 40_000 {
            let mut ats: Vec<i64> = k.map.values().map(|(_, at)| *at).collect();
            ats.sort_unstable();
            let keep_from = ats[ats.len() - 30_000];
            k.map.retain(|_, (_, at)| *at >= keep_from);
        }
        let body = Value::Object(k.map.iter().map(|(key, (u, at))| (key.clone(), json!([u, at]))).collect());
        let path = file();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = path.with_extension("tmp");
        if std::fs::write(&tmp, body.to_string()).is_ok() && std::fs::rename(&tmp, &path).is_ok() {
            k.saved = Instant::now();
            k.dirty = false;
        }
    }
}

fn items_of(model: &Value) -> Vec<Item> {
    model
        .get("assets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|v| int(v, "id") > 0)
        .map(|v| Item {
            id: int(v, "id"),
            name: text(v, "name"),
            pic: String::new(),
            kind: v.pointer("/assetType/id").and_then(Value::as_i64).unwrap_or(0),
            meta: v.get("meta").filter(|m| !m.is_null()).cloned(),
            ..Default::default()
        })
        .collect()
}

/// What set-wearing-assets takes: each id, with its fitting when it has one.
fn wear_list(items: &[Item]) -> Value {
    Value::Array(
        items
            .iter()
            .map(|i| match &i.meta {
                Some(m) => serde_json::json!({ "id": i.id, "meta": m }),
                None => serde_json::json!({ "id": i.id }),
            })
            .collect(),
    )
}

/// A user's avatar (no sign-in needed: Roblox shows avatars to anyone).
pub fn avatar(user: i64) -> Result<Avatar, String> {
    let d = get(&format!("https://avatar.roblox.com/v4/avatar/users/{user}"), None)?;
    let model = d.get("avatarModel").cloned().unwrap_or(Value::Null);
    let mut items = items_of(&model);
    let worn: Vec<i64> = items.iter().map(|i| i.id).collect();
    // The rest doesn't wait for each other: the outfits (then their pictures), the big picture, the wheel (then its
    // pictures; an extra, the avatar shows without it), the pictures of what is worn, and the rules (extras too:
    // without them the body shows, with nothing to pick).
    let ((outfits, pic), ((emotes, got), rules)) = both(
        || {
            both(
                || {
                    let mut outfits: Vec<Outfit> = get(&format!("https://avatar.roblox.com/v2/avatar/users/{user}/outfits?page=1&itemsPerPage=50&isEditable=true"), None)
                        .ok()
                        .and_then(|d| d.get("data").and_then(Value::as_array).cloned())
                        .unwrap_or_default()
                        .iter()
                        .filter(|v| int(v, "id") > 0)
                        .map(|v| Outfit { id: int(v, "id"), name: text(v, "name"), pic: String::new() })
                        .collect();
                    let ids: Vec<i64> = outfits.iter().map(|o| o.id).collect();
                    let got = thumbs("users/outfits?size=150x150&userOutfitIds=", &ids);
                    for o in &mut outfits {
                        o.pic = got.get(&o.id).cloned().unwrap_or_default();
                    }
                    outfits
                },
                // (Asked afresh each time: this is read again after every change to the avatar.)
                || thumbs_from("users/avatar?size=420x420&userIds=", &[user], false).remove(&user).unwrap_or_default(),
            )
        },
        || {
            both(
                || {
                    both(
                        || {
                            let mut emotes = emotes_of(user, None).unwrap_or_default();
                            let ids: Vec<i64> = emotes.iter().map(|e| e.id).collect();
                            let got = thumbs("assets?size=150x150&assetIds=", &ids);
                            for e in &mut emotes {
                                e.pic = got.get(&e.id).cloned().unwrap_or_default();
                            }
                            emotes
                        },
                        || thumbs("assets?size=150x150&assetIds=", &worn),
                    )
                },
                || get("https://avatar.roblox.com/v1/avatar-rules", None).unwrap_or(Value::Null),
            )
        },
    );
    for i in &mut items {
        i.pic = got.get(&i.id).cloned().unwrap_or_default();
    }
    let num = |v: Option<&Value>, or: f64| v.and_then(Value::as_f64).unwrap_or(or);
    let mut scales = [1., 1., 1., 0., 0.];
    for (i, k) in SCALES.iter().enumerate() {
        scales[i] = num(model.pointer(&format!("/scales/{k}")), scales[i]);
    }
    let mut ranges = [(0., 0.); 5];
    for (i, k) in SCALES.iter().enumerate() {
        ranges[i] = (num(rules.pointer(&format!("/scales/{k}/min")), 0.), num(rules.pointer(&format!("/scales/{k}/max")), 0.));
    }
    let palette = rules.get("basicBodyColorsPalette").and_then(Value::as_array).into_iter().flatten().map(|c| text(c, "hexColor").trim_start_matches('#').to_uppercase()).filter(|h| h.len() == 6).collect();
    let colour = model.pointer("/bodyColors/headColor").and_then(Value::as_str).unwrap_or_default().trim_start_matches('#').to_uppercase();
    let body: Vec<String> = ["headColor", "torsoColor", "rightArmColor", "leftArmColor", "rightLegColor", "leftLegColor"]
        .iter()
        .filter_map(|k| model.pointer(&format!("/bodyColors/{k}")).and_then(Value::as_str))
        .map(|h| h.trim_start_matches('#').to_uppercase())
        .filter(|h| h.len() == 6)
        .collect();
    let (looks, now) = keep_look(user, &model, &pic);
    Ok(Avatar { pic, rig: text(&model, "playerAvatarType"), items, outfits, scales, colour, body, ranges, palette, looks, now, emotes })
}

/// Roblox's answer to a list of things to wear: refused when it says so, or names any it would not put on.
fn worn(d: &Value) -> Result<(), String> {
    let bad = d.get("invalidAssetIds").and_then(Value::as_array).is_some_and(|l| !l.is_empty()) || d.get("invalidAssets").and_then(Value::as_array).is_some_and(|l| !l.is_empty());
    if d.get("success").and_then(Value::as_bool) == Some(false) || bad { Err("Roblox".into()) } else { Ok(()) }
}

/// The signed-in account takes one thing off: what it wears now, without that, set again (Roblox has no call for one).
pub fn take_off(cookie: &str, user: i64, asset: i64) -> Result<(), String> {
    let d = get(&format!("https://avatar.roblox.com/v4/avatar/users/{user}"), Some(cookie))?;
    // (No model in the answer: setting the list from nothing would take everything off.)
    let mut items = items_of(d.get("avatarModel").filter(|m| m.is_object()).ok_or("Roblox")?);
    items.retain(|i| i.id != asset);
    worn(&post("https://avatar.roblox.com/v2/avatar/set-wearing-assets", &serde_json::json!({ "assets": wear_list(&items) }), Some(cookie))?)
}

/// A saved outfit with the things in it (its details), for changing one thing in many outfits at once.
#[derive(Clone, Debug, Default)]
pub struct OutfitParts {
    pub id: i64,
    pub name: String,
    pub pic: String,
    pub items: Vec<Item>,
}

/// Each saved outfit's things (v4 details, eight asked at a time), and the pictures of all of them. An outfit Roblox
/// doesn't answer for is left out.
pub fn outfit_parts(cookie: &str, outfits: &[Outfit]) -> Result<Vec<OutfitParts>, String> {
    let mut parts: Vec<OutfitParts> = Vec::new();
    for chunk in outfits.chunks(8) {
        let got = each(chunk.to_vec(), |o| {
            let d = get(&format!("https://avatar.roblox.com/v4/outfits/{}/details", o.id), Some(cookie)).ok()?;
            let m = d.get("outfitModel")?;
            Some(OutfitParts { id: o.id, name: o.name.clone(), pic: o.pic.clone(), items: items_of(m) })
        });
        parts.extend(got.into_iter().flatten());
    }
    if parts.is_empty() && !outfits.is_empty() {
        return Err("Roblox".into());
    }
    let mut ids: Vec<i64> = parts.iter().flat_map(|p| p.items.iter().map(|i| i.id)).collect();
    ids.sort_unstable();
    ids.dedup();
    let got = thumbs("assets?size=150x150&assetIds=", &ids);
    for p in &mut parts {
        for i in &mut p.items {
            i.pic = got.get(&i.id).cloned().unwrap_or_default();
        }
    }
    Ok(parts)
}

impl Item {
    /// Roblox's number for its kind (8 a hat, 11 a shirt…).
    pub fn kind_id(&self) -> i64 {
        self.kind
    }
}

/// The name Roblox's inventory and avatar rules give a kind (avatar-rules, spaces taken out), by its number.
fn kind_name(kind: i64) -> Result<String, String> {
    let rules = get("https://avatar.roblox.com/v1/avatar-rules", None)?;
    rules
        .get("wearableAssetTypes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|t| int(t, "id") == kind)
        .map(|t| text(t, "name").replace([' ', '-'], ""))
        .ok_or_else(|| "Roblox".into())
}

/// What the signed-in account owns of one kind (its first page, newest first), to put in an outfit in place of
/// another thing of that kind; and the kind's name.
pub fn owned_of_kind(cookie: &str, user: i64, kind: i64) -> Result<(Vec<Owned>, String), String> {
    let name = kind_name(kind)?;
    Ok((inventory(cookie, user, &name, "")?.0, name))
}

/// A saved outfit's things set to `items` (v3: a part of an outfit can be sent alone; here only its things).
pub fn set_outfit_items(cookie: &str, outfit: i64, items: &[Item]) -> Result<(), String> {
    let said = send_text("PATCH", &format!("https://avatar.roblox.com/v3/outfits/{outfit}"), &serde_json::json!({ "assets": wear_list(items) }), Some(cookie))?;
    match serde_json::from_str::<Value>(&said) {
        Ok(d) => worn(&d),
        // (Nothing, or no JSON, with a 200: done.)
        Err(_) => Ok(()),
    }
}

/// One thing changed in a saved outfit: `from` out, and `to` (its number and kind's name) in its place; None just takes
/// it out.
pub fn swap_in_outfit(cookie: &str, outfit: &OutfitParts, from: i64, to: Option<(i64, i64, String)>) -> Result<(), String> {
    let mut items: Vec<Item> = outfit.items.iter().filter(|i| i.id != from).cloned().collect();
    if let Some((id, kind, name)) = to {
        if !items.iter().any(|i| i.id == id) {
            items.push(Item { id, kind, meta: layer(&name).map(|o| serde_json::json!({ "order": o, "version": 1 })), ..Default::default() });
        }
    }
    set_outfit_items(cookie, outfit.id, &items)
}

/// The signed-in account puts a saved outfit on: its body kind, sizes and colours, then what it wears.
pub fn wear_outfit(cookie: &str, outfit: i64) -> Result<(), String> {
    let d = get(&format!("https://avatar.roblox.com/v4/outfits/{outfit}/details"), Some(cookie))?;
    let o = d.get("outfitModel").ok_or("Roblox")?;
    become_model(cookie, &text(o, "playerAvatarType"), o.get("scale"), o.get("bodyColors"), &items_of(o))
}

/// A community (group) a user is in, with their rank's name there.
#[derive(Clone, Debug, Default)]
pub struct Group {
    pub id: i64,
    pub name: String,
    pub role: String,
    pub members: i64,
    pub pic: String,
}

/// A game a community made.
#[derive(Clone, Debug, Default)]
pub struct ClubGame {
    pub place: i64,
    pub name: String,
    pub visits: i64,
    pub pic: String,
    /// Its wide picture (16:9), for a shelf's tile.
    pub cover: String,
    /// Its universe, for more of its pictures.
    pub universe: i64,
}

/// A community's page, as Roblox shows it to anyone: who owns it, what it says of itself, its shout, its ranks with how
/// many hold each (highest first) and the games it made.
#[derive(Clone, Debug, Default)]
pub struct Club {
    pub name: String,
    pub about: String,
    pub owner: String,
    pub owner_id: i64,
    pub shout: String,
    pub members: i64,
    pub pic: String,
    pub roles: Vec<(String, i64)>,
    pub games: Vec<ClubGame>,
    /// Its games that aren't public (private, or not open yet), which Roblox's community page leaves out but its games
    /// list gives anyone (accessFilter=1, "All"; checked 2026-11-24: one community 22 games of which 6 public, another
    /// 36 of which 2). Their names, pictures and visits come; who plays them and their game page don't.
    pub hidden: Vec<ClubGame>,
}

/// Games as games.roblox.com lists them (a community's, a user's), each with its icon and wide picture.
fn listed_games(all: &[Value]) -> Vec<ClubGame> {
    let universes: Vec<i64> = all.iter().map(|v| int(v, "id")).filter(|u| *u > 0).collect();
    let (pics, wide) = both(|| icons(&universes), || covers(&universes, "480x270"));
    all.iter()
        .filter(|v| v.pointer("/rootPlace/id").and_then(Value::as_i64).unwrap_or(0) > 0)
        .map(|v| ClubGame {
            universe: int(v, "id"),
            place: v.pointer("/rootPlace/id").and_then(Value::as_i64).unwrap_or(0),
            name: text(v, "name").trim().to_string(),
            visits: int(v, "placeVisits"),
            pic: pics.get(&int(v, "id")).cloned().filter(|u| !stand_in(u)).unwrap_or_default(),
            cover: wide.get(&int(v, "id")).cloned().filter(|u| !stand_in(u)).unwrap_or_default(),
        })
        .collect()
}

/// A games list's pages (games.roblox.com, `url` without its cursor), `pages` at most; and whether it goes on past them.
fn games_pages(url: &str, pages: usize) -> (Vec<Value>, bool) {
    let mut out = Vec::new();
    let mut cursor = String::new();
    for _ in 0..pages {
        let page = if cursor.is_empty() { url.to_string() } else { format!("{url}&cursor={cursor}") };
        let Ok(d) = get(&page, None) else { return (out, false) };
        out.extend(d.get("data").and_then(Value::as_array).cloned().unwrap_or_default());
        cursor = text(&d, "nextPageCursor");
        if cursor.is_empty() {
            break;
        }
    }
    (out, !cursor.is_empty())
}

pub fn club(id: i64) -> Result<Club, String> {
    let d = get(&format!("https://groups.roblox.com/v1/groups/{id}"), None)?;
    let mut roles: Vec<(i64, String, i64)> = get(&format!("https://groups.roblox.com/v1/groups/{id}/roles"), None)
        .ok()
        .and_then(|d| d.get("roles").and_then(Value::as_array).cloned())
        .unwrap_or_default()
        .iter()
        // (Rank 0 is the guest; the "base" role doubles the members' own.)
        .filter(|v| int(v, "rank") > 0 && v.get("isBase").and_then(Value::as_bool) != Some(true))
        .map(|v| (int(v, "rank"), text(v, "name"), int(v, "memberCount")))
        .collect();
    roles.sort_by_key(|r| std::cmp::Reverse(r.0));
    // The public ones and all of them, asked at once; the rest of "all" is what isn't public. (Two lists, not one:
    // "all" in pages of 100 could leave a public one out past the pages read.)
    let (public, every) = both(
        || games_pages(&format!("https://games.roblox.com/v2/groups/{id}/gamesV2?accessFilter=2&limit=50&sortOrder=Desc"), 1).0,
        || games_pages(&format!("https://games.roblox.com/v2/groups/{id}/gamesV2?accessFilter=1&limit=100&sortOrder=Desc"), 2).0,
    );
    let shown: std::collections::HashSet<i64> = public.iter().map(|v| int(v, "id")).collect();
    let others: Vec<Value> = every.into_iter().filter(|v| !shown.contains(&int(v, "id"))).collect();
    let (mut games, mut hidden) = both(|| listed_games(&public), || listed_games(&others));
    games.sort_by_key(|g| std::cmp::Reverse(g.visits));
    hidden.sort_by_key(|g| std::cmp::Reverse(g.visits));
    Ok(Club {
        name: text(&d, "name"),
        about: text(&d, "description").trim().to_string(),
        owner: d.pointer("/owner/username").and_then(Value::as_str).unwrap_or_default().to_string(),
        owner_id: d.pointer("/owner/userId").and_then(Value::as_i64).unwrap_or(0),
        shout: d.pointer("/shout/body").and_then(Value::as_str).unwrap_or_default().trim().to_string(),
        members: int(&d, "memberCount"),
        pic: thumbs("groups/icons?size=150x150&groupIds=", &[id]).remove(&id).unwrap_or_default(),
        roles: roles.into_iter().map(|r| (r.1, r.2)).collect(),
        games,
        hidden,
    })
}

/// The communities each user is in, with their rank's name there, asked once since the app opened.
fn club_book() -> &'static std::sync::Mutex<HashMap<i64, HashMap<i64, String>>> {
    static BOOK: std::sync::OnceLock<std::sync::Mutex<HashMap<i64, HashMap<i64, String>>>> = std::sync::OnceLock::new();
    BOOK.get_or_init(Default::default)
}

pub fn club_left(id: i64, user: i64) {
    {
        let mut book = club_book().hold();
        if let Some(groups) = book.get_mut(&user) {
            groups.remove(&id);
        }
    }
}

/// Which of these users are in a community, each with their rank's name (Roblox shows anyone's communities). Roblox's
/// own page tells this of one account at a time.
pub fn club_mine(id: i64, users: &[i64]) -> Vec<(i64, String)> {
    let mut out = Vec::new();
    for &user in users.iter().take(40) {
        let known = club_book().hold().get(&user).cloned();
        let of = match known {
            Some(of) => of,
            None => {
                let Ok(d) = get(&format!("https://groups.roblox.com/v2/users/{user}/groups/roles"), None) else { continue };
                let of: HashMap<i64, String> = d
                    .get("data")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .map(|v| (v.pointer("/group/id").and_then(Value::as_i64).unwrap_or(0), v.pointer("/role/name").and_then(Value::as_str).unwrap_or_default().to_string()))
                    .collect();
                club_book().hold().insert(user, of.clone());
                of
            }
        };
        if let Some(role) = of.get(&id) {
            out.push((user, role.clone()));
        }
    }
    out
}

/// The signed-in user's Roblox connections who joined this community.
pub fn club_friends(group: i64, user: i64, cookie: &str) -> Result<Vec<Friend>, String> {
    let response = get(&format!("https://groups.roblox.com/v1/users/{user}/friends/groups/roles"), Some(cookie))?;
    let rows = response.get("data").and_then(Value::as_array).ok_or_else(String::new)?;
    let mut members: Vec<Friend> = rows.iter().filter_map(|row| {
        let member = row.get("user")?;
        let id = int(member, "userId");
        let groups = row.get("groups").and_then(Value::as_array)?;
        if id <= 0 || !groups.iter().any(|g| g.pointer("/group/id").and_then(Value::as_i64) == Some(group)) {
            return None;
        }
        let name = text(member, "username");
        let display = text(member, "displayName");
        let display = if !display.is_empty() { display } else if !name.is_empty() { name.clone() } else { id.to_string() };
        Some(Friend { id, name, display, ..Default::default() })
    }).collect();
    let ids: Vec<i64> = members.iter().map(|f| f.id).collect();
    let mut pictures = faces(&ids);
    for friend in &mut members {
        friend.face = pictures.remove(&friend.id).unwrap_or_default();
    }
    members.sort_by_cached_key(|f| (f.display.to_lowercase(), f.id));
    Ok(members)
}

/// A user's profile as Roblox shows it to anyone, and what only their own sign-in gets (the Robux).
#[derive(Clone, Debug, Default)]
pub struct Profile {
    pub name: String,
    pub display: String,
    pub about: String,
    /// The day they joined, as Roblox writes it (2006-03-08T…).
    pub joined: String,
    pub face: String,
    /// The whole avatar, standing (the face is the head only).
    pub body: String,
    pub friends: i64,
    pub followers: i64,
    pub following: i64,
    pub robux: Option<i64>,
    pub groups: Vec<Group>,
    /// The games they marked as favourites (newest first) and all public ones they made (most played first: the page
    /// shows a shelf of them, See all the rest), as Roblox shows
    /// anyone; and their newest badges, which Roblox gives only to a signed-in request.
    pub favs: Vec<ClubGame>,
    pub made: Vec<ClubGame>,
    /// They made more than `made` holds (Roblox's list goes on past the pages read).
    pub made_more: bool,
    pub badges: Vec<Badge>,
    /// The things they show off on their page (Roblox's Collections: up to six, chosen by them).
    pub collection: Vec<Ware>,
    /// The pictures across the top of their page, sharp (768×432): the games they like, then more of the first one's
    /// own pictures when they like few. One 480×270 cover stretched across the page came out blurred (owner,
    /// 2026-11-17: "banner mờ câm").
    pub banner: Vec<String>,
    /// Each banner picture's id as uploaded (0: none), for the page to swap in the picture at 1024×576
    /// (`uploaded_picture`) once it is fetched (owner, 2026-11-19: "khi set banner thì cố gắng làm cho nó nét nhất").
    pub banner_assets: Vec<i64>,
}

/// A badge won in a game.
#[derive(Clone, Debug, Default)]
pub struct Badge {
    pub name: String,
    pub pic: String,
}

/// How many games or badges a profile's shelf shows.
pub const SHELF: usize = 14;

/// A list of games as games.roblox.com gives one (a user's favourites, the ones they made), with their icons.
fn games_list(url: &str) -> Vec<ClubGame> {
    listed_games(&games_pages(url, 1).0)
}

/// Roblox's grey stand-in for a game that has no picture of its own (its address names "GameIcon<n>" or
/// "GameMediaItem<n>" where a real one names "Image"; checked 2026-11-12): left out, so its tile shows the game's
/// letter rather than a grey Roblox mark that reads as a broken picture.
pub fn stand_in(url: &str) -> bool {
    url.split('/').any(|part| part.starts_with("GameIcon") || part.starts_with("GameMediaItem"))
}

/// A user's profile. The user must come; the counts, the Robux and the communities are left out when Roblox doesn't
/// give them.
pub fn profile(cookie: &str, user: i64) -> Result<Profile, String> {
    let count = |url: String, with: Option<&str>| get(&url, with).ok().map(|d| int(&d, "count")).unwrap_or(0);
    let signed = !cookie.is_empty();
    let communities = || {
        let mut groups: Vec<Group> = get(&format!("https://groups.roblox.com/v2/users/{user}/groups/roles"), None)
            .ok()
            .and_then(|d| d.get("data").and_then(Value::as_array).cloned())
            .unwrap_or_default()
            .iter()
            .filter_map(|v| {
                let g = v.get("group")?;
                Some(Group { id: int(g, "id"), name: text(g, "name"), role: v.get("role").map(|r| text(r, "name")).unwrap_or_default(), members: int(g, "memberCount"), pic: String::new() })
            })
            .filter(|g| g.id > 0)
            .collect();
        let ids: Vec<i64> = groups.iter().map(|g| g.id).collect();
        let got = thumbs("groups/icons?size=150x150&groupIds=", &ids);
        for g in &mut groups {
            g.pic = got.get(&g.id).cloned().unwrap_or_default();
        }
        groups
    };
    let favourites = || {
        let mut favs = games_list(&format!("https://games.roblox.com/v2/users/{user}/favorite/games?limit=25&sortOrder=Desc&accessFilter=2"));
        favs.truncate(SHELF);
        favs
    };
    // All their public games (two pages of 50), most played first: the page shows a shelf of them and the rest on See
    // all. Roblox's own profile shows only the ones they added to it, so the others read as hidden there.
    let creations = || {
        let (all, more) = games_pages(&format!("https://games.roblox.com/v2/users/{user}/games?accessFilter=2&limit=50&sortOrder=Desc"), 2);
        let mut made = listed_games(&all);
        made.sort_by_key(|g| std::cmp::Reverse(g.visits));
        (made, more)
    };
    let won_badges = || {
        if !signed {
            return Vec::new();
        }
        let mut badges: Vec<(i64, Badge)> = get(&format!("https://badges.roblox.com/v1/users/{user}/badges?limit=25&sortOrder=Desc"), Some(cookie))
            .ok()
            .and_then(|d| d.get("data").and_then(Value::as_array).cloned())
            .unwrap_or_default()
            .iter()
            .filter(|v| int(v, "id") > 0)
            .map(|v| (int(v, "id"), Badge { name: text(v, "name").trim().to_string(), pic: String::new() }))
            .take(SHELF)
            .collect();
        let ids: Vec<i64> = badges.iter().map(|b| b.0).collect();
        let got = thumbs("badges/icons?size=150x150&badgeIds=", &ids);
        for (id, b) in &mut badges {
            b.pic = got.get(id).cloned().unwrap_or_default();
        }
        badges.into_iter().map(|b| b.1).collect::<Vec<Badge>>()
    };
    let won = || both(won_badges, || collection(user));
    // Nothing here waits for anything else, so it is all asked at once (ten asks one after another took 3 s).
    let ((d, (friends, robux)), ((groups, (favs, made, made_more, banner)), ((badges, collection), ((face, body), (followers, following))))) = both(
        || {
            both(
                || get(&format!("https://users.roblox.com/v1/users/{user}"), None),
                || {
                    both(
                        // Roblox counts friends only for a signed-in request (it answers 0 to others).
                        || if signed { count("https://friends.roblox.com/v1/my/friends/count".into(), Some(cookie)) } else { count(format!("https://friends.roblox.com/v1/users/{user}/friends/count"), None) },
                        || if signed { robux(cookie, user) } else { None },
                    )
                },
            )
        },
        || {
            both(
                || {
                    let (groups, (favs, (made, made_more))) = both(communities, || both(favourites, creations));
                    let banner = banner_pics(&favs, &made);
                    (groups, (favs, made, made_more, banner))
                },
                || {
                    both(won, || {
                        both(
                            || both(|| faces(&[user]).remove(&user).unwrap_or_default(), || thumbs("users/avatar?size=420x420&userIds=", &[user]).remove(&user).unwrap_or_default()),
                            || both(|| count(format!("https://friends.roblox.com/v1/users/{user}/followers/count"), None), || count(format!("https://friends.roblox.com/v1/users/{user}/followings/count"), None)),
                        )
                    })
                },
            )
        },
    );
    let d = d?;
    Ok(Profile {
        name: text(&d, "name"),
        display: text(&d, "displayName"),
        about: text(&d, "description"),
        joined: text(&d, "created"),
        face,
        body,
        friends,
        followers,
        following,
        robux,
        groups,
        favs,
        made,
        made_more,
        badges,
        collection,
        banner_assets: banner.iter().map(|b| b.1).collect(),
        banner: banner.into_iter().map(|b| b.0).collect(),
    })
}

/// A profile's banner: the liked and made games' wide pictures at 768×432 (six at most), filled up with the first
/// game's own pictures (its Roblox page's) when there are fewer; each with its id as uploaded (0: none), which the
/// page fetches at 1024×576.
fn banner_pics(favs: &[ClubGame], made: &[ClubGame]) -> Vec<(String, i64)> {
    const MOST: usize = 6;
    let us: Vec<i64> = favs.iter().chain(made.iter()).map(|g| g.universe).filter(|u| *u > 0).take(MOST).collect();
    let big = covers_with_assets(&us);
    let mut out: Vec<(String, i64)> = Vec::new();
    let add = |out: &mut Vec<(String, i64)>, (u, a): (String, i64)| {
        if !stand_in(&u) && !out.iter().any(|(o, _)| *o == u) {
            out.push((u, a));
        }
    };
    for u in &us {
        if let Some(pic) = big.get(u) {
            add(&mut out, pic.clone());
        }
    }
    if out.len() < MOST {
        if let Some(first) = us.first() {
            for pic in shots_with_assets(*first) {
                if out.len() >= MOST {
                    break;
                }
                add(&mut out, pic);
            }
        }
    }
    out
}

/// Something in the Marketplace: an item or a bundle, who made it, and its price (None: not on sale; 0: free).
#[derive(Clone, Debug, Default)]
pub struct Ware {
    pub id: i64,
    pub bundle: bool,
    pub name: String,
    pub maker: String,
    pub price: Option<i64>,
    pub pic: String,
    /// What it says of itself, and Roblox's id for it as a thing sold (empty: an old kind this app can't buy).
    pub about: String,
    pub collectible: String,
}

/// What a thing's maker sells it for on the Marketplace, as Roblox says when its page is opened: the price, and
/// whether it can be bought there now (on sale, not only inside a game, not sold out).
#[derive(Clone, Debug, Default)]
pub struct Offer {
    pub price: i64,
    pub open: bool,
    product: String,
    seller: i64,
    seller_kind: String,
}

pub fn offer(collectible: &str) -> Result<Offer, String> {
    let d = post("https://apis.roblox.com/marketplace-items/v1/items/details", &serde_json::json!({ "itemIds": [collectible] }), None)?;
    let v = d.get(0).ok_or("Roblox")?;
    let product = text(v, "collectibleProductId");
    // (productSaleStatus 3: on sale; 0: off sale. A stock of 0 is a thing with no limit.)
    let open = int(v, "productSaleStatus") == 3 && text(v, "saleLocationType").starts_with("Shop") && (int(v, "assetStock") == 0 || int(v, "unitsAvailableForConsumption") > 0) && !product.is_empty();
    Ok(Offer { price: int(v, "price"), open, product, seller: int(v, "creatorId"), seller_kind: text(v, "creatorType") })
}

/// The signed-in account buys a thing from its maker at the price it was shown. The offer is read again first: a sale
/// that closed ("off") or a price that moved ("price") stops it here, and Roblox holds the purchase to the same price
/// itself. Asked once: nothing here sends it a second time.
pub fn buy(cookie: &str, user: i64, collectible: &str, price: i64) -> Result<(), String> {
    let o = offer(collectible)?;
    if !o.open {
        return Err("off".into());
    }
    if o.price != price {
        return Err("price".into());
    }
    let body = serde_json::json!({
        "collectibleItemId": collectible,
        "expectedCurrency": 1,
        "expectedPrice": price,
        "expectedPurchaserId": user.to_string(),
        "expectedPurchaserType": "User",
        "expectedSellerId": o.seller,
        "expectedSellerType": o.seller_kind,
        "idempotencyKey": session(),
        "collectibleProductId": o.product,
    });
    let d = post(&format!("https://apis.roblox.com/marketplace-sales/v1/item/{collectible}/purchase-item"), &body, Some(cookie))?;
    if d.get("purchased").and_then(Value::as_bool) == Some(true) {
        return Ok(());
    }
    Err(["errorMessage", "purchaseResult"].iter().map(|k| text(&d, k)).find(|t| !t.is_empty()).unwrap_or_else(|| "Roblox".into()))
}

/// A game's pass: what it gives, its price (None: not on sale), and whether the signed-in account that asked owns it.
/// (Kept in the cart between runs: pages/passes.rs.)
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Pass {
    pub id: i64,
    product: i64,
    pub name: String,
    pub about: String,
    pub price: Option<i64>,
    pub pic: String,
    pub owned: bool,
}

/// A game's passes, as Roblox lists them to anyone (a sign-in adds which of them its account owns).
pub fn passes(place: i64, cookie: Option<&str>) -> Result<Vec<Pass>, String> {
    let universe = int(&get(&format!("https://apis.roblox.com/universes/v1/places/{place}/universe"), None)?, "universeId");
    if universe == 0 {
        return Err("Roblox".into());
    }
    let d = get(&format!("https://apis.roblox.com/game-passes/v1/universes/{universe}/game-passes?passView=Full&pageSize=100"), cookie)?;
    let mut list: Vec<Pass> = d
        .get("gamePasses")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|v| int(v, "id") > 0)
        .map(|v| {
            let sold = v.get("isForSale").and_then(Value::as_bool) == Some(true);
            let name = [text(v, "displayName"), text(v, "name")].into_iter().find(|n| !n.trim().is_empty()).unwrap_or_default();
            Pass {
                id: int(v, "id"),
                product: int(v, "productId"),
                name: name.trim().to_string(),
                about: text(v, "displayDescription").trim().to_string(),
                price: v.get("price").and_then(Value::as_i64).filter(|_| sold),
                pic: String::new(),
                owned: v.get("isOwned").and_then(Value::as_bool) == Some(true),
            }
        })
        .collect();
    let ids: Vec<i64> = list.iter().map(|p| p.id).collect();
    let pics = thumbs("game-passes?size=150x150&gamePassIds=", &ids);
    for p in &mut list {
        p.pic = pics.get(&p.id).cloned().unwrap_or_default();
    }
    Ok(list)
}

/// The signed-in account buys a pass at the price it was shown. What Roblox sells it for is read again first: a sale
/// that closed ("off") or a price that moved ("price") stops it here, and Roblox holds the purchase to the same price
/// itself. Asked once: nothing here sends it a second time.
/// Passes as Roblox sells them now (their price, None when off sale): one it doesn't answer for is None.
pub fn passes_now(list: Vec<Pass>) -> Vec<Option<Pass>> {
    each(list, |mut p| {
        let now = get(&format!("https://apis.roblox.com/game-passes/v1/game-passes/{}/product-info", p.id), None).ok()?;
        p.price = now.get("PriceInRobux").and_then(Value::as_i64).filter(|_| now.get("IsForSale").and_then(Value::as_bool) == Some(true));
        p.product = int(&now, "ProductId");
        Some(p)
    })
}

pub fn buy_pass(cookie: &str, pass: &Pass, price: i64) -> Result<(), String> {
    let now = get(&format!("https://apis.roblox.com/game-passes/v1/game-passes/{}/product-info", pass.id), None)?;
    if now.get("IsForSale").and_then(Value::as_bool) != Some(true) {
        return Err("off".into());
    }
    let product = int(&now, "ProductId");
    if now.get("PriceInRobux").and_then(Value::as_i64) != Some(price) || product == 0 || product != pass.product {
        return Err("price".into());
    }
    let d = post(&format!("https://apis.roblox.com/game-passes/v1/game-passes/{product}/purchase"), &serde_json::json!({ "expectedPrice": price }), Some(cookie))?;
    if d.get("purchased").and_then(Value::as_bool) == Some(true) {
        return Ok(());
    }
    Err(["reason", "errorMessage"].iter().map(|k| text(&d, k)).find(|t| !t.is_empty()).unwrap_or_else(|| "Roblox".into()))
}

/// Which of these users own a thing of a kind ("Asset", "Bundle", "GamePass"): Roblox tells it of anyone whose
/// inventory is open; `me`, with its sign-in, of itself whatever its inventory's setting. Asked eight at a time; an
/// answer Roblox gave is kept for the session (a thing bought here is noted by the page itself), one it refused isn't.
pub fn owners(kind: &str, id: i64, users: &[i64], me: Option<(i64, &str)>) -> Vec<i64> {
    owned_by(kind, id, users, me).into_iter().filter(|(_, o)| *o).map(|(u, _)| u).collect()
}

/// Whether one account owns it: None when Roblox didn't say (not taken as "no": Buy waits rather than buy it twice).
pub fn owns(kind: &str, id: i64, user: i64, cookie: &str) -> Option<bool> {
    owned_by(kind, id, &[user], Some((user, cookie))).get(&user).copied()
}

/// What Roblox said of each account, kept for the session; one it didn't answer for is left out (asked again next time).
fn owned_by(kind: &str, id: i64, users: &[i64], me: Option<(i64, &str)>) -> HashMap<i64, bool> {
    type Owns = std::sync::Mutex<HashMap<(String, i64, i64), bool>>;
    static OWNS: std::sync::OnceLock<Owns> = std::sync::OnceLock::new();
    let known = OWNS.get_or_init(Default::default);
    let users: Vec<i64> = users.iter().take(40).copied().collect();
    let mut owns: HashMap<i64, bool> = {
        let k = known.lock().unwrap_or_else(|e| e.into_inner());
        users.iter().filter_map(|u| k.get(&(kind.to_string(), id, *u)).map(|o| (*u, *o))).collect()
    };
    let ask: Vec<i64> = users.iter().copied().filter(|u| !owns.contains_key(u)).collect();
    for group in ask.chunks(8) {
        let got = each(group.to_vec(), |user| {
            let cookie = me.filter(|m| m.0 == user).map(|m| m.1);
            (user, get(&format!("https://inventory.roblox.com/v1/users/{user}/items/{kind}/{id}/is-owned"), cookie).ok().and_then(|v| v.as_bool()))
        });
        let mut k = known.lock().unwrap_or_else(|e| e.into_inner());
        for (user, answer) in got {
            if let Some(o) = answer {
                k.insert((kind.to_string(), id, user), o);
                owns.insert(user, o);
            }
        }
    }
    owns.retain(|u, _| users.contains(u));
    owns
}

/// What is unread for the account: its notifications and its messages, as Roblox's top bar counts them (0 for one
/// Roblox doesn't say).
pub fn unread(cookie: &str) -> (i64, i64) {
    let n = get("https://notifications.roblox.com/v2/stream-notifications/unread-count", Some(cookie)).ok().and_then(|d| d.get("unreadNotifications").and_then(Value::as_i64)).unwrap_or(0);
    let m = get("https://privatemessages.roblox.com/v1/messages/unread/count", Some(cookie)).ok().and_then(|d| d.get("count").and_then(Value::as_i64)).unwrap_or(0);
    (n, m)
}

/// Roblox holding the account (its "not approved" page): the kind as Roblox writes it ("Warn", "Ban 1 Day", "Ban 7
/// Days", "Delete"…), when it began and ends (unix seconds; no end: for good, or a warning), the moderator's note, and
/// whether it can be taken back from here (a warning, or a ban that has run out: Roblox's own "I agree" button).
#[derive(Clone, Debug, Default)]
pub struct Held {
    pub kind: String,
    pub ends: Option<i64>,
    pub note: String,
    pub back: bool,
}

/// None when Roblox has nothing against the account (usermoderation's not-approved, as the website's page asks it).
pub fn held(cookie: &str) -> Result<Option<Held>, String> {
    let d = get("https://usermoderation.roblox.com/v1/not-approved", Some(cookie))?;
    let kind = d.get("punishmentTypeDescription").and_then(Value::as_str).unwrap_or_default().trim().to_string();
    if kind.is_empty() {
        return Ok(None);
    }
    let when = |k: &str| d.get(k).and_then(Value::as_str).and_then(crate::time::iso_secs);
    let ends = when("endDate");
    let now = crate::time::now_secs();
    let warn = kind.to_ascii_lowercase().starts_with("warn") || kind.to_ascii_lowercase().contains("reminder");
    Ok(Some(Held {
        back: warn || ends.is_some_and(|e| e <= now),
        ends,
        note: d.get("messageToUser").and_then(Value::as_str).unwrap_or_default().trim().to_string(),
        kind,
    }))
}

/// One place the account is signed in: Roblox's Settings › Security › Where you're logged in, read the way its own
/// page reads it (token-metadata-service; the page's source, account-security/sessionManagement, 2026-11-18).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Session {
    /// What logs it out (Roblox's "token": not the sign-in itself).
    pub token: String,
    /// Roblox's kind of device: "App", "Browser", "Studio" or "Unknown" ("" when Roblox has none).
    pub kind: String,
    /// The browser ("Chrome"…) for a browser, and the system ("Windows", "Android", "Xbox"…), as Roblox names them.
    pub app: String,
    pub os: String,
    /// City, region and country, the ones Roblox has (from the address: a VPN's place shows as the place).
    pub place: Vec<String>,
    pub ip: String,
    /// Last used (unix seconds); None for old ones Roblox knows nothing of.
    pub last: Option<i64>,
    /// Bloxkeep's own sign-in for the account (Roblox's "current session": it can't be logged out from itself).
    pub this: bool,
    /// A device Roblox trusts more (its "Trusted" chip).
    pub trusted: bool,
    /// The sign-in it was made from (a Roblox window opened from a signed-in one), "" for none.
    pub parent: String,
}

/// Every place the account is signed in, page after page (25 a page, as Roblox's page asks; at most 400).
pub fn sessions(cookie: &str) -> Result<Vec<Session>, String> {
    let base = "https://apis.roblox.com/token-metadata-service/v1/sessions?desiredLimit=25";
    let mut out: Vec<Session> = Vec::new();
    let mut cursor = String::new();
    for _ in 0..16 {
        let url = if cursor.is_empty() {
            base.to_string()
        } else {
            format!("{base}&nextCursor={}", percent_encoding::utf8_percent_encode(&cursor, percent_encoding::NON_ALPHANUMERIC))
        };
        let d = ask(&url, cookie, "sessions")?;
        let word = |v: Option<&Value>, k: &str| v.and_then(|v| v.get(k)).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
        for s in d.get("sessions").and_then(Value::as_array).into_iter().flatten() {
            let (agent, at) = (s.get("agent"), s.get("location"));
            let token = word(Some(s), "token").unwrap_or_default();
            // (A page can repeat one from the page before: Roblox's own page asks again for short pages.)
            if token.is_empty() || out.iter().any(|o| o.token == token) {
                continue;
            }
            out.push(Session {
                token,
                kind: word(agent, "type").unwrap_or_default(),
                app: word(agent, "value").unwrap_or_default(),
                os: word(agent, "os").unwrap_or_default(),
                place: ["city", "subdivision", "country"].iter().filter_map(|k| word(at, k)).collect(),
                ip: word(Some(s), "lastAccessedIp").unwrap_or_default(),
                // (Milliseconds, as a string.)
                last: s.get("lastAccessedTimestampEpochMilliseconds").and_then(|v| v.as_str().and_then(|t| t.parse::<i64>().ok()).or_else(|| v.as_i64())).filter(|ms| *ms > 0).map(|ms| ms / 1000),
                this: s.get("isCurrentSession").and_then(Value::as_bool).unwrap_or(false),
                trusted: s.get("isTrustedSession").and_then(Value::as_bool).unwrap_or(false),
                parent: word(Some(s), "parentSessionToken").unwrap_or_default(),
            });
        }
        cursor = word(Some(&d), "nextCursor").unwrap_or_default();
        if !d.get("hasMore").and_then(Value::as_bool).unwrap_or(false) || cursor.is_empty() {
            break;
        }
    }
    Ok(out)
}

/// Logs the account out of one place (Roblox's "Log out of session"). Roblox refuses Bloxkeep's own.
pub fn log_out_session(cookie: &str, token: &str) -> Result<(), String> {
    post_ok("https://apis.roblox.com/token-metadata-service/v1/logout", &serde_json::json!({ "token": token }), Some(cookie))
}

/// The places sorted as Roblox's page shows them, and this PC's part apart from the rest.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Places {
    /// Bloxkeep's own sign-in, and how many Roblox windows were opened from it on this PC (same address): neither is
    /// logged out from here, so logging out every other place never closes a game being played.
    pub this: Option<Session>,
    pub here: usize,
    /// The others, the latest used first.
    pub known: Vec<Session>,
    /// Old ones Roblox knows nothing of (no time): one row on Roblox's page, logged out together.
    pub unknown: Vec<Session>,
}

/// Roblox's page's rules (commonHelpers.ts): one made from another at the same address, and not a different kind of
/// device, shows once (the newer); then the latest used first, the ones with no time last.
pub fn places(all: Vec<Session>) -> Places {
    let this = all.iter().find(|s| s.this).cloned();
    let (mut here, mut rest) = (0, Vec::new());
    for s in all.into_iter().filter(|s| !s.this) {
        match &this {
            Some(t) if s.parent == t.token && s.ip == t.ip => here += 1,
            _ => rest.push(s),
        }
    }
    let same_kind = |a: &Session, b: &Session| a.kind.is_empty() || b.kind.is_empty() || a.kind == b.kind;
    let gone: Vec<String> = rest
        .iter()
        .filter(|s| !s.parent.is_empty())
        .filter_map(|s| rest.iter().find(|p| p.token == s.parent && p.ip == s.ip && !(p.ip.is_empty() && s.ip.is_empty()) && same_kind(p, s)).map(|p| p.token.clone()))
        .collect();
    rest.retain(|s| !gone.contains(&s.token));
    let (mut known, unknown): (Vec<Session>, Vec<Session>) = rest.into_iter().partition(|s| s.last.is_some());
    known.sort_by(|a, b| b.last.cmp(&a.last));
    Places { this, here, known, unknown }
}

/// Takes the account back after a warning or a ban that has run out (the page's "I agree").
pub fn reactivate(cookie: &str) -> Result<(), String> {
    send_text("POST", "https://usermoderation.roblox.com/v1/not-approved/reactivate", &serde_json::json!({}), Some(cookie)).map(|_| ())
}

/// Saves what the account wears now as a new outfit called `name` (Roblox's "Create outfit"): its things, colours,
/// scales and body type, read from the avatar as it is.
pub fn save_outfit(cookie: &str, name: &str) -> Result<(), String> {
    let mut body = look_now(cookie)?;
    body["name"] = name.into();
    body["outfitType"] = "Avatar".into();
    send_text("POST", "https://avatar.roblox.com/v3/outfits/create", &body, Some(cookie)).map(|_| ())
}

/// An outfit made what the account wears now (its things, colours, scales and body type; its name kept).
pub fn update_outfit(cookie: &str, outfit: i64) -> Result<(), String> {
    let body = look_now(cookie)?;
    send_text("PATCH", &format!("https://avatar.roblox.com/v3/outfits/{outfit}"), &body, Some(cookie)).map(|_| ())
}

/// An outfit gone from the account (Roblox's Delete; what it held stays in the inventory).
pub fn delete_outfit(cookie: &str, outfit: i64) -> Result<(), String> {
    send_text("POST", &format!("https://avatar.roblox.com/v1/outfits/{outfit}/delete"), &serde_json::json!({}), Some(cookie)).map(|_| ())
}

/// What the account wears now, as an outfit's body.
fn look_now(cookie: &str) -> Result<Value, String> {
    let a = get("https://avatar.roblox.com/v2/avatar/avatar", Some(cookie))?;
    let assets: Vec<Value> = a
        .get("assets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|x| {
            let id = x.get("id")?.clone();
            let mut o = serde_json::Map::new();
            o.insert("id".into(), id);
            if let Some(m) = x.get("meta").filter(|m| m.is_object()) {
                o.insert("meta".into(), m.clone());
            }
            Some(Value::Object(o))
        })
        .collect();
    let mut body = serde_json::json!({ "assets": assets });
    for (from, to) in [("bodyColor3s", "bodyColor3s"), ("scales", "scale"), ("playerAvatarType", "playerAvatarType")] {
        if let Some(v) = a.get(from) {
            body[to] = v.clone();
        }
    }
    Ok(body)
}

/// The account wearing one more thing, as a picture, without changing what it wears (the Marketplace's "Try on"):
/// what it wears now, that thing put on (in the place of one of its kind, but for accessories, which stack), drawn by
/// Roblox's avatar render. The picture's address, or why not.
pub fn try_on(cookie: &str, asset: i64) -> Result<String, String> {
    // (Kinds worn many at once: hats, hair, face, neck, shoulder, front, back and waist things, layered clothes.)
    const STACK: [i64; 17] = [8, 41, 42, 43, 44, 45, 46, 47, 64, 65, 66, 67, 68, 69, 70, 71, 72];
    let d = get(&format!("https://economy.roblox.com/v2/assets/{asset}/details"), Some(cookie))?;
    let kind = d.get("AssetTypeId").and_then(Value::as_i64).ok_or_else(String::new)?;
    let a = get("https://avatar.roblox.com/v2/avatar/avatar", Some(cookie))?;
    let kind_of = |x: &Value| x.pointer("/assetType/id").and_then(Value::as_i64).unwrap_or(0);
    let mut assets: Vec<Value> = a
        .get("assets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|x| STACK.contains(&kind) || kind_of(x) != kind)
        .filter(|x| x.get("id").and_then(Value::as_i64) != Some(asset))
        .map(|x| {
            let mut o = serde_json::json!({ "id": x.get("id"), "assetType": { "id": kind_of(x), "name": x.pointer("/assetType/name").cloned().unwrap_or_default() } });
            for k in ["name", "currentVersionId", "meta"] {
                if let Some(v) = x.get(k) {
                    o[k] = v.clone();
                }
            }
            o
        })
        .collect();
    assets.push(serde_json::json!({ "id": asset, "name": d.get("Name").cloned().unwrap_or_default(), "assetType": { "id": kind, "name": "" } }));
    let colour = |k: &str| a.pointer(&format!("/bodyColor3s/{k}3")).cloned().unwrap_or_default();
    let body = serde_json::json!({
        "thumbnailConfig": { "thumbnailId": 1, "thumbnailType": "2d", "size": "420x420" },
        "avatarDefinition": {
            "assets": assets,
            "bodyColors": {
                "headColor": colour("headColor"), "torsoColor": colour("torsoColor"),
                "rightArmColor": colour("rightArmColor"), "leftArmColor": colour("leftArmColor"),
                "rightLegColor": colour("rightLegColor"), "leftLegColor": colour("leftLegColor"),
            },
            "scales": a.get("scales").cloned().unwrap_or_default(),
            "playerAvatarType": { "playerAvatarType": a.get("playerAvatarType").cloned().unwrap_or_default() },
        },
    });
    // (Roblox draws it a moment later: asked again until it is done.)
    for _ in 0..10 {
        let said: Value = serde_json::from_str(&send_text("POST", "https://avatar.roblox.com/v1/avatar/render", &body, Some(cookie))?).unwrap_or_default();
        let url = said.get("imageUrl").and_then(Value::as_str).unwrap_or_default();
        match said.get("state").and_then(Value::as_str).unwrap_or_default() {
            "Completed" if !url.is_empty() => return Ok(url.to_string()),
            "Pending" | "" => std::thread::sleep(std::time::Duration::from_millis(1200)),
            other => return Err(other.to_string()),
        }
    }
    Err("Pending".into())
}

/// The signed-in account's Robux.
pub fn robux(cookie: &str, user: i64) -> Option<i64> {
    get(&format!("https://economy.roblox.com/v1/users/{user}/currency"), Some(cookie)).ok().and_then(|d| d.get("robux").and_then(Value::as_i64))
}

/// A free limited that can still be had: how many are left of how many, whether its Marketplace page gives it too
/// (else only the game does), and the game it is given in (0 and empty until Roblox has said which).
#[derive(Clone, Debug, Default)]
pub struct Free {
    pub id: i64,
    pub name: String,
    pub pic: String,
    pub left: i64,
    pub total: i64,
    pub shop: bool,
    pub universe: i64,
    pub place: i64,
    pub game: String,
    /// Roblox has been asked for its game and answered (with no place still: it names none).
    pub known: bool,
}

/// Asked once since the app opened: the game a free limited is given in (0: Roblox names none), and a game's start
/// place and name.
fn free_book() -> &'static std::sync::Mutex<(HashMap<i64, i64>, HashMap<i64, (i64, String)>)> {
    static BOOK: std::sync::OnceLock<std::sync::Mutex<(HashMap<i64, i64>, HashMap<i64, (i64, String)>)>> = std::sync::OnceLock::new();
    BOOK.get_or_init(Default::default)
}

/// The most new things asked about in one read (a call each): the rest wait for the next read.
const FREE_ASKS: usize = 12;

/// The free limiteds still in stock among the Marketplace's lately updated free things (its first two pages; no
/// sign-in needed), the most left first. Roblox's own app has no such list. Each one's game is asked once (economy's
/// details name the universe, develop's universe its start place and name: games.roblox.com tells nothing of a game
/// to a request that isn't signed in).
pub fn free_new() -> Result<Vec<Free>, String> {
    let mut list: Vec<Free> = Vec::new();
    let mut cursor = String::new();
    for _ in 0..2 {
        let mut url = String::from("https://catalog.roblox.com/v1/search/items/details?Category=1&MaxPrice=0&SortType=3&Limit=30&IncludeNotForSale=false");
        if !cursor.is_empty() {
            url += &format!("&Cursor={}", enc(&cursor));
        }
        let d = match get(&url, None) {
            Ok(d) => d,
            Err(e) if list.is_empty() => return Err(e),
            Err(_) => break,
        };
        for v in d.get("data").and_then(Value::as_array).into_iter().flatten() {
            let (id, left, total) = (int(v, "id"), int(v, "unitsAvailableForConsumption"), int(v, "totalQuantity"));
            if text(v, "itemType") != "Asset" || text(v, "priceStatus") != "Free" || left <= 0 || total <= 0 || list.iter().any(|f| f.id == id) {
                continue;
            }
            list.push(Free { id, name: text(v, "name").trim().to_string(), left, total, shop: text(v, "saleLocationType").starts_with("Shop"), ..Default::default() });
        }
        cursor = text(&d, "nextPageCursor");
        if cursor.is_empty() {
            break;
        }
    }
    let ids: Vec<i64> = list.iter().map(|f| f.id).collect();
    let pics = thumbs("assets?size=420x420&assetIds=", &ids);
    let (mut wheres, mut games) = free_book().hold().clone();
    let mut asks = 0;
    // The ones with the most left are asked about first: those are what is worth going after.
    list.sort_by_key(|f| std::cmp::Reverse(f.left));
    for f in &mut list {
        f.pic = pics.get(&f.id).cloned().unwrap_or_default();
        let universe = match wheres.get(&f.id) {
            Some(u) => *u,
            None if asks >= FREE_ASKS => continue,
            None => {
                asks += 1;
                // Refused (asked too much): no more this read.
                let Ok(d) = get(&format!("https://economy.roblox.com/v2/assets/{}/details", f.id), None) else {
                    asks = FREE_ASKS;
                    continue;
                };
                let u = d.pointer("/SaleLocation/UniverseIds/0").and_then(Value::as_i64).unwrap_or(0);
                wheres.insert(f.id, u);
                u
            }
        };
        if universe == 0 {
            f.known = true;
            continue;
        }
        f.universe = universe;
        let game = match games.get(&universe) {
            Some(g) => g.clone(),
            None if asks >= FREE_ASKS => continue,
            None => {
                asks += 1;
                let Ok(d) = get(&format!("https://develop.roblox.com/v1/universes/{universe}"), None) else {
                    asks = FREE_ASKS;
                    continue;
                };
                let g = (int(&d, "rootPlaceId"), text(&d, "name").trim().to_string());
                games.insert(universe, g.clone());
                g
            }
        };
        (f.place, f.game) = game;
        f.known = true;
    }
    *free_book().hold() = (wheres, games);
    Ok(list)
}

/// A page (30) of the Marketplace, the one on roblox.com/catalog (no sign-in needed): what a search finds, or its
/// front shelf when nothing is asked; and the mark for the next page (empty: the last).
/// Narrowed and ordered as the Marketplace's filters ask: `filters` is the search's own words for them,
/// "&Category=11&SortType=3" and the like (catalog.roblox.com's v1 search: Category 1 all, 3 clothing, 11 accessories,
/// 12 animations; SalesTypeFilter 2 the Limiteds; SortType 1 most favourited, 2 best selling, 3 newest, 4 the lowest
/// price, which wants MinPrice=1 or the free things come first; no SortType with words: the best match).
pub fn catalog(query: &str, cursor: &str, filters: &str) -> Result<(Vec<Ware>, String), String> {
    let mut url = String::from("https://catalog.roblox.com/v1/search/items/details?Limit=30");
    url += filters;
    if !query.is_empty() {
        url += &format!("&Keyword={}", enc(query));
    }
    if !cursor.is_empty() {
        url += &format!("&Cursor={}", enc(cursor));
    }
    let d = get(&url, None)?;
    let mut list: Vec<Ware> = d
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|v| int(v, "id") > 0)
        .map(|v| {
            let status = text(v, "priceStatus").replace(' ', "").to_lowercase();
            let price = match status.as_str() {
                "free" => Some(0),
                "offsale" | "noresellers" => None,
                _ => Some(if int(v, "lowestPrice") > 0 { int(v, "lowestPrice") } else { int(v, "price") }),
            };
            Ware {
                id: int(v, "id"),
                bundle: text(v, "itemType") == "Bundle",
                name: text(v, "name"),
                maker: text(v, "creatorName"),
                price,
                pic: String::new(),
                about: text(v, "description").trim().to_string(),
                collectible: text(v, "collectibleItemId"),
            }
        })
        .collect();
    let of = |bundle: bool| list.iter().filter(|w| w.bundle == bundle).map(|w| w.id).collect::<Vec<i64>>();
    // (420 px: a tile is about 150 px wide on a 150 % screen and the thing's page 260; Roblox's 150 px came out soft.)
    let items = thumbs("assets?size=420x420&assetIds=", &of(false));
    let bundles = thumbs("bundles/thumbnails?size=420x420&bundleIds=", &of(true));
    for w in &mut list {
        w.pic = if w.bundle { bundles.get(&w.id) } else { items.get(&w.id) }.cloned().unwrap_or_default();
    }
    Ok((list, text(&d, "nextPageCursor")))
}

/// An account's own settings, as Roblox's Settings page shows them (accountinformation and accountsettings, both in
/// Roblox's published API docs): its email (Roblox masks it) and whether it is verified, whether a phone is verified,
/// its birthday, its country, its About words, and who may send it trades and of what worth. Each part is asked on
/// its own; one Roblox doesn't answer is left unknown (None), and only when none answers is it an error.
#[derive(Clone, Debug, Default)]
pub struct AccountSettings {
    pub email: Option<(String, bool)>,
    pub phone: Option<bool>,
    pub birthday: Option<(i64, i64, i64)>,
    pub country: Option<String>,
    pub about: Option<String>,
    pub trade_privacy: Option<String>,
    pub trade_value: Option<String>,
    pub voice: Option<Voice>,
}

/// Voice chat for the account, as voice.roblox.com/v1/settings answers it (the same read Roblox's own client makes,
/// CoreScripts VoiceChatServiceManager): on, or why not; locked with an end (Unix seconds), or locked for good (no end).
#[derive(Clone, Debug, Default)]
pub struct Voice {
    pub on: bool,
    pub opted: bool,
    pub eligible: bool,
    pub banned: bool,
    pub until: Option<i64>,
}

/// Who may send trades (Roblox's names, in the order its page lists them) and the number each is set by.
pub const TRADE_PRIVACY: [(&str, i64); 5] = [("All", 7), ("Followers", 6), ("Following", 5), ("Friends", 3), ("NoOne", 2)];
/// The least a trade sent to it must be worth.
pub const TRADE_VALUE: [(&str, i64); 4] = [("None", 1), ("Low", 2), ("Medium", 3), ("High", 4)];

pub fn account_settings(cookie: &str) -> Result<AccountSettings, String> {
    let info = "https://accountinformation.roblox.com/v1";
    let set = "https://accountsettings.roblox.com/v1";
    let urls = [
        (format!("{set}/email"), "email"),
        (format!("{info}/phone"), "phone"),
        (format!("{info}/birthdate"), "birthdate"),
        (format!("{set}/account/settings/account-country"), "country"),
        (format!("{info}/description"), "description"),
        (format!("{set}/trade-privacy"), "tradePrivacy"),
        (format!("{set}/trade-value"), "tradeValue"),
        ("https://voice.roblox.com/v1/settings".to_string(), "voice"),
    ];
    let got: Vec<Result<Value, String>> = crate::threads::scope(|sc| {
        let jobs: Vec<_> = urls.iter().map(|(u, k)| sc.spawn(move || ask(u, cookie, k))).collect();
        jobs.into_iter().map(|j| j.join().unwrap_or_else(|_| Err(String::new()))).collect()
    });
    if let Some(Err(why)) = got.iter().find(|g| g.is_err()).filter(|_| got.iter().all(|g| g.is_err() || g.as_ref().is_ok_and(Value::is_null))) {
        return Err(why.clone());
    }
    let part = |i: usize| got[i].as_ref().ok().filter(|v| v.is_object());
    let word = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    Ok(AccountSettings {
        email: part(0).map(|v| (text(v, "emailAddress"), v.get("verified").and_then(Value::as_bool).unwrap_or(false))),
        phone: part(1).map(|v| v.get("isVerified").and_then(Value::as_bool).unwrap_or(false) && !text(v, "phone").is_empty()),
        birthday: part(2).map(|v| (int(v, "birthDay"), int(v, "birthMonth"), int(v, "birthYear"))).filter(|b| b.1 > 0 && b.2 > 0),
        country: part(3).and_then(|v| v.get("value")).and_then(|c| word(c, "localizedName").or_else(|| word(c, "countryName"))).filter(|c| !c.is_empty()),
        about: part(4).map(|v| text(v, "description")),
        trade_privacy: part(5).and_then(|v| word(v, "tradePrivacy")),
        trade_value: part(6).and_then(|v| word(v, "tradeValue")),
        voice: part(7).map(|v| {
            let yes = |k: &str| v.get(k).and_then(Value::as_bool).unwrap_or(false);
            Voice {
                on: yes("isVoiceEnabled"),
                opted: yes("isUserOptIn"),
                eligible: yes("isUserEligible"),
                banned: yes("isBanned"),
                // ({"Seconds": n, "Nanos": n}; null when the lock has no end.)
                until: v.get("bannedUntil").and_then(|b| b.get("Seconds").or_else(|| b.get("seconds"))).and_then(Value::as_i64),
            }
        }),
    })
}

/// The account's About words, saved (Roblox answers with them as kept: it may have hidden some with #).
pub fn set_about(cookie: &str, words: &str) -> Result<String, String> {
    let d = post("https://accountinformation.roblox.com/v1/description", &serde_json::json!({ "description": words }), Some(cookie))?;
    Ok(text(&d, "description"))
}

/// Who may send trades: one of TRADE_PRIVACY's numbers.
pub fn set_trade_privacy(cookie: &str, n: i64) -> Result<(), String> {
    post_ok("https://accountsettings.roblox.com/v1/trade-privacy", &serde_json::json!({ "tradePrivacy": n }), Some(cookie))
}

/// The least a trade must be worth: one of TRADE_VALUE's numbers.
pub fn set_trade_value(cookie: &str, n: i64) -> Result<(), String> {
    post_ok("https://accountsettings.roblox.com/v1/trade-value", &serde_json::json!({ "tradeValue": n }), Some(cookie))
}

/// A call only a signed-in account gets an answer to. BLOXKEEP_SHOT_INBOX=file.json: a hidden shot answers from that
/// file instead ({ messages, notifications, transactions }: each as Roblox sends it), so the reading is exercised.
fn ask(url: &str, cookie: &str, _key: &str) -> Result<Value, String> {
    #[cfg(feature = "shot")]
    if let Ok(path) = std::env::var("BLOXKEEP_SHOT_INBOX") {
        let d: Value = std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
        return Ok(d.get(_key).cloned().unwrap_or(Value::Null));
    }
    get(url, Some(cookie))
}

/// Words out of Roblox's marked-up text: tags dropped (a line break kept), the common entities read.
fn plain(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut tag = String::new();
    let mut inside = false;
    for ch in s.chars() {
        match ch {
            '<' => {
                inside = true;
                tag.clear();
            }
            '>' if inside => {
                inside = false;
                let t = tag.trim_start_matches('/').to_lowercase();
                if t.starts_with("br") || t == "p" || t == "div" {
                    out.push('\n');
                }
            }
            _ if inside => tag.push(ch),
            _ => out.push(ch),
        }
    }
    crate::val::entities(&out).trim().to_string()
}

/// A chat: who it is with (its name, or the other person's), its last words, and the names of those in it.
#[derive(Clone, Debug, Default)]
pub struct Talk {
    pub id: String,
    pub name: String,
    pub face: String,
    pub last: String,
    pub unread: i64,
    pub group: bool,
    pub people: HashMap<i64, String>,
}

/// A line of a chat.
#[derive(Clone, Debug, Default)]
pub struct Line {
    pub sender: i64,
    pub text: String,
    pub when: String,
}

const CHAT: &str = "https://apis.roblox.com/platform-chat-api/v1";

/// The signed-in account's chats, the latest 30 (Roblox has no documentation for these: the names are the ones the
/// open-source roblox-api crate reads, 0.1.8).
pub fn talks(cookie: &str, me: i64) -> Result<Vec<Talk>, String> {
    let d = ask(&format!("{CHAT}/get-user-conversations?include_user_data=true&pageSize=30"), cookie, "conversations")?;
    let mut list: Vec<(Talk, i64)> = d
        .get("conversations")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|v| !text(v, "id").is_empty())
        .map(|v| {
            let people: HashMap<i64, String> = v
                .get("user_data")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
                .filter_map(|(k, u)| {
                    let n = text(u, "display_name");
                    Some((k.parse().ok()?, if n.is_empty() { text(u, "name") } else { n }))
                })
                .collect();
            let other = v.get("participant_user_ids").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_i64).find(|id| *id != me).unwrap_or(0);
            let group = text(v, "type") == "group";
            let name = text(v, "name");
            let name = if !group || name.is_empty() { people.get(&other).cloned().unwrap_or(name) } else { name };
            let talk = Talk {
                id: text(v, "id"),
                name,
                face: String::new(),
                last: v.pointer("/preview_message/content").and_then(Value::as_str).map(plain).unwrap_or_default(),
                unread: int(v, "unread_message_count"),
                group,
                people,
            };
            (talk, other)
        })
        .collect();
    let ids: Vec<i64> = list.iter().map(|(_, o)| *o).filter(|o| *o > 0).collect();
    let got = faces(&ids);
    for (t, o) in &mut list {
        t.face = got.get(o).cloned().unwrap_or_default();
    }
    Ok(list.into_iter().map(|(t, _)| t).collect())
}

/// A chat's latest lines, oldest first.
pub fn lines(cookie: &str, id: &str) -> Result<Vec<Line>, String> {
    let d = ask(&format!("{CHAT}/get-conversation-messages?conversation_id={}&pageSize=30", enc(id)), cookie, "chatMessages")?;
    let mut out: Vec<Line> = d
        .get("messages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|v| v.get("is_deleted").and_then(Value::as_bool) != Some(true))
        .map(|v| Line { sender: int(v, "sender_user_id"), text: plain(&text(v, "content")), when: text(v, "created_at") })
        .filter(|l| !l.text.is_empty())
        .collect();
    out.sort_by(|a, b| a.when.cmp(&b.when));
    Ok(out)
}

/// A group chat of the signed-in account and `people`, named `name`: its id.
pub fn make_group(cookie: &str, name: &str, people: &[i64]) -> Result<String, String> {
    let url = format!("{CHAT}/create-conversations");
    let d = post(&url, &serde_json::json!({ "conversations": [{ "type": "group", "name": name, "participant_user_ids": people }], "include_user_data": true }), Some(cookie))?;
    chat_refused(&url, &d)?;
    let id = d.pointer("/conversations/0/id").map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())).unwrap_or_default();
    if id.is_empty() {
        let why = format!("no chat in Roblox's answer: {}", d.to_string().chars().take(300).collect::<String>());
        crate::net::note_failed("Roblox tab (group chat)", &url, &why);
        return Err(why);
    }
    Ok(id)
}

/// People added to a group chat.
pub fn add_to_group(cookie: &str, id: &str, people: &[i64]) -> Result<(), String> {
    let url = format!("{CHAT}/add-users");
    chat_refused(&url, &post(&url, &serde_json::json!({ "conversation_id": id, "user_ids": people }), Some(cookie))?)
}

/// People taken out of a group chat.
pub fn remove_from_group(cookie: &str, id: &str, people: &[i64]) -> Result<(), String> {
    let url = format!("{CHAT}/remove-users");
    chat_refused(&url, &post(&url, &serde_json::json!({ "conversation_id": id, "user_ids": people }), Some(cookie))?)
}

/// A group chat's new name.
pub fn rename_group(cookie: &str, id: &str, name: &str) -> Result<(), String> {
    let url = format!("{CHAT}/update-conversations");
    chat_refused(&url, &post(&url, &serde_json::json!({ "conversations": [{ "id": id, "name": name }] }), Some(cookie))?)
}

/// An answer that came back fine but says no in its `status` (the chat calls answer with one): the refusal, in
/// error.log too.
fn chat_refused(url: &str, d: &Value) -> Result<(), String> {
    let status = text(d, "status");
    let low = status.to_ascii_lowercase();
    if ["fail", "error", "denied", "invalid", "forbidden", "not"].iter().any(|w| low.contains(w)) {
        crate::net::note_failed("Roblox tab (group chat)", url, &status);
        return Err(status);
    }
    Ok(())
}

/// The signed-in account says something in a chat.
pub fn say(cookie: &str, id: &str, words: &str) -> Result<(), String> {
    post_ok(&format!("{CHAT}/send-messages"), &serde_json::json!({ "conversation_id": id, "messages": [{ "content": words }] }), Some(cookie))
}

/// The signed-in account has seen a chat.
pub fn seen(cookie: &str, id: &str) -> Result<(), String> {
    post_ok(&format!("{CHAT}/mark-conversations"), &serde_json::json!({ "conversation_ids": [id] }), Some(cookie))
}

/// A message in the account's inbox.
#[derive(Clone, Debug, Default)]
pub struct Letter {
    pub id: i64,
    pub from: String,
    /// The sender's face.
    pub face: String,
    pub subject: String,
    pub body: String,
    pub when: String,
    pub read: bool,
}

/// The newest 30 messages of the signed-in account's inbox.
pub fn letters(cookie: &str) -> Result<Vec<Letter>, String> {
    let d = ask("https://privatemessages.roblox.com/v1/messages?pageNumber=0&pageSize=30&messageTab=Inbox", cookie, "messages")?;
    let mut list: Vec<(i64, Letter)> = d
        .get("collection")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|v| {
            let who = v.get("sender").cloned().unwrap_or(Value::Null);
            let name = text(&who, "displayName");
            (
                int(&who, "id"),
                Letter {
                    id: int(v, "id"),
                    from: if name.is_empty() { text(&who, "name") } else { name },
                    face: String::new(),
                    subject: plain(&text(v, "subject")),
                    body: plain(&text(v, "body")),
                    when: text(v, "created"),
                    read: v.get("isRead").and_then(Value::as_bool).unwrap_or(true),
                },
            )
        })
        .collect();
    // Who each is from, as a face (a face is public: no sign-in goes with the ask).
    let mut ids: Vec<i64> = list.iter().map(|x| x.0).filter(|id| *id > 0).collect();
    ids.sort_unstable();
    ids.dedup();
    let got = faces(&ids);
    for (id, m) in &mut list {
        m.face = got.get(id).cloned().unwrap_or_default();
    }
    Ok(list.into_iter().map(|x| x.1).collect())
}

/// The signed-in account has read that message.
/// The notifications seen: Roblox's count of unread ones (the bell's) back to 0, as its own site does when its
/// notification list is opened (POST, as the roblox-api crate's client calls it).
pub fn notices_seen(cookie: &str) -> Result<(), String> {
    post_ok("https://notifications.roblox.com/v2/stream-notifications/clear-unread", &serde_json::json!({}), Some(cookie))
}

pub fn mark_read(cookie: &str, id: i64) -> Result<(), String> {
    post_ok("https://privatemessages.roblox.com/v1/messages/mark-read", &serde_json::json!({ "messageIds": [id] }), Some(cookie))
}

/// A notification: Roblox's own words when it sends them, else its kind ("FriendRequestReceived"…) and the name it is
/// about, for the page to word.
#[derive(Clone, Debug, Default)]
pub struct Notice {
    pub text: String,
    pub kind: String,
    pub name: String,
    pub when: String,
}

/// The signed-in account's latest notifications: two pages of 20, as roblox.com asks for them (its notification stream's
/// PAGE_SIZE is 20; maxRows=30 was answered 400 and the tab showed nothing, owner 2026-11-15). A short page isn't the end
/// (Roblox filters within the window), an empty one is; a second page that fails keeps the first.
pub fn notices(cookie: &str) -> Result<Vec<Notice>, String> {
    const PAGE: usize = 20;
    let mut all: Vec<Value> = Vec::new();
    for page in 0..2 {
        let url = format!("https://notifications.roblox.com/v2/stream-notifications/get-recent?maxRows={PAGE}&startIndex={}", page * PAGE);
        let d = match ask(&url, cookie, "notifications") {
            Ok(d) => d,
            Err(e) if page == 0 => return Err(e),
            Err(_) => break,
        };
        let rows: Vec<Value> = d.as_array().cloned().unwrap_or_default();
        if rows.is_empty() || cfg!(feature = "shot") {
            all.extend(rows);
            break;
        }
        all.extend(rows);
    }
    crate::net::note_worked(&format!("Roblox tab: notifications read ({} at {PAGE} a page).", all.len()));
    Ok(all
        .iter()
        .map(|v| {
            let c = v.get("content").filter(|c| c.is_object());
            let state = c.map(|c| text(c, "currentState")).filter(|s| !s.is_empty()).unwrap_or_else(|| "default".into());
            let words = c.and_then(|c| c.pointer(&format!("/states/{state}/visualItems/textBody/0/label/text"))).and_then(Value::as_str).map(plain).unwrap_or_default();
            // The name it is about: the first "…UserName" its details carry.
            let name = v
                .get("metadataCollection")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_object)
                .flat_map(|o| o.iter())
                .find(|(k, v)| k.ends_with("UserName") && v.is_string())
                .and_then(|(_, v)| v.as_str())
                .unwrap_or_default()
                .to_string();
            Notice { text: words, kind: text(v, "notificationSourceType"), name, when: text(v, "eventDate") }
        })
        .collect())
}

/// A line of the Robux account: what it was for, with whom, how much (in is above zero) and when.
#[derive(Clone, Debug, Default)]
pub struct Deal {
    pub what: String,
    pub who: String,
    pub amount: i64,
    pub when: String,
    pub pending: bool,
}

/// A trade in one of the account's lists: who it is with, when it was made and when it lapses, and Roblox's word for
/// where it stands ("Open", "Completed", "Declined"…).
#[derive(Clone, Debug, Default)]
pub struct Trade {
    pub id: i64,
    pub who_id: i64,
    pub who: String,
    pub face: String,
    pub when: String,
    pub until: String,
    pub status: String,
}

/// One thing in a trade: its serial number when it has one (0: none), and what it has sold for lately (RAP).
#[derive(Clone, Debug, Default)]
pub struct TradeItem {
    /// The copy as a trade names it (a counter puts the same ones back in).
    pub instance: String,
    pub name: String,
    pub serial: i64,
    pub rap: i64,
    pub pic: String,
}

/// What one side puts in.
#[derive(Clone, Debug, Default)]
pub struct TradeSide {
    pub robux: i64,
    pub items: Vec<TradeItem>,
}

/// A trade's two sides: the account's own, and the other's.
#[derive(Clone, Debug, Default)]
pub struct TradeDetail {
    pub mine: TradeSide,
    pub theirs: TradeSide,
}

/// The lists Roblox keeps trades in, as its address names them.
pub const TRADES: [&str; 4] = ["inbound", "outbound", "completed", "inactive"];

/// The signed-in account's trades of one list, newest first (trades.roblox.com v1, as Roblox documents it).
pub fn trades(cookie: &str, list: &str) -> Result<Vec<Trade>, String> {
    let d = ask(&format!("https://trades.roblox.com/v1/trades/{list}?limit=25&sortOrder=Desc"), cookie, &format!("trades{list}"))?;
    let mut out: Vec<Trade> = d
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|v| int(v, "id") > 0)
        .map(|v| {
            let u = v.get("user").cloned().unwrap_or(Value::Null);
            let name = [text(&u, "displayName"), text(&u, "name")].into_iter().find(|n| !n.is_empty()).unwrap_or_default();
            Trade { id: int(v, "id"), who_id: int(&u, "id"), who: name, face: String::new(), when: text(v, "created"), until: text(v, "expiration"), status: text(v, "status") }
        })
        .collect();
    let users: Vec<i64> = out.iter().map(|t| t.who_id).filter(|u| *u > 0).collect();
    let got = faces(&users);
    for t in &mut out {
        t.face = got.get(&t.who_id).cloned().unwrap_or_default();
    }
    Ok(out)
}

/// A trade's two sides (v2, which names the things as they are traded now; v1's shape is read when that is what
/// comes). `with`: the other party, which tells the sides apart.
pub fn trade(cookie: &str, id: i64, with: i64) -> Result<TradeDetail, String> {
    let d = ask(&format!("https://trades.roblox.com/v2/trades/{id}"), cookie, "tradeDetail")?;
    // (side, the asset its picture is asked by)
    let mut sides: Vec<(i64, TradeSide, Vec<i64>)> = Vec::new();
    for key in ["participantAOffer", "participantBOffer"] {
        let Some(o) = d.get(key).filter(|o| o.is_object()) else { continue };
        let mut assets = Vec::new();
        let items = o
            .get("items")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|i| {
                let asset = if i.pointer("/itemTarget/itemType").and_then(Value::as_str) == Some("Asset") { i.pointer("/itemTarget/targetId").and_then(Value::as_str).and_then(|t| t.parse().ok()).unwrap_or(0) } else { 0 };
                assets.push(asset);
                TradeItem { instance: text(i, "collectibleItemInstanceId"), name: text(i, "itemName"), serial: int(i, "serialNumber"), rap: int(i, "recentAveragePrice"), pic: String::new() }
            })
            .collect();
        sides.push((o.pointer("/user/id").and_then(Value::as_i64).unwrap_or(0), TradeSide { robux: int(o, "robux"), items }, assets));
    }
    if sides.is_empty() {
        for o in d.get("offers").and_then(Value::as_array).into_iter().flatten() {
            let mut assets = Vec::new();
            let items = o
                .get("userAssets")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|i| {
                    assets.push(int(i, "assetId"));
                    TradeItem { instance: String::new(), name: text(i, "name"), serial: int(i, "serialNumber"), rap: int(i, "recentAveragePrice"), pic: String::new() }
                })
                .collect();
            sides.push((o.pointer("/user/id").and_then(Value::as_i64).unwrap_or(0), TradeSide { robux: int(o, "robux"), items }, assets));
        }
    }
    if sides.len() != 2 {
        return Err("Roblox".into());
    }
    let all: Vec<i64> = sides.iter().flat_map(|s| s.2.iter().copied()).filter(|a| *a > 0).collect();
    let pics = thumbs("assets?size=150x150&assetIds=", &all);
    for s in &mut sides {
        for (item, asset) in s.1.items.iter_mut().zip(&s.2) {
            item.pic = pics.get(asset).cloned().unwrap_or_default();
        }
    }
    let theirs = if sides[0].0 == with { 0 } else { 1 };
    let mut sides = sides.into_iter().map(|s| s.1);
    let (a, b) = (sides.next().unwrap_or_default(), sides.next().unwrap_or_default());
    Ok(if theirs == 0 { TradeDetail { mine: b, theirs: a } } else { TradeDetail { mine: a, theirs: b } })
}

/// The signed-in account accepts a trade, or declines it (its own, sent to someone: takes it back).
pub fn trade_act(cookie: &str, id: i64, accept: bool) -> Result<(), String> {
    post_ok(&format!("https://trades.roblox.com/v1/trades/{id}/{}", if accept { "accept" } else { "decline" }), &serde_json::json!({}), Some(cookie))
}

/// The signed-in account's latest Robux lines: purchases, sales, Robux bought and Premium's, newest first. A kind
/// Roblox doesn't answer for is left out; none answered is the error.
pub fn deals(cookie: &str, user: i64) -> Result<Vec<Deal>, String> {
    let mut out: Vec<Deal> = Vec::new();
    let mut failed = None;
    let mut any = false;
    for kind in ["Purchase", "Sale", "CurrencyPurchase", "PremiumStipend"] {
        match ask(&format!("https://economy.roblox.com/v2/users/{user}/transactions?transactionType={kind}&limit=25"), cookie, &format!("transactions{kind}")) {
            Ok(d) => {
                any = true;
                out.extend(d.get("data").and_then(Value::as_array).into_iter().flatten().map(|v| {
                    let what = v.get("details").map(|x| text(x, "name")).unwrap_or_default();
                    Deal {
                        what: if what.is_empty() { kind.to_string() } else { what },
                        who: v.get("agent").map(|x| text(x, "name")).unwrap_or_default(),
                        amount: v.pointer("/currency/amount").and_then(Value::as_i64).unwrap_or(0),
                        when: text(v, "created"),
                        pending: v.get("isPending").and_then(Value::as_bool).unwrap_or(false),
                    }
                }));
            }
            Err(e) => failed = Some(e),
        }
    }
    if !any {
        return Err(failed.unwrap_or_default());
    }
    out.sort_by(|a, b| b.when.cmp(&a.when));
    Ok(out)
}

/// Runs a call on the shared pool (crate::threads); its answer comes through the channel. A failed answer is written to error.log
/// with the page that asked (owner, 2026-11-15: every failure a person can meet is in error.log), ids as "#".
pub fn run<T: Answer + Send + 'static, F: FnOnce() -> T + Send + 'static>(f: F) -> oneshot::Receiver<T> {
    let (tx, rx) = oneshot::channel();
    crate::threads::run(move || {
        let answer = f();
        if let Some(why) = answer.failed() {
            crate::net::note_failed(&asker::<F>(), "", if why.is_empty() { "no reason given" } else { why });
        }
        let _ = tx.send(answer);
    });
    rx
}

/// "pages::avatar::wear" from the closure's type name: which page and step asked.
fn asker<F>() -> String {
    let name = std::any::type_name::<F>();
    let name = name.split_once("::").map(|x| x.1).unwrap_or(name);
    name.split("::").filter(|p| !p.starts_with('{') && !p.starts_with('<')).collect::<Vec<_>>().join("::")
}

/// What a background call answered, as far as failing goes: a Result's Err, the first failure in a pair; the rest
/// never fail (an empty list is an answer).
pub trait Answer {
    fn failed(&self) -> Option<&str> {
        None
    }
}
impl<T> Answer for Result<T, String> {
    fn failed(&self) -> Option<&str> {
        self.as_ref().err().map(String::as_str)
    }
}
impl<A: Answer, B: Answer> Answer for (A, B) {
    fn failed(&self) -> Option<&str> {
        self.0.failed().or_else(|| self.1.failed())
    }
}
impl<A: Answer, B: Answer, C: Answer> Answer for (A, B, C) {
    fn failed(&self) -> Option<&str> {
        self.0.failed().or_else(|| self.1.failed()).or_else(|| self.2.failed())
    }
}
impl<T> Answer for Option<T> {}
impl<T> Answer for Vec<T> {}
impl<K, V, S> Answer for HashMap<K, V, S> {}
impl Answer for Value {}
impl Answer for bool {}
impl Answer for () {}
impl Answer for i64 {}
impl Answer for String {}

/// BLOXKEEP_SHOT_EXPLORE=file.json: a hidden shot's charts, { sorts: [{ sortId, sortDisplayName, games: […] }] }, since
/// Roblox fills them only for a signed-in account.
#[cfg(feature = "shot")]
fn fixture() -> Option<(Vec<Sort>, HashMap<String, Vec<Game>>)> {
    let path = std::env::var("BLOXKEEP_SHOT_EXPLORE").ok()?;
    let d: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    let mut sorts = Vec::new();
    let mut games = HashMap::new();
    for v in d.get("sorts").and_then(Value::as_array).into_iter().flatten() {
        let id = v.get("sortId").and_then(Value::as_str).unwrap_or_default().to_string();
        sorts.push(Sort { id: id.clone(), name: v.get("sortDisplayName").and_then(Value::as_str).unwrap_or_default().into(), target: None, kind: None, treatment: None });
        games.insert(id, v.get("games").and_then(Value::as_array).into_iter().flatten().filter_map(game_of).collect());
    }
    Some((sorts, games))
}

const USER_SETTINGS: &str = "https://apis.roblox.com/user-settings-api/v1/user-settings";

/// One of the account's privacy settings: Roblox's word for it now, and the words it may be set to for this account,
/// in Roblox's order (a young account is offered fewer).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Choice {
    pub now: String,
    pub options: Vec<String>,
}

/// Who sees the account online and who may join it in a game (Roblox's Settings › Privacy), each None when Roblox
/// doesn't have it for the account. Roblox's words differ between the two: joining says "All" where online status
/// says "AllUsers" (RoSeal's and RoValra's code, 2026-11-15; Bloxkeep sent "AllUsers" for joining, which did nothing).
/// Read from settings-and-options, which has both with their choices: the plain user-settings answer left online
/// status out for the owner's account ("online status missing", their error.log, 2026-11-15).
pub fn visibility(cookie: &str) -> Result<(Option<Choice>, Option<Choice>), String> {
    // (A hidden shot of the account's Settings: an answer of its own.)
    #[cfg(feature = "shot")]
    if std::env::var("BLOXKEEP_SHOT_RSET").is_ok() {
        let c = |now: &str, o: &[&str]| Some(Choice { now: now.into(), options: o.iter().map(|s| s.to_string()).collect() });
        return Ok((c("Friends", &["AllUsers", "FriendsFollowingAndFollowers", "FriendsAndFollowing", "Friends", "NoOne"]), c("All", &["All", "Followers", "Following", "Friends", "NoOne"])));
    }
    let both = get(&format!("{USER_SETTINGS}/settings-and-options"), Some(cookie));
    let (online, join, from) = match &both {
        Ok(d) => {
            let (online, join) = visibility_full(d);
            (online, join, "settings-and-options")
        }
        // (That answer refused: the plain one, with the choices every account has.)
        Err(_) => {
            let (online, join) = visibility_plain(&get(USER_SETTINGS, Some(cookie))?);
            (online, join, "user-settings")
        }
    };
    let said = |c: &Option<Choice>| c.as_ref().map_or("missing".to_string(), |c| format!("{} choices", c.options.len()));
    crate::net::note_worked(&format!("Roblox tab: privacy settings read from {from} (online status {}, joining {}).", said(&online), said(&join)));
    Ok((online, join))
}

/// Sets them for the account (None leaves one as it is), in Roblox's own words for each. "AllUsers" for joining, the
/// word this used to send, is Roblox's "All".
pub fn set_visibility(cookie: &str, online: Option<&str>, join: Option<&str>) -> Result<(), String> {
    send_text("POST", USER_SETTINGS, &visibility_body(online, join), Some(cookie)).map(|_| ())
}

/// settings-and-options' answer: who sees the account online and who may join it, with the choices it offers.
pub fn visibility_full(d: &Value) -> (Option<Choice>, Option<Choice>) {
    let one = |key: &str| -> Option<Choice> {
        let v = d.get(key)?;
        let now = v.get("currentValue").and_then(Value::as_str).unwrap_or_default().to_string();
        // (An option that needs a parent's consent or an age check can't be set from here.)
        let options: Vec<String> = v
            .get("options")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|o| !matches!(o.get("requirement").and_then(Value::as_str), Some("ParentalConsent" | "ParentConsentInherited" | "ContentAgeRestrictionVerification")))
            .filter_map(|o| o.pointer("/option/optionValue").or(o.get("optionValue")).and_then(Value::as_str).map(str::to_string))
            .collect();
        (!now.is_empty() || !options.is_empty()).then_some(Choice { now, options })
    };
    (one("whoCanSeeMyOnlineStatus"), one("whoCanJoinMeInExperiences"))
}

/// The plain user-settings answer: the same two, with the choices every account has.
pub fn visibility_plain(d: &Value) -> (Option<Choice>, Option<Choice>) {
    let one = |key: &str, options: &[&str]| -> Option<Choice> {
        let now = match d.get(key)? {
            Value::String(t) => t.clone(),
            o => o.get("currentValue").and_then(Value::as_str).unwrap_or_default().to_string(),
        };
        Some(Choice { now, options: options.iter().map(|s| s.to_string()).collect() })
    };
    (one("whoCanSeeMyOnlineStatus", &["AllUsers", "Friends", "NoOne"]), one("whoCanJoinMeInExperiences", &["All", "Friends", "NoOne"]))
}

/// What user-settings is sent to set them (None leaves one as it is).
pub fn visibility_body(online: Option<&str>, join: Option<&str>) -> Value {
    let mut body = serde_json::Map::new();
    if let Some(v) = online {
        body.insert("whoCanSeeMyOnlineStatus".into(), (if v == "All" { "AllUsers" } else { v }).into());
    }
    if let Some(v) = join {
        body.insert("whoCanJoinMeInExperiences".into(), (if v == "AllUsers" { "All" } else { v }).into());
    }
    Value::Object(body)
}

impl Choice {
    /// As the core hands it to the window ({ now, options }), and back.
    pub fn json(&self) -> Value {
        serde_json::json!({ "now": self.now, "options": self.options })
    }

    pub fn from_json(v: &Value) -> Option<Choice> {
        let now = v.get("now")?.as_str()?.to_string();
        let options = v.get("options")?.as_array()?.iter().filter_map(Value::as_str).map(str::to_string).collect();
        Some(Choice { now, options })
    }
}

// ───────────────────────── a game's badges ─────────────────────────

/// A game's badge as Roblox's game page lists it: what it is, its picture, and its figures (badges v1's statistics:
/// the share of the game's players who won it, as a fraction, how many won it yesterday and ever).
#[derive(Clone, Debug)]
pub struct GameBadge {
    pub id: i64,
    pub name: String,
    pub about: String,
    pub pic: String,
    pub rate: f64,
    pub yesterday: i64,
    pub ever: i64,
}

/// A game's badges, every page of them, the ones it has switched off left out (Roblox's page doesn't show them). No
/// sign-in needed (badges v1 universes/{id}/badges, checked 2026-11-21).
pub fn badges(place: i64) -> Result<Vec<GameBadge>, String> {
    let universe = int(&get(&format!("https://apis.roblox.com/universes/v1/places/{place}/universe"), None)?, "universeId");
    if universe == 0 {
        return Err("Roblox".into());
    }
    let mut list: Vec<GameBadge> = Vec::new();
    let mut cursor = String::new();
    for _ in 0..10 {
        let mut url = format!("https://badges.roblox.com/v1/universes/{universe}/badges?limit=100&sortOrder=Asc");
        if !cursor.is_empty() {
            url += &format!("&cursor={}", enc(&cursor));
        }
        let d = match get(&url, None) {
            Ok(d) => d,
            Err(e) if list.is_empty() => return Err(e),
            Err(_) => break,
        };
        for v in d.get("data").and_then(Value::as_array).into_iter().flatten() {
            if v.get("enabled").and_then(Value::as_bool) == Some(false) || int(v, "id") <= 0 {
                continue;
            }
            let pick = |a: &str, b: &str| [text(v, a), text(v, b)].into_iter().find(|t| !t.trim().is_empty()).unwrap_or_default().trim().to_string();
            let st = v.get("statistics").cloned().unwrap_or_default();
            list.push(GameBadge {
                id: int(v, "id"),
                name: pick("displayName", "name"),
                about: pick("displayDescription", "description"),
                pic: String::new(),
                rate: st.get("winRatePercentage").and_then(Value::as_f64).unwrap_or(0.),
                yesterday: int(&st, "pastDayAwardedCount"),
                ever: int(&st, "awardedCount"),
            });
        }
        cursor = text(&d, "nextPageCursor");
        if cursor.is_empty() {
            break;
        }
    }
    let ids: Vec<i64> = list.iter().map(|b| b.id).collect();
    let pics = thumbs("badges/icons?size=150x150&badgeIds=", &ids);
    for b in &mut list {
        b.pic = pics.get(&b.id).cloned().unwrap_or_default();
    }
    Ok(list)
}

/// Which of the badges each account has won, and when (seconds; 0 when Roblox said only that it has it). Asked with
/// the signed-in account's sign-in, 100 badges a call (badges v1 users/{id}/badges/awarded-dates, which refuses a call
/// that isn't signed in: "Request Context Failure", checked 2026-11-21); for an account it refuses, or with no sign-in,
/// the inventory's is-owned is asked of its first 40 badges (an open inventory answers it to anyone). An account
/// Roblox answered nothing for is left out: unknown, not "has none".
pub fn badge_owners(ids: &[i64], users: &[i64], cookie: Option<&str>) -> HashMap<i64, HashMap<i64, i64>> {
    let users: Vec<i64> = users.iter().take(40).copied().collect();
    let ids = ids.to_vec();
    let mut out = HashMap::new();
    for group in users.chunks(6) {
        let got = each(group.to_vec(), |user| {
            let dated = cookie.filter(|c| !c.is_empty()).and_then(|ck| {
                let mut won = HashMap::new();
                for chunk in ids.chunks(100) {
                    let list: Vec<String> = chunk.iter().map(i64::to_string).collect();
                    let d = get_once(&format!("https://badges.roblox.com/v1/users/{user}/badges/awarded-dates?badgeIds={}", list.join(",")), Some(ck)).ok()?;
                    for v in d.get("data").and_then(Value::as_array).into_iter().flatten() {
                        won.insert(int(v, "badgeId"), crate::time::iso_secs(&text(v, "awardedDate")).unwrap_or(0));
                    }
                }
                Some(won)
            });
            if let Some(won) = dated {
                return (user, Some(won));
            }
            let asked: Vec<(i64, Option<bool>)> = ids
                .iter()
                .take(40)
                .copied()
                .collect::<Vec<_>>()
                .chunks(8)
                .flat_map(|c| each(c.to_vec(), |id| (id, get_once(&format!("https://inventory.roblox.com/v1/users/{user}/items/Badge/{id}/is-owned"), None).ok().and_then(|v| v.as_bool()))))
                .collect();
            if asked.iter().all(|a| a.1.is_none()) {
                return (user, None);
            }
            (user, Some(asked.into_iter().filter(|a| a.1 == Some(true)).map(|a| (a.0, 0)).collect()))
        });
        for (user, won) in got {
            if let Some(w) = won {
                out.insert(user, w);
            }
        }
    }
    out
}

/// The things a user shows off on their profile (Roblox's Collections, showcases-api: public, checked 2026-11-21), as
/// the Marketplace lists them (its maker, price and id to buy it by come from catalog's items/details), with their
/// pictures at 420 px (the showcase's own are 140).
pub fn collection(user: i64) -> Vec<Ware> {
    let Ok(d) = get(&format!("https://apis.roblox.com/showcases-api/v1/users/profile/robloxcollections-json?userId={user}"), None) else { return Vec::new() };
    let mut list: Vec<Ware> = d
        .as_array()
        .into_iter()
        .flatten()
        .filter(|v| int(v, "id") > 0)
        .take(12)
        .map(|v| Ware {
            id: int(v, "id"),
            bundle: false,
            name: text(v, "name").trim().to_string(),
            maker: String::new(),
            price: None,
            pic: v.pointer("/thumbnail/url").and_then(Value::as_str).unwrap_or_default().to_string(),
            about: text(v, "description").trim().to_string(),
            collectible: String::new(),
        })
        .collect();
    if list.is_empty() {
        return list;
    }
    let ids: Vec<i64> = list.iter().map(|w| w.id).collect();
    let items: Vec<Value> = ids.iter().map(|id| serde_json::json!({ "itemType": "Asset", "id": id })).collect();
    let (details, pics) = both(|| post("https://catalog.roblox.com/v1/catalog/items/details", &serde_json::json!({ "items": items }), None).ok(), || thumbs("assets?size=420x420&assetIds=", &ids));
    for v in details.as_ref().and_then(|d| d.get("data")).and_then(Value::as_array).into_iter().flatten() {
        if let Some(w) = list.iter_mut().find(|w| w.id == int(v, "id")) {
            let status = text(v, "priceStatus").replace(' ', "").to_lowercase();
            w.price = match status.as_str() {
                "free" => Some(0),
                "offsale" | "noresellers" => None,
                _ => Some(if int(v, "lowestPrice") > 0 { int(v, "lowestPrice") } else { int(v, "price") }).filter(|p| *p > 0),
            };
            w.maker = text(v, "creatorName");
            w.collectible = text(v, "collectibleItemId");
        }
    }
    for w in &mut list {
        if let Some(p) = pics.get(&w.id) {
            w.pic = p.clone();
        }
    }
    list
}

/// A resold thing's sales by day, as Roblox's own item page and its chart read them (marketplace-sales v1
/// item/{collectibleItemId}/resale-data: no sign-in, checked 2026-11-21; economy v1's assets/{id}/resale-data, the old
/// one, stopped at early 2025): Roblox's recent average price, and each day's average price and number sold, oldest
/// first (seconds, price, sold).
#[derive(Clone, Debug, Default)]
pub struct Resale {
    pub rap: i64,
    pub days: Vec<(i64, i64, i64)>,
}

/// The sales of a thing on the Marketplace, by its asset id (its id as a collectible is asked of the catalog when the
/// page doesn't have it). Ok(None): it isn't resold (not a Limited, or no sale yet).
pub fn resale(asset: i64, collectible: &str) -> Result<Option<Resale>, String> {
    let id = if collectible.is_empty() {
        let d = post("https://catalog.roblox.com/v1/catalog/items/details", &serde_json::json!({ "items": [{ "itemType": "Asset", "id": asset }] }), None)?;
        let v = d.pointer("/data/0").cloned().unwrap_or_default();
        let limited = v.get("itemRestrictions").and_then(Value::as_array).is_some_and(|l| l.iter().filter_map(Value::as_str).any(|r| r.starts_with("Limited") || r == "Collectible"));
        if !limited {
            return Ok(None);
        }
        text(&v, "collectibleItemId")
    } else {
        collectible.to_string()
    };
    if id.is_empty() {
        return Ok(None);
    }
    let d = match get_once(&format!("https://apis.roblox.com/marketplace-sales/v1/item/{}/resale-data", enc(&id)), None) {
        Ok(d) => d,
        Err(e) if e == "Roblox 400" || e == "Roblox 404" => return Ok(None),
        Err(e) => return Err(e),
    };
    let when = |v: &Value| crate::time::iso_secs(&text(v, "date")).unwrap_or(0);
    let sold: HashMap<i64, i64> = d.get("volumeDataPoints").and_then(Value::as_array).into_iter().flatten().map(|v| (when(v), int(v, "value"))).collect();
    let mut days: Vec<(i64, i64, i64)> = d
        .get("priceDataPoints")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|v| (when(v), int(v, "value")))
        .filter(|(t, p)| *t > 0 && *p > 0)
        .map(|(t, p)| (t, p, sold.get(&t).copied().unwrap_or(0)))
        .collect();
    days.sort_by_key(|d| d.0);
    days.dedup_by_key(|d| d.0);
    let rap = d.get("recentAveragePrice").and_then(Value::as_f64).unwrap_or(0.).round() as i64;
    if days.is_empty() && rap <= 0 {
        return Ok(None);
    }
    Ok(Some(Resale { rap, days }))
}

// ───────────────────────── blocked people ─────────────────────────
// Roblox's user-blocking-api, as its own pages call it (Roblox's friends-common userBlocking service and RoSeal's
// users service, read 2026-11-21): every call answers only a sign-in (401 / 403 XSRF without one).

/// The people the account has blocked (Roblox's Settings › Privacy › Blocked users), with their names and faces.
pub fn blocked(cookie: &str) -> Result<Vec<Friend>, String> {
    #[cfg(feature = "shot")]
    if let Ok(v) = std::env::var("BLOXKEEP_SHOT_BLOCKED") {
        return Ok(dress(v.split(',').filter_map(|x| x.trim().parse().ok()).map(|id| Friend { id, ..Default::default() }).collect()));
    }
    let mut ids: Vec<i64> = Vec::new();
    let mut cursor = String::new();
    for _ in 0..10 {
        let mut url = String::from("https://apis.roblox.com/user-blocking-api/v1/users/get-blocked-users?count=50");
        if !cursor.is_empty() {
            url += &format!("&cursor={}", enc(&cursor));
        }
        let d = get(&url, Some(cookie))?;
        let data = d.get("data").cloned().unwrap_or_default();
        ids.extend(data.get("blockedUserIds").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_i64).filter(|id| *id > 0));
        cursor = text(&data, "cursor");
        if cursor.is_empty() {
            break;
        }
    }
    ids.dedup();
    Ok(dress(ids.into_iter().map(|id| Friend { id, ..Default::default() }).collect()))
}

/// Whether the account has blocked this person.
pub fn is_blocked(cookie: &str, user: i64) -> Result<bool, String> {
    #[cfg(feature = "shot")]
    if let Ok(v) = std::env::var("BLOXKEEP_SHOT_ISBLOCKED") {
        return Ok(v == "1");
    }
    let d = get(&format!("https://apis.roblox.com/user-blocking-api/v1/users/{user}/is-blocked"), Some(cookie))?;
    Ok(d.as_bool().unwrap_or(false))
}

/// Blocks the person, or unblocks them.
pub fn set_blocked(cookie: &str, user: i64, on: bool) -> Result<(), String> {
    post_ok(&format!("https://apis.roblox.com/user-blocking-api/v1/users/{user}/{}", if on { "block-user" } else { "unblock-user" }), &serde_json::json!({}), Some(cookie))
}

// ───────────────────────── communities: search, primary ─────────────────────────
// Roblox's Search › Communities and a community page's Make Primary (groups v1, read 2026-11-21: groups/search is
// public; users/{id}/groups/primary/role is public and answers null without one; user/groups/primary needs a sign-in).

/// A community found by words, as Roblox's search lists it.
#[derive(Clone, Debug, Default)]
pub struct ClubHit {
    pub id: i64,
    pub name: String,
    pub about: String,
    pub members: i64,
    pub pic: String,
    pub verified: bool,
}

/// The communities Roblox finds for these words (its first page of 10, as its search shows them), with their icons.
pub fn club_search(words: &str) -> Result<Vec<ClubHit>, String> {
    let d = get(&format!("https://groups.roblox.com/v1/groups/search?keyword={}&limit=10&prioritizeExactMatch=true", enc(words)), None)?;
    let mut hits: Vec<ClubHit> = d
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|v| ClubHit {
            id: int(v, "id"),
            name: text(v, "name"),
            about: text(v, "description"),
            members: int(v, "memberCount"),
            pic: String::new(),
            verified: v.get("hasVerifiedBadge").and_then(Value::as_bool).unwrap_or(false),
        })
        .filter(|h| h.id > 0)
        .collect();
    let ids: Vec<i64> = hits.iter().map(|h| h.id).collect();
    let pics = thumbs("groups/icons?size=150x150&groupIds=", &ids);
    for h in &mut hits {
        h.pic = pics.get(&h.id).cloned().unwrap_or_default();
    }
    Ok(hits)
}

/// The community this person shows as their primary one (None: none).
pub fn primary_club(user: i64) -> Result<Option<i64>, String> {
    #[cfg(feature = "shot")]
    if let Ok(v) = std::env::var("BLOXKEEP_SHOT_PRIMARY") {
        return Ok(v.parse().ok());
    }
    let d = get(&format!("https://groups.roblox.com/v1/users/{user}/groups/primary/role"), None)?;
    Ok(d.pointer("/group/id").and_then(Value::as_i64).filter(|id| *id > 0))
}

/// Makes this community the account's primary one, or (None) shows none.
pub fn set_primary_club(cookie: &str, group: Option<i64>) -> Result<(), String> {
    match group {
        Some(id) => post_ok("https://groups.roblox.com/v1/user/groups/primary", &serde_json::json!({ "groupId": id }), Some(cookie)),
        None => send_text("DELETE", "https://groups.roblox.com/v1/user/groups/primary", &serde_json::json!({}), Some(cookie)).map(|_| ()),
    }
}

// ───────────────────────── private servers: create, configure ─────────────────────────
// Roblox's Create Private Server box and its Configure page (games v1 and private-servers-api, shapes as RoSeal's
// privateServers.ts calls them, read 2026-11-21; the price call is public, the rest need the owner's sign-in).

/// The game's universe, and what a private server of it costs a month (Robux; 0: free), or None when it sells none.
pub fn ps_price(place: i64) -> Result<(i64, Option<i64>), String> {
    let universe = int(&get(&format!("https://apis.roblox.com/universes/v1/places/{place}/universe"), None)?, "universeId");
    let d = get(&format!("https://apis.roblox.com/private-servers-api/Universe-Private-Server-Settings?universeId={universe}"), None)?;
    let p = d.get("privateServerData").cloned().unwrap_or_default();
    Ok((universe, (p.get("isAvailable").and_then(Value::as_bool) == Some(true)).then(|| int(&p, "price"))))
}

/// Buys a private server of this game for the account: `price` is what was shown, so Roblox refuses if it changed.
pub fn ps_create(cookie: &str, universe: i64, name: &str, price: i64) -> Result<(), String> {
    #[cfg(feature = "shot")]
    if std::env::var("BLOXKEEP_SHOT_PS").is_ok() {
        return Ok(());
    }
    post(
        &format!("https://games.roblox.com/v1/games/vip-servers/{universe}"),
        &serde_json::json!({ "name": name, "expectedPrice": price, "isPurchaseConfirmed": true }),
        Some(cookie),
    )
    .map(|_| ())
}

/// A private server as its Configure page shows it to its owner.
#[derive(Clone, Debug, Default)]
pub struct PsInfo {
    pub active: bool,
    /// Its invite link (empty: none made yet).
    pub link: String,
    /// The subscription renews by itself; when it ends or renews.
    pub renews: bool,
    pub expires: String,
}

fn ps_info(d: &Value) -> PsInfo {
    let sub = d.get("subscription").cloned().unwrap_or_default();
    PsInfo {
        active: d.get("active").and_then(Value::as_bool).unwrap_or(false),
        link: text(d, "link"),
        renews: sub.get("active").and_then(Value::as_bool).unwrap_or(false),
        expires: text(&sub, "expirationDate"),
    }
}

pub fn ps_details(cookie: &str, id: i64) -> Result<PsInfo, String> {
    #[cfg(feature = "shot")]
    if std::env::var("BLOXKEEP_SHOT_PS").is_ok() {
        return Ok(PsInfo { active: true, link: "https://www.roblox.com/share?code=4f1c0e7d2a&type=Server".into(), renews: true, expires: "2026-12-04T10:00:00Z".into() });
    }
    get(&format!("https://games.roblox.com/v1/vip-servers/{id}"), Some(cookie)).map(|d| ps_info(&d))
}

/// Changes a private server: its name, whether it runs, a new invite link (`body` as Roblox takes it).
pub fn ps_update(cookie: &str, id: i64, body: Value) -> Result<PsInfo, String> {
    send("PATCH", &format!("https://games.roblox.com/v1/vip-servers/{id}"), &body, Some(cookie)).map(|d| ps_info(&d))
}

/// Turns a private server's renewal on or off.
pub fn ps_renew(cookie: &str, id: i64, on: bool) -> Result<(), String> {
    send_text("PATCH", &format!("https://games.roblox.com/v1/vip-servers/{id}/subscription"), &serde_json::json!({ "active": on }), Some(cookie)).map(|_| ())
}

// ───────────────────────── trades: make one, counter one ─────────────────────────
// Roblox's trade window (trades v2, shapes as Roblox's OpenAPI gives them through RoZod's tradesv2.ts, read
// 2026-11-21): each side's tradable collectibles, whether the two can trade, and send or counter with the item
// instances each side puts in.

/// One copy of a collectible a person can put in a trade.
#[derive(Clone, Debug, Default)]
pub struct Tradable {
    pub instance: String,
    pub asset: i64,
    pub name: String,
    pub rap: i64,
    pub serial: i64,
    /// On hold (just bought or traded): Roblox doesn't let it be traded yet.
    pub hold: bool,
    pub pic: String,
}

/// What this person can trade, most valuable first (up to 200 copies).
pub fn tradables(cookie: &str, user: i64) -> Result<Vec<Tradable>, String> {
    let mut out: Vec<Tradable> = Vec::new();
    #[cfg(feature = "shot")]
    // BLOXKEEP_SHOT_COMPOSE=them[,pick]: a hidden shot's trade window with that person, both sides made up here.
    if let Ok(v) = std::env::var("BLOXKEEP_SHOT_COMPOSE") {
        let mine = v.split(',').next().and_then(|x| x.parse::<i64>().ok()) != Some(user);
        let made: &[(i64, &str, i64, i64, bool)] = if mine {
            &[(1365767, "Valkyrie Helm", 300161, 412, false), (1031429, "Domino Crown", 1180000, 77, false), (16630147, "Bluesteel Swordpack", 9200, 1630, true), (1191125008, "Dark Lord of SQL", 54000, 9, false)]
        } else {
            &[(21070012, "Dominus Empyreus", 6100000, 18, false), (1029025, "The Classic ROBLOX Fedora", 64000, 2201, false), (20573078, "Shaggy", 39000, 902, false)]
        };
        out = made.iter().map(|&(asset, name, rap, serial, hold)| Tradable { instance: format!("i{asset}"), asset, name: name.into(), rap, serial, hold, pic: String::new() }).collect();
    }
    #[cfg(feature = "shot")]
    let shot = std::env::var("BLOXKEEP_SHOT_COMPOSE").is_ok();
    #[cfg(not(feature = "shot"))]
    let shot = false;
    let mut cursor = String::new();
    while !shot && out.len() < 200 {
        let mut url = format!("https://trades.roblox.com/v2/users/{user}/tradableItems?limit=50&sortOrder=Desc");
        if !cursor.is_empty() {
            url += &format!("&cursor={}", enc(&cursor));
        }
        let d = get(&url, Some(cookie))?;
        for item in d.get("items").and_then(Value::as_array).into_iter().flatten() {
            let asset = item.pointer("/itemTarget/targetId").and_then(Value::as_str).and_then(|t| t.parse().ok()).unwrap_or(0);
            for i in item.get("instances").and_then(Value::as_array).into_iter().flatten() {
                out.push(Tradable {
                    instance: text(i, "collectibleItemInstanceId"),
                    asset,
                    name: [text(i, "itemName"), text(item, "itemName")].into_iter().find(|n| !n.is_empty()).unwrap_or_default(),
                    rap: [int(i, "recentAveragePrice"), int(item, "recentAveragePrice")].into_iter().max().unwrap_or(0),
                    serial: int(i, "serialNumber"),
                    hold: i.get("isOnHold").and_then(Value::as_bool).unwrap_or(false),
                    pic: String::new(),
                });
            }
        }
        cursor = text(&d, "nextPageCursor");
        if cursor.is_empty() {
            break;
        }
    }
    out.retain(|t| !t.instance.is_empty());
    out.sort_by(|a, b| b.rap.cmp(&a.rap));
    let assets: Vec<i64> = out.iter().map(|t| t.asset).filter(|a| *a > 0).collect();
    let pics = thumbs("assets?size=150x150&assetIds=", &assets);
    for t in &mut out {
        t.pic = pics.get(&t.asset).cloned().unwrap_or_default();
    }
    Ok(out)
}

/// Whether the signed-in account and this person can trade (Roblox's reason when not: its own word for it).
pub fn can_trade_with(cookie: &str, user: i64) -> Result<(bool, String), String> {
    #[cfg(feature = "shot")]
    if std::env::var("BLOXKEEP_SHOT_COMPOSE").is_ok() {
        return Ok((true, String::new()));
    }
    let d = get(&format!("https://trades.roblox.com/v2/users/{user}/can-trade-with"), Some(cookie))?;
    Ok((d.get("canTrade").and_then(Value::as_bool).unwrap_or(false), text(&d, "mutualTradeEligibility")))
}

/// Sends a trade (or, with `counter`, counters that one): what the account puts in and what it asks for, each as
/// item instances and Robux. Roblox's new trade's id.
pub fn trade_send(cookie: &str, me: i64, them: i64, give: (Vec<String>, i64), get_back: (Vec<String>, i64), counter: Option<i64>) -> Result<i64, String> {
    let body = serde_json::json!({
        "senderOffer": { "userId": me, "robux": give.1, "collectibleItemInstanceIds": give.0 },
        "recipientOffer": { "userId": them, "robux": get_back.1, "collectibleItemInstanceIds": get_back.0 },
    });
    let url = match counter {
        Some(id) => format!("https://trades.roblox.com/v2/trades/{id}/counter"),
        None => "https://trades.roblox.com/v2/trades/send".to_string(),
    };
    post(&url, &body, Some(cookie)).map(|d| int(&d, "tradeId"))
}
