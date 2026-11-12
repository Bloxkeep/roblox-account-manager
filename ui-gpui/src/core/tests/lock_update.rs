// Tests/Bloxkeep.Tests/LockUpdateSuite.cs: an update found before the vault is opened installs at the lock screen
// (app/relaunch.rs, 2026-10-25): the screen is offered it while it downloads, gives it its place when nothing was typed,
// and it then goes with nobody's use holding it back. Found after the unlock, it waits for a quiet moment rather than
// cutting in. Nothing is installed here: the decisions are asked, not acted on.
//
// PORT: the updater's status and downloaded asset are set through its state (the C# set its backing fields).
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use serde_json::{Value, json};

use super::rejoin::quiet_app;
use super::{Checks, PASSWORD, call, unlock};
use crate::core::app_updater::guard;
use crate::core::base::Stamp;
use crate::core::models::{LaunchPreset, PresetSchedule};
use crate::core::vault::Vault;
use crate::core::{prefs, program, relaunch, rt};

pub async fn run(c: &mut Checks, _dir: &str) {
    Vault::create(PASSWORD, None).expect("a vault").save().expect("saved");
    let (app, _said) = quiet_app();
    let window = Rc::new(RefCell::new("shown".to_string()));
    {
        let window = window.clone();
        *app.relaunch.window_now.borrow_mut() = Some(Rc::new(move || (window.borrow().clone(), true, false)));
    }
    let updater = app.updates.updater.borrow().clone();
    let status = |s: &str| guard(&updater.state).status = s.to_string();
    let offered = || app.update_offers_at_lock();
    let goes = || app.update_goes_at_lock();
    let quiet = |at_lock: bool| app.quiet_for_relaunch(Stamp::utc_now(), at_lock);
    let lock_update = || app.build_state().get("lockUpdate").cloned().unwrap_or(Value::Null);

    c.check("no update found: the lock screen isn't offered one", !offered() && lock_update().is_null(), lock_update());
    status("downloading");
    c.check(
        "an update downloading before the unlock: offered to the lock screen, not answered yet",
        offered() && lock_update().get("go").is_some_and(Value::is_null),
        lock_update(),
    );
    c.check("…and nothing goes while the screen, on screen, hasn't given it its place", !goes(), ());
    let _ = call(&app, "updateAtLock", json!({ "go": true })).await;
    c.check(
        "the lock screen gave its place: the update goes once downloaded, nobody's use holding it back",
        goes() && quiet(true) && lock_update().get("go").and_then(Value::as_bool) == Some(true),
        lock_update(),
    );
    let _ = call(&app, "updateAtLock", json!({ "go": false })).await;
    c.check(
        "its link took the form back: nothing goes while the window is on screen, and the answer stays (a redraw keeps it)",
        !goes() && lock_update().get("go").and_then(Value::as_bool) == Some(false),
        lock_update(),
    );
    *window.borrow_mut() = "hidden".into();
    c.check("in the tray (opened at Windows' sign-in), still locked: it goes without asking", goes(), ());
    *window.borrow_mut() = "shown".into();
    status("error");
    app.on_updater_changed();
    c.check("a download that failed: no offer", !offered(), ());

    status("ready");
    let _ = call(&app, "updateAtLock", json!({ "go": true })).await;
    unlock(&app).await;
    c.check(
        "unlocked: the offer and its answer go (fails on the old code: installed at once in the first 5 minutes)",
        !offered() && !goes() && lock_update().is_null(),
        lock_update(),
    );
    c.check(
        "…and the update waits for a quiet moment: not in the first minutes, nor with the window on screen and the PC in use",
        !quiet(false),
        (),
    );

    // 2026-10-30 review: a quiet restart a few seconds before a scheduled preset's minute carried it past that minute,
    // and missed minutes aren't caught up.
    app.relaunch.started.set(Stamp::utc_now().plus_secs(-600.0));
    *window.borrow_mut() = "hidden".into();
    c.check("in the tray a while after the start: the update goes", quiet(false), ());
    let vault = app.vault_ref().expect("open");
    let preset = LaunchPreset { name: "Night".into(), ..Default::default() };
    let preset_id = preset.id;
    vault.data().settings.presets.push(preset);
    let at = |from_secs: f64| {
        let t = Stamp::now().plus_secs(from_secs).local();
        format!("{:02}:{:02}", t.hour, t.minute)
    };
    let set_schedule = |s: PresetSchedule| {
        if let Some(p) = vault.data().settings.presets.iter_mut().find(|p| p.id == preset_id) {
            p.schedule = Some(s);
        }
    };
    let every_day = |time: String| PresetSchedule { enabled: true, time, days: vec![0, 1, 2, 3, 4, 5, 6], ..Default::default() };
    set_schedule(every_day(at(120.0)));
    c.check("…but not with a scheduled preset due in 2 minutes (fails on the old code)", !quiet(false), ());
    let now_minute = at(0.0);
    set_schedule(every_day(now_minute.clone()));
    c.check("…nor in its minute before it ran", !quiet(false), ());
    let today = Stamp::now().local();
    set_schedule(PresetSchedule {
        last_run: format!("{:04}-{:02}-{:02} {now_minute}", today.year, today.month, today.day),
        ..every_day(now_minute.clone())
    });
    c.check("…once it ran, the update goes", quiet(false), ());
    set_schedule(every_day(at(1200.0)));
    c.check("…and with the next one 20 minutes away", quiet(false), ());
    vault.data().settings.presets.retain(|p| p.id != preset_id);

    // 2026-10-30 review: Get beta versions turned off kept a downloaded beta, which the quiet restart or the quit installed.
    let staged = |version: &str| {
        let mut s = guard(&updater.state);
        s.ready = Some(velopack::VelopackAsset {
            Version: version.into(),
            FileName: format!("BloxkeepApp-{version}-full.nupkg"),
            ..Default::default()
        });
        s.status = "ready".into();
    };
    prefs::lock().beta_updates = true;
    staged("9.9.9-beta.1");
    let _ = call(&app, "setBetaUpdates", json!({ "enabled": false })).await;
    c.check(
        "Get beta versions turned off: the downloaded beta isn't installed (fails on the old code)",
        guard(&updater.state).ready.is_none() && updater.status() != "ready",
        updater.status(),
    );
    prefs::lock().beta_updates = true;
    staged("9.9.9-alpha.1");
    let _ = call(&app, "setBetaUpdates", json!({ "enabled": false })).await;
    c.check("…a downloaded quiet fix stays", guard(&updater.state).ready.is_some() && updater.status() == "ready", updater.status());
    guard(&updater.state).ready = None;
    status("idle");

    // 2026-10-30 review: Bloxkeep opened by hand a second after "Restart to update" stopped waiting as soon as some
    // Bloxkeep held the single-instance mutex, the run on its way out. It waits until the update's run took the hand-over.
    let _ = relaunch::write(&relaunch::State { written_at: Stamp::utc_now(), ..Default::default() });
    let waiting = std::thread::spawn(|| program::wait_for_update_run(Duration::from_secs(10)));
    rt::sleep(Duration::from_millis(800)).await;
    let still_waiting = !waiting.is_finished();
    let _ = relaunch::take(&[]); // the update's run takes it
    let mut over = false;
    for _ in 0..30 {
        if waiting.is_finished() {
            over = true;
            break;
        }
        rt::sleep(Duration::from_millis(100)).await;
    }
    c.check(
        "a start by hand during an update waits until the update's run took the hand-over, then hands over to it",
        still_waiting && over && !relaunch::hand_over_waiting(),
        format!("{still_waiting} {over}"),
    );
}
