// Tests/Bloxkeep.Tests/AutoLockSuite.cs: the lock screen after a while without use (app/auto_lock.rs): when it is due,
// what the page gets and may do behind it, the master password to take it down, the Lock button from it, and a restart
// for an update behind it. The idle time is handed in (Windows' own input clock isn't something a test can wind on).
//
// PORT: the C#'s stand-in sign-in window was a real (off-screen) WinForms form; the two checks that need it showing are
// skipped, the lock put up and the vault closed without it, and the checks after them still run.
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use base64::{Engine, engine::general_purpose::STANDARD as B64};
use serde_json::{Value, json};

use super::rejoin::{quiet_app, seed};
use super::{Checks, PASSWORD, Said, call, unlock, user_error};
use crate::core::app::App;
use crate::core::app::auto_lock::auto_lock_due;
use crate::core::base::{Fail, Res, Stamp, new_guid};
use crate::core::models::Account;
use crate::core::relaunch;
use crate::core::rt;

fn screen_locked(app: &App) -> bool {
    app.auto_lock.screen_locked.get()
}

fn check(app: &App, idle: Duration) {
    app.check_auto_lock(idle, Stamp::utc_now());
}

fn long_ago_active(app: &App) {
    app.auto_lock.active_at.set(Stamp::utc_now().plus_secs(-3.0 * 3600.0));
}

fn last_state(said: &Said) -> String {
    said.borrow().iter().rev().find(|p| p.contains("\"name\":\"state\"")).cloned().unwrap_or_default()
}

fn minutes(app: &App) -> i32 {
    app.vault_ref().map(|v| v.data().settings.auto_lock_minutes).unwrap_or(-99)
}

/// A command started now and awaited later.
fn start(app: &Rc<App>, command: &'static str, args: Value) -> Rc<RefCell<Option<Res<Value>>>> {
    let out: Rc<RefCell<Option<Res<Value>>>> = Rc::new(RefCell::new(None));
    let (app, keep) = (app.clone(), out.clone());
    rt::spawn(async move {
        let r = call(&app, command, args).await;
        *keep.borrow_mut() = Some(r);
    });
    out
}

async fn finish(task: &Rc<RefCell<Option<Res<Value>>>>) -> Option<Res<Value>> {
    for _ in 0..600 {
        if task.borrow().is_some() {
            break;
        }
        rt::sleep(Duration::from_millis(100)).await;
    }
    task.borrow_mut().take()
}

fn same_vault(a: &Option<std::sync::Arc<crate::core::vault::Vault>>, b: &Option<std::sync::Arc<crate::core::vault::Vault>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => std::sync::Arc::ptr_eq(a, b),
        _ => false,
    }
}

pub async fn run(c: &mut Checks, _dir: &str) {
    // When it is due.
    c.check("0 minutes (Never): never due, whatever the wait", !auto_lock_due(0, Duration::from_secs(3 * 86400)), ());
    c.check("a negative number is never due", !auto_lock_due(-5, Duration::from_secs(3600)), ());
    c.check("15 minutes: not at 14:59", !auto_lock_due(15, Duration::from_secs(15 * 60 - 1)), ());
    c.check("15 minutes: due at 15:00", auto_lock_due(15, Duration::from_secs(15 * 60)), ());
    c.check("15 minutes: due after a night", auto_lock_due(15, Duration::from_secs(9 * 3600)), ());

    let secret = Account { username: "SecretUser42".into(), alias: "HiddenAlias42".into(), user_id: 4242, ..Default::default() };
    let secret_id = secret.id;
    seed(vec![secret]);

    let (app, posts) = quiet_app();
    let windows_toasts: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    {
        let toasts = windows_toasts.clone();
        *app.notify.show_notification.borrow_mut() = Some(Rc::new(move |text: &str, _: &str| {
            toasts.borrow_mut().push(text.to_string());
            true
        }));
    }
    unlock(&app).await;
    app.controller.page_attached.set(true);

    // The setting: only the choices offered.
    let _ = call(&app, "setSettings", json!({ "autoLockMinutes": 15 })).await;
    let vault = app.vault_ref();
    c.check("Lock automatically: 15 minutes saved", minutes(&app) == 15, minutes(&app));
    let _ = call(&app, "setSettings", json!({ "autoLockMinutes": 7 })).await;
    c.check("a time Settings doesn't offer (7) is ignored", minutes(&app) == 15, minutes(&app));
    let _ = call(&app, "setSettings", json!({ "autoLockMinutes": -1 })).await;
    c.check("a negative time is ignored", minutes(&app) == 15, minutes(&app));

    // Choosing the setting counts as using the PC: a long idle reading right after doesn't lock.
    check(&app, Duration::from_secs(2 * 3600));
    c.check("just chosen: Windows' long idle reading doesn't lock yet", !screen_locked(&app), ());
    long_ago_active(&app);
    check(&app, Duration::from_secs(15 * 60 - 1));
    c.check("14:59 without input: still open", !screen_locked(&app), ());

    let _ = call(&app, "setSettings", json!({ "autoLockMinutes": 0 })).await;
    long_ago_active(&app);
    check(&app, Duration::from_secs(86400));
    c.check("Never: a day without input doesn't lock", !screen_locked(&app), ());
    let _ = call(&app, "setSettings", json!({ "autoLockMinutes": 15 })).await;

    // Due: the lock screen, the vault open behind it.
    long_ago_active(&app);
    check(&app, Duration::from_secs(15 * 60));
    c.check("15:00 without input: the lock screen is up", screen_locked(&app), ());
    c.check("the vault stays open behind it (the same one: auto-rejoin, schedules and Discord carry on)", same_vault(&app.vault_ref(), &vault), ());
    let locked = last_state(&posts);
    c.check(
        "the page is told: locked, with the screen lock's minutes",
        locked.contains("\"locked\":true") && locked.contains("\"screenLock\":{\"minutes\":15}"),
        locked.len(),
    );
    c.check(
        "the state the page gets names no account",
        !locked.contains("SecretUser42") && !locked.contains("HiddenAlias42") && locked.contains("\"accounts\":[]"),
        (),
    );
    c.check("nor its settings or Discord", locked.contains("\"settings\":null") && locked.contains("\"discord\":null"), ());
    let before = posts.borrow().len();
    check(&app, Duration::from_secs(5 * 3600));
    c.check("already up: the check doesn't put it up again (no new state)", posts.borrow().len() == before, ());

    // What the page may do behind it.
    let id = secret_id.to_string();
    let behind: Vec<(&str, Value)> = vec![
        ("reveal", json!({ "id": id, "field": "password" })),
        ("copy", json!({ "id": id, "field": "username" })),
        ("setSettings", json!({ "autoLockMinutes": 0 })),
        ("launch", json!({ "ids": [id], "placeInput": "", "jobId": "" })),
        ("exportActivity", json!({})),
        ("totpCode", json!({ "id": id })),
        ("toggleMini", json!({})),
        ("setDiscord", json!({ "enabled": true })),
        ("closeAll", json!({})),
        ("playStats", json!({ "days": 7 })),
    ];
    for (cmd, args) in behind {
        let r = call(&app, cmd, args).await;
        c.check(&format!("behind the lock screen, {cmd} is refused"), user_error(&r).is_some_and(|m| !m.is_empty()), &r);
    }
    c.check("…and Lock automatically wasn't turned off by the refused setSettings", minutes(&app) == 15, ());
    let got = call(&app, "getState", json!({})).await.unwrap_or(Value::Null);
    c.check(
        "getState answers: locked, no accounts, no view to restore",
        got["state"]["locked"].as_bool() == Some(true) && got["state"]["accounts"].as_array().is_some_and(|a| a.is_empty()) && got["view"].is_null(),
        got.to_string().chars().take(200).collect::<String>(),
    );
    let lang = call(&app, "setLanguage", json!({ "language": "en" })).await;
    c.check("the lock screen's language switch still works", user_error(&lang).is_none(), &lang);
    let before = posts.borrow().len();
    app.toast("Farm was kicked", "warn");
    app.progress(Some("Opening SecretUser42"));
    // Looked for by what they are, not by counting every post: a state post from the background may land in between.
    let leaked: Vec<String> = posts.borrow()[before..]
        .iter()
        .filter(|p| p.contains("\"name\":\"toast\"") || p.contains("\"name\":\"progress\"") || p.contains("Farm was kicked") || p.contains("SecretUser42"))
        .map(|p| p.chars().take(120).collect())
        .collect();
    c.check("no toast or progress reaches the page meanwhile (it could name an account)", leaked.is_empty(), leaked.join(" // "));
    c.check("the mini window lists no client meanwhile", app.running_clients().is_empty(), ());
    c.check("…and counts as locked (asks for the password, Close all off)", !app.unlocked(), ());
    let areas: Rc<RefCell<Vec<bool>>> = Rc::new(RefCell::new(Vec::new()));
    {
        let areas = areas.clone();
        *app.monitor.set_monitor_area.borrow_mut() = Some(Rc::new(move |a| areas.borrow_mut().push(a.is_some())));
    }
    let _ = call(&app, "monitorArea", json!({ "x": 0, "y": 0, "w": 800, "h": 600, "scale": 1 })).await;
    c.check("a monitor-tab move arriving behind the lock screen lays no live pictures over it", *areas.borrow() == [false], areas.borrow().len());

    // A command the page sends anyway is answered with the refusal, through the real reply path.
    let before = posts.borrow().len();
    app.handle_message(&format!("{{\"id\":7,\"cmd\":\"reveal\",\"args\":{{\"id\":\"{id}\",\"field\":\"password\"}}}}")).await;
    let reply = posts.borrow()[before..].last().cloned().unwrap_or_default();
    c.check("its reply: refused, \"Bloxkeep is locked.\"", reply.contains("\"ok\":false") && reply.contains("Bloxkeep is locked."), &reply);

    // The password.
    let empty = call(&app, "unlock", json!({ "password": "" })).await;
    c.check("an empty password: asked for", user_error(&empty).is_some(), &empty);
    let wrong = call(&app, "unlock", json!({ "password": "not-the-password" })).await;
    c.check("a wrong password: refused, in the app's words", user_error(&wrong).as_deref() == Some("Wrong master password."), &wrong);
    c.check("…and the lock screen stays up", screen_locked(&app), ());
    let right = call(&app, "unlock", json!({ "password": PASSWORD })).await.unwrap_or(Value::Null);
    c.check("the right password takes it down", !screen_locked(&app) && right["locked"].as_bool() == Some(false), right["locked"].clone());
    c.check("…and the page gets the accounts again", right["accounts"].as_array().is_some_and(|a| a.len() == 1), ());
    c.check("…the vault never closed", same_vault(&app.vault_ref(), &vault), ());
    c.check("…and the mini window counts as unlocked again", app.unlocked(), ());
    check(&app, Duration::from_secs(3 * 3600));
    c.check("just unlocked: Windows' old idle reading doesn't put it straight back up", !screen_locked(&app), ());
    let reveal = call(&app, "reveal", json!({ "id": id, "field": "password" })).await;
    c.check("commands go through again", user_error(&reveal).as_deref() != Some("Bloxkeep is locked."), &reveal);

    // A command that started before the lock screen came up and ends behind it: what it did stands, its answer doesn't
    // reach the lock screen. Changing the master password takes a moment (key stretching), long enough to lock meanwhile.
    const NEW_PASSWORD: &str = "harness-pass-2";
    let changing = start(&app, "changePassword", json!({ "current": PASSWORD, "next": NEW_PASSWORD }));
    rt::sleep(Duration::from_millis(20)).await;
    long_ago_active(&app);
    check(&app, Duration::from_secs(3600));
    let changed = finish(&changing).await;
    c.check(
        "a command still running when it locked: its answer is withheld",
        changed.as_ref().is_some_and(|r| user_error(r).as_deref() == Some("Bloxkeep is locked.")),
        &changed,
    );
    let old_pw = call(&app, "unlock", json!({ "password": PASSWORD })).await;
    c.check("…what it did stands: the old password no longer opens the lock screen", user_error(&old_pw).is_some() && screen_locked(&app), &old_pw);
    let _ = call(&app, "unlock", json!({ "password": NEW_PASSWORD })).await;
    c.check("…and the new one does", !screen_locked(&app), ());
    let _ = call(&app, "changePassword", json!({ "current": NEW_PASSWORD, "next": PASSWORD })).await;

    // The Lock button from the lock screen: Bloxkeep closes for real, and a normal unlock opens it again.
    long_ago_active(&app);
    check(&app, Duration::from_secs(3600));
    c.check("locked again after a while", screen_locked(&app), ());
    let _ = call(&app, "lock", json!({})).await;
    c.check("Lock from the lock screen closes the vault", app.vault_ref().is_none(), ());
    c.check("…and isn't a screen lock any more", !screen_locked(&app), ());
    unlock(&app).await;
    c.check("a normal unlock afterwards opens it, screen not locked", app.vault_ref().is_some() && !screen_locked(&app), ());

    // Locked for real while the lock screen's password is still being checked: the lock stands.
    long_ago_active(&app);
    check(&app, Duration::from_secs(3600));
    let unlocking = start(&app, "unlock", json!({ "password": PASSWORD }));
    rt::sleep(Duration::from_millis(20)).await;
    let _ = call(&app, "lock", json!({})).await;
    let late = finish(&unlocking).await;
    c.check("an unlock that finishes after Lock doesn't open anything", app.vault_ref().is_none() && !screen_locked(&app), &late);
    unlock(&app).await;

    // 2026-11 audit: the sign-in and captcha windows are modal and stayed up over the lock screen, and the browser one could
    // still save an account into the locked vault; Windows notifications ignored the lock.
    sign_in_windows(c, &app, &windows_toasts).await;
    unlock(&app).await;

    // A restart for an update behind the lock screen comes back behind it (the key travels for the background).
    let (key, salt, iterations) = app.vault_ref().expect("open").export_key().expect("the key");
    for screen in [true, false] {
        let next = App::start();
        next.init(0, Rc::new(|_: &str| {}));
        next.controller.proc_interval_ms.set(86_400_000);
        next.resume(relaunch::State {
            key: Some(B64.encode(&key)),
            salt: Some(B64.encode(&salt)),
            iterations,
            screen_locked: screen,
            view: Some("{\"view\":\"accounts\"}".into()),
            ..Default::default()
        });
        let next_locked = next.auto_lock.screen_locked.get();
        let open = next.vault_ref().is_some();
        let first = next.initial_state();
        if screen {
            c.check("restarted behind the lock screen: still behind it, the vault open", next_locked && open, ());
            c.check(
                "…the page gets no accounts and no view back",
                first["state"]["locked"].as_bool() == Some(true) && first["view"].is_null(),
                (),
            );
        } else {
            c.check(
                "restarted unlocked: unlocked again, as before",
                !next_locked && open && first["state"]["locked"].as_bool() == Some(false),
                (),
            );
        }
    }
}

/// The lock put up over a sign-in window, and the Windows notifications behind the lock screen.
async fn sign_in_windows(c: &mut Checks, app: &Rc<App>, windows_toasts: &Rc<RefCell<Vec<String>>>) {
    let toasts = || windows_toasts.borrow().len();
    let notify = |kind: &str| app.notify("Farm was kicked", "warn", kind, new_guid(), true, None);

    // The lock screen.
    c.skip("a sign-in window is closed when the lock screen comes up", "needs a window");
    app.screen_lock();
    c.check("…the lock screen is up", app.auto_lock.screen_locked.get(), ());
    let refused = app.add_or_merge(Account { cookie: "cookie".into(), user_id: 999, username: "LateSignIn".into(), ..Default::default() });
    c.check("…and a sign-in finishing behind it saves nothing", matches!(refused, Err(Fail::User(_))), refused.as_ref().err());
    let before = toasts();
    notify("drop");
    c.check("no Windows notification behind the lock screen", toasts() == before, toasts() - before);
    let _ = call(app, "unlock", json!({ "password": PASSWORD })).await;
    notify("signin");
    c.check("…and they are shown again once it is down", toasts() == before + 1, toasts() - before);

    // The Lock button closes the vault: the window goes with it.
    c.skip("a sign-in window is closed when Lock closes the vault", "needs a window");
    let _ = call(app, "lock", json!({})).await;
}
