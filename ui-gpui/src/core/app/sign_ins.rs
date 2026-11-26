//! AppController.SignIns.cs: keeping saved sign-ins working. Roblox replaces an account's .ROBLOSECURITY every few hours to
//! days and, months later, refuses the replaced one (devforum.roblox.com/t/4328913: announced 2026-02-03, enforced from
//! 2026-05-01). The new cookie comes in the Set-Cookie of an answer to the old one (RobloxApi queues it) and is saved once
//! Roblox says it's the same user. Bloxkeep never asks for one (auth's /v1/session/refresh): that ends the session it renews,
//! signing out a browser or another app sharing the account.
use std::rc::Rc;
use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;

use super::App;
use crate::core::base::{Cancel, Fail, Stamp};
use crate::core::models::Account;
use crate::core::roblox_api::{self, RenewedCookie, RobloxUser, realm};
use crate::core::{log, rt};

/// Tries at asking Roblox whose a renewed cookie is, before it is let go.
pub const RENEWED_CHECK_TRIES: i32 = 4;

static HTTP_STATUS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"HTTP (\d{3})").unwrap());

/// Why a request got no answer, in a few fixed words for error.log (never the message itself: it starts with the
/// address, which can hold an account's id). ureq writes "<url>: <kind>[: message][: source]", so only what follows its
/// kind is looked at; an io error from reading the answer has no address.
pub fn network_reason(message: &str) -> &'static str {
    const KINDS: [&str; 13] = [
        "Dns Failed",
        "Connection Failed",
        "Network Error",
        "Too Many Redirects",
        "Bad Status",
        "Bad Header",
        "Bad URL",
        "Unknown Scheme",
        "Proxy failed to connect",
        "Malformed proxy",
        "Provided proxy credentials are incorrect",
        "HTTP status error",
        "Insecure request",
    ];
    let tail = KINDS.iter().filter_map(|k| message.find(k)).min().map_or(message, |at| &message[at..]);
    let low = tail.to_ascii_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| low.contains(w));
    if has(&["dns failed", "os error 11001", "os error 11004", "no such host", "host not known", "name or service not known"]) {
        "name lookup failed"
    } else if has(&["proxy"]) {
        "proxy failed"
    } else if has(&["certificate", "tls", "ssl"]) || (low.contains("handshake") && !low.contains("closing handshake")) {
        "secure connection failed"
    } else if has(&["os error 10054", "connection reset", "forcibly closed"]) {
        "connection reset by the other side"
    } else if has(&["os error 10053", "connection aborted", "aborted by the software"]) {
        "connection aborted on this PC"
    } else if has(&["os error 10061", "connection refused", "actively refused"]) {
        "connection refused"
    } else if has(&["os error 10060", "timed out", "timeout"]) {
        "no answer in time"
    } else if has(&["os error 10051", "os error 10065", "unreachable"]) {
        "network unreachable"
    } else if has(&["unexpected eof", "end of file", "connection closed", "broken pipe"]) {
        "connection closed early"
    } else if has(&["connection failed"]) {
        "could not connect"
    } else if has(&["too many redirects"]) {
        "too many redirects"
    } else if has(&["bad status", "bad header", "http status error"]) {
        "bad answer"
    } else if has(&["bad url", "unknown scheme", "insecure request"]) {
        "bad address"
    } else {
        "cause not given"
    }
}

impl App {
    /// An error's kind for error.log: its type and HTTP status, never its message text (it can hold a name).
    pub fn log_kind_of(ex: &Fail) -> String {
        match ex {
            Fail::Roblox { message, .. } => {
                let kind = if ex.rate_limited() {
                    "rate limit"
                } else if ex.cookie_invalid() {
                    "signed out"
                } else if ex.moderated() {
                    "moderated"
                } else if ex.server_error() {
                    "server error"
                } else if ex.sign_in_refused() {
                    "sign-in refused"
                } else {
                    "refusal"
                };
                let status = HTTP_STATUS.captures(message).map(|m| format!(" HTTP {}", &m[1])).unwrap_or_default();
                format!("Roblox API {kind}{status}")
            }
            // PORT: HttpRequestException's status code and HttpRequestError aren't kept by Fail; its kind is said instead.
            _ if roblox_api::is_fail_kind(ex, roblox_api::TASK_CANCELED_EXCEPTION) => "timeout".into(),
            // With its cause, as the C# app wrote HttpRequestError (NameResolutionError…): the Rust port wrote a bare
            // "network error", so a report couldn't tell a PC's DNS failing from a server dropping it (2026-11-17).
            Fail::Other(m) if roblox_api::is_fail_kind(ex, roblox_api::HTTP_REQUEST_EXCEPTION) => format!("network error ({})", network_reason(m)),
            _ => roblox_api::fail_type_name(ex).to_string(),
        }
    }

    /// From the process timer: the cookies Roblox renewed in its answers go to their accounts.
    pub fn save_renewed_sign_ins(self: &Rc<Self>) {
        if self.vault_ref().is_some() && roblox_api::renewed_cookies_waiting() {
            let app = self.clone();
            rt::spawn(async move { app.save_renewed_sign_ins_async().await });
        }
    }

    pub async fn save_renewed_sign_ins_async(self: &Rc<Self>) {
        // Background work must never take the app down: each save ends on its own, its failure only noted.
        let mut saves = Vec::new();
        while let Some(renewed) = roblox_api::try_take_renewed_cookie() {
            saves.push(self.save_renewed_sign_in(renewed));
        }
        futures::future::join_all(saves).await;
    }

    /// Saves a renewed cookie to the account(s) holding the old one, once Roblox says it signs in that same user. One no
    /// account holds (deleted, another vault's, replaced meanwhile) is let go.
    pub async fn save_renewed_sign_in(self: &Rc<Self>, renewed: RenewedCookie) {
        let holds = |a: &Account| !a.cookie.is_empty() && a.cookie == renewed.sent && realm::normalize(Some(&a.realm)) == renewed.realm;
        let Some(vault) = self.vault_ref() else { return };
        if !vault.data().accounts.iter().any(holds) {
            return;
        }
        let mut user: Option<RobloxUser>;
        let mut attempt = 1;
        loop {
            let (c, r) = (renewed.renewed.clone(), renewed.realm.clone());
            match rt::blocking(move || roblox_api::get_authenticated_user(&c, &r, &Cancel::new())).await {
                Ok(u) => {
                    user = u;
                    break;
                }
                // Offline or rate-limited: asked again a little later. Not a notice to accept (403 "User is moderated"): it stays until
                // the person accepts, so asking again only adds requests (a real PC: 46 in five minutes).
                Err(ex) if roblox_api::is_expected_error(&ex) && attempt < RENEWED_CHECK_TRIES && !ex.moderated() => {
                    rt::sleep(Duration::from_secs(10 * (attempt * attempt) as u64)).await;
                    if !self.is_vault(&vault) {
                        return;
                    }
                }
                Err(ex) if roblox_api::is_expected_error(&ex) => {
                    log::note(&format!(
                        "Roblox renewed an account's sign-in, but couldn't be asked whose it is ({}, try {attempt}); the saved one is kept.",
                        App::log_kind_of(&ex)
                    ));
                    return;
                }
                Err(ex) => {
                    // Background work must never take the app down.
                    log::write(&ex);
                    return;
                }
            }
            attempt += 1;
        }
        if !self.is_vault(&vault) {
            return;
        }
        let saved = {
            let mut d = vault.data();
            let holding: Vec<usize> = d.accounts.iter().enumerate().filter(|(_, a)| holds(a)).map(|(i, _)| i).collect();
            match user.take() {
                Some(user) if !holding.is_empty() && holding.iter().any(|i| d.accounts[*i].user_id == user.id) => {
                    for i in holding {
                        let account = &mut d.accounts[i];
                        if account.user_id != user.id {
                            continue;
                        }
                        account.cookie = renewed.renewed.clone();
                        account.cookie_valid = Some(true);
                        account.cookie_renewed = Some(Stamp::now());
                        account.last_checked = Some(Stamp::now());
                        account.username = user.name.clone();
                        account.display_name = user.display_name.clone();
                        // A request with the old cookie after the swap may have been refused and the account taken for signed out.
                        App::signed_in_again(account);
                    }
                    true
                }
                _ => {
                    if !holding.is_empty() {
                        log::note("Roblox sent a sign-in for another account, or one that doesn't work; the saved one is kept.");
                    }
                    false
                }
            }
        };
        if !saved {
            return;
        }
        self.persist(); // on disk at once: the cookie it replaced may stop working any time now
        self.push_state();
        log::note("Roblox renewed an account's sign-in; saved.");
    }
}
