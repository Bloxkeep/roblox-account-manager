// A rejoin's window goes back where the account's last window stood: noted while the old one is open, put back once the
// new one is in its game and has settled, checked once more after, and left alone when an Arrange placed it meanwhile.
// The rule is window_plan.rs's and the window desk carries it out (window_desk.rs), through app/rejoin_place.rs's entry
// points, a tick at a time with a made-up clock, on real windows of the test's own standing in for Roblox's, on a
// desktop of their own that is never shown (hidden_desktop.rs).
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

use windows_sys::Win32::UI::WindowsAndMessaging::WS_OVERLAPPEDWINDOW;

use super::{
    Checks,
    hidden_desktop::{self as hd, Rec, make_windows, put, rect_of, settle, wait_for, zoomed},
    rejoin::{no_network, quiet_app, seed},
    unlock,
};
use crate::core::{
    app::{App, controller::PendingLaunch},
    base::{Guid, Stamp, new_guid},
    game_target::GameTarget,
    models::Account,
    roblox_log_watcher::{ClientState, Tail},
    win_kit::{self, Rect, Size},
    window_desk::{Arrangement, Lift},
    window_plan::{Last, in_tile},
};

pub async fn run(c: &mut Checks, dir: &str) {
    if hd::in_view() {
        hd::run_there(c, dir, "rejoin-place", 120_000);
        return;
    }
    let mut r = Rec::new(c);
    checks(&mut r).await;
    r.save(dir);
}

/// The client `pid` of `account`, opened just now (by a rejoin or not).
fn run_client(app: &App, account: Guid, pid: i32, rejoin: bool) {
    let launch = Rc::new(PendingLaunch::new(account, HashSet::new(), Stamp::utc_now(), Some(GameTarget::new(4242, None, None, None)), rejoin, None));
    app.controller.running.borrow_mut().insert(account, (pid, launch));
}

/// Its Roblox log says `status` ("joining", "ingame") since `at`.
fn log_says(app: &App, pid: i32, status: &str, at: Stamp) {
    let mut tail = Tail::new(format!("stand-in-{pid}.log"), pid);
    tail.state = Some(ClientState::new(status, Some(4242), (status == "ingame").then(|| "job".to_string()), None, at));
    app.controller.logs.borrow_mut().tails.insert(pid, tail);
}

/// One tick of the window desk with this one client's window, as on_proc_timer runs it; its asks come about a moment
/// later (and are taken in on the next tick).
fn tick(app: &App, now: Stamp, pid: i32, w: isize) {
    app.keep_rejoin_places(now, &HashMap::from([(pid, vec![w])]));
    settle();
}

async fn checks(r: &mut Rec<'_>) {
    no_network();
    let id = new_guid();
    seed(vec![Account { id, username: "Tiled".into(), user_id: 41, cookie: "cookie-tiled".into(), cookie_valid: Some(true), ..Default::default() }]);
    let (app, _said) = quiet_app();
    unlock(&app).await;
    let screens = win_kit::all_screens();
    let Some(screen) = screens.iter().find(|s| s.primary).or(screens.first()).cloned() else {
        r.skip("the account's window place is noted while it is open", "no screen");
        return;
    };
    let area = screen.working_area;

    // Two windows, as Roblox's: the old one in its tile and the rejoin's new one at Roblox's own size.
    let tile = Rect::new(area.x + 40, area.y + 40, 640, 516);
    let roblox_size = Rect::new(area.x + 300, area.y + 200, 816, 638);
    let ws = make_windows(2, WS_OVERLAPPEDWINDOW);
    let (old, new) = (ws.all[0], ws.all[1]);
    for w in [old, new] {
        hd::set_title(w, "Roblox");
    }
    put(old, tile);
    put(new, roblox_size);
    settle();
    let old_place = rect_of(old);
    let roblox_size = rect_of(new);
    let last = || app.rejoin_place.last.borrow().get(&id).copied();
    const OLD: i32 = 777_201;
    const NEW: i32 = 777_202;
    const THIRD: i32 = 777_203;
    let t0 = Stamp::utc_now();
    let s = |secs: f64| t0.plus_secs(secs);

    // The old window, in its tile and in its game: its place is noted.
    run_client(&app, id, OLD, false);
    log_says(&app, OLD, "ingame", s(-60.));
    tick(&app, s(0.), OLD, old);
    r.check("the account's window place is noted while it is open", last() == Some(Last { place: Some(old_place), minimised: false }), (last(), old_place));

    // It crashes; the rejoin's window comes up at Roblox's own size, still joining.
    app.controller.logs.borrow_mut().tails.remove(&OLD);
    run_client(&app, id, NEW, true);
    log_says(&app, NEW, "joining", s(0.));
    tick(&app, s(0.5), NEW, new);
    r.check("the rejoin's window waits to go back", app.going_back(NEW), app.desk_memory(NEW));
    tick(&app, s(20.), NEW, new);
    r.check("not moved while it joins", rect_of(new) == roblox_size, rect_of(new));
    r.check("the account's place isn't overwritten by the new window's Roblox size", last().and_then(|l| l.place) == Some(old_place), last());

    // In its game: a moment to settle, then back in the old tile.
    log_says(&app, NEW, "ingame", s(21.));
    tick(&app, s(21.), NEW, new);
    r.check("not moved the moment it gets in", rect_of(new) == roblox_size, rect_of(new));
    tick(&app, s(25.), NEW, new);
    let _ = wait_for(1500, || rect_of(new) == old_place);
    settle();
    r.check("put back where the old window stood", rect_of(new) == old_place, (rect_of(new), old_place));

    // Roblox sizes it again a moment after: put back once more at the look after.
    put(new, roblox_size);
    settle();
    tick(&app, s(32.), NEW, new);
    let _ = wait_for(1500, || rect_of(new) == old_place);
    settle();
    r.check("sized again after joining, it is put back once more", rect_of(new) == old_place, (rect_of(new), old_place));
    tick(&app, s(33.), NEW, new);
    r.check("done going back", !app.going_back(NEW), app.desk_memory(NEW));
    // Settings' Maximise on: the window stays where it went.
    let _ = app.vault_mut(|d| d.settings.maximize_on_open = true);
    tick(&app, s(34.), NEW, new);
    tick(&app, s(35.), NEW, new);
    r.check(
        "Settings' Maximise leaves it where it went",
        app.presets.maximised.borrow().get(&NEW).is_some_and(|s| s == "ingame") && !zoomed(new) && rect_of(new) == old_place,
        (app.presets.maximised.borrow().get(&NEW).cloned(), zoomed(new), rect_of(new)),
    );
    let _ = app.vault_mut(|d| d.settings.maximize_on_open = false);

    // Moved by hand later: that is the place noted for the next rejoin.
    let moved = Rect::new(tile.x + 100, tile.y + 60, 700, 540);
    put(new, moved);
    settle();
    tick(&app, s(40.), NEW, new);
    r.check("a window moved by hand: its new place is noted", last().and_then(|l| l.place) == Some(rect_of(new)) && rect_of(new) != old_place, (last(), rect_of(new)));

    // A rejoin that an Arrange placed before it went back stays in its new tile.
    app.controller.logs.borrow_mut().tails.remove(&NEW);
    put(old, roblox_size);
    settle();
    run_client(&app, id, THIRD, true);
    log_says(&app, THIRD, "joining", s(50.));
    tick(&app, s(50.), THIRD, old);
    r.check("the next rejoin waits to go back", app.going_back(THIRD), app.desk_memory(THIRD));
    let there = Rect::new(area.x + 20, area.y + 20, 900, 700);
    let a = Arrangement { windows: vec![(THIRD, old)], slots: vec![(THIRD, old)], area: there, layout: "fill".into(), custom: Size::new(0, 0), open: Vec::new(), clicked: false, lift: Lift::No };
    let laid = app.desk_arrange(move |_| a).await.ok();
    let given = laid.as_ref().and_then(|l| l.took.first().map(|t| t.2));
    log_says(&app, THIRD, "ingame", s(60.));
    tick(&app, s(60.), THIRD, old);
    tick(&app, s(65.), THIRD, old);
    tick(&app, s(66.), THIRD, old);
    let now = rect_of(old);
    r.check(
        "arranged meanwhile: not taken back to the old place",
        given.is_some_and(|t| in_tile(now, t)) && !in_tile(now, rect_of(new)) && !app.going_back(THIRD),
        (now, given, laid.map(|l| l.note), app.desk_memory(THIRD)),
    );

    app.controller.running.borrow_mut().clear();
    drop(ws);
}
