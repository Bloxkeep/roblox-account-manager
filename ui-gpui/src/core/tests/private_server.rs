// Tests/Bloxkeep.Tests/PrivateServerSuite.cs: your private servers in Browse servers. Each selected account asks Roblox
// (games' /v1/games/{placeId}/private-servers) with its own sign-in, the servers are merged by access code with the
// accounts that can join each, and one picked goes in the game box as roblox://placeId=…&accessCode=…, which GameTarget
// reads and the launcher sends as RequestPrivateGame&accessCode=…. Roblox here is a stand-in (roblox_api::STAND_IN);
// nothing is launched.
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use serde_json::{Value, json};

use super::{
    Checks,
    rejoin::{no_network, quiet_app, seed},
    unlock,
};
use crate::core::{
    base::Cancel,
    game_target::GameTarget,
    models::Account,
    roblox_api::{self, HttpRequest, HttpResponse, unescape_data_string},
    roblox_launcher,
};

const PLACE: i64 = 2753915549;
const SHARED: &str = "11111111-2222-4333-8444-555555555555";
const ONLY_A: &str = "66666666-7777-4888-8999-000000000000";

fn json_answer(status: u16, body: Value) -> HttpResponse {
    HttpResponse { status, headers: vec![("Content-Type".into(), "application/json; charset=utf-8".into())], body: body.to_string().into_bytes() }
}

fn server(code: &str, name: &str, playing: i32) -> Value {
    json!({
        "id": "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee", "maxPlayers": 12, "playing": playing, "playerTokens": [], "players": [],
        "fps": 59.9, "ping": 80, "name": name, "vipServerId": if code == SHARED { 901 } else { 902 }, "accessCode": code,
        "owner": { "id": 31, "name": "alpha_owner", "displayName": "Alpha", "hasVerifiedBadge": false },
    })
}

fn roblox(req: &HttpRequest, asked: &Mutex<Vec<String>>, fail_b: &AtomicBool) -> HttpResponse {
    let cookie = req.headers_named("Cookie").next().map(|c| c.replace(".ROBLOSECURITY=", "")).unwrap_or_default();
    let rest = req.url.split("://").nth(1).unwrap_or("");
    let (host, path_query) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
    let (path, query) = match path_query.find('?') {
        Some(i) => (&path_query[..i], &path_query[i..]),
        None => (path_query, ""),
    };
    if host != "games.roblox.com" || path != format!("/v1/games/{PLACE}/private-servers") {
        return json_answer(404, json!({}));
    }
    asked.lock().unwrap().push(format!("{cookie}{query}"));
    if cookie == "cookie-b" && fail_b.load(Ordering::SeqCst) {
        return json_answer(429, json!({}));
    }
    // Alpha's list comes in two pages; Beta sees only the shared server; a server without an access code is left out.
    match cookie.as_str() {
        "cookie-a" if !query.contains("exclusiveStartKeyCursor") => json_answer(200, json!({ "data": [server(SHARED, "Farm", 3)], "nextPageCursor": "page2" })),
        "cookie-a" => json_answer(
            200,
            json!({ "data": [server(ONLY_A, "Base", 7), { "name": "No code", "playing": 1, "maxPlayers": 5 }], "nextPageCursor": null }),
        ),
        "cookie-b" => json_answer(200, json!({ "data": [server(SHARED, "Farm", 3)], "nextPageCursor": null })),
        _ => json_answer(200, json!({ "data": [] })),
    }
}

/// Share links beyond private servers (RoALT reads them; Bloxkeep refused all but type=Server until 2026-11-14): a friend's
/// invite, a game's page, a "?v=v2" link, each read at Roblox (a stand-in here, in the website's answer shape).
async fn share_links(c: &mut Checks) {
    use crate::core::{game_target::SHARE_V2, roblox_api::play::resolve_share, rt};
    const JOB: &str = "12345678-1234-4234-8234-123456789abc";
    let invite = GameTarget::try_parse("https://www.roblox.com/share?code=abc123&type=ExperienceInvite", "");
    c.check(
        "a friend's invite link: taken, to read at launch (was refused)",
        invite.as_ref().is_ok_and(|t| t.share_code.as_deref() == Some("abc123") && t.share_type.as_deref() == Some("ExperienceInvite")),
        format!("{invite:?}"),
    );
    let server = GameTarget::try_parse("https://www.roblox.com/share?code=xyz&type=Server", "");
    c.check(
        "…a private server's: as before (no type kept)",
        server.as_ref().is_ok_and(|t| t.share_code.as_deref() == Some("xyz") && t.share_type.is_none()),
        format!("{server:?}"),
    );
    let v2 = GameTarget::try_parse("https://www.roblox.com/share?code=v2code&v=v2", "");
    c.check(
        "…a v2 link: read at deeplinks/v2",
        v2.as_ref().is_ok_and(|t| t.share_code.as_deref() == Some("v2code") && t.share_type.as_deref() == Some(SHARE_V2)),
        format!("{v2:?}"),
    );
    let item = GameTarget::try_parse("https://www.roblox.com/share?code=p1&type=AvatarItemDetails", "");
    c.check("…an avatar item's: refused, saying it isn't a game", item.as_ref().is_err_and(|e| e.contains("doesn't lead to a game")), format!("{item:?}"));

    let status = Arc::new(Mutex::new("Valid".to_string()));
    let asked = Arc::new(Mutex::new(Vec::<String>::new()));
    {
        let (status, asked) = (status.clone(), asked.clone());
        *roblox_api::STAND_IN.write().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(move |req: &HttpRequest| {
            let rest = req.url.split("://").nth(1).unwrap_or("");
            let path = rest[rest.find('/').unwrap_or(rest.len())..].split('?').next().unwrap_or("").to_string();
            let body = req.body.as_ref().map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default();
            asked.lock().unwrap().push(format!("{path} {body}"));
            if req.headers_named("x-csrf-token").next().is_none() {
                let mut csrf = json_answer(403, json!({ "errors": [{ "code": 0, "message": "Token Validation Failed" }] }));
                csrf.headers.push(("x-csrf-token".into(), "t".into()));
                return Ok(csrf);
            }
            Ok(match path.as_str() {
                "/sharelinks/v1/resolve-link" if body.contains("ExperienceInvite") => json_answer(
                    200,
                    json!({ "experienceInviteData": { "status": status.lock().unwrap().clone(), "placeId": PLACE, "instanceId": JOB, "inviterId": 5 } }),
                ),
                "/sharelinks/v1/resolve-link" => json_answer(200, json!({ "experienceDetailsInviteData": { "status": "Valid", "placeId": PLACE, "launchData": "event=7" } })),
                "/deeplinks/v2/resolve" => json_answer(200, json!({ "linkType": "Experience", "status": "Valid", "targetId": PLACE, "customData": "from-v2" })),
                _ => json_answer(404, json!({})),
            })
        }));
    }
    let resolve = |code: &'static str, kind: Option<&'static str>| rt::blocking(move || resolve_share("cookie-a", code, kind, &Cancel::new()));
    let r = resolve("abc123", Some("ExperienceInvite")).await;
    c.check(
        "an invite still valid: the friend's server of that game",
        r.as_ref().is_ok_and(|t| t.place_id == PLACE && t.job_id.as_deref() == Some(JOB) && t.link_code.is_none() && !t.expired),
        format!("{r:?}"),
    );
    *status.lock().unwrap() = "Expired".into();
    let r = resolve("abc123", Some("ExperienceInvite")).await;
    c.check(
        "…expired: the game, not that server, and said expired (joined anyway, as the website does)",
        r.as_ref().is_ok_and(|t| t.place_id == PLACE && t.job_id.is_none() && t.expired),
        format!("{r:?}"),
    );
    let r = resolve("d1", Some("ExperienceDetails")).await;
    c.check(
        "a game's link: the game with its launch data",
        r.as_ref().is_ok_and(|t| t.place_id == PLACE && t.job_id.is_none() && t.launch_data.as_deref() == Some("event=7")),
        format!("{r:?}"),
    );
    let r = resolve("v2code", Some(SHARE_V2)).await;
    c.check(
        "a v2 link: its place and custom data",
        r.as_ref().is_ok_and(|t| t.place_id == PLACE && t.launch_data.as_deref() == Some("from-v2"))
            && asked.lock().unwrap().iter().any(|a| a.starts_with("/deeplinks/v2/resolve")),
        format!("{r:?}"),
    );
    let target = GameTarget { launch_data: Some("event=7".into()), ..GameTarget::new(PLACE, None, None, None) };
    let link = unescape_data_string(&roblox_launcher::build_launch_uri("TICKET", Some(&target), None));
    c.check(
        "the launch carries the launch data in a plain join",
        link.contains("request=RequestGame&") && link.contains(&format!("&placeId={PLACE}&launchData=event%3D7&isPlayTogetherGame=false")),
        &link,
    );
}

pub async fn run(c: &mut Checks, _dir: &str) {
    // ── The game box's deep link ──
    let parsed = GameTarget::try_parse(&format!("roblox://placeId={PLACE}&accessCode={}", SHARED.to_uppercase()), "");
    c.check(
        "roblox://placeId=…&accessCode=…: that place, that access code, a private server",
        parsed.as_ref().is_ok_and(|t| t.place_id == PLACE && t.access_code.as_deref() == Some(SHARED) && t.link_code.is_none() && t.is_private()),
        format!("{parsed:?}"),
    );
    let parsed = GameTarget::try_parse(&format!("roblox://placeId={PLACE}&accessCode=not-a-code"), "");
    c.check(
        "…a code that isn't one: refused, saying so",
        parsed.as_ref().is_err_and(|e| e == "Invalid private server access code."),
        format!("{parsed:?}"),
    );
    let parsed = GameTarget::try_parse(&format!("https://www.roblox.com/games/{PLACE}"), "");
    c.check(
        "a plain game link: no access code, not private",
        parsed.as_ref().is_ok_and(|t| t.access_code.is_none() && !t.is_private()),
        format!("{parsed:?}"),
    );

    let private = GameTarget { access_code: Some(SHARED.into()), ..GameTarget::new(PLACE, None, None, None) };
    let link = unescape_data_string(&roblox_launcher::build_launch_uri("TICKET", Some(&private), None));
    c.check(
        "the launch: RequestPrivateGame with the access code, as the website's Join button asks",
        link.contains("request=RequestPrivateGame&")
            && link.contains(&format!("&placeId={PLACE}&"))
            && link.contains(&format!("&accessCode={SHARED}&"))
            && !link.contains("gameId="),
        &link,
    );

    let written = serde_json::to_string(&private).unwrap_or_default();
    let back: Option<GameTarget> = serde_json::from_str(&written).ok();
    c.check(
        "saved with the open and waiting sessions and read back: the access code kept (IsPrivate isn't stored)",
        back.as_ref().is_some_and(|b| b.access_code.as_deref() == Some(SHARED) && b.is_private()) && !written.contains("IsPrivate"),
        &written,
    );

    // ── Asking Roblox ──
    no_network();
    let made = |name: &str, user_id: i64, cookie: &str| Account { username: name.into(), user_id, cookie: cookie.into(), ..Default::default() };
    let (alpha, beta, gamma) = (made("Alpha", 31, "cookie-a"), made("Beta", 32, "cookie-b"), made("Gamma", 33, "cookie-c"));
    let (a, b) = (alpha.id, beta.id);
    seed(vec![alpha, beta, gamma]);
    let (app, _said) = quiet_app();
    unlock(&app).await;

    let asked: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let fail_b = Arc::new(AtomicBool::new(false));
    {
        let (asked, fail_b) = (asked.clone(), fail_b.clone());
        *roblox_api::STAND_IN.write().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(move |req| Ok(roblox(req, &asked, &fail_b))));
    }
    let asked_now = || asked.lock().unwrap().clone();

    let list = crate::core::rt::blocking(|| roblox_api::get_private_servers("cookie-a", PLACE, &Cancel::new())).await;
    let list = list.unwrap_or_default();
    let codes: Vec<&str> = list.iter().map(|s| s.access_code.as_str()).collect();
    c.check(
        "one account's list: both pages read (the next asked with exclusiveStartKeyCursor), the server without a code left out",
        codes == [SHARED, ONLY_A] && asked_now().iter().any(|q| q.ends_with("?exclusiveStartKeyCursor=page2")),
        list.iter().map(|s| s.name.clone()).collect::<Vec<_>>().join(","),
    );
    let first = list.first();
    c.check(
        "…each with its name, owner, players and running server",
        first.is_some_and(|s| {
            s.id == 901
                && s.name == "Farm"
                && s.owner_name == "alpha_owner"
                && s.owner_display_name == "Alpha"
                && s.playing == 3
                && s.max_players == 12
                && s.job_id.as_deref() == Some("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")
        }),
        format!("{first:?}"),
    );

    asked.lock().unwrap().clear();
    let dto = app.private_servers(PLACE, &[a, b], false).await.unwrap_or_else(|e| json!({ "error": e.to_string() }));
    let servers = dto["servers"].as_array().cloned().unwrap_or_default();
    let code = |s: &Value| s["accessCode"].as_str().unwrap_or("").to_string();
    let via = |s: &Value| {
        let mut v: Vec<String> = s["via"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
        v.sort();
        v
    };
    let mut who: Vec<String> = asked_now().iter().map(|q| q.split('?').next().unwrap_or("").to_string()).collect();
    who.sort();
    who.dedup();
    c.check(
        "two selected accounts: each asked with its own sign-in, the third (not selected) not asked",
        who == ["cookie-a", "cookie-b"],
        asked_now().join(","),
    );
    c.check(
        "…the server both can join once, first, saying both can join",
        servers.len() == 2 && code(&servers[0]) == SHARED && via(&servers[0]) == ["Alpha", "Beta"],
        &dto,
    );
    c.check(
        "…the one only Alpha can join after it, saying only Alpha can",
        servers.get(1).is_some_and(|s| code(s) == ONLY_A && via(s) == ["Alpha"]),
        &dto,
    );
    c.check("…owner shown by display name", servers.first().is_some_and(|s| s["owner"] == "Alpha"), &dto);

    fail_b.store(true, Ordering::SeqCst);
    asked.lock().unwrap().clear();
    let _ = app.private_servers(PLACE, &[a, b], false).await;
    c.check("the regular list uses its fresh cache", asked_now().is_empty(), ());
    let dto = app.private_servers(PLACE, &[a, b], true).await.unwrap_or_else(|e| json!({ "error": e.to_string() }));
    c.check("manual refresh asks Roblox again after checkout", asked_now().iter().any(|q| q.starts_with("cookie-b")), asked_now());
    c.check(
        "Roblox refusing one account (429): the other's servers still listed, not marked failed",
        dto["servers"].as_array().is_some_and(|s| s.len() == 2) && dto["failed"] == false,
        &dto,
    );
    let dto = app.private_servers(PLACE, &[b], false).await.unwrap_or_else(|e| json!({ "error": e.to_string() }));
    c.check(
        "…only that account selected: failed, so the page says it couldn't load instead of \"none\"",
        dto["failed"] == true && dto["servers"].as_array().is_some_and(|s| s.is_empty()),
        &dto,
    );

    // The dock: the game box with the deep link is a private server.
    let g = app.resolve_game(&format!("roblox://placeId={PLACE}&accessCode={SHARED}")).await.unwrap_or_else(|e| json!({ "error": e.to_string() }));
    c.check(
        "the game box holding it reads as a private server of that game",
        g["ok"] == true && g["kind"] == "private" && g["placeId"].as_i64() == Some(PLACE),
        &g,
    );
    share_links(c).await;
    no_network();
}
