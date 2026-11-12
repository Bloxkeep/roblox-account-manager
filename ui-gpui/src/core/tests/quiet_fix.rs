// Tests/Bloxkeep.Tests/QuietFixSuite.cs: quiet fixes (whats_new::is_quiet_fix, release-fix.ps1): small fixes after a
// public version, numbered 1.0.7-alpha.N after 1.0.6. Velopack's own comparison puts them above that version and below
// the next one's betas and the next version itself; the app shows the version they fix, and What's new treats them as
// coming before the version they lead to. Then the hand-over a start by hand waits for during an update.
//
// PORT: Velopack's SemanticVersion is the velopack crate's (semver's Version), reached through its Manifest's field.
use super::Checks;
use crate::core::base::Stamp;
use crate::core::{relaunch, vault, whats_new};

/// SemanticVersion.Parse, as the velopack crate reads a version.
fn v(text: &str) -> impl PartialOrd {
    let mut m = velopack::bundle::Manifest::default();
    m.version = text.parse().expect("a version");
    m.version
}

pub async fn run(c: &mut Checks, _dir: &str) {
    c.check("Velopack takes a quiet fix over the version it fixes", v("1.0.7-alpha.1") > v("1.0.6"), ());
    c.check(
        "and a later quiet fix over an earlier one",
        v("1.0.7-alpha.2") > v("1.0.7-alpha.1") && v("1.0.7-alpha.10") > v("1.0.7-alpha.9"),
        (),
    );
    c.check("a beta of the next version stays above every quiet fix (beta copies don't go back)", v("1.0.7-beta.1") > v("1.0.7-alpha.99"), ());
    c.check("the next version stays above every quiet fix", v("1.0.7") > v("1.0.7-alpha.99"), ());

    let fix = |s: Option<&str>| whats_new::is_quiet_fix(s);
    c.check(
        "a quiet fix is told apart from betas and versions",
        fix(Some("1.0.7-alpha.2")) && fix(Some("1.0.7-alpha.2+abc123")) && !fix(Some("1.0.7-beta.1")) && !fix(Some("1.0.6")) && !fix(None),
        (),
    );
    c.check("a quiet fix shows as the version it fixes", whats_new::shown("1.0.7-alpha.2") == "1.0.6", whats_new::shown("1.0.7-alpha.2"));
    c.check(
        "a version shows as itself",
        whats_new::shown("1.0.6") == "1.0.6" && whats_new::shown("1.0.7-beta.1") == "1.0.7-beta.1",
        (),
    );

    let older = whats_new::older;
    c.check(
        "versions in order: the fixed version, its quiet fixes, the next version",
        older("1.0.6", "1.0.7-alpha.1") && older("1.0.7-alpha.1", "1.0.7-alpha.2") && older("1.0.7-alpha.2", "1.0.7") && !older("1.0.7", "1.0.7-alpha.2"),
        (),
    );
    c.check(
        "a test build between two fixes counts as the first: the next fix and the next version are newer",
        older("1.0.8-alpha.3.1", "1.0.8-alpha.4") && older("1.0.8-alpha.3.1", "1.0.8") && !older("1.0.8-alpha.3", "1.0.8-alpha.3.1"),
        (),
    );
    c.check(
        "a beta comes before its version, an earlier beta before a later one, and a quiet fix before the betas (fails on the old code)",
        older("1.0.6-beta.8", "1.0.6")
            && !older("1.0.6", "1.0.6-beta.8")
            && older("1.0.6-beta.2", "1.0.6-beta.10")
            && older("1.0.7-alpha.3", "1.0.7-beta.1")
            && !older("1.0.7-beta.1", "1.0.7-alpha.3"),
        (),
    );
    let quiet = |a: &str, b: &str| whats_new::is_quiet_update(Some(a), Some(b));
    c.check(
        "a beta copy's next beta comes in quietly (no Restart to update), as quiet fixes do",
        quiet("1.0.8-beta.5", "1.0.8-beta.4") && quiet("1.0.8-alpha.4", "1.0.8"),
        (),
    );
    c.check(
        "…but not a regular copy's first beta, nor the regular version after the betas, nor the next version's beta",
        !quiet("1.0.8-beta.1", "1.0.8-alpha.3") && !quiet("1.0.8", "1.0.8-beta.5") && !quiet("1.0.9-beta.1", "1.0.8-beta.5"),
        (),
    );

    let versions = |after: Option<&str>, up_to: &str| -> Vec<String> { whats_new::between(after, up_to).into_iter().map(|r| r.version).collect() };
    c.check("updated to a quiet fix: no notes to show", versions(Some("1.0.6"), "1.0.7-alpha.1").is_empty(), versions(Some("1.0.6"), "1.0.7-alpha.1"));
    c.check(
        "from an older version to a quiet fix: the public versions in between",
        versions(Some("1.0.5"), "1.0.7-alpha.1") == ["1.0.6"],
        versions(Some("1.0.5"), "1.0.7-alpha.1"),
    );
    c.check(
        "from a quiet fix to the version it leads to: that version's notes",
        versions(Some("1.0.6-alpha.3"), "1.0.6") == ["1.0.6"],
        versions(Some("1.0.6-alpha.3"), "1.0.6"),
    );
    c.check("all the notes, as Settings' What's new lists them", versions(None, "1.0.6").first().map(String::as_str) == Some("1.0.6"), ());
    let shown = |last: &str, current: &str| -> Vec<String> { whats_new::after_update(last, current).into_iter().map(|r| r.version).collect() };
    c.check(
        "from a beta to its version: that version's notes, once (fails on the old code: none)",
        shown("1.0.10-beta.3", "1.0.10") == ["1.0.10"],
        shown("1.0.10-beta.3", "1.0.10"),
    );
    c.check("…and from the version before, the same", shown("1.0.9", "1.0.10") == ["1.0.10"], shown("1.0.9", "1.0.10"));
    c.check("a beta copy's next beta: no notes (it comes in quietly)", shown("1.0.10-beta.1", "1.0.10-beta.3").is_empty(), ());
    c.check(
        "from a beta to a quiet fix after its version: that version's notes (a copy still noted as a beta)",
        shown("1.0.10-beta.3", "1.0.11-alpha.1") == ["1.0.10"],
        shown("1.0.10-beta.3", "1.0.11-alpha.1"),
    );
    c.check("the same version again, or an older one: nothing", shown("1.0.10", "1.0.10").is_empty() && shown("1.0.10", "1.0.9").is_empty(), ());

    // Bloxkeep clicked open while an update installs (2026-10-20, 8:43): that start waits for the update's run, which
    // carries the unlocked vault over, rather than taking its place. It knows by the fresh hand-over on disk.
    let hand_over = vault::data_dir().join("resume.bin");
    c.check("no update under way: a start goes ahead", !relaunch::hand_over_waiting(), ());
    let _ = relaunch::write(&relaunch::State {
        written_at: Stamp::utc_now(),
        to: Some("1.0.7-alpha.3".into()),
        window: "minimized".into(),
        ..Default::default()
    });
    c.check("an update handing over right now: a start by hand waits for its run", relaunch::hand_over_waiting(), ());
    let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    c.check(
        "the update's own run (and the watch's) doesn't wait for itself",
        relaunch::is_update_run(&args(&["--resume", "ab12"])) && relaunch::is_update_run(&args(&["--after-update", "1.0.7-alpha.3"])) && !relaunch::is_update_run(&[]),
        (),
    );
    if let Ok(f) = std::fs::OpenOptions::new().write(true).open(&hand_over) {
        let _ = f.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(600));
    }
    c.check("a hand-over left from long ago doesn't hold a start back", !relaunch::hand_over_waiting(), ());
    let _ = std::fs::remove_file(&hand_over);
}
