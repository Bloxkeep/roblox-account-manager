// Tests/Bloxkeep.Tests/RejoinWithSuite.cs: a preset's "Rejoin server" (LaunchPreset::rejoin_with): saved only with a
// game, as "any" or one of its ticked accounts; a dropped window rejoins the server its chosen account (or any other of
// the preset's) plays in, as that window's log says, passing over a server Roblox calls full; nobody in a game, or no
// server that will take it: any server. Windows and logs are stand-ins; nothing is launched.
use std::{
    collections::HashSet,
    rc::Rc,
    sync::{Arc, Mutex},
};

use serde_json::json;

use super::{
    Checks, call,
    rejoin::{answer, no_network, quiet_app, seed},
    unlock,
};
use crate::core::{
    app::{
        App,
        controller::{PendingLaunch, RejoinRequest},
    },
    base::{Cancel, Guid, Stamp},
    game_target::GameTarget,
    models::{Account, LaunchPreset},
    roblox_log_watcher::{ClientState, Tail},
};

const PLACE: i64 = 4242;

fn preset(app: &App) -> LaunchPreset {
    app.vault(|d| d.settings.presets[0].clone()).expect("unlocked")
}

async fn save(app: &Rc<App>, ids: &[Guid], rejoin_with: &str, place: &str) -> LaunchPreset {
    let id = app.vault(|d| d.settings.presets.first().map(|p| p.id.to_string())).ok().flatten().unwrap_or_default();
    let saved = call(app, "savePreset", json!({ "id": id, "name": "Farm", "ids": ids, "placeInput": place, "server": "any", "rejoinWith": rejoin_with })).await;
    if let Err(e) = saved {
        println!("      (savePreset refused: {e})");
    }
    preset(app)
}

/// A window of `account` opened `minutes_ago`, its log's state as given (a stand-in log, never read).
fn window(app: &App, account: Guid, pid: i32, status: &str, job: Option<&str>, minutes_ago: f64, preset: Option<Guid>) {
    let launch = Rc::new(PendingLaunch::new(
        account,
        HashSet::new(),
        Stamp::utc_now().plus_secs(-minutes_ago * 60.0),
        Some(GameTarget::new(PLACE, None, None, None)),
        false,
        preset,
    ));
    app.controller.running.borrow_mut().insert(account, (pid, launch));
    let mut tail = Tail::new(format!("stand-in-{pid}.log"), pid);
    tail.state = Some(ClientState::new(status, Some(PLACE), job.map(str::to_string), None, Stamp::utc_now()));
    app.controller.logs.borrow_mut().tails.insert(pid, tail);
}

pub async fn run(c: &mut Checks, _dir: &str) {
    no_network();
    let made = |name: &str, cookie: &str, user_id: i64| Account { username: name.into(), cookie: cookie.into(), user_id, cookie_valid: Some(true), ..Default::default() };
    let (a1, a2, a3, other) = (made("farm1", "x1", 1001), made("farm2", "x2", 1002), made("farm3", "x3", 1003), made("main", "x4", 1004));
    let (a1, a2, a3, other) = (a1.id, a2.id, a3.id, other.id);
    seed(vec![made_with(a1, "farm1", "x1", 1001), made_with(a2, "farm2", "x2", 1002), made_with(a3, "farm3", "x3", 1003), made_with(other, "main", "x4", 1004)]);
    let (app, _said) = quiet_app();
    unlock(&app).await;
    // The game's name isn't asked from Roblox here.
    app.controller.games.borrow_mut().insert(PLACE, None);
    let ids = [a1, a2, a3];

    let p = save(&app, &ids, &a2.to_string(), "4242").await;
    c.check("a ticked account is saved as the one to rejoin with", p.rejoin_with == a2.to_string(), &p.rejoin_with);
    let dto = app.preset_dto(&p);
    c.check("…and comes back with the preset", dto["rejoinWith"].as_str() == Some(a2.to_string().as_str()), &dto["rejoinWith"]);
    let p = save(&app, &ids, &other.to_string(), "4242").await;
    c.check("an account the preset doesn't open: any server", p.rejoin_with.is_empty(), &p.rejoin_with);
    let p = save(&app, &ids, "any", "").await;
    c.check("no game: any server", p.rejoin_with.is_empty(), &p.rejoin_with);
    let p = save(&app, &ids, "nonsense", "4242").await;
    c.check("anything else: any server", p.rejoin_with.is_empty(), &p.rejoin_with);
    let p = save(&app, &ids, &a2.to_string(), "4242").await;

    // Windows of the preset's accounts, each with its log's state.
    window(&app, a2, 902, "ingame", Some("server-of-farm2"), 5.0, Some(p.id));
    window(&app, a3, 903, "ingame", Some("server-of-farm3"), 9.0, Some(p.id));

    let full: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let asked: Arc<Mutex<Vec<Option<String>>>> = Arc::new(Mutex::new(Vec::new()));
    {
        let (full, asked) = (full.clone(), asked.clone());
        *app.rejoin.ask_join.borrow_mut() = Some(Arc::new(move |_, target, _| {
            asked.lock().unwrap().push(target.job_id.clone());
            let is_full = target.job_id.as_ref().is_some_and(|j| full.lock().unwrap().contains(j));
            Ok(answer(200, Some(if is_full { 6 } else { 2 }), None))
        }));
    }
    let rejoin = |app: Rc<App>, account: Guid, preset: Guid, asked: Arc<Mutex<Vec<Option<String>>>>| async move {
        asked.lock().unwrap().clear();
        let request = RejoinRequest { preset_id: Some(preset), ..RejoinRequest::new(account, GameTarget::new(PLACE, None, None, None), Stamp::utc_now()) };
        let acc = app.find_account(account).expect("account");
        app.teammate_target(&acc, &request, &Cancel::new()).await.ok().flatten()
    };
    let asked_now = || asked.lock().unwrap().iter().map(|j| j.clone().unwrap_or_default()).collect::<Vec<_>>();

    let target = rejoin(app.clone(), a1, p.id, asked.clone()).await;
    c.check(
        "dropped: rejoins the server its chosen account plays in",
        target.as_ref().is_some_and(|t| t.place_id == PLACE && t.job_id.as_deref() == Some("server-of-farm2")),
        &target,
    );
    full.lock().unwrap().insert("server-of-farm2".into());
    let target = rejoin(app.clone(), a1, p.id, asked.clone()).await;
    c.check(
        "…that server full: any server (not another account's when one is chosen)",
        target.is_none() && asked_now() == ["server-of-farm2"],
        asked_now().join(","),
    );
    let target = rejoin(app.clone(), a2, p.id, asked.clone()).await;
    c.check("the chosen account itself dropping: any server", target.is_none(), &target);

    let p = save(&app, &ids, "any", "4242").await;
    full.lock().unwrap().clear();
    full.lock().unwrap().insert("server-of-farm3".into());
    let target = rejoin(app.clone(), a1, p.id, asked.clone()).await;
    c.check(
        "with another account: the earliest opened first, a full server passed over for the next one",
        target.as_ref().is_some_and(|t| t.job_id.as_deref() == Some("server-of-farm2")) && asked_now() == ["server-of-farm3", "server-of-farm2"],
        asked_now().join(","),
    );
    full.lock().unwrap().clear();
    full.lock().unwrap().insert("server-of-farm2".into());
    window(&app, a3, 903, "joining", None, 9.0, Some(p.id));
    let target = rejoin(app.clone(), a1, p.id, asked.clone()).await;
    c.check("…nobody else in a game (one full, one still joining): any server", target.is_none(), &target);
    full.lock().unwrap().clear();
    let target = rejoin(app.clone(), a1, p.id, asked.clone()).await;
    c.check("…the full one has room again: rejoins it", target.as_ref().is_some_and(|t| t.job_id.as_deref() == Some("server-of-farm2")), &target);

    // A window of the same account opened by hand (no preset) doesn't count as the preset's.
    app.controller.running.borrow_mut().shift_remove(&a2);
    window(&app, other, 904, "ingame", Some("server-of-main"), 3.0, Some(p.id));
    let launch = Rc::new(PendingLaunch::new(other, HashSet::new(), Stamp::utc_now(), Some(GameTarget::new(PLACE, None, None, None)), false, None));
    app.controller.running.borrow_mut().insert(other, (904, launch));
    let target = rejoin(app.clone(), a1, p.id, asked.clone()).await;
    c.check("another account's window not opened by the preset: not followed", target.is_none(), &target);
    app.controller.running.borrow_mut().clear();
}

fn made_with(id: Guid, name: &str, cookie: &str, user_id: i64) -> Account {
    Account { id, username: name.into(), cookie: cookie.into(), user_id, cookie_valid: Some(true), ..Default::default() }
}
