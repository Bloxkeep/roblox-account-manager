use serde_json::{Value, json};

use super::{Cancel, Fail, Res, realm, request, send};

fn read(service: &str, path: &str, cookie: Option<&str>) -> Res<Value> {
    let url = format!("{}{path}", realm::api(realm::VNG, service));
    let response = send(&request("GET", &url, cookie, true, realm::VNG), &Cancel::new())?;
    if !(200..300).contains(&response.status) {
        return Err(Fail::user(format!("Roblox VNG · HTTP {}", response.status)));
    }
    serde_json::from_slice(&response.body).map_err(|_| Fail::user("Roblox VNG · JSON"))
}

fn list<'a>(data: &'a Value, key: &str) -> Res<&'a [Value]> {
    data[key].as_array().map(Vec::as_slice).ok_or_else(|| Fail::user("Roblox VNG · JSON"))
}

fn words<'a>(data: &'a Value, key: &str) -> &'a str {
    data[key].as_str().unwrap_or_default()
}

fn name(data: &Value) -> &str {
    ["displayName", "name", "username", "Username"].into_iter()
        .map(|key| words(data, key)).find(|s| !s.is_empty()).unwrap_or_default()
}

fn avatar_picture(user: i64) -> Option<String> {
    let data = read("thumbnails", &format!("/v1/users/avatar?userIds={user}&size=420x420&format=Png"), None).ok()?;
    let url = data.pointer("/data/0/imageUrl")?.as_str()?;
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    (parsed.scheme() == "https" && (host == "rbxcdn.com" || host.ends_with(".rbxcdn.com"))).then(|| url.to_owned())
}

fn friends(user: i64, cookie: &str) -> Res<Value> {
    let mut rows = Vec::new();
    let mut cursor = String::new();
    for page in 0..20 {
        let mut path = format!("/v1/users/{user}/friends/find?userSort=1&limit=50");
        if !cursor.is_empty() {
            path.push_str("&cursor=");
            path.push_str(&percent_encoding::utf8_percent_encode(&cursor, percent_encoding::NON_ALPHANUMERIC).to_string());
        }
        let data = match read("friends", &path, Some(cookie)) {
            Ok(data) => data,
            Err(_) if page == 0 => {
                let data = read("friends", &format!("/v1/users/{user}/friends"), Some(cookie))?;
                rows.extend(list(&data, "data")?.iter().map(friend_row));
                break;
            }
            Err(err) => return Err(err),
        };
        rows.extend(list(&data, "PageItems")?.iter().map(friend_row));
        let next = words(&data, "NextCursor");
        if next.is_empty() || next == cursor {
            break;
        }
        cursor = next.to_owned();
    }
    Ok(json!({ "rows": rows }))
}

fn friend_row(data: &Value) -> Value {
    let username = ["username", "name", "Username"].into_iter().map(|key| words(data, key))
        .find(|s| !s.is_empty()).unwrap_or_default();
    let id = ["id", "userId", "UserId", "Id"].into_iter().find_map(|key| data[key].as_i64());
    let title = if name(data).is_empty() { id.map(|id| format!("#{id}")).unwrap_or_default() } else { name(data).to_owned() };
    json!({ "title": title, "detail": if username.is_empty() { String::new() } else { format!("@{username}") } })
}

fn settings(cookie: &str) -> Res<Value> {
    let (email, about) = crate::roblox::both(
        || read("accountsettings", "/v1/email", Some(cookie)),
        || read("accountinformation", "/v1/description", Some(cookie)),
    );
    let (phone, country) = crate::roblox::both(
        || read("accountinformation", "/v1/phone", Some(cookie)),
        || read("accountsettings", "/v1/account/settings/account-country", Some(cookie)),
    );
    if email.is_err() && about.is_err() && phone.is_err() && country.is_err() {
        return email;
    }
    // Return only the fields used by the panel. Phone numbers are represented by verification status.
    Ok(json!({
        "email": email.ok().and_then(|data| data["emailAddress"].as_str().map(str::to_owned)),
        "about": about.ok().and_then(|data| data["description"].as_str().map(str::to_owned)),
        "phoneVerified": phone.ok().and_then(|data| data["isVerified"].as_bool()),
        "country": country.ok().and_then(|data| {
            data.pointer("/value/localizedName").or_else(|| data.pointer("/value/countryName"))
                .and_then(Value::as_str).map(str::to_owned)
        }),
    }))
}

pub(crate) fn load(panel: &str, user: i64, cookie: &str) -> Res<Value> {
    match panel {
        "friends" => friends(user, cookie),
        "profile" => {
            let data = read("users", &format!("/v1/users/{user}"), None)?;
            Ok(json!({ "title": name(&data), "username": words(&data, "name"),
                "detail": words(&data, "description"), "created": words(&data, "created"),
                "image": avatar_picture(user) }))
        }
        "avatar" => {
            let (model, image) = crate::roblox::both(
                || read("avatar", &format!("/v4/avatar/users/{user}"), None),
                || avatar_picture(user),
            );
            let model = model?;
            let rows: Vec<_> = list(&model["avatarModel"], "assets")?.iter()
                .map(|item| json!({ "title": words(item, "name") })).collect();
            Ok(json!({ "image": image, "rows": rows }))
        }
        "messages" => {
            let data = read("privatemessages", "/v1/messages?pageNumber=0&pageSize=30&messageTab=Inbox", Some(cookie))?;
            let rows: Vec<_> = list(&data, "collection")?.iter().map(|message| json!({
                "title": words(message, "subject"), "detail": name(&message["sender"]),
                "body": words(message, "body"), "unread": message["isRead"].as_bool() == Some(false),
            })).collect();
            Ok(json!({ "rows": rows }))
        }
        "notifications" => {
            let data = read("notifications", "/v2/stream-notifications/get-recent?maxRows=20&startIndex=0", Some(cookie))?;
            let rows: Vec<_> = data.as_array().ok_or_else(|| Fail::user("Roblox VNG · JSON"))?.iter()
                .filter_map(|notice| {
                    let content = notice.get("content")?;
                    let state = content["currentState"].as_str().unwrap_or("default");
                    let title = content["states"][state].pointer("/visualItems/textBody/0/label/text")?.as_str()?;
                    Some(json!({ "title": title }))
                }).collect();
            Ok(json!({ "rows": rows }))
        }
        "settings" => settings(cookie),
        _ => Err(Fail::user("Roblox VNG")),
    }
}
