// Tests/Bloxkeep.Tests/BotCheckSuite.cs: "Solve the bot check" on a window Roblox holds for its check
// (app/bot_check.rs). The join asked with the account's sign-in gets the check (rblx-challenge-* headers), a proof of work
// is computed and traded for its token, the answer goes to Roblox's challenge/continue with the account's cookie, the
// join is asked again carrying it, and once let in the held window is closed and opened again into its game. A kind not
// handled is said and nothing is sent. Roblox here is a stand-in (roblox_api::STAND_IN); no window is closed.
use std::{
    cell::RefCell,
    collections::HashSet,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use base64::Engine;
use serde_json::{Value, json};

use super::{
    Checks, call,
    rejoin::{no_network, quiet_app, refusal, seed},
    unlock,
};
use crate::core::{
    app::{bot_check::captcha_answer, controller::PendingLaunch},
    base::{Cancel, Stamp},
    game_target::GameTarget,
    models::Account,
    roblox_api::{self, HttpRequest, HttpResponse, JoinChallenge},
    roblox_log_watcher::{ClientState, Tail},
    rt,
};

const PLACE: i64 = 4242;
const PID: i32 = 777001;

fn json_answer(status: u16, body: &str) -> HttpResponse {
    HttpResponse { status, headers: vec![("Content-Type".into(), "application/json; charset=utf-8".into())], body: body.as_bytes().to_vec() }
}

#[derive(Default)]
struct Roblox {
    kind: Mutex<String>,
    still_held: AtomicBool,
    continue_refuses: AtomicBool,
    seen: Mutex<Vec<String>>,
    continued: Mutex<Option<String>>,
    continue_cookie: Mutex<Option<String>>,
    retry_header: Mutex<Option<String>>,
}

impl Roblox {
    fn answer(&self, req: &HttpRequest) -> HttpResponse {
        let rest = req.url.split("://").nth(1).unwrap_or("");
        let path = rest[rest.find('/').unwrap_or(rest.len())..].split('?').next().unwrap_or("").to_string();
        self.seen.lock().unwrap().push(path.clone());
        let body = req.body.as_ref().map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default();
        if path.ends_with("/v1/join-game") {
            if let Some(id) = req.headers_named("rblx-challenge-id").next()
                && !self.still_held.load(Ordering::SeqCst)
            {
                *self.retry_header.lock().unwrap() = Some(id.to_string());
                return json_answer(200, r#"{"status":2,"message":"ok"}"#);
            }
            let mut held = json_answer(403, r#"{"errors":[{"code":0,"message":"Challenge is required to authorize the request"}]}"#);
            held.headers.push(("rblx-challenge-id".into(), "check-1".into()));
            held.headers.push(("rblx-challenge-type".into(), self.kind.lock().unwrap().clone()));
            held.headers.push((
                "rblx-challenge-metadata".into(),
                base64::engine::general_purpose::STANDARD.encode(r#"{"sessionId":"pow-1","userId":"31"}"#),
            ));
            return held;
        }
        if path.ends_with("/pow-puzzle") && req.method == "GET" {
            return json_answer(200, r#"{"puzzleType":"1","artifacts":"{\"N\":\"77\",\"A\":\"5\",\"T\":\"3\"}"}"#);
        }
        if path.ends_with("/pow-puzzle") {
            // 5 squared three times modulo 77 is 4.
            return json_answer(
                200,
                if body.contains("\"solution\":\"4\"") { r#"{"answerCorrect":true,"redemptionToken":"token-1"}"# } else { r#"{"answerCorrect":false}"# },
            );
        }
        if path.ends_with("/captcha/v1/metadata") {
            return json_answer(200, r#"{"funCaptchaPublicKeys":{"ACTION_TYPE_WEB_LOGIN":"key-login","ACTION_TYPE_GENERIC_CHALLENGE":"key-generic"}}"#);
        }
        if path.ends_with("/challenge/v1/continue") {
            *self.continued.lock().unwrap() = Some(body);
            if self.continue_refuses.load(Ordering::SeqCst) {
                return json_answer(403, r#"{"errors":[{"code":1,"message":"an internal error occurred"}]}"#);
            }
            let cookies: Vec<&str> = req.headers_named("Cookie").collect();
            *self.continue_cookie.lock().unwrap() = if cookies.is_empty() { None } else { Some(cookies.join(";")) };
            return json_answer(
                200,
                r#"{"challengeId":"check-1","challengeType":"proofofwork","challengeMetadata":"{\"sessionId\":\"pow-1\",\"redemptionToken\":\"token-1\"}"}"#,
            );
        }
        json_answer(404, "{}")
    }
}

pub async fn run(c: &mut Checks, _dir: &str) {
    no_network();
    let held_account = Account { username: "Held".into(), user_id: 31, cookie: "cookie-held".into(), cookie_valid: Some(true), ..Default::default() };
    let id = held_account.id;
    seed(vec![held_account]);
    let (app, _said) = quiet_app();
    unlock(&app).await;

    // The held window: its log says joining, held for the check.
    let mut tail = Tail::new("stand-in.log".into(), PID);
    tail.state = Some(ClientState::new("joining", Some(PLACE), None, None, Stamp::utc_now()));
    app.controller.logs.borrow_mut().tails.insert(PID, tail);
    let held = |on: bool| {
        if let Some(t) = app.controller.logs.borrow_mut().tails.get_mut(&PID) {
            t.join_challenged = if on { Some(Stamp::utc_now()) } else { None };
        }
    };
    let launch = Rc::new(PendingLaunch::new(id, HashSet::new(), Stamp::utc_now().plus_secs(-60.0), Some(GameTarget::new(PLACE, None, None, None)), false, None));
    app.controller.running.borrow_mut().insert(id, (PID, launch));
    let closed: Rc<RefCell<Vec<i32>>> = Rc::new(RefCell::new(Vec::new()));
    let real_close = app.bot_check.close_for_reopen.borrow().clone();
    {
        let closed = closed.clone();
        *app.bot_check.close_for_reopen.borrow_mut() = Some(Rc::new(move |pid, _| closed.borrow_mut().push(pid)));
    }
    let closed_now = || closed.borrow().clone();

    let roblox = Arc::new(Roblox { kind: Mutex::new("proofofwork".into()), ..Default::default() });
    {
        let r = roblox.clone();
        *roblox_api::STAND_IN.write().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(move |req| Ok(r.answer(req))));
    }
    let seen = || roblox.seen.lock().unwrap().clone();
    let solve = || call(&app, "solveBotCheck", json!({ "id": id }));

    held(false);
    let not_held = solve().await;
    c.check("a window not held by a check: said, nothing asked", refusal(&not_held).is_some() && seen().is_empty(), format!("{not_held:?}"));

    held(true);
    let unknown = "unknownkind";
    *roblox.kind.lock().unwrap() = unknown.into();
    let refused = solve().await;
    c.check(
        "a kind of check not handled: said with its kind, nothing answered",
        refusal(&refused).is_some_and(|m| m.contains(unknown)) && roblox.continued.lock().unwrap().is_none() && closed_now().is_empty(),
        format!("{refused:?}"),
    );

    *roblox.kind.lock().unwrap() = "proofofwork".into();
    roblox.still_held.store(true, Ordering::SeqCst);
    roblox.seen.lock().unwrap().clear();
    let not_let = solve().await;
    c.check(
        "answered but Roblox still holds the join: said, the window left as it is",
        refusal(&not_let).is_some() && closed_now().is_empty(),
        format!("{not_let:?}"),
    );

    roblox.still_held.store(false, Ordering::SeqCst);
    roblox.seen.lock().unwrap().clear();
    *roblox.continued.lock().unwrap() = None;
    let done = solve().await;
    c.check(
        "a proof of work: computed and traded for its token",
        refusal(&done).is_none() && seen().iter().filter(|p| p.ends_with("/pow-puzzle")).count() == 2,
        seen().join(","),
    );
    let continued = roblox.continued.lock().unwrap().clone();
    let cookie = roblox.continue_cookie.lock().unwrap().clone();
    c.check(
        "…the answer sent to Roblox with the account's sign-in, carrying the token",
        continued.as_ref().is_some_and(|b| b.contains("token-1") && b.contains("check-1")) && cookie.as_ref().is_some_and(|k| k.contains("cookie-held")),
        &continued,
    );
    let retry = roblox.retry_header.lock().unwrap().clone();
    c.check("…the join asked again carrying the check", retry.as_deref() == Some("check-1"), &retry);
    c.check(
        "…let in: the held window closed and opened again into its game",
        closed_now() == [PID] && app.presets.restarts.borrow().contains_key(&id),
        format!("{:?}", closed_now()),
    );

    // A join's check as Roblox answered it on 2026-10-27: continue refused (403, "an internal error occurred"). The join
    // itself is asked again carrying the answer.
    roblox.continue_refuses.store(true, Ordering::SeqCst);
    closed.borrow_mut().clear();
    app.presets.restarts.borrow_mut().clear();
    *roblox.retry_header.lock().unwrap() = None;
    held(true);
    let via_join = solve().await;
    let retry = roblox.retry_header.lock().unwrap().clone();
    c.check(
        "continue refused: the join asked again carrying the answer, and once let in the window reopens",
        refusal(&via_join).is_none() && retry.as_deref() == Some("check-1") && closed_now() == [PID],
        format!("{via_join:?}"),
    );
    roblox.continue_refuses.store(false, Ordering::SeqCst);

    // The puzzle kind's answer: the check's whole metadata (as a join's check carries it, 2026-10-27), token filled in.
    let check = JoinChallenge {
        id: "check-2".into(),
        r#type: "captcha".into(),
        metadata_json: r#"{"captchaToken":"","unifiedCaptchaId":"u-1","dataExchangeBlob":"blob","actionType":"Generic","requestPath":"/v1/join-game","requestMethod":"POST","sharedParameters":"{}"}"#.into(),
    };
    let answer: Value = captcha_answer(&check, "solved-token").ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
    c.check(
        "a solved puzzle goes back with every field of the check, its token filled in",
        answer["captchaToken"] == "solved-token"
            && answer["requestPath"] == "/v1/join-game"
            && answer["sharedParameters"] == "{}"
            && answer["dataExchangeBlob"] == "blob"
            && answer["unifiedCaptchaId"] == "u-1"
            && answer["actionType"] == "Generic",
        answer.to_string(),
    );

    // The puzzle kind's key, from Roblox's captcha settings (real names, 2026-10-27): there is none for a join.
    let key = |action: &'static str| rt::blocking(move || roblox_api::get_captcha_key("", action, &Cancel::new()).ok().flatten());
    let (login, generic, join) = (key("WebLogin").await, key("Generic").await, key("JoinGame").await);
    c.check(
        "the captcha key: WebLogin's own, Generic to the generic challenge's, a join's to the generic one",
        login.as_deref() == Some("key-login") && generic.as_deref() == Some("key-generic") && join.as_deref() == Some("key-generic"),
        format!("{login:?} {generic:?} {join:?}"),
    );

    app.controller.running.borrow_mut().clear();
    app.presets.restarts.borrow_mut().clear();
    *app.bot_check.close_for_reopen.borrow_mut() = real_close;
    no_network();

    // (Rust only, 2026-11-12.) The captcha window passes HUMAN's requests on to HUMAN: only HUMAN's own cookies go
    // with them, never the page's other roblox.com cookies.
    let kept = crate::core::forms::captcha_form::px_cookies("RBXEventTrackerV2=CreateDate=1&browserid=9; _pxvid=abc; GuestData=UserID=-1; pxcts=12; _px3=zz");
    c.check("the captcha's relay sends HUMAN only HUMAN's own cookies", kept.as_deref() == Some("_pxvid=abc; pxcts=12; _px3=zz"), &kept);
    let none = crate::core::forms::captcha_form::px_cookies("RBXEventTrackerV2=x; GuestData=y");
    c.check("…and no Cookie at all when the page has none of them", none.is_none(), &none);
}
