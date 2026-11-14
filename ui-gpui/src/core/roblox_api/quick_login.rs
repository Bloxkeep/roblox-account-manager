// RobloxApi.QuickLogin.cs: Quick Login: signing an account in with a code approved on a device already signed in.
use base64::Engine;
use serde_json::{Value, json};

use super::{
    COOKIE_NAME, QuickLoginCode, QuickLoginStatus, get_prop, get_str, header, is_expected_error, normalize_cookie, parse_assume_utc, parse_json,
    post_with_csrf, rate_limited_by, realm, refused, request, send, try_prop,
};
use crate::core::base::{Cancel, Fail, Res, Stamp};
use crate::core::l;

fn quick_login_base(realm: &str) -> String {
    format!("{}/auth-token-service/v1/login/", realm::api(realm, "apis"))
}

/// Starts a Quick Login code to confirm on a signed-in device. `QuickLoginCode::private_key` proves we created it;
/// keep it in memory only.
pub fn create_quick_login(realm: &str, ct: &Cancel) -> Res<QuickLoginCode> {
    let res = post_with_csrf(&format!("{}create", quick_login_base(realm)), "", "{}", None, ct, realm)?;
    let body = res.text();
    if res.status == 429 {
        return Err(rate_limited_by(&res));
    }
    if !res.is_success() {
        return Err(refused("Quick Login code", res.status, &body, false));
    }
    let root = parse_json(&body)?;
    let code = get_str(get_prop(&root, "code")?)?.unwrap_or_default();
    let key = get_str(get_prop(&root, "privateKey")?)?.unwrap_or_default();
    if code.is_empty() || key.is_empty() {
        return Err(super::RobloxApiException::new(l::t("Roblox didn't return a sign-in code.")).into());
    }
    // Roblox writes the expiry in UTC without a zone; codes last about five minutes.
    let expires = match try_prop(&root, "expirationTime")? {
        Some(e) => get_str(e)?.and_then(|t| parse_assume_utc(&t)),
        None => None,
    }
    .unwrap_or_else(|| Stamp::utc_now().plus_secs(5.0 * 60.0));
    let image = match try_prop(&root, "imagePath")? {
        Some(i) => get_str(i)?,
        None => None,
    };
    Ok(QuickLoginCode { realm: realm.into(), code, private_key: key, expires_utc: expires, image_path: image })
}

/// The QR picture of a Quick Login code as a data: URL (the page may only show images from Roblox's CDN).
pub fn get_quick_login_qr(login: &QuickLoginCode, ct: &Cancel) -> Res<Option<String>> {
    let Some(path) = login.image_path.as_deref().filter(|p| !p.is_empty() && p.starts_with('/')) else { return Ok(None) };
    let req = request("GET", &format!("{}/auth-token-service{path}", realm::api(&login.realm, "apis")), None, false, realm::GLOBAL);
    let res = send(&req, ct)?;
    if !res.is_success() || res.media_type().as_deref() != Some("image/png") {
        return Ok(None);
    }
    Ok(Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(&res.body))))
}

/// Created, UserLinked (someone typed the code; the name is theirs), Validated, Cancelled or Expired.
pub fn get_quick_login_status(login: &QuickLoginCode, ct: &Cancel) -> Res<QuickLoginStatus> {
    let json = json!({ "code": login.code, "privateKey": login.private_key }).to_string();
    let res = post_with_csrf(&format!("{}status", quick_login_base(&login.realm)), "", &json, None, ct, &login.realm)?;
    let body = res.text();
    // An expired or unknown code is answered with CodeInvalid / CodeNotFound.
    if body.contains("\"CodeInvalid\"") || body.contains("\"CodeNotFound\"") {
        return Ok(QuickLoginStatus { status: "Expired".into(), account_name: None, picture_url: None });
    }
    if res.status == 429 {
        return Err(rate_limited_by(&res));
    }
    if !res.is_success() {
        return Err(refused("Quick Login status", res.status, &body, false));
    }
    let root = parse_json(&body)?;
    let text = |name: &str| -> Res<Option<String>> {
        Ok(match try_prop(&root, name)? {
            Some(Value::String(v)) => Some(v.clone()),
            _ => None,
        })
    };
    Ok(QuickLoginStatus { status: text("status")?.unwrap_or_else(|| "Created".into()), account_name: text("accountName")?, picture_url: text("accountPictureUrl")? })
}

/// Trades a confirmed code for the account's session cookie, exactly as roblox.com does after Quick Login.
pub fn finish_quick_login(login: &QuickLoginCode, ct: &Cancel) -> Res<String> {
    let json = json!({ "ctype": "AuthToken", "cvalue": login.code, "password": login.private_key }).to_string();
    let res = post_with_csrf(&format!("{}/v2/login", realm::api(&login.realm, "auth")), "", &json, None, ct, &login.realm)?;
    let body = res.text();
    // The browser sign-in is roblox.com only, so Roblox VNG is sent to the password sign-in.
    if header(&res, "rblx-challenge-id").is_some() {
        return Err(Fail::user(if login.realm == realm::VNG {
            l::t("Roblox wants an extra check for this sign-in. Sign in with the username and password instead.")
        } else {
            l::t("Roblox wants an extra check for this sign-in. Use Sign in with browser instead.")
        }));
    }
    let cookie = res.headers_named("Set-Cookie").filter(|v| v.starts_with(&format!("{COOKIE_NAME}="))).map(normalize_cookie).find(|v| !v.is_empty());
    if res.is_success()
        && let Some(cookie) = cookie
    {
        return Ok(cookie);
    }
    if res.status == 429 {
        return Err(rate_limited_by(&res));
    }
    Err(refused("Quick Login finish", res.status, &body, false))
}

/// Withdraws a code nobody used, so it can't be confirmed after the window closes.
pub fn cancel_quick_login(login: &QuickLoginCode) -> Res<()> {
    match post_with_csrf(&format!("{}cancel", quick_login_base(&login.realm)), "", &json!({ "code": login.code }).to_string(), None, &Cancel::new(), &login.realm) {
        Ok(_) => Ok(()),
        // The code expires on its own a few minutes later.
        Err(ex) if is_expected_error(&ex) => Ok(()),
        Err(ex) => Err(ex),
    }
}
