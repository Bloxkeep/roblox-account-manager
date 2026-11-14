// Tests/Bloxkeep.Tests/RestartSuite.cs: Bloxkeep starting while Roblox windows already run (an update, a restart), with
// made-up Roblox logs this process holds open (the log watcher ties a log to whoever holds it): a long log, an old idle
// kick, and a client without a log. The logs are in a folder of the test's own (BLOXKEEP_ROBLOX_LOGS); no Roblox
// process is touched.
use std::{
    collections::HashSet,
    io::{Seek, SeekFrom, Write},
    rc::Rc,
    time::{Duration, Instant},
};

use super::{
    Checks, call,
    rejoin::{log_line, log_stamp, no_network, quiet_app, seed_with},
    unlock,
};
use crate::core::{
    app::{
        App,
        controller::{PendingLaunch, RejoinRequest},
    },
    base::{Guid, Stamp},
    game_target::GameTarget,
    models::{Account, WaitingRejoin},
    roblox_api::Presence,
    roblox_log_watcher::{ClientState, RobloxLogWatcher},
    rt,
};

const JOIN: &str = "[FLog::Output] ! Joining game '11111111-2222-3333-4444-555555555555' place 2753915549 at 10.0.0.1";
const CONNECTED: &str = "[FLog::Network] serverId: 10.0.0.1|1234";
const KICK: &str = "Info [DFLog::NetworkClient] Client:Disconnect setting replicator disconnect reason to 278";
const ENDED: &str = "[FLog::Network] Time to disconnect replication data: 0.4";
const PLACE: i64 = 2753915549;

fn fake_log(logs: &std::path::Path, tag: &str, text: &str) -> std::fs::File {
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .read(true)
        .write(true)
        .open(logs.join(format!("0.743.0.7431129_{}Z_Player_{tag}_last.log", log_stamp(Stamp::utc_now()))))
        .expect("a log");
    f.write_all(text.as_bytes()).expect("written");
    f.flush().expect("flushed");
    f
}

fn append(f: &mut std::fs::File, text: &str) {
    f.seek(SeekFrom::End(0)).expect("seek");
    f.write_all(text.as_bytes()).expect("written");
    f.flush().expect("flushed");
}

async fn until(mut get: impl FnMut() -> Option<ClientState>) -> Option<ClientState> {
    let begun = Instant::now();
    while begun.elapsed() < Duration::from_secs(15) {
        if let Some(v) = get() {
            return Some(v);
        }
        rt::sleep(Duration::from_millis(200)).await;
    }
    None
}

/// Sets a file's last write time `ago` before now.
fn written_ago(path: &std::path::Path, ago: Duration) {
    if let Ok(f) = std::fs::File::options().write(true).open(path) {
        let _ = f.set_modified(std::time::SystemTime::now() - ago);
    }
}

fn old_logs_and_crashes(c: &mut Checks, logs: &std::path::Path) {
    use crate::core::{roblox_crash_info, roblox_log_watcher::clean_old_logs};
    const DAY: u64 = 24 * 3600;
    // Roblox's logs older than a week go (Roblox keeps every one); the newest stay, and a clock set far ahead deletes nothing.
    let (old_log, new_log, note) = (logs.join("old_Player_last.log"), logs.join("new_Player_last.log"), logs.join("keep.txt"));
    for f in [&old_log, &new_log, &note] {
        std::fs::write(f, "x").expect("written");
    }
    written_ago(&old_log, Duration::from_secs(10 * DAY));
    written_ago(&note, Duration::from_secs(10 * DAY));
    c.check(
        "a clock set a year ahead: no Roblox log deleted",
        clean_old_logs(std::time::SystemTime::now() + Duration::from_secs(365 * DAY)) == 0 && new_log.exists() && old_log.exists(),
        (),
    );
    let cleaned = clean_old_logs(std::time::SystemTime::now());
    c.check(
        "Roblox's logs over a week old deleted, the recent one and other files kept",
        cleaned == 1 && !old_log.exists() && new_log.exists() && note.exists(),
        cleaned,
    );
    let _ = std::fs::remove_file(&new_log);
    let _ = std::fs::remove_file(&note);

    // A window that ended on its own: Roblox's crash reports since it opened, and Windows' record for its process id.
    let reports = logs.join("crashes").join("reports");
    std::fs::create_dir_all(&reports).expect("reports folder");
    let (old_dump, new_dump) = (reports.join("old.dmp"), reports.join("new.dmp"));
    std::fs::write(&old_dump, "x").expect("written");
    std::fs::write(&new_dump, "x").expect("written");
    written_ago(&old_dump, Duration::from_secs(2 * 3600));
    let dumps = roblox_crash_info::dumps_since(std::time::SystemTime::now() - Duration::from_secs(3600));
    c.check("crash reports: only the ones since the window opened", dumps == ["new.dmp"], dumps.join(","));
    let _ = std::fs::remove_dir_all(logs.join("crashes"));
    const EVENTS: &str = concat!(
        "<Event xmlns='x'><System><Provider Name='Application Error'/></System><EventData><Data Name='AppName'>Other.exe</Data>",
        "<Data Name='ProcessId'>0x10</Data></EventData></Event><Event xmlns='x'><System/><EventData><Data Name='AppName'>RobloxPlayerBeta.exe</Data>",
        "<Data Name='AppVersion'>0.747.0.7471481</Data><Data Name='ModuleName'>RobloxPlayerBeta.dll</Data><Data Name='ExceptionCode'>c0000005</Data>",
        "<Data Name='FaultingOffset'>0000000000123abc</Data><Data Name='ProcessId'>0x950c</Data></EventData></Event>"
    );
    let fault = roblox_crash_info::fault_in(EVENTS, 0x950c);
    c.check(
        "Windows' error record found by its hex process id",
        fault.as_deref().is_some_and(|f| f.contains("RobloxPlayerBeta.dll") && f.contains("c0000005")) && roblox_crash_info::fault_in(EVENTS, 0x11).is_none(),
        &fault,
    );
}

fn open(app: &App, account: Guid, pid: i32, started_at: Stamp) {
    let launch = Rc::new(PendingLaunch::new(account, HashSet::new(), started_at, Some(GameTarget::new(PLACE, None, None, None)), false, None));
    app.controller.running.borrow_mut().insert(account, (pid, launch));
}

pub async fn run(c: &mut Checks, dir: &str) {
    no_network();
    let logs = std::path::Path::new(dir).join("roblox-logs");
    std::fs::create_dir_all(&logs).expect("logs folder");
    // SAFETY: this suite's own process, before any other thread reads the environment.
    unsafe { std::env::set_var("BLOXKEEP_ROBLOX_LOGS", &logs) };
    let me = std::process::id() as i32;
    let line = log_line;
    old_logs_and_crashes(c, &logs);

    // A client running for 12 hours: its join is more than the 4 MB a read takes at once back.
    let t0 = Stamp::utc_now().plus_secs(-12.0 * 3600.0);
    let mut text = line(t0, JOIN) + &line(t0.plus_secs(3.0), CONNECTED);
    for i in 0..60000 {
        text.push_str(&line(t0.plus_secs(10.0 + i as f64), "[FLog::Output] Wrap-deformer fetching meshes resulted in error.  Skipping follow-on stages."));
    }
    let long = fake_log(&logs, "LONGTEST", &text);
    let mut watcher = RobloxLogWatcher::new();
    let state = until(|| {
        watcher.poll(&[me]);
        watcher.state_of(me)
    })
    .await;
    c.check("12-hour log: still known to be in its game", state.as_ref().is_some_and(|s| s.status == "ingame" && s.place_id == Some(PLACE)), &state);
    drop(long);
    drop(watcher);
    for f in std::fs::read_dir(&logs).expect("listed").flatten() {
        let _ = std::fs::remove_file(f.path());
    }

    let kicked = Account { username: "Kicked".into(), user_id: 11, ..Default::default() };
    let nolog = Account { username: "NoLog".into(), user_id: 12, ..Default::default() };
    let fresh = Account { username: "Fresh".into(), user_id: 13, ..Default::default() };
    let (kicked, nolog, fresh) = {
        let ids = (kicked.id, nolog.id, fresh.id);
        seed_with(vec![kicked, nolog, fresh], |d| d.settings.anti_afk = true);
        ids
    };
    let (app, _said) = quiet_app();
    unlock(&app).await;
    let started = app.relaunch.started.get();

    // Kicked for idling an hour ago, the window still on Roblox's message.
    let hour_ago = Stamp::utc_now().plus_secs(-3600.0);
    let mut kick_log = fake_log(
        &logs,
        "KICKTEST",
        &(line(hour_ago.plus_secs(-1800.0), JOIN) + &line(hour_ago.plus_secs(-1800.0 + 3.0), CONNECTED) + &line(hour_ago, KICK) + &line(hour_ago, ENDED)),
    );
    open(&app, kicked, me, Stamp::utc_now().plus_secs(-7200.0));
    let kick_state = until(|| {
        app.controller.logs.borrow_mut().poll(&[me]);
        app.controller.logs.borrow().state_of(me)
    })
    .await;
    c.check("old kick read from the log", kick_state.as_ref().is_some_and(|s| s.status == "disconnected" && s.reason == Some(278)), &kick_state);
    // Each idle kick told is also written to error.log.
    let error_log = std::path::Path::new(dir).join("error.log");
    let told = || std::fs::read_to_string(&error_log).map(|t| t.matches("Idle kick with anti-AFK on").count()).unwrap_or(0);
    let before = told();
    app.report_idle_kicks(&HashSet::from([me]));
    c.check("a kick from before Bloxkeep started isn't told again", told() == before, told() as i64 - before as i64);
    app.anti_afk.idle_kick_reported.borrow_mut().clear();
    let now_join = Stamp::utc_now();
    append(
        &mut kick_log,
        &(line(now_join, JOIN) + &line(now_join.plus_secs(1.0), CONNECTED) + &line(now_join.plus_secs(2.0), KICK) + &line(now_join.plus_secs(2.0), ENDED)),
    );
    let new_kick = until(|| {
        app.controller.logs.borrow_mut().poll(&[me]);
        app.controller.logs.borrow().state_of(me).filter(|s| s.at > started)
    })
    .await;
    app.report_idle_kicks(&HashSet::from([me]));
    c.check("a kick after the start is told", new_kick.is_some() && told() == before + 1, format!("{new_kick:?} {}", told() as i64 - before as i64));

    // A client open before the start whose log isn't found: no stay from its launch hours ago.
    const NO_LOG_PID: i32 = 999_991;
    open(&app, nolog, NO_LOG_PID, Stamp::utc_now().plus_secs(-3.0 * 3600.0));
    let stays_of = |id: Guid| app.vault(|d| d.play_sessions.iter().filter(|s| s.account_id == id).count()).unwrap_or(0);
    let last_stay = |id: Guid| app.vault(|d| d.play_sessions.iter().rev().find(|s| s.account_id == id).map(|s| s.started_at)).ok().flatten();
    let stays = stays_of(nolog);
    app.track_play(started.plus_secs(50.0));
    c.check("no-log client open before the start: no stay counted at once", stays_of(nolog) == stays, stays_of(nolog));
    app.track_play(started.plus_secs(180.0));
    let stay = last_stay(nolog);
    c.check(
        "after the log search: a stay from Bloxkeep's start, not from the launch",
        stay.is_some_and(|s| s.secs_since(started).abs() < 1.0),
        &stay,
    );

    // A client opened after the start still gets its no-log stay after 45 s, from its launch.
    const FRESH_PID: i32 = 999_992;
    let fresh_start = started.plus_secs(240.0);
    open(&app, fresh, FRESH_PID, fresh_start);
    app.track_play(fresh_start.plus_secs(50.0));
    let fresh_stay = last_stay(fresh);
    c.check(
        "client opened after the start: stay from its launch after 45 s",
        fresh_stay.is_some_and(|s| s.secs_since(fresh_start).abs() < 1.0),
        &fresh_stay,
    );

    // Clients closed here just before a restart (an update's): Roblox still says "in game" for minutes, which after the
    // restart must still read as that late report, not as playing on another device.
    const SERVER: &str = "11111111-2222-3333-4444-555555555555";
    let end = |at: Stamp| (Some(SERVER.to_string()), Some(PLACE), at);
    app.controller.running.borrow_mut().clear();
    app.state.ended_here.borrow_mut().insert(kicked, end(Stamp::utc_now().plus_secs(-25.0)));
    app.state.ended_here.borrow_mut().insert(nolog, end(Stamp::utc_now().plus_secs(-1200.0))); // too long ago: Roblox has caught up by then
    app.note_open_sessions();
    let ended_ids = app.vault(|d| d.ended_sessions.iter().map(|e| (e.account_id, e.job_id.clone())).collect::<Vec<_>>()).unwrap_or_default();
    c.check(
        "a client that ended here lately is kept in the vault, an old one isn't",
        ended_ids.len() == 1 && ended_ids[0].0 == kicked && ended_ids[0].1.as_deref() == Some(SERVER),
        ended_ids.len(),
    );
    let _ = call(&app, "lock", serde_json::json!({})).await;
    c.check("…locked: forgotten in memory", app.state.ended_here.borrow().is_empty(), app.state.ended_here.borrow().len());
    unlock(&app).await;
    let p = Presence {
        r#type: 2,
        location: Some("Steal An Egg".into()),
        place_id: Some(PLACE),
        root_place_id: Some(PLACE),
        job_id: Some(SERVER.into()),
        last_online: None,
        universe_id: Some(1),
    };
    let stale = |id: Guid| app.is_stale_in_game(&app.find_account(id).expect("account"), &p);
    c.check(
        "…and after unlocking (as after an update) its server's \"in game\" is still the late report",
        app.state.ended_here.borrow().contains_key(&kicked) && stale(kicked) && !stale(nolog),
        (app.state.ended_here.borrow().len(), stale(kicked), stale(nolog)),
    );

    // A rejoin waiting its turn (auto-rejoin's wait after failed tries): it holds no update back, and an update's restart
    // (or a lock) doesn't lose it.
    let waiting = |in_secs: f64| RejoinRequest::new(fresh, GameTarget::new(PLACE, None, None, None), Stamp::utc_now().plus_secs(in_secs));
    app.controller.rejoin_queue.borrow_mut().clear();
    app.controller.rejoin_queue.borrow_mut().push(waiting(300.0));
    c.check("a rejoin waiting 5 minutes isn't \"opening Roblox\": updating goes ahead", !app.opening_roblox(), ());
    app.controller.rejoin_queue.borrow_mut()[0] = waiting(10.0);
    c.check("…one due in 10 seconds is", app.opening_roblox(), ());
    app.controller.rejoin_queue.borrow_mut()[0] = waiting(300.0);
    // With another window open, as at an update: picking the open windows up again saves the vault, which must not wipe
    // the lists before they are read.
    open(&app, nolog, 999_993, Stamp::utc_now());
    app.note_open_sessions();
    let _ = call(&app, "lock", serde_json::json!({})).await;
    unlock(&app).await;
    let back: Vec<RejoinRequest> = app.controller.rejoin_queue.borrow().clone();
    c.check(
        "…and kept through a lock and unlock (as through an update's restart, with a window open), at its time",
        back.len() == 1 && back[0].account_id == fresh && back[0].due > Stamp::utc_now().plus_secs(240.0),
        format!("{} {:?}", back.len(), back.first().map(|b| b.due)),
    );
    c.check("…the client that ended lately too", app.state.ended_here.borrow().contains_key(&kicked), app.state.ended_here.borrow().len());
    c.check(
        "…and the vault is saved again soon (picking the open windows up saved it without the rejoin)",
        app.persist.save_due.get().is_some(),
        app.persist.save_due.get(),
    );
    app.controller.running.borrow_mut().clear();
    app.controller.rejoin_queue.borrow_mut().clear();
    // As the vault holds them when it is opened: one due long ago isn't done by itself; one due a minute ago is, shortly.
    let vault = app.controller.vault.borrow().clone().expect("unlocked");
    let restore = |due: Stamp| {
        let w = vec![WaitingRejoin { account_id: fresh, target: Some(GameTarget::new(PLACE, None, None, None)), due, ..Default::default() }];
        vault.data().waiting_rejoins = w.clone();
        app.restore_waiting_rejoins(&vault, &w);
    };
    restore(Stamp::utc_now().plus_secs(-9.0 * 3600.0));
    c.check(
        "…but one from hours ago isn't done after an unlock (no Roblox opening by itself the next day)",
        app.controller.rejoin_queue.borrow().is_empty(),
        app.controller.rejoin_queue.borrow().len(),
    );
    restore(Stamp::utc_now().plus_secs(-60.0));
    c.check("…one due a minute ago is, shortly", app.controller.rejoin_queue.borrow().len() == 1, app.controller.rejoin_queue.borrow().len());
    app.controller.rejoin_queue.borrow_mut().clear();
    vault.data().waiting_rejoins.clear();
    drop(kick_log);
}
