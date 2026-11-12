// Tests/Bloxkeep.Tests/ScheduleSuite.cs: the schedule engine on a walked clock (core/clock.rs). A minute runs once, a
// clock set back or forward doesn't run twice, a gap and a locked minute are each said once, a one-off that went by is
// turned off, and the DST fall-back hour doesn't run a preset twice while the spring-forward hour reports the minute it
// skipped. Each preset has no accounts, so a run that fires ends at once in "didn't run: no accounts left": nothing
// opens Roblox.
use std::{
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicI64, Ordering},
    },
    time::Duration,
};

use serde_json::Value;

use super::{
    Checks, Said,
    rejoin::{no_network, quiet_app, seed_with},
    unlock,
};
use crate::core::{
    app::App,
    base::Stamp,
    clock,
    models::{LaunchPreset, PresetSchedule},
    rt,
};

fn t(day: u32, h: u32, m: u32, s: u32) -> Stamp {
    Stamp::from_local(2026, 11, day, h, m, s)
}

fn daily(name: &str, time: &str) -> LaunchPreset {
    LaunchPreset {
        name: name.into(),
        schedule: Some(PresetSchedule { enabled: false, time: time.into(), days: (0..7).collect(), ..Default::default() }),
        ..Default::default()
    }
}

fn toasts(said: &Said) -> Vec<String> {
    said.borrow()
        .iter()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|v| v["name"] == "toast")
        .filter_map(|v| v["data"]["text"].as_str().map(str::to_string))
        .collect()
}

pub async fn run(c: &mut Checks, _dir: &str) {
    no_network();
    let now = Arc::new(Mutex::new(t(2, 7, 59, 30))); // a Monday
    let tick = Arc::new(AtomicI64::new(1_000_000));
    {
        let (n, u, k) = (now.clone(), now.clone(), tick.clone());
        clock::set_now(Some(Box::new(move || *n.lock().unwrap())));
        clock::set_utc_now(Some(Box::new(move || u.lock().unwrap().to_utc())));
        clock::set_tick(Some(Box::new(move || k.load(Ordering::SeqCst))));
    }
    seed_with(Vec::new(), |d| {
        d.settings.presets.extend([
            daily("Daily", "08:00"),
            LaunchPreset {
                name: "Once".into(),
                schedule: Some(PresetSchedule { enabled: true, time: "09:00".into(), date: "2026-11-01".into(), ..Default::default() }),
                ..Default::default()
            },
            daily("Gap", "10:00"),
            daily("Locked", "12:00"),
            daily("Fall", "01:30"),
            daily("Spring", "02:30"),
        ])
    });
    let (app, said) = quiet_app();
    unlock(&app).await;
    body(c, &app, &said, &now, &tick).await;
    clock::set_now(None);
    clock::set_utc_now(None);
    clock::set_tick(None);
}

async fn body(c: &mut Checks, app: &Rc<App>, said: &Said, now: &Arc<Mutex<Stamp>>, tick: &Arc<AtomicI64>) {
    let schedule = |name: &str| app.vault(|d| d.settings.presets.iter().find(|p| p.name == name).and_then(|p| p.schedule.clone())).ok().flatten().unwrap_or_default();
    let enable = |name: &str, on: bool| {
        let _ = app.vault_mut(|d| {
            if let Some(s) = d.settings.presets.iter_mut().find(|p| p.name == name).and_then(|p| p.schedule.as_mut()) {
                s.enabled = on;
            }
        });
    };
    let count = |text: &str| toasts(said).iter().filter(|t| t.contains(text)).count();
    let runs = |name: &str| count(&format!("Scheduled preset {name} didn't run"));
    let at = |local: Stamp, ms: i64| {
        let app = app.clone();
        let (now, tick) = (now.clone(), tick.clone());
        async move {
            *now.lock().unwrap() = local;
            tick.fetch_add(ms, Ordering::SeqCst);
            app.check_schedules();
            rt::sleep(Duration::from_millis(150)).await; // the run itself is async: it ends at once in "no accounts left"
        }
    };

    // A one-off whose date went by while Bloxkeep was closed: turned off, never run.
    enable("Daily", true);
    at(t(2, 7, 59, 30), 0).await;
    c.check("a one-off that went by while closed is turned off, not run", !schedule("Once").enabled && runs("Once") == 0, ());

    // The minute comes: one run, however many ticks land in it.
    at(t(2, 8, 0, 5), 35_000).await;
    at(t(2, 8, 0, 50), 45_000).await;
    c.check(
        "its minute runs the preset once, however many ticks land in it",
        runs("Daily") == 1 && schedule("Daily").last_run == "2026-11-02 08:00",
        format!("runs {}, last {}", runs("Daily"), schedule("Daily").last_run),
    );

    // The clock set back into the minute that ran: not again.
    at(t(2, 8, 1, 10), 20_000).await;
    at(t(2, 8, 0, 20), 10_000).await;
    c.check("the clock set back into the minute that ran: not run again", runs("Daily") == 1, format!("runs {}", runs("Daily")));

    // Next day's minute runs; then the clock set back a day (uptime says a minute went by): not again.
    at(t(3, 8, 0, 10), 86_400_000).await;
    c.check("the next day's minute runs", runs("Daily") == 2, format!("runs {}", runs("Daily")));
    at(t(2, 8, 0, 30), 60_000).await;
    c.check("the clock set back a day right after a run: not run again", runs("Daily") == 2, format!("runs {}", runs("Daily")));

    // The clock jumps forward past a run (PC asleep, or set forward): said once as missed, not run late.
    at(t(3, 9, 59, 0), 60_000).await;
    enable("Gap", true);
    at(t(3, 10, 30, 0), 60_000).await;
    at(t(3, 10, 31, 0), 60_000).await;
    c.check(
        "a minute skipped by a jump forward is said once as missed, not run",
        count("Gap didn't run at 10:00: the PC was asleep or busy.") == 1 && runs("Gap") == 0,
        format!("missed {}, runs {}", count("Gap didn't run at 10:00"), runs("Gap")),
    );
    enable("Gap", false);

    // Locked through its minute: said once that minute is over, not while it is still on.
    enable("Locked", true);
    at(t(3, 11, 59, 30), 60_000).await;
    app.remember_schedules_for_lock();
    let vault = app.controller.vault.borrow_mut().take();
    at(t(3, 12, 0, 10), 40_000).await;
    let during = count("Locked didn't run at 12:00: Bloxkeep was locked.");
    at(t(3, 12, 1, 5), 55_000).await;
    at(t(3, 12, 2, 5), 60_000).await;
    c.check(
        "locked through its minute: said once, after the minute (an unlock within it could still run it)",
        during == 0 && count("Locked didn't run at 12:00: Bloxkeep was locked.") == 1 && runs("Locked") == 0,
        format!("during {during}, after {}", count("Locked didn't run at 12:00")),
    );
    *app.controller.vault.borrow_mut() = vault;
    at(t(3, 12, 3, 5), 60_000).await;
    enable("Locked", false);

    // DST fall-back: 01:00–02:00 comes twice. The preset runs in the first 01:30 only.
    at(t(4, 1, 29, 50), 13 * 3_600_000).await;
    enable("Fall", true);
    at(t(4, 1, 30, 10), 20_000).await;
    at(t(4, 1, 59, 30), 29 * 60_000).await;
    at(t(4, 1, 0, 30), 60_000).await; // clocks go back an hour
    for m in 1..=31 {
        at(t(4, 1, m, 30), 60_000).await;
    }
    c.check(
        "DST fall-back: the repeated 01:30 doesn't run the preset twice",
        runs("Fall") == 1 && count("Fall didn't run at") == 0,
        format!("runs {}, missed {}", runs("Fall"), count("Fall didn't run at")),
    );
    enable("Fall", false);

    // DST spring-forward: 02:00–03:00 never comes. A preset set inside it is said as missed, once.
    at(t(5, 1, 59, 0), 24 * 3_600_000).await;
    enable("Spring", true);
    at(t(5, 3, 0, 5), 65_000).await;
    at(t(5, 3, 1, 5), 60_000).await;
    c.check(
        "DST spring-forward: the skipped 02:30 is said once as missed",
        count("Spring didn't run at 02:30: the PC was asleep or busy.") == 1 && runs("Spring") == 0,
        format!("missed {}, runs {}", count("Spring didn't run at 02:30"), runs("Spring")),
    );
}
