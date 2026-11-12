// RobloxApi.Play.cs: what opening Roblox for an account needs: its sign-in ticket, whether it may play a game, its
// private servers, its Robux.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::lookups::get_json;
use super::{
    HTTP_REQUEST_EXCEPTION, Playability, RobloxApiException, errors, escape_data_string, get_i64, get_str, header, is_fail_kind, is_moderated,
    parse_json, post_with_csrf, rate_limited, realm, request, send, try_prop,
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

/// The account's Robux via its own cookie (economy …/v1/users/{id}/currency; 401 without a sign-in, checked on
/// both hosts 2026-10-19). None when Roblox didn't say; 429 fails, so a round stops there.
pub fn get_robux(cookie: &str, realm: &str, user_id: i64, ct: &Cancel) -> Res<Option<i64>> {
    let realm = realm::normalize(Some(realm));
    let req = request("GET", &format!("{}/v1/users/{user_id}/currency", realm::api(realm, "economy")), Some(cookie), false, realm);
    let res = send(&req, ct)?;
    if res.status == 429 {
        return Err(rate_limited());
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

/// One-time ticket that signs the Roblox client in as this account, from the account's own realm. VNG's client
/// redeemed roblox.com tickets at auth.robloxapp.vnggames.com (seen 2026-10-19): one sign-in service, two names.
pub fn get_auth_ticket(cookie: &str, ct: &Cancel, realm: &str) -> Res<String> {
    let realm = realm::normalize(Some(realm));
    let body = get_client_assertion(cookie, ct, realm)?.unwrap_or_else(|| "{}".into());
    let configure = |req: &mut super::HttpRequest| req.add_header("RBXAuthenticationNegotiation", "1");
    let res = post_with_csrf(&format!("{}/v1/authentication-ticket/", realm::api(realm, "auth")), cookie, &body, Some(&configure as &dyn Fn(&mut super::HttpRequest)), ct, realm)?;

    if let Some(ticket) = header(&res, "rbx-authentication-ticket").filter(|t| !t.is_empty()) {
        return Ok(ticket);
    }
    if res.status == 401 {
        return Err(RobloxApiException { status: 401, cookie_invalid: true, ..RobloxApiException::new(l::t("This account was signed out — sign in to it again.")) }.into());
    }
    if res.status == 429 {
        return Err(rate_limited());
    }
    let text = res.text();
    log::note(&format!("Sign-in ticket{}: HTTP {}, errors {}", if realm == realm::VNG { " (VNG)" } else { "" }, res.status, errors(&text).1));
    // A 5xx is Roblox's problem; anything else (a ban, a warning to read, a challenge) won't change by retrying, and
    // roblox.com shows the account why.
    if res.status >= 500 {
        return Err(RobloxApiException {
            status: res.status,
            server_error: true,
            ..RobloxApiException::new(l::f("Couldn't get a sign-in ticket (HTTP {0}). Try again in a moment.", &[&res.status]))
        }
        .into());
    }
    if is_moderated(res.status, &text) {
        return Err(RobloxApiException {
            status: res.status,
            sign_in_refused: true,
            moderated: true,
            ..RobloxApiException::new(l::t("Roblox has a notice on this account to accept first. Open its roblox.com and accept it."))
        }
        .into());
    }
    Err(RobloxApiException {
        status: res.status,
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
            return Err(rate_limited());
        }
        if res.status == 401 {
            return Err(RobloxApiException { status: 401, cookie_invalid: true, ..RobloxApiException::new(l::t("This account was signed out — sign in to it again.")) }.into());
        }
        if !res.is_success() {
            return Err(RobloxApiException {
                status: res.status,
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

/// Turns a "roblox.com/share?code=…&type=Server" link into the place and private-server link code.
pub fn resolve_private_server_share(cookie: &str, share_code: &str, ct: &Cancel) -> Res<(i64, String)> {
    let json = json!({ "linkId": share_code, "linkType": "Server" }).to_string();
    let res = post_with_csrf("https://apis.roblox.com/sharelinks/v1/resolve-link", cookie, &json, None, ct, realm::GLOBAL)?;
    let body = res.text();
    if res.status == 401 {
        return Err(RobloxApiException { status: 401, cookie_invalid: true, ..RobloxApiException::new(l::t("This account was signed out — sign in to it again.")) }.into());
    }
    if !res.is_success() {
        log::note(&format!("Private server link: HTTP {}, errors {}", res.status, errors(&body).1));
        return Err(RobloxApiException {
            status: res.status,
            ..RobloxApiException::new(l::f("Couldn't open the private server link (HTTP {0}). Check the link, or try again in a moment.", &[&res.status]))
        }
        .into());
    }

    let doc = parse_json(&body)?;
    if let Some(data @ Value::Object(_)) = try_prop(&doc, "privateServerInviteData")?
        && let Some(place @ Value::Number(_)) = try_prop(data, "placeId")?
        && let Some(link) = try_prop(data, "linkCode")?
        && let Some(link_code) = get_str(link)?.filter(|l| !l.is_empty())
    {
        return Ok((get_i64(place)?, link_code));
    }
    Err(RobloxApiException::new(l::t("The private server link is invalid or has expired.")).into())
}
