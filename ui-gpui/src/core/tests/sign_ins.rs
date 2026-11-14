// Tests/Bloxkeep.Tests/SignInsSuite.cs: saved sign-ins kept up to date as Roblox renews them (app/sign_ins.rs, roblox_api's
// cookie watch): a new .ROBLOSECURITY in the answer to any request made with an account's cookie is saved to that account,
// on disk at once, but only when Roblox says it signs in that same user; a cookie deleted or already expired, or set in an
// answer to a request without a cookie, changes nothing. Bloxkeep never renews one on purpose (auth's /v1/session/refresh
// ends the session it renews). Roblox here is a stand-in answering in this process (roblox_api::STAND_IN), with made-up
// cookies: nothing is asked of Roblox's servers.
use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    path::Path,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use serde_json::{Value, json};

use super::{
    Checks, PASSWORD, call,
    rejoin::{acc, no_network, quiet_app, refusal, seed},
    unlock, user_error,
};
use crate::core::{
    app::App,
    base::{Cancel, Guid, Res, Stamp, new_guid},
    models::Account,
    roblox_api::{self, HttpRequest, HttpResponse, realm},
    rt,
    vault::Vault,
};

#[derive(Clone, Debug)]
struct Request {
    host: String,
    path: String,
    cookie: Option<String>,
    csrf: bool,
    body: String,
    like_client: bool,
}

/// Who each made-up cookie signs in, and a new cookie to set in the next answer to a request carrying a given one.
#[derive(Default)]
struct Inner {
    users: HashMap<String, i64>,
    rotate: HashMap<String, String>,
    /// Cookies of accounts Roblox holds: 403 to every question, as for a warning not yet accepted.
    held: HashSet<String>,
    /// Who is in a game (the place), as Roblox's presence service and Roblox VNG's each say.
    in_game: HashMap<(bool, i64), i64>,
}

/// Roblox as far as these checks go.
#[derive(Default)]
struct Roblox {
    inner: Mutex<Inner>,
    /// A Set-Cookie header added to every answer while set.
    extra: Mutex<Option<String>>,
    /// Roblox's sign-in service down: "who am I" rejects every cookie.
    down: AtomicBool,
    /// Roblox VNG's sign-in service down only.
    down_vng: AtomicBool,
    /// While set, an account's favourite games are answered only once it is cleared.
    gated: AtomicBool,
    asked: Mutex<Vec<Request>>,
}

fn json_answer(status: u16, body: Value) -> HttpResponse {
    HttpResponse { status, headers: vec![("Content-Type".into(), "application/json; charset=utf-8".into())], body: body.to_string().into_bytes() }
}

fn refusal_answer(status: u16, message: &str) -> HttpResponse {
    json_answer(status, json!({ "errors": [{ "code": 0, "message": message }] }))
}

fn csrf_answer() -> HttpResponse {
    let mut res = refusal_answer(403, "Token Validation Failed");
    res.headers.push(("x-csrf-token".into(), "stand-in-csrf".into()));
    res
}

fn ticket_answer() -> HttpResponse {
    let mut res = json_answer(200, json!({}));
    res.headers.push(("rbx-authentication-ticket".into(), "stand-in-ticket".into()));
    res
}

/// .NET's "R" format (RFC 1123), as the C#'s `DateTime.UtcNow.AddDays(30):R`.
fn rfc1123(at: Stamp) -> String {
    const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let c = at.utc();
    format!("{}, {:02} {} {:04} {:02}:{:02}:{:02} GMT", DAYS[c.weekday as usize], c.day, MONTHS[(c.month - 1) as usize], c.year, c.hour, c.minute, c.second)
}

impl Roblox {
    fn answer(&self, req: &HttpRequest) -> HttpResponse {
        let url = url::Url::parse(&req.url).ok();
        let host = url.as_ref().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_default();
        let path = url.as_ref().map(|u| u.path().to_string()).unwrap_or_default();
        let cookie = req.headers_named("Cookie").next().map(|h| h.replace(".ROBLOSECURITY=", ""));
        let csrf = req.headers_named("x-csrf-token").next().is_some();
        let text = req.body.as_ref().map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default();
        self.asked.lock().unwrap().push(Request { host: host.clone(), path: path.clone(), cookie: cookie.clone(), csrf, body: text.clone(), like_client: req.like_roblox_client });
        if path.ends_with("/favorite/games") {
            while self.gated.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(20));
            }
        }

        let vng = host.ends_with(".vnggames.com");
        let mut res = {
            let mut inner = self.inner.lock().unwrap();
            let user = cookie.as_ref().and_then(|c| inner.users.get(c).copied());
            let mut res = if cookie.as_ref().is_some_and(|c| inner.held.contains(c)) && (csrf || !path.starts_with("/v1/authentication-ticket")) {
                refusal_answer(403, "User is moderated")
            } else {
                match path.as_str() {
                    "/v1/users/authenticated" => match user {
                        Some(u) if !self.down.load(Ordering::SeqCst) && !(self.down_vng.load(Ordering::SeqCst) && vng) => {
                            json_answer(200, json!({ "id": u, "name": format!("user{u}"), "displayName": format!("User {u}") }))
                        }
                        _ => refusal_answer(401, "Authorization has been denied for this request."),
                    },
                    "/v1/authentication-ticket/" if !csrf => csrf_answer(),
                    "/v1/authentication-ticket/" => ticket_answer(),
                    "/v1/join-game" if !csrf => csrf_answer(),
                    "/v1/join-game" => json_answer(200, json!({ "status": 12, "message": "Unable to join Game 211" })),
                    "/v1/presence/users" if !csrf => csrf_answer(),
                    "/v1/presence/users" => json_answer(200, json!({ "userPresences": presences_in(&inner, &text, vng) })),
                    _ if path.ends_with("/currency") => json_answer(200, json!({ "robux": 0 })),
                    _ if host.starts_with("thumbnails.") => json_answer(200, json!({ "data": [] })),
                    _ => json_answer(404, json!({})),
                }
            };
            // Only an answer that went through (not the CSRF refusal before it) renews, as Roblox's would.
            if let Some(c) = &cookie
                && res.status != 403
                && let Some(next) = inner.rotate.remove(c)
            {
                let domain = if host.ends_with(".robloxapp.vnggames.com") { ".robloxapp.vnggames.com" } else { ".roblox.com" };
                res.headers.push((
                    "Set-Cookie".into(),
                    format!(".ROBLOSECURITY={next}; domain={domain}; expires={}; path=/; secure; HttpOnly", rfc1123(Stamp::utc_now().plus_secs(30.0 * 86400.0))),
                ));
            }
            res
        };
        if let Some(extra) = self.extra.lock().unwrap().clone() {
            res.headers.push(("Set-Cookie".into(), extra));
        }
        res
    }

    fn asked(&self) -> Vec<Request> {
        self.asked.lock().unwrap().clone()
    }

    fn asked_with(&self, cookie: &str) -> Vec<Request> {
        self.asked().into_iter().filter(|r| r.cookie.as_deref() == Some(cookie)).collect()
    }

    fn clear(&self) {
        self.asked.lock().unwrap().clear();
    }

    fn user(&self, cookie: &str, id: i64) {
        self.inner.lock().unwrap().users.insert(cookie.into(), id);
    }

    fn rotate(&self, from: &str, to: &str) {
        self.inner.lock().unwrap().rotate.insert(from.into(), to.into());
    }

    fn set_extra(&self, header: Option<String>) {
        *self.extra.lock().unwrap() = header;
    }
}

fn presences_in(inner: &Inner, body: &str, vng: bool) -> Vec<Value> {
    let doc: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    doc["userIds"]
        .as_array()
        .map(|ids| {
            ids.iter()
                .filter_map(Value::as_i64)
                .filter_map(|id| inner.in_game.get(&(vng, id)).map(|place| (id, *place)))
                .map(|(id, place)| json!({ "userPresenceType": 2, "userId": id, "placeId": place, "rootPlaceId": place, "gameId": format!("server-{id}") }))
                .collect()
        })
        .unwrap_or_default()
}

/// Roblox's refusals are answers here too: what counts is the cookie the answer set.
fn quietly<T>(result: Res<T>) {
    if let Err(ex) = result
        && !roblox_api::is_expected_error(&ex)
    {
        panic!("{ex}");
    }
}

/// The renewals Roblox's answers brought, saved as the process timer saves them.
async fn renewals(app: &Rc<App>) {
    app.save_renewed_sign_ins_async().await;
}

async fn until(done: impl Fn() -> bool, seconds: u64) -> bool {
    let until = Instant::now() + Duration::from_secs(seconds);
    while Instant::now() < until {
        if done() {
            return true;
        }
        rt::sleep(Duration::from_millis(100)).await;
    }
    done()
}

/// Whether the core (outside the checks) defines a function of this name: the C# asked reflection for the method.
fn core_defines(name: &str) -> bool {
    fn walk(dir: &Path, needle: &str) -> bool {
        let Ok(entries) = std::fs::read_dir(dir) else { return false };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n != "tests") && walk(&path, needle) {
                    return true;
                }
            } else if path.extension().is_some_and(|e| e == "rs")
                && std::fs::read_to_string(&path).is_ok_and(|text| text.contains(&format!("fn {needle}(")) || text.contains(&format!("fn {needle}<")))
            {
                return true;
            }
        }
        false
    }
    walk(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("core"), name)
}

pub async fn run(c: &mut Checks, _dir: &str) {
    no_network();
    let mark: String = new_guid().to_string().replace('-', "")[..10].to_string();
    let roblox = Arc::new(Roblox::default());
    {
        let r = roblox.clone();
        *roblox_api::STAND_IN.write().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(move |req| Ok(r.answer(req))));
    }
    let cookie = |name: &str| format!("_TEST_{name}_{mark}");

    let mut accounts = Vec::new();
    let mut add = |name: &str, user_id: i64, works: bool, also: &dyn Fn(&mut Account)| -> Guid {
        let mut account = Account { username: name.into(), user_id, cookie: cookie(name), cookie_valid: Some(true), ..Default::default() };
        also(&mut account);
        if works {
            roblox.user(&account.cookie, user_id);
        }
        let id = account.id;
        accounts.push(account);
        id
    };
    let a = add("A", 2001, true, &|_| {});
    let b = add("B", 2002, true, &|_| {});
    let v = add("V", 2003, true, &|x| x.realm = realm::VNG.into());
    let s = add("S", 2004, false, &|x| x.cookie_valid = Some(false)); // signed out
    let _x = add("X", 2005, true, &|x| x.banned = Some(true));
    let cc = add("C", 2006, true, &|_| {});
    let w = add("W", 2007, true, &|_| {}); // open on the website
    let d = add("D", 2008, false, &|_| {}); // Roblox ended it, Bloxkeep doesn't know yet
    seed(accounts);

    let (app, _said) = quiet_app();
    app.power.network_up.set(true); // as on a PC online, whatever this one is
    unlock(&app).await;
    // Unlocking asks Roblox for status, avatars and Robux: those answers are let through first.
    until(|| app.controller.boot.borrow().is_none() && !app.controller.presence_busy.get() && !app.robux.robux_busy.get(), 30).await;

    body(c, &app, &roblox, &cookie, [a, b, v, s, cc, w, d]).await;

    // Every refusal and warning the person sees is in error.log, names scrubbed (owner, 2026-11-15: error.log must cover
    // every case, nobody is asked for a screenshot).
    {
        // (The checks above end locked: the names to scrub come from the open vault.)
        if app.controller.vault.borrow().is_none() {
            unlock(&app).await;
        }
        let error_log = std::path::Path::new(_dir).join("error.log");
        let read = || std::fs::read_to_string(&error_log).unwrap_or_default();
        app.handle_message(r#"{"id":901,"cmd":"loginPassword","args":{"username":"","password":"","realm":""}}"#).await;
        let text = read();
        c.check("a command refused with a message: the message is in error.log", text.contains("Command \"loginPassword\" refused: Enter the username and password."), last_line(&text));
        let name = "ScrubbedPlayer".to_string();
        let _ = app.vault_mut(|d| d.accounts.push(Account { username: name.clone(), user_id: 2099, ..Default::default() }));
        app.toast(&format!("{name}: Roblox closed again before its window opened."), "warn");
        let text = read();
        c.check(
            "a warning shown: in error.log, without the account's name",
            !name.is_empty() && text.contains("Shown (warn): [account]: Roblox closed again") && !text.contains(&format!("{name}: Roblox closed again")),
            last_line(&text),
        );
        // Locked again with its window still open: a warning about it still has the name taken out.
        let _ = app.lock();
        app.toast(&format!("{name}: Roblox closed while Bloxkeep was locked."), "warn");
        let text = read();
        c.check(
            "…and once Bloxkeep is locked again too",
            app.controller.vault.borrow().is_none() && text.contains("Shown (warn): [account]: Roblox closed while") && !text.contains(&format!("{name}: Roblox closed while")),
            last_line(&text),
        );
    }

    no_network();
}

fn last_line(text: &str) -> String {
    text.lines().filter(|l| !l.trim().is_empty()).last().unwrap_or_default().to_string()
}

async fn body(c: &mut Checks, app: &Rc<App>, roblox: &Arc<Roblox>, cookie: &dyn Fn(&str) -> String, ids: [Guid; 7]) {
    let [a, b, v, s, cc, w, d] = ids;
    let of = |id: Guid| acc(app, id);
    let on_disk = |id: Guid| -> String {
        Vault::open(PASSWORD, None).ok().and_then(|disk| disk.data().accounts.iter().find(|k| k.id == id).map(|k| k.cookie.clone())).unwrap_or_default()
    };
    let ct = Cancel::new;

    // ── A new cookie in an answer ──
    // Whatever the request made with the account's cookie, the new one in its answer is saved: one after another, each
    // answer renewing the cookie the one before saved.
    let asks = ["its Robux", "presence", "the sign-in check", "a sign-in ticket to open Roblox", "a join check"];
    for (i, what) in asks.iter().enumerate() {
        let before = of(a).cookie;
        let next = cookie(&format!("A{}", i + 2));
        roblox.user(&next, 2001);
        roblox.rotate(&before, &next);
        let k = before.clone();
        let result: Res<()> = rt::blocking(move || match i {
            0 => roblox_api::get_robux(&k, realm::GLOBAL, 2001, &ct()).map(drop),
            1 => roblox_api::get_presence(Some(&k), &[2001], realm::GLOBAL, &ct()).map(drop),
            2 => roblox_api::get_authenticated_user(&k, realm::GLOBAL, &ct()).map(drop),
            3 => roblox_api::get_auth_ticket(&k, &ct(), realm::GLOBAL).map(drop),
            _ => roblox_api::join_check(&k, 1818, None, realm::GLOBAL, &ct(), None).map(drop),
        })
        .await;
        quietly(result);
        renewals(app).await;
        let now = of(a).cookie;
        c.check(
            &format!("a new cookie in the answer to {what} is saved to the account"),
            now == next,
            if now == before { "kept the old one" } else { "another" },
        );
        if i > 0 {
            continue;
        }
        let disk = on_disk(a);
        c.check("…and on disk at once", disk == next, if disk == before { "the old one on disk" } else { "another" });
        // Only that check asks anything with the new cookie here (the Robux request went with the old one).
        c.check(
            "…once Roblox said the new one signs in that same user",
            roblox.asked_with(&next).iter().any(|r| r.path == "/v1/users/authenticated"),
            roblox.asked_with(&next).iter().map(|r| r.path.clone()).collect::<Vec<_>>().join(", "),
        );
    }
    // A sign-in ticket is bound to the address that asked for it, and Roblox's client redeems it without Windows' proxy
    // (a user's report, 2026-11-14: 403 at sign-in): asked the client's way; every other call as before.
    let asked = roblox.asked.lock().unwrap().iter().map(|r| (r.path.clone(), r.like_client)).collect::<Vec<_>>();
    let tickets: Vec<bool> = asked.iter().filter(|(p, _)| p == "/v1/authentication-ticket/").map(|(_, l)| *l).collect();
    c.check(
        "the sign-in ticket is asked the way Roblox's client connects (no Windows proxy), its CSRF try too",
        tickets.len() >= 2 && tickets.iter().all(|l| *l),
        &tickets,
    );
    c.check(
        "…and nothing else is",
        asked.iter().all(|(p, l)| !*l || p == "/v1/authentication-ticket/"),
        asked.iter().filter(|(_, l)| *l).map(|(p, _)| p.clone()).collect::<Vec<_>>(),
    );
    // libcurl's choice from the environment (Roblox's client): https_proxy first, all_proxy after, none for a host in
    // no_proxy; Windows' proxy setting never.
    let env = |pairs: &'static [(&'static str, &'static str)]| move |n: &str| pairs.iter().find(|(k, _)| *k == n).map(|(_, v)| v.to_string());
    let pick = |pairs: &'static [(&'static str, &'static str)]| crate::net::env_proxy_for("auth.roblox.com", env(pairs));
    c.check("no proxy in the environment: straight to Roblox", pick(&[]).is_none(), pick(&[]));
    c.check(
        "https_proxy before all_proxy",
        pick(&[("ALL_PROXY", "http://b:2"), ("https_proxy", "http://a:1")]).as_deref() == Some("http://a:1"),
        pick(&[("ALL_PROXY", "http://b:2"), ("https_proxy", "http://a:1")]),
    );
    c.check("all_proxy alone is taken", pick(&[("ALL_PROXY", "http://b:2")]).as_deref() == Some("http://b:2"), pick(&[("ALL_PROXY", "http://b:2")]));
    c.check(
        "no_proxy naming roblox.com (or *) skips it",
        pick(&[("https_proxy", "http://a:1"), ("no_proxy", "localhost,.roblox.com")]).is_none() && pick(&[("https_proxy", "http://a:1"), ("NO_PROXY", "*")]).is_none(),
        pick(&[("https_proxy", "http://a:1"), ("no_proxy", "localhost,.roblox.com")]),
    );
    c.check(
        "…but not a name that only ends the same (notroblox.com)",
        pick(&[("https_proxy", "http://a:1"), ("no_proxy", "x.notroblox.com,blox.com")]).as_deref() == Some("http://a:1"),
        pick(&[("https_proxy", "http://a:1"), ("no_proxy", "x.notroblox.com,blox.com")]),
    );

    let renewed = of(a).cookie_renewed;
    c.check("…and noted as renewed", renewed.is_some_and(|at| Stamp::now().secs_since(at) < 60.0), renewed);

    // Never another user's sign-in in an account's place, nor one that doesn't work.
    roblox.user(&cookie("B-other"), 2001);
    roblox.rotate(&of(b).cookie, &cookie("B-other"));
    let k = of(b).cookie;
    quietly(rt::blocking(move || roblox_api::get_robux(&k, realm::GLOBAL, 2002, &Cancel::new())).await);
    renewals(app).await;
    c.check("a new cookie that signs in another user is not saved", of(b).cookie == cookie("B") && on_disk(b) == cookie("B"), ());
    let held = of(b).cookie;
    roblox.rotate(&held, &cookie("B-dead"));
    let k = held.clone();
    quietly(rt::blocking(move || roblox_api::get_robux(&k, realm::GLOBAL, 2002, &Cancel::new())).await);
    renewals(app).await;
    c.check("…nor one Roblox doesn't accept", of(b).cookie == held, ());

    // A cookie deleted (signing out sets an empty one, expired in 1970) or already expired is no renewal, even when it
    // would sign in the account.
    let held = of(b).cookie;
    roblox.user(&cookie("B-expired"), 2002);
    for header in [
        ".ROBLOSECURITY=; domain=.roblox.com; expires=Thu, 01-Jan-1970 00:00:00 GMT; path=/; secure; HttpOnly".to_string(),
        format!(".ROBLOSECURITY={}; domain=.roblox.com; expires=Thu, 01-Jan-1970 00:00:00 GMT; path=/; secure; HttpOnly", cookie("B-expired")),
        format!(".ROBLOSECURITY={}; domain=.roblox.com; Max-Age=0; path=/; secure; HttpOnly", cookie("B-expired")),
    ] {
        roblox.set_extra(Some(header));
        let k = held.clone();
        quietly(rt::blocking(move || roblox_api::get_robux(&k, realm::GLOBAL, 2002, &Cancel::new())).await);
    }
    roblox.set_extra(None);
    renewals(app).await;
    c.check("a cookie deleted or already expired in the answer changes nothing", of(b).cookie == held, ());
    // An answer to a request made without a cookie (a sign-in, a public lookup) renews nobody's.
    let held = of(b).cookie;
    roblox.user(&cookie("anon"), 2002);
    roblox.set_extra(Some(format!(".ROBLOSECURITY={}; domain=.roblox.com; path=/; secure; HttpOnly", cookie("anon"))));
    quietly(rt::blocking(|| roblox_api::get_banned(2002, realm::GLOBAL, &Cancel::new())).await);
    roblox.set_extra(None);
    renewals(app).await;
    c.check("a cookie in the answer to a request without one renews no account", of(b).cookie == held, ());
    // Undone, should a check above have let one through, so the rest start from B's own sign-in.
    let own_b = cookie("B");
    app.update_account(b, |x| x.cookie = own_b);

    // Roblox VNG: its answer renews its account, checked with Roblox VNG.
    roblox.user(&cookie("V2"), 2003);
    roblox.rotate(&of(v).cookie, &cookie("V2"));
    let k = of(v).cookie;
    quietly(rt::blocking(move || roblox_api::get_robux(&k, realm::VNG, 2003, &Cancel::new())).await);
    renewals(app).await;
    c.check("Roblox VNG's new cookie is saved to its account", of(v).cookie == cookie("V2"), ());
    c.check(
        "…once Roblox VNG said whose it is",
        roblox.asked_with(&cookie("V2")).iter().any(|r| r.host == "users.robloxapp.vnggames.com"),
        roblox.asked_with(&cookie("V2")).iter().map(|r| r.host.clone()).collect::<Vec<_>>().join(", "),
    );

    // ── Never renewed on purpose ──
    // Roblox's session refresh ends the session it renews: an account pasted from a browser, or shared with another app,
    // would be signed out there. Only what Roblox sends on its own is saved (the owner's choice, 2026-10-22).
    // PORT: the C# asked reflection for RefreshSessionAsync / RenewSignInsIfDue; here the core's source is searched for
    // their Rust names.
    c.check(
        "Bloxkeep has no way to end a session to renew it (no call to auth's /v1/session/refresh)",
        !core_defines("refresh_session") && !core_defines("refresh_session_async") && !core_defines("renew_sign_ins_if_due"),
        (),
    );
    c.check(
        "…and nothing in this run asked for one",
        roblox.asked().iter().all(|r| r.path != "/v1/session/refresh"),
        roblox.asked().iter().filter(|r| r.path == "/v1/session/refresh").count(),
    );

    // ── Signed out, or Roblox's sign-in service down ──
    // An outage of Roblox's sign-in service answers 401 to everyone: outside the daily check (which has its own test), an
    // account is marked signed out only when Roblox accepts another account's sign-in at the same time.
    roblox.down.store(true, Ordering::SeqCst);
    let marked_in_outage = app.confirm_signed_out(cc, &Cancel::new(), false).await;
    c.check(
        "Roblox's sign-in service rejecting everyone: an account isn't marked signed out",
        !marked_in_outage && of(cc).cookie_valid == Some(true),
        of(cc).cookie_valid,
    );
    roblox.down.store(false, Ordering::SeqCst);
    let marked_for_real = app.confirm_signed_out(d, &Cancel::new(), false).await;
    c.check("…one Roblox rejects while it accepts another account: marked signed out", marked_for_real && of(d).cookie_valid == Some(false), of(d).cookie_valid);
    // The daily check's word (it ruled an outage out, or trusts its second run): no other account asked.
    roblox.down.store(true, Ordering::SeqCst);
    roblox.clear();
    let trusted_mark = app.confirm_signed_out(w, &Cancel::new(), true).await;
    let w_cookie = of(w).cookie;
    c.check(
        "…the daily check's own confirmation asks no other account",
        trusted_mark && of(w).cookie_valid == Some(false) && roblox.asked().iter().all(|r| r.cookie.as_deref() == Some(w_cookie.as_str())),
        roblox.asked().iter().map(|r| if r.cookie.as_deref() == Some(w_cookie.as_str()) { "W" } else { "another" }).collect::<Vec<_>>().join(", "),
    );
    // Nobody else to ask (one account, the rest signed out): no telling, so marked as before.
    let signed_in: Vec<Guid> = app.vault(|dd| dd.accounts.iter().filter(|k| k.id != cc && k.cookie_valid != Some(false)).map(|k| k.id).collect()).unwrap_or_default();
    for k in &signed_in {
        app.update_account(*k, |x| x.cookie_valid = Some(false));
    }
    let alone = app.confirm_signed_out(cc, &Cancel::new(), false).await;
    for k in &signed_in {
        app.update_account(*k, |x| x.cookie_valid = Some(true));
    }
    c.check("…with no other account to ask: marked as before", alone && of(cc).cookie_valid == Some(false), of(cc).cookie_valid);
    // The account menu's check in an outage: nobody accepted, nothing marked; not checked rather than signed out.
    let both = call(app, "checkAccounts", json!({ "ids": [a, b] })).await;
    c.check(
        "the account menu's check while Roblox's sign-in service rejects everyone: nobody marked signed out",
        of(a).cookie_valid == Some(true) && of(b).cookie_valid == Some(true) && both.as_ref().is_ok_and(|r| r["bad"] == 0),
        format!("{both:?}"),
    );
    roblox.down.store(false, Ordering::SeqCst);
    // Roblox VNG's sign-in service down while Roblox's is up (review, 2026-10-30): a Roblox account accepted says nothing of
    // VNG's, so its accounts aren't marked signed out.
    let v2 = Account { username: "V2".into(), user_id: 2009, cookie: cookie("V2nd"), cookie_valid: Some(true), realm: realm::VNG.into(), ..Default::default() };
    let v2_id = v2.id;
    roblox.user(&v2.cookie, 2009);
    let _ = app.vault_mut(|dd| dd.accounts.push(v2));
    roblox.down_vng.store(true, Ordering::SeqCst);
    let vng_down = call(app, "checkAccounts", json!({ "ids": [a, v] })).await;
    roblox.down_vng.store(false, Ordering::SeqCst);
    let _ = app.vault_mut(|dd| dd.accounts.retain(|k| k.id != v2_id));
    c.check(
        "the account menu's check with Roblox VNG's sign-in service down and Roblox's up: the VNG account isn't marked signed out",
        of(v).cookie_valid == Some(true) && of(a).cookie_valid == Some(true) && vng_down.as_ref().is_ok_and(|r| r["bad"] == 0),
        format!("{vng_down:?}"),
    );
    // Another user's cookie pasted into an account's edit form (review, 2026-10-30): refused, the account unchanged.
    let b_before = of(b).cookie;
    let pasted = call(app, "updateAccount", json!({ "id": b, "group": "", "note": "", "cookie": of(a).cookie })).await;
    c.check(
        "another user's cookie in an account's edit form: refused, the account keeps its own user and cookie",
        user_error(&pasted).is_some() && of(b).cookie == b_before && of(b).user_id == 2002,
        format!("{pasted:?}"),
    );
    // Signed in again: an account marked signed out that Roblox accepts again is signed in, auto-rejoin back on.
    app.update_account(cc, |x| {
        x.auto_rejoin = false;
        x.rejoin_off_by_sign_out = true;
    });
    let _ = call(app, "checkAccounts", json!({ "ids": [cc] })).await;
    let now_c = of(cc);
    c.check(
        "the account menu's check: an account marked signed out that Roblox accepts again is signed in, auto-rejoin back on",
        now_c.cookie_valid == Some(true) && now_c.auto_rejoin && !now_c.rejoin_off_by_sign_out,
        format!("{:?} {}", now_c.cookie_valid, now_c.auto_rejoin),
    );
    // The daily check asks the accounts marked signed out again too, quietly.
    app.update_account(w, |x| {
        x.auto_rejoin = false;
        x.rejoin_off_by_sign_out = true;
    });
    let vault = app.controller.vault.borrow().clone().expect("unlocked");
    let healed = app.recheck_signed_out(&vault, &[]).await;
    let now_w = of(w);
    c.check(
        "the daily check asks again an account marked signed out: accepted now, it is signed in, auto-rejoin back on",
        healed.as_ref().is_ok_and(|h| *h) && now_w.cookie_valid == Some(true) && now_w.auto_rejoin,
        format!("{healed:?} {:?} {}", now_w.cookie_valid, now_w.auto_rejoin),
    );
    c.check("…ones Roblox still rejects stay signed out", of(d).cookie_valid == Some(false) && of(s).cookie_valid == Some(false), ());

    // An account Roblox holds (a warning waiting to be accepted, 2026-10-29): 403 on everything, cookie still good. Not
    // "network", not signed out: said apart, with its roblox.com to open.
    roblox.inner.lock().unwrap().held.insert(of(a).cookie);
    let held_check = call(app, "checkAccounts", json!({ "ids": [a] })).await;
    let field = |name: &str| held_check.as_ref().map(|r| r[name].clone()).unwrap_or(Value::Null);
    c.check(
        "an account Roblox holds (403): counted as refused, not as a network failure, not signed out",
        field("refused") == 1
            && field("moderated") == 1
            && field("failed") == 0
            && field("bad") == 0
            && of(a).cookie_valid == Some(true)
            && field("refusedIds") == json!([a]),
        format!("{held_check:?}"),
    );
    let k = of(a).cookie;
    let ticket = rt::blocking(move || roblox_api::get_auth_ticket(&k, &Cancel::new(), realm::GLOBAL)).await;
    let message = ticket.as_ref().err().map(|e| e.to_string());
    c.check(
        "…its sign-in ticket refused: marked so the launch offers its roblox.com",
        ticket.as_ref().is_err_and(|e| matches!(e, crate::core::base::Fail::Roblox { .. }) && e.sign_in_refused() && !e.server_error()),
        &message,
    );
    c.check(
        "…Roblox's reason (\"User is moderated\") named: a notice on the account to accept",
        ticket.as_ref().is_err_and(|e| matches!(e, crate::core::base::Fail::Roblox { .. }) && e.moderated())
            && message.as_deref().is_some_and(|m| m.contains("notice on this account")),
        &message,
    );
    // A renewed cookie of such an account: whose it is asked once, not again 10, 40 and 90 s later (the notice stays until
    // the person accepts it). The renewal is let go, the saved one kept.
    let held_before = of(a).cookie;
    let held_next = cookie("A-held");
    {
        let mut inner = roblox.inner.lock().unwrap();
        inner.held.remove(&held_before);
        inner.held.insert(held_next.clone());
        inner.rotate.insert(held_before.clone(), held_next.clone());
    }
    let k = held_before.clone();
    quietly(rt::blocking(move || roblox_api::get_presence(Some(&k), &[2001], realm::GLOBAL, &Cancel::new())).await);
    let done = Rc::new(Cell::new(false));
    {
        let (app, done) = (app.clone(), done.clone());
        rt::spawn(async move {
            renewals(&app).await;
            done.set(true);
        });
    }
    let quick = until(|| done.get(), 5).await;
    let asked = roblox.asked_with(&held_next).iter().filter(|r| r.path == "/v1/users/authenticated").count();
    c.check(
        "a renewed cookie of an account with a notice to accept: whose it is asked once, not again later",
        quick && asked == 1 && of(a).cookie == held_before,
        format!("done in 5 s: {quick}, asked {asked}, kept: {}", of(a).cookie == held_before),
    );
    until(|| done.get(), 300).await;
    roblox.inner.lock().unwrap().held.clear();

    // ── Roblox VNG's accounts in the status round ──
    // Roblox and Roblox VNG count user ids apart: the same number is another person on the other. A Roblox VNG account's
    // status comes from Roblox VNG only; asked of Roblox, its number is a stranger's.
    let v_user = of(v).user_id;
    {
        let mut inner = roblox.inner.lock().unwrap();
        inner.in_game.insert((false, v_user), 4242); // a stranger on Roblox with V's number, in a game
        inner.in_game.insert((true, v_user), 1818); // V itself, on Roblox VNG
    }
    let status_of = || app.controller.presence.borrow().get(&v_user).and_then(|p| p.place_id);
    let _ = app.refresh_presence().await;
    c.check("a Roblox VNG account's status: its own, from Roblox VNG", status_of() == Some(1818), status_of());
    app.update_account(v, |x| x.cookie_valid = Some(false)); // its own question isn't asked
    let _ = app.refresh_presence().await;
    c.check(
        "…its own question not asked (signed out): not the status of the stranger with its number on Roblox",
        status_of() != Some(4242),
        status_of(),
    );
    c.check(
        "…and Roblox isn't asked about its number",
        roblox.asked().iter().filter(|r| r.path == "/v1/presence/users" && r.host == "presence.roblox.com" && r.body.contains(&v_user.to_string())).count() == 0,
        (),
    );
    app.update_account(v, |x| x.cookie_valid = Some(true));
    roblox.inner.lock().unwrap().in_game.clear();

    // ── Roblox's CSRF token for calls without a sign-in (review, 2026-10-30) ──
    // Kept like a signed-in session's: before, each such call (Quick Login's status, every 2.5 s) paid a 403 and a replay.
    roblox_api::CSRF_TOKENS.lock().unwrap_or_else(|e| e.into_inner()).clear();
    roblox.clear();
    for _ in 0..2 {
        quietly(rt::blocking(|| roblox_api::get_presence(None, &[2001], realm::GLOBAL, &Cancel::new())).await);
    }
    let anonymous: Vec<Request> = roblox.asked().into_iter().filter(|r| r.path == "/v1/presence/users" && r.cookie.is_none()).collect();
    c.check(
        "two calls without a sign-in: Roblox's CSRF refusal paid once, not on each",
        anonymous.len() == 3 && anonymous.iter().filter(|r| !r.csrf).count() == 1,
        anonymous.iter().map(|r| if r.csrf { "with token" } else { "without" }).collect::<Vec<_>>().join(", "),
    );

    // ── A private server's share link (review, 2026-10-30) ──
    // Read on apis.roblox.com with a Roblox account's sign-in, never a Roblox VNG one's, even first in launch order.
    roblox.clear();
    let vng_only = app.resolve_share(&[of(v)], "share-code", None, &Cancel::new()).await;
    let v_cookie = of(v).cookie;
    c.check(
        "a share link with only a Roblox VNG account selected: said, and its sign-in not sent to roblox.com",
        refusal(&vng_only).is_some() && roblox.asked_with(&v_cookie).is_empty(),
        format!("{vng_only:?}"),
    );
    let _ = app.resolve_share(&[of(v), of(a)], "share-code", None, &Cancel::new()).await;
    let mut sharers: Vec<Option<String>> = Vec::new();
    for r in roblox.asked().into_iter().filter(|r| r.path.contains("resolve-link")) {
        if !sharers.contains(&r.cookie) {
            sharers.push(r.cookie);
        }
    }
    let a_cookie = of(a).cookie;
    c.check(
        "…a Roblox VNG account first and a Roblox one after: read with the Roblox one only",
        sharers == [Some(a_cookie.clone())],
        sharers
            .iter()
            .map(|k| if k.as_deref() == Some(v_cookie.as_str()) { "VNG" } else if k.as_deref() == Some(a_cookie.as_str()) { "A" } else { "another" })
            .collect::<Vec<_>>()
            .join(", "),
    );

    // ── The game picker's favourites (review, 2026-10-30) ──
    // Roblox's own accounts only: a Roblox VNG account's number is a stranger's on roblox.com.
    roblox.clear();
    let _ = call(app, "gameLists", json!({})).await;
    let favorites_of: Vec<String> = roblox.asked().into_iter().filter(|r| r.path.ends_with("/favorite/games")).map(|r| r.path).collect();
    c.check(
        "the game picker's favourites: asked for Roblox accounts, never a Roblox VNG account's number",
        !favorites_of.is_empty() && !favorites_of.iter().any(|p| p.contains(&format!("/users/{v_user}/"))),
        favorites_of.join(", "),
    );
    // Locked while they load: not kept, so the next vault opened doesn't show this one's accounts' favourites.
    *app.game_picker.favorite_games.borrow_mut() = None;
    roblox.gated.store(true, Ordering::SeqCst);
    let loading: Rc<std::cell::RefCell<Option<Res<Value>>>> = Rc::new(std::cell::RefCell::new(None));
    {
        let (app, out) = (app.clone(), loading.clone());
        rt::spawn(async move {
            let got = call(&app, "gameLists", json!({})).await;
            *out.borrow_mut() = Some(got);
        });
    }
    let before = favorites_of.len();
    until(|| roblox.asked().iter().filter(|r| r.path.ends_with("/favorite/games")).count() > before, 30).await;
    let _ = call(app, "lock", json!({})).await;
    roblox.gated.store(false, Ordering::SeqCst);
    until(|| loading.borrow().is_some(), 60).await;
    let after_lock = loading.borrow().clone();
    c.check(
        "…locked while they load: not kept for the next vault opened",
        app.game_picker.favorite_games.borrow().is_none(),
        format!("{after_lock:?}"),
    );
}
