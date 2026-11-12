// Tests/Bloxkeep.Tests/TickTimeSuite.cs: the process check timed (App::note_tick_time): one that held the core's thread
// 100 ms or more goes to error.log with its slowest parts, named by the code; another in the same hour is only counted,
// unless it took twice as long as the slowest written (then it is written too, with the count). A quick one writes
// nothing. Then what is installed (Roblox, its launchers): read again on a worker a little before the cached read runs
// out, taken in at a later tick.
use std::path::Path;
use std::time::Duration;

use super::Checks;
use super::rejoin::quiet_app;
use crate::core::base::Stamp;

fn log_text(dir: &str) -> String {
    std::fs::read_to_string(Path::new(dir).join("error.log")).unwrap_or_default()
}

pub async fn run(c: &mut Checks, dir: &str) {
    let (app, _said) = quiet_app();
    let note = |ms: u64, parts: &[(&str, u64)]| {
        app.note_tick_time(Duration::from_millis(ms), parts.iter().map(|p| (p.0.to_string(), Duration::from_millis(p.1))).collect())
    };

    note(60, &[("process list", 4), ("() => ApplyWindowIcons(pids, windows)", 50)]);
    c.check("a quick check writes nothing", log_text(dir).is_empty(), log_text(dir));

    note(
        250,
        &[("process list", 4), ("() => ApplyWindowIcons(pids, windows)", 200), ("Roblox logs", 30), ("window list", 2), ("() => TickDiscord(now, players)", 1)],
    );
    let first = log_text(dir);
    c.check(
        "a slow check is written with its time and its slowest parts, named by the code",
        first.contains("Slow process check: 250 ms")
            && first.contains("ApplyWindowIcons(pids, windows) 200 ms, Roblox logs 30 ms")
            && !first.contains("() =>")
            && !first.contains("TickDiscord")
            && first.contains("the rest 13 ms"),
        &first,
    );

    note(400, &[("Roblox logs", 390)]);
    c.check("another in the same hour, under twice as long, is only counted", log_text(dir) == first, log_text(dir));

    note(500, &[("() => PersistIfDue(now)", 480)]);
    let all = log_text(dir);
    let second = all.get(first.len()..).unwrap_or("").to_string();
    c.check(
        "one twice as long is written too, with how many came unwritten",
        second.contains("Slow process check: 500 ms") && second.contains("PersistIfDue(now) 480 ms") && second.contains("1 slow check(s) came"),
        &second,
    );

    // What is installed (Roblox, its launchers): read again on a worker a little before the cached read runs out, taken
    // in at a later tick, never read on the core's thread by the tick while a read is cached (2026-10-29: 0.9 s of one
    // check on the owner's PC).
    // Waits for the worker's read (Task.Wait) without losing it: the read is put back in a fresh channel.
    let wait_for_read = || {
        let rx = app.roblox_updates.installs_reading.borrow_mut().take();
        if let Some(rx) = rx {
            let (tx, again) = std::sync::mpsc::channel();
            if let Ok(read) = rx.recv_timeout(Duration::from_secs(30)) {
                let _ = tx.send(read);
            }
            *app.roblox_updates.installs_reading.borrow_mut() = Some(again);
        }
    };
    app.installs();
    let read_at = Stamp::utc_now().plus_secs(-3.0 * 60.0 + 5.0); // 5 s before the cached read runs out
    app.controller.installs_read_at.set(read_at);
    app.tick_roblox_updates(Stamp::utc_now(), 0);
    c.check(
        "the tick starts reading what is installed on a worker, the cached read still in use",
        app.roblox_updates.installs_reading.borrow().is_some() && app.controller.installs_read_at.get() == read_at,
        (),
    );
    wait_for_read();
    app.tick_roblox_updates(Stamp::utc_now(), 0);
    c.check(
        "…its read taken in at a later tick",
        app.roblox_updates.installs_reading.borrow().is_none() && app.controller.installs_read_at.get() > read_at,
        app.controller.installs_read_at.get(),
    );
    // A read started before the cache was dropped (another vault, another launch option) is left unused.
    app.controller.installs_read_at.set(read_at);
    app.tick_roblox_updates(Stamp::utc_now(), 0);
    wait_for_read();
    let fresh = Stamp::utc_now();
    app.controller.installs_read_at.set(fresh); // read again meanwhile
    app.tick_roblox_updates(Stamp::utc_now(), 0);
    c.check("…one started before another read is left unused", app.controller.installs_read_at.get() == fresh, app.controller.installs_read_at.get());
}
