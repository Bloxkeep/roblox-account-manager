// Tests/Bloxkeep.Tests/ServerHopSuite.cs: "Switch to another server" (app/server_hop.rs): windows playing in a public
// server are closed and opened again into another server of their game with room, the same one for accounts of the same
// game, never the one they leave; a private server or a window not in a game is left alone; no other server with room:
// said, nothing closed. Roblox's server list is a stand-in (roblox_api::STAND_IN); no window is closed (the close desk's
// hook is the test's).
use std::{
    cell::RefCell,
    collections::HashSet,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicI32, Ordering},
    },
    time::Duration,
};

use serde_json::json;

use super::{
    Checks, call,
    rejoin::{no_network, quiet_app, seed},
    unlock,
};
use crate::core::{
    app::{App, controller::PendingLaunch},
    base::{Cancel, Guid, Stamp},
    close_plan::Cause,
    game_target::GameTarget,
    multi_instance,
    models::{Account, LaunchPreset},
    roblox_api::{self, HttpRequest, HttpResponse},
    roblox_log_watcher::{ClientState, Tail},
    rt,
};

const PLACE: i64 = 5151;
const FIRST_LIST: &str = r#"{"data":[{"id":"old-server","playing":5,"maxPlayers":20},{"id":"full","playing":20,"maxPlayers":20},{"id":"one-left","playing":19,"maxPlayers":20},{"id":"roomy","playing":3,"maxPlayers":20}],"nextPageCursor":null}"#;

fn window(app: &App, account: Guid, pid: i32, target: GameTarget, status: &str, job: Option<&str>, preset: Option<Guid>) {
    joined(app, account, pid, target, status, job, Stamp::utc_now(), preset);
}

#[allow(clippy::too_many_arguments)]
fn joined(app: &App, account: Guid, pid: i32, target: GameTarget, status: &str, job: Option<&str>, at: Stamp, preset: Option<Guid>) {
    let place = target.place_id;
    let launch = Rc::new(PendingLaunch::new(account, HashSet::new(), Stamp::utc_now().plus_secs(-300.0), Some(target), false, preset));
    app.controller.running.borrow_mut().insert(account, (pid, launch));
    let mut tail = Tail::new(format!("stand-in-{pid}.log"), pid);
    tail.state = Some(ClientState::new(status, Some(place), job.map(str::to_string), None, at));
    app.controller.logs.borrow_mut().tails.insert(pid, tail);
}

struct Roblox {
    list: Mutex<String>,
    server_errors: AtomicI32,
    /// Roblox refuses the sign-in ticket asked before a window is closed to move.
    signed_out: AtomicBool,
}

impl Roblox {
    fn answer(&self, req: &HttpRequest) -> HttpResponse {
        let rest = req.url.split("://").nth(1).unwrap_or("");
        let (host, path_query) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
        let path = path_query.split('?').next().unwrap_or("");
        if path.starts_with("/v1/authentication-ticket") {
            return if self.signed_out.load(Ordering::SeqCst) {
                HttpResponse {
                    status: 401,
                    headers: vec![("Content-Type".into(), "application/json".into())],
                    body: br#"{"errors":[{"code":0,"message":"Authorization has been denied for this request."}]}"#.to_vec(),
                }
            } else {
                HttpResponse { status: 200, headers: vec![("rbx-authentication-ticket".into(), "stand-in-ticket".into())], body: b"{}".to_vec() }
            };
        }
        let servers = path.contains("/servers/Public");
        if servers && self.server_errors.load(Ordering::SeqCst) > 0 {
            self.server_errors.fetch_sub(1, Ordering::SeqCst);
            return HttpResponse { status: 500, headers: vec![], body: b"{}".to_vec() };
        }
        // A game's details: its name from develop.roblox.com, its player count garbled (a cut-off answer).
        let body = if servers {
            self.list.lock().unwrap().clone()
        } else if path == "/v1/universes/multiget" {
            format!(r#"{{"data":[{{"id":77,"name":"Hop Game","creatorName":"Maker","rootPlaceId":{PLACE}}}]}}"#)
        } else if host == "games.roblox.com" && path == "/v1/games" {
            r#"{"data":[{"id":77,"pla"#.to_string()
        } else {
            "{}".to_string()
        };
        HttpResponse { status: 200, headers: vec![("Content-Type".into(), "application/json; charset=utf-8".into())], body: body.into_bytes() }
    }
}

fn set_stand_in(f: impl Fn(&HttpRequest) -> HttpResponse + Send + Sync + 'static) {
    *roblox_api::STAND_IN.write().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(move |req| Ok(f(req))));
}

pub async fn run(c: &mut Checks, _dir: &str) {
    no_network();
    let made = |name: &str, cookie: &str, user_id: i64| Account { username: name.into(), cookie: cookie.into(), user_id, cookie_valid: Some(true), ..Default::default() };
    let (h1, h2, h3) = (made("hop1", "x1", 2001), made("hop2", "x2", 2002), made("private", "x3", 2003));
    let (a1, a2, a3) = (h1.id, h2.id, h3.id);
    seed(vec![h1, h2, h3]);
    let (app, _said) = quiet_app();
    unlock(&app).await;

    let place = || GameTarget::new(PLACE, None, None, None);
    window(&app, a1, 801, place(), "ingame", Some("old-server"), None);
    window(&app, a2, 802, place(), "ingame", Some("old-server"), None);
    window(&app, a3, 803, GameTarget { access_code: Some("private-code".into()), ..place() }, "ingame", Some("private-server"), None);
    let closed: Rc<RefCell<Vec<i32>>> = Rc::new(RefCell::new(Vec::new()));
    let real_close = app.close_desk.close_hook.borrow().clone();
    {
        let closed = closed.clone();
        *app.close_desk.close_hook.borrow_mut() = Some(Rc::new(move |pid, _| closed.borrow_mut().push(pid)));
    }
    let closed_now = || closed.borrow().clone();
    let going = |a: Guid| app.close_desk.reopen_of(a);
    let moving_len = || app.close_desk.moving.borrow().len();
    let was_armed = multi_instance::armed();

    let roblox = Arc::new(Roblox { list: Mutex::new(FIRST_LIST.into()), server_errors: AtomicI32::new(0), signed_out: AtomicBool::new(false) });
    let plain = {
        let r = roblox.clone();
        move || {
            let r = r.clone();
            set_stand_in(move |req| r.answer(req));
        }
    };
    plain();

    let only_private = call(&app, "switchServer", json!({ "ids": [a3] })).await;
    c.check(
        "a private server only: said, nothing closed",
        super::rejoin::refusal(&only_private).is_some() && closed_now().is_empty(),
        format!("{only_private:?}"),
    );

    // Without Multi-Roblox, a window reopened while others are open would close them: none is closed to begin with, as
    // a restart isn't (close_plan.rs A4; fails on the old code).
    multi_instance::set_armed(false);
    let unarmed = call(&app, "switchServer", json!({ "ids": [a1, a2] })).await;
    c.check(
        "Multi-Roblox not ready with other windows open: nothing closed, nothing marked as moving (fails on the old code)",
        matches!(&unarmed, Ok(v) if v == 0) && closed_now().is_empty() && moving_len() == 0 && going(a1).is_none(),
        format!("{unarmed:?} {:?}", closed_now()),
    );
    multi_instance::set_armed(true);

    let moved = call(&app, "switchServer", json!({ "ids": [a1, a2, a3] })).await;
    let mut sorted = closed_now();
    sorted.sort();
    c.check(
        "two accounts of a game: both closed to reopen, the private one left alone",
        matches!(&moved, Ok(v) if v == 2) && sorted == [801, 802] && going(a3).is_none(),
        sorted.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(","),
    );
    c.check(
        "…into the same server with room for both, not the one they left",
        going(a1).is_some_and(|t| t.place_id == PLACE && t.job_id.as_deref() == Some("roomy")) && going(a2) == going(a1),
        going(a1),
    );
    let marked: HashSet<Guid> = app.close_desk.moving.borrow().keys().copied().collect();
    c.check(
        "…marked as moving, so a preset's Rejoin server doesn't send them back",
        marked == HashSet::from([a1, a2]) && app.hopping(),
        (),
    );
    // A switch lasts until the account's next window is in its game, not just until its reopen starts (fails on the
    // old code: the mark lasted only by its clock).
    let close_of = |pid: i32| app.close_desk.closing_of(pid);
    c.check(
        "…a server switch outranks nothing the person asks: Close of a moving account ends its switch (fails on the old code)",
        {
            let _ = app.close_accounts(&[a2], false);
            close_of(802).is_some_and(|r| r.cause == Cause::User && r.then == crate::core::close_plan::Then::StayClosed) && !app.close_desk.moving_of(a2)
        },
        format!("{:?}", close_of(802)),
    );
    let at_rest = app.close_ended(a1, 801, None);
    window(&app, a1, 805, GameTarget::new(PLACE, Some("roomy".into()), None, None), "joining", None, None);
    app.close_desk_tick(Stamp::utc_now(), &HashSet::from([805]));
    let still_moving = app.close_desk.moving_of(a1);
    window(&app, a1, 805, GameTarget::new(PLACE, Some("roomy".into()), None, None), "ingame", Some("roomy"), None);
    app.close_desk_tick(Stamp::utc_now(), &HashSet::from([805]));
    c.check(
        "…the switched window ended: reopened into the new server, moving until the new window is in its game, then not (fails on the old code)",
        matches!(&at_rest, crate::core::close_plan::End::Reopen(t) if t.job_id.as_deref() == Some("roomy")) && still_moving && !app.close_desk.moving_of(a1),
        format!("{at_rest:?} {still_moving}"),
    );
    app.close_desk.moving.borrow_mut().insert(a1, Stamp::utc_now());
    for since in app.close_desk.moving.borrow_mut().values_mut() {
        *since = Stamp::utc_now().plus_secs(-600.0);
    }
    c.check(
        "…a mark no reopening took counts no more after a few minutes (fails on the old code)",
        !app.hopping() && moving_len() == 0,
        (),
    );

    // The window closed while Roblox gave the list: nothing to move, and nothing marked.
    closed.borrow_mut().clear();
    app.close_desk.clear();
    window(&app, a1, 821, place(), "ingame", Some("old-server"), None);
    // The stand-in answers on a worker thread; the window is taken away when the server list is asked for.
    let asked_list = Arc::new(AtomicI32::new(0));
    {
        let (r, asked_list) = (roblox.clone(), asked_list.clone());
        set_stand_in(move |req| {
            if req.url.contains("/servers/Public") {
                asked_list.fetch_add(1, Ordering::SeqCst);
                // Give the main thread time to take the window away before Roblox's answer lands.
                std::thread::sleep(Duration::from_millis(400));
            }
            r.answer(req)
        });
    }
    let gone_out: Rc<RefCell<Option<crate::core::base::Res<serde_json::Value>>>> = Rc::new(RefCell::new(None));
    {
        let (app, out) = (app.clone(), gone_out.clone());
        rt::spawn(async move {
            let got = call(&app, "switchServer", json!({ "ids": [a1] })).await;
            *out.borrow_mut() = Some(got);
        });
    }
    for _ in 0..250 {
        if asked_list.load(Ordering::SeqCst) > 0 {
            break;
        }
        rt::sleep(Duration::from_millis(20)).await;
    }
    app.controller.running.borrow_mut().shift_remove(&a1);
    for _ in 0..500 {
        if gone_out.borrow().is_some() {
            break;
        }
        rt::sleep(Duration::from_millis(20)).await;
    }
    let gone = gone_out.borrow().clone();
    c.check(
        "a window that closed while Roblox answered: not closed again, not marked as moving (fails on the old code)",
        matches!(&gone, Some(Ok(v)) if v == 0) && closed_now().is_empty() && moving_len() == 0 && going(a1).is_none(),
        format!("{gone:?} {:?} {}", closed_now(), moving_len()),
    );
    plain();

    // Roblox won't sign the account in again (signed out): its window stays where it is instead of closing for nothing
    // (RoALT checks first too; before, it closed and the reopening was refused).
    window(&app, a1, 823, place(), "ingame", Some("old-server"), None);
    roblox.signed_out.store(true, Ordering::SeqCst);
    let refused = call(&app, "switchServer", json!({ "ids": [a1] })).await;
    roblox.signed_out.store(false, Ordering::SeqCst);
    c.check(
        "a sign-in Roblox refuses: the window isn't closed, not marked as moving (fails on the old code)",
        matches!(&refused, Ok(v) if v == 0) && closed_now().is_empty() && moving_len() == 0 && going(a1).is_none(),
        format!("{refused:?} {:?} {}", closed_now(), moving_len()),
    );

    // Locked after the switch: the reopenings went with the vault, and so do the marks.
    window(&app, a1, 822, place(), "ingame", Some("old-server"), None);
    let _ = call(&app, "switchServer", json!({ "ids": [a1] })).await;
    let was_marked = moving_len() == 1;
    let _ = call(&app, "lock", json!({})).await;
    c.check("locked after a switch: no account is still marked as moving (fails on the old code)", was_marked && moving_len() == 0, ());
    unlock(&app).await;
    // Unlocking may have picked windows up again: the checks below set their own.
    app.controller.running.borrow_mut().clear();

    closed.borrow_mut().clear();
    app.close_desk.clear();
    window(&app, a1, 811, place(), "ingame", Some("roomy"), None);
    *roblox.list.lock().unwrap() = r#"{"data":[{"id":"roomy","playing":5,"maxPlayers":20},{"id":"full","playing":20,"maxPlayers":20}],"nextPageCursor":null}"#.into();
    let none = call(&app, "switchServer", json!({ "ids": [a1] })).await;
    c.check("no other server with room: nothing closed", matches!(&none, Ok(v) if v == 0) && closed_now().is_empty(), format!("{none:?}"));

    window(&app, a2, 812, place(), "joining", None, None);
    let not_in = call(&app, "switchServer", json!({ "ids": [a2] })).await;
    c.check(
        "a window still joining: said, nothing closed",
        super::rejoin::refusal(&not_in).is_some() && closed_now().is_empty(),
        format!("{not_in:?}"),
    );

    // ── the server window ──
    // A one-off 500 is said, and not kept: before, the window showed "no servers" for the minute the list is kept.
    roblox.server_errors.store(1, Ordering::SeqCst);
    let failed_list = call(&app, "servers", json!({ "placeId": PLACE + 1, "fewest": false })).await;
    let after_it = call(&app, "servers", json!({ "placeId": PLACE + 1, "fewest": false })).await;
    let count = |r: &crate::core::base::Res<serde_json::Value>| r.as_ref().ok().and_then(|v| v["servers"].as_array().map(|a| a.len()));
    c.check("the server list answered with a 500: said, not shown as no servers", failed_list.is_err(), format!("{failed_list:?}"));
    c.check(
        "…asked again right after: Roblox's list, not an empty one kept from the failure",
        count(&after_it) == Some(2),
        format!("{after_it:?}"),
    );

    // A game's player count that came back garbled leaves the count blank, not the name.
    let games = rt::blocking(|| roblox_api::get_games([77], Some("x1"), &Cancel::new())).await;
    let name = games.as_ref().ok().and_then(|g| g.get(&77).map(|i| i.name.clone()));
    c.check(
        "a game's player count answer garbled: its name still comes back",
        name.as_deref() == Some("Hop Game"),
        format!("{:?}", games.as_ref().map(|g| g.values().map(|i| i.name.clone()).collect::<Vec<_>>())),
    );

    // ── each account in a server of its own (a preset's "Own server each") ──
    window(&app, a1, 821, place(), "ingame", Some("old-server"), None);
    window(&app, a2, 822, place(), "ingame", Some("old-server"), None);
    *roblox.list.lock().unwrap() = FIRST_LIST.into();
    let each = app.servers_apart("fewest");
    let first = each(place(), Cancel::new()).await;
    let second = each(place(), Cancel::new()).await;
    let third = each(place(), Cancel::new()).await;
    let job = |r: &crate::core::base::Res<GameTarget>| r.as_ref().ok().and_then(|t| t.job_id.clone());
    c.check(
        "own server each: the first with room, then the next, never where the accounts already are",
        job(&first).as_deref() == Some("one-left") && job(&second).as_deref() == Some("roomy"),
        format!("{:?} {:?}", job(&first), job(&second)),
    );
    c.check("…none left: Roblox picks for the rest", third.is_ok() && job(&third).is_none(), format!("{third:?}"));

    // ── leaving a crowded server ──
    closed.borrow_mut().clear();
    app.close_desk.clear();
    app.controller.running.borrow_mut().shift_remove(&a3);
    let ago = || Stamp::utc_now().plus_secs(-300.0);
    joined(&app, a1, 831, place(), "ingame", Some("old-server"), ago(), None);
    joined(&app, a2, 832, place(), "ingame", Some("roomy"), ago(), None);
    *roblox.list.lock().unwrap() =
        r#"{"data":[{"id":"quiet","playing":2,"maxPlayers":20},{"id":"roomy","playing":3,"maxPlayers":20},{"id":"old-server","playing":5,"maxPlayers":20}],"nextPageCursor":null}"#.into();
    let count_servers = |above: i32| {
        let app = app.clone();
        async move {
            let _ = app.vault_mut(|d| d.settings.hop_above_players = above);
            app.server_hop.next_crowd_check.set(Stamp::from_unix_secs(-62_135_596_800));
            app.hop_crowded_clients(Stamp::utc_now());
            for _ in 0..100 {
                if !app.server_hop.crowd_busy.get() {
                    break;
                }
                rt::sleep(Duration::from_millis(20)).await;
            }
        }
    };
    count_servers(0).await;
    c.check("leave crowded servers off: nothing closed", closed_now().is_empty(), ());
    count_servers(1).await;
    c.check("no server quiet enough: nothing closed", closed_now().is_empty(), format!("{:?}", closed_now()));
    count_servers(3).await;
    c.check(
        "a crowded server: its window moves to a quiet one, the others stay",
        closed_now() == [831] && going(a1).and_then(|t| t.job_id).as_deref() == Some("quiet"),
        format!("{:?} {:?}", closed_now(), going(a1).and_then(|t| t.job_id)),
    );
    closed.borrow_mut().clear();
    count_servers(3).await;
    c.check("…not counted again while one is on its way", closed_now().is_empty(), ());
    app.close_desk.clear();
    joined(&app, a1, 833, place(), "ingame", Some("old-server"), Stamp::utc_now(), None);
    count_servers(3).await;
    c.check("a window just in is left alone", closed_now().is_empty(), format!("{:?}", closed_now()));
    // A preset that sends its accounts to a server on purpose (its Job ID, or the fullest one) keeps them there.
    for server in ["job", "most"] {
        let preset = LaunchPreset { name: server.into(), server: server.into(), account_ids: vec![a1], ..Default::default() };
        let preset_id = preset.id;
        let _ = app.vault_mut(|d| d.settings.presets.push(preset));
        joined(&app, a1, 834, place(), "ingame", Some("old-server"), ago(), Some(preset_id));
        count_servers(3).await;
        c.check(
            &format!("a preset's window sent to a server on purpose (\"{server}\") stays in it, crowded or not (fails on the old code)"),
            closed_now().is_empty(),
            format!("{:?}", closed_now()),
        );
        let _ = app.vault_mut(|d| d.settings.presets.retain(|p| p.id != preset_id));
    }
    closed.borrow_mut().clear();
    app.close_desk.clear();
    let any_server = LaunchPreset { name: "any".into(), server: "any".into(), account_ids: vec![a1], ..Default::default() };
    let any_id = any_server.id;
    let _ = app.vault_mut(|d| d.settings.presets.push(any_server));
    joined(&app, a1, 835, place(), "ingame", Some("old-server"), ago(), Some(any_id));
    count_servers(3).await;
    c.check("…one where Roblox picked the server still moves", closed_now() == [835], format!("{:?}", closed_now()));
    let _ = app.vault_mut(|d| d.settings.presets.retain(|p| p.id != any_id));

    app.controller.running.borrow_mut().clear();
    app.close_desk.clear();
    *app.close_desk.close_hook.borrow_mut() = real_close;
    multi_instance::set_armed(was_armed);
    no_network();
}
