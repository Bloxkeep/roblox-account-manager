// Tests/Bloxkeep.Tests/AdoptSuite.cs: a window opened outside Bloxkeep (Roblox's own app) that joined a game. The join
// report in its log names the signed-in user, and Bloxkeep takes it as that account's window. Not a stranger's, not a
// second window for an account that has one, not while a launch could still claim it. No real window is involved.
use std::{cell::RefCell, collections::HashSet, rc::Rc};

use super::{
    Checks,
    rejoin::{no_network, quiet_app, seed},
    unlock,
};
use crate::core::{
    app::{App, controller::PendingLaunch},
    base::{Guid, Stamp, new_guid},
    game_target::GameTarget,
    models::Account,
    roblox_api::{escape_data_string, realm},
    roblox_log_watcher::{ClientState, Tail, apply},
    roblox_processes::{self, PlayerProcess},
};

const PLACE: i64 = 4242;

fn report(user: i64) -> String {
    format!(
        "2026-11-02T22:26:48.946Z,1.5,1a2b,6 [FLog::GameJoinLoadTime] Report game_join_loadtime: placeid:{PLACE}, join_time:1.5, universeid:9, referral_page:Home, sid:b73ee3cd-d8e0-4fcc-850b-b461be75a8e1, clienttime:1.2, userid:{user},"
    )
}

fn window(app: &App, pid: i32, line: &str) {
    let mut tail = Tail::new("stand-in.log".into(), pid);
    tail.state = Some(ClientState::new("ingame", Some(PLACE), Some("job".into()), None, Stamp::utc_now()));
    apply(&mut tail, line);
    app.controller.logs.borrow_mut().tails.insert(pid, tail);
}

fn adopt(app: &App, pids: &[i32]) -> bool {
    let players: Vec<PlayerProcess> = pids.iter().map(|&pid| PlayerProcess { pid, ..Default::default() }).collect();
    app.adopt_windows_opened_elsewhere(&players)
}

fn pid_of(app: &App, id: Guid) -> i32 {
    app.controller.running.borrow().get(&id).map(|r| r.0).unwrap_or(0)
}

fn target_of(app: &App, id: Guid) -> Option<GameTarget> {
    app.controller.running.borrow().get(&id).and_then(|r| r.1.target.clone())
}

fn launcher(query: &str) -> String {
    format!(
        "roblox-player:1+launchmode:play+gameinfo:abc+launchtime:1+placelauncherurl:{}+browsertrackerid:7+robloxLocale:en_us",
        escape_data_string(&format!("https://www.roblox.com/Game/PlaceLauncher.ashx?{query}"))
    )
}

pub async fn run(c: &mut Checks, _dir: &str) {
    no_network();
    let made = |name: &str, user_id: i64, cookie: &str| Account { username: name.into(), user_id, cookie: cookie.into(), cookie_valid: Some(true), ..Default::default() };
    let (m, a) = (made("Main", 31, "cookie-main"), made("Alt", 32, "cookie-alt"));
    let (main, alt) = (m.id, a.id);
    seed(vec![m, a]);
    let (app, _said) = quiet_app();
    unlock(&app).await;
    let user_of = |pid: i32| app.controller.logs.borrow().user_id_of(pid);
    let running_len = || app.controller.running.borrow().len();

    window(&app, 900001, &report(31));
    c.check("the join report gives the signed-in user", user_of(900001) == Some(31), format!("{:?}", user_of(900001)));
    window(&app, 900002, "2026-11-02T22:26:40.000Z,1.0,1a2b,6 [FLog::Output] ! Joining game 'job' place 4242 at 1.2.3.4");
    c.check("…and other lines give none", user_of(900002).is_none(), ());

    window(&app, 900003, &report(99));
    let adopted = adopt(&app, &[900001, 900002, 900003]);
    c.check(
        "a window opened elsewhere, signed in as a vault account, becomes its window",
        adopted && pid_of(&app, main) == 900001,
        pid_of(&app, main),
    );
    c.check("…a stranger's window and one not yet in a game are left alone", running_len() == 1, ());

    app.controller.running.borrow_mut().shift_remove(&main);
    window(&app, 900004, &report(31));
    window(&app, 900005, &report(32));
    app.controller.pending.borrow_mut().push(Rc::new(PendingLaunch::new(alt, HashSet::new(), Stamp::utc_now(), Some(GameTarget::new(PLACE, None, None, None)), false, None)));
    c.check("not while a launch could still claim the window", !adopt(&app, &[900004, 900005]) && running_len() == 0, ());
    app.controller.pending.borrow_mut().clear();

    let adopted = adopt(&app, &[900004, 900001, 900005]);
    c.check(
        "one window per account: the first one",
        adopted && pid_of(&app, main) == 900004 && pid_of(&app, alt) == 900005,
        format!("main {} alt {}", pid_of(&app, main), pid_of(&app, alt)),
    );
    c.check("…and nothing again for windows already known", !adopt(&app, &[900004, 900005]), ());

    // Signed out in the window: the user it named is gone with it.
    window(&app, 900006, &report(31));
    if let Some(t) = app.controller.logs.borrow_mut().tails.get_mut(&900006) {
        apply(t, "2026-11-02T22:30:00.000Z,1.0,1a2b,6 [FLog::Output] logout sync cookie to native");
    }
    c.check("a sign-out in the window forgets the signed-in user", user_of(900006).is_none(), format!("{:?}", user_of(900006)));

    app.controller.running.borrow_mut().clear();
    // A Global and a Roblox VNG account can have the same user id (VNG counts its own): the log can't say which.
    let _ = app.vault_mut(|d| {
        d.accounts.push(made("Twin", 33, "cookie-twin"));
        d.accounts.push(Account { realm: realm::VNG.into(), ..made("TwinVng", 33, "cookie-twin-vng") });
    });
    window(&app, 900007, &report(33));
    c.check("a user id two accounts share (Global and VNG) adopts nothing", !adopt(&app, &[900007]) && running_len() == 0, running_len());

    // Being closed: Close all's sweep, or the monitor tab's Close of a window nobody tracked.
    app.controller.sweep_background_until.set(Stamp::utc_now().plus_secs(15.0));
    c.check("nothing is adopted during Close all's sweep", !adopt(&app, &[900004]) && running_len() == 0, ());
    app.controller.sweep_background_until.set(Stamp::from_unix_secs(-62_135_596_800));
    window(&app, 900009, &report(31));
    roblox_processes::close(900009, false); // an odd number: never a real process id
    c.check("…nor a window being closed", !adopt(&app, &[900009]) && running_len() == 0, ());

    // A separate sign-in window (the tray's New Roblox window) isn't a vault account's client.
    app.tray.opening_home.set(true);
    c.check("nothing is adopted while a separate sign-in window opens", !adopt(&app, &[900004]) && running_len() == 0, ());
    app.tray.opening_home.set(false);

    // Saved soon, not on the window's thread in the tick, and not maximised as a window Bloxkeep opened.
    app.persist.save_due.set(None);
    let adopted = adopt(&app, &[900004]);
    c.check("an adopted window is saved with the next save, not at once", adopted && app.persist.save_due.get().is_some(), ());
    let maximised: Vec<i32> = app.presets.maximised.borrow().keys().copied().collect();
    c.check("…and isn't maximised by Settings' Maximise", maximised.contains(&900004), format!("{maximised:?}"));

    // Its server is unknown: only a window whose own start asked for a public server of the place it is in keeps that game.
    app.update_account(main, |a| a.auto_rejoin = true);
    let dropped = ClientState::new("disconnected", Some(PLACE), Some("job".into()), Some(277), Stamp::utc_now());
    let drop = |id: Guid| {
        let launch = app.controller.running.borrow().get(&id).map(|r| r.1.clone());
        launch.is_some_and(|l| app.queue_rejoin(id, &l, Some(&dropped)))
    };
    c.check("a window with no sign of a public server gets no game to rejoin", target_of(&app, main).is_none(), &target_of(&app, main));
    c.check("…so a drop isn't rejoined into a public server", !drop(main) && app.controller.rejoin_queue.borrow().is_empty(), ());
    const EXE: &str = "\"C:\\Users\\x\\AppData\\Local\\Roblox\\Versions\\version-1\\RobloxPlayerBeta.exe\" ";
    let command_line: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let real_url_of = app.processes.launch_url_of.borrow().clone();
    {
        // Only the PlaceLauncher link is taken from the command line, never the sign-in ticket on it.
        let line = command_line.clone();
        *app.processes.launch_url_of.borrow_mut() = Some(Rc::new(move |_, _| line.borrow().as_ref().map(|l| roblox_processes::place_launcher_url(l))));
    }
    let adopt_with = |line: Option<String>| {
        app.controller.running.borrow_mut().shift_remove(&main);
        *command_line.borrow_mut() = line;
        adopt(&app, &[900004]);
    };
    adopt_with(Some(format!("{EXE}{}", launcher(&format!("request=RequestGame&browserTrackerId=7&placeId={PLACE}&isPlayTogetherGame=false")))));
    let t = target_of(&app, main);
    c.check(
        "a window started for a public server of its place keeps that game",
        t.as_ref().is_some_and(|t| t.place_id == PLACE && t.job_id.is_none() && !t.is_private()),
        &t,
    );
    c.check("…and is rejoined after a drop", drop(main) && app.controller.rejoin_queue.borrow().len() == 1, ());
    app.controller.rejoin_queue.borrow_mut().clear();
    adopt_with(Some(format!("{EXE}{}", launcher(&format!("request=RequestPrivateGame&browserTrackerId=7&placeId={PLACE}&accessCode={}", new_guid())))));
    c.check("a private server's window gets none", target_of(&app, main).is_none(), &target_of(&app, main));
    adopt_with(Some(format!("{EXE}{}", launcher("request=RequestGame&browserTrackerId=7&placeId=77&isPlayTogetherGame=false"))));
    c.check("…nor one in another place than it started for", target_of(&app, main).is_none(), &target_of(&app, main));
    adopt_with(Some(format!("{EXE}--app")));
    c.check("…nor one started on Roblox's home screen", target_of(&app, main).is_none(), &target_of(&app, main));
    app.update_account(main, |a| a.auto_rejoin = false);
    *app.processes.launch_url_of.borrow_mut() = real_url_of;

    // A late launch's window is checked against the log's user before it is claimed.
    let account = |id: Guid| app.find_account(id).expect("account");
    c.check("a window signed in as another user isn't the account's", !app.claim_user_matches(900005, &account(main)), ());
    c.check("…one signed in as the account is", app.claim_user_matches(900004, &account(main)), ());
    c.check("…and one whose log hasn't named a user yet may be", app.claim_user_matches(900002, &account(main)), ());

    // Another vault's windows, still open after a switch, aren't written into this vault's open clients.
    let stranger = new_guid();
    let main_launch = app.controller.running.borrow().get(&main).map(|r| r.1.clone());
    if let Some(l) = main_launch {
        app.controller.running.borrow_mut().insert(stranger, (900011, l));
    }
    app.state.ended_here.borrow_mut().insert(stranger, (Some("job".into()), Some(PLACE), Stamp::utc_now()));
    app.state.ended_here.borrow_mut().insert(alt, (Some("job".into()), Some(PLACE), Stamp::utc_now()));
    app.note_open_sessions();
    let (open, ended): (Vec<Guid>, Vec<Guid>) = app
        .vault(|d| (d.open_sessions.iter().map(|s| s.account_id).collect(), d.ended_sessions.iter().map(|s| s.account_id).collect()))
        .unwrap_or_default();
    c.check(
        "another vault's open clients aren't noted in this one",
        !open.is_empty() && open.iter().all(|s| *s != stranger),
        format!("{open:?}"),
    );
    c.check(
        "…nor its windows that ended here",
        ended.contains(&alt) && ended.iter().all(|s| *s != stranger),
        format!("{ended:?}"),
    );
    app.controller.running.borrow_mut().clear();
    app.controller.logs.borrow_mut().tails.clear();
}
