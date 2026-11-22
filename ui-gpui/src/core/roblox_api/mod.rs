// RobloxApi.cs: the Roblox web layer (one partial static class over six files: this one, login.rs, lookups.rs, play.rs,
// quick_login.rs, friends.rs), with the records it hands out, Realm and the shape of RobloxApiException.
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::{LazyLock, Mutex, MutexGuard, OnceLock, RwLock};
use std::time::Duration;

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::core::base::{Cancel, Fail, Res, Stamp, new_guid};
use crate::core::{l, log};

pub mod friends;
pub(crate) mod community;
pub mod login;
pub mod lookups;
pub mod play;
pub mod quick_login;
pub(crate) mod vng_panel;

pub use friends::*;
pub use login::*;
pub use lookups::*;
pub use play::*;
pub use quick_login::*;

use login::{decode_json, decode_metadata, field, safe_name, with_field};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct RobloxUser {
    pub id: i64,
    pub name: String,
    pub display_name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct GameInfo {
    pub place_id: i64,
    pub universe_id: i64,
    pub name: String,
    pub creator: String,
    pub playing: i64,
    pub icon_url: Option<String>,
}

/// A game's page (RobloxApi.GetGameDetailsAsync): 0 / null for what Roblox didn't say.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct GameDetailsInfo {
    pub name: String,
    pub creator: String,
    pub creator_group_id: Option<i64>,
    pub creator_verified: bool,
    pub description: Option<String>,
    pub genre: Option<String>,
    /// Roblox's newer genre and subgenre (games API `genre_l1`, `genre_l2`, e.g. "Roleplay & Avatar Sim", "Pet Care";
    /// only to a signed-in ask), the ones its own page shows; the old `genre` is "All" for most games.
    pub genres: Vec<String>,
    pub playing: i64,
    pub visits: i64,
    pub favorites: i64,
    pub likes_percent: Option<i32>,
    pub created: Option<Stamp>,
    pub updated: Option<Stamp>,
    pub root_place_id: i64,
    /// Roblox's thumbs up and down (public), and how many players a server holds (a signed-in ask only; 0 unknown).
    pub up_votes: i64,
    pub down_votes: i64,
    pub max_players: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct GameSummary {
    pub place_id: i64,
    pub universe_id: i64,
    pub name: String,
    pub playing: i64,
}

/// Which Roblox an account signs in to. Roblox VNG (Vietnam) serves the same API under robloxapp.vnggames.com,
/// and its accounts only work there.
pub mod realm {
    pub const GLOBAL: &str = "";
    pub const VNG: &str = "vng";

    pub fn normalize(realm: Option<&str>) -> &'static str {
        if realm == Some(VNG) { VNG } else { GLOBAL }
    }

    /// The API host for a service, e.g. Api(Vng, "auth") = https://auth.robloxapp.vnggames.com.
    pub fn api(realm: &str, service: &str) -> String {
        if realm == VNG { format!("https://{service}.robloxapp.vnggames.com") } else { format!("https://{service}.roblox.com") }
    }

    pub fn website(realm: &str) -> &'static str {
        if realm == VNG { "https://www.robloxapp.vnggames.com" } else { "https://www.roblox.com" }
    }
}

/// Whether an account can play a game (multiget-playability-status, asked by Roblox's client before joining):
/// `status` is its PlayabilityStatus (1 playable, 11 account restricted, 26 age check required…),
/// `text` Roblox's own words for why not.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Playability {
    pub playable: bool,
    pub status: Option<i32>,
    pub text: Option<String>,
}

impl Playability {
    /// The names of Roblox's PlayabilityStatus numbers, as its games API documents them: its live answer gives the name.
    const NAMES: &'static [&'static str] = &[
        "UnplayableOtherReason", "Playable", "GuestProhibited", "GameUnapproved", "IncorrectConfiguration", "UniverseRootPlaceIsPrivate",
        "InsufficientPermissionFriendsOnly", "InsufficientPermissionGroupOnly", "DeviceRestricted", "UnderReview", "PurchaseRequired",
        "AccountRestricted", "TemporarilyUnavailable", "PlaceHasNoPublishedVersion", "ComplianceBlocked",
        "ContextualPlayabilityRegionalAvailability", "ContextualPlayabilityRegionalCompliance",
        "ContextualPlayabilityAgeRecommendationParentalControls", "ContextualPlayabilityExperienceBlockedParentalControls",
        "ContextualPlayabilityAgeGated", "ContextualPlayabilityUnverifiedSeventeenPlusUser", "FiatPurchaseRequired",
        "FiatPurchaseDeviceRestricted", "ContextualPlayabilityUnrated", "ContextualPlayabilityAgeGatedByDescriptor",
        "ContextualPlayabilityGeneral", "ContextualPlayabilityAgeCheckRequired", "ContextualPlayabilityRequireParentApproval",
        "ContextualPlayabilityCoreGated", "ContextualPlayabilityTrustedFriendRequired", "PlusSubscriptionRequired",
        "ContextualPlayabilityPlaytestDisabled", "InsufficientPermissionEditorsOnly", "InsufficientPermissionPlayTestersOnly",
    ];

    pub fn status_of(name: Option<&str>) -> Option<i32> {
        let name = name?;
        Playability::NAMES.iter().position(|n| *n == name).map(|at| at as i32)
    }
}

/// A Quick Login code. `private_key` is a secret: it never leaves the app's memory.
#[derive(Clone)]
pub struct QuickLoginCode {
    pub realm: String,
    pub code: String,
    pub private_key: String,
    pub expires_utc: Stamp,
    pub image_path: Option<String>,
}

impl std::fmt::Debug for QuickLoginCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QuickLoginCode")
            .field("realm", &self.realm)
            .field("code", &self.code)
            .field("private_key", &crate::core::base::Redacted)
            .field("expires_utc", &self.expires_utc)
            .field("image_path", &self.image_path)
            .finish()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct QuickLoginStatus {
    pub status: String,
    pub account_name: Option<String>,
    pub picture_url: Option<String>,
}

/// A public server of a game, as listed on the game's page.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct ServerInfo {
    pub job_id: String,
    pub playing: i32,
    pub max_players: i32,
    pub ping: i32,
    pub fps: f64,
}

/// `type`: 0 offline, 1 online (website/app), 2 in game, 3 in Studio, 4 invisible.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Presence {
    pub r#type: i32,
    pub location: Option<String>,
    pub place_id: Option<i64>,
    pub root_place_id: Option<i64>,
    pub job_id: Option<String>,
    pub last_online: Option<Stamp>,
    pub universe_id: Option<i64>,
}

/// A sign-in cookie Roblox replaced: `sent` went out, the answer set `renewed`. Both are secrets, so its Debug and
/// Display leave them out.
#[derive(Clone, PartialEq, Eq)]
pub struct RenewedCookie {
    pub realm: String,
    pub sent: String,
    pub renewed: String,
}

impl std::fmt::Display for RenewedCookie {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RenewedCookie ({})", if self.realm == realm::VNG { "Roblox VNG" } else { "Roblox" })
    }
}

impl std::fmt::Debug for RenewedCookie {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

/// C#'s RobloxApiException, field for field. It becomes a `Fail::Roblox` when it leaves this layer.
#[derive(Clone, Debug, Default)]
pub struct RobloxApiException {
    pub message: String,
    pub cookie_invalid: bool,
    /// Roblox refused a password sign-in (wrong password, a locked account…): that attempt can't go on, only start over.
    pub attempt_over: bool,
    /// Roblox answered 429: wait a little and try again.
    pub rate_limited: bool,
    /// With `rate_limited`: how long until Roblox takes requests again, when it said.
    pub retry_after: Option<Duration>,
    /// Roblox answered 5xx (down or busy): worth trying again later, unlike a refusal (400, 403).
    pub server_error: bool,
    /// Roblox wouldn't give the account a sign-in ticket (4xx, not 401/429): its roblox.com says why.
    pub sign_in_refused: bool,
    /// Roblox holds the account until a moderation notice (a warning, an ended ban) is accepted on roblox.com:
    /// HTTP 403, "User is moderated" (seen 2026-10-29 on the sign-in check for an account warned that day).
    pub moderated: bool,
}

impl RobloxApiException {
    pub fn new(message: impl Into<String>) -> RobloxApiException {
        RobloxApiException { message: message.into(), ..Default::default() }
    }
}

impl From<RobloxApiException> for Fail {
    fn from(e: RobloxApiException) -> Fail {
        Fail::Roblox {
            message: e.message,
            rate_limited: e.rate_limited,
            cookie_invalid: e.cookie_invalid,
            server_error: e.server_error,
            attempt_over: e.attempt_over,
            retry_after: e.retry_after,
            sign_in_refused: e.sign_in_refused,
            moderated: e.moderated,
        }
    }
}

/// How long a 429 asks to wait: Roblox's x-ratelimit-reset (seconds until its window starts over), else Retry-After, else
/// 20 s; 1 to 120 s. Its Retry-After is the shorter and wrong one: recorded 429s (RoALT's fixtures) said "Retry-After: 5"
/// with a reset of 30 (friends) and 50 (server list).
pub(crate) fn rate_limit_wait(res: &HttpResponse) -> Duration {
    let secs = lookups::first_number(res, "x-ratelimit-reset").or_else(|| lookups::retry_after_secs(res)).unwrap_or(20).clamp(1, 120);
    Duration::from_secs(secs as u64)
}

/// Roblox's 429 as Bloxkeep says it, with how long to wait (`rate_limit_wait`).
fn rate_limited_by(res: &HttpResponse) -> Fail {
    RobloxApiException {
        rate_limited: true,
        retry_after: Some(rate_limit_wait(res)),
        ..RobloxApiException::new(l::t("Roblox is rate-limiting requests; try again shortly."))
    }
    .into()
}

// .NET's exception kinds, as the start of a `Fail::Other`'s text, so a `catch (HttpRequestException)` keeps its meaning.
pub const HTTP_REQUEST_EXCEPTION: &str = "HttpRequestException";
pub const TASK_CANCELED_EXCEPTION: &str = "TaskCanceledException";
pub const JSON_EXCEPTION: &str = "JsonException";
pub const KEY_NOT_FOUND_EXCEPTION: &str = "KeyNotFoundException";
pub const INVALID_OPERATION_EXCEPTION: &str = "InvalidOperationException";
pub const FORMAT_EXCEPTION: &str = "FormatException";
pub const ARGUMENT_EXCEPTION: &str = "ArgumentException";

/// A failure of one of .NET's exception kinds.
pub fn fail_of(kind: &str, text: impl std::fmt::Display) -> Fail {
    Fail::Other(format!("{kind}: {text}"))
}

/// Whether a failure is of that .NET exception kind (`Cancel::check`'s "cancelled" counts as a TaskCanceledException).
pub fn is_fail_kind(f: &Fail, kind: &str) -> bool {
    match f {
        Fail::Other(m) => (m.starts_with(kind) && m[kind.len()..].starts_with(':')) || (kind == TASK_CANCELED_EXCEPTION && m == "cancelled"),
        _ => false,
    }
}

/// The .NET type name of a failure, for logs ("RobloxApiException", "HttpRequestException", …).
pub fn fail_type_name(f: &Fail) -> String {
    match f {
        Fail::Roblox { .. } => "RobloxApiException".into(),
        Fail::User(_) => "UserError".into(),
        Fail::Other(m) if m == "cancelled" => TASK_CANCELED_EXCEPTION.into(),
        Fail::Other(m) => match m.split_once(':') {
            Some((kind, _)) if kind.ends_with("Exception") && !kind.contains(' ') => kind.into(),
            _ => "Exception".into(),
        },
    }
}

// ---- the request and answer, as HttpRequestMessage / HttpResponseMessage carried them ----

/// A request about to go out: the headers are set by hand, exactly as the C# did.
#[derive(Clone)]
pub struct HttpRequest {
    pub method: &'static str,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    /// Sent the way Roblox's own client goes, not through Windows' proxy (net::builder_like_roblox_client): the sign-in
    /// ticket, which Roblox refuses when the client redeems it from another address.
    pub like_roblox_client: bool,
}

impl HttpRequest {
    pub fn new(method: &'static str, url: impl Into<String>) -> HttpRequest {
        HttpRequest { method, url: url.into(), headers: Vec::new(), body: None, like_roblox_client: false }
    }
    /// TryAddWithoutValidation.
    pub fn add_header(&mut self, name: &str, value: &str) {
        self.headers.push((name.to_string(), value.to_string()));
    }
    /// Every value of a header (names compared without case).
    pub fn headers_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.headers.iter().filter(move |(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
    /// new StringContent(json, Encoding.UTF8, "application/json").
    pub fn set_json_body(&mut self, json: &str) {
        self.headers.retain(|(n, _)| !n.eq_ignore_ascii_case("Content-Type"));
        self.add_header("Content-Type", "application/json; charset=utf-8");
        self.body = Some(json.as_bytes().to_vec());
    }
}

/// An answer, its body read in full (as HttpClient buffers it).
#[derive(Clone, Default)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// Header names and the body's size only: a Set-Cookie carries .ROBLOSECURITY, a body can carry tokens.
impl std::fmt::Debug for HttpResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpResponse")
            .field("status", &self.status)
            .field("headers", &self.header_names())
            .field("body_len", &self.body.len())
            .finish()
    }
}

impl HttpResponse {
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
    pub fn headers_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.headers.iter().filter(move |(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
    /// The distinct header names, as they came.
    pub fn header_names(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for (n, _) in &self.headers {
            if !names.iter().any(|x| x.eq_ignore_ascii_case(n)) {
                names.push(n.clone());
            }
        }
        names
    }
    /// ReadAsStringAsync: the body as text (UTF-8, a byte order mark left out).
    pub fn text(&self) -> String {
        let bytes = self.body.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(&self.body);
        String::from_utf8_lossy(bytes).into_owned()
    }
    /// Content.Headers.ContentType?.MediaType.
    pub fn media_type(&self) -> Option<String> {
        let ct = self.headers_named("Content-Type").next()?;
        Some(ct.split(';').next().unwrap_or("").trim().to_string())
    }
}

/// The checks' Roblox (Tests/Bloxkeep.Tests): when set, it answers every request instead of the network. The app never sets it.
pub type StandIn = Box<dyn Fn(&HttpRequest) -> Res<HttpResponse> + Send + Sync>;
pub static STAND_IN: RwLock<Option<StandIn>> = RwLock::new(None);

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36";
const COOKIE_NAME: &str = ".ROBLOSECURITY";
/// The most of an answer that is read (HttpClient's own buffer limit is 2 GB; no Roblox answer comes near this).
const MAX_BODY: u64 = 64 * 1024 * 1024;

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn http() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        // PORT: ureq has no PooledConnectionLifetime (5 minutes in the C#); idle connections are kept by its own pool rules.
        crate::net::builder()
            .redirects(0)
            .timeout(Duration::from_secs(20))
            .resolver(like_roblox_client)
            .build()
    })
}

/// http()'s agent, reaching the host as Roblox's client does (no Windows proxy). Made per call: one per launch.
fn http_like_roblox_client(url: &str) -> ureq::Agent {
    let host = url::Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_default();
    crate::net::builder_like_roblox_client(&host)
        .redirects(0)
        .timeout(Duration::from_secs(20))
        .resolver(like_roblox_client)
        .build()
}

/// A host's addresses in the order Windows gives them, tried in that order, as Roblox's client (libcurl) tries them: a
/// sign-in ticket is bound to the address that asks for it, so it must leave the way the client's redeem will. IPv4
/// only, as before, made every launch fail on a network where Roblox is reached over IPv6 through NAT64 (a user's
/// error.log, 2026-11-22: Roblox's client redeemed at 64:ff9b::8074:7803 and got 403 every time, while the ticket was
/// asked at 128.116.120.3). Where roblox.com has no IPv6 address (most networks) both go over IPv4 still.
fn like_roblox_client(netloc: &str) -> std::io::Result<Vec<SocketAddr>> {
    Ok(netloc.to_socket_addrs()?.collect())
}

/// Where the last request sent the client's way went (send_on): error.log says it with the ticket.
pub(crate) static LAST_CLIENT_WAY: Mutex<Option<SocketAddr>> = Mutex::new(None);

/// One request on an agent, its answer read whole: any HTTP status is an answer; no answer is an HttpRequestException
/// (or a TaskCanceledException when it timed out), as HttpClient threw them.
pub fn send_on(agent: &ureq::Agent, req: &HttpRequest) -> Res<HttpResponse> {
    let mut r = agent.request(req.method, &req.url);
    for (name, value) in &req.headers {
        r = r.set(name, value);
    }
    let result = match &req.body {
        Some(body) => r.send_bytes(body),
        None => r.call(),
    };
    let res = match result {
        Ok(res) | Err(ureq::Error::Status(_, res)) => {
            if req.like_roblox_client {
                *lock(&LAST_CLIENT_WAY) = Some(res.remote_addr());
            }
            res
        }
        Err(ureq::Error::Transport(t)) => {
            let text = t.to_string();
            let timed_out = t.kind() == ureq::ErrorKind::Io && (text.contains("timed out") || text.contains("timeout"));
            return Err(fail_of(if timed_out { TASK_CANCELED_EXCEPTION } else { HTTP_REQUEST_EXCEPTION }, text));
        }
    };
    let status = res.status();
    let mut headers = Vec::new();
    let mut names: Vec<String> = Vec::new();
    for name in res.headers_names() {
        if !names.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
            names.push(name);
        }
    }
    for name in &names {
        for value in res.all(name) {
            headers.push((name.clone(), value.to_string()));
        }
    }
    let mut body = Vec::new();
    res.into_reader().take(MAX_BODY).read_to_end(&mut body).map_err(|e| {
        if e.kind() == std::io::ErrorKind::TimedOut { fail_of(TASK_CANCELED_EXCEPTION, e) } else { fail_of(HTTP_REQUEST_EXCEPTION, e) }
    })?;
    Ok(HttpResponse { status, headers, body })
}

/// Cookies Roblox renewed, waiting for the app to save them (AppController.SignIns.cs). Roblox sends a new
/// .ROBLOSECURITY every few hours to days and months later refuses the one it replaced (devforum.roblox.com/t/4328913,
/// announced 2026-02-03, enforced from 2026-05-01).
static RENEWALS: Mutex<VecDeque<RenewedCookie>> = Mutex::new(VecDeque::new());

pub fn try_take_renewed_cookie() -> Option<RenewedCookie> {
    lock(&RENEWALS).pop_front()
}

pub fn renewed_cookies_waiting() -> bool {
    !lock(&RENEWALS).is_empty()
}

/// Every answer from Roblox passes here (C#'s CookieWatch): one to a request that carried a sign-in cookie may set a
/// new one.
pub fn send(req: &HttpRequest, ct: &Cancel) -> Res<HttpResponse> {
    if ct.is_cancelled() {
        return Err(fail_of(TASK_CANCELED_EXCEPTION, "The request was canceled."));
    }
    // A damaged saved cookie (it never signed in): said as a sign-in to redo, not as a network error.
    let named = format!("{COOKIE_NAME}=");
    if req.headers_named("Cookie").any(|c| c.strip_prefix(named.as_str()).is_some_and(|v| !cookie_text_valid(v))) {
        return Err(RobloxApiException { cookie_invalid: true, ..RobloxApiException::new(l::t("This account's saved sign-in is damaged — sign in to it again.")) }.into());
    }
    let res = {
        let stand_in = STAND_IN.read().unwrap_or_else(|e| e.into_inner());
        match stand_in.as_ref() {
            Some(answer) => answer(req)?,
            None if req.like_roblox_client => send_on(&http_like_roblox_client(&req.url), req)?,
            None => send_on(http(), req)?,
        }
    };
    if let Some(renewed) = renewed_by(req, &res) {
        lock(&RENEWALS).push_back(renewed);
    }
    // PORT: a blocking request can't be called off half way as HttpClient's can; a call cancelled meanwhile fails here,
    // after its answer came (a renewed cookie in it is kept above).
    if ct.is_cancelled() {
        return Err(fail_of(TASK_CANCELED_EXCEPTION, "The request was canceled."));
    }
    Ok(res)
}

/// The cookie an answer sets in place of the request's; None when none went or came back, the same came back,
/// or the answer deletes it (signing out sets an empty one, expired in 1970).
pub(crate) fn renewed_by(req: &HttpRequest, res: &HttpResponse) -> Option<RenewedCookie> {
    let host = url::Url::parse(&req.url).ok()?.host_str()?.to_string();
    let set_cookies: Vec<&str> = res.headers_named("Set-Cookie").collect();
    if set_cookies.is_empty() {
        return None;
    }
    let carried = req.headers_named("Cookie").find(|c| c.contains(&format!("{COOKIE_NAME}=")))?;
    // Only Roblox's own hosts set its sign-in cookie (and every request here goes to them anyway).
    let realm = if ends_with_ci(&host, ".robloxapp.vnggames.com") {
        Some(realm::VNG)
    } else if ends_with_ci(&host, ".roblox.com") {
        Some(realm::GLOBAL)
    } else {
        None
    };
    let sent = normalize_cookie(carried);
    // The last one wins, as in a browser.
    let renewed = set_cookies.iter().filter_map(|h| sets_cookie(h)).last();
    match (realm, renewed) {
        (Some(realm), Some(renewed)) if !sent.is_empty() && renewed != sent => Some(RenewedCookie { realm: realm.into(), sent, renewed }),
        _ => None,
    }
}

/// The value a Set-Cookie header gives .ROBLOSECURITY; None for another cookie, an empty value, or one already expired.
fn sets_cookie(header: &str) -> Option<String> {
    let parts: Vec<&str> = header.split(';').collect();
    let eq = parts[0].find('=')?;
    if parts[0][..eq].trim() != COOKIE_NAME {
        return None;
    }
    for part in parts.iter().skip(1) {
        let Some(at) = part.find('=') else { continue };
        let (name, value) = (part[..at].trim(), part[at + 1..].trim());
        if name.eq_ignore_ascii_case("max-age") && value.parse::<i64>().is_ok_and(|age| age <= 0) {
            return None;
        }
        if name.eq_ignore_ascii_case("expires") && parse_cookie_date(value).is_some_and(|expires| expires <= Stamp::utc_now()) {
            return None;
        }
    }
    let cookie = parts[0][eq + 1..].trim().trim_matches('"');
    if cookie.is_empty() { None } else { Some(cookie.to_string()) }
}

/// "ddd, dd-MMM-yyyy HH:mm:ss 'GMT'", "ddd, dd MMM yyyy HH:mm:ss 'GMT'" or "r" (RFC 1123), read exactly as .NET's
/// ParseExact reads them: two-digit day and clock, four-digit year, the weekday matching the date.
fn parse_cookie_date(text: &str) -> Option<Stamp> {
    const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let (day_name, rest) = text.split_once(", ")?;
    let weekday = DAYS.iter().position(|d| d.eq_ignore_ascii_case(day_name))?;
    let b = rest.as_bytes();
    // "dd-MMM-yyyy HH:mm:ss GMT" or "dd MMM yyyy HH:mm:ss GMT": 24 characters either way.
    if b.len() != 24 || !(b[2] == b'-' && b[6] == b'-' || b[2] == b' ' && b[6] == b' ') || b[11] != b' ' || b[14] != b':' || b[17] != b':' || rest.get(20..) != Some(" GMT") {
        return None;
    }
    let num = |from: usize, len: usize| -> Option<u32> {
        let s = rest.get(from..from + len)?;
        if s.bytes().all(|c| c.is_ascii_digit()) { s.parse().ok() } else { None }
    };
    let day = num(0, 2)?;
    let month_name = rest.get(3..6)?;
    let month = MONTHS.iter().position(|m| m.eq_ignore_ascii_case(month_name))? as u32 + 1;
    let (year, hour, minute, second) = (num(7, 4)?, num(12, 2)?, num(15, 2)?, num(18, 2)?);
    if day == 0 || day > 31 || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let stamp = Stamp::parse(&format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"))?;
    let c = stamp.utc();
    (c.day == day && c.month == month && c.weekday as usize == weekday).then_some(stamp)
}

/// Accepts a bare value, "name=value", or a whole Cookie header pasted from DevTools.
pub fn normalize_cookie(raw: &str) -> String {
    let mut c = raw.trim().trim_matches(|ch| ch == '"' || ch == '\'').trim().to_string();
    if let Some(i) = find_ci(&c, &format!("{COOKIE_NAME}=")) {
        c = c[i + COOKIE_NAME.len() + 1..].to_string();
    }
    if let Some(semi) = c.find(';') {
        c.truncate(semi);
    }
    // A cookie has no spaces: a paste wrapped over lines (a chat, a text file) is joined back.
    c.chars().filter(|ch| !ch.is_whitespace()).collect()
}

/// Only what a header can carry (visible ASCII), as every real cookie is. "…" (a cut-off copy), curly quotes or accents
/// made the request fail, which Bloxkeep then called a network error (RoALT checks the same).
pub fn cookie_text_valid(cookie: &str) -> bool {
    cookie.chars().all(|ch| ch > ' ' && ch < '\u{7f}')
}

/// Said for a pasted cookie that isn't one (`cookie_text_valid`).
pub fn cookie_text_error() -> String {
    l::t("That isn't a whole Roblox cookie: it has characters a cookie never has (\"…\", curly quotes or accents). Copy it again, all of it.")
}

/// HttpRequestException, TaskCanceledException, JsonException, RobloxApiException, KeyNotFoundException or
/// InvalidOperationException: what a Roblox call is expected to fail with.
pub fn is_expected_error(ex: &Fail) -> bool {
    matches!(ex, Fail::Roblox { .. })
        || [HTTP_REQUEST_EXCEPTION, TASK_CANCELED_EXCEPTION, JSON_EXCEPTION, KEY_NOT_FOUND_EXCEPTION, INVALID_OPERATION_EXCEPTION].iter().any(|k| is_fail_kind(ex, k))
}

fn request(method: &'static str, url: &str, cookie: Option<&str>, from_website: bool, realm: &str) -> HttpRequest {
    let mut req = HttpRequest::new(method, url);
    req.add_header("User-Agent", USER_AGENT);
    req.add_header("Accept", "application/json, text/plain, */*");
    if let Some(cookie) = cookie.filter(|c| !c.is_empty()) {
        req.add_header("Cookie", &format!("{COOKIE_NAME}={cookie}"));
    }
    if from_website {
        req.add_header("Origin", realm::website(realm));
        req.add_header("Referer", &format!("{}/", realm::website(realm)));
    }
    req
}

fn header(res: &HttpResponse, name: &str) -> Option<String> {
    res.headers_named(name).next().map(str::to_string)
}

/// A 403 whose error says the account is moderated (`RobloxApiException::moderated`).
fn is_moderated(status: u16, body: &str) -> bool {
    status == 403 && find_ci(&errors(body).1, "moderated").is_some()
}

/// Roblox's error codes, and for the log its error messages: never the rest of a reply (it may name the account).
fn errors(body: &str) -> (Vec<i32>, String) {
    let mut codes = Vec::new();
    let mut parts = Vec::new();
    if let Some(Value::Object(root)) = decode_json(body)
        && let Some(Value::Array(errors)) = root.get("errors")
    {
        for e in errors.iter().filter(|e| e.is_object()) {
            let code = match e.get("code") {
                Some(c @ Value::Number(_)) => c.as_i64().and_then(|n| i32::try_from(n).ok()).unwrap_or(-1),
                _ => -1,
            };
            let message = match e.get("message") {
                Some(Value::String(m)) => m.clone(),
                _ => String::new(),
            };
            codes.push(code);
            parts.push(format!("{code} '{}'", if message.chars().count() > 120 { format!("{}…", message.chars().take(120).collect::<String>()) } else { message }));
        }
    }
    let summary = if parts.is_empty() { format!("none listed ({} chars)", body.encode_utf16().count()) } else { parts.join(", ") };
    (codes, summary)
}

/// A refusal from Roblox: a short sentence for the person; what Roblox said goes to the log.
fn refused(what: &str, status: u16, body: &str, cookie_invalid: bool) -> Fail {
    let code = status as i32;
    log::note(&format!("{what}: HTTP {code}, errors {}", errors(body).1));
    if is_moderated(status, body) {
        return RobloxApiException {
            moderated: true,
            ..RobloxApiException::new(l::t("Roblox has a notice on this account to accept first. Open its roblox.com and accept it."))
        }
        .into();
    }
    RobloxApiException {
        cookie_invalid,
        server_error: code >= 500,
        ..RobloxApiException::new(if code >= 500 {
            l::f("Roblox isn't working properly right now (HTTP {0}). Try again in a moment.", &[&code])
        } else {
            l::f("Roblox refused the request (HTTP {0}). Try again in a moment.", &[&code])
        })
    }
    .into()
}

/// A refused password sign-in (/v2/login or a check on the way) in plain words; the attempt is over either way.
/// Codes from auth.roblox.com's /v2/login docs; 16 to 28 only roblox.com's own login page knows (screen time,
/// security questions, blocked, account-switch limits, passkey-only).
fn login_refused(step: &str, status: u16, body: &str) -> Fail {
    let (codes, summary) = errors(body);
    log::note(&format!("Sign-in refused at {step}: HTTP {status}, errors {summary}"));
    let has = |wanted: &[i32]| codes.iter().any(|c| wanted.contains(c));
    let too_many = status == 429 || has(&[7, 15]);
    let server = status >= 500;
    let code = status as i32;
    let message = if too_many {
        l::t("Too many sign-in attempts. Wait a few minutes and try again.")
    } else if has(&[1]) || body.contains("Incorrect username or password") {
        l::t("Incorrect username or password.")
    } else if has(&[4]) {
        l::t("Roblox has locked this account to protect it. Reset its password on roblox.com (Forgot password) and sign in again, or sign in with your phone.")
    } else if has(&[3]) {
        l::t("Enter the username and password.")
    } else if has(&[6]) {
        l::t("Roblox reports a problem with this account (it may be restricted or banned). Check it on roblox.com.")
    } else if has(&[5, 8, 9, 10, 12, 14, 16, 18, 22, 23, 24, 25, 26, 27, 28]) {
        l::t("Roblox doesn't allow a password sign-in for this account here. Sign in with your phone or the browser instead.")
    } else if has(&[2]) {
        l::t("Roblox wants a captcha Bloxkeep didn't get. Try again, or sign in with your phone or the browser.")
    } else if server {
        l::f("Roblox isn't working properly right now (HTTP {0}). Try again in a moment.", &[&code])
    } else {
        l::f("Roblox didn't accept the sign-in (HTTP {0}). Try again, or sign in with your phone or the browser.", &[&code])
    };
    RobloxApiException { rate_limited: too_many, server_error: server, attempt_over: true, ..RobloxApiException::new(message) }.into()
}

/// None when Roblox rejects the cookie (expired, logged out, or invalid).
pub fn get_authenticated_user(cookie: &str, realm: &str, ct: &Cancel) -> Res<Option<RobloxUser>> {
    let req = request("GET", &format!("{}/v1/users/authenticated", realm::api(realm, "users")), Some(cookie), false, realm::GLOBAL);
    let res = send(&req, ct)?;
    if res.status == 401 {
        return Ok(None);
    }
    if res.status == 429 {
        return Err(rate_limited_by(&res));
    }
    let body = res.text();
    if !res.is_success() {
        return Err(refused("Signed-in user", res.status, &body, false));
    }
    let root = parse_json(&body)?;
    Ok(Some(RobloxUser {
        id: get_i64(get_prop(&root, "id")?)?,
        name: get_str(get_prop(&root, "name")?)?.unwrap_or_default(),
        display_name: get_str(get_prop(&root, "displayName")?)?.unwrap_or_default(),
    }))
}

/// Roblox's answer to gamejoin's join-game (what a Roblox window asks when it joins): HTTP status, Roblox's
/// status, message and statusData (raw, cut short). A successful answer's join ticket is left unread.
/// `queue_position`: the account's place in a full game's line, when Roblox gives one (its client shows "Position in queue is N").
#[derive(Clone, Debug, Default)]
pub struct JoinAnswer {
    pub http: i32,
    pub status: Option<i32>,
    pub message: Option<String>,
    pub data: Option<String>,
    pub queue_position: Option<i32>,
    /// Roblox's bot check holding this join (a 403 with its rblx-challenge-* headers), or None.
    pub challenge: Option<JoinChallenge>,
    /// The join named a Job ID and Roblox refused it without a ban: the server is gone, not the game closed to
    /// the account. Seen 2026-10-29: joins to a server that had ended with a system error (PlacelaunchGameEnded) got
    /// status 12, "Unable to join Game 311", every time. Counted as ended so the accounts try another server. Set by
    /// the join check, which knows the target.
    pub server_gone: bool,
}

static JOIN_CODE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"Unable to join Game (\d+)").unwrap());

impl JoinAnswer {
    /// Status 6: "Requested experience is full"; the client's ConnectionError 518 PlacelaunchGameFull (512 +
    /// the status, as 524 is for 12).
    pub fn full(&self) -> bool {
        self.status == Some(6)
    }

    /// The server has ended: status 5, the client's ConnectionError 517 PlacelaunchGameEnded (Roblox's API dump).
    pub fn ended(&self) -> bool {
        self.status == Some(5) || self.server_gone
    }

    /// Roblox refuses the account this game, for good: a status its client counts as a join error
    /// (FStringGameJoinStatusConsideredError, "12;29;30;31;32;33;" in 0.744), not a wait. Includes `banned`.
    pub fn refused(&self) -> bool {
        !self.server_gone && matches!(self.status, Some(12) | Some(29..=33))
    }

    /// The number in "Unable to join Game 211": what the window turns into its own error.
    pub fn code(&self) -> Option<i32> {
        let found = JOIN_CODE.captures(self.message.as_deref()?)?;
        found[1].parse::<i32>().ok()
    }

    /// The game or its moderators banned the account: status 12, "Unable to join Game 211" (seen 2026-10-21 with
    /// the window's Join Error 600, "You were banned by this experience or its moderators"). Words about a ban count too.
    pub fn banned(&self) -> bool {
        (self.status == Some(12) && self.code() == Some(211))
            || self.message.as_deref().is_some_and(|m| contains_ci(m, "banned") || m.to_lowercase().contains(&"bị cấm".to_lowercase()))
    }

    /// The game's own words: after "Moderation message:", or a message or reason in statusData; None otherwise
    /// (the 211 answer carries none).
    pub fn reason(&self) -> Option<String> {
        if let Some(m) = &self.message
            && let Some(at) = find_ci(m, "Moderation message:")
        {
            let words = m[at + 19..].trim();
            if !words.is_empty() {
                return Some(words.to_string());
            }
        }
        let data = self.data.as_deref()?;
        if !data.trim_start().starts_with('{') {
            return None;
        }
        // A JsonException: None.
        if let Ok(Value::Object(root)) = serde_json::from_str::<Value>(data) {
            for (name, value) in &root {
                if let Value::String(said) = value
                    && !said.is_empty()
                    && (contains_ci(name, "message") || contains_ci(name, "reason"))
                {
                    return Some(said.clone());
                }
            }
        }
        None
    }
}

/// The headers carrying a check already answered, as roblox.com sends them on the retried request.
fn add_challenge_headers(req: &mut HttpRequest, answered: &JoinChallenge) {
    req.add_header("rblx-challenge-id", &answered.id);
    req.add_header("rblx-challenge-type", &answered.r#type);
    req.add_header("rblx-challenge-metadata", &base64::engine::general_purpose::STANDARD.encode(answered.metadata_json.as_bytes()));
}

/// Asks Roblox to let the account into a game only to read its answer, after a window waited a minute
/// without getting in and Roblox logged nothing about why.
/// `answered`: a check answered (`continue_challenge`): the join is asked again carrying it, as roblox.com retries a
/// request once its check is passed.
pub fn join_check(cookie: &str, place_id: i64, job_id: Option<&str>, realm: &str, ct: &Cancel, answered: Option<&JoinChallenge>) -> Res<JoinAnswer> {
    let url = format!("{}{}", realm::api(realm, "gamejoin"), if job_id.is_none() { "/v1/join-game" } else { "/v1/join-game-instance" });
    let body = match job_id {
        None => json!({ "placeId": place_id, "isTeleport": false, "gameJoinAttemptId": new_guid().to_string() }),
        Some(job) => json!({ "placeId": place_id, "gameId": job, "isTeleport": false, "gameJoinAttemptId": new_guid().to_string() }),
    };
    let configure = |req: &mut HttpRequest| {
        if let Some(answered) = answered {
            add_challenge_headers(req, answered);
        }
    };
    let res = post_with_csrf(&url, cookie, &body.to_string(), answered.map(|_| &configure as &dyn Fn(&mut HttpRequest)), ct, realm)?;
    let text = res.text();
    // Roblox's bot check ("Challenge is required to authorize the request", 403): which check, and what it carries, by
    // name only (its values tie it to this account).
    let mut challenge = None;
    if let Some(challenge_id) = header(&res, "rblx-challenge-id").filter(|c| !c.is_empty()) {
        let meta = decode_metadata(header(&res, "rblx-challenge-metadata").as_deref());
        let found = JoinChallenge {
            id: challenge_id,
            r#type: header(&res, "rblx-challenge-type").unwrap_or_default(),
            metadata_json: meta.as_ref().map(raw_text).unwrap_or_else(|| "{}".into()),
        };
        // The action and the request it holds are Roblox's own names, nothing of the account's: said as they are.
        fn plain(s: Option<String>) -> String {
            match s {
                Some(s) if !s.is_empty() && s.chars().count() <= 60 && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '.')) => s,
                _ => "#".into(),
            }
        }
        let fields = match &meta {
            Some(Value::Object(m)) => m.keys().map(|k| safe_name(k)).collect::<Vec<_>>().join(", "),
            _ => "(none)".into(),
        };
        log::note(&format!(
            "Join held for Roblox's check: type '{}', metadata fields {fields}; action '{}', request {} {}",
            safe_name(&found.r#type),
            plain(found.meta("actionType")),
            plain(found.meta("requestMethod")),
            plain(found.meta("requestPath"))
        ));
        challenge = Some(found);
    }
    let (mut status, mut queue, mut message, mut data) = (None, None, None, None);
    if text.trim_start().starts_with('{') {
        let root = parse_json(&text)?;
        if let Some(s @ Value::Number(_)) = try_prop(&root, "status")? {
            status = Some(get_i32(s)?);
        }
        if let Some(q @ Value::Number(_)) = try_prop(&root, "queuePosition")?
            && let Some(place) = try_i32(q)?
            && place > 0
        {
            queue = Some(place);
        }
        if let Some(Value::String(m)) = try_prop(&root, "message")? {
            message = Some(m.clone());
        }
        // A refusal of the request itself: {"errors":[{"code":…,"message":…}]}.
        else if let Some(Value::Array(errors)) = try_prop(&root, "errors")?
            && !errors.is_empty()
            && let Some(Value::String(e)) = try_prop(&errors[0], "message")?
        {
            message = Some(e.clone());
        }
        // Never with a join ticket: only an answer that let the account in holds one, and that one is left unread.
        if status != Some(2)
            && let Some(d @ (Value::Object(_) | Value::String(_))) = try_prop(&root, "statusData")?
        {
            let raw = raw_text(d);
            data = Some(if raw.chars().count() > 400 { raw.chars().take(400).collect() } else { raw });
        }
    }
    Ok(JoinAnswer { http: res.status as i32, status, message, data, queue_position: queue, challenge, server_gone: false })
}

/// A public server's address (joinScript › UdmuxEndpoints › Address, else MachineAddress), from the join answer
/// roblox.com gets before it opens that server (server_regions.rs): nothing is joined, and the answer's ticket is
/// neither kept nor written anywhere. None when Roblox gave no address (the server full or gone, a check asked, the
/// account not let in); Err when it refused the asking itself (too many at once, the sign-in gone).
/// `Err` when Roblox stops answering (too many, signed out); `Ok(Err(why))` when it answered without an address, in a few
/// words for the log: the HTTP status, or the join answer's own status and message (its bot check, a server closing).
/// What gamejoin says of one server: its address, the game version it runs (`PlaceVersion`) and when it started
/// (`ServerClaimedTime`, Unix ms; 0 when Roblox doesn't say, about one in four).
#[derive(Clone, Debug)]
pub struct ServerJoin {
    pub address: String,
    pub version: i64,
    pub started_ms: i64,
}

pub fn server_address(cookie: &str, place_id: i64, job_id: &str, ct: &Cancel) -> Res<Result<ServerJoin, String>> {
    // Asked as roblox.com asks, every server answers "join status 12 Unable to join Game 311" (the owner's 60 of 60,
    // error.log 2026-11-19; "chưa bao giờ chọn được server luôn"). Asked as Roblox's own client asks, the same sign-in
    // got the address (status 2), "full" (22), or the real reason (12 "This game is unavailable in your region to
    // comply with regulations.", Blox Fruits and Jailbreak from Vietnam): measured that evening on 7 games.
    server_address_as(cookie, place_id, job_id, ct, true)
}

/// Roblox doesn't open this game where the account is (its gamejoin's own words): no server of it will be placed.
pub fn is_region_blocked(why: &str) -> bool {
    why.contains("unavailable in your region")
}

/// gamejoin's join-game-instance for one server: its address, or why there is none. `as_client`: asked with Roblox's own
/// client's user agent and no website Origin/Referer, as RobloxPlayerBeta asks it.
fn server_address_as(cookie: &str, place_id: i64, job_id: &str, ct: &Cancel, as_client: bool) -> Res<Result<ServerJoin, String>> {
    let url = format!("{}/v1/join-game-instance", realm::api(realm::GLOBAL, "gamejoin"));
    let body = json!({ "placeId": place_id, "gameId": job_id, "isTeleport": false, "gameJoinAttemptId": new_guid().to_string() });
    let client = |req: &mut HttpRequest| {
        req.headers.retain(|(n, _)| !matches!(n.to_ascii_lowercase().as_str(), "user-agent" | "origin" | "referer"));
        req.add_header("User-Agent", "Roblox/WinInet");
    };
    let res = post_with_csrf(&url, cookie, &body.to_string(), as_client.then_some(&client as &dyn Fn(&mut HttpRequest)), ct, realm::GLOBAL)?;
    if res.status == 429 || res.status == 401 {
        return Err(Fail::other(format!("gamejoin answered HTTP {} to a server's address", res.status)));
    }
    if !res.is_success() {
        let check = res.headers_named("rblx-challenge-type").next().map(|t| format!(", check {t}")).unwrap_or_default();
        return Ok(Err(format!("HTTP {}{check}", res.status)));
    }
    let Ok(root) = serde_json::from_str::<Value>(&res.text()) else { return Ok(Err("an answer that isn't JSON".into())) };
    let script = root.get("joinScript");
    let address = script
        .and_then(|s| s.get("UdmuxEndpoints"))
        .and_then(Value::as_array)
        .and_then(|l| l.first())
        .and_then(|e| e.get("Address"))
        .and_then(Value::as_str)
        .or_else(|| script.and_then(|s| s.get("MachineAddress")).and_then(Value::as_str))
        .filter(|a| !a.is_empty());
    let num = |k: &str| script.and_then(|s| s.get(k)).and_then(Value::as_i64).unwrap_or(0);
    Ok(address.map(|a| ServerJoin { address: a.to_string(), version: num("PlaceVersion"), started_ms: num("ServerClaimedTime") }).ok_or_else(|| {
        let status = root.get("status").map(|s| s.to_string()).unwrap_or_else(|| "none".into());
        let message: String = root.get("message").and_then(Value::as_str).unwrap_or_default().chars().take(80).collect();
        format!("join status {status} {message}").trim().to_string()
    }))
}

/// A plain GET of an address that isn't Roblox's, with no sign-in (server_regions.rs asks ipinfo.io this way).
pub fn get_plain(url: &str, ct: &Cancel) -> Res<HttpResponse> {
    send(&request("GET", url, None, false, realm::GLOBAL), ct)
}

/// The check on a join (rblx-challenge-* headers): id, kind ("captcha", "captchav2", "proofofwork"…), metadata JSON.
#[derive(Clone, Default, PartialEq)]
pub struct JoinChallenge {
    pub id: String,
    pub r#type: String,
    pub metadata_json: String,
}

/// The metadata (the check's tokens) never printed; JoinAnswer and FriendAnswer print through this.
impl std::fmt::Debug for JoinChallenge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JoinChallenge")
            .field("id", &self.id)
            .field("type", &self.r#type)
            .field("metadata_json", &crate::core::base::Redacted)
            .finish()
    }
}

impl JoinChallenge {
    /// A text field of what the check carries, or None.
    pub fn meta(&self, name: &str) -> Option<String> {
        field(decode_json(&self.metadata_json).as_ref(), name)
    }
    /// What it carries with one field added (the answer: a redemption or captcha token).
    pub fn meta_with(&self, name: &str, value: &str) -> Res<String> {
        with_field(&self.metadata_json, name, value)
    }
}

/// Hands Roblox a check's answer with the account's sign-in, as roblox.com does, and returns the check the
/// retried request carries (echoed, sometimes with a new id, or a different kind to answer in turn).
pub fn continue_challenge(realm: &str, cookie: &str, answered: &JoinChallenge, ct: &Cancel) -> Res<JoinChallenge> {
    let json = json!({ "challengeId": answered.id, "challengeType": answered.r#type, "challengeMetadata": answered.metadata_json }).to_string();
    let res = post_with_csrf(&format!("{}/challenge/v1/continue", realm::api(realm, "apis")), cookie, &json, None, ct, realm)?;
    let body = res.text();
    if !res.is_success() {
        return Err(refused("Check answer", res.status, &body, false));
    }
    let reply = decode_json(&body);
    let fields = match &reply {
        Some(Value::Object(r)) => r.keys().map(|k| safe_name(k)).collect::<Vec<_>>().join(", "),
        _ => "(none)".into(),
    };
    log::note(&format!(
        "Check answer ({}) reply: fields {fields}, type '{}'",
        safe_name(&answered.r#type),
        safe_name(&field(reply.as_ref(), "challengeType").unwrap_or_default())
    ));
    let pick = |name: &str, otherwise: &str| field(reply.as_ref(), name).filter(|v| !v.is_empty()).unwrap_or_else(|| otherwise.to_string());
    Ok(JoinChallenge {
        id: pick("challengeId", &answered.id),
        r#type: pick("challengeType", &answered.r#type),
        metadata_json: pick("challengeMetadata", &answered.metadata_json),
    })
}

/// The FunCaptcha key Roblox uses for `action_type` (its captcha settings name each action), or None.
pub fn get_captcha_key(realm: &str, action_type: &str, ct: &Cancel) -> Res<Option<String>> {
    let req = request("GET", &format!("{}/captcha/v1/metadata", realm::api(realm, "apis")), None, false, realm::GLOBAL);
    let res = send(&req, ct)?;
    let body = res.text();
    if !res.is_success() {
        return Err(refused("Captcha settings", res.status, &body, false));
    }
    let root = parse_json(&body)?;
    let Some(Value::Object(keys)) = try_prop(&root, "funCaptchaPublicKeys")? else {
        return Ok(None);
    };
    // Compared without the underscores: "WebLogin" is ACTION_TYPE_WEB_LOGIN, "Generic" ACTION_TYPE_GENERIC_CHALLENGE (a
    // name it starts). Roblox lists no key of a join's own (2026-10-27): a join's check, seen as "captcha" in the
    // client's log, takes the generic one when nothing else fits.
    fn plain(s: &str) -> String {
        s.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_uppercase()).collect()
    }
    let wanted = format!("ACTIONTYPE{}", plain(action_type));
    let all: Vec<(&String, &str)> = keys.iter().filter_map(|(k, v)| v.as_str().map(|s| (k, s))).collect();
    let key = all
        .iter()
        .find(|(k, _)| plain(k) == wanted)
        .or_else(|| all.iter().find(|(k, _)| plain(k).starts_with(&wanted)))
        .or_else(|| all.iter().find(|(k, _)| k.as_str() == "ACTION_TYPE_GENERIC_CHALLENGE"))
        .map(|(_, v)| v.to_string());
    log::note(&format!(
        "Captcha key for '{}': {} among {}",
        safe_name(action_type),
        if key.is_some() { "found" } else { "none" },
        keys.keys().map(|k| safe_name(k)).collect::<Vec<_>>().join(", ")
    ));
    Ok(key)
}

/// Last CSRF token per signed-in session (keyed by a hash of the cookie, never the cookie itself).
pub(crate) static CSRF_TOKENS: LazyLock<Mutex<HashMap<(String, u64), String>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// POST with Roblox's CSRF handshake: a 403 with a fresh x-csrf-token is replayed once. Only that refusal:
/// any other 403 (wrong password, locked account, a check) is Roblox's answer, and repeating it could send a
/// password twice.
fn post_with_csrf(url: &str, cookie: &str, json_body: &str, configure: Option<&dyn Fn(&mut HttpRequest)>, ct: &Cancel, realm: &str) -> Res<HttpResponse> {
    send_with_csrf("POST", url, cookie, Some(json_body), configure, ct, realm)
}

fn send_with_csrf(method: &'static str, url: &str, cookie: &str, json_body: Option<&str>, configure: Option<&dyn Fn(&mut HttpRequest)>, ct: &Cancel, realm: &str) -> Res<HttpResponse> {
    // The token stays valid for a while, so it is reused for the same session instead of paying the 403 each time. Calls
    // without a sign-in share one per Roblox: Quick Login's status, polled every 2.5 s, paid a 403 and a replay on each
    // (review, 2026-10-30). A stale one is only that same 403 and replay.
    let session = (realm.to_string(), if cookie.is_empty() { 0 } else { hash_of(cookie) });
    let mut token = lock(&CSRF_TOKENS).get(&session).cloned();
    let mut attempt = 0;
    loop {
        let mut req = request(method, url, Some(cookie), true, realm);
        if let Some(token) = &token {
            req.add_header("x-csrf-token", token);
        }
        if let Some(configure) = configure {
            configure(&mut req);
        }
        if let Some(json_body) = json_body {
            req.set_json_body(json_body);
        }
        // Cancelled meanwhile: nothing more goes out; the same failure a cancelled request gives.
        if ct.is_cancelled() {
            return Err(fail_of(TASK_CANCELED_EXCEPTION, "The request was canceled."));
        }
        let res = send(&req, ct)?;
        let fresh = header(&res, "x-csrf-token");
        // The body is buffered, so reading it here leaves it for the caller.
        if attempt == 0
            && res.status == 403
            && let Some(fresh) = fresh.filter(|f| !f.is_empty() && Some(f) != token.as_ref())
            && is_csrf_refusal(res.status, Some(&fresh), token.as_deref(), header(&res, "rblx-challenge-id").as_deref(), &res.text())
        {
            lock(&CSRF_TOKENS).insert(session.clone(), fresh.clone());
            token = Some(fresh);
            attempt += 1;
            continue;
        }
        return Ok(res);
    }
}

fn hash_of(text: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}

/// Roblox's CSRF refusal and nothing else: 403, a new x-csrf-token, no challenge, no error code but 0, and the
/// "Token Validation Failed" message (apis.roblox.com's services say "XSRF token invalid") — which may be missing only
/// when no token was sent at all: then the request can't have got past the token check to the password. Without the
/// second wording a stale token kept for the session was never replaced there: every private server link failed with
/// that 403 until Bloxkeep restarted (a user's error log, 2026-11-06).
pub(crate) fn is_csrf_refusal(status: u16, fresh: Option<&str>, sent: Option<&str>, challenge_id: Option<&str>, body: &str) -> bool {
    if status != 403 || fresh.is_none_or(str::is_empty) || fresh == sent || challenge_id.is_some_and(|c| !c.is_empty()) {
        return false;
    }
    errors(body).0.iter().all(|c| *c == 0)
        && (sent.is_none() || contains_ci(body, "Token Validation Failed") || contains_ci(body, "XSRF token invalid"))
}

// ---- JsonElement's ways, on serde_json's Value: the same calls fail where .NET's threw ----

fn kind_name(v: &Value) -> &'static str {
    match v {
        Value::Object(_) => "Object",
        Value::Array(_) => "Array",
        Value::String(_) => "String",
        Value::Number(_) => "Number",
        Value::Bool(true) => "True",
        Value::Bool(false) => "False",
        Value::Null => "Null",
    }
}

fn wrong_kind(wanted: &str, v: &Value) -> Fail {
    fail_of(
        INVALID_OPERATION_EXCEPTION,
        format!("The requested operation requires an element of type '{wanted}', but the target element has type '{}'.", kind_name(v)),
    )
}

/// JsonDocument.Parse.
pub(crate) fn parse_json(text: &str) -> Res<Value> {
    serde_json::from_str(text).map_err(|e| fail_of(JSON_EXCEPTION, e))
}

/// TryGetProperty: fails on an element that isn't an object.
pub(crate) fn try_prop<'a>(v: &'a Value, name: &str) -> Res<Option<&'a Value>> {
    match v {
        Value::Object(o) => Ok(o.get(name)),
        _ => Err(wrong_kind("Object", v)),
    }
}

/// GetProperty: fails on an element that isn't an object, or without that property.
pub(crate) fn get_prop<'a>(v: &'a Value, name: &str) -> Res<&'a Value> {
    try_prop(v, name)?.ok_or_else(|| fail_of(KEY_NOT_FOUND_EXCEPTION, format!("The given key '{name}' was not present in the dictionary.")))
}

/// GetString: None for null, fails for anything but a string.
pub(crate) fn get_str(v: &Value) -> Res<Option<String>> {
    match v {
        Value::Null => Ok(None),
        Value::String(s) => Ok(Some(s.clone())),
        _ => Err(wrong_kind("String", v)),
    }
}

/// GetInt64.
pub(crate) fn get_i64(v: &Value) -> Res<i64> {
    try_i64(v)?.ok_or_else(|| fail_of(FORMAT_EXCEPTION, "The JSON value is not in a supported format."))
}

/// TryGetInt64: fails for anything but a number.
pub(crate) fn try_i64(v: &Value) -> Res<Option<i64>> {
    match v {
        Value::Number(n) => Ok(n.as_i64()),
        _ => Err(wrong_kind("Number", v)),
    }
}

/// TryGetInt32: fails for anything but a number.
pub(crate) fn try_i32(v: &Value) -> Res<Option<i32>> {
    Ok(try_i64(v)?.and_then(|n| i32::try_from(n).ok()))
}

/// GetInt32.
pub(crate) fn get_i32(v: &Value) -> Res<i32> {
    try_i32(v)?.ok_or_else(|| fail_of(FORMAT_EXCEPTION, "The JSON value is not in a supported format."))
}

/// GetDouble.
pub(crate) fn get_f64(v: &Value) -> Res<f64> {
    match v {
        Value::Number(n) => Ok(n.as_f64().unwrap_or(0.0)),
        _ => Err(wrong_kind("Number", v)),
    }
}

/// EnumerateArray: fails on an element that isn't an array.
pub(crate) fn as_array(v: &Value) -> Res<&Vec<Value>> {
    match v {
        Value::Array(a) => Ok(a),
        _ => Err(wrong_kind("Array", v)),
    }
}

/// EnumerateObject: fails on an element that isn't an object.
pub(crate) fn as_object(v: &Value) -> Res<&serde_json::Map<String, Value>> {
    match v {
        Value::Object(o) => Ok(o),
        _ => Err(wrong_kind("Object", v)),
    }
}

/// GetRawText (written again from the value: the same JSON, its spacing normalised).
pub(crate) fn raw_text(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

/// ValueKind.ToString().ToLowerInvariant().
pub(crate) fn kind_lower(v: &Value) -> String {
    kind_name(v).to_ascii_lowercase()
}

// ---- small things of .NET's the layer leans on ----

/// Uri.EscapeDataString: everything but RFC 3986's unreserved characters percent-encoded, as UTF-8.
pub fn escape_data_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Uri.UnescapeDataString: %XX runs that make UTF-8 are decoded; any other is left as it is.
pub fn unescape_data_string(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let hex = |c: u8| (c as char).to_digit(16);
    while i < bytes.len() {
        if bytes[i] == b'%' {
            // A run of escapes, decoded together (a character may take several).
            let start = i;
            let mut run = Vec::new();
            while i + 2 < bytes.len() && bytes[i] == b'%' {
                match (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                    (Some(h), Some(l)) => {
                        run.push((h * 16 + l) as u8);
                        i += 3;
                    }
                    _ => break,
                }
            }
            if run.is_empty() {
                out.push('%');
                i = start + 1;
                continue;
            }
            match String::from_utf8(run) {
                Ok(text) => out.push_str(&text),
                Err(_) => out.push_str(&s[start..i]),
            }
        } else {
            let ch = s[i..].chars().next().unwrap_or('\u{FFFD}');
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// IndexOf(…, OrdinalIgnoreCase) for an ASCII needle: the byte position in `hay`.
pub fn find_ci(hay: &str, needle: &str) -> Option<usize> {
    hay.to_ascii_lowercase().find(&needle.to_ascii_lowercase())
}

pub fn contains_ci(hay: &str, needle: &str) -> bool {
    hay.to_lowercase().contains(&needle.to_lowercase())
}

pub fn starts_with_ci(s: &str, prefix: &str) -> bool {
    s.to_lowercase().starts_with(&prefix.to_lowercase())
}

pub fn ends_with_ci(s: &str, suffix: &str) -> bool {
    s.to_lowercase().ends_with(&suffix.to_lowercase())
}

pub fn eq_ci(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// DateTime.TryParse with AssumeUniversal | AdjustToUniversal: a moment written without a zone is UTC.
pub(crate) fn parse_assume_utc(text: &str) -> Option<Stamp> {
    let stamp = Stamp::parse(text)?;
    if stamp.kind() == crate::core::base::Kind::Unspecified { Stamp::parse(&format!("{}Z", text.trim())).map(Stamp::to_utc) } else { Some(stamp.to_utc()) }
}
