// Tests/Bloxkeep.Tests/PresetSuite.cs: presets' options, a preset's own Discord link, closing a preset's windows,
// deleting accounts, signing in again.
use std::collections::HashSet;

use serde_json::{Value, json};

use super::{
    Checks, call,
    rejoin::{no_network, quiet_app, refusal, seed, told},
    unlock,
};
use crate::core::{
    app::{App, vault_import::merge_import},
    base::{Cancel, Guid, Stamp},
    models::{Account, LaunchPreset, PresetSchedule, VaultData},
    prefs, roblox_processes,
};

fn one_off(enabled: bool, last_run: &str) -> PresetSchedule {
    PresetSchedule { enabled, time: "08:00".into(), date: "2026-11-02".into(), last_run: last_run.into(), ..Default::default() }
}

fn guid_of(v: &Value) -> Option<Guid> {
    v.as_str().and_then(|s| s.parse().ok())
}

pub async fn run(c: &mut Checks, _dir: &str) {
    no_network();
    let made = |name: &str, cookie: &str, user_id: i64| Account { username: name.into(), cookie: cookie.into(), user_id, cookie_valid: Some(true), ..Default::default() };
    let (f1, f2, f3) = (made("farm1", "x1", 1001), made("farm2", "x2", 1002), made("main", "x3", 1003));
    let (a1, a2, a3) = (f1.id, f2.id, f3.id);
    seed(vec![f1, f2, f3]);
    let (app, said) = quiet_app();
    unlock(&app).await;
    c.check("unlocked", app.controller.vault.borrow().is_some(), ());
    let preset_of = |id: Guid| app.vault(|d| d.settings.presets.iter().find(|p| p.id == id).cloned()).ok().flatten();

    // A preset saved seconds before its schedule's minute: a run in between must stand.
    let mut read = one_off(true, "");
    App::catch_up_schedule(Some(&mut read), Some(&one_off(false, "2026-11-02 08:00")));
    c.check("a one-off that ran during the save stays over, with its run noted", !read.enabled && read.last_run == "2026-11-02 08:00", &read.last_run);
    let mut moved = one_off(true, "");
    moved.time = "09:00".into();
    App::catch_up_schedule(Some(&mut moved), Some(&one_off(false, "2026-11-02 08:00")));
    c.check("…one given another time in the same save is on for that time", moved.enabled && moved.last_run == "2026-11-02 08:00", ());
    let mut weekly = PresetSchedule { enabled: true, time: "08:00".into(), days: vec![1], last_run: "2026-10-26 08:00".into(), ..Default::default() };
    App::catch_up_schedule(
        Some(&mut weekly),
        Some(&PresetSchedule { enabled: true, time: "08:00".into(), days: vec![1], last_run: "2026-11-02 08:00".into(), ..Default::default() }),
    );
    c.check(
        "a weekly one that ran during the save keeps that run (not again this minute)",
        weekly.enabled && weekly.last_run == "2026-11-02 08:00",
        &weekly.last_run,
    );

    let a68 = "A".repeat(68);
    let hook = format!("https://discordapp.com/api/v10/webhooks/123456789012345678/{a68}");
    let saved = call(
        &app,
        "savePreset",
        json!({ "name": "Farm", "ids": [a1, a2], "placeInput": "", "arrange": true, "discordWebhook": hook,
            "schedule": { "enabled": true, "time": "20:00", "days": [1, 2, 3], "date": "", "stop": "never", "stopHours": 0, "stopTime": "" } }),
    )
    .await;
    let id = saved.as_ref().ok().and_then(guid_of);
    c.check("savePreset returns an id", id.is_some(), format!("{saved:?}"));
    let id = id.unwrap_or_default();
    let p = preset_of(id).unwrap_or_default();
    c.check(
        "link normalised to discord.com",
        p.discord_webhook == format!("https://discord.com/api/webhooks/123456789012345678/{a68}"),
        &p.discord_webhook,
    );
    c.check("arrange saved", p.arrange == Some(true), ());
    let dto = app.preset_dto(&p);
    c.check("dto: masked link only", dto["discord"] == "discord.com/api/webhooks/…5678/••••••", &dto["discord"]);
    c.check("dto: no raw link anywhere", !dto.to_string().contains(&"A".repeat(20)), ());
    c.check("dto: arrange", dto["arrange"] == true, ());
    c.check("dto: running 0", dto["running"] == 0, ());

    let _ = call(&app, "savePreset", json!({ "id": id, "name": "Farm", "ids": [a1, a2], "placeInput": "", "arrange": null })).await;
    let p = preset_of(id).unwrap_or_default();
    c.check("no discordWebhook key keeps the link", !p.discord_webhook.is_empty(), ());
    c.check("arrange back to follow", p.arrange.is_none(), ());

    let bad = call(&app, "savePreset", json!({ "id": id, "name": "Farm", "ids": [a1], "placeInput": "", "discordWebhook": "https://example.com/hook" })).await;
    c.check("a link that isn't Discord's is refused", refusal(&bad).is_some(), refusal(&bad));
    c.check("refused save changed nothing", preset_of(id).is_some_and(|p| p.account_ids.len() == 2), ());

    let _ = call(&app, "savePreset", json!({ "id": id, "name": "Farm", "ids": [a1, a2], "placeInput": "", "discordWebhook": "" })).await;
    c.check("'' goes back to Settings' channel", preset_of(id).is_some_and(|p| p.discord_webhook.is_empty()), ());

    c.check("test with a bad link says so", refusal(&call(&app, "presetDiscordTest", json!({ "id": id, "url": "nope" })).await).is_some(), ());
    c.check("test without any link says so", refusal(&call(&app, "presetDiscordTest", json!({ "id": id, "url": "" })).await).is_some(), ());

    let closed = call(&app, "closePreset", json!({ "id": id })).await;
    c.check("closePreset with nothing open: 0", matches!(&closed, Ok(v) if v == 0), format!("{closed:?}"));

    let dup = call(&app, "duplicatePreset", json!({ "id": id })).await;
    let copy = dup.as_ref().ok().and_then(guid_of).and_then(preset_of);
    let p = preset_of(id).unwrap_or_default();
    c.check(
        "duplicate keeps arrange and webhook",
        copy.as_ref().is_some_and(|copy| copy.arrange == p.arrange && copy.discord_webhook == p.discord_webhook),
        (),
    );

    // A copy of a preset that follows a player follows them too, so it can run.
    let mut extra = crate::core::base::Extra::new();
    extra.insert("later".into(), json!(5));
    let follows = LaunchPreset {
        name: "With Friend".into(),
        account_ids: vec![a3],
        server: "follow".into(),
        follow_player: "Friend".into(),
        follow_user_id: 77,
        restart_hours: 4,
        last_run_at: Some(Stamp::utc_now()),
        extra,
        schedule: Some(PresetSchedule { enabled: true, time: "08:00".into(), days: vec![1], last_run: "2026-11-02 08:00".into(), ..Default::default() }),
        ..Default::default()
    };
    let follows_id = follows.id;
    let _ = app.vault_mut(|d| d.settings.presets.push(follows.clone()));
    let follow_copy_id = call(&app, "duplicatePreset", json!({ "id": follows_id })).await.ok().as_ref().and_then(guid_of).unwrap_or_default();
    let fc = preset_of(follow_copy_id).unwrap_or_default();
    c.check(
        "a copy of a preset that follows a player follows them too",
        fc.server == "follow" && fc.follow_player == "Friend" && fc.follow_user_id == 77 && fc.restart_hours == 4,
        format!("{} {} {} {}", fc.server, fc.follow_player, fc.follow_user_id, fc.restart_hours),
    );
    c.check("…keeps what a newer Bloxkeep saved with it", fc.extra.contains_key("later"), ());
    c.check(
        "…is its own: another id, its schedule off, no run yet",
        fc.id != follows_id
            && fc.schedule.as_ref().is_some_and(|s| !s.enabled && s.last_run.is_empty() && s.time == "08:00")
            && fc.last_run_at.is_none(),
        (),
    );
    let _ = app.vault_mut(|d| d.settings.presets.retain(|p| p.id != follow_copy_id && p.id != follows_id));

    // A scheduled run that fails is said, not only written to error.log.
    // PORT: the C# broke the preset with a null account list (a NullReferenceException); a list can't be null here, so the
    // run fails on a preset the vault doesn't hold.
    told(&said);
    let broken = LaunchPreset { name: "Broken".into(), ..Default::default() };
    app.run_scheduled(vec![broken]).await;
    let told_broken = told(&said);
    c.check(
        "a scheduled run that failed unexpectedly is said",
        told_broken.as_deref().is_some_and(|t| t.contains("Scheduled preset Broken didn't run")),
        &told_broken,
    );

    // Close, update, reopen while Bloxkeep is opening Roblox: refused. Only checked with no Roblox on this PC.
    if roblox_processes::get_player_pids().is_empty() {
        *app.controller.launch_cts.borrow_mut() = Some(Cancel::new());
        let reopen = call(&app, "updateAndReopen", json!({})).await;
        c.check(
            "Close, update, reopen while opening Roblox: refused, and says why",
            refusal(&reopen).is_some() && app.roblox_updates.update_reopen_since.get().is_none(),
            refusal(&reopen).unwrap_or_else(|| "not refused".into()),
        );
        *app.controller.launch_cts.borrow_mut() = None;
        app.roblox_updates.update_reopen_since.set(None);
    }

    let _ = app.vault_mut(|d| {
        d.settings.launch_order.extend([a1, a2, a3]);
        d.discord.summary_accounts.extend([a1, a3]);
    });
    let _ = call(&app, "deleteAccounts", json!({ "ids": [a1, a2] })).await;
    let after = preset_of(id).unwrap_or_default();
    c.check("deleted accounts gone from the preset", after.account_ids.is_empty(), after.account_ids.len());
    c.check("empty preset's schedule switched off", after.schedule.as_ref().is_some_and(|s| !s.enabled), after.schedule.as_ref().map(|s| s.enabled));
    let order = app.vault(|d| d.settings.launch_order.clone()).unwrap_or_default();
    c.check("launch order cleaned", !order.contains(&a1) && order.contains(&a3), ());
    let summary = app.vault(|d| d.discord.summary_accounts.clone()).unwrap_or_default();
    c.check("Discord summary accounts cleaned", summary == [a3], ());

    let mut acc = app.vault(|d| d.accounts[0].clone()).expect("one account");
    acc.auto_rejoin = false;
    acc.rejoin_off_by_sign_out = true;
    App::signed_in_again(&mut acc);
    c.check("signed in again: auto-rejoin back on", acc.auto_rejoin && !acc.rejoin_off_by_sign_out, ());
    app.update_account(acc.id, |a| {
        a.auto_rejoin = true;
        a.rejoin_off_by_sign_out = true;
    });
    let _ = call(&app, "setAutoRejoin", json!({ "ids": [acc.id], "enabled": false })).await;
    let now = app.find_account(acc.id).unwrap_or_default();
    c.check("own choice clears the flag", !now.auto_rejoin && !now.rejoin_off_by_sign_out, ());

    let state = app.build_state();
    c.check("state has lastSignInCheck", state.get("lastSignInCheck").is_some(), ());
    c.check("state has the questions answered Don't ask again", state.get("skipConfirm").is_some(), ());
    let _ = call(&app, "skipConfirm", json!({ "key": "deletePreset" })).await;
    c.check("Don't ask again is kept", prefs::lock().skip_confirm.contains("deletePreset"), ());
    let unknown = call(&app, "skipConfirm", json!({ "key": "deleteAccounts" })).await;
    c.check(
        "an unknown question is refused",
        refusal(&unknown).is_some() && !prefs::lock().skip_confirm.contains("deleteAccounts"),
        (),
    );

    // A preset from another vault's file comes without its own Discord link, which could be someone else's channel.
    let mut file = VaultData::default();
    let theirs = Account { username: "main".into(), user_id: 1003, ..Default::default() };
    let theirs_id = theirs.id;
    file.accounts.push(theirs);
    file.settings.presets.push(LaunchPreset { name: "Theirs".into(), account_ids: vec![theirs_id], discord_webhook: hook.clone(), ..Default::default() });
    file.settings.presets.push(LaunchPreset {
        name: "Their friend".into(),
        account_ids: vec![theirs_id],
        server: "follow".into(),
        follow_player: "Friend".into(),
        follow_user_id: 77,
        restart_hours: 4,
        rejoin_with: "any".into(),
        schedule: Some(PresetSchedule { enabled: true, time: "08:00".into(), days: vec![1], ..Default::default() }),
        ..Default::default()
    });
    let _ = app.vault_mut(|d| {
        merge_import(d, &file, &HashSet::new(), true);
    });
    let find_named = |name: &str| app.vault(|d| d.settings.presets.iter().find(|p| p.name == name).cloned()).ok().flatten();
    let imported = find_named("Theirs");
    c.check(
        "an imported preset comes without its Discord link",
        imported.as_ref().is_some_and(|p| p.discord_webhook.is_empty() && p.account_ids == [acc.id]),
        imported.as_ref().map(|p| p.discord_webhook.clone()),
    );
    let f = find_named("Their friend");
    c.check(
        "an imported preset that follows a player follows them too, restarts kept as saving allows",
        f.as_ref().is_some_and(|p| {
            p.server == "follow" && p.follow_player == "Friend" && p.follow_user_id == 77 && p.restart_hours == 4 && p.schedule.as_ref().is_some_and(|s| !s.enabled)
        }),
        f.as_ref().map_or("missing".to_string(), |p| format!("{} {} {:?}", p.follow_user_id, p.restart_hours, p.schedule.as_ref().map(|s| s.enabled))),
    );
}
