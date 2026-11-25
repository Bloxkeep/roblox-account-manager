// RobloxApi.Play.cs: what opening Roblox for an account needs: its sign-in ticket, whether it may play a game, its
// private servers, its Robux.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::lookups::get_json;
use super::{
    HTTP_REQUEST_EXCEPTION, Playability, RobloxApiException, errors, escape_data_string, get_i64, get_str, header, is_fail_kind, is_moderated,
    parse_json, post_with_csrf, rate_limit_wait, rate_limited_by, realm, request, send, try_prop,
};
use crate::core::base::{Cancel, Res};
use crate::core::{l, log};

/// Since Sep 2026 roblox.com posts a client assertion as the authentication-ticket request's body; the older
/// empty body is the fallback if the endpoint is unavailable.
fn get_client_assertion(cookie: &str, ct: &Cancel, realm: &str) -> Res<Option<String>> {
    let url = format!("{}/v1/client-assertion/", realm::api(realm, "auth"));
    let attempt = || -> Res<Option<String>> {
        {
            let req = request("GET", &url, Some(cookie), true, realm);
            let res = send(&req, ct)?;
            let body = res.text().trim().to_string();
            if res.is_success() && body.starts_with('{') {
                return Ok(Some(body));
            }
            if res.status != 405 {
                return Ok(None);
            }
        }
        let post = post_with_csrf(&url, cookie, "{}", None, ct, realm)?;
        let post_body = post.text().trim().to_string();
        Ok(if post.is_success() && post_body.starts_with('{') { Some(post_body) } else { None })
    };
    match attempt() {
        Err(ex) if is_fail_kind(&ex, HTTP_REQUEST_EXCEPTION) => Ok(None),
        other => other,
    }
}

/// The minutes the signed-in account played on each of the last 7 days, as its own Settings page reads them (Privacy,
/// Screen time): (days ago, minutes), 0 today. Roblox keeps no longer record anyone can read. None when Roblox didn't
/// say (a VNG account, an answer of another shape); 429 fails, so a round stops there.
pub fn get_daily_screen_time(cookie: &str, user_id: i64, ct: &Cancel) -> Res<Option<Vec<(i64, i64)>>> {
    let url = format!("https://apis.roblox.com/parental-controls-api/v1/parental-controls/get-weekly-screentime?userId={user_id}");
    let res = send(&request("GET", &url, Some(cookie), false, realm::GLOBAL), ct)?;
    if res.status == 429 {
        return Err(rate_limited_by(&res));
    }
    if !res.is_success() {
        return Ok(None);
    }
    Ok(read_daily_screen_time(&res.text()))
}

/// {"dailyScreentimes":[{"daysAgo":0,"minutesPlayed":302},…]} as (days ago, minutes); None for anything else.
pub(crate) fn read_daily_screen_time(body: &str) -> Option<Vec<(i64, i64)>> {
    let v: Value = serde_json::from_str(body).ok()?;
    let days = v.get("dailyScreentimes")?.as_array()?;
    Some(days.iter().filter_map(|d| Some((d.get("daysAgo")?.as_i64()?, d.get("minutesPlayed")?.as_i64()?))).filter(|(ago, m)| (0..7).contains(ago) && *m >= 0).collect())
}

/// The account's Robux via its own cookie (economy …/v1/users/{id}/currency; 401 without a sign-in, checked on
/// both hosts 2026-10-19). None when Roblox didn't say; 429 fails, so a round stops there.
pub fn get_robux(cookie: &str, realm: &str, user_id: i64, ct: &Cancel) -> Res<Option<i64>> {
    let realm = realm::normalize(Some(realm));
    let req = request("GET", &format!("{}/v1/users/{user_id}/currency", realm::api(realm, "economy")), Some(cookie), false, realm);
    let res = send(&req, ct)?;
    if res.status == 429 {
        return Err(rate_limited_by(&res));
    }
    if !res.is_success() {
        return Ok(None);
    }
    Ok(read_robux(&res.text()))
}

/// The number in a currency answer, or None for anything else.
pub(crate) fn read_robux(body: &str) -> Option<i64> {
    match serde_json::from_str::<Value>(body).ok()? {
        Value::Object(o) => o.get("robux")?.as_i64().filter(|n| *n >= 0),
        _ => None,
    }
}

const MAX_TICKET_WAITS: i32 = 2;
const MAX_TICKET_WAIT: std::time::Duration = std::time::Duration::from_secs(60);

/// One-time ticket that signs the Roblox client in as this account, from the account's own realm. VNG's client
/// redeemed roblox.com tickets at auth.robloxapp.vnggames.com (seen 2026-10-19): one sign-in service, two names.
pub fn get_auth_ticket(cookie: &str, ct: &Cancel, realm: &str) -> Res<String> {
    let realm = realm::normalize(Some(realm));
    let assertion = get_client_assertion(cookie, ct, realm)?;
    // Said in error.log (a user's report, 2026-11-14: tickets taken, then refused by Roblox's client at sign-in, 403).
    if assertion.is_none() {
        log::note("Sign-in ticket: Roblox gave no client assertion; the ticket was asked with an empty body.");
    }
    let body = assertion.unwrap_or_else(|| "{}".into());
    // The ticket is bound to the address that asks for it, and Roblox's client redeems it without Windows' proxy: asked
    // the client's way. Only when Roblox can't be reached that way, through the proxy as before (said in error.log).
    let proxy = crate::net::proxy_in_use();
    let mut like_client = true;
    let configure_client = |req: &mut super::HttpRequest| {
        req.add_header("RBXAuthenticationNegotiation", "1");
        req.like_roblox_client = true;
    };
    let configure_proxy = |req: &mut super::HttpRequest| req.add_header("RBXAuthenticationNegotiation", "1");
    let url = format!("{}/v1/authentication-ticket/", realm::api(realm, "auth"));
    // Rate-limited: waited out as Roblox says (its reset, up to a minute) and asked again, twice at most; a launch of many
    // accounts lost the ones refused instead (RoALT waits and asks again too).
    let mut attempt = 0;
    let res = loop {
        let configure: &dyn Fn(&mut super::HttpRequest) = if like_client { &configure_client } else { &configure_proxy };
        let res = match post_with_csrf(&url, cookie, &body, Some(configure), ct, realm) {
            Err(e) if like_client && !ct.is_cancelled() && proxy.is_some() && (is_fail_kind(&e, HTTP_REQUEST_EXCEPTION) || is_fail_kind(&e, super::TASK_CANCELED_EXCEPTION)) => {
                log::note(&format!(
                    "Sign-in ticket: Roblox not reached without Windows' proxy ({}); asked through it ({}) instead, which Roblox's client may refuse (it doesn't use that proxy).",
                    super::fail_type_name(&e),
                    proxy.as_deref().unwrap_or_default()
                ));
                like_client = false;
                continue;
            }
            r => r?,
        };
        if attempt == 0 && like_client && let Some(p) = &proxy {
            log::note(&format!("Sign-in ticket: asked without Windows' proxy ({p}), as Roblox's client signs in."));
        }
        if res.status == 429 && attempt < MAX_TICKET_WAITS {
            let wait = rate_limit_wait(&res);
            if wait <= MAX_TICKET_WAIT {
                log::note(&format!("Sign-in ticket: rate-limited, asked again in {} s.", wait.as_secs()));
                if !ct.sleep(wait) {
                    ct.check()?;
                }
                attempt += 1;
                continue;
            }
        }
        break res;
    };

    if let Some(ticket) = header(&res, "rbx-authentication-ticket").filter(|t| !t.is_empty()) {
        // (Which way it went, once a run per way: a 403 at the client's redeem is read against it.)
        if like_client && let Some(at) = super::lock(&super::LAST_CLIENT_WAY).take() {
            static SAID: std::sync::Mutex<Vec<bool>> = std::sync::Mutex::new(Vec::new());
            let v6 = at.is_ipv6();
            let mut said = super::lock(&SAID);
            if !said.contains(&v6) {
                said.push(v6);
                log::note(&format!("Sign-in ticket: asked over {} at {}.", if v6 { "IPv6" } else { "IPv4" }, at.ip()));
            }
        }
        return Ok(ticket);
    }
    if res.status == 401 {
        return Err(RobloxApiException { cookie_invalid: true, ..RobloxApiException::new(l::t("This account was signed out — sign in to it again.")) }.into());
    }
    if res.status == 429 {
        return Err(rate_limited_by(&res));
    }
    let text = res.text();
    log::note(&format!("Sign-in ticket{}: HTTP {}, errors {}", if realm == realm::VNG { " (VNG)" } else { "" }, res.status, errors(&text).1));
    // A 5xx is Roblox's problem; anything else (a ban, a warning to read, a challenge) won't change by retrying, and
    // roblox.com shows the account why.
    if res.status >= 500 {
        return Err(RobloxApiException {
            server_error: true,
            ..RobloxApiException::new(l::f("Couldn't get a sign-in ticket (HTTP {0}). Try again in a moment.", &[&res.status]))
        }
        .into());
    }
    if is_moderated(res.status, &text) {
        return Err(RobloxApiException {
            sign_in_refused: true,
            moderated: true,
            ..RobloxApiException::new(l::t("Roblox has a notice on this account to accept first. Open its roblox.com and accept it."))
        }
        .into());
    }
    Err(RobloxApiException {
        sign_in_refused: true,
        ..RobloxApiException::new(l::f("Roblox refused to sign this account in (HTTP {0}). Open its roblox.com to see why.", &[&res.status]))
    }
    .into())
}

/// Whether the account may play the game (universe), as Roblox's client asks before joining
/// (multiget-playability-status). None when Roblox gave no answer for that game.
pub fn get_playability(cookie: &str, realm: &str, universe_id: i64, ct: &Cancel) -> Res<Option<Playability>> {
    let realm = realm::normalize(Some(realm));
    let doc = get_json(&format!("{}/v1/games/multiget-playability-status?universeIds={universe_id}", realm::api(realm, "games")), ct, Some(cookie))?;
    let Some(Value::Array(items)) = doc else { return Ok(None) };
    for e in &items {
        if !e.is_object() {
            continue;
        }
        if let Some(id @ Value::Number(_)) = try_prop(e, "universeId")?
            && get_i64(id)? != universe_id
        {
            continue;
        }
        // The documented number, or the name Roblox's live answer gives (seen 2026-10-21: "GuestProhibited").
        let status = match try_prop(e, "playabilityStatus")? {
            Some(Value::Number(n)) => n.as_i64().and_then(|n| i32::try_from(n).ok()),
            Some(s @ Value::String(_)) => Playability::status_of(get_str(s)?.as_deref()),
            _ => None,
        };
        let playable = match try_prop(e, "isPlayable")? {
            Some(p) => *p == Value::Bool(true),
            None => status == Some(1),
        };
        let text = match try_prop(e, "unplayableDisplayText")? {
            Some(Value::String(said)) if !said.is_empty() => Some(said.trim().to_string()),
            _ => None,
        };
        return Ok(Some(Playability { playable, status, text }));
    }
    Ok(None)
}

/// A private server the account owns or was invited to. The access code is what the website's Join button
/// sends (RequestPrivateGame&accessCode=…); job_id is its running instance, if any.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct PrivateServer {
    pub id: i64,
    pub name: String,
    pub owner_id: i64,
    pub owner_name: String,
    pub owner_display_name: String,
    pub access_code: String,
    pub job_id: Option<String>,
    pub playing: i32,
    pub max_players: i32,
}

/// The game's private servers this account can join (GET /v1/games/{placeId}/private-servers; nextPageCursor is
/// sent back as exclusiveStartKeyCursor). At most three pages; 429 and a lost sign-in fail.
pub fn get_private_servers(cookie: &str, place_id: i64, ct: &Cancel) -> Res<Vec<PrivateServer>> {
    let mut list = Vec::new();
    let mut cursor: Option<String> = None;
    for _page in 0..3 {
        let url = format!(
            "https://games.roblox.com/v1/games/{place_id}/private-servers{}",
            cursor.as_deref().map(|c| format!("?exclusiveStartKeyCursor={}", escape_data_string(c))).unwrap_or_default()
        );
        let req = request("GET", &url, Some(cookie), false, realm::GLOBAL);
        let res = send(&req, ct)?;
        if res.status == 429 {
            return Err(rate_limited_by(&res));
        }
        if res.status == 401 {
            return Err(RobloxApiException { cookie_invalid: true, ..RobloxApiException::new(l::t("This account was signed out — sign in to it again.")) }.into());
        }
        if !res.is_success() {
            return Err(RobloxApiException {
                server_error: res.status >= 500,
                ..RobloxApiException::new(l::f("Roblox didn't answer (HTTP {0}).", &[&res.status]))
            }
            .into());
        }
        let (page, next) = read_private_servers(&res.text());
        list.extend(page);
        cursor = next;
        if cursor.as_deref().is_none_or(str::is_empty) {
            break;
        }
    }
    Ok(list)
}

/// The servers in one page of that answer (those without an access code can't be joined and are left out), and the
/// next page's cursor.
pub(crate) fn read_private_servers(body: &str) -> (Vec<PrivateServer>, Option<String>) {
    let mut next = None;
    let mut list = Vec::new();
    let Ok(Value::Object(root)) = serde_json::from_str::<Value>(body) else { return (list, next) };
    if let Some(Value::String(n)) = root.get("nextPageCursor") {
        next = Some(n.clone());
    }
    let Some(Value::Array(data)) = root.get("data") else { return (list, next) };
    fn text(e: &serde_json::Map<String, Value>, name: &str) -> String {
        match e.get(name) {
            Some(Value::String(v)) => v.clone(),
            _ => String::new(),
        }
    }
    fn number(e: &serde_json::Map<String, Value>, name: &str) -> i64 {
        e.get(name).and_then(Value::as_i64).unwrap_or(0)
    }
    for s in data {
        let Value::Object(s) = s else { continue };
        let code = text(s, "accessCode");
        if code.is_empty() {
            continue;
        }
        let owner = match s.get("owner") {
            Some(Value::Object(o)) => Some(o),
            _ => None,
        };
        let job = text(s, "id");
        list.push(PrivateServer {
            id: number(s, "vipServerId"),
            name: text(s, "name"),
            owner_id: owner.map(|o| number(o, "id")).unwrap_or(0),
            owner_name: owner.map(|o| text(o, "name")).unwrap_or_default(),
            owner_display_name: owner.map(|o| text(o, "displayName")).unwrap_or_default(),
            access_code: code,
            job_id: if job.is_empty() { None } else { Some(job) },
            playing: number(s, "playing") as i32,
            max_players: number(s, "maxPlayers") as i32,
        });
    }
    (list, next)
}

/// A private server an account owns, as Roblox lists them under Inventory › Private Servers (its MyPrivateServersData:
/// create.roblox.com's "Private servers" reference). `expires` is Roblox's date text, "" when it gives none.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OwnedServer {
    pub id: i64,
    pub name: String,
    pub place_id: i64,
    pub universe_id: i64,
    pub game: String,
    pub owner_id: i64,
    pub active: bool,
    pub price: i64,
    pub expires: String,
    pub renews: bool,
}

/// One page of that answer and the next page's cursor.
pub(crate) fn read_owned_servers(body: &str) -> (Vec<OwnedServer>, Option<String>) {
    let Ok(Value::Object(root)) = serde_json::from_str::<Value>(body) else { return (Vec::new(), None) };
    let next = root.get("nextPageCursor").and_then(Value::as_str).filter(|n| !n.is_empty()).map(str::to_string);
    let list = root
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|s| {
            let num = |k: &str| s.get(k).and_then(Value::as_i64).unwrap_or(0);
            let text = |k: &str| s.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
            let yes = |k: &str| s.get(k).and_then(Value::as_bool).unwrap_or(false);
            let id = num("privateServerId");
            (id > 0).then(|| OwnedServer {
                id,
                name: text("name"),
                place_id: num("placeId"),
                universe_id: num("universeId"),
                game: text("universeName"),
                owner_id: num("ownerId"),
                active: yes("active"),
                price: num("priceInRobux"),
                expires: text("expirationDate"),
                renews: yes("willRenew"),
            })
        })
        .collect();
    (list, next)
}

/// The private servers this account owns (GET /v1/private-servers/my-private-servers, the "MyPrivateServers" tab), 25
/// a page as roblox.com asks, at most 12 pages; 429 and a lost sign-in fail.
pub fn get_owned_servers(cookie: &str, ct: &Cancel) -> Res<Vec<OwnedServer>> {
    let mut list = Vec::new();
    let mut cursor: Option<String> = None;
    for _page in 0..12 {
        let url = format!(
            "https://games.roblox.com/v1/private-servers/my-private-servers?privateServersTab=MyPrivateServers&itemsPerPage=25{}",
            cursor.as_deref().map(|c| format!("&cursor={}", escape_data_string(c))).unwrap_or_default()
        );
        let req = request("GET", &url, Some(cookie), false, realm::GLOBAL);
        let res = send(&req, ct)?;
        if res.status == 429 {
            return Err(rate_limited_by(&res));
        }
        if res.status == 401 {
            return Err(RobloxApiException { cookie_invalid: true, ..RobloxApiException::new(l::t("This account was signed out — sign in to it again.")) }.into());
        }
        if !res.is_success() {
            return Err(RobloxApiException {
                server_error: res.status >= 500,
                ..RobloxApiException::new(l::f("Roblox didn't answer (HTTP {0}).", &[&res.status]))
            }
            .into());
        }
        let (page, next) = read_owned_servers(&res.text());
        list.extend(page);
        cursor = next;
        if cursor.is_none() {
            break;
        }
    }
    Ok(list)
}

/// Where a share link leads: the place, a private server's link code, an invite's server (while the invite is valid), a
/// link's launch data. Expired links still join, as on the website.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShareTarget {
    pub place_id: i64,
    pub link_code: Option<String>,
    pub job_id: Option<String>,
    pub launch_data: Option<String>,
    pub expired: bool,
}

/// Each share-link type's part of resolve-link's answer (the website's names; one is filled, by type).
const SHARE_FIELDS: [(&str, &str); 7] = [
    ("Server", "privateServerInviteData"),
    ("ExperienceInvite", "experienceInviteData"),
    ("NotificationExperienceInvite", "notificationExperienceInviteData"),
    ("ScreenshotInvite", "screenshotInviteData"),
    ("ExperienceDetails", "experienceDetailsInviteData"),
    ("ExperienceAffiliate", "experienceAffiliateData"),
    ("ExperienceEvent", "experienceEventData"),
];

/// Roblox's limit on a link's launch data; longer is left out.
const MAX_LAUNCH_DATA: usize = 200;

/// Reads a roblox.com share link with the account's sign-in: a private server's (type=Server, the only kind until
/// 2026-11-14), a friend's invite, a game's, an event's or a screenshot's (sharelinks/v1/resolve-link), or a "?v=v2" one
/// (deeplinks/v2/resolve). RoALT reads them the same way.
pub fn resolve_share(cookie: &str, share_code: &str, share_type: Option<&str>, ct: &Cancel) -> Res<ShareTarget> {
    let v2 = share_type == Some(crate::core::game_target::SHARE_V2);
    let kind = share_type.unwrap_or("Server");
    let json = if v2 { json!({ "linkId": share_code }) } else { json!({ "linkId": share_code, "linkType": kind }) }.to_string();
    let url = if v2 { "https://apis.roblox.com/deeplinks/v2/resolve" } else { "https://apis.roblox.com/sharelinks/v1/resolve-link" };
    let res = post_with_csrf(url, cookie, &json, None, ct, realm::GLOBAL)?;
    let body = res.text();
    if res.status == 401 {
        return Err(RobloxApiException { cookie_invalid: true, ..RobloxApiException::new(l::t("This account was signed out — sign in to it again.")) }.into());
    }
    if res.status == 429 {
        return Err(rate_limited_by(&res));
    }
    if !res.is_success() {
        log::note(&format!("Share link ({kind}): HTTP {}, errors {}", res.status, errors(&body).1));
        return Err(RobloxApiException {
            ..RobloxApiException::new(l::f("Couldn't open the share link (HTTP {0}). Check the link, or try again in a moment.", &[&res.status]))
        }
        .into());
    }
    let invalid = || -> crate::core::base::Fail { RobloxApiException::new(l::t("The share link is invalid or has expired.")).into() };
    let is = |status: &Option<String>, what: &str| status.as_deref().is_some_and(|s| s.eq_ignore_ascii_case(what));
    let root = parse_json(&body)?;
    if !root.is_object() {
        return Err(invalid());
    }
    if v2 {
        let status = share_text(&root, "status");
        let Some(target) = share_num(&root, "targetId").filter(|_| !is(&status, "Invalid") && share_text(&root, "linkType").as_deref() == Some("Experience"))
        else {
            return Err(invalid());
        };
        let custom = root.get("customData").and_then(Value::as_str).map(str::to_string);
        return Ok(ShareTarget { place_id: target, launch_data: launch_data_of(custom), expired: is(&status, "Expired"), ..Default::default() });
    }
    for (kind, field) in SHARE_FIELDS {
        let Some(d) = root.get(field).filter(|d| d.is_object()) else { continue };
        let status = share_text(d, "status");
        if is(&status, "Invalid") {
            return Err(invalid());
        }
        let place = match share_num(d, "placeId") {
            Some(p) => p,
            None => match share_num(d, "universeId") {
                Some(universe) => super::lookups::get_universe_details(&[universe], ct)?.get(&universe).map(|d| d.2).unwrap_or(0),
                None => 0,
            },
        };
        if place <= 0 {
            return Err(invalid());
        }
        let expired = is(&status, "Expired");
        if kind == "Server" {
            return match share_text(d, "linkCode") {
                Some(link_code) => Ok(ShareTarget { place_id: place, link_code: Some(link_code), expired, ..Default::default() }),
                None => Err(invalid()),
            };
        }
        // An invite joins the friend's server only while it is valid, as the website does; else the game.
        if kind.ends_with("ExperienceInvite")
            && (status.is_none() || is(&status, "Valid"))
            && let Some(job) = share_text(d, "instanceId").and_then(|j| crate::core::game_target::guid_try_parse(&j))
        {
            return Ok(ShareTarget { place_id: place, job_id: Some(job), expired, ..Default::default() });
        }
        let launch = share_text(d, "launchData").or_else(|| d.get("joinData").filter(|j| j.is_object()).and_then(|j| share_text(j, "launchData")));
        return Ok(ShareTarget { place_id: place, launch_data: launch_data_of(launch), expired, ..Default::default() });
    }
    Err(invalid())
}

fn share_text(e: &Value, name: &str) -> Option<String> {
    e.get(name).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string)
}

fn share_num(e: &Value, name: &str) -> Option<i64> {
    e.get(name).filter(|v| v.is_number()).and_then(Value::as_i64).filter(|n| *n > 0)
}

fn launch_data_of(data: Option<String>) -> Option<String> {
    match data {
        Some(d) if d.chars().count() > MAX_LAUNCH_DATA => {
            log::note(&format!("Share link: its launch data ({} characters) is over Roblox's {MAX_LAUNCH_DATA} and was left out.", d.chars().count()));
            None
        }
        d => d,
    }
}
