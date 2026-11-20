// RobloxApi.Login.cs: signing in with a username and password: Roblox's checks on the way (captcha, proof of work, two-step).
use base64::Engine;
use num_bigint::BigUint;
use serde_json::{Value, json};

use super::{
    COOKIE_NAME, HTTP_REQUEST_EXCEPTION, HttpRequest, HttpResponse, JSON_EXCEPTION, RobloxApiException, TASK_CANCELED_EXCEPTION, as_object, errors,
    fail_of, fail_type_name, get_str, header, is_fail_kind, kind_lower, login_refused, normalize_cookie, parse_json, post_with_csrf, raw_text,
    realm, refused, request, send, try_prop,
};
use crate::core::base::{Cancel, Fail, Redacted, Res};
use crate::core::{l, log};

/// A password sign-in's outcome: a cookie (done), a two-step prompt or captcha to answer, or a check this app
/// can't show (`blocked`: what to tell the person; the attempt ends).
#[derive(Clone, Default)]
pub struct LoginResult {
    pub cookie: Option<String>,
    pub two_step: Option<TwoStepPrompt>,
    pub captcha: Option<CaptchaPrompt>,
    pub blocked: Option<String>,
    pub proof_of_work: Option<ProofOfWorkPrompt>,
    pub human_captcha: Option<HumanCaptchaPrompt>,
}

/// `cookie` left out: it is the account.
impl std::fmt::Debug for LoginResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoginResult")
            .field("cookie", &self.cookie.as_ref().map(|_| Redacted))
            .field("two_step", &self.two_step)
            .field("captcha", &self.captcha)
            .field("blocked", &self.blocked)
            .field("proof_of_work", &self.proof_of_work)
            .field("human_captcha", &self.human_captcha)
            .finish()
    }
}

impl LoginResult {
    fn blocked(text: String) -> LoginResult {
        LoginResult { blocked: Some(text), ..Default::default() }
    }
}

/// Roblox's "captchav2", HUMAN Security's press-and-hold check: fetched from apis.roblox.com/v2/captcha with
/// `captcha_challenge_id` and solved in CaptchaForm, which hands back a redemption token.
/// `metadata`: the challenge's own metadata, sent back with the token filled in.
#[derive(Clone, Default)]
pub struct HumanCaptchaPrompt {
    pub realm: String,
    pub login_challenge_id: String,
    pub captcha_challenge_id: String,
    pub metadata: String,
    pub username: String,
    pub password: String,
}

impl std::fmt::Debug for HumanCaptchaPrompt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HumanCaptchaPrompt")
            .field("realm", &self.realm)
            .field("login_challenge_id", &self.login_challenge_id)
            .field("captcha_challenge_id", &self.captcha_challenge_id)
            .field("metadata", &Redacted) // a challenge token: kept out of logs as the password is
            .field("username", &self.username)
            .field("password", &Redacted)
            .finish()
    }
}

/// A proof-of-work check: a puzzle of a few CPU seconds, solved in the background as roblox.com does.
/// `metadata`: the challenge's own metadata (session, request path…); it goes back to Roblox as-is, with the token filled in.
#[derive(Clone, Default)]
pub struct ProofOfWorkPrompt {
    pub realm: String,
    pub login_challenge_id: String,
    pub session_id: String,
    pub metadata: String,
    pub username: String,
    pub password: String,
}

impl std::fmt::Debug for ProofOfWorkPrompt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProofOfWorkPrompt")
            .field("realm", &self.realm)
            .field("login_challenge_id", &self.login_challenge_id)
            .field("session_id", &self.session_id)
            .field("metadata", &Redacted) // a challenge token: kept out of logs as the password is
            .field("username", &self.username)
            .field("password", &Redacted)
            .finish()
    }
}

/// What's needed to finish a two-step sign-in: the user picks where the code comes from and types it.
/// `media_type`: set when the login answer carried "twoStepVerificationData" (a 200, no cookie) instead of a
/// challenge header: the code went that way already (e.g. "Email"), and the sign-in finishes through
/// /v3/users/{id}/two-step-verification/login with the ticket rather than by repeating the login.
#[derive(Clone)]
pub struct TwoStepPrompt {
    pub realm: String,
    pub user_id: i64,
    pub challenge_id: String,
    pub login_challenge_id: String,
    pub username: String,
    pub password: String,
    pub media_type: Option<String>,
    pub action_type: String,
    /// Where this account can get a code (authenticator, email, sms), the one Roblox prefers first.
    pub methods: Vec<String>,
    /// Set once Roblox accepted the code, so a retry after a failed finish doesn't send the spent code again.
    pub verification_token: Option<String>,
}

impl std::fmt::Debug for TwoStepPrompt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TwoStepPrompt")
            .field("realm", &self.realm)
            .field("user_id", &self.user_id)
            .field("challenge_id", &self.challenge_id)
            .field("login_challenge_id", &self.login_challenge_id)
            .field("username", &self.username)
            .field("password", &Redacted)
            .field("media_type", &self.media_type)
            .field("action_type", &self.action_type)
            .field("methods", &self.methods)
            .field("verification_token", &self.verification_token.as_ref().map(|_| Redacted))
            .finish()
    }
}

impl TwoStepPrompt {
    pub fn new(realm: &str, user_id: i64, challenge_id: &str, login_challenge_id: &str, username: &str, password: &str, media_type: Option<String>, action_type: &str) -> TwoStepPrompt {
        TwoStepPrompt {
            realm: realm.into(),
            user_id,
            challenge_id: challenge_id.into(),
            login_challenge_id: login_challenge_id.into(),
            username: username.into(),
            password: password.into(),
            media_type,
            action_type: action_type.into(),
            methods: vec!["authenticator".into(), "email".into()],
            verification_token: None,
        }
    }
}

/// The 2-step methods Bloxkeep can answer: a code typed by the user, checked at challenges/{method}/verify.
const CODE_METHODS: [&str; 3] = ["authenticator", "email", "sms"];

/// A 2-step method's name from either spelling Roblox uses: "Email", or its TwoStepVerificationMediaType number
/// (Email 0, SMS 1, Authenticator 2, RecoveryCode 3, SecurityKey 4, CrossDevice 5, Password 6, Passkey 7).
fn media_name(e: &Value) -> String {
    match e {
        Value::String(s) => s.clone(),
        Value::Number(n) => match n.as_i64().and_then(|n| i32::try_from(n).ok()) {
            Some(n) if n >= 0 && (n as usize) < MEDIA_TYPES.len() => MEDIA_TYPES[n as usize].into(),
            _ => String::new(),
        },
        _ => String::new(),
    }
}

const MEDIA_TYPES: [&str; 8] = ["Email", "SMS", "Authenticator", "RecoveryCode", "SecurityKey", "CrossDevice", "Password", "Passkey"];

/// The path name of a method answered with a typed code, or None for one Bloxkeep can't do (security key…).
fn code_method(media_type: &str) -> Option<&'static str> {
    CODE_METHODS.iter().copied().find(|m| m.eq_ignore_ascii_case(media_type))
}

/// A captcha on a sign-in: Arkose Labs' FunCaptcha (see apis.roblox.com/captcha/v1/metadata); `blob` ties it to this attempt.
/// `challenge_type`: "captcha", or "captchav2" when Roblox asks for one after another check.
/// `metadata`: the challenge's own metadata, sent back with the token filled in.
#[derive(Clone)]
pub struct CaptchaPrompt {
    pub realm: String,
    pub login_challenge_id: String,
    pub unified_captcha_id: String,
    pub blob: String,
    pub action_type: String,
    pub username: String,
    pub password: String,
    pub challenge_type: String,
    pub metadata: String,
}

impl std::fmt::Debug for CaptchaPrompt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CaptchaPrompt")
            .field("realm", &self.realm)
            .field("login_challenge_id", &self.login_challenge_id)
            .field("unified_captcha_id", &self.unified_captcha_id)
            .field("blob", &Redacted)
            .field("action_type", &self.action_type)
            .field("username", &self.username)
            .field("password", &Redacted)
            .field("challenge_type", &self.challenge_type)
            .field("metadata", &Redacted) // a challenge token: kept out of logs as the password is
            .finish()
    }
}

fn cookie_from(res: &HttpResponse) -> Option<String> {
    res.headers_named("Set-Cookie").filter(|v| v.starts_with(&format!("{COOKIE_NAME}="))).map(normalize_cookie).find(|v| !v.is_empty())
}

/// Signs in as the Roblox client does: username and password go only to Roblox's (or VNG's) official auth
/// host over HTTPS and are never stored or logged by the app.
pub fn login_with_password(username: &str, password: &str, realm: &str, ct: &Cancel) -> Res<LoginResult> {
    send_login(username, password, realm, None, ct)
}

/// POST /v2/login, optionally carrying a solved challenge, and sorts out what Roblox answered.
fn send_login(username: &str, password: &str, realm: &str, configure: Option<&dyn Fn(&mut HttpRequest)>, ct: &Cancel) -> Res<LoginResult> {
    let json = json!({ "ctype": "Username", "cvalue": username, "password": password }).to_string();
    let res = post_with_csrf(&format!("{}/v2/login", realm::api(realm, "auth")), "", &json, configure, ct, realm)?;
    if let Some(cookie) = cookie_from(&res)
        && res.is_success()
    {
        return Ok(LoginResult { cookie: Some(cookie), ..Default::default() });
    }

    let step = if configure.is_none() { "login" } else { "login after a check" };
    if let Some(challenge_id) = header(&res, "rblx-challenge-id").filter(|c| !c.is_empty()) {
        return Ok(parse_challenge(
            realm,
            &challenge_id,
            &header(&res, "rblx-challenge-type").unwrap_or_default(),
            decode_metadata(header(&res, "rblx-challenge-metadata").as_deref()).as_ref(),
            username,
            password,
            step,
            Some(&res),
        ));
    }

    let body = res.text();
    if res.is_success() {
        if let Some(two_step) = two_step_from_body(realm, &body, username, password) {
            return Ok(LoginResult { two_step: Some(two_step), ..Default::default() });
        }
        let root = decode_json(&body);
        // Accepted, but Roblox wants an ID check first (done only in its own apps and site).
        if field(root.as_ref(), "identityVerificationLoginTicket").is_some_and(|t| !t.is_empty()) {
            return Ok(LoginResult::blocked(cant_show(realm, "identity verification")));
        }
        if let Some(Value::Object(r)) = &root
            && r.get("isBanned") == Some(&Value::Bool(true))
        {
            return Err(RobloxApiException {
                attempt_over: true,
                ..RobloxApiException::new(l::t("Roblox reports a problem with this account (it may be restricted or banned). Check it on roblox.com."))
            }
            .into());
        }
        return Ok(LoginResult::blocked(unsupported(step, realm, Some(&res), root.as_ref(), None)));
    }
    Err(login_refused(step, res.status, &body))
}

pub(super) fn field(obj: Option<&Value>, name: &str) -> Option<String> {
    match obj {
        Some(Value::Object(o)) => match o.get(name) {
            Some(Value::String(v)) => Some(v.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// A check Roblox's site or app does that Bloxkeep can't (yet); `kind` names it.
fn cant_show(realm: &str, kind: &str) -> String {
    log::note(&format!("Sign-in check from Roblox{} that Bloxkeep can't show: {kind}", if realm == realm::VNG { " VNG" } else { "" }));
    l::f("Roblox asked for a check Bloxkeep can't show yet ({0}). Try again later, or sign in with your phone.", &[&kind])
}

/// A sign-in answer of unknown shape: logs only its layout (step, HTTP status, challenge header names, field
/// names and value kinds, error codes), never a value (password, cookie, ticket, token, name, id). Returns what to
/// tell the person.
fn unsupported(step: &str, realm: &str, res: Option<&HttpResponse>, json: Option<&Value>, r#type: Option<&str>) -> String {
    let headers: Vec<String> = res.map(|r| r.header_names().into_iter().filter(|k| k.to_ascii_lowercase().starts_with("rblx-")).collect()).unwrap_or_default();
    let codes: Vec<String> = match json {
        Some(Value::Object(o)) => match o.get("errors") {
            Some(Value::Array(errors)) => errors
                .iter()
                .map(|e| match e {
                    Value::Object(eo) => eo.get("code").map(raw_text).unwrap_or_else(|| "?".into()),
                    _ => "?".into(),
                })
                .collect(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    };
    log::note(&format!(
        "Sign-in answer not understood{}: step '{step}'{}{}, headers [{}], body {}{}",
        if realm == realm::VNG { " (VNG)" } else { "" },
        res.map(|r| format!(", HTTP {}", r.status)).unwrap_or_default(),
        r#type.map(|t| format!(", type '{}'", safe_name(t))).unwrap_or_default(),
        headers.join(", "),
        json.map(|j| shape(j, 0)).unwrap_or_else(|| "not JSON".into()),
        if codes.is_empty() { String::new() } else { format!(", errors [{}]", codes.join(", ")) }
    ));
    l::t("Roblox asked for a sign-in check Bloxkeep doesn't support yet. Sign in with your phone or the browser instead. The details were saved in error.log (Settings › Backup & data › Open folder).")
}

/// A name from Roblox (a check's type, a field) as it may go to the log: short and plain, else "#".
pub(super) fn safe_name(name: &str) -> String {
    if name.is_empty() {
        return String::new();
    }
    if name.chars().count() <= 40 && !name.chars().all(|c| c.is_ascii_digit()) && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')) {
        name.to_string()
    } else {
        "#".into()
    }
}

/// Checks Roblox's site knows but that only it (or the Roblox app) can show.
const BROWSER_ONLY_CHECKS: [&str; 12] = [
    "securityquestions", "security-questions", "reauthentication", "rostile", "privateaccesstoken", "deviceintegrity",
    "proofofspace", "phoneverification", "emailverification", "biometric", "chef", "turnstile",
];

/// Turns a challenge Roblox put in front of the sign-in into what the user (or the app) has to answer.
/// `res`: the answer the challenge came in, when it came in headers (for the log of an unknown one).
#[allow(clippy::too_many_arguments)]
fn parse_challenge(realm: &str, challenge_id: &str, r#type: &str, meta: Option<&Value>, username: &str, password: &str, step: &str, res: Option<&HttpResponse>) -> LoginResult {
    if let Some(known) = known_challenge(realm, challenge_id, r#type, meta, username, password) {
        // Only the kind of check and the layout of its fields: enough to follow a change on Roblox's side.
        log::note(&format!(
            "Sign-in check from Roblox{}: type '{}', fields {}",
            if realm == realm::VNG { " VNG" } else { "" },
            safe_name(r#type),
            meta.map(|m| shape(m, 0)).unwrap_or_else(|| "none".into())
        ));
        return known;
    }
    // An unknown check, or a known one in a shape that changed: one line with its layout, for a later version.
    LoginResult::blocked(unsupported(step, realm, res, meta, Some(r#type)))
}

fn known_challenge(realm: &str, challenge_id: &str, r#type: &str, meta: Option<&Value>, username: &str, password: &str) -> Option<LoginResult> {
    let text = |name: &str| field(meta, name);
    let is = |kinds: &[&str]| kinds.iter().any(|k| r#type.eq_ignore_ascii_case(k));
    if is(&["twostepverification"])
        && let Some(two_step_id) = text("challengeId").filter(|t| !t.is_empty())
        && let Some(Value::Object(m)) = meta
        && let Some(uid) = m.get("userId")
    {
        let user_id = match uid {
            Value::Number(n) => n.as_i64().unwrap_or(0),
            Value::String(s) => s.trim().parse::<i64>().unwrap_or(0),
            _ => 0,
        };
        if user_id > 0 {
            let action = text("actionType").filter(|a| !a.is_empty()).unwrap_or_else(|| "Login".into());
            return Some(LoginResult {
                two_step: Some(TwoStepPrompt::new(realm, user_id, &two_step_id, challenge_id, username, password, None, &action)),
                ..Default::default()
            });
        }
    }
    if is(&["captcha", "captchav2"])
        && let Some(captcha_id) = text("unifiedCaptchaId").filter(|t| !t.is_empty())
        && let Some(blob) = text("dataExchangeBlob").filter(|t| !t.is_empty())
    {
        return Some(LoginResult {
            captcha: Some(CaptchaPrompt {
                realm: realm.into(),
                login_challenge_id: challenge_id.into(),
                unified_captcha_id: captcha_id,
                blob,
                action_type: text("actionType").unwrap_or_else(|| "Login".into()),
                username: username.into(),
                password: password.into(),
                challenge_type: r#type.into(),
                metadata: meta.map(raw_text).unwrap_or_default(),
            }),
            ..Default::default()
        });
    }
    if is(&["captchav2"])
        && let Some(human_id) = text("challengeId").filter(|t| !t.is_empty())
    {
        return Some(LoginResult {
            human_captcha: Some(HumanCaptchaPrompt {
                realm: realm.into(),
                login_challenge_id: challenge_id.into(),
                captcha_challenge_id: human_id,
                metadata: meta.map(raw_text).unwrap_or_default(),
                username: username.into(),
                password: password.into(),
            }),
            ..Default::default()
        });
    }
    if is(&["proofofwork"])
        && let Some(pow_session) = text("sessionId").filter(|t| !t.is_empty())
    {
        return Some(LoginResult {
            proof_of_work: Some(ProofOfWorkPrompt {
                realm: realm.into(),
                login_challenge_id: challenge_id.into(),
                session_id: pow_session,
                metadata: meta.map(raw_text).unwrap_or_default(),
                username: username.into(),
                password: password.into(),
            }),
            ..Default::default()
        });
    }
    // The account has to turn on 2-step (or an authenticator) before it may sign in: roblox.com sends it to settings.
    if is(&["forcetwostepverification", "forceauthenticator"]) {
        return Some(LoginResult::blocked(l::t(
            "Roblox wants this account to turn on 2-step verification before it signs in. Sign in on roblox.com or in the Roblox app, turn it on there, then try again, or sign in with your phone.",
        )));
    }
    if is(&["blocksession", "denied"]) {
        return Some(LoginResult::blocked(l::t("Roblox blocked this sign-in. Sign in with your phone or the browser instead.")));
    }
    if is(&BROWSER_ONLY_CHECKS) {
        return Some(LoginResult::blocked(l::f(
            "Roblox asked for a check Bloxkeep can't show yet ({0}). Try again later, or sign in with your phone.",
            &[&safe_name(r#type)],
        )));
    }
    None
}

/// 2-step asked in a 200 answer's body ("twoStepVerificationData": { mediaType, ticket }), seen after a solved
/// captcha. None when the body isn't that.
fn two_step_from_body(realm: &str, body: &str, username: &str, password: &str) -> Option<TwoStepPrompt> {
    // A field of another kind than expected (GetString on a number…), or no JSON: not this shape.
    let root: Value = serde_json::from_str(body).ok()?;
    let Value::Object(root) = &root else { return None };
    let Some(Value::Object(data)) = root.get("twoStepVerificationData") else { return None };
    let Some(Value::String(ticket_id)) = data.get("ticket") else { return None };
    if ticket_id.is_empty() {
        return None;
    }
    let Some(Value::Object(user)) = root.get("user") else { return None };
    let user_id = user.get("id")?.as_i64()?;
    let media = data.get("mediaType").map(media_name).filter(|k| !k.is_empty()).unwrap_or_else(|| "Email".into());
    log::note(&format!("Sign-in check from Roblox{}: two-step in the answer, by {}", if realm == realm::VNG { " VNG" } else { "" }, safe_name(&media)));
    Some(TwoStepPrompt::new(realm, user_id, ticket_id, "", username, password, Some(media), "Login"))
}

pub(super) fn decode_metadata(base64: Option<&str>) -> Option<Value> {
    let base64 = base64.filter(|b| !b.is_empty())?;
    // Convert.FromBase64String skips white space.
    let clean: String = base64.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = base64::engine::general_purpose::STANDARD.decode(clean).ok()?;
    serde_json::from_str(&String::from_utf8_lossy(&bytes)).ok()
}

/// The FunCaptcha key Roblox (or VNG) uses for sign-ins, from its public captcha settings.
pub fn get_login_captcha_key(realm: &str, ct: &Cancel) -> Res<String> {
    let req = request("GET", &format!("{}/captcha/v1/metadata", realm::api(realm, "apis")), None, false, realm::GLOBAL);
    let res = send(&req, ct)?;
    let body = res.text();
    if !res.is_success() {
        return Err(refused("Captcha settings", res.status, &body, false));
    }
    let root = parse_json(&body)?;
    if let Some(keys) = try_prop(&root, "funCaptchaPublicKeys")?
        && let Some(key) = try_prop(keys, "ACTION_TYPE_WEB_LOGIN")?
        && let Some(value) = get_str(key)?.filter(|v| !v.is_empty())
    {
        return Ok(value);
    }
    Err(RobloxApiException::new(l::t("Roblox didn't say which captcha to show.")).into())
}

/// Hands Roblox a solved captcha's token and retries the sign-in with it, as roblox.com does; the answer can
/// still be a two-step prompt (or, rarely, another captcha).
pub fn finish_captcha_login(prompt: &CaptchaPrompt, captcha_token: &str, ct: &Cancel) -> Res<LoginResult> {
    let metadata = if prompt.challenge_type == "captcha" {
        json!({ "unifiedCaptchaId": prompt.unified_captcha_id, "captchaToken": captcha_token, "actionType": prompt.action_type }).to_string()
    } else {
        with_field(&prompt.metadata, "captchaToken", captcha_token)?
    };
    continue_login(&prompt.realm, &prompt.login_challenge_id, &prompt.challenge_type, &metadata, &prompt.username, &prompt.password, ct)
}

/// Carries on with the sign-in once the press-and-hold check handed back its redemption token.
pub fn finish_human_captcha_login(prompt: &HumanCaptchaPrompt, redemption_token: &str, ct: &Cancel) -> Res<LoginResult> {
    let metadata = with_field(&prompt.metadata, "redemptionToken", redemption_token)?;
    continue_login(&prompt.realm, &prompt.login_challenge_id, "captchav2", &metadata, &prompt.username, &prompt.password, ct)
}

/// Solves a proof-of-work check as roblox.com does, then carries on with the sign-in.
pub fn finish_proof_of_work_login(prompt: &ProofOfWorkPrompt, ct: &Cancel) -> Res<LoginResult> {
    let redemption_token = proof_of_work_token(&prompt.realm, &prompt.session_id, ct)?;
    let metadata = with_field(&prompt.metadata, "redemptionToken", &redemption_token)?;
    continue_login(&prompt.realm, &prompt.login_challenge_id, "proofofwork", &metadata, &prompt.username, &prompt.password, ct)
}

/// Fetches a proof-of-work puzzle, computes its answer and trades it for the redemption token the check wants.
pub fn proof_of_work_token(realm: &str, session_id: &str, ct: &Cancel) -> Res<String> {
    let service = format!("{}/proof-of-work-service/v1/pow-puzzle", realm::api(realm, "apis"));
    let (puzzle_type, artifacts, body_of_puzzle);
    {
        let req = request("GET", &format!("{service}?sessionID={}", super::escape_data_string(session_id)), None, false, realm::GLOBAL);
        let res = send(&req, ct)?;
        let body = res.text();
        if !res.is_success() {
            return Err(refused("Proof-of-work puzzle", res.status, &body, false));
        }
        let root = parse_json(&body)?;
        puzzle_type = match try_prop(&root, "puzzleType")? {
            None => String::new(),
            Some(Value::String(p)) => p.clone(),
            Some(p) => raw_text(p),
        };
        artifacts = match try_prop(&root, "artifacts")? {
            None => String::new(),
            Some(Value::String(a)) => a.clone(),
            Some(a) => raw_text(a),
        };
        body_of_puzzle = body.clone();
        let fields = as_object(&root)?.keys().cloned().collect::<Vec<_>>().join(", ");
        let art = match decode_json(&artifacts) {
            Some(Value::Object(art)) => format!(", artifacts {}", art.keys().cloned().collect::<Vec<_>>().join(", ")),
            _ => String::new(),
        };
        log::note(&format!("Proof-of-work puzzle: type '{}', fields {fields}{art}", safe_name(&puzzle_type)));
    }

    let solution = solve_puzzle(&puzzle_type, &artifacts, &body_of_puzzle, ct)?;

    let res = post_with_csrf(&service, "", &json!({ "sessionID": session_id, "solution": solution }).to_string(), None, ct, realm)?;
    let body = res.text();
    if !res.is_success() {
        return Err(refused("Proof-of-work answer", res.status, &body, false));
    }
    let root = parse_json(&body)?;
    if try_prop(&root, "answerCorrect")? == Some(&Value::Bool(false)) {
        return Err(RobloxApiException::new(l::t("Roblox didn't accept the sign-in check. Please try again.")).into());
    }
    let redemption_token = match try_prop(&root, "redemptionToken")? {
        Some(r) => get_str(r)?.unwrap_or_default(),
        None => String::new(),
    };
    if redemption_token.is_empty() {
        log::note(&format!("Proof-of-work answer reply fields: {}", as_object(&root)?.keys().cloned().collect::<Vec<_>>().join(", ")));
        return Err(RobloxApiException::new(l::t("Roblox didn't accept the sign-in check. Please try again.")).into());
    }
    Ok(redemption_token)
}

/// A JSON number's digits as written, by its field name (any case), in any of `texts` (each also read as base64), the
/// field's quotes escaped or not: a number too big for f64.
fn raw_digits(texts: &[&str], name: &str) -> Option<String> {
    let pattern = regex::Regex::new(&format!(r#"(?i)\\?"{}\\?"\s*:\s*(\d+)"#, regex::escape(name))).ok()?;
    let decoded: Vec<String> = texts
        .iter()
        .filter_map(|t| base64::engine::general_purpose::STANDARD.decode(t.trim()).ok())
        .filter_map(|b| String::from_utf8(b).ok())
        .collect();
    texts.iter().copied().chain(decoded.iter().map(String::as_str)).find_map(|t| pattern.captures(t).map(|c| c[1].to_string()))
}

/// Roblox's time-lock puzzle: square A modulo N, T times. Inherently serial: a few seconds of one CPU core per
/// sign-in.
/// `raw`: the answer's own text, where a number too big for serde_json's f64 still has all its digits.
fn solve_puzzle(puzzle_type: &str, artifacts: &str, raw: &str, ct: &Cancel) -> Res<String> {
    let number = |obj: &serde_json::Map<String, Value>, name: &str| -> Option<BigUint> {
        let (_, value) = obj.iter().find(|(k, _)| k.eq_ignore_ascii_case(name))?;
        let text = match value {
            Value::String(s) => s.clone(),
            // GetRawText: the digits as Roblox wrote them. serde_json reads a 2048-bit number as an f64 ("1.2e616"),
            // so they are read from the text itself (the artifacts, base64 or not, or the whole answer).
            Value::Number(_) => {
                let parsed = raw_text(value);
                if parsed.bytes().all(|b| b.is_ascii_digit()) { parsed } else { raw_digits(&[artifacts, raw], name).unwrap_or(parsed) }
            }
            _ => return None,
        };
        // NumberStyles.None: digits only.
        if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        BigUint::parse_bytes(text.as_bytes(), 10)
    };
    // Roblox names the type as a number: 1 is the time-lock puzzle (seen Oct 2026, with artifacts N, A, T).
    let parts = if puzzle_type == "1" || puzzle_type.eq_ignore_ascii_case("TimeLockPuzzle") {
        match decode_json(artifacts) {
            Some(Value::Object(art)) => match (number(&art, "N"), number(&art, "A"), number(&art, "T")) {
                (Some(n), Some(x), Some(t)) => Some((n, x, t)),
                _ => None,
            },
            _ => None,
        }
    } else {
        None
    };
    let Some((n, mut x, t)) = parts else {
        let kind = safe_name(puzzle_type);
        let kind = if !kind.is_empty() && kind != "#" { kind } else { "proofofwork".into() };
        return Err(Fail::user(l::f("Roblox asked for a check Bloxkeep can't show yet ({0}). Try again later, or sign in with your phone.", &[&kind])));
    };
    if n == BigUint::ZERO {
        return Err(Fail::other("DivideByZeroException: Attempted to divide by zero."));
    }
    let two = BigUint::from(2u32);
    let mut i = BigUint::ZERO;
    let one = BigUint::from(1u32);
    let mask = BigUint::from(0xFFFu32);
    while i < t {
        if (&i & &mask) == BigUint::ZERO {
            ct.check()?;
        }
        x = x.modpow(&two, &n);
        i += &one;
    }
    Ok(x.to_str_radix(10))
}

/// A JSON value's layout for the log: field names and value kinds (text by length), never values. Strings
/// holding (base64) JSON are opened too; three levels at most; a field name that looks like data is written "#".
pub(super) fn shape(e: &Value, depth: usize) -> String {
    if depth >= 3 && (e.is_object() || e.is_array()) {
        return "…".into();
    }
    match e {
        Value::Object(o) => format!("{{{}}}", o.iter().map(|(k, v)| format!("{}: {}", safe_name(k), shape(v, depth + 1))).collect::<Vec<_>>().join(", ")),
        Value::Array(a) => format!("[{} x {}]", a.len(), if a.is_empty() { "-".into() } else { shape(&a[0], depth + 1) }),
        Value::String(s) => {
            let inner = decode_json(s).or_else(|| {
                let len = s.encode_utf16().count();
                if len > 0 && len % 4 == 0 { decode_metadata(Some(s)) } else { None }
            });
            match inner {
                Some(inner @ (Value::Object(_) | Value::Array(_))) => format!("json{}", shape(&inner, depth + 1)),
                _ => format!("text({})", s.encode_utf16().count()),
            }
        }
        Value::Bool(_) => "bool".into(),
        _ => kind_lower(e),
    }
}

/// The JSON object with one string field set (added or replaced); every other field is kept as it was.
pub(super) fn with_field(json: &str, name: &str, value: &str) -> Res<String> {
    // JsonNode.Parse throws on text that isn't JSON; anything but an object starts a new one.
    let parsed: Value = serde_json::from_str(json).map_err(|e| fail_of(JSON_EXCEPTION, e))?;
    let mut obj = match parsed {
        Value::Object(o) => o,
        _ => serde_json::Map::new(),
    };
    obj.insert(name.to_string(), Value::String(value.to_string()));
    Ok(Value::Object(obj).to_string())
}

pub(super) fn decode_json(json: &str) -> Option<Value> {
    serde_json::from_str(json).ok()
}

/// Tells Roblox a challenge was answered, then retries the sign-in carrying the answer, as roblox.com does.
fn continue_login(realm: &str, challenge_id: &str, r#type: &str, metadata_json: &str, username: &str, password: &str, ct: &Cancel) -> Res<LoginResult> {
    let json = json!({ "challengeId": challenge_id, "challengeType": r#type, "challengeMetadata": metadata_json }).to_string();
    let (mut retry_id, mut retry_type, mut retry_meta) = (challenge_id.to_string(), r#type.to_string(), metadata_json.to_string());
    {
        let res = post_with_csrf(&format!("{}/challenge/v1/continue", realm::api(realm, "apis")), "", &json, None, ct, realm)?;
        if res.is_success() {
            // Roblox answers with the challenge the retried request must carry; use it when it's there.
            let reply = res.text();
            let answer = decode_json(&reply);
            // roblox.com reads challengeType from every reply ("" when no check is left); without it, the answer is new.
            let r = match &answer {
                Some(r @ Value::Object(_)) if field(Some(r), "challengeType").is_some() => r,
                _ => return Ok(LoginResult::blocked(unsupported(&format!("{} continue", safe_name(r#type)), realm, Some(&res), answer.as_ref(), None))),
            };
            let text = |name: &str| field(Some(r), name);
            log::note(&format!(
                "Challenge continue ({}) reply: fields {}, type '{}'",
                safe_name(r#type),
                as_object(r)?.keys().map(|k| safe_name(k)).collect::<Vec<_>>().join(", "),
                safe_name(&text("challengeType").unwrap_or_default())
            ));
            // Roblox can chain checks (e.g. a captcha after proof-of-work): answer the next one before retrying.
            if let Some(next) = text("challengeType").filter(|n| !n.is_empty() && !n.eq_ignore_ascii_case(r#type)) {
                let raw = text("challengeMetadata").unwrap_or_else(|| "{}".into());
                let next_meta = decode_json(&raw).or_else(|| decode_metadata(Some(&raw)));
                let next_id = text("challengeId").filter(|i| !i.is_empty()).unwrap_or_else(|| challenge_id.to_string());
                return Ok(parse_challenge(realm, &next_id, &next, next_meta.as_ref(), username, password, &format!("{} continue", safe_name(r#type)), None));
            }
            if let Some(id) = text("challengeId").filter(|v| !v.is_empty()) {
                retry_id = id;
            }
            if let Some(t) = text("challengeType").filter(|v| !v.is_empty()) {
                retry_type = t;
            }
            if let Some(m) = text("challengeMetadata").filter(|v| !v.is_empty()) {
                retry_meta = m;
            }
        } else {
            // The answered check is spent: this attempt can only start over.
            let body = res.text();
            return Err(login_refused(&format!("the {} check", r#type), res.status, &body));
        }
    }
    let meta = base64::engine::general_purpose::STANDARD.encode(retry_meta.as_bytes());
    let configure = |req: &mut HttpRequest| {
        req.add_header("rblx-challenge-id", &retry_id);
        req.add_header("rblx-challenge-type", &retry_type);
        req.add_header("rblx-challenge-metadata", &meta);
    };
    match send_login(username, password, realm, Some(&configure as &dyn Fn(&mut HttpRequest)), ct) {
        Err(ex @ Fail::Roblox { .. }) => {
            // Its message may carry Roblox's reply.
            log::note(&format!("Sign-in retry after {} refused{}.", r#type, if ex.rate_limited() { " (rate-limited)" } else { "" }));
            Err(ex)
        }
        other => other,
    }
}

fn two_step_url(prompt: &TwoStepPrompt, path: &str) -> String {
    format!("{}/v1/users/{}/{path}", realm::api(&prompt.realm, "twostepverification"), prompt.user_id)
}

/// Learns the account's 2-step methods as roblox.com does (no cookie: the challenge id proves the sign-in).
/// Only typed-code methods are kept, Roblox's preferred first; with none left (security key, passkey, another
/// device's approval) the sign-in can't go on here.
pub fn with_two_step_methods(prompt: &TwoStepPrompt, ct: &Cancel) -> Res<LoginResult> {
    let mut enabled: Vec<String> = Vec::new();
    let mut primary = String::new();
    let url = format!(
        "{}?challengeId={}&actionType={}",
        two_step_url(prompt, "configuration"),
        super::escape_data_string(&prompt.challenge_id),
        super::escape_data_string(&prompt.action_type)
    );
    let req = request("GET", &url, None, true, &prompt.realm);
    match send(&req, ct) {
        Ok(res) => {
            let body = res.text();
            let config = decode_json(&body);
            if res.is_success()
                && let Some(Value::Object(c)) = &config
                && let Some(Value::Array(methods)) = c.get("methods")
            {
                primary = c.get("primaryMediaType").map(media_name).unwrap_or_default();
                enabled = methods
                    .iter()
                    .filter_map(|m| match m {
                        Value::Object(mo) if mo.get("enabled") == Some(&Value::Bool(true)) => mo.get("mediaType").map(media_name),
                        _ => None,
                    })
                    .filter(|m| !m.is_empty())
                    .collect();
                log::note(&format!("Two-step methods: {}, preferred {}", enabled.iter().map(|m| safe_name(m)).collect::<Vec<_>>().join(", "), safe_name(&primary)));
            } else {
                // Not needed to go on: the code screen then offers the usual methods.
                log::note(&format!("Two-step methods unknown: HTTP {}, body {}", res.status, config.as_ref().map(|j| shape(j, 0)).unwrap_or_else(|| "not JSON".into())));
            }
        }
        Err(ex) if (is_fail_kind(&ex, HTTP_REQUEST_EXCEPTION) || is_fail_kind(&ex, TASK_CANCELED_EXCEPTION)) && !ct.is_cancelled() => {
            log::note(&format!("Two-step methods unknown: {}", fail_type_name(&ex)));
        }
        Err(ex) => return Err(ex),
    }
    Ok(pick_two_step_methods(prompt, enabled, &primary))
}

/// `enabled`: the account's enabled methods as Roblox names them; empty when unknown.
fn pick_two_step_methods(prompt: &TwoStepPrompt, mut enabled: Vec<String>, primary: &str) -> LoginResult {
    // Unknown: the method the answer named, then the ones most accounts have (unless it named one without a code).
    if enabled.is_empty() {
        if let Some(sent) = &prompt.media_type {
            enabled.push(sent.clone());
        }
        if prompt.media_type.as_deref().is_none_or(|m| code_method(m).is_some()) {
            enabled.extend(["Authenticator".to_string(), "Email".to_string()]);
        }
    }
    let mut usable: Vec<String> = Vec::new();
    for m in enabled.iter().filter_map(|m| code_method(m)) {
        if !usable.iter().any(|u| u == m) {
            usable.push(m.to_string());
        }
    }
    if usable.is_empty() {
        log::note(&format!("Two-step by {} only: not a typed code.", enabled.iter().map(|m| safe_name(m)).collect::<Vec<_>>().join(", ")));
        return LoginResult::blocked(l::t(
            "This account confirms sign-ins with a security key, a passkey or another device, which Bloxkeep can't do. Sign in with your phone or the browser instead.",
        ));
    }
    // The method the code was already sent by (or Roblox's preferred one) comes first.
    let first = code_method(prompt.media_type.as_deref().unwrap_or("")).or_else(|| code_method(primary));
    if let Some(first) = first
        && let Some(at) = usable.iter().position(|u| u == first)
    {
        let m = usable.remove(at);
        usable.insert(0, m);
    }
    LoginResult { two_step: Some(TwoStepPrompt { methods: usable, ..prompt.clone() }), ..Default::default() }
}

/// Has Roblox send a (new) code by email or SMS; the answer is just whether it was sent.
pub fn send_two_step_code(prompt: &TwoStepPrompt, method: &str, ct: &Cancel) -> Res<()> {
    let body = json!({ "challengeId": prompt.challenge_id, "actionType": prompt.action_type }).to_string();
    let res = post_with_csrf(&two_step_url(prompt, &format!("challenges/{method}/send-code")), "", &body, None, ct, &prompt.realm)?;
    if res.is_success() {
        return Ok(());
    }
    Err(two_step_refused("Two-step code", res.status, &res.text()))
}

/// A twostepverification.roblox.com refusal by its documented codes: 10 wrong code (typed again), 18
/// authenticator code used already, 1 challenge expired (the attempt is over), 5 or 429 too many tries.
fn two_step_refused(what: &str, status: u16, body: &str) -> Fail {
    let (codes, _) = errors(body);
    if status == 429 || codes.contains(&5) {
        return RobloxApiException { rate_limited: true, ..RobloxApiException::new(l::t("Too many sign-in attempts. Wait a few minutes and try again.")) }.into();
    }
    if codes.contains(&10) {
        return Fail::user(l::t("That verification code is wrong or expired."));
    }
    if codes.contains(&18) {
        return Fail::user(l::t("That code was used already. Wait for the next one in the authenticator app."));
    }
    if codes.contains(&1) {
        log::note(&format!("{what}: HTTP {status}, the challenge expired"));
        return RobloxApiException { attempt_over: true, ..RobloxApiException::new(l::t("This sign-in has expired. Sign in again.")) }.into();
    }
    refused(what, status, body, false)
}

/// Verifies the code, then finishes the way 2-step was asked: from the login's answer, through
/// /v3/users/{id}/two-step-verification/login with the ticket; from a challenge header, via /challenge/v1/continue
/// and a repeated login (as roblox.com does), which may end in another check.
pub fn verify_two_step(prompt: &mut TwoStepPrompt, method: &str, code: &str, ct: &Cancel) -> Res<LoginResult> {
    let token = match prompt.verification_token.clone().filter(|t| !t.is_empty()) {
        Some(accepted) => accepted,
        None => {
            let verify_body = json!({ "challengeId": prompt.challenge_id, "actionType": prompt.action_type, "code": code }).to_string();
            let res = post_with_csrf(&two_step_url(prompt, &format!("challenges/{method}/verify")), "", &verify_body, None, ct, &prompt.realm)?;
            let body = res.text();
            if !res.is_success() {
                return Err(two_step_refused("Two-step verification", res.status, &body));
            }
            let answer = decode_json(&body);
            let Some(verified) = field(answer.as_ref(), "verificationToken").filter(|v| !v.is_empty()) else {
                return Err(RobloxApiException {
                    attempt_over: true,
                    ..RobloxApiException::new(unsupported("two-step verify", &prompt.realm, Some(&res), answer.as_ref(), None))
                }
                .into());
            };
            prompt.verification_token = Some(verified.clone());
            verified
        }
    };

    if prompt.media_type.is_none() {
        let meta = json!({ "verificationToken": token, "rememberDevice": false, "challengeId": prompt.challenge_id, "actionType": prompt.action_type }).to_string();
        return continue_login(&prompt.realm, &prompt.login_challenge_id, "twostepverification", &meta, &prompt.username, &prompt.password, ct);
    }

    let finish = json!({ "challengeId": prompt.challenge_id, "verificationToken": token, "rememberDevice": false }).to_string();
    let done = post_with_csrf(
        &format!("{}/v3/users/{}/two-step-verification/login", realm::api(&prompt.realm, "auth"), prompt.user_id),
        "",
        &finish,
        None,
        ct,
        &prompt.realm,
    )?;
    let done_body = done.text();
    if let Some(signed_in) = cookie_from(&done)
        && done.is_success()
    {
        return Ok(LoginResult { cookie: Some(signed_in), ..Default::default() });
    }
    if done.is_success() {
        let answer = decode_json(&done_body);
        return Ok(LoginResult::blocked(if field(answer.as_ref(), "identityVerificationLoginTicket").is_some_and(|t| !t.is_empty()) {
            cant_show(&prompt.realm, "identity verification")
        } else {
            unsupported("two-step finish", &prompt.realm, Some(&done), answer.as_ref(), None)
        }));
    }
    // 400: the ticket (1, 5) or the verification (10) is no longer good; only a new sign-in helps.
    if done.status == 400 {
        log::note(&format!("Sign-in refused at login after two-step: HTTP 400, errors {}", errors(&done_body).1));
        return Err(RobloxApiException { attempt_over: true, ..RobloxApiException::new(l::t("This sign-in has expired. Sign in again.")) }.into());
    }
    Err(login_refused("login after two-step", done.status, &done_body))
}
