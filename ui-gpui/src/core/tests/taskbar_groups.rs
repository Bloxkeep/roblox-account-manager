// Tests/Bloxkeep.Tests/TaskbarGroupsSuite.cs: Settings › Taskbar buttons (TaskbarGroups) on a window of the test's own.
// Every check reads the group id back from a real window (made by the test), so none of them runs here.
use super::Checks;

pub async fn run(c: &mut Checks, _dir: &str) {
    for what in [
        "a window of its own program has no group id of its own",
        "an account's group is put on its window",
        "…moved when the grouping changes",
        "turned off: the window's own grouping back",
        "left by a crash, the same group found again: turned off, the window's own grouping back",
        "…another group: the leftover isn't taken for the window's own",
        "…the setting off: taken off at the first look at the windows",
    ] {
        c.skip(what, "needs a window");
    }
}
