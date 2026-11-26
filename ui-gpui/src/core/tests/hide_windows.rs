// Hiding a window while its program runs (roblox_processes::hide_windows), on a window of the test's own standing in for
// a Roblox window (no Roblox is touched): it goes from the screen but still counts as the client's window; Show brings it
// back; the windows hidden are written down, and a run that ended without showing them (a crash) has them shown again at
// the next start. Then the page's Show and the Monitor tab's Minimise and Restore while a nudge, a picture or a macro
// has the window (input_turn): they wait for the turn to end, as the turn puts the window back as it found it at its
// start. Checked on a desktop of their own that is never shown (hidden_desktop.rs); anti-AFK's key is a stand-in there,
// and nothing is typed.
use std::cell::Cell;

use serde_json::json;
use windows_sys::Win32::UI::WindowsAndMessaging::{SW_HIDE, SW_RESTORE, SW_SHOWMINNOACTIVE, SW_SHOWNA, SW_SHOWNOACTIVATE, WS_OVERLAPPEDWINDOW};

use super::{
    Checks, PASSWORD, call,
    hidden_desktop::{self as hd, Rec, iconic, make_windows, settle, show, visible, wait_for},
    rejoin::{no_network, quiet_app, seed},
};
use crate::core::{
    anti_afk,
    app::monitor::monitor_id,
    base::Stamp,
    input_turn,
    roblox_processes::{self as rp, PlayerProcess},
    vault,
};

pub async fn run(c: &mut Checks, dir: &str) {
    if hd::in_view() {
        hd::run_there(c, dir, "hide-windows", 120_000);
        return;
    }
    let mut r = Rec::new(c);
    checks(&mut r).await;
    r.save(dir);
}

async fn checks(r: &mut Rec<'_>) {
    let me = std::process::id() as i32;
    let ws = make_windows(1, WS_OVERLAPPEDWINDOW);
    let w = ws.all[0];
    // The one window of this process titled as Roblox's: the one taken for the client's window.
    hd::set_title(w, "Roblox hide check");
    settle();
    let file = vault::data_dir().join("hidden-windows.txt");
    let listed = || rp::visible_windows(&[me]).get(&me).is_some_and(|l| l.contains(&w));

    r.check("a shown window to start with", visible(w) && listed(), (visible(w), rp::visible_windows(&[me]), w));
    let hid = rp::hide_windows(&[me]);
    settle();
    r.check("hidden: gone from the screen", hid == 1 && !visible(w), (hid, visible(w)));
    r.check("…still counted as the program's window (never taken for a windowless copy, closed gracefully)", listed() && rp::has_hidden(me), (listed(), rp::has_hidden(me)));
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    r.check("…written down, so a crash doesn't leave it hidden", text.contains(&format!("{me} {}", w as i64)), &text);
    r.check("hiding it again does nothing", rp::hide_windows(&[me]) == 0, ());

    // Anti-AFK on a hidden window: shown unseen for the moment of its key (a stand-in here, nothing typed), then hidden
    // again. (Here the window can't be given the keyboard, so the key's moment never comes: what is left to see is
    // that it is hidden again after.)
    let shown_for_key = Cell::new(false);
    let reached = anti_afk::nudge(&[w], &|| true, Some(&|_| shown_for_key.set(visible(w))));
    settle();
    r.check(
        "anti-AFK on a hidden window: shown for its key, hidden again after",
        (!reached.contains(&w) || shown_for_key.get()) && !visible(w) && rp::has_hidden(me),
        format!("reached {}, shown {}, visible after {}, hidden {}", reached.len(), shown_for_key.get(), visible(w), rp::has_hidden(me)),
    );

    // Hidden while minimised: the nudge opens it, and it must not go back to the screen minimised.
    rp::show_windows(Some(&[me]));
    settle();
    show(w, SW_SHOWMINNOACTIVE);
    settle();
    let hid_min = rp::hide_windows(&[me]);
    settle();
    let before = format!("hid {hid_min}, visible after hide {}, iconic {}", visible(w), iconic(w));
    let reached = anti_afk::nudge(&[w], &|| true, Some(&|_| {}));
    settle();
    r.check(
        "anti-AFK on a hidden, minimized window: hidden again after, not back on the taskbar",
        !visible(w) && rp::has_hidden(me),
        format!("{before}; visible {}, iconic {}, reached {}", visible(w), iconic(w), reached.len()),
    );
    rp::show_windows(Some(&[me]));
    settle();
    show(w, SW_RESTORE);
    settle();
    rp::hide_windows(&[me]);
    settle();

    let shown = rp::show_windows(Some(&[me]));
    settle();
    r.check("shown again", shown == 1 && visible(w) && !rp::has_hidden(me), (shown, visible(w), rp::has_hidden(me)));
    r.check("…and no longer written down", !file.exists(), ());

    // A run that ended without showing it: Bloxkeep's memory of it is gone, the file it wrote is left.
    rp::hide_windows(&[me]);
    settle();
    let left = std::fs::read_to_string(&file).unwrap_or_default();
    let forgot = rp::forget_hidden(me);
    let rewritten = std::fs::write(&file, &left).is_ok();
    r.check(
        "crashed with it hidden: hidden, and no longer known",
        forgot && rewritten && !left.is_empty() && !visible(w) && !rp::has_hidden(me),
        (forgot, &left, visible(w), rp::has_hidden(me)),
    );
    let back = rp::show_left_hidden();
    settle();
    r.check("the next start shows it again from the file", back == 1 && visible(w) && !file.exists(), (back, visible(w), file.exists()));

    // A client that ended hidden: known to have been (its rejoin's window is hidden too), then forgotten.
    rp::hide_windows(&[me]);
    settle();
    r.check("a client that ended with its window hidden is said so", rp::forget_hidden(me) && !rp::has_hidden(me) && !file.exists(), file.exists());
    r.check("…once", !rp::forget_hidden(me), ());
    show(w, SW_SHOWNA);
    settle();

    // A light-AFK preset's minimise and Settings' Maximise leave a hidden window hidden (they would show it, still known
    // as hidden); Show (Mini window, tray, Monitor) shows it again, known as shown.
    rp::hide_windows(&[me]);
    settle();
    let (minimised, maximised) = (rp::minimise(w), rp::maximise(w));
    settle();
    r.check(
        "minimised or maximised by Bloxkeep, a hidden window stays hidden",
        !visible(w) && rp::has_hidden(me),
        format!("asked {minimised}/{maximised}, visible {}, iconic {}", visible(w), iconic(w)),
    );
    rp::focus(me);
    settle();
    r.check(
        "Show on a hidden window shows it, no longer hidden",
        visible(w) && !rp::has_hidden(me) && !file.exists(),
        format!("visible {}, hidden {}", visible(w), rp::has_hidden(me)),
    );

    in_turns(r, w).await;
}

/// The page's Show and the Monitor tab's Minimise and Restore while a nudge, a picture or a macro has the window (a
/// stand-in turn here, putting the window back as it found it at its start, as real turns do): they wait for the turn
/// to end. Done during it, the turn undid them: a window shown from the page went unseen again while Bloxkeep counted it
/// as shown.
async fn in_turns(r: &mut Rec<'_>, w: isize) {
    let me = std::process::id() as i32;
    no_network();
    seed(Vec::new());
    let (app, _said) = quiet_app();
    let unlocked = call(&app, "unlock", json!({ "password": PASSWORD })).await;
    r.check("the vault unlocked for the turn checks", unlocked.is_ok(), unlocked.as_ref().err().map(|e| e.to_string()));
    if unlocked.is_err() {
        return;
    }
    input_turn::reset();
    let until = |done: &dyn Fn() -> bool| {
        let _ = wait_for(2000, done);
    };

    // Hidden; a turn shows it unseen for its key and hides it again at its end, as it found it.
    rp::hide_windows(&[me]);
    settle();
    let turn = input_turn::try_enter("macro");
    let had_turn = turn.is_some();
    let unshown = !visible(w);
    show(w, SW_SHOWNOACTIVATE);
    settle();
    let answer = call(&app, "showWindows", json!({ "ids": [] })).await;
    settle();
    if unshown {
        show(w, SW_HIDE);
    }
    until(&|| !visible(w));
    drop(turn);
    until(&|| visible(w));
    r.check(
        "Show from the page during a turn: shown once the turn ends, not left unseen while counted as shown",
        had_turn && answer.is_ok() && visible(w) && !rp::is_hidden(w),
        format!("turn {had_turn}, answered {:?}, visible {}, hidden {}", answer.as_ref().map_err(|e| e.to_string()), visible(w), rp::is_hidden(w)),
    );
    rp::show_windows(Some(&[me]));
    settle();

    // The Monitor tab's tiles are this process's windows: a snapshot of the clients that has it, as the process check
    // leaves one (fresh for each action, as the tab's are).
    let act = |what: &str| {
        *app.processes.last_players.borrow_mut() = Some((vec![PlayerProcess { pid: me, ..PlayerProcess::default() }], Stamp::utc_now()));
        let tile = app.monitor_clients().iter().any(|c| c.hwnd == w);
        (tile, app.monitor_act_command(&json!({ "id": monitor_id(w), "what": what })).map_err(|e| e.to_string()))
    };

    // Shown; a turn brings it forward, and puts it back as it found it (not minimised) at its end.
    let turn = input_turn::try_enter("macro");
    let had_turn = turn.is_some();
    let was_minimised = iconic(w);
    let asked = act("minimize");
    settle();
    if !was_minimised {
        show(w, SW_RESTORE);
    }
    until(&|| !iconic(w));
    drop(turn);
    until(&|| iconic(w));
    r.check(
        "Minimise on the Monitor tab during a turn: minimised once the turn ends, not undone by it",
        had_turn && asked.0 && iconic(w),
        format!("turn {had_turn}, tile and answer {asked:?}, iconic {}", iconic(w)),
    );

    // Minimised; a turn opens it for its key and minimises it again at its end.
    show(w, SW_SHOWMINNOACTIVE);
    until(&|| iconic(w));
    let turn = input_turn::try_enter("macro");
    let had_turn = turn.is_some();
    let was_minimised = iconic(w);
    show(w, SW_RESTORE);
    settle();
    let asked = act("restore");
    settle();
    if was_minimised {
        show(w, SW_SHOWMINNOACTIVE);
    }
    until(&|| iconic(w)); // as a turn waits for its window to be minimised again
    drop(turn);
    until(&|| !iconic(w));
    r.check(
        "Restore on the Monitor tab during a turn: open once the turn ends, not minimised again by it",
        had_turn && asked.0 && was_minimised && !iconic(w),
        format!("turn {had_turn}, tile and answer {asked:?}, minimised before {was_minimised}, iconic {}", iconic(w)),
    );
    input_turn::reset();
}
