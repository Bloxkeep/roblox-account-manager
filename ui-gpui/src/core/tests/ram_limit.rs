// Tests/Bloxkeep.Tests/RamLimitSuite.cs: Settings › Staying in game › RAM limit for resting windows (limit_resting_ram,
// roblox_processes::limit_ram), on the test's own process. The C# also used a window of its own (shown, minimized, hidden)
// and a PowerShell it started to hold 800 MB: what needs either is skipped here. What runs: the hard limit set and lifted,
// only Settings' choices taken, the low memory priority set right at the first look, and the limit and priority given back
// at locking and at closing (the state a minimized window would have had set by hand: limit and priority on the process).
// The CPU limit for resting windows (cpu_plan, cpu_cap.rs) is checked the same way: its plan over every case, its job's
// hard cap on the test's own process, and its lift at locking and at closing.
use std::{collections::HashMap, time::Duration};

use serde_json::json;

use super::{call, cores::memory_priority, unlock, user_error, window::quiet_app as app, Checks, PASSWORD};
use crate::core::{
    app::memory::{
        auto_cap, cpu_plan, ram_plan, settling, AutoRam, CpuAsk, CpuCap, CpuMemory, Game, RamAsk, RamCap, RamFacts, RamMemory, RamSetting, CPU_LIMIT_CHOICES,
        MOVING_LIMIT_MB,
    },
    base::Stamp,
    cpu_cap::{self, JOB_OBJECT_CPU_RATE_CONTROL_ENABLE, JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP},
    roblox_log_watcher::{apply, ClientState, Tail},
    roblox_processes::{self, PlayerProcess},
    rt,
    system_memory::MemoryStatus,
    vault::Vault,
};

const MB: i64 = 1024 * 1024;

/// A resting window as the plan sees it: minimized, not in front, its game 5 min old, Settings at 1 GB, Save RAM on.
fn facts(now: Stamp) -> RamFacts {
    RamFacts {
        pid: 1000,
        now,
        setting: RamSetting::Mb(1024),
        open_too: false,
        save_ram: true,
        front: false,
        main: false,
        has_window: true,
        minimised: true,
        hidden: false,
        lent: false,
        game: Game::InGame { at: now.plus_secs(-300.0), teleport: None },
        working_set: 2200 * MB,
        page_faults: 0,
        hard_faults: 0,
        load_percent: 80,
        memory: RamMemory::default(),
        cpu_percent: 0,
        cpu: CpuMemory::default(),
    }
}

/// Auto's state for a window that rested long ago and was limited to `cap_mb` long ago, no fault counts yet.
fn auto_at(now: Stamp, cap_mb: i64) -> AutoRam {
    AutoRam {
        cap_mb,
        changed_at: now.plus_secs(-3600.0),
        resting_since: now.plus_secs(-3600.0),
        raises: 0,
        left_alone_until: None,
        faults: -1,
        hard_faults: -1,
        last_look: now.plus_secs(-1.5),
        rates: Vec::new(),
    }
}

/// The limit pass as it decided one window before the plan (limit_resting_ram of 1.1.6-alpha.9, minus the pass skipped
/// for every window while any was shown for a moment): what it asked Windows (`Some(None)` a lift).
fn ask_before(f: &RamFacts) -> Option<Option<i64>> {
    if f.lent {
        return None;
    }
    let setting: i64 = match f.setting {
        RamSetting::Off => 0,
        RamSetting::Auto => -1,
        RamSetting::Mb(mb) => mb,
    };
    let put_away = setting != 0 && !f.front && !f.main && f.has_window && (f.minimised || f.hidden || (f.open_too && f.game != Game::Unknown));
    let moving = put_away && f.game.settling(f.now);
    let resting = put_away && !moving;
    let auto = if setting == -1 { f.memory.auto.clone() } else { None };
    let want = if moving {
        Some(MOVING_LIMIT_MB.max(setting).max(auto.as_ref().map_or(0, |a| a.cap_mb)))
    } else if !resting {
        None
    } else if setting == -1 {
        auto_cap(&mut auto.clone(), f).0
    } else {
        Some(setting)
    };
    match (want, f.memory.limit()) {
        (Some(limit), had) if had != Some(limit) => Some(Some(limit)),
        (None, Some(_)) => Some(None),
        _ => None,
    }
}

/// Save RAM's emptying as it decided one window before the plan (empty_resting_ram_once): emptied now, the rest kept.
fn empty_before(f: &RamFacts) -> (bool, Option<Stamp>, bool) {
    let (mut since, emptied) = (f.memory.resting_since, f.memory.emptied);
    if f.lent {
        return (false, since, emptied);
    }
    if !f.save_ram || f.front || f.main || !f.has_window || !(f.minimised || f.hidden) || f.game.settling(f.now) {
        return (false, None, false);
    }
    let added = since.is_none();
    let at = *since.get_or_insert(f.now);
    if !added && f.now.secs_since(at) >= 60.0 && !emptied {
        return (true, since, true);
    }
    (false, since, emptied)
}

/// Every window, setting and memory against the decisions before the plan: the same, but for Auto's limit kept across a
/// move to a server (it was forgotten there).
fn plan_table(c: &mut Checks) {
    let now = Stamp::utc_now();
    let ago = |s: f64| now.plus_secs(-s);
    let places: [(&str, fn(&mut RamFacts)); 6] = [
        ("in front", |f| (f.front, f.minimised) = (true, false)),
        ("a main account's, minimized", |f| f.main = true),
        ("no window", |f| (f.has_window, f.minimised) = (false, false)),
        ("minimized", |_| {}),
        ("hidden by Bloxkeep", |f| (f.minimised, f.hidden) = (false, true)),
        ("in view", |f| f.minimised = false),
    ];
    let games = [
        Game::Unknown,
        Game::Joining { at: ago(3.0) },
        Game::Joining { at: ago(360.0) },
        Game::InGame { at: ago(5.0), teleport: None },
        Game::InGame { at: ago(13.0), teleport: None },
        Game::InGame { at: ago(300.0), teleport: None },
        Game::InGame { at: ago(300.0), teleport: Some(ago(30.0)) },
        Game::InGame { at: ago(300.0), teleport: Some(ago(360.0)) },
        Game::Left,
        Game::Disconnected,
    ];
    let limits: [(Option<Option<i64>>, Option<i64>); 5] = [(None, None), (None, Some(1024)), (None, Some(-1)), (Some(Some(2048)), None), (Some(None), Some(1024))];
    let rests = [(None, false), (Some(ago(30.0)), false), (Some(ago(61.0)), false), (Some(ago(61.0)), true)];
    for setting in [RamSetting::Off, RamSetting::Auto, RamSetting::Mb(1024)] {
        for (place, put) in places {
            let (mut rows, mut wrong) = (0, Vec::new());
            for open_too in [false, true] {
                for game in games {
                    for lent in [false, true] {
                        for (asked, held) in limits {
                            for auto in [None, Some(auto_at(now, 1152))] {
                                for (resting_since, emptied) in rests {
                                    for save_ram in [false, true] {
                                        let mut f = facts(now);
                                        put(&mut f);
                                        (f.setting, f.open_too, f.game, f.lent, f.save_ram) = (setting, open_too, game, lent, save_ram);
                                        f.memory = RamMemory { asked, held, refused: false, auto: auto.clone(), resting_since, emptied };
                                        rows += 1;
                                        let w = ram_plan(&f);
                                        let (empty, since, was_emptied) = empty_before(&f);
                                        let moving = f.game.settling(now) && !matches!(w.cap, RamCap::Lift | RamCap::Leave);
                                        let auto_kept = if lent {
                                            f.memory.auto.is_some()
                                        } else if setting != RamSetting::Auto {
                                            false
                                        } else if moving {
                                            f.memory.auto.is_some()
                                        } else {
                                            matches!(w.cap, RamCap::Mb(_)) || w.why == "auto left it alone"
                                        };
                                        let asked_after = w.ask.or(asked);
                                        let same = w.ask == ask_before(&f)
                                            && (w.cap == RamCap::Leave) == lent
                                            && w.empty_once == empty
                                            && w.memory.resting_since == since
                                            && w.memory.emptied == was_emptied
                                            && w.memory.auto.is_some() == auto_kept
                                            && w.memory.asked == asked_after
                                            && w.memory.held == held;
                                        if !same && wrong.len() < 3 {
                                            wrong.push(format!(
                                                "open_too {open_too}, {game:?}, lent {lent}, asked {asked:?} held {held:?}, auto {}, rest {resting_since:?}/{emptied}, save {save_ram}: plan {:?}/{:?} empty {}, before {:?} empty {empty}",
                                                auto.is_some(),
                                                w.cap,
                                                w.ask,
                                                w.empty_once,
                                                ask_before(&f)
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            c.check(&format!("plan: {setting:?}, {place}: as before in every case ({rows} cases)"), wrong.is_empty(), wrong);
        }
    }
}

/// Named cases of the plan, and the faults it mends.
fn plan_cases(c: &mut Checks) {
    let now = Stamp::utc_now();
    let ago = |s: f64| now.plus_secs(-s);
    let plan = |change: &dyn Fn(&mut RamFacts)| {
        let mut f = facts(now);
        change(&mut f);
        ram_plan(&f)
    };
    let w = plan(&|_| {});
    c.check("plan: minimized, 1 GB chosen, in its game 5 min: limited to 1024 MB (chosen)", w.ask == Some(Some(1024)) && w.why == "chosen", (w.ask, w.why));
    let w = plan(&|f| f.memory.held = Some(1024));
    c.check("plan: …already held there: nothing asked", w.ask.is_none() && w.cap == RamCap::Mb(1024), (w.ask, w.cap));
    let w = plan(&|f| f.game = Game::InGame { at: ago(5.0), teleport: None });
    c.check("plan: just joined (5 s): 2048 MB (moving to a server)", w.ask == Some(Some(2048)) && w.why == "moving to a server", (w.ask, w.why));
    let w = plan(&|f| (f.memory.held, f.minimised) = (Some(1024), false));
    c.check("plan: shown again: lifted", w.ask == Some(None) && w.why == "shown, in front, or the option off", (w.ask, w.why));
    let w = plan(&|f| (f.memory.held, f.front) = (Some(1024), true));
    c.check("plan: in front: lifted", w.ask == Some(None), w.ask);
    let w = plan(&|f| (f.memory.held, f.main) = (Some(1024), true));
    c.check("plan: a main account's: lifted", w.ask == Some(None), w.ask);
    let w = plan(&|f| (f.memory.held, f.setting) = (Some(-1), RamSetting::Off));
    c.check("plan: off: a limit found set is lifted", w.ask == Some(None), w.ask);
    let w = plan(&|f| (f.minimised, f.open_too) = (false, true));
    c.check("plan: \"Limit open windows too\": one in view in its game is limited", w.ask == Some(Some(1024)), w.ask);
    let w = plan(&|f| (f.minimised, f.open_too, f.game) = (false, true, Game::Unknown));
    c.check("plan: …but not one still opening (its log says nothing yet)", w.ask.is_none() && w.cap == RamCap::Lift, (w.ask, w.cap));

    // A window lent for a moment is left alone, it alone (it was the whole pass for every window).
    let lent = plan(&|f| (f.memory.held, f.minimised, f.lent) = (Some(1024), false, true));
    let other = plan(&|f| f.memory.held = Some(2048));
    c.check("plan: a lent window shown for a moment keeps its limit", lent.cap == RamCap::Leave && lent.ask.is_none(), (lent.cap, lent.ask));
    c.check("plan: …while another window is still decided as usual", other.ask == Some(Some(1024)), other.ask);
    let w = plan(&|f| (f.lent, f.memory.resting_since) = (true, Some(ago(61.0))));
    c.check("plan: …and a lent window isn't emptied, its rest going on", !w.empty_once && w.memory.resting_since == Some(ago(61.0)), w.memory.resting_since);

    // An ask on its way isn't asked twice; a refused one is asked again.
    let w = plan(&|f| f.memory.asked = Some(Some(1024)));
    c.check("plan: a limit on its way to Windows isn't asked again", w.ask.is_none(), w.ask);
    let w = plan(&|f| (f.memory.held, f.memory.asked, f.minimised) = (Some(1024), Some(None), false));
    c.check("plan: nor a lift on its way", w.ask.is_none(), w.ask);
    let w = plan(&|f| (f.memory.held, f.memory.refused, f.minimised) = (Some(1024), true, false));
    c.check("plan: a lift Windows refused (the limit still held) is asked again", w.ask == Some(None) && w.memory.asked == Some(None), (w.ask, w.memory.asked));
    let w = plan(&|f| f.memory.refused = true);
    c.check("plan: a limit Windows refused is asked again", w.ask == Some(Some(1024)), w.ask);

    // Auto, step by step (what needed a window: the plan has no Win32).
    let mut f = facts(now);
    f.setting = RamSetting::Auto;
    f.load_percent = 80;
    let w = ram_plan(&f);
    c.check("plan: Auto: a window minimized a moment ago isn't limited yet", w.ask.is_none() && w.why == "auto left it alone" && w.memory.auto.is_some(), (w.ask, w.why));
    let mut auto = w.memory.auto.clone();
    let look = |f: &mut RamFacts, at: f64, faults: i64, auto: &mut Option<AutoRam>| {
        f.now = now.plus_secs(at);
        f.page_faults = faults;
        auto_cap(auto, f).0
    };
    let mut quiet = facts(now);
    quiet.load_percent = 40;
    let mut quiet_auto = auto.clone();
    c.check("plan: …nor with the PC's RAM mostly free (40 % used)", look(&mut quiet, 61.0, 0, &mut quiet_auto).is_none(), ());
    let cap = look(&mut f, 61.0, 1000, &mut auto);
    c.check("plan: …rested a minute with RAM 80 % used: limited to 3/4 of what it holds (2200 -> 1600 MB)", cap == Some(1600), cap);
    let mut faults = 1000;
    let mut cap = None;
    for step in 1..=40 {
        faults += 100;
        cap = look(&mut f, 61.0 + step as f64 * 1.5, faults, &mut auto);
    }
    c.check("plan: …calm a minute: a quarter lower (1152 MB)", cap == Some(1152), cap);
    // Faulting hard while it holds well under its limit: Windows took its RAM, not the limit.
    let mut starved = f.clone();
    starved.working_set = 400 * MB;
    let mut starved_auto = auto.clone();
    let mut s_faults = faults;
    let mut s_cap = None;
    for step in 1..=5 {
        s_faults += 100_000;
        s_cap = look(&mut starved, 125.0 + step as f64 * 1.5, s_faults, &mut starved_auto);
    }
    c.check("plan: …faulting with Windows taking its RAM (400 of 1152 MB held): not raised, the limit isn't why", s_cap == Some(1152), s_cap);
    f.working_set = 1152 * MB;
    for step in 1..=3 {
        faults += 100_000;
        cap = look(&mut f, 125.0 + step as f64 * 1.5, faults, &mut auto);
    }
    c.check("plan: …faulting a lot: half again (1728 MB)", cap == Some(1728), cap);
    f.working_set = 3000 * MB; // holding all it may: its faults are the limit's doing
    let mut note = None;
    for step in 4..=9 {
        faults += 100_000;
        f.now = now.plus_secs(125.0 + step as f64 * 1.5);
        f.page_faults = faults;
        let (got, said) = auto_cap(&mut auto, &f);
        cap = got;
        note = note.or(said);
    }
    c.check("plan: …still faulting after two raises: left alone, the limit lifted", cap.is_none() && note.as_deref().is_some_and(|n| n.contains("needs its RAM")), (cap, note));
    let later = look(&mut f, 600.0, faults, &mut auto);
    c.check("plan: …and not limited again for a while", later.is_none(), later);

    // Auto's limit kept across a move to a server (it was forgotten there, and waited its minute again after).
    let mut f = facts(now);
    f.setting = RamSetting::Auto;
    f.memory = RamMemory { held: Some(1152), auto: Some(auto_at(now, 1152)), ..Default::default() };
    f.game = Game::InGame { at: ago(300.0), teleport: Some(ago(2.0)) };
    let moving = ram_plan(&f);
    c.check(
        "plan: Auto: moving to a server: 2048 MB, Auto's limit kept",
        moving.ask == Some(Some(2048)) && moving.memory.auto.as_ref().is_some_and(|a| a.cap_mb == 1152 && a.faults == -1),
        (moving.ask, moving.memory.auto.as_ref().map(|a| a.cap_mb)),
    );
    f.memory = RamMemory { held: Some(2048), ..moving.memory };
    f.now = now.plus_secs(20.0);
    f.game = Game::InGame { at: now.plus_secs(5.0), teleport: None };
    let settled = ram_plan(&f);
    c.check("plan: …settled on the new server: back to its 1152 MB at once (auto)", settled.ask == Some(Some(1152)) && settled.why == "auto", (settled.ask, settled.why));
    let mut f = facts(now);
    f.setting = RamSetting::Auto;
    f.memory = RamMemory { held: Some(1152), auto: Some(auto_at(now, 1152)), ..Default::default() };
    f.minimised = false;
    let shown = ram_plan(&f);
    c.check("plan: …shown: lifted, and Auto starts over, waiting its minute", shown.ask == Some(None) && shown.memory.auto.is_none(), (shown.ask, shown.memory.auto.is_some()));
    f.setting = RamSetting::Mb(1024);
    f.minimised = true;
    let off_auto = ram_plan(&f);
    c.check("plan: …and Auto left for a chosen limit: forgotten", off_auto.memory.auto.is_none() && off_auto.ask == Some(Some(1024)), off_auto.ask);

    // Save RAM.
    let rest = |since: Option<f64>, emptied: bool, change: &dyn Fn(&mut RamFacts)| {
        let mut f = facts(now);
        f.memory.resting_since = since.map(ago);
        f.memory.emptied = emptied;
        change(&mut f);
        ram_plan(&f)
    };
    let w = rest(None, false, &|_| {});
    c.check("plan: Save RAM: not emptied when its rest begins", !w.empty_once && w.memory.resting_since == Some(now), w.memory.resting_since);
    c.check("plan: …nor half a minute on", !rest(Some(30.0), false, &|_| {}).empty_once, ());
    c.check("plan: …emptied a minute on", rest(Some(61.0), false, &|_| {}).empty_once, ());
    c.check("plan: …only once", !rest(Some(120.0), true, &|_| {}).empty_once, ());
    let w = rest(Some(120.0), true, &|f| f.minimised = false);
    c.check("plan: …shown: the rest ends", w.memory.resting_since.is_none() && !w.memory.emptied, w.memory.resting_since);
    let w = rest(Some(120.0), false, &|f| f.game = Game::InGame { at: ago(5.0), teleport: None });
    c.check("plan: …not emptied while moving to a server", !w.empty_once && w.memory.resting_since.is_none(), ());
    let w = rest(Some(120.0), false, &|f| f.save_ram = false);
    c.check("plan: Save RAM off: a resting window keeps its RAM", !w.empty_once && w.memory.resting_since.is_none(), ());
}

/// The CPU limit over every window, setting and memory: the rule written out here once more, and the same windows as the
/// RAM limit's chosen number (minus the move to a server, which has no CPU limit at all).
fn cpu_table(c: &mut Checks) {
    let now = Stamp::utc_now();
    let ago = |s: f64| now.plus_secs(-s);
    let games = [
        ("no log yet", Game::Unknown),
        ("joining", Game::Joining { at: ago(3.0) }),
        ("a join stuck 6 min", Game::Joining { at: ago(360.0) }),
        ("in its game 5 s", Game::InGame { at: ago(5.0), teleport: None }),
        ("in its game 5 min", Game::InGame { at: ago(300.0), teleport: None }),
        ("teleporting", Game::InGame { at: ago(300.0), teleport: Some(ago(30.0)) }),
        ("left", Game::Left),
    ];
    let views: [(&str, bool, bool); 3] = [("minimized", true, false), ("hidden", false, true), ("in view", false, false)];
    for percent in CPU_LIMIT_CHOICES {
        let (mut rows, mut wrong) = (0, Vec::new());
        for open_too in [false, true] {
            for front in [false, true] {
                for main in [false, true] {
                    for has_window in [false, true] {
                        for (view, minimised, hidden) in views {
                            for (game_name, game) in games {
                                for lent in [false, true] {
                                    for (asked, held) in [(None, None), (None, Some(percent.max(5))), (None, Some(50)), (Some(Some(20)), None), (Some(None), Some(20))] {
                                        let mut f = facts(now);
                                        (f.cpu_percent, f.open_too, f.front, f.main, f.has_window) = (percent, open_too, front, main, has_window);
                                        (f.minimised, f.hidden) = (has_window && minimised, has_window && hidden);
                                        (f.game, f.lent) = (game, lent);
                                        f.setting = RamSetting::Mb(1024);
                                        f.cpu = CpuMemory { asked, held, refused: false };
                                        rows += 1;
                                        let w = cpu_plan(&f);
                                        let resting = !front && !main && has_window && (f.minimised || f.hidden || (open_too && game != Game::Unknown));
                                        let want = if lent {
                                            CpuCap::Leave
                                        } else if percent == 0 || !resting || game.settling(now) {
                                            CpuCap::Lift
                                        } else {
                                            CpuCap::Percent(percent)
                                        };
                                        let had = asked.unwrap_or(held);
                                        let want_ask = match want {
                                            CpuCap::Leave => None,
                                            CpuCap::Lift => had.map(|_| None),
                                            CpuCap::Percent(p) => (had != Some(p)).then_some(Some(p)),
                                        };
                                        let ram = ram_plan(&f);
                                        let same_windows = lent || percent == 0 || ((ram.why == "chosen") == matches!(w.cap, CpuCap::Percent(_)));
                                        let ok = w.cap == want && w.ask == want_ask && w.memory.asked == want_ask.or(asked) && w.memory.held == held && same_windows;
                                        if !ok && wrong.len() < 3 {
                                            wrong.push(format!(
                                                "open_too {open_too}, front {front}, main {main}, window {has_window}, {view}, {game_name}, lent {lent}, asked {asked:?} held {held:?}: {:?}/{:?}, wanted {want:?}/{want_ask:?}, RAM {}",
                                                w.cap, w.ask, ram.why
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        let name = if percent == 0 { "off".to_string() } else { format!("{percent} %") };
        c.check(&format!("CPU plan: {name}: every case as the rule says, the same windows as the RAM limit's ({rows} cases)"), wrong.is_empty(), wrong);
    }
}

/// Named cases of the CPU plan.
fn cpu_cases(c: &mut Checks) {
    let now = Stamp::utc_now();
    let ago = |s: f64| now.plus_secs(-s);
    let plan = |change: &dyn Fn(&mut RamFacts)| {
        let mut f = facts(now);
        f.cpu_percent = 10;
        change(&mut f);
        cpu_plan(&f)
    };
    let w = plan(&|_| {});
    c.check("CPU plan: minimized, 10 % chosen, in its game 5 min: limited to 10 % (chosen)", w.ask == Some(Some(10)) && w.why == "chosen", (w.ask, w.why));
    let w = plan(&|f| f.cpu.held = Some(10));
    c.check("CPU plan: …already held there: nothing asked", w.ask.is_none() && w.cap == CpuCap::Percent(10), (w.ask, w.cap));
    let w = plan(&|f| f.cpu.held = Some(20));
    c.check("CPU plan: …held at another number: changed", w.ask == Some(Some(10)), w.ask);
    let w = plan(&|f| (f.cpu.held, f.game) = (Some(10), Game::InGame { at: ago(5.0), teleport: None }));
    c.check("CPU plan: just joined (5 s): no limit at all while it loads (lifted)", w.ask == Some(None) && w.why == "moving to a server", (w.ask, w.why));
    let w = plan(&|f| (f.cpu.held, f.game) = (Some(10), Game::InGame { at: ago(300.0), teleport: Some(ago(2.0)) }));
    c.check("CPU plan: teleporting: lifted", w.ask == Some(None) && w.cap == CpuCap::Lift, w.ask);
    let w = plan(&|f| f.game = Game::Joining { at: ago(3.0) });
    c.check("CPU plan: joining with no limit: nothing asked", w.ask.is_none() && w.cap == CpuCap::Lift, w.ask);
    let w = plan(&|f| (f.cpu.held, f.minimised) = (Some(10), false));
    c.check("CPU plan: shown again: lifted", w.ask == Some(None), w.ask);
    let w = plan(&|f| (f.cpu.held, f.front) = (Some(10), true));
    c.check("CPU plan: in front: lifted", w.ask == Some(None), w.ask);
    let w = plan(&|f| (f.cpu.held, f.main) = (Some(10), true));
    c.check("CPU plan: a main account's: lifted", w.ask == Some(None), w.ask);
    let w = plan(&|f| (f.cpu.held, f.cpu_percent) = (Some(10), 0));
    c.check("CPU plan: off: what Bloxkeep set is lifted", w.ask == Some(None) && w.why == "the option off", (w.ask, w.why));
    let w = plan(&|f| f.cpu_percent = 0);
    c.check("CPU plan: off with nothing set: nothing asked", w.ask.is_none(), w.ask);
    let w = plan(&|f| (f.minimised, f.open_too) = (false, true));
    c.check("CPU plan: \"Limit open windows too\": one in view in its game is limited", w.ask == Some(Some(10)), w.ask);
    let w = plan(&|f| (f.minimised, f.open_too, f.game) = (false, true, Game::Unknown));
    c.check("CPU plan: …but not one still opening (its log says nothing yet)", w.ask.is_none() && w.cap == CpuCap::Lift, (w.ask, w.cap));
    let w = plan(&|f| (f.minimised, f.open_too) = (false, false));
    c.check("CPU plan: in view with the switch off: not limited", w.ask.is_none() && w.cap == CpuCap::Lift, w.cap);
    let w = plan(&|f| (f.minimised, f.hidden) = (false, true));
    c.check("CPU plan: hidden by Bloxkeep: limited", w.ask == Some(Some(10)), w.ask);
    let w = plan(&|f| (f.has_window, f.minimised) = (false, false));
    c.check("CPU plan: no window (still opening): not limited", w.ask.is_none() && w.cap == CpuCap::Lift, w.cap);
    let w = plan(&|f| (f.cpu.held, f.minimised, f.lent) = (Some(10), false, true));
    c.check("CPU plan: a lent window shown for a moment keeps its limit", w.cap == CpuCap::Leave && w.ask.is_none(), (w.cap, w.ask));
    let w = plan(&|f| f.cpu.asked = Some(Some(10)));
    c.check("CPU plan: a limit on its way to Windows isn't asked again", w.ask.is_none(), w.ask);
    let w = plan(&|f| (f.cpu.held, f.cpu.asked, f.minimised) = (Some(10), Some(None), false));
    c.check("CPU plan: nor a lift on its way", w.ask.is_none(), w.ask);
    let w = plan(&|f| f.cpu.refused = true);
    c.check("CPU plan: a limit Windows refused is asked again", w.ask == Some(Some(10)), w.ask);
    let w = plan(&|f| (f.cpu.held, f.cpu.refused, f.minimised) = (Some(10), true, false));
    c.check("CPU plan: a lift Windows refused (the limit still held) is asked again", w.ask == Some(None), w.ask);
}

/// This process's CPU time so far (user and kernel), in seconds.
fn own_cpu_seconds() -> f64 {
    use windows_sys::Win32::{Foundation::FILETIME, System::Threading::{GetCurrentProcess, GetProcessTimes}};
    let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
    // SAFETY: four out values on the stack; the pseudo handle of this process.
    unsafe { GetProcessTimes(GetCurrentProcess(), &mut created, &mut exited, &mut kernel, &mut user) };
    let ticks = |t: FILETIME| (((t.dwHighDateTime as u64) << 32) | t.dwLowDateTime as u64) as f64 / 10_000_000.0;
    ticks(kernel) + ticks(user)
}

/// Waits (at most 3 s) for the RAM turn's answers to a CPU ask to come back to the app.
async fn cpu_answered(app: &crate::core::app::App, pid: i32) -> CpuAsk {
    for _ in 0..30 {
        if app.memory.cpu_asks.borrow().get(&pid).is_some_and(|a| a.asked.is_none()) {
            break;
        }
        rt::sleep(Duration::from_millis(100)).await;
    }
    app.memory.cpu_asks.borrow().get(&pid).cloned().unwrap_or_default()
}

/// The CPU limit on the test's own process (no program is started; a job that holds the test ends with it): the job's
/// cap set, read back, changed and switched off; Windows holding a spinning process to it; the process still running
/// once the job's handle is closed; and the app's checked path, the front window's lift, a refusal, the setting's
/// choices, and every cap switched off at locking and at closing.
async fn cpu_on_own_process(c: &mut Checks, app: &std::rc::Rc<crate::core::app::App>) {
    let me = std::process::id() as i32;
    const ON: u32 = JOB_OBJECT_CPU_RATE_CONTROL_ENABLE | JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP;
    let set = cpu_cap::set(me, Some(20));
    c.check("CPU cap: set to 20 %: Windows holds the job at 2000 of 10 000 cycles, a hard cap", set.is_ok() && cpu_cap::read(me) == Some((ON, 2000)), (set, cpu_cap::read(me)));
    let set = cpu_cap::set(me, Some(10));
    c.check("CPU cap: changed to 10 %: the same job, at 1000", set.is_ok() && cpu_cap::read(me) == Some((ON, 1000)), (set, cpu_cap::read(me)));

    // Held to it: a few threads spinning 2 s under a 5 % cap use at most about 5 % of all the PC's processors. Uncapped
    // they would use one core each, so with as many cores as that the measure says nothing.
    let cores = crate::core::cpu_cores::logical_count().max(1) as f64;
    let threads = 4usize;
    if cores * 0.05 * 1.5 < threads as f64 * 0.75 {
        let set = cpu_cap::set(me, Some(5));
        let before = own_cpu_seconds();
        let start = std::time::Instant::now();
        let spinners: Vec<_> = (0..threads)
            .map(|_| {
                std::thread::spawn(move || {
                    let mut x = 0u64;
                    while start.elapsed() < Duration::from_secs(2) {
                        x = std::hint::black_box(x.wrapping_mul(6364136223846793005).wrapping_add(1));
                    }
                })
            })
            .collect();
        for s in spinners {
            let _ = s.join();
        }
        let used = own_cpu_seconds() - before;
        let wall = start.elapsed().as_secs_f64();
        let most = cores * 0.05 * wall * 1.5 + 0.3; // half again the cap, and a little for the start
        c.check(
            &format!("CPU cap: {threads} threads spinning {wall:.1} s under 5 %: held near 5 % of {cores} processors ({used:.2} s of CPU, uncapped {:.1})", threads as f64 * wall),
            set.is_ok() && used <= most && used < threads as f64 * wall * 0.75,
            (set, used, most),
        );
    } else {
        c.skip("CPU cap: threads spinning under 5 %: held near 5 % of the PC's processors", "too many processors to tell");
    }

    // Bloxkeep crashing switches every cap off (the crash filter calls it; no later Bloxkeep can reach the job).
    let set = cpu_cap::set(me, Some(25));
    cpu_cap::lift_all_at_crash();
    c.check("CPU cap: switched off as Bloxkeep crashes", set.is_ok() && cpu_cap::read(me).is_some_and(|(flags, _)| flags == 0), (set, cpu_cap::read(me)));

    let set = cpu_cap::set(me, None);
    c.check("CPU cap: switched off: the job's rate control is off", set.is_ok() && cpu_cap::read(me).is_some_and(|(flags, _)| flags == 0), (set, cpu_cap::read(me)));
    let forgot = cpu_cap::forget(me);
    c.check("CPU cap: the job's handle closed: the process goes on (no kill on close)", forgot && !roblox_processes::ended(me) && cpu_cap::read(me).is_none(), forgot);
    let set = cpu_cap::set(me, None);
    c.check("CPU cap: a lift for a process with no job of Bloxkeep's has nothing to do", set.is_ok() && cpu_cap::read(me).is_none(), set);

    // The setting: only the choices Settings offers.
    let percent = || app.vault(|d| d.settings.cpu_limit_percent).unwrap_or(i32::MIN);
    let refused = call(app, "setSettings", json!({ "cpuLimit": 7 })).await;
    c.check("CPU limit: off by default, and only the choices Settings offers are taken (7 refused)", percent() == 0 && user_error(&refused).is_none(), percent());
    let _ = call(app, "setSettings", json!({ "cpuLimit": 101 })).await;
    c.check("CPU limit: …nor 101", percent() == 0, percent());
    for p in [5, 50, 20] {
        let _ = call(app, "setSettings", json!({ "cpuLimit": p })).await;
        c.check(&format!("CPU limit: {p} % can be chosen"), percent() == p, percent());
    }

    // The checked path: remembered as held only once Windows said yes; the front window's lifted at once.
    app.ask_cpu_limit(me, Some(30), "chosen");
    let ask = cpu_answered(app, me).await;
    c.check("CPU limit: asked through the RAM turn, held once Windows said yes", ask.held == Some(30) && !ask.refused && cpu_cap::read(me) == Some((ON, 3000)), (ask.held, cpu_cap::read(me)));
    app.front_memory(me);
    let ask = cpu_answered(app, me).await;
    c.check(
        "CPU limit: brought to the front: lifted at once, remembered lifted once Windows said yes",
        ask.held.is_none() && ask.asked.is_none() && cpu_cap::read(me).is_some_and(|(flags, _)| flags == 0),
        (ask.held, cpu_cap::read(me)),
    );
    const GONE: i32 = 0x7FFF_FFF0;
    app.ask_cpu_limit(GONE, Some(10), "chosen");
    let ask = cpu_answered(app, GONE).await;
    c.check("CPU limit: a limit that didn't hold is remembered as not done, to be asked again", ask.held.is_none() && ask.refused && ask.asked.is_none(), (ask.held, ask.refused));
    app.memory.cpu_asks.borrow_mut().remove(&GONE);

    // Locking and closing switch every cap off: a job outlives Bloxkeep while its process runs.
    app.ask_cpu_limit(me, Some(20), "chosen");
    let held = cpu_answered(app, me).await.held;
    let _ = call(app, "lock", json!({})).await;
    c.check(
        "CPU limit: locking the vault switches it off",
        held == Some(20) && cpu_cap::read(me).is_some_and(|(flags, _)| flags == 0) && app.memory.cpu_asks.borrow().is_empty(),
        (held, cpu_cap::read(me)),
    );
    let _ = call(app, "unlock", json!({ "password": PASSWORD })).await;
    app.ask_cpu_limit(me, Some(15), "chosen");
    let held = cpu_answered(app, me).await.held;
    c.check("CPU limit: …after an unlock the same job is used again", held == Some(15) && cpu_cap::read(me) == Some((ON, 1500)), (held, cpu_cap::read(me)));
    app.lift_cpu_limits();
    c.check("CPU limit: Bloxkeep closing switches it off", cpu_cap::read(me).is_some_and(|(flags, _)| flags == 0), cpu_cap::read(me));
    let _ = call(app, "setSettings", json!({ "cpuLimit": 0 })).await;
}

/// Waits (at most 3 s) for the RAM turn's answers to come back to the app.
async fn answered(app: &crate::core::app::App, pid: i32) -> RamAsk {
    for _ in 0..30 {
        if app.memory.ram_asks.borrow().get(&pid).is_some_and(|a| a.asked.is_none()) {
            break;
        }
        rt::sleep(Duration::from_millis(100)).await;
    }
    app.memory.ram_asks.borrow().get(&pid).cloned().unwrap_or_default()
}

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
        ("on the new server 5 s: not yet limited", Some(state("ingame", 5., None)), true),
        ("on the new server 2 min: limited again", Some(state("ingame", 120., None)), false),
        ("a join stuck 6 min: limited again", Some(state("joining", 360., None)), false),
        ("dropped: limited", Some(state("disconnected", 5., None)), false),
        ("no log read: limited", None, false),
    ] {
        c.check(what, settling(s.as_ref(), now) == want, s.map(|s| (s.status, now.secs_since(s.at))));
    }
    // The same read from Roblox's own lines (a real teleport's, a user's 0.748 log, times moved), the limit back on
    // every way out: in the new server, a teleport that never went on, a join that hangs.
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
        ("log: just joined: not limited", &in_game[..], 5.0, true),
        ("log: in its game 13 s: limited", &in_game[..], 13.4, false),
        ("log: a teleport begun: not limited", &teleport[..2 + 1], 600.1, true),
        ("log: joining the next server: not limited", &teleport[..2 + 4], 602.5, true),
        ("log: 5 s on the new server: not limited", &teleport[..], 608.1, true),
        ("log: 13 s on the new server: limited again", &teleport[..], 616.2, false),
        ("log: a teleport that never went on, 4 min: not limited", &teleport[..2 + 1], 840.0, true),
        ("log: …6 min: limited again", &teleport[..2 + 1], 960.0, false),
        ("log: a join that hangs 6 min: limited again", &teleport[..2 + 4], 962.5, false),
    ] {
        c.check(what, read(lines, at) == want, at);
    }
    plan_table(c);
    plan_cases(c);
    cpu_table(c);
    cpu_cases(c);

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

    // The limit pass on the test's own process (the option off, no window): a client found with a limit nobody here
    // asked for is taken on at first sight and lifted, and a lift is remembered only once Windows said yes.
    let players = [PlayerProcess { pid: me, ..Default::default() }];
    let half_used = MemoryStatus { total_mb: 16000, available_mb: 8000, load_percent: 50, ..Default::default() };
    roblox_processes::limit_ram(me, Some(512));
    app.limit_resting_ram(&players, &HashMap::new(), Stamp::utc_now(), &half_used);
    let ask = answered(&app, me).await;
    c.check(
        "a window found limited at first sight (no record of it) is taken on and lifted",
        !limited() && ask.held.is_none() && !app.memory.ram_limited.borrow().contains_key(&me),
        (limited(), ask.held),
    );
    // A lift Windows refused before a lock leaves the limit; after it every client is looked at afresh.
    roblox_processes::limit_ram(me, Some(512));
    app.lift_ram_limits();
    app.limit_resting_ram(&players, &HashMap::new(), Stamp::utc_now(), &half_used);
    let ask = answered(&app, me).await;
    c.check("…after a lock, a limit still there is found again and lifted", !limited() && ask.held.is_none(), (limited(), ask.held));
    roblox_processes::limit_ram(me, Some(512));
    app.memory.ram_asks.borrow_mut().insert(me, RamAsk { held: Some(512), ..Default::default() });
    app.front_memory(me);
    let ask = answered(&app, me).await;
    c.check(
        "brought to the front: its limit lifted at once, remembered lifted once Windows said yes",
        !limited() && ask.held.is_none() && ask.asked.is_none(),
        (limited(), ask.held, ask.asked),
    );
    // A lift that didn't hold (here: a process id no process has) stays remembered as limited, to be asked again.
    const GONE: i32 = 0x7FFF_FFF0;
    let from = app.memory.ram_ask_serial.get();
    app.memory.ram_asks.borrow_mut().insert(GONE, RamAsk { held: Some(1024), from, ..Default::default() });
    app.front_memory(GONE);
    let ask = answered(&app, GONE).await;
    c.check(
        "a lift that didn't hold is remembered as not done: still limited, asked again",
        ask.held == Some(1024) && ask.asked.is_none() && ask.refused && app.memory.ram_limited.borrow().get(&GONE) == Some(&1024),
        (ask.held, ask.asked, ask.refused),
    );
    app.memory.ram_asks.borrow_mut().remove(&GONE);
    app.memory.ram_limited.borrow_mut().remove(&GONE);

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

    cpu_on_own_process(c, &app).await;
}
