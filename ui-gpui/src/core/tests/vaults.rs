// Tests/Bloxkeep.Tests/VaultsSuite.cs: several vaults (vaults.rs, app/vaults.rs): the vault from before joins the list
// with its backup state; a vault is added with its own name and master password; each keeps its own accounts and file
// (an open vault writes only its own), the one last used comes back after a restart, the picker shows at start when
// asked for, and one taken off the list is set aside, never deleted, but not the one open.
//
// PORT: "a new AppController shows the picker" is read from a fresh VaultsState (what the App's constructor sets up);
// a command the C# started without awaiting (an unlock or a set-up still working out its key) runs as a task of its own.
use std::cell::RefCell;
use std::collections::HashSet;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use serde_json::{Value, json};

use super::rejoin::quiet_app;
use super::{Checks, call, user_error};
use crate::core::app::App;
use crate::core::app::controller::PendingLaunch;
use crate::core::app::schedule::Wall;
use crate::core::app::vaults::VaultsState;
use crate::core::base::{Res, Stamp};
use crate::core::game_target::GameTarget;
use crate::core::models::{Account, LaunchPreset, PresetSchedule};
use crate::core::vault::{self, Vault};
use crate::core::{backup, prefs, rt, themes, vaults};

const MAIN_PASSWORD: &str = "main vault password";
const FARM_PASSWORD: &str = "farm vault password";

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn open_vault(app: &App) -> Option<std::sync::Arc<Vault>> {
    app.vault_ref()
}

fn current_id() -> Option<String> {
    vaults::current().map(|v| v.id)
}

fn list() -> Vec<prefs::VaultEntry> {
    prefs::lock().vault_list.clone()
}

/// A command started now and awaited later (the C#'s `var t = Harness.Call(...)` … `await t`).
fn start(app: &Rc<App>, command: &'static str, args: Value) -> Rc<RefCell<Option<Res<Value>>>> {
    let out: Rc<RefCell<Option<Res<Value>>>> = Rc::new(RefCell::new(None));
    let (app, keep) = (app.clone(), out.clone());
    rt::spawn(async move {
        let r = call(&app, command, args).await;
        *keep.borrow_mut() = Some(r);
    });
    out
}

async fn finish(task: &Rc<RefCell<Option<Res<Value>>>>) {
    for _ in 0..600 {
        if task.borrow().is_some() {
            return;
        }
        rt::sleep(Duration::from_millis(100)).await;
    }
}

pub async fn run(c: &mut Checks, dir: &str) {
    let data = vault::data_dir().to_path_buf();
    // The first vault set up on a PC: vault.rmv, and on the list as that file (review, 2026-10-30: the entry was made once
    // the file existed, so it named another file). Cleared after, for the PC from before below.
    {
        let (first, _said) = quiet_app();
        let _ = call(&first, "createVault", json!({ "password": MAIN_PASSWORD, "name": "" })).await;
        let first_file = open_vault(&first).map(|v| file_name(v.file_path()));
        let files: Vec<String> = list().iter().map(|v| v.file.clone()).collect();
        c.check(
            "the first vault on a PC: vault.rmv, on the list as that file and offered",
            first_file.as_deref() == Some("vault.rmv") && files == ["vault.rmv"] && vaults::current().is_some_and(|v| v.file == "vault.rmv"),
            format!("{first_file:?} / {}", files.join(", ")),
        );
        let _ = call(&first, "lock", json!({})).await;
    }
    for f in std::fs::read_dir(&data).map(|d| d.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect::<Vec<_>>()).unwrap_or_default() {
        let _ = std::fs::remove_file(f);
    }
    {
        let mut s = prefs::lock();
        s.vault_list.clear();
        s.last_vault = None;
    }
    vaults::use_entry(None);
    // A PC from before several vaults: vault.rmv, and its backup state in prefs.json's old keys.
    {
        let old = Vault::create(MAIN_PASSWORD, None).expect("a vault");
        old.data().accounts.push(Account { username: "MainOne".into(), user_id: 51, ..Default::default() });
        old.save().expect("saved");
    }
    let backed_up = Stamp::utc_now().plus_secs(-3.0 * 3600.0);
    prefs::lock().set_last_backup(Some(backed_up));
    prefs::save();
    prefs::lock().vault_list.clear();
    prefs::load();
    vaults::load();
    let main = {
        let l = list();
        if l.len() == 1 { Some(l[0].clone()) } else { None }
    };
    c.check(
        "the vault from before joins the list, with its backup state",
        main.as_ref().is_some_and(|m| {
            m.file == "vault.rmv" && current_id().as_ref() == Some(&m.id) && m.last_backup.is_some_and(|at| at.secs_since(backed_up).abs() < 2.0)
        }),
        main.as_ref().map(|m| m.name.clone()),
    );
    let Some(main) = main else { return };

    let (app, _said) = quiet_app();
    let _ = call(&app, "unlock", json!({ "password": MAIN_PASSWORD })).await;
    c.check("it opens as before", open_vault(&app).is_some_and(|v| v.data().accounts.iter().any(|a| a.username == "MainOne")), ());

    let _ = call(&app, "pickVault", json!({})).await;
    c.check("Switch vault shows the picker over the open vault, which stays open", open_vault(&app).is_some() && app.vaults.vault_pick.get(), ());
    let _ = call(&app, "closeVaultPicker", json!({})).await;
    c.check("Back returns to it without the password", open_vault(&app).is_some() && !app.vaults.vault_pick.get(), ());
    let _ = call(&app, "pickVault", json!({})).await;
    let _ = call(&app, "selectVault", json!({ "id": main.id })).await;
    c.check("picking the open vault in the picker returns to it as well", open_vault(&app).is_some() && !app.vaults.vault_pick.get(), ());

    let _ = call(&app, "pickVault", json!({})).await;
    let _ = call(&app, "addVault", json!({})).await;
    c.check(
        "adding one: the open vault stays open behind the form, the new one isn't on the list yet",
        open_vault(&app).is_some() && vault::active_file() == "vault.rmv" && list().len() == 1,
        (),
    );
    let _ = call(&app, "cancelAddVault", json!({})).await;
    c.check("Cancel goes back to the picker, the open vault still open", open_vault(&app).is_some() && app.vaults.vault_pick.get(), ());
    let _ = call(&app, "addVault", json!({})).await;
    let short_refused = user_error(&call(&app, "createVault", json!({ "password": "short", "name": "Farm" })).await).is_some();
    c.check(
        "a password too short is refused before the open vault locks",
        short_refused && open_vault(&app).is_some() && vault::active_file() == "vault.rmv",
        (),
    );
    let _ = call(&app, "createVault", json!({ "password": FARM_PASSWORD, "name": "  Farm  " })).await;
    let farm = list().into_iter().find(|v| v.name == "Farm");
    c.check(
        "set up with its name and its own master password, it joins the list and opens empty",
        farm.as_ref().is_some_and(|f| {
            current_id().as_ref() == Some(&f.id)
                && prefs::lock().last_vault.as_ref() == Some(&f.id)
                && open_vault(&app).is_some_and(|v| v.data().accounts.is_empty() && file_name(v.file_path()) == f.file)
        }),
        list().iter().map(|v| format!("{} {}", v.name, v.file)).collect::<Vec<_>>().join(", "),
    );
    let Some(farm) = farm else { return };
    if let Some(v) = open_vault(&app) {
        v.data().accounts.push(Account { username: "FarmOne".into(), user_id: 52, ..Default::default() });
        v.save().expect("saved");
    }
    c.check("a new vault has no backup yet (each has its own)", prefs::lock().last_backup().is_none(), ());

    let farm_bytes = std::fs::read(data.join(&farm.file)).unwrap_or_default();
    let _ = call(&app, "selectVault", json!({ "id": main.id })).await;
    c.check("picking the other one locks this one and offers that one", open_vault(&app).is_none() && vault::active_file() == "vault.rmv", ());
    let refused = user_error(&call(&app, "unlock", json!({ "password": FARM_PASSWORD })).await).is_some();
    c.check("each vault has its own master password", refused && open_vault(&app).is_none(), ());
    let _ = call(&app, "unlock", json!({ "password": MAIN_PASSWORD })).await;
    let main_open = open_vault(&app).expect("the main vault open");
    {
        let d = main_open.data();
        c.check(
            "and its own accounts",
            d.accounts.iter().any(|a| a.username == "MainOne") && !d.accounts.iter().any(|a| a.username == "FarmOne"),
            (),
        );
    }
    main_open.data().accounts.push(Account { username: "MainTwo".into(), user_id: 53, ..Default::default() });
    main_open.save().expect("saved");
    c.check("an open vault writes only its own file", std::fs::read(data.join(&farm.file)).unwrap_or_default() == farm_bytes, ());

    // The other vault picked while this one's key is still being worked out (the unlock takes a moment): this unlock is
    // let go, and nothing of this vault ever lands in the other's file (2026-10-30 review: it saved over it).
    let _ = call(&app, "lock", json!({})).await;
    drop(main_open);
    let unlocking = start(&app, "unlock", json!({ "password": MAIN_PASSWORD }));
    rt::sleep(Duration::from_millis(50)).await;
    let _ = call(&app, "selectVault", json!({ "id": farm.id })).await;
    finish(&unlocking).await;
    c.check(
        "another vault picked during an unlock: that unlock is let go, the picked one waits for its own password",
        open_vault(&app).is_none() && vault::active_file() == farm.file,
        format!("{:?} {}", open_vault(&app).map(|v| v.file_path().to_path_buf()), vault::active_file()),
    );
    c.check("…and the other vault's file is untouched", std::fs::read(data.join(&farm.file)).unwrap_or_default() == farm_bytes, ());
    let _ = call(&app, "selectVault", json!({ "id": main.id })).await;
    let _ = call(&app, "unlock", json!({ "password": MAIN_PASSWORD })).await;
    let main_open = open_vault(&app).expect("the main vault open");

    // A Roblox window of one vault keeps running while another is open (2026-10-30 review): its play time and its
    // scheduled stop stay its own vault's, never written into the other's.
    let main_account = main_open.data().accounts.iter().find(|a| a.username == "MainOne").map(|a| a.id).expect("MainOne");
    app.controller.running.borrow_mut().insert(
        main_account,
        (999_981, Rc::new(PendingLaunch::new(main_account, HashSet::new(), Stamp::utc_now(), Some(GameTarget::new(2753915549, None, None, None)), false, None))),
    );
    let farm_preset = LaunchPreset {
        name: "Main farm".into(),
        account_ids: vec![main_account],
        schedule: Some(PresetSchedule { enabled: true, time: "08:00".into(), days: vec![1], stop: "after".into(), stop_hours: 5.0, ..Default::default() }),
        ..Default::default()
    };
    main_open.data().settings.presets.push(farm_preset.clone());
    app.arm_scheduled_stop(&farm_preset, Some(main_open.clone()), Wall::now(), HashSet::new(), Vec::new(), true);
    let armed = main_open.data().scheduled_runs.len();
    c.check("(a scheduled stop is armed for the main vault's window)", armed == 1, armed);
    drop(main_open);
    let _ = call(&app, "selectVault", json!({ "id": farm.id })).await;
    let _ = call(&app, "unlock", json!({ "password": FARM_PASSWORD })).await;
    let farm_open = open_vault(&app).expect("the farm vault open");
    app.track_play(Stamp::utc_now().plus_secs(3600.0));
    let n = farm_open.data().play_sessions.len();
    c.check("the other vault's window still open: no stay of it in this vault's statistics", n == 0, n);
    app.note_scheduled_runs();
    let n = farm_open.data().scheduled_runs.len();
    c.check("…nor its scheduled stop in this vault's file", n == 0, n);
    drop(farm_open);
    let _ = call(&app, "selectVault", json!({ "id": main.id })).await;
    let _ = call(&app, "unlock", json!({ "password": MAIN_PASSWORD })).await;
    let main_open = open_vault(&app).expect("the main vault open");
    let runs = app.schedule.scheduled_runs.borrow().len();
    let in_file = main_open.data().scheduled_runs.len();
    c.check("back in its own vault: the stop is armed once, and still in its file", runs == 1 && in_file == 1, format!("{runs} {in_file}"));
    app.controller.running.borrow_mut().clear();
    app.schedule.scheduled_runs.borrow_mut().clear();
    main_open.data().settings.presets.retain(|p| p.id != farm_preset.id);
    app.note_scheduled_runs();
    main_open.save().expect("saved");
    drop(main_open);

    // Back after a restart: the list, and the last vault used is the one offered.
    prefs::save();
    {
        let mut s = prefs::lock();
        s.vault_list.clear();
        s.last_vault = None;
    }
    prefs::load();
    vaults::load();
    c.check(
        "after a restart the list is back and the last vault used is offered",
        list().len() == 2 && current_id().as_ref() == Some(&main.id),
        list().iter().map(|v| v.name.clone()).collect::<Vec<_>>().join(", "),
    );

    let _ = call(&app, "setAskVault", json!({ "enabled": true })).await;
    c.check("asked for: with two vaults the picker shows at start", VaultsState::default().vault_pick.get(), ());

    let kept = user_error(&call(&app, "removeVault", json!({ "id": main.id })).await).is_some() && list().iter().any(|v| v.id == main.id);
    c.check("the vault open right now can't be taken off the list", kept, ());
    let _ = call(&app, "removeVault", json!({ "id": farm.id })).await;
    let set_aside: Vec<String> = std::fs::read_dir(vault::old_vaults_dir())
        .map(|d| d.flatten().map(|e| file_name(&e.path())).filter(|n| n.starts_with("vault-removed-") && n.ends_with(".rmv")).collect())
        .unwrap_or_default();
    c.check(
        "one taken off the list is set aside, never deleted",
        list().len() == 1 && !data.join(&farm.file).exists() && set_aside.iter().any(|n| !n.contains("-spare")),
        set_aside.join(", "),
    );
    c.check("with one vault left the picker doesn't show at start", !VaultsState::default().vault_pick.get(), ());

    c.check(
        "a vault's picture is served by its own kind of name only",
        themes::is_picture_name("vault-picture-a1b2c3d4-638700000000000000.png")
            && !themes::is_picture_name("vault-picture-a1b2c3d4-1.png/../vault.rmv")
            && !themes::is_picture_name("vault-picture-xyz-1.png"),
        (),
    );

    // Back pressed while a new vault is being set up (its key takes a moment): the vault written is the one that joins the
    // list, not another entry pointing at a file never written (review, 2026-10-30: the new vault was left an orphan).
    let _ = call(&app, "addVault", json!({})).await;
    let creating = start(&app, "createVault", json!({ "password": FARM_PASSWORD, "name": "Third" }));
    rt::sleep(Duration::from_millis(50)).await;
    let _ = call(&app, "cancelAddVault", json!({})).await;
    finish(&creating).await;
    let open_file = open_vault(&app).map(|v| file_name(v.file_path()));
    let third = list().into_iter().find(|v| v.name == "Third");
    c.check(
        "Back during a new vault's set-up: the vault written joins the list with its own file, and is the one offered",
        third.as_ref().is_some_and(|t| Some(&t.file) == open_file.as_ref() && data.join(&t.file).is_file() && current_id().as_ref() == Some(&t.id)),
        format!("{open_file:?} / {}", list().iter().map(|v| format!("{} {}", v.name, v.file)).collect::<Vec<_>>().join(", ")),
    );
    // A set-up that started and failed, then Cancel: the lock screen offers the vault from before, not the unwritten file.
    let _ = call(&app, "addVault", json!({})).await;
    let _ = app.switch_to_new_vault();
    let _ = call(&app, "cancelAddVault", json!({})).await;
    c.check(
        "…a set-up that failed, then Cancel: the vault from before is offered again",
        third.as_ref().is_some_and(|t| current_id().as_ref() == Some(&t.id) && vault::active_file() == t.file),
        vault::active_file(),
    );

    let Some(third) = third else {
        c.check("the checks after this need the third vault on the list", false, ());
        return;
    };

    // A backup cut short (review, 2026-10-30): one only the size of a header isn't taken as a vault to restore; one that got
    // through and won't open says where the vault it replaced is kept.
    let third_path = data.join(&third.file);
    let cut = Path::new(dir).join("cut-short.rmv");
    let whole = std::fs::read(&third_path).unwrap_or_default();
    std::fs::write(&cut, &whole[..100.min(whole.len())]).expect("written");
    c.check(
        "a backup cut short to 100 bytes isn't taken as a vault; a whole one is",
        !vault::looks_like_vault(&cut).unwrap_or(true) && vault::looks_like_vault(&third_path).unwrap_or(false),
        (),
    );
    std::fs::create_dir_all(vault::old_vaults_dir()).expect("folder");
    let replaced = vault::old_vaults_dir().join("vault-before-restore-20261030-120000.rmv");
    std::fs::copy(&third_path, &replaced).expect("copied");
    let replaced = replaced.to_string_lossy().into_owned();
    *app.backup.restored_over.borrow_mut() = Some((third.file.clone(), replaced.clone()));
    let failed_unlock = call(&app, "unlock", json!({ "password": "not this vault's password" })).await;
    c.check(
        "…the first unlock after a restore fails: says where the vault it replaced is kept",
        user_error(&failed_unlock).is_some_and(|m| m.contains(&replaced)),
        failed_unlock.as_ref().err().map(|e| e.message().to_string()),
    );
    let _ = call(&app, "unlock", json!({ "password": FARM_PASSWORD })).await;
    c.check("…once it opens, not said again", open_vault(&app).is_some() && app.backup.restored_over.borrow().is_none(), ());

    // A backup made under another password, restored and unlocked with the usual one (review, 2026-10-30): the replaced
    // vault, left as the spare, opened with it and put itself back, the restored file set aside as "damaged".
    let _ = call(&app, "lock", json!({})).await;
    let other_password = "the backup's own password";
    let older = Path::new(dir).join("other-password.rmv");
    {
        let o = Vault::create(other_password, Some(older.clone())).expect("a vault");
        o.save().expect("saved");
    }
    let _ = backup::restore(&older, true);
    let usual_opened = match Vault::open(FARM_PASSWORD, None) {
        Ok(_) => true,
        Err(f) => !vault::is_invalid_password(&f),
    };
    c.check("a restored backup with another password: the usual password doesn't bring the old vault back", !usual_opened, ());
    let restored = Vault::open(other_password, None);
    let damaged = std::fs::read_dir(&data)
        .map(|d| d.flatten().map(|e| file_name(&e.path())).any(|n| n.contains(".damaged-") && n.contains(&third.file)))
        .unwrap_or(false);
    c.check("…its own opens it, the file still the restored one", restored.as_ref().is_ok_and(|r| !r.id().is_empty()) && !damaged, ());
}
