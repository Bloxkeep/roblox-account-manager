use serde_json::{Value, json};

use super::{header, parse_json, realm, refused, request, send, send_with_csrf};
use crate::core::{base::{Cancel, Fail, Res}, l};

const HOST: &str = "https://groups.roblox.com";

fn read(url: &str, cookie: Option<&str>, ct: &Cancel) -> Res<Value> {
    let response = send(&request("GET", url, cookie, false, realm::GLOBAL), ct)?;
    if response.is_success() {
        parse_json(&response.text())
    } else {
        Err(refused("Roblox community", response.status, &response.text(), response.status == 401))
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

pub fn change_group(group: i64, user: i64, cookie: &str, join: bool, ct: &Cancel) -> Res<Value> {
    let mut details = read_group(group, Some((user, cookie)), ct)?;
    if details["state"] == "owner" && !join {
        return Err(Fail::user(l::t("Transfer community ownership before leaving.")));
    }
    if (join && details["state"] != "join") || (!join && details["state"] != "member") {
        return Ok(details);
    }
    let members = format!("{HOST}/v1/groups/{group}/users");
    let response = if join {
        send_with_csrf("POST", &members, cookie, Some("{}"), None, ct, realm::GLOBAL)?
    } else {
        send_with_csrf("DELETE", &format!("{members}/{user}"), cookie, None, None, ct, realm::GLOBAL)?
    };
    if header(&response, "rblx-challenge-id").is_some_and(|id| !id.is_empty()) {
        return Err(Fail::user(l::t("Roblox requires verification to change this membership.")));
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
    Ok(details)
}
