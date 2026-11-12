// Tests/Bloxkeep.Tests/DialogSuite.cs: the app's message box (forms/dialog.rs). What its fallback is given of the wait is
// checked here; the box itself is a page in a window of its own (WebView2), shown and answered: none of that runs here.
use std::time::Duration;

use super::Checks;
use crate::core::forms::dialog;

pub async fn run(c: &mut Checks, _dir: &str) {
    // The uninstall question's fallback (Windows' own dialog, after the page failed to draw in 6 s) gets what is left of the
    // wait, not all of it again: Velopack allows the step 30 s in all.
    let six = Duration::from_secs(6);
    c.check(
        "the fallback gets the timeout less the time the page took",
        dialog::remaining(Some(Duration::from_secs(20)), six) == Some(Duration::from_secs(14)),
        dialog::remaining(Some(Duration::from_secs(20)), six),
    );
    c.check(
        "…at least a few seconds, however long that took",
        dialog::remaining(Some(Duration::from_secs(5)), six) == Some(Duration::from_secs(3)),
        dialog::remaining(Some(Duration::from_secs(5)), six),
    );
    c.check("…and no timeout stays none", dialog::remaining(None, six).is_none(), ());

    for what in [
        "the box shows once its page has drawn",
        "a click on the page answers, with Don't ask again",
        "sized to its page (460 CSS px wide, as tall as its text) and centred on the screen",
        "Esc or ✕ picks the cancel choice",
        "the timeout picks the cancel one",
        "Confirm: yes",
        "no box left open",
    ] {
        c.skip(what, "needs a window");
    }
}
