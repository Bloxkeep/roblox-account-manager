use serde_json::{Value, json};

use super::login::{decode_metadata, safe_name};
use super::{HttpRequest, JoinChallenge, add_challenge_headers, header, parse_json, raw_text, realm, refused, request, send, send_with_csrf};
use crate::core::{base::{Cancel, Fail, Res}, l};

const HOST: &str = "https://groups.roblox.com";

/// Reads without a sign-in, kept a minute: a community's page and the account's buttons on it ask the same ones.
static KEPT: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<String, (std::time::Instant, Value)>>> = std::sync::LazyLock::new(Default::default);
const KEEP: std::time::Duration = std::time::Duration::from_secs(60);

fn read(url: &str, cookie: Option<&str>, ct: &Cancel) -> Res<Value> {
    if cookie.is_none()
        && let Some(v) = KEPT.lock().unwrap_or_else(|e| e.into_inner()).get(url).filter(|k| k.0.elapsed() < KEEP).map(|k| k.1.clone())
    {
        return Ok(v);
    }
    let mut tries = 0;
    loop {
        // A 429 (Roblox's rate limit) waits the time out with every read to that host and asks again (net::wait_cool).
        crate::net::wait_cool(url);
        let response = send(&request("GET", url, cookie, false, realm::GLOBAL), ct)?;
        tries += 1;
        if response.status == 429 && tries < crate::net::RATE_TRIES && !ct.is_cancelled() {
            let said = super::lookups::first_number(&response, "x-ratelimit-reset").or_else(|| super::lookups::retry_after_secs(&response));
            crate::net::cooled(url, said.map(|s| std::time::Duration::from_secs(s.max(1) as u64)));
            continue;
        }
        if !response.is_success() {
            return Err(refused("Roblox community", response.status, &response.text(), response.status == 401));
        }
        crate::net::calm(url);
        let v = parse_json(&response.text())?;
        if cookie.is_none() {
            let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
            kept.retain(|_, k| k.0.elapsed() < KEEP);
            kept.insert(url.to_string(), (std::time::Instant::now(), v.clone()));
        }
        return Ok(v);
    }
}

pub fn read_group(group: i64, viewer: Option<(i64, &str)>, ct: &Cancel) -> Res<Value> {
    let details = read(&format!("{HOST}/v1/groups/{group}"), None, ct)?;
    let approval = details["publicEntryAllowed"].as_bool() == Some(false);
    let state = match viewer {
        None => "guest",
        Some((user, _)) if details.pointer("/owner/userId").and_then(Value::as_i64) == Some(user) => "owner",
        Some((user, cookie)) => {
            let memberships = read(&format!("{HOST}/v2/users/{user}/groups/roles"), None, ct)?;
            let rows = memberships["data"].as_array().ok_or_else(|| Fail::user(l::t("Roblox did not return community membership.")))?;
            if rows.iter().any(|row| row.pointer("/group/id").and_then(Value::as_i64) == Some(group)) {
                "member"
            } else if read(&format!("{HOST}/v1/groups/{group}/membership"), Some(cookie), ct)
                .is_ok_and(|member| member["isPendingJoin"].as_bool() == Some(true))
            {
                "pending"
            } else {
                "join"
            }
        }
    };
    Ok(json!({
        "name": details["name"],
        "description": details["description"],
        "members": details["memberCount"],
        "approval": approval,
        "state": state,
    }))
}

/// Joins or leaves; when Roblox holds the change for a check (rblx-challenge-* headers, as it does a friend request),
/// the check comes back to be solved, and `answered` carries it solved when the change is asked again.
pub fn change_group(group: i64, user: i64, cookie: &str, join: bool, answered: Option<&JoinChallenge>, ct: &Cancel) -> Res<(Value, Option<JoinChallenge>)> {
    // (Asked fresh, and nothing kept from before the change: membership is what changes.)
    KEPT.lock().unwrap_or_else(|e| e.into_inner()).clear();
    let mut details = read_group(group, Some((user, cookie)), ct)?;
    KEPT.lock().unwrap_or_else(|e| e.into_inner()).clear();
    if details["state"] == "owner" && !join {
        return Err(Fail::user(l::t("Transfer community ownership before leaving.")));
    }
    if (join && details["state"] != "join") || (!join && details["state"] != "member") {
        return Ok((details, None));
    }
    let members = format!("{HOST}/v1/groups/{group}/users");
    let configure = |req: &mut HttpRequest| {
        if let Some(answered) = answered {
            add_challenge_headers(req, answered);
        }
    };
    let configure = answered.map(|_| &configure as &dyn Fn(&mut HttpRequest));
    let response = if join {
        send_with_csrf("POST", &members, cookie, Some("{}"), configure, ct, realm::GLOBAL)?
    } else {
        send_with_csrf("DELETE", &format!("{members}/{user}"), cookie, None, configure, ct, realm::GLOBAL)?
    };
    if let Some(id) = header(&response, "rblx-challenge-id").filter(|id| !id.is_empty()) {
        let kind = header(&response, "rblx-challenge-type").unwrap_or_default();
        crate::core::log::note(&format!("Community {}: HTTP {}, held for a check ({}){}.", if join { "join" } else { "leave" }, response.status, safe_name(&kind), if answered.is_some() { " again after one was solved" } else { "" }));
        if answered.is_some() {
            return Err(Fail::user(l::t("Roblox requires verification to change this membership.")));
        }
        let meta = decode_metadata(header(&response, "rblx-challenge-metadata").as_deref());
        return Ok((details, Some(JoinChallenge { id, r#type: kind, metadata_json: meta.as_ref().map(raw_text).unwrap_or_else(|| "{}".into()) })));
    }
    if !response.is_success() {
        return Err(refused("Roblox community", response.status, &response.text(), response.status == 401));
    }
    let answer = parse_json(&response.text()).unwrap_or(Value::Null);
    details["state"] = json!(if !join {
        "join"
    } else if answer["isPendingJoin"].as_bool().unwrap_or(details["approval"].as_bool().unwrap_or(false)) {
        "pending"
    } else {
        "member"
    });
    Ok((details, None))
}
