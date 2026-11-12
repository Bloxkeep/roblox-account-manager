// Tests/Bloxkeep.Tests/TickSuite.cs: the process check (App::track_processes) as a whole: one of its jobs that throws
// is logged and the rest still run, above all the save and auto-rejoin (audit R1). The check runs as in the tray with
// nothing of Roblox's found, so it reads no process list and touches no window; the Roblox logs are a folder of the
// test's own.
//
// PORT: the C# made a job throw by setting `_maximised` (Settings' Maximise) to null, a NullReferenceException. Here the
// same job (maximise_new_windows) is made to fail by holding its map borrowed, so its borrow panics; the error logged is
// that panic. The steps are named by the code without C#'s "() => " (App::step).
use std::path::Path;

use super::rejoin::{no_network, quiet_app, seed_with};
use super::{Checks, unlock};
use crate::core::app::App;
use crate::core::base::Stamp;
use crate::core::prefs;

/// One check in the tray: whether it panicked out, and the jobs that ran.
fn tick(app: &std::rc::Rc<App>) -> (Option<String>, Vec<String>) {
    // In the tray, nothing found at the last read moments ago: no process list is read (scan_due).
    app.controller.background.set(true);
    app.processes.last_scan_found_none.set(true);
    app.processes.last_scan.set(Stamp::utc_now());
    app.persist.save_due.set(Some(Stamp::utc_now().plus_secs(-1.0)));
    *app.processes.tick_parts.borrow_mut() = Some(Vec::new());
    let thrown = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| app.track_processes())).err().map(|p| {
        p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_else(|| "panic".into())
    });
    let parts = app.processes.tick_parts.borrow_mut().take().unwrap_or_default();
    (thrown, parts.into_iter().map(|p| p.0).collect())
}

pub async fn run(c: &mut Checks, dir: &str) {
    no_network();
    let logs = Path::new(dir).join("roblox-logs");
    std::fs::create_dir_all(&logs).expect("logs folder");
    // SAFETY: this suite's own process, before any other thread reads the environment.
    unsafe { std::env::set_var("BLOXKEEP_ROBLOX_LOGS", &logs) };
    {
        let mut p = prefs::lock();
        p.count_online = false;
        p.check_updates = false;
    }
    seed_with(Vec::new(), |_| {});
    let (app, _said) = quiet_app();
    unlock(&app).await;

    // Bad data in one job of the first half (here: what Settings' Maximise keeps, unusable).
    let (thrown, ran) = {
        let _held = app.presets.maximised.borrow_mut();
        tick(&app)
    };
    c.check("a job that throws doesn't end the process check", thrown.is_none(), &thrown);
    c.check(
        "…the save still runs in the same check",
        ran.iter().any(|r| r == "PersistIfDue(now)") && app.persist.save_due.get().is_none(),
        ran.join(" | "),
    );
    c.check("…and so does auto-rejoin", ran.iter().any(|r| r == "RunDueRejoin(now)"), ran.join(" | "));
    c.check(
        "…with the jobs after the one that threw",
        ran.iter().any(|r| r == "TickMacros(now)") && ran.iter().any(|r| r == "TickPanels()"),
        ran.join(" | "),
    );
    let log = std::fs::read_to_string(Path::new(dir).join("error.log")).unwrap_or_default();
    c.check("…and the error is logged", log.contains("panic"), &log);

    let (thrown, ran) = tick(&app);
    c.check("the next check runs whole", thrown.is_none() && ran.iter().any(|r| r == "RunDueRejoin(now)"), &thrown);
}
