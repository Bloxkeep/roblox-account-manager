// Tests/Bloxkeep.Tests/ForeignVaultSuite.cs: a backup made somewhere else (app/backup.rs, note_restored and the foreign
// links dropped at the first unlock): restored and unlocked, it keeps its accounts but not the launcher it names nor its
// Discord links, which whoever made it chose; a backup of this PC's own vault keeps everything. A launcher on the
// network is never looked at or started (roblox_launcher::is_local_program).
//
// PORT: the data folder is fixed for the process (vault::data_dir), so the C#'s "<dir>\data" is the runner's folder and
// the backups and the launcher sit in a folder beside the vaults ("elsewhere").
use std::path::Path;

use serde_json::json;

use super::rejoin::{no_network, quiet_app};
use super::{Checks, call};
use crate::core::app::backup::note_restored;
use crate::core::base::Fail;
use crate::core::models::{Account, LaunchPreset};
use crate::core::vault::{self, Vault};
use crate::core::{backup, prefs, roblox_launcher};

const OWN_PASSWORD: &str = "own vault password";
const THEIR_PASSWORD: &str = "their vault password";
const NETWORK_LAUNCHER: &str = r"\\attacker@SSL\share\RobloxPlayerLauncher.exe";

pub async fn run(c: &mut Checks, dir: &str) {
    let their_hook = format!("https://discord.com/api/webhooks/123456789012345678/{}", "T".repeat(68));
    let own_hook = format!("https://discord.com/api/webhooks/876543210987654321/{}", "O".repeat(68));
    no_network();
    let elsewhere = Path::new(dir).join("elsewhere");
    std::fs::create_dir_all(&elsewhere).expect("a folder");
    let theirs = elsewhere.join("their-backup.rmv");
    let ours = elsewhere.join("our-backup.rmv");
    let launcher = elsewhere.join("MyLauncher.exe").to_string_lossy().into_owned();
    std::fs::write(&launcher, "").expect("a launcher");

    // Someone else's vault: an account, a launcher on their share, their Discord channel for alerts and a preset.
    let their_id = {
        let made = Vault::create(THEIR_PASSWORD, None).expect("a vault");
        {
            let mut d = made.data();
            d.accounts.push(Account { username: "TheirAlt".into(), user_id: 77, ..Default::default() });
            d.settings.launch_via = "custom".into();
            d.settings.custom_launcher = NETWORK_LAUNCHER.into();
            d.discord.enabled = true;
            d.discord.webhook_url = their_hook.clone();
            d.settings.presets.push(LaunchPreset { name: "Farm".into(), discord_webhook: their_hook.clone(), ..Default::default() });
        }
        made.save().expect("saved");
        let id = made.id().to_string();
        made.dispose();
        id
    };
    std::fs::copy(vault::vault_path(), &theirs).expect("copied");
    if let Ok(entries) = std::fs::read_dir(vault::data_dir()) {
        for e in entries.flatten() {
            if e.path().is_file() && e.file_name().to_string_lossy().starts_with("vault") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }

    // This PC's own vault, with its own launcher and Discord channel.
    let own_id = {
        let own = Vault::create(OWN_PASSWORD, None).expect("a vault");
        {
            let mut d = own.data();
            d.accounts.push(Account { username: "MyMain".into(), user_id: 11, ..Default::default() });
            d.settings.launch_via = "custom".into();
            d.settings.custom_launcher = launcher.clone();
            d.discord.enabled = true;
            d.discord.webhook_url = own_hook.clone();
        }
        own.save().expect("saved");
        let id = own.id().to_string();
        own.dispose();
        id
    };

    let (app, _said) = quiet_app();
    let v = || app.vault_ref().expect("unlocked");
    let _ = call(&app, "unlock", json!({ "password": OWN_PASSWORD })).await;
    {
        let p = prefs::lock();
        c.check(
            "a vault unlocked here is one of this PC's",
            p.known_vault_ids.contains(&own_id) && !p.known_vault_ids.contains(&their_id),
            (),
        );
    }
    std::fs::copy(vault::vault_path(), &ours).expect("copied");

    // What Restore backup does once the file is picked (restore_backup, the dialog left out).
    let restore = async |file: &Path| {
        let _ = call(&app, "lock", json!({})).await;
        // The copy kept aside is named to the second; two restores here can fall in the same one.
        if let Ok(entries) = std::fs::read_dir(vault::old_vaults_dir()) {
            for e in entries.flatten() {
                if e.file_name().to_string_lossy().starts_with("vault-before-restore-") {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
        backup::restore(file, true).expect("restored");
        note_restored(file);
    };

    // This PC's own backup: nothing dropped.
    restore(&ours).await;
    let _ = call(&app, "unlock", json!({ "password": OWN_PASSWORD })).await;
    {
        let vault = v();
        let d = vault.data();
        let s = &d.settings;
        c.check(
            "this PC's own backup, restored: its launcher and Discord link kept",
            prefs::lock().foreign_restore.is_none()
                && s.launch_via == "custom"
                && s.custom_launcher == launcher
                && d.discord.enabled
                && d.discord.webhook_url == own_hook,
            format!("{} {} {}", s.launch_via, s.custom_launcher, d.discord.webhook_url.len()),
        );
    }

    // Someone else's backup: noted at the restore, then its launcher and Discord links dropped at the first unlock.
    restore(&theirs).await;
    let noted = prefs::lock().foreign_restore.clone();
    c.check("a backup made elsewhere is noted at the restore", noted.as_deref() == Some(their_id.as_str()), &noted);
    let _ = call(&app, "unlock", json!({ "password": THEIR_PASSWORD })).await;
    {
        let vault = v();
        let d = vault.data();
        let s = &d.settings;
        c.check("…unlocked: its accounts are there", d.accounts.iter().any(|a| a.username == "TheirAlt"), ());
        c.check(
            "…the launcher it named is gone, Roblox opens the usual way",
            s.custom_launcher.is_empty() && s.launch_via == "auto",
            format!("{} {}", s.launch_via, s.custom_launcher),
        );
        c.check("…its Discord channel is gone and Discord is off", !d.discord.enabled && d.discord.webhook_url.is_empty(), &d.discord.webhook_url);
        c.check("…its presets' Discord links are gone", s.presets.iter().all(|p| p.discord_webhook.is_empty()), s.presets.len());
    }
    {
        let p = prefs::lock();
        c.check("…and it is one of this PC's from now on", p.foreign_restore.is_none() && p.known_vault_ids.contains(&their_id), ());
    }

    // Launchers on the network: never this PC's program, never looked at.
    for path in [NETWORK_LAUNCHER, r"\\server\share\Bloxstrap.exe", r"\\?\UNC\server\share\a.exe", r"\\.\pipe\x", "RobloxPlayerLauncher.exe", ""] {
        c.check(
            &format!("a launcher at \"{path}\" isn't a program on this PC"),
            !roblox_launcher::is_local_program(path) && !roblox_launcher::custom_launcher_ready(path),
            (),
        );
    }
    c.check(
        "a launcher on this PC's disk is",
        roblox_launcher::is_local_program(&launcher) && roblox_launcher::custom_launcher_ready(&launcher),
        (),
    );
    v().data().settings.custom_launcher = NETWORK_LAUNCHER.into();
    let _ = call(&app, "setSettings", json!({ "launchVia": "custom" })).await;
    let via = v().data().settings.launch_via.clone();
    c.check("Settings won't switch to a launcher on the network", via == "auto", &via);
    // Launch refuses it before starting anything: checked first, so a port that would start it never gets the chance.
    if roblox_launcher::custom_launcher_ready(NETWORK_LAUNCHER) || roblox_launcher::other_name("custom").is_some() {
        c.skip("Play with a launcher on the network: refused as gone, nothing started", "starts Roblox");
    } else {
        let result = roblox_launcher::launch("ticket", None, "custom", NETWORK_LAUNCHER, None);
        c.check(
            "Play with a launcher on the network: refused as gone, nothing started",
            matches!(&result, Err(Fail::User(m)) if m.contains("gone")),
            &result,
        );
    }
}
