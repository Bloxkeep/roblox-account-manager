// Tests/Bloxkeep.Tests/AfkSuite.cs: anti-AFK's decisions (app/anti_afk.rs) with made-up process ids, window handles, idle
// times, lock and full-screen states: which windows are nudged, why the others wait, retries, and windows already open
// when Bloxkeep starts. The nudge itself never runs (`nudging` set, anti_afk_due called directly), so nothing on the
// desktop is touched. The C#'s checks on real windows of its own (made unseen, put back in their place) and on Windows'
// raw input are skipped here: they need windows.
use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    rc::Rc,
    time::{Duration, Instant},
};

use super::{unlock, window::quiet_app as app, Checks, PASSWORD};
use crate::core::{
    anti_afk::{self, LAST_CHANCE},
    app::App,
    base::Stamp,
    input_turn,
    roblox_input,
    roblox_log_watcher::{ClientState, Tail},
    rt,
    vault::Vault,
};

fn mins(m: f64) -> f64 {
    m * 60.0
}

pub async fn run(c: &mut Checks, _dir: &str) {
    let seed = Vault::create(PASSWORD, None).expect("vault");
    seed.data().settings.anti_afk = true;
    seed.save().expect("saved");
    drop(seed);
    let (app, _said) = app();
    unlock(&app).await;
    app.controller.nudging.set(true);
    let st = &app.anti_afk;
    let last_chance = LAST_CHANCE.as_secs_f64();
    let li = |pid: i32| st.last_input.borrow().get(&pid).copied();

    // Windows already open when Bloxkeep starts (an update's restart).
    let now = Stamp::utc_now();
    app.run_anti_afk(now, &HashMap::from([(990001, vec![1]), (990002, vec![2])]));
    c.check("open at start: due at once, even for a nudge while the PC is in use", li(990001) == Some(now.plus_secs(-last_chance)), li(990001));
    c.check("both windows open at start", li(990002) == Some(now.plus_secs(-last_chance)), li(990002));
    let later = now.plus_secs(30.0);
    app.run_anti_afk(later, &HashMap::from([(990001, vec![1]), (990002, vec![2]), (990003, vec![3])]));
    c.check("opened later: counts from now", li(990003) == Some(later), li(990003));
    c.check("start windows keep their time", li(990001) == Some(now.plus_secs(-last_chance)), ());
    app.run_anti_afk(later.plus_secs(1.0), &HashMap::from([(990003, vec![3])]));
    c.check("closed windows forgotten", li(990001).is_none() && li(990003).is_some(), ());

    // The decision. Windows idle 5 min (A), 13 min (B), 19 min (C); D is a Roblox window that may be in front.
    const A: i32 = 1001;
    const B: i32 = 1002;
    const C: i32 = 1003;
    const D: i32 = 1004;
    const OTHER: i32 = 4242;
    let wins: HashMap<i32, Vec<isize>> = HashMap::from([(A, vec![11]), (B, vec![12]), (C, vec![13]), (D, vec![14])]);
    let on: HashSet<i32> = HashSet::from([A, B, C, D]);
    let full_asked = Rc::new(Cell::new(false));
    let clear = || {
        st.last_input.borrow_mut().clear();
        st.nudge_retry_at.borrow_mut().clear();
        st.afk_held.borrow_mut().clear();
    };
    let decide = |idle: Duration, locked: bool, foreground: i32, full_screen: bool, only: Option<HashSet<i32>>| -> Vec<i32> {
        let at = Stamp::utc_now();
        clear();
        {
            let mut l = st.last_input.borrow_mut();
            l.insert(A, at.plus_secs(-mins(5.0)));
            l.insert(B, at.plus_secs(-mins(13.0)));
            l.insert(C, at.plus_secs(-mins(19.0)));
            l.insert(D, at.plus_secs(-mins(1.0)));
        }
        full_asked.set(false);
        let asked = full_asked.clone();
        let mut r = app.anti_afk_due(at, only.as_ref().unwrap_or(&on), &wins, locked, idle, foreground, &move || {
            asked.set(true);
            full_screen
        });
        r.sort();
        r
    };
    let held = |pid: i32| st.afk_held.borrow().get(&pid).map(|h| h.to_string()).unwrap_or_else(|| "-".into());
    let list = |r: &[i32]| r.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(",");
    let secs = Duration::from_secs;

    let r = decide(secs(90), false, OTHER, false, None);
    c.check("away: windows idle 12+ min nudged, the 5-min one not", r == [B, C], list(&r));
    // A window Bloxkeep is closing (closed as it came up, here) was still nudged, brought to the front on its way out.
    app.close_desk.closing.borrow_mut().insert(
        C,
        crate::core::close_plan::Closing {
            pid: C,
            account: None,
            cause: crate::core::close_plan::Cause::User,
            then: crate::core::close_plan::Then::StayClosed,
            asked: Stamp::utc_now(),
            forced: false,
            sent: true,
        },
    );
    let r = decide(secs(90), false, OTHER, false, None);
    c.check("a window being closed isn't nudged, nor held (fails on the old code)", r == [B] && held(C) == "-", list(&r));
    app.close_desk.closing.borrow_mut().clear();
    let r = decide(secs(20), false, OTHER, false, None);
    c.check("in use, 20 s since input: every due window goes (12 min by default), not only the one about to be kicked", r == [B, C], list(&r));
    c.check(
        "in use: due windows noted as held (PC in use)",
        held(B) == "InUse" && held(C) == "InUse" && held(A) == "-",
        format!("{} {} {}", held(A), held(B), held(C)),
    );
    let r = decide(secs(20), false, OTHER, false, Some(HashSet::from([A, D])));
    c.check("in use, nothing due: nothing nudged", r.is_empty(), list(&r));
    c.check("nothing due: full-screen state not asked", !full_asked.get(), ());
    let r = decide(secs(1), false, OTHER, false, None);
    c.check("in use, 1 s pause: due windows wait", r.is_empty(), list(&r));
    let r = decide(secs(3), false, OTHER, false, None);
    c.check("in use, 3 s pause: the due windows", r == [B, C], list(&r));
    c.check("full-screen state asked only when it matters", full_asked.get(), ());
    let r = decide(secs(3), false, OTHER, true, None);
    c.check("full-screen app in front: not interrupted", r.is_empty() && held(C) == "FullScreen", format!("{} {}", list(&r), held(C)));
    let r = decide(secs(3), false, D, true, None);
    c.check(
        "a Roblox window in front (even full screen): the other due ones go",
        r == [B, C] && !full_asked.get(),
        format!("{} asked={}", list(&r), full_asked.get()),
    );
    c.check(
        "the Roblox window in front counts as played in",
        li(D).is_some_and(|d| Stamp::utc_now().secs_since(d) < 5.0),
        li(D),
    );
    let r = decide(Duration::from_secs(5 * 60), true, OTHER, false, None);
    c.check(
        "locked: nothing tried, noted as locked",
        r.is_empty() && held(B) == "Locked" && held(C) == "Locked",
        format!("{} {} {}", list(&r), held(B), held(C)),
    );
    // Settings' minutes (5 to 18): a game that kicks sooner than Roblox's 20 (Pet Simulator 99, about 15) is pressed for
    // in time, the PC in use or not.
    let _ = app.vault_mut(|d| d.settings.anti_afk_minutes = 5);
    let r = decide(secs(3), false, OTHER, false, None);
    c.check("every 5 minutes, in use: the 5-minute window goes too", r == [A, B, C], list(&r));
    let _ = app.vault_mut(|d| d.settings.anti_afk_minutes = 13);
    let r = decide(secs(90), false, OTHER, false, None);
    c.check("every 13 minutes, away: the 13-minute window goes, the 5-minute one not", r == [B, C], list(&r));
    let _ = app.vault_mut(|d| d.settings.anti_afk_minutes = 40);
    c.check("a time past 18 minutes is read as 18 (Roblox kicks at 20)", app.afk_every() == Duration::from_secs(18 * 60), app.afk_every());
    let _ = app.vault_mut(|d| d.settings.anti_afk_minutes = 12);
    // Settings' key: F15 by default, Space to jump, any macro key; not a mouse button or a made-up name.
    c.check("the key is F15 unless picked", app.vault(|d| d.settings.anti_afk_key.clone()).ok().as_deref() == Some("F15"), ());
    c.check(
        "F15, Space and other keyboard keys can be picked; a mouse button or an unknown name can't",
        anti_afk::key_ok("F15") && anti_afk::key_ok("Space") && anti_afk::key_ok("E") && !anti_afk::key_ok("MouseLeft") && !anti_afk::key_ok("Nope"),
        (),
    );

    // A refused window waits before its next try, but not past its last chance.
    let at0 = Stamp::utc_now();
    let within = |misses: i32, idle_min: f64| App::nudge_retry_at(at0, misses, at0.plus_secs(-mins(idle_min))).secs_since(at0) / 60.0;
    c.check("miss 1 at 13 min: next try in 1 min", (within(1, 13.0) - 1.0).abs() < 0.01, within(1, 13.0));
    c.check("miss 4 at 13 min: not after the last chance (18 min)", (within(4, 13.0) - 5.0).abs() < 0.01, within(4, 13.0));
    c.check("past the last chance: tried again within a minute", (within(9, 19.0) - 1.0).abs() < 0.01, within(9, 19.0));
    c.check("long run of misses early on: 15 min at most", (within(5, 2.0) - 15.0).abs() < 0.01, within(5, 2.0));

    // A due window keeps its wait while the PC is in use; the others' waits are cleared.
    clear();
    let t1 = Stamp::utc_now();
    st.last_input.borrow_mut().insert(B, t1.plus_secs(-mins(10.0)));
    st.last_input.borrow_mut().insert(C, t1.plus_secs(-mins(19.0)));
    st.nudge_retry_at.borrow_mut().insert(B, t1.plus_secs(mins(3.0)));
    st.nudge_retry_at.borrow_mut().insert(C, t1.plus_secs(40.0));
    let rw = app.anti_afk_due(t1, &HashSet::from([B, C]), &wins, false, secs(3), OTHER, &|| false);
    c.check("due window still waiting: not tried yet", rw.is_empty(), list(&rw));
    let keys: Vec<i32> = st.nudge_retry_at.borrow().keys().copied().collect();
    c.check(
        "in use clears the other windows' waits only",
        !keys.contains(&B) && keys.contains(&C),
        list(&keys),
    );

    // The person's own input as Windows' raw input names it (RobloxInput): a Roblox window in front gets only what reached
    // it. The PC in use with it in front but none of the input its own (a mouse on another monitor): not counted, so at its
    // last chance it is nudged in a pause. Before, any input then counted as its own, and it was kicked (2026-10-19:
    // idle-kicked 8 minutes after Bloxkeep last counted input for it).
    *st.roblox_input_at.borrow_mut() = Some(Box::new(|_| None));
    {
        clear();
        let t2 = Stamp::utc_now();
        st.last_input.borrow_mut().insert(D, t2.plus_secs(-mins(19.0)));
        let rd = app.anti_afk_due(t2, &HashSet::from([D]), &wins, false, secs(3), D, &|| false);
        c.check(
            "the PC in use with a Roblox window in front, none of the input its own: not counted, its last chance nudged in a pause",
            rd == [D] && li(D).is_some_and(|d| d < t2.plus_secs(-mins(18.0))),
            format!("{} {:?}", list(&rd), li(D)),
        );
        let typed = t2.plus_secs(-1.0);
        *st.roblox_input_at.borrow_mut() = Some(Box::new(move |pid| if pid == D { Some(typed) } else { None }));
        st.last_input.borrow_mut().insert(D, t2.plus_secs(-mins(19.0)));
        let rd = app.anti_afk_due(t2, &HashSet::from([D]), &wins, false, secs(3), D, &|| false);
        c.check("…keys typed into it, or the mouse over it: counted, nothing nudged", rd.is_empty() && li(D) == Some(typed), li(D));
    }
    *st.roblox_input_at.borrow_mut() = None;
    // The clock set back half an hour (Windows' time synced, a wrong time zone fixed): times taken before it are half an
    // hour ahead, and a window idle 13 minutes then read as not idle at all until the clock caught up, so Roblox kicked it
    // first. Its input time and its wait before the next try move back with the clock; a log time ahead of now is passed
    // over (it can't be moved).
    {
        clear();
        let back = Duration::from_secs(30 * 60);
        let b = back.as_secs_f64();
        let t3 = Stamp::utc_now();
        st.last_input.borrow_mut().insert(B, t3.plus_secs(b - mins(13.0)));
        st.nudge_retry_at.borrow_mut().insert(C, t3.plus_secs(b + mins(1.0)));
        let mut tail = Tail::new("stand-in.log".into(), B);
        tail.state = Some(ClientState::new("ingame", Some(1818), Some("job".into()), None, t3.plus_secs(b - mins(13.0))));
        app.controller.logs.borrow_mut().tails.insert(B, tail);
        app.move_clock_back(t3, back);
        c.check("clock set back: the input time moves back with it", li(B) == Some(t3.plus_secs(-mins(13.0))), li(B));
        let retry_c = st.nudge_retry_at.borrow().get(&C).copied();
        c.check("…and the wait before the next try", retry_c == Some(t3.plus_secs(mins(1.0))), retry_c);
        let rc = app.anti_afk_due(t3, &HashSet::from([B]), &wins, false, secs(90), OTHER, &|| false);
        c.check("…so the window idle 13 minutes is nudged, its join logged before the change notwithstanding", rc == [B], list(&rc));
        app.controller.logs.borrow_mut().tails.remove(&B);
        clear();
    }
    c.check("a key reaches the window in front, wherever the pointer is", roblox_input::reached(true, D, OTHER) == D, ());
    c.check(
        "the mouse reaches it only with the pointer over it",
        roblox_input::reached(false, D, D) == D && roblox_input::reached(false, D, OTHER) == 0,
        (),
    );
    // Windows' raw input: a window of its own registered for the whole PC's input.
    c.skip("Windows' raw input set up, for the background too", "needs a window");
    c.skip("…and let go with no Roblox window", "needs a window");
    c.skip("…taken again when one opens", "needs a window");

    unseen(c);

    // A nudge due while a macro's turn has the PC waits for it; an idle kick meanwhile is told as the macro's doing. The
    // window is none (0) and counts as played in before the turn is handed on: nothing is ever nudged.
    input_turn::reset();
    clear();
    const E: i32 = 1005;
    st.last_input.borrow_mut().insert(E, Stamp::utc_now().plus_secs(-mins(19.0)));
    let macro_turn = input_turn::try_enter("macro").expect("the macro's turn");
    let done = Rc::new(Cell::new(false));
    {
        let (app, done) = (app.clone(), done.clone());
        rt::spawn(async move {
            app.nudge(vec![(E, 0)]).await;
            done.set(true);
        });
    }
    // The nudge's first steps, up to its wait for the turn.
    let begun = Instant::now();
    while held(E) != "MacroBusy" && begun.elapsed() < Duration::from_secs(2) {
        rt::sleep(Duration::from_millis(10)).await;
    }
    c.check("a nudge waiting behind a macro's turn notes why (an idle kick then says so)", held(E) == "MacroBusy", held(E));
    st.last_input.borrow_mut().insert(E, Stamp::utc_now().plus_secs(60.0));
    macro_turn.dispose();
    let begun = Instant::now();
    while !done.get() && begun.elapsed() < Duration::from_secs(5) {
        rt::sleep(Duration::from_millis(20)).await;
    }
    c.check("…and once the turn is handed on, nothing left to nudge", done.get() && !input_turn::busy(), input_turn::holder());
    app.controller.nudging.set(true);
    input_turn::reset();
}

/// The nudge's parts on windows of the test's own: only what needs no window is checked here.
fn unseen(c: &mut Checks) {
    c.check(
        "INPUT is the size SendInput expects (40 bytes in a 64-bit process)",
        anti_afk::input_size() == if cfg!(target_pointer_width = "64") { 40 } else { 28 },
        anti_afk::input_size(),
    );
    for what in [
        "hidden: layered and click-through",
        "hidden: all but invisible",
        "shown again: its own style back",
        "own opacity: hidden all the same",
        "own opacity: back to its own, still layered, not click-through",
        "its place: under the shown window that was above it",
        "put back under the same window",
        "first of the usual windows: goes under none of the always-on-top ones",
        "recovered: neither layered nor click-through",
        "another app's faded window untouched",
        "turned off: no window reached, its style untouched",
    ] {
        c.skip(what, "needs a window");
    }
    // A nudge that touches nothing: no window that answers. Nothing pressed, nothing changed.
    let pressed = Cell::new(0);
    let press = |_h: isize| pressed.set(pressed.get() + 1);
    let dead: isize = 0x7FFF_FFF0;
    // SAFETY: IsWindow only reads.
    if unsafe { windows_sys::Win32::UI::WindowsAndMessaging::IsWindow(dead as _) } == 0 {
        let none = anti_afk::nudge(&[dead], &|| true, Some(&press));
        c.check("a window that's gone: skipped", none.is_empty(), ());
    }
    c.check("no key pressed", pressed.get() == 0, pressed.get());
}
