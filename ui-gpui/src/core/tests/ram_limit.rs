// Tests/Bloxkeep.Tests/RamLimitSuite.cs: Settings › Staying in game › RAM limit for resting windows (limit_resting_ram,
// roblox_processes::limit_ram), on the test's own process. The C# also used a window of its own (shown, minimized, hidden)
// and a PowerShell it started to hold 800 MB: what needs either is skipped here. What runs: the hard limit set and lifted,
// only Settings' choices taken, the low memory priority set right at the first look, and the limit and priority given back
// at locking and at closing (the state a minimized window would have had set by hand: limit and priority on the process).
use serde_json::json;

use super::{call, cores::memory_priority, unlock, user_error, window::quiet_app as app, Checks, PASSWORD};
use crate::core::{
    app::memory::settling,
    base::Stamp,
    roblox_log_watcher::{apply, ClientState, Tail},
    roblox_processes::{self, PlayerProcess},
    system_memory::MemoryStatus,
    vault::Vault,
};

pub async fn run(c: &mut Checks, _dir: &str) {
    let me = std::process::id() as i32;
    c.check(
        "a hard RAM limit is set and lifted",
        roblox_processes::limit_ram(me, Some(512)) && roblox_processes::has_ram_limit(me) && roblox_processes::limit_ram(me, None) && !roblox_processes::has_ram_limit(me),
        (),
    );
    // A user's 4 minimized windows held at 512 MB froze in a teleport and all dropped (2026-11-24): no limit while moving.
    let now = Stamp::utc_now();
    let ago = |s: f64| now.plus_secs(-s);
    let state = |status: &str, at: f64, teleport: Option<f64>| ClientState { teleport_started: teleport.map(ago), ..ClientState::new(status, Some(1), None, None, ago(at)) };
    for (what, s, want) in [
        ("in a game for 10 min: limited", Some(state("ingame", 600., None)), false),
        ("a teleport begun: not limited", Some(state("ingame", 600., Some(5.))), true),
        ("joining the next server: not limited", Some(state("joining", 3., Some(8.))), true),
        ("on the new server 20 s: not yet limited", Some(state("ingame", 20., None)), true),
        ("on the new server 2 min: limited again", Some(state("ingame", 120., None)), false),
        ("a join stuck 6 min: limited again", Some(state("joining", 360., None)), false),
        ("dropped: limited", Some(state("disconnected", 5., None)), false),
        ("no log read: limited", None, false),
    ] {
        c.check(what, settling(s.as_ref(), now) == want, s.map(|s| (s.status, now.secs_since(s.at))));
    }
    // The same read from Roblox's own lines (a real teleport's, the owner's 0.748 log of 2026-11-24, times moved), the
    // limit back on every way out: in the new server, a teleport that never went on, a join that hangs.
    let t0 = Stamp::utc_now().plus_secs(-3600.0);
    let read = |lines: &[(f64, &str)], at: f64| {
        let mut tail = Tail::new("stand-in.log".into(), 1);
        for (when, text) in lines {
            apply(&mut tail, &format!("{},1.0,a2e8,6 {text}", t0.plus_secs(*when).to_iso()));
        }
        settling(tail.state.as_ref(), t0.plus_secs(at))
    };
    const JOIN_1: &str = "[FLog::Output] ! Joining game 'ed12e24f-74b9-4627-8842-b91942d249b8' place 107778070777162 at 10.32.36.144";
    const SERVER_1: &str = "[FLog::Network] serverId: 128.116.46.33|61851";
    const TELEPORT: &str = "Info [FLog::UgcExperienceController] UgcExperienceController: doTeleport: url ";
    const LEFT_OLD: &str = "[FLog::Network] Time to disconnect replication data: 0.405400";
    const MOVED: &str = "[FLog::SessionTransitionFSM] Teleported.";
    const JOIN_2: &str = "[FLog::Output] ! Joining game 'f8c11172-c670-4252-ac92-c2b1cc490248' place 107778070777162 at 10.204.0.5";
    const SERVER_2: &str = "[FLog::Network] serverId: 128.116.127.33|55447";
    let in_game = [(0.0, JOIN_1), (0.4, SERVER_1)];
    let mut teleport = in_game.to_vec();
    teleport.extend([(600.0, TELEPORT), (600.3, LEFT_OLD), (602.4, MOVED), (602.4, JOIN_2), (603.1, SERVER_2)]);
    for (what, lines, at, want) in [
        ("log: in a game 5 min: limited", &in_game[..], 300.0, false),
        ("log: just joined: not limited", &in_game[..], 30.0, true),
        ("log: a teleport begun: not limited", &teleport[..2 + 1], 600.1, true),
        ("log: joining the next server: not limited", &teleport[..2 + 4], 602.5, true),
        ("log: a minute on the new server: not limited", &teleport[..], 663.0, true),
        ("log: 91 s on the new server: limited again", &teleport[..], 694.2, false),
        ("log: a teleport that never went on, 4 min: not limited", &teleport[..2 + 1], 840.0, true),
        ("log: …6 min: limited again", &teleport[..2 + 1], 960.0, false),
        ("log: a join that hangs 6 min: limited again", &teleport[..2 + 4], 962.5, false),
    ] {
        c.check(what, read(lines, at) == want, at);
    }

    for what in [
        "the stand-in holds its 800 MB",
        "another process limited to 256 MB: its RAM stays there while it keeps working",
        "…its memory is still all its own, the rest paged out",
        "…lifted: it takes its RAM back",
    ] {
        c.skip(what, "starts a program");
    }

    Vault::create(PASSWORD, None).expect("vault");
    let (app, _said) = app();
    unlock(&app).await;
    let limit_mb = || app.vault(|d| d.settings.ram_limit_mb).unwrap_or(i32::MIN);
    let limited = || roblox_processes::has_ram_limit(me);

    c.skip("off by default: a minimized window isn't limited", "needs a window");

    let refused = call(&app, "setSettings", json!({ "ramLimitMb": 777 })).await;
    c.check(
        "only the choices Settings offers are taken",
        matches!(refused, Ok(serde_json::Value::Null)) && user_error(&refused).is_none() && limit_mb() == 0,
        (limit_mb(), refused.err().map(|e| e.to_string())),
    );
    for mb in [512, 768] {
        let _ = call(&app, "setSettings", json!({ "ramLimitMb": mb })).await;
        c.check(&format!("{mb} MB can be chosen"), limit_mb() == mb, limit_mb());
    }
    let _ = call(&app, "setSettings", json!({ "ramLimitMb": 0 })).await;

    for what in [
        "1 GB chosen: the minimized window is limited",
        "…left limited while anti-AFK shows it for a moment",
        "in view: lifted",
        "hidden by Bloxkeep: limited",
        "…shown again: lifted",
        "the process check doesn't wait for a limit being set",
        "…and a lift asked meanwhile comes after it, so the window isn't left limited",
        "Ctrl+Alt+M with a window open: minimized",
        "…and so the RAM limit applies to it",
        "Ctrl+Alt+M with all minimized: opened again",
        "Ctrl+Alt+N: the next window opened from minimized",
        "…a window hidden with Ctrl+Alt+H is left hidden by Ctrl+Alt+M",
        "Auto: a window minimized a moment ago isn't limited yet",
        "…nor with the PC's RAM mostly free (40 % used)",
        "…rested a minute with RAM 80 % used: limited to 3/4 of what it holds (2200 -> 1600 MB)",
        "…calm a minute: a quarter lower (1152 MB)",
        "…faulting with Windows taking its RAM (400 of 1152 MB held): not raised, the limit isn't why",
        "…faulting a lot: half again (1728 MB)",
        "…still faulting after two raises: left alone, the limit lifted",
        "…and not limited again for a while",
        "shown and minimized again: Auto starts over, waiting its minute",
    ] {
        c.skip(what, "needs a window");
    }
    let _ = call(&app, "setSettings", json!({ "ramLimitMb": -1 })).await;
    c.check("Auto is one of the choices Settings offers", limit_mb() == -1, limit_mb());
    c.skip("turned off: lifted, though still minimized", "needs a window");
    let _ = call(&app, "setSettings", json!({ "ramLimitMb": 0 })).await;

    for what in [
        "Save RAM off: a resting window keeps its RAM",
        "Save RAM on: not emptied when its rest begins",
        "…nor half a minute on",
        "…emptied a minute on",
        "…only once",
        "…left alone while anti-AFK shows it for a moment",
        "…and that doesn't start a new rest",
        "shown and minimized again: a new rest, waiting its minute",
        "…then emptied once more",
    ] {
        c.skip(what, "needs a window");
    }

    // Save RAM's low memory priority (guard_resources), on the same process: a window left low by a Bloxkeep that crashed
    // gets its priority set at the first look, and one lowered is raised again when Bloxkeep closes.
    roblox_processes::set_memory_priority(me, true);
    c.check("(the test can read its own memory priority)", memory_priority() == 2, memory_priority());
    app.guard_resources(
        &[PlayerProcess { pid: me, ..Default::default() }],
        &MemoryStatus { total_mb: 16000, available_mb: 8000, load_percent: 50, ..Default::default() },
        Stamp::utc_now(),
    );
    c.check(
        "a window left at low priority by an earlier Bloxkeep: normal again at the first look (Save RAM off)",
        memory_priority() == 5,
        memory_priority(),
    );
    roblox_processes::set_memory_priority(me, true);
    app.memory.low_memory_priority.borrow_mut().insert(me);

    // Locked, nothing looks after the windows: a window opened from the taskbar mustn't stay limited or low. A minimized
    // window's limit is set here as limit_resting_ram would have set it.
    let _ = call(&app, "setSettings", json!({ "ramLimitMb": 1024 })).await;
    roblox_processes::limit_ram(me, Some(1024));
    app.memory.ram_limited.borrow_mut().insert(me, 1024);
    let limited_before_lock = limited();
    let _ = call(&app, "lock", json!({})).await;
    c.check(
        "locking the vault lifts the limit and gives the priority back",
        limited_before_lock && !limited() && memory_priority() == 5,
        (limited_before_lock, limited(), memory_priority()),
    );
    let _ = call(&app, "unlock", json!({ "password": PASSWORD })).await;
    c.skip("…unlocked: limited again", "needs a window");
    roblox_processes::set_memory_priority(me, true);
    app.memory.low_memory_priority.borrow_mut().insert(me);

    let _ = call(&app, "setSettings", json!({ "ramLimitMb": 2048 })).await;
    roblox_processes::limit_ram(me, Some(2048));
    app.memory.ram_limited.borrow_mut().insert(me, 2048);
    let before = limited();
    // PORT: the C# disposed the whole app here. App::dispose also tells the bot this PC went offline and puts back whatever
    // Roblox windows the PC has open, so only its two steps for this run: lift_ram_limits and give_memory_priority_back.
    app.lift_ram_limits();
    app.give_memory_priority_back();
    c.check("Bloxkeep closing lifts it (no window is left limited without it)", before && !limited(), before);
    c.check("…and gives a window it lowered its normal memory priority back", memory_priority() == 5, memory_priority());
    roblox_processes::limit_ram(me, None);
}
