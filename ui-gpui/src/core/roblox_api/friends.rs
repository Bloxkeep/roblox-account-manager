// RobloxApi.Friends.cs: friends between the person's own accounts (Friend each other): where two stand, a request,
// its acceptance.
use std::collections::HashMap;

use serde_json::{Value, json};

use super::login::{decode_json, decode_metadata, safe_name};
use super::{
    HttpRequest, JoinChallenge, RobloxApiException, add_challenge_headers, errors, header, post_with_csrf, rate_limited_by, raw_text, realm, refused,
    request, send,
};
use crate::core::base::{Cancel, Res};
use crate::core::{l, log};

/// Where the signed-in account stands with each of `others`: "Friends", "RequestSent", "RequestReceived" or
/// "NotFriends", by user id (one missing from Roblox's answer is left out).
pub fn friend_statuses(cookie: &str, user_id: i64, others: &[i64], realm: &str, ct: &Cancel) -> Res<HashMap<i64, String>> {
    let mut result = HashMap::new();
    if others.is_empty() {
        return Ok(result);
    }
    // At most 50 ids a call: Roblox has a "too many ids" error with no stated limit (RoALT uses 50, as presence's is).
    let mut distinct: Vec<i64> = Vec::new();
    for id in others {
        if !distinct.contains(id) {
            distinct.push(*id);
        }
    }
    for chunk in distinct.chunks(50) {
        let url = format!(
            "{}/v1/users/{user_id}/friends/statuses?userIds={}",
            realm::api(realm, "friends"),
            chunk.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
        );
        let req = request("GET", &url, Some(cookie), false, realm::GLOBAL);
        let res = send(&req, ct)?;
        let body = res.text();
        if res.status == 429 {
            return Err(rate_limited_by(&res));
        }
        if !res.is_success() {
            return Err(refused("Friend statuses", res.status, &body, res.status == 401));
        }
        if let Some(Value::Object(root)) = decode_json(&body)
            && let Some(Value::Array(data)) = root.get("data")
        {
            for d in data {
                // TryGetProperty on an entry that isn't an object throws in the C#; such an answer never comes.
                let Value::Object(d) = d else { continue };
                if let Some(Value::Number(id)) = d.get("id")
                    && let Some(other) = id.as_i64()
                    && let Some(name) = d.get("status").and_then(status_name)
                {
                    result.insert(other, name);
                }
            }
        }
    }
    Ok(result)
}

/// A friend status as its name: Roblox sends the name, and its numbered form (0–3, the same order) has been seen too.
fn status_name(status: &Value) -> Option<String> {
    match status {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => match n.as_i64() {
            Some(n @ 0..=3) => Some(["NotFriends", "Friends", "RequestSent", "RequestReceived"][n as usize].to_string()),
            _ => None,
        },
        _ => None,
    }
}

/// Roblox's answer to a friend request or an acceptance: HTTP status, its error codes, and the check holding it
/// (rblx-challenge-* headers), if any.
#[derive(Clone, Debug, Default)]
pub struct FriendAnswer {
    pub http: i32,
    pub codes: Vec<i32>,
    pub challenge: Option<JoinChallenge>,
}

impl FriendAnswer {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.http)
    }
}

/// The signed-in account asks `target_user_id` to be friends; `answered` carries a check passed, as roblox.com asks
/// again once its check is solved.
pub fn request_friendship(cookie: &str, target_user_id: i64, realm: &str, answered: Option<&JoinChallenge>, ct: &Cancel) -> Res<FriendAnswer> {
    friend_post(
        &format!("{}/v1/users/{target_user_id}/request-friendship", realm::api(realm, "friends")),
        &json!({ "friendshipOriginSourceType": 0 }).to_string(),
        cookie,
        realm,
        answered,
        "Friend request",
        ct,
    )
}

/// The signed-in account accepts the request `requester_user_id` sent it.
pub fn accept_friend_request(cookie: &str, requester_user_id: i64, realm: &str, answered: Option<&JoinChallenge>, ct: &Cancel) -> Res<FriendAnswer> {
    friend_post(&format!("{}/v1/users/{requester_user_id}/accept-friend-request", realm::api(realm, "friends")), "{}", cookie, realm, answered, "Friend accept", ct)
}

/// The signed-in account turns down the request `requester_user_id` sent it.
pub fn decline_friend_request(cookie: &str, requester_user_id: i64, realm: &str, answered: Option<&JoinChallenge>, ct: &Cancel) -> Res<FriendAnswer> {
    friend_post(&format!("{}/v1/users/{requester_user_id}/decline-friend-request", realm::api(realm, "friends")), "{}", cookie, realm, answered, "Friend decline", ct)
}

/// The signed-in account stops being friends with `target_user_id`.
pub fn unfriend(cookie: &str, target_user_id: i64, realm: &str, answered: Option<&JoinChallenge>, ct: &Cancel) -> Res<FriendAnswer> {
    friend_post(&format!("{}/v1/users/{target_user_id}/unfriend", realm::api(realm, "friends")), "{}", cookie, realm, answered, "Unfriend", ct)
}

/// The signed-in account follows `target_user_id`, or stops (friends.roblox.com's /v1/users/{id}/follow and /unfollow,
/// as Roblox's own API description gives them; a check Roblox holds it for is answered like a friend request's).
pub fn follow_user(cookie: &str, target_user_id: i64, on: bool, realm: &str, answered: Option<&JoinChallenge>, ct: &Cancel) -> Res<FriendAnswer> {
    let (path, what) = if on { ("follow", "Follow") } else { ("unfollow", "Unfollow") };
    friend_post(&format!("{}/v1/users/{target_user_id}/{path}", realm::api(realm, "friends")), "{}", cookie, realm, answered, what, ct)
}

fn friend_post(url: &str, json: &str, cookie: &str, realm: &str, answered: Option<&JoinChallenge>, what: &str, ct: &Cancel) -> Res<FriendAnswer> {
    let configure = |req: &mut HttpRequest| {
        if let Some(answered) = answered {
            add_challenge_headers(req, answered);
        }
    };
    let res = post_with_csrf(url, cookie, json, answered.map(|_| &configure as &dyn Fn(&mut HttpRequest)), ct, realm)?;
    let body = res.text();
    let mut challenge = None;
    if let Some(challenge_id) = header(&res, "rblx-challenge-id").filter(|c| !c.is_empty()) {
        let meta = decode_metadata(header(&res, "rblx-challenge-metadata").as_deref());
        challenge = Some(JoinChallenge {
            id: challenge_id,
            r#type: header(&res, "rblx-challenge-type").unwrap_or_default(),
            metadata_json: meta.as_ref().map(raw_text).unwrap_or_else(|| "{}".into()),
        });
    }
    let (codes, summary) = errors(&body);
    // Codes and Roblox's messages only: never the rest of the reply.
    log::note(&format!(
        "{what}: HTTP {}{}{}",
        res.status,
        challenge.as_ref().map(|c| format!(", held for a check ({})", safe_name(&c.r#type))).unwrap_or_default(),
        if res.is_success() { String::new() } else { format!(", errors {summary}") }
    ));
    if res.status == 401 {
        return Err(RobloxApiException { status: 401, cookie_invalid: true, ..RobloxApiException::new(l::t("This account was signed out — sign in to it again.")) }.into());
    }
    if res.status == 429 && challenge.is_none() {
        return Err(rate_limited_by(&res));
    }
    Ok(FriendAnswer { http: res.status as i32, codes, challenge })
}
