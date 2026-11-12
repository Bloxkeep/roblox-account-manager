// Tests/Bloxkeep.Tests/WatchSuite.cs: what Bloxkeep takes from the Roblox watch's status file (roblox_watch): verdicts and
// lock names, with anything out of shape left out; the names it holds besides its own; and Multi-Roblox holding several
// names, made up for the test (its own name is only opened, which an open Roblox or Bloxkeep doesn't notice).
use std::collections::HashMap;

use super::{unlock, window::quiet_app as app, Checks, PASSWORD};
use crate::core::{
    multi_instance, prefs,
    roblox_installs,
    roblox_watch::{self, Build, Status},
    vault::Vault,
    win_kit::wide,
};

fn status(builds: Vec<Build>) -> Status {
    Status { live: None, next: None, by_hash: builds.into_iter().map(|b| (b.hash.to_uppercase(), b)).collect::<HashMap<_, _>>() }
}

pub async fn run(c: &mut Checks, _dir: &str) {
    launch_warning(c).await;
    // Roblox from the Microsoft Store: its exe has no version, its package folder does.
    c.check(
        "the Store client's version, in Roblox's form",
        roblox_installs::store_version("ROBLOXCorporation.RobloxGDK_2.743.1134.0_x64__55nm5eh3cm0pr").as_deref() == Some("0.743.0.7431134")
            && roblox_installs::store_version("ROBLOXCorporation.RobloxGDK_2.744.84.0_x64__55nm5eh3cm0pr").as_deref() == Some("0.744.0.7440084")
            && roblox_installs::store_version("RobloxPlayerBeta").is_none(),
        (),
    );
    let too_long = format!("ROBLOX_{}", "x".repeat(70));
    let json = format!(
        r#"{{
  "schema": 1,
  "live": {{ "version": "0.743.0.7431129", "hash": "version-aaaa" }},
  "beta": {{ "version": "0.743.0.7431134", "hash": "version-bbbb" }},
  "builds": [
    {{ "version": "0.742.0.7421053", "hash": "version-old", "verdict": "first", "locks": ["ROBLOX_singletonEvent"] }},
    {{ "version": "0.743.0.7431129", "hash": "version-aaaa", "verdict": "renamed",
      "hold": ["ROBLOX_singletonEvent2", "not a name", "Global\\Evil", "{too_long}", "ROBLOX_a", "ROBLOX_b", "ROBLOX_c", "ROBLOX_d"] }},
    {{ "version": "0.743.0.7431134", "hash": "version-bbbb", "verdict": "same", "hold": ["ROBLOX_singletonEvent"] }},
    {{ "version": "0.744.0.1", "hash": "version-bad", "verdict": "exploded" }},
    "junk",
    {{ "hash": "version-noversion", "verdict": "same" }}
  ]
}}"#
    );
    let s = roblox_watch::parse(&json);
    c.check("the status file is read", s.is_some(), ());
    let Some(s) = s else { return };
    let mut keys: Vec<String> = s.by_hash.values().map(|b| b.hash.clone()).collect();
    keys.sort();
    c.check("builds out of shape are left out", keys == ["version-aaaa", "version-bbbb", "version-old"], keys.join(","));
    let hold = s.get("version-aaaa").and_then(|b| b.hold.clone()).unwrap_or_default();
    c.check("only lock-shaped names are taken, at most 4", hold == ["ROBLOX_singletonEvent2", "ROBLOX_a", "ROBLOX_b", "ROBLOX_c"], hold.join(","));
    c.check("no hold list: none taken", s.get("version-old").is_some_and(|b| b.hold.is_none()), ());
    c.check(
        "live and next builds found",
        s.live.as_ref().map(|b| b.hash.as_str()) == Some("version-aaaa") && s.next.as_ref().map(|b| b.hash.as_str()) == Some("version-bbbb"),
        (),
    );
    let older_beta = json.replace(
        "\"0.743.0.7431134\", \"hash\": \"version-bbbb\", \"verdict\"",
        "\"0.743.0.7431100\", \"hash\": \"version-bbbb\", \"verdict\"",
    );
    c.check(
        "a beta build that isn't newer isn't the next one",
        roblox_watch::parse(&older_beta).is_some_and(|s| s.next.is_none()),
        (),
    );
    c.check(
        "not a status file: nothing",
        roblox_watch::parse("<html>404</html>").is_none() && roblox_watch::parse("{}").is_none() && roblox_watch::parse("[]").is_none(),
        (),
    );

    roblox_watch::set_current(Some(s.clone()));
    let own = multi_instance::EVENT_NAME;
    let extra = roblox_watch::extra_names(Some("version-bbbb"), own);
    c.check(
        "names to hold: the live build's, its own left out",
        extra == ["ROBLOX_singletonEvent2", "ROBLOX_a", "ROBLOX_b", "ROBLOX_c"],
        extra.join(","),
    );
    c.check(
        "a build the watch knows: its verdict",
        roblox_watch::for_hash(Some("VERSION-AAAA")).is_some_and(|b| b.verdict == "renamed") && roblox_watch::for_hash(Some("version-zzzz")).is_none(),
        (),
    );
    let plain = Build { version: "0.743.0.7431129".into(), hash: "version-aaaa".into(), verdict: "same".into(), hold: Some(vec![own.to_string()]) };
    let mut only_plain = status(vec![plain.clone()]);
    only_plain.live = Some(plain);
    roblox_watch::set_current(Some(only_plain));
    let extra = roblox_watch::extra_names(Some("version-old"), own);
    c.check("no renamed lock anywhere: nothing more to hold", extra.is_empty(), extra.join(","));
    roblox_watch::set_current(None);

    // Holding several names: made-up ones next to Multi-Roblox's own.
    let pid = std::process::id();
    let (a, b) = (format!("ROBLOX_bloxkeepTestA_{pid}"), format!("ROBLOX_bloxkeepTestB_{pid}"));
    let exists = |name: &str| {
        let n = wide(name);
        // SAFETY: plain calls; the handle is closed at once.
        unsafe {
            let h = windows_sys::Win32::System::Threading::OpenMutexW(0x0010_0000, 0, n.as_ptr());
            if h.is_null() {
                false
            } else {
                windows_sys::Win32::Foundation::CloseHandle(h);
                true
            }
        }
    };
    multi_instance::set_extra_names(&[a.clone(), b.clone(), own.to_string()]);
    multi_instance::enable();
    if !multi_instance::armed() {
        // A Roblox client is open here: its own name is its event until it closes, so nothing can be held.
        c.check("several names held (skipped: a Roblox client is open)", true, ());
        multi_instance::disable();
        return;
    }
    let held = multi_instance::extra_held();
    c.check("every name held", exists(&a) && exists(&b) && held == [a.clone(), b.clone()], held.join(","));
    c.check("same names: not stale", !multi_instance::stale(), ());
    multi_instance::set_extra_names(std::slice::from_ref(&a));
    c.check("other names wanted: stale until taken again", multi_instance::stale(), ());
    multi_instance::enable();
    c.check("taken again: only the wanted ones", !multi_instance::stale() && exists(&a) && !exists(&b), ());
    multi_instance::disable();
    c.check(
        "off: every name let go",
        !multi_instance::armed() && !exists(&a) && multi_instance::extra_held().is_empty(),
        (),
    );
    multi_instance::set_extra_names(&[]);
}

/// The warning before opening several windows, for the Roblox build installed here (skipped without one): said for a
/// changed, renamed or gone lock, not for no change, and not once two windows played together on that build.
async fn launch_warning(c: &mut Checks) {
    Vault::create(PASSWORD, None).expect("vault");
    let (app, _said) = app();
    unlock(&app).await;
    let Some(installed) = app.installs().client.filter(|cl| !cl.folder.is_empty()) else {
        c.check("launch warning (skipped: no Roblox installed here)", true, ());
        return;
    };
    let warning = |verdict: &str| {
        roblox_watch::set_current(Some(status(vec![Build {
            version: installed.version.clone(),
            hash: installed.folder.clone(),
            verdict: verdict.into(),
            hold: None,
        }])));
        app.watch_warning()
    };
    c.check(
        "launch warning: changed",
        warning("changed").is_some_and(|w| w.contains(&installed.version)),
        warning("changed"),
    );
    c.check("launch warning: renamed and gone too", warning("renamed").is_some() && warning("gone").is_some(), ());
    c.check("no warning for no change", warning("same").is_none() && warning("first").is_none(), ());
    prefs::lock().multi_worked.push(installed.folder.clone());
    c.check("no warning once two windows played together on that build", warning("changed").is_none(), warning("changed"));
    prefs::lock().multi_worked.clear();
    roblox_watch::set_current(None);
    c.check("no warning without the watch's word", app.watch_warning().is_none(), ());
}

/// The real status file, from the repository's roblox-status branch: all of it needs the network.
pub async fn run_online(c: &mut Checks, _dir: &str) {
    c.skip("the status file is read from GitHub", "needs the network");
    c.skip("it knows the live build", "needs the network");
    c.skip("read again: nothing changed", "needs the network");
}
