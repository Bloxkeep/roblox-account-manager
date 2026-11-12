// Tests/Bloxkeep.Tests/ShutdownSuite.cs: quitting (App::dispose), on a stand-in Roblox folder (never the real one): the
// last change still reaches the vault on disk and the vault is closed; a Light AFK file that can't be put back at quit is
// kept for the next start; quitting is quick and a second time does nothing.
//
// PORT: a step that fails can't be planted (the C# set the Discord presence to null so its step threw; the port's steps
// return Res and a panic isn't caught), so that check is skipped. Quitting also makes visible again any Roblox window
// left unseen by a cut-short nudge (anti_afk::recover, on the PC's real Roblox windows): with Roblox running on this PC
// the quitting checks are skipped rather than let it touch them.
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::macros::{FakeDesk, is, lk, mk, queue, run_at, setup};
use super::rejoin::quiet_app;
use super::{Checks, PASSWORD, unlock};
use crate::core::models::Account;
use crate::core::vault::Vault;
use crate::core::{input_turn, prefs, roblox_flags, roblox_processes, rt};

fn read_only(path: &Path, on: bool) {
    if let Ok(meta) = std::fs::metadata(path) {
        let mut p = meta.permissions();
        p.set_readonly(on);
        let _ = std::fs::set_permissions(path, p);
    }
}

const QUIT_CHECKS: [&str; 6] = [
    "…quitting takes a few seconds at most",
    "…the last change is in the vault on disk",
    "…the vault is closed",
    "a Light AFK file that couldn't be put back is kept for the next start",
    "a macro holding a key when quitting lets it go, its run stopped",
    "quitting a second time does nothing",
];

pub async fn run(c: &mut Checks, dir: &str) {
    c.skip("a failing step doesn't stop quitting", "a throwing step can't be planted: the port's steps return Res");
    let running = roblox_processes::get_player_pids();
    if !running.is_empty() {
        for what in QUIT_CHECKS {
            c.skip(what, "quitting acts on the PC's real Roblox windows, and Roblox is running");
        }
        c.skip("…and the next start puts the person's file back", "follows the quit above");
        return;
    }

    prefs::lock().count_online = false; // quitting never tells the bot this PC went offline
    Vault::create(PASSWORD, None).expect("a vault").save().expect("saved");
    let (app, _said) = quiet_app();
    unlock(&app).await;
    let vault = app.vault_ref().expect("open");

    // A change made just before quitting, not saved yet.
    vault.data().accounts.push(Account { username: "quit_user".into(), alias: "changed before quitting".into(), ..Default::default() });
    app.persist_soon();

    // Light AFK's flags written for a window, the file then held read-only: the restore at quit can't write it.
    let build = Path::new(dir).join("Roblox").join("Versions").join("version-aaaa");
    std::fs::create_dir_all(&build).expect("a build");
    std::fs::write(build.join("RobloxPlayerBeta.exe"), "").expect("a client");
    let flags = build.join("ClientSettings").join("ClientAppSettings.json");
    std::fs::create_dir_all(flags.parent().unwrap()).expect("a folder");
    const OWN: &str = r#"{"FFlagMyOwn": true}"#;
    std::fs::write(&flags, OWN).expect("written");
    roblox_flags::apply(Some(&build.join("RobloxPlayerBeta.exe").to_string_lossy()));
    app.controller.roblox_flags_pending.set(true);
    read_only(&flags, true);

    // A macro holding W when quitting (on the made-up desktop: nothing is sent).
    let (player, desk, accounts) = setup(1);
    *lk(&desk.suspend_when) = Some(Arc::new(|d: &FakeDesk| d.any("key W down")));
    queue(&player, &mk(r#"[{"kind":"down","key":"W"},{"kind":"wait","ms":20000},{"kind":"up","key":"W"}]"#, ""), &accounts);
    let begun = Instant::now();
    while !desk.any("key W down") && begun.elapsed() < Duration::from_secs(5) {
        rt::sleep(Duration::from_millis(10)).await;
    }
    *app.macros.macro_player.borrow_mut() = Some(player.clone());

    let clock = Instant::now();
    app.dispose();
    let took = clock.elapsed();
    c.check("…quitting takes a few seconds at most", took < Duration::from_secs(4), took);

    drop(vault);
    let alias = || match Vault::open(PASSWORD, None) {
        Ok(opened) => {
            let a = opened.data().accounts.iter().find(|a| a.username == "quit_user").map(|a| a.alias.clone()).unwrap_or_else(|| "(not on disk)".into());
            opened.dispose();
            a
        }
        Err(e) => format!("not opened: {e:?}"),
    };
    c.check("…the last change is in the vault on disk", alias() == "changed before quitting", alias());
    c.check("…the vault is closed", app.vault_ref().is_none(), ());
    c.check("a Light AFK file that couldn't be put back is kept for the next start", roblox_flags::pending(), ());
    // The run is stopped by release_all on this thread; its own thread notes it a moment later.
    let begun = Instant::now();
    while !is(&run_at(&player, 0), "stopped", "closing") && begun.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(10));
    }
    c.check(
        "a macro holding a key when quitting lets it go, its run stopped",
        desk.any("key W up") && is(&run_at(&player, 0), "stopped", "closing"),
        desk.log().join(" | "),
    );
    input_turn::reset();

    let again = Instant::now();
    app.dispose();
    c.check("quitting a second time does nothing", again.elapsed() < Duration::from_secs(1), again.elapsed());

    read_only(&flags, false);
    roblox_flags::restore();
    c.check(
        "…and the next start puts the person's file back",
        std::fs::read_to_string(&flags).is_ok_and(|t| t == OWN) && !roblox_flags::pending(),
        (),
    );
}
