// Tests/Bloxkeep.Tests/HideWindowsSuite.cs: hiding a window while its program runs (roblox_processes::hide_windows), shown
// again, written down for a crash, and Show / Minimise / Restore while a turn has the window. Every check there works on a
// real window of the test's own (shown, hidden, minimised, nudged): none of it runs here.
use super::Checks;

pub async fn run(c: &mut Checks, _dir: &str) {
    for what in [
        "a shown window to start with",
        "hidden: gone from the screen",
        "…still counted as the program's window (never taken for a windowless copy, closed gracefully)",
        "…written down, so a crash doesn't leave it hidden",
        "hiding it again does nothing",
        "anti-AFK on a hidden window: shown for its key, hidden again after",
        "anti-AFK on a hidden, minimized window: hidden again after, not back on the taskbar",
        "shown again",
        "…and no longer written down",
        "crashed with it hidden: hidden, and no longer known",
        "the next start shows it again from the file",
        "a client that ended with its window hidden is said so",
        "…once",
        "minimised or maximised by Bloxkeep, a hidden window stays hidden",
        "Show on a hidden window shows it, no longer hidden",
        "the vault unlocked for the turn checks",
        "Show from the page during a turn: shown once the turn ends, not left unseen while counted as shown",
        "Minimise on the Monitor tab during a turn: minimised once the turn ends, not undone by it",
        "Restore on the Monitor tab during a turn: open once the turn ends, not minimised again by it",
    ] {
        c.skip(what, "needs a window");
    }
}
