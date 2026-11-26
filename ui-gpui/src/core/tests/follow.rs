// Tests/Bloxkeep.Tests/FollowSuite.cs: following a player on auto-rejoin. "Join a player…" with its box ticked (or a
// preset that follows someone) keeps the player in the target, through drops, restarts and Bloxkeep closing; each rejoin
// asks Roblox where the player is now, with the rejoining account first and the other signed-in ones after, and goes to
// that server, checked with Roblox's join answer. Offline, the website, a hidden, full or ended server, a game that bans
// the account: the rejoin waits a minute and looks again, said once per reason. Roblox here is a stand-in
// (roblox_api::STAND_IN and the join answer's field); it never gives a sign-in ticket, so nothing can open Roblox.
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicI32, Ordering},
    },
    time::{Duration, Instant},
};

use serde_json::{Value, json};

use super::{
    Checks, call,
    rejoin::{acc, answer, no_network, quiet_app, refusal, seed, told},
    unlock,
};
use crate::core::{
    app::{
        App,
        controller::{PendingLaunch, RejoinRequest},
        schedule::ScheduledRun,
    },
    base::{Cancel, Guid, Stamp},
    game_target::{FollowedPlayer, GameTarget},
    models::{Account, GameBan, LaunchPreset, WaitingRejoin},
    roblox_api::{self, HttpRequest, HttpResponse, JoinAnswer},
    roblox_log_watcher::ClientState,
    rt,
};

const FRIEND: i64 = 77;
const PLACE: i64 = 111;
const OTHER_PLACE: i64 = 222;
const SERVER: &str = "11111111-2222-3333-4444-555555555555";

fn json_answer(status: u16, body: Value) -> HttpResponse {
    HttpResponse { status, headers: vec![("Content-Type".into(), "application/json; charset=utf-8".into())], body: body.to_string().into_bytes() }
}

/// Roblox's presence service as each sign-in is shown it, and the order they asked in.
#[derive(Default)]
struct Roblox {
    seen: Mutex<HashMap<String, Value>>,
    asked: Mutex<Vec<String>>,
    rate_limited: AtomicBool,
    /// Set: the presence answer waits until `gate_open` (a slow Roblox), after telling it was asked.
    gated: AtomicBool,
    gate_open: AtomicBool,
    presence_asked: AtomicBool,
}

impl Roblox {
    fn answer(&self, req: &HttpRequest) -> HttpResponse {
        let cookie = req.headers_named("Cookie").next().map(|c| c.replace(".ROBLOSECURITY=", "")).unwrap_or_default();
        let body = req.body.as_ref().map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default();
        let path = req.url.split("://").nth(1).and_then(|r| r.find('/').map(|i| r[i..].to_string())).unwrap_or_default();
        let path = path.split('?').next().unwrap_or("").to_string();
        // Never a sign-in ticket: nothing here can open Roblox. A 503 is Roblox busy, which a rejoin tries again later.
        if path.starts_with("/v1/authentication-ticket") {
            return json_answer(503, json!({}));
        }
        if path != "/v1/presence/users" {
            return json_answer(404, json!({}));
        }
        if req.headers_named("x-csrf-token").next().is_none() {
            let mut csrf = json_answer(403, json!({ "errors": [{ "code": 0, "message": "Token Validation Failed" }] }));
            csrf.headers.push(("x-csrf-token".into(), "token".into()));
            return csrf;
        }
        self.asked.lock().unwrap().push(cookie.clone());
        if self.gated.load(Ordering::SeqCst) {
            self.presence_asked.store(true, Ordering::SeqCst);
            while !self.gate_open.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        if self.rate_limited.load(Ordering::SeqCst) {
            return json_answer(429, json!({}));
        }
        let presences: Vec<Value> =
            if body.contains(&FRIEND.to_string()) { self.seen.lock().unwrap().get(&cookie).cloned().into_iter().collect() } else { Vec::new() };
        json_answer(200, json!({ "userPresences": presences }))
    }

    fn see(&self, cookie: &str, presence: Value) {
        self.seen.lock().unwrap().insert(cookie.into(), presence);
    }

    fn asked(&self) -> Vec<String> {
        self.asked.lock().unwrap().clone()
    }
}

fn in_server(place: i64, job: &str) -> Value {
    json!({ "userPresenceType": 2, "lastLocation": "A game", "placeId": place, "rootPlaceId": place, "gameId": job, "universeId": 9, "userId": FRIEND })
}
fn in_hidden_server() -> Value {
    json!({ "userPresenceType": 2, "lastLocation": "A game", "userId": FRIEND })
}
fn offline() -> Value {
    json!({ "userPresenceType": 0, "lastLocation": "", "userId": FRIEND })
}
fn website() -> Value {
    json!({ "userPresenceType": 1, "lastLocation": "Website", "userId": FRIEND })
}

fn followed(place: i64) -> GameTarget {
    GameTarget { follow: Some(FollowedPlayer { user_id: FRIEND, name: "Friend".into() }), ..GameTarget::new(place, None, None, None) }
}

pub async fn run(c: &mut Checks, _dir: &str) {
    no_network();
    let rejoiner = Account { username: "Rejoiner".into(), user_id: 21, cookie: "cookie-a".into(), auto_rejoin: true, ..Default::default() };
    let other = Account { username: "Other".into(), user_id: 22, cookie: "cookie-b".into(), ..Default::default() };
    let (id, other_id) = (rejoiner.id, other.id);
    seed(vec![rejoiner, other]);
    let (app, said) = quiet_app();
    unlock(&app).await;
    // Unlocking starts a presence round of its own; let it go out (refused, no network) before Roblox's stand-in counts
    // who was asked.
    rt::sleep(Duration::from_millis(300)).await;
    let follow = FollowedPlayer { user_id: FRIEND, name: "Friend".into() };

    let roblox = Arc::new(Roblox::default());
    {
        let r = roblox.clone();
        *roblox_api::STAND_IN.write().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(move |req| Ok(r.answer(req))));
    }
    let join_answer = Arc::new(Mutex::new(answer(200, Some(2), None)));
    let join_asked: Arc<Mutex<Vec<GameTarget>>> = Arc::new(Mutex::new(Vec::new()));
    let real_ask = app.rejoin.ask_join.borrow().clone();
    let plain_ask = {
        let (a, asked) = (join_answer.clone(), join_asked.clone());
        Arc::new(move |_: &Account, target: &GameTarget, _: &Cancel| -> crate::core::base::Res<JoinAnswer> {
            asked.lock().unwrap().push(target.clone());
            Ok(a.lock().unwrap().clone())
        })
    };
    *app.rejoin.ask_join.borrow_mut() = Some(plain_ask.clone());

    body(c, &app, &said, &roblox, id, other_id, &follow, &join_answer, &join_asked, plain_ask).await;

    no_network();
    *app.rejoin.ask_join.borrow_mut() = real_ask;
}

#[allow(clippy::too_many_arguments)]
async fn body(
    c: &mut Checks,
    app: &Rc<App>,
    said: &super::Said,
    roblox: &Arc<Roblox>,
    id: Guid,
    other_id: Guid,
    follow: &FollowedPlayer,
    join_answer: &Arc<Mutex<JoinAnswer>>,
    join_asked: &Arc<Mutex<Vec<GameTarget>>>,
    plain_ask: crate::core::app::rejoin::AskJoin,
) {
    let told = || told(said);
    let account = || acc(app, id);
    let queue_len = || app.controller.rejoin_queue.borrow().len();
    let waiting = || {
        let q = app.controller.rejoin_queue.borrow();
        if q.len() == 1 { Some((q[0].target.clone(), q[0].due)) } else { None }
    };
    let set_join = |a: JoinAnswer| *join_answer.lock().unwrap() = a;

    // ── Join a player's box ──
    let target = App::join_target(&json!({ "placeId": PLACE, "jobId": SERVER, "follow": true, "followUserId": FRIEND, "followName": "Friend" }));
    c.check(
        "Join a player with its box ticked: the server found, following that player",
        target.as_ref().is_ok_and(|t| t.place_id == PLACE && t.job_id.as_deref() == Some(SERVER) && t.follow.as_ref() == Some(follow)),
        format!("{target:?}"),
    );
    let target = App::join_target(&json!({ "placeId": PLACE, "jobId": SERVER, "follow": false, "followUserId": FRIEND, "followName": "Friend" }));
    c.check(
        "…unticked: the server found, following nobody",
        target.as_ref().is_ok_and(|t| t.place_id == PLACE && t.job_id.as_deref() == Some(SERVER) && t.follow.is_none()),
        format!("{target:?}"),
    );

    // ── kept through a drop, a game picked in the window, and Bloxkeep closing ──
    let opened = GameTarget { follow: Some(follow.clone()), ..GameTarget::new(PLACE, Some(SERVER.into()), None, None) };
    let launch = |target: &GameTarget| Rc::new(PendingLaunch::new(id, HashSet::new(), Stamp::utc_now().plus_secs(-1200.0), Some(target.clone()), false, None));
    app.queue_rejoin(id, &launch(&opened), None, crate::core::close_plan::End::Unasked);
    let queued = waiting().map(|w| w.0);
    c.check("a drop of a window that follows a player: its rejoin still follows them", queued.as_ref().is_some_and(|q| q.follow.as_ref() == Some(follow)), &queued);
    app.controller.rejoin_queue.borrow_mut().clear();
    let mut picked = ClientState::new("disconnected", Some(OTHER_PLACE), None, Some(277), Stamp::utc_now());
    picked.chosen_place = Some(OTHER_PLACE);
    app.queue_rejoin(id, &launch(&opened), Some(&picked), crate::core::close_plan::End::Unasked);
    let queued = waiting().map(|w| w.0);
    c.check(
        "…a game picked in the window since: still following the player (fails on the old code: a new GameTarget dropped it)",
        queued.as_ref().is_some_and(|q| q.place_id == OTHER_PLACE && q.follow.as_ref() == Some(follow)),
        &queued,
    );
    app.controller.rejoin_queue.borrow_mut().clear();
    let written = serde_json::to_string(&WaitingRejoin { account_id: id, target: Some(opened.clone()), ..Default::default() }).unwrap_or_default();
    let saved: Option<WaitingRejoin> = serde_json::from_str(&written).ok();
    c.check(
        "a waiting rejoin saved with the vault and read back: still following the player",
        saved.as_ref().and_then(|s| s.target.as_ref()).and_then(|t| t.follow.as_ref()) == Some(follow),
        saved.as_ref().map(|s| s.target.clone()),
    );

    // ── where a rejoin goes ──
    // Each check below is a minute after the one before, as a waiting rejoin's: nothing remembered from the last.
    let follow_to = |target: GameTarget, fresh: bool| {
        let app = app.clone();
        async move {
            if fresh {
                app.join_player.follow_seen.borrow_mut().clear();
            }
            let request = RejoinRequest::new(id, target, Stamp::utc_now());
            let account = acc(&app, id);
            app.follow_target(&account, &request, &Cancel::new()).await
        }
    };

    roblox.see("cookie-a", in_server(OTHER_PLACE, "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"));
    roblox.asked.lock().unwrap().clear();
    told();
    let went = follow_to(followed(PLACE), true).await.ok().flatten();
    c.check(
        "the player found in another game's server: the rejoin goes there, still following them",
        went.as_ref().is_some_and(|w| w.place_id == OTHER_PLACE && w.job_id.as_deref() == Some("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee") && w.follow.as_ref() == Some(follow)),
        &went,
    );
    c.check("…asked with the rejoining account's own sign-in, and no other once it was shown a server", roblox.asked() == ["cookie-a"], roblox.asked().join(","));
    let last_join = join_asked.lock().unwrap().last().cloned();
    c.check(
        "…and checked with Roblox's join answer for that server",
        last_join.as_ref().is_some_and(|t| t.place_id == OTHER_PLACE && t.job_id.as_deref() == Some("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")),
        &last_join,
    );
    let nothing_said = told();
    c.check("…nothing waits, nothing said", queue_len() == 0 && nothing_said.is_none(), &nothing_said);

    roblox.see("cookie-a", in_hidden_server());
    roblox.see("cookie-b", in_server(PLACE, SERVER));
    roblox.asked.lock().unwrap().clear();
    let went = follow_to(followed(PLACE), true).await.ok().flatten();
    c.check(
        "a server Roblox shows another of your accounts but not this one: that one's answer is used",
        went.as_ref().is_some_and(|w| w.place_id == PLACE && w.job_id.as_deref() == Some(SERVER)) && roblox.asked() == ["cookie-a", "cookie-b"],
        format!("{went:?} asked {}", roblox.asked().join(",")),
    );

    roblox.seen.lock().unwrap().remove("cookie-b");
    roblox.see("cookie-a", offline());
    let before = Stamp::utc_now();
    let went = follow_to(followed(PLACE), true).await.ok().flatten();
    let s = told();
    let w = waiting();
    c.check(
        "the player offline: no server, the rejoin waits a minute and looks again",
        went.is_none()
            && w.as_ref().is_some_and(|(t, due)| t.follow.as_ref() == Some(follow) && *due >= before.plus_secs(55.0) && *due <= Stamp::utc_now().plus_secs(65.0)),
        w.map(|w| w.1),
    );
    c.check("…said once, as offline", s.as_deref().is_some_and(|s| s.contains("waits to join Friend: Friend is offline")), &s);
    app.controller.rejoin_queue.borrow_mut().clear();
    let went = follow_to(followed(PLACE), true).await.ok().flatten();
    let again = told();
    c.check("…still offline a minute later: waits again, not said again", went.is_none() && queue_len() == 1 && again.is_none(), &again);
    app.controller.rejoin_queue.borrow_mut().clear();

    // Many accounts following someone offline: a few accounts asked, not every one, and the answer shared for half a minute.
    let extra: Vec<Account> =
        (0..3).map(|i| Account { username: format!("More{i}"), user_id: 30 + i, cookie: format!("cookie-x{i}"), ..Default::default() }).collect();
    let extra_ids: Vec<Guid> = extra.iter().map(|a| a.id).collect();
    let _ = app.vault_mut(|d| d.accounts.extend(extra));
    roblox.asked.lock().unwrap().clear();
    let _ = follow_to(followed(PLACE), true).await;
    let first_asked = roblox.asked().len();
    app.controller.rejoin_queue.borrow_mut().clear();
    let _ = follow_to(followed(PLACE), false).await;
    c.check("a player offline, five accounts signed in: three asked, not all five", first_asked == 3, roblox.asked().join(","));
    c.check(
        "…another rejoin following them right after asks Roblox nothing",
        roblox.asked().len() == first_asked && queue_len() == 1,
        roblox.asked().join(","),
    );
    app.controller.rejoin_queue.borrow_mut().clear();
    let _ = app.vault_mut(|d| d.accounts.retain(|a| !extra_ids.contains(&a.id)));
    told();

    roblox.see("cookie-a", website());
    let went = follow_to(followed(PLACE), true).await.ok().flatten();
    let s = told();
    c.check(
        "now on the website: waits, said again for the new reason",
        went.is_none() && s.as_deref().is_some_and(|s| s.contains("Friend is on the Roblox website")),
        &s,
    );
    app.controller.rejoin_queue.borrow_mut().clear();

    roblox.see("cookie-a", in_server(PLACE, SERVER));
    set_join(answer(200, Some(6), Some("The game is full")));
    let went = follow_to(followed(PLACE), true).await.ok().flatten();
    let s = told();
    c.check(
        "the player's server full (Roblox's join answer, status 6): waits instead of opening a window that fails",
        went.is_none() && queue_len() == 1 && s.as_deref().is_some_and(|s| s.contains("Friend's server is full")),
        &s,
    );
    app.controller.rejoin_queue.borrow_mut().clear();
    set_join(answer(200, Some(5), Some("The game has ended")));
    let went = follow_to(followed(PLACE), true).await.ok().flatten();
    let s = told();
    c.check("…a server that has just ended (status 5): waits and looks again", went.is_none() && s.as_deref().is_some_and(|s| s.contains("has just ended")), &s);
    app.controller.rejoin_queue.borrow_mut().clear();
    set_join(answer(200, Some(2), None));

    app.update_account(id, |a| a.game_bans = vec![GameBan { place_id: PLACE, game_name: None, at: Stamp::utc_now(), message: None, until: None }]);
    let joins_before = join_asked.lock().unwrap().len();
    let went = follow_to(followed(OTHER_PLACE), true).await.ok().flatten();
    let s = told();
    c.check(
        "the player's game has banned this account: waits, without asking Roblox to join",
        went.is_none() && join_asked.lock().unwrap().len() == joins_before && s.as_deref().is_some_and(|s| s.contains("has banned this account")),
        &s,
    );
    app.controller.rejoin_queue.borrow_mut().clear();

    // The rejoin itself: a ban in the game it was opened into doesn't stop a follow (the player's game counts), and a
    // player who can't be followed opens no window.
    roblox.see("cookie-a", offline());
    told();
    app.controller.running.borrow_mut().clear();
    app.join_player.follow_seen.borrow_mut().clear();
    app.clone().rejoin(RejoinRequest::new(id, followed(PLACE), Stamp::utc_now())).await;
    let s = told();
    c.check(
        "a rejoin that follows a player, banned in the game it was opened into: still looks for the player (fails on the old code: dropped at once)",
        queue_len() == 1 && s.as_deref().is_some_and(|s| s.contains("Friend is offline")),
        &s,
    );
    c.check(
        "…and opens no window while the player can't be followed",
        app.controller.pending.borrow().is_empty() && !app.controller.rejoining.get(),
        (app.controller.pending.borrow().len(), app.controller.rejoining.get()),
    );
    app.controller.rejoin_queue.borrow_mut().clear();
    app.update_account(id, |a| a.game_bans.clear());

    // Roblox couldn't answer at all: thrown, so the rejoin's own retry for a lost connection takes it.
    roblox.rate_limited.store(true, Ordering::SeqCst);
    let failed = follow_to(followed(PLACE), true).await;
    c.check(
        "Roblox rate-limiting the question: thrown for the rejoin's usual retry, nothing waits here",
        failed.as_ref().is_err_and(|e| e.rate_limited()) && queue_len() == 0,
        format!("{failed:?}"),
    );
    roblox.rate_limited.store(false, Ordering::SeqCst);

    // ── Discord ──
    roblox.see("cookie-a", offline());
    told();
    let _ = follow_to(followed(PLACE), true).await;
    let s = told();
    app.controller.rejoin_queue.borrow_mut().clear();
    let made = crate::core::l::Made::new(
        "{0} waits to join {1}: {1} is offline, or hides being online. Bloxkeep looks again every minute.",
        vec![account().label(), "Friend".to_string()],
    );
    let alert = app.alert_for("rejoinFail", id, Some(&made), s.as_deref().unwrap_or(""), Stamp::utc_now());
    c.check("Discord's alert: waiting to join a player (not the memory wait's title)", alert.title == "Waiting to join a player", &alert.title);

    // ── a preset that follows a player ──
    let preset = LaunchPreset {
        name: "With Friend".into(),
        account_ids: vec![id],
        server: "follow".into(),
        follow_player: "Friend".into(),
        follow_user_id: FRIEND,
        ..Default::default()
    };
    let vault = app.controller.vault.borrow().clone().expect("unlocked");
    roblox.see("cookie-a", in_server(PLACE, SERVER));
    let preset_target = app.preset_follow_target(vault.clone(), &preset, &[account()], &Cancel::new()).await;
    c.check(
        "a preset that follows a player: opens into their server, its windows following them",
        preset_target.as_ref().is_ok_and(|t| t.place_id == PLACE && t.job_id.as_deref() == Some(SERVER) && t.follow.as_ref() == Some(follow)),
        format!("{preset_target:?}"),
    );
    roblox.see("cookie-a", offline());
    let refused = app.preset_follow_target(vault.clone(), &preset, &[account()], &Cancel::new()).await;
    let refused_words = refusal(&refused);
    c.check(
        "…the player offline: the preset doesn't run, and says why",
        refused_words.as_deref().is_some_and(|m| m.contains("Friend is offline")),
        format!("{refused:?}"),
    );

    // ── Close wins over a rejoin already under way ──
    let _ = app.vault_mut(|d| d.settings.multi_instance = false);
    let full = answer(200, Some(6), Some("The game is full"));
    let let_in = answer(200, Some(2), None);
    let mut card_rejoining = true;
    const NOTHING: &str = "queue 0, pending 0, rejoining false";
    for (answer_now, stop, what) in [
        (full.clone(), 0, "Close while a rejoin looks for its player: the rejoin stops, it doesn't wait to look again"),
        (let_in.clone(), 0, "…nor goes on to open the window (Roblox busy: it would go back in the queue)"),
        (full.clone(), 1, "…its rejoin called off on the card: stops too"),
    ] {
        let (after, card) = stopped_under_way(app, roblox, id, answer_now, |app| match stop {
            0 => {
                app.close_accounts(&[id], false);
            }
            _ => {
                app.cancel_rejoin(id);
            }
        })
        .await;
        card_rejoining &= card;
        c.check(what, after == NOTHING, &after);
    }
    c.check("…and while it is under way, the card says it's rejoining (Cancel and Close on it), not idle", card_rejoining, ());
    let path = vault.file_path().to_string_lossy().into_owned();
    let (after, _) = stopped_under_way(app, roblox, id, let_in, move |app| {
        app.stop_scheduled_run(ScheduledRun::new(crate::core::base::new_guid(), path.clone(), "Night farm".into(), vec![id]));
    })
    .await;
    c.check("…and a scheduled stop that comes meanwhile stops it", after == NOTHING, &after);
    *app.rejoin.ask_join.borrow_mut() = Some(plain_ask);

    // ── a follow preset holds the launch while it looks for the player ──
    let preset_id = preset.id;
    let _ = app.vault_mut(|d| d.settings.presets.push(preset.clone()));
    roblox.see("cookie-a", in_server(PLACE, SERVER));
    app.join_player.follow_seen.borrow_mut().clear();
    roblox.presence_asked.store(false, Ordering::SeqCst);
    roblox.gate_open.store(false, Ordering::SeqCst);
    roblox.gated.store(true, Ordering::SeqCst);
    let preset_run: Rc<RefCell<Option<crate::core::base::Res<Value>>>> = Rc::new(RefCell::new(None));
    {
        let (app, out) = (app.clone(), preset_run.clone());
        rt::spawn(async move {
            let got = call(&app, "runPreset", json!({ "id": preset_id, "force": false })).await;
            *out.borrow_mut() = Some(got);
        });
    }
    let begun = Instant::now();
    while !roblox.presence_asked.load(Ordering::SeqCst) && begun.elapsed() < Duration::from_secs(10) {
        rt::sleep(Duration::from_millis(20)).await;
    }
    // A banned account, so this Play never gets as far as Roblox whatever happens.
    app.update_account(other_id, |a| a.banned = Some(true));
    let second = call(app, "launch", json!({ "ids": [other_id], "placeInput": PLACE.to_string(), "jobId": "" })).await;
    let second_words = refusal(&second);
    c.check(
        "a follow preset looking for its player: a Play meanwhile is told a launch is under way",
        second_words.as_deref().is_some_and(|m| m.starts_with("Already launching")),
        format!("{second:?}"),
    );
    let _ = call(app, "cancelLaunch", json!({})).await;
    roblox.gate_open.store(true, Ordering::SeqCst);
    roblox.gated.store(false, Ordering::SeqCst);
    let begun = Instant::now();
    while preset_run.borrow().is_none() && begun.elapsed() < Duration::from_secs(30) {
        rt::sleep(Duration::from_millis(50)).await;
    }
    let stopped = preset_run.borrow().clone();
    c.check(
        "…Stop meanwhile stops the preset before it opens anything",
        matches!(&stopped, Some(Ok(v)) if v["cancelled"] == true || v["Cancelled"] == true) && app.controller.pending.borrow().is_empty(),
        format!("{stopped:?}"),
    );
    c.check("…and the launch is free again", app.controller.launch_cts.borrow().is_none(), ());
    app.update_account(other_id, |a| a.banned = None);
    let _ = app.vault_mut(|d| d.settings.presets.retain(|p| p.id != preset_id));
}

/// A rejoin following its player, under way and waiting on Roblox's join answer for the player's server, when `stop`
/// comes; then Roblox answers `answer_now`. What is left of it afterwards, and whether the card said it was rejoining.
async fn stopped_under_way(app: &Rc<App>, roblox: &Arc<Roblox>, id: Guid, answer_now: JoinAnswer, stop: impl FnOnce(&Rc<App>)) -> (String, bool) {
    app.controller.running.borrow_mut().clear();
    app.controller.rejoin_queue.borrow_mut().clear();
    app.join_player.follow_seen.borrow_mut().clear();
    roblox.see("cookie-a", in_server(PLACE, SERVER));
    let gate: Arc<Mutex<Option<JoinAnswer>>> = Arc::new(Mutex::new(None));
    let asked_now = Arc::new(AtomicI32::new(0));
    {
        let (gate, asked_now) = (gate.clone(), asked_now.clone());
        *app.rejoin.ask_join.borrow_mut() = Some(Arc::new(move |_, _, _| {
            asked_now.store(1, Ordering::SeqCst);
            loop {
                if let Some(a) = gate.lock().unwrap().clone() {
                    return Ok(a);
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }));
    }
    let done = Rc::new(RefCell::new(false));
    {
        let (app, done) = (app.clone(), done.clone());
        rt::spawn(async move {
            app.rejoin(RejoinRequest::new(id, followed(PLACE), Stamp::utc_now())).await;
            *done.borrow_mut() = true;
        });
    }
    let begun = Instant::now();
    while asked_now.load(Ordering::SeqCst) == 0 && begun.elapsed() < Duration::from_secs(10) {
        rt::sleep(Duration::from_millis(20)).await;
    }
    let card = !app.account_dto(&acc(app, id))["rejoin"].is_null();
    stop(app);
    *gate.lock().unwrap() = Some(answer_now);
    let begun = Instant::now();
    while !*done.borrow() && begun.elapsed() < Duration::from_secs(30) {
        rt::sleep(Duration::from_millis(50)).await;
    }
    let state = format!(
        "queue {}, pending {}, rejoining {}",
        app.controller.rejoin_queue.borrow().len(),
        app.controller.pending.borrow().len(),
        app.controller.rejoining.get()
    );
    app.controller.rejoin_queue.borrow_mut().clear();
    app.controller.pending.borrow_mut().clear();
    (state, card)
}
