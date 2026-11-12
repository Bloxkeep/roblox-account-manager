// Tests/Bloxkeep.Tests/ClaimsSuite.cs: which Roblox window is which launch's. Each launch link has a browser tracker id
// of its own, which the client's command line carries: a window naming a launch's is that launch's whatever started
// first, one naming another isn't, and one naming none goes by the order of starts. The C# read command lines off
// stand-in processes (cmd, ping); starting those is skipped here. The windows handed to launches are made-up process ids,
// nothing of Roblox runs.
use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{Arc, Mutex},
};

use regex::Regex;

use super::{
    Checks,
    rejoin::{no_network, quiet_app, seed},
    unlock,
};
use crate::core::{
    app::{
        App,
        controller::{LaunchRef, PendingLaunch, start_error_text},
        schedule::ScheduledRun,
    },
    base::{Fail, Guid, Stamp, new_guid},
    game_target::GameTarget,
    models::Account,
    multi_instance,
    roblox_api::fail_of,
    roblox_launcher::build_launch_uri,
    roblox_log_watcher::{Tail, apply_text},
    roblox_processes,
};

const FIRST: &str = "111111111111";
const SECOND: &str = "222222222222";

fn launch(who: Guid, seconds_ago: f64, tracker: &str, preset: Option<Guid>) -> Rc<PendingLaunch> {
    let mut l = PendingLaunch::new(who, HashSet::new(), Stamp::utc_now().plus_secs(-seconds_ago), Some(GameTarget::new(1818, None, None, None)), false, preset);
    l.tracker = Some(tracker.to_string());
    Rc::new(l)
}

fn window(app: &App, who: Guid) -> i32 {
    app.controller.running.borrow().get(&who).map(|r| r.0).unwrap_or(0)
}

fn has(list: &[Rc<PendingLaunch>], l: &Rc<PendingLaunch>) -> bool {
    list.iter().any(|x| Rc::ptr_eq(x, l))
}

fn reset(app: &App) {
    app.controller.pending.borrow_mut().clear();
    app.launching.late.borrow_mut().clear();
    app.controller.running.borrow_mut().clear();
}

pub async fn run(c: &mut Checks, dir: &str) {
    let link = build_launch_uri("TICKET", Some(&GameTarget::new(1818, None, None, None)), Some(FIRST));
    c.check("a launch link names the browser tracker id it was given", link.contains(&format!("+browsertrackerid:{FIRST}")), link.replace("TICKET", "…"));
    let fresh = build_launch_uri("TICKET", None, None);
    c.check("…and a new one when none is given", Regex::new(r"\+browsertrackerid:\d{12}\+").unwrap().is_match(&fresh), ());

    for what in [
        "a client started with a launch link: its browser tracker id read from its command line",
        "…one started by Roblox's launcher of old (-b N) too",
        "…one naming none: none",
    ] {
        c.skip(what, "starts a program (stand-in processes: cmd waiting on ping)");
    }
    c.check("…a process that isn't there: not read", roblox_processes::launch_tracker_of(i32::MAX - 7, 1).is_none(), ());

    no_network();
    let (fa, fb) = (Account { username: "First".into(), user_id: 31, ..Default::default() }, Account { username: "Second".into(), user_id: 32, ..Default::default() });
    let (a, b) = (fa.id, fb.id);
    seed(vec![fa, fb]);
    let (app, _said) = quiet_app();
    unlock(&app).await;

    let updating = Rc::new(Cell::new(false));
    let claim = |windows: &[(i32, Option<&str>)]| {
        let map: HashMap<i32, Option<String>> = windows.iter().map(|(p, t)| (*p, t.map(str::to_string))).collect();
        let pids: HashSet<i32> = map.keys().copied().collect();
        let up = updating.clone();
        app.claim_windows(&pids, &|_| true, &|_| Vec::new(), &|pid| map.get(&pid).cloned().flatten(), &move |_| up.get(), Stamp::utc_now());
    };
    let pending_len = || app.controller.pending.borrow().len();

    // Two accounts opening, each window naming its launch: each gets its own, whichever started first.
    app.controller.pending.borrow_mut().push(launch(a, 30.0, FIRST, None));
    app.controller.pending.borrow_mut().push(launch(b, 10.0, SECOND, None));
    claim(&[(1001, Some(FIRST)), (1002, Some(SECOND))]);
    c.check(
        "two launches, each window naming its own: each account gets its window",
        window(&app, a) == 1001 && window(&app, b) == 1002,
        format!("{} {}", window(&app, a), window(&app, b)),
    );
    // A launch past its wait and the next account's still opening: the window that comes, naming the first, is the first's.
    reset(&app);
    app.launching.late.borrow_mut().push(launch(a, 150.0, FIRST, None));
    app.controller.pending.borrow_mut().push(launch(b, 10.0, SECOND, None));
    claim(&[(1003, Some(FIRST))]);
    c.check(
        "a late launch's window, the next account still opening: the late one's, not the next account's",
        window(&app, a) == 1003 && window(&app, b) == 0 && pending_len() == 1,
        format!("{} {}", window(&app, a), window(&app, b)),
    );
    // A window the person opened from the website (its tracker id is the browser's): no launch's.
    reset(&app);
    app.controller.pending.borrow_mut().push(launch(b, 10.0, SECOND, None));
    claim(&[(1004, Some("999999999999"))]);
    c.check(
        "a window naming another tracker id (the person's own, from the website): not taken for a launch's",
        window(&app, b) == 0 && pending_len() == 1,
        window(&app, b),
    );
    app.launching.late.borrow_mut().push(launch(a, 150.0, FIRST, None));
    claim(&[(1004, Some("999999999999"))]);
    c.check("…nor for a late one's", window(&app, a) == 0, window(&app, a));
    // A command line naming none, or not read: the order of starts, as before.
    reset(&app);
    app.controller.pending.borrow_mut().push(launch(b, 10.0, SECOND, None));
    claim(&[(1005, Some(""))]);
    c.check("a window naming no tracker id: taken by the order of starts, as before", window(&app, b) == 1005, window(&app, b));
    reset(&app);
    app.controller.pending.borrow_mut().push(launch(b, 10.0, SECOND, None));
    claim(&[(1006, None)]);
    c.check("…and one whose command line can't be read", window(&app, b) == 1006, window(&app, b));
    reset(&app);

    // An outdated client hands its launch to Roblox's updater and quits; the updated client comes a while later.
    let handed = launch(b, 20.0, SECOND, None);
    app.controller.pending.borrow_mut().push(handed.clone());
    app.controller.pending_seen_alive.borrow_mut().insert(LaunchRef(handed.clone()), Stamp::utc_now().plus_secs(-10.0));
    updating.set(true);
    claim(&[]);
    c.check(
        "every client of a launch quit, Roblox's updater started since: still waited for",
        has(&app.controller.pending.borrow(), &handed),
        pending_len(),
    );
    updating.set(false);
    app.controller.pending_seen_alive.borrow_mut().insert(LaunchRef(handed.clone()), Stamp::utc_now().plus_secs(-10.0));
    claim(&[]);
    c.check("…without it: no longer waited for, as before", !has(&app.controller.pending.borrow(), &handed), pending_len());
    reset(&app);

    // Why Roblox wouldn't start, as the person reads it.
    c.skip(".NET's message for a start that failed names the program (the link, when Windows opens one)", "starts a program (a missing one, to get Windows' error)");
    // What a start that failed gives here (roblox_launcher: the spawn's io error as a Win32Exception), without starting anything.
    let ticketed = format!("{dir}\\gameinfo_SECRET-TICKET.exe");
    // Worded as .NET's (the program named) so the check still means something: the spawn's own text doesn't name it.
    let failed = fail_of(
        "Win32Exception",
        format!("An error occurred trying to start process '{ticketed}' with working directory 'C:\\'. {}", std::io::Error::from_raw_os_error(2)),
    );
    let said = start_error_text(&failed);
    c.check("…never shown: a sentence of Bloxkeep's with Windows' error number", !said.contains("SECRET") && said.contains("(error 2)"), &said);
    let link_named = build_launch_uri("SECRET-TICKET", Some(&GameTarget::new(1818, None, None, None)), None);
    let shell_failed = fail_of(
        "Win32Exception",
        format!("An error occurred trying to start process '{link_named}' with working directory 'C:\\'. No application is associated."),
    );
    let said = start_error_text(&shell_failed);
    c.check("…nor the link Windows was asked to open", !said.contains("SECRET"), &said);
    let not_installed = Fail::user("Bloxstrap is not installed.");
    let said = start_error_text(&not_installed);
    c.check("Bloxkeep's own sentence (a launcher not installed) is shown as it is", said == "Bloxstrap is not installed.", &said);
    c.check("…a ticket in one hidden all the same", !start_error_text(&Fail::user(format!("gone: {link_named}"))).contains("SECRET"), ());

    // A launch handed to Roblox's updater and past its 2 minutes, the updater still downloading: still waited for.
    let handed_over = |l: &Rc<PendingLaunch>| app.launching.handed_over.borrow().iter().any(|(x, _)| Rc::ptr_eq(x, l));
    let expire = |up: bool| {
        app.expire_pending(Stamp::utc_now(), &move |_| up);
    };
    let slow = launch(b, 150.0, SECOND, None);
    app.controller.pending.borrow_mut().push(slow.clone());
    app.launching.handed_over.borrow_mut().push((slow.clone(), (4242, None)));
    expire(true);
    c.check(
        "handed to Roblox's updater, past 2 minutes, the updater still at it: still waited for",
        has(&app.controller.pending.borrow(), &slow) && handed_over(&slow),
        (),
    );
    expire(false);
    c.check(
        "…once the updater is done without a window: no longer waited for, as before",
        !has(&app.controller.pending.borrow(), &slow) && !handed_over(&slow),
        (),
    );
    let stuck = launch(b, 60.0 * 8.0, SECOND, None);
    app.controller.pending.borrow_mut().push(stuck.clone());
    app.launching.handed_over.borrow_mut().push((stuck.clone(), (4242, None)));
    expire(true);
    c.check("…nor for ever: past 7 minutes, given up whatever the updater does", !has(&app.controller.pending.borrow(), &stuck), ());
    reset(&app);

    // Which client handed its launch to the updater, by its log.
    const UPDATE_START: &str = r#"2026-10-30T04:53:44.774Z,0.774840,9b3c,6,Info [FLog::UpdateController] Update thread: start ""C:\Users\A\AppData\Local\Roblox\Versions\version-1\RobloxPlayerInstaller.exe" roblox-player:1+launchmode:play+gameinfo:TICKET+launchtime:1+browsertrackerid:111111111111""#;
    const BACKGROUND: &str = "2026-10-30T04:53:44.770Z,0.770000,9b3c,6,Info [FLog::UpdateController] Update mode is chosen as BACKGROUND. Telemetry sent";
    const REQUIRED: &str = "2026-10-30T04:53:44.691Z,0.691337,54a0,6,Info [FLog::UpdateController] Update check thread: updateRequired TRUE";
    const GAME_START: &str = "2026-10-30T04:53:46.342Z,2.342439,956c,6 [FLog::SingleSurfaceApp] launchUGCGameInternal";
    let follow = |pid: i32, lines: &[&str]| {
        let mut tail = Tail::new("stand-in.log".into(), pid);
        apply_text(&mut tail, &(lines.join("\n") + "\n"));
        app.controller.logs.borrow_mut().tails.insert(pid, tail);
    };
    follow(5001, &[REQUIRED, UPDATE_START]);
    follow(5002, &[REQUIRED, BACKGROUND, UPDATE_START, GAME_START]);
    follow(5003, &[GAME_START]);
    let hand_off = |pid: i32| app.controller.logs.borrow().update_hand_off_at(pid);
    c.check(
        "a client that started Roblox's updater in the foreground handed its launch to it",
        hand_off(5001).is_some() && app.may_have_handed_to_updater(5001),
        (),
    );
    let state = app.controller.logs.borrow().state_of(5002);
    c.check(
        "…one updating in the background didn't: it went on into its game",
        hand_off(5002).is_none() && !app.may_have_handed_to_updater(5002) && state.as_ref().is_some_and(|s| s.status == "joining"),
        &state,
    );
    c.check("…nor one that started no update", hand_off(5003).is_none() && !app.may_have_handed_to_updater(5003), ());
    c.check("…one whose log isn't found yet (it quit within a second or two) may have", app.may_have_handed_to_updater(5004), ());
    app.controller.logs.borrow_mut().tails.clear();

    // Roblox's notification-area copy held the name Multi-Roblox takes: once it is ended with no Roblox left, the name is
    // taken before the launch's client can; with a Roblox still running it can't be.
    let _ = app.vault_mut(|d| d.settings.multi_instance = true);
    let was_armed = multi_instance::armed();
    multi_instance::disable();
    app.arm_multi_if_free(&[4242]);
    c.check("a Roblox still running: Multi-Roblox not taken", !multi_instance::armed(), ());
    // Taken only with no Roblox on this PC (a real one holds the name: nothing to check then), and let go at once.
    if roblox_processes::get_player_pids().is_empty() {
        app.arm_multi_if_free(&[]);
        let taken = multi_instance::armed();
        multi_instance::disable();
        c.check("…none left: taken at once", taken, ());
    }
    multi_instance::disable();
    let _ = app.vault_mut(|d| d.settings.multi_instance = false);
    if was_armed {
        multi_instance::enable();
    }

    // Opened again after Close all: the earlier launch still awaited late leaves the launches to close as well.
    let late_one = launch(a, 150.0, FIRST, None);
    let closing = launch(a, 5.0, FIRST, None);
    app.launching.late.borrow_mut().push(late_one.clone());
    app.controller.close_when_claimed.borrow_mut().insert(LaunchRef(late_one.clone()));
    app.controller.pending.borrow_mut().push(closing.clone());
    app.controller.close_when_claimed.borrow_mut().insert(LaunchRef(closing.clone()));
    app.forget_late(a);
    c.check(
        "opened again: the earlier late launch is forgotten, from the launches to close too",
        !has(&app.launching.late.borrow(), &late_one) && !app.controller.close_when_claimed.borrow().contains(&LaunchRef(late_one.clone())),
        (),
    );
    c.check("…one still opening, to be closed, isn't", app.controller.close_when_claimed.borrow().contains(&LaunchRef(closing.clone())), ());
    app.controller.close_when_claimed.borrow_mut().clear();
    reset(&app);

    // Closed while its window is late: the window that comes later is closed as it is claimed.
    let closed_pids: Arc<Mutex<Vec<i32>>> = Arc::new(Mutex::new(Vec::new()));
    let real_close = app.launching.close_client.borrow().clone();
    {
        let closed = closed_pids.clone();
        *app.launching.close_client.borrow_mut() = Some(Arc::new(move |pid, _| closed.lock().unwrap().push(pid)));
    }
    let closed_now = || closed_pids.lock().unwrap().clone();
    app.launching.late.borrow_mut().push(launch(a, 150.0, FIRST, None));
    app.close_accounts(&[a], false);
    claim(&[(1010, Some(FIRST))]);
    c.check(
        "Close on an account whose window is late: that window is closed once it comes, not kept",
        window(&app, a) == 0 && closed_now().contains(&1010),
        format!("{} closed {:?}", window(&app, a), closed_now()),
    );
    reset(&app);
    app.controller.close_when_claimed.borrow_mut().clear();

    let preset_id = new_guid();
    app.launching.late.borrow_mut().push(launch(a, 150.0, FIRST, Some(preset_id)));
    let preset_windows = app.preset_windows(preset_id);
    c.check("…a preset whose window is late counts it as open, so its Close is offered", preset_windows.contains(&a), preset_windows.len());
    let vault_path = app.controller.vault.borrow().as_ref().map(|v| v.file_path().to_string_lossy().into_owned()).unwrap_or_default();
    let run = ScheduledRun::new(preset_id, vault_path, "Night farm".into(), vec![a]);
    app.schedule.scheduled_runs.borrow_mut().push(run.clone());
    app.stop_scheduled_run(run);
    claim(&[(1011, Some(FIRST))]);
    c.check(
        "…its scheduled stop too: the late window is closed once it comes",
        window(&app, a) == 0 && closed_now().contains(&1011),
        format!("{} closed {:?}", window(&app, a), closed_now()),
    );
    *app.launching.close_client.borrow_mut() = real_close;
    app.controller.close_when_claimed.borrow_mut().clear();
    reset(&app);

    // A rejoin whose client quit before its window: tried again later, a few times, as a drop in game is.
    app.update_account(a, |x| x.auto_rejoin = true);
    let rejoin = |tries: i32| {
        let mut l = PendingLaunch::new(a, HashSet::new(), Stamp::utc_now().plus_secs(-20.0), Some(GameTarget::new(1818, None, None, None)), true, None);
        l.open_retries = tries;
        let l = Rc::new(l);
        app.controller.pending.borrow_mut().push(l.clone());
        app.controller.pending_seen_alive.borrow_mut().insert(LaunchRef(l), Stamp::utc_now().plus_secs(-10.0));
    };
    rejoin(0);
    claim(&[]);
    let (len, tries, due) = {
        let q = app.controller.rejoin_queue.borrow();
        (q.len(), q.first().map(|r| r.open_retries), q.first().map(|r| r.due))
    };
    c.check(
        "a rejoin whose client quit before its window: tried again 30 s on",
        len == 1 && tries == Some(1) && due.is_some_and(|d| d > Stamp::utc_now().plus_secs(20.0)),
        len,
    );
    app.controller.rejoin_queue.borrow_mut().clear();
    rejoin(3);
    claim(&[]);
    let len = app.controller.rejoin_queue.borrow().len();
    c.check("…its tries used up: not again (it is said instead)", len == 0 && pending_len() == 0, len);
    app.update_account(a, |x| x.auto_rejoin = false);
    rejoin(0);
    claim(&[]);
    let len = app.controller.rejoin_queue.borrow().len();
    c.check("…nor with auto-rejoin off", len == 0, len);
    reset(&app);
    app.controller.rejoin_queue.borrow_mut().clear();

    // Roblox's updater read from Windows' process list: the C# started a stand-in under its name (ping, copied).
    for what in [
        "Roblox's updater running, started after the launch: seen",
        "…one started before the launch isn't this launch's",
        "…and once it has quit, none",
    ] {
        c.skip(what, "starts a program (a copy of ping named RobloxPlayerInstaller.exe)");
    }
}
