// Tests/Bloxkeep.Tests/LockWallSuite.cs: the wall behind the lock screen (app/auto_lock.rs, SCREEN_LOCK_COMMANDS): every
// command the page can send, but the few the lock screen itself uses, is refused with "Bloxkeep is locked." and does
// nothing: the page's list is read from the source (so a command added later is under the check at once), the vault,
// the settings file, the posts and the running and pending launches are the same afterwards, the tray's entry points
// stay no-ops. A command that is no command is refused the same way.
//
// PORT: the command list is read from the port's dispatch (app/controller.rs, dispatch_command), as the C# read
// AppController.cs's DispatchCommandAsync. The four checks of the C# test harness's own reflection lookups have no
// counterpart (the port's fields are reached directly) and are skipped.
use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use regex::Regex;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::rejoin::quiet_app;
use super::{Checks, PASSWORD, call, unlock};
use crate::core::app::controller::SCREEN_LOCK_COMMANDS;
use crate::core::base::new_guid;
use crate::core::models::{Account, LaunchPreset, to_net_json};
use crate::core::vault::{self, Vault};
use crate::core::{prefs, rt};

const LOCKED: &str = "Bloxkeep is locked.";

fn hash_bytes(b: &[u8]) -> String {
    Sha256::digest(b).iter().map(|x| format!("{x:02X}")).collect()
}

fn hash(path: &Path) -> String {
    std::fs::read(path).map(|b| hash_bytes(&b)).unwrap_or_else(|_| "absent".into())
}

/// The app's own timers may push the lock screen's state at any moment (that is allowed: it names nothing); a toast,
/// another event or a state with accounts in it is not.
fn lock_screen_state(post: &str) -> bool {
    post.starts_with("{\"type\":\"event\",\"name\":\"state\"") && post.contains("\"locked\":true") && post.contains("\"accounts\":[]")
}

fn cut(p: &str, n: usize) -> String {
    p.chars().take(n).collect()
}

pub async fn run(c: &mut Checks, _dir: &str) {
    // The command list, as the source has it: the match in dispatch_command.
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("core").join("app").join("controller.rs");
    let mut commands: Vec<String> = Vec::new();
    // A checkout with Windows line ends (GitHub's build machines) reads the same as one without.
    if let Ok(text) = std::fs::read_to_string(&source).map(|t| t.replace("\r\n", "\n"))
        && let Some(start) = text.find("async fn dispatch_command(")
        && let Some(len) = text[start..].find("\n        }\n    }")
    {
        let pattern = Regex::new(r#"(?m)^            "(\w+)" =>"#).unwrap();
        for m in pattern.captures_iter(&text[start..start + len]) {
            if !commands.contains(&m[1].to_string()) {
                commands.push(m[1].to_string());
            }
        }
    }
    c.check(
        "the command list was read from AppController.cs (more than 100 names)",
        commands.len() > 100,
        format!("{} names from {}", commands.len(), source.display()),
    );
    let allowed: Vec<&str> = SCREEN_LOCK_COMMANDS.to_vec();
    let typos: Vec<&str> = allowed.iter().copied().filter(|a| !commands.iter().any(|n| n == a)).collect();
    c.check("every name the lock screen may use is a command (no typo in the allow-list)", typos.is_empty(), typos.join(", "));
    let mut sorted = allowed.clone();
    sorted.sort();
    c.check(
        "the allow-list is the few the lock screen needs (at most 12), none of them reading or changing accounts",
        !allowed.is_empty()
            && allowed.len() <= 12
            && !allowed.iter().any(|a| a.contains("ccount") || a.starts_with("reveal") || a.starts_with("launch") || a.starts_with("export")),
        sorted.join(", "),
    );
    let walled: Vec<String> = commands.iter().filter(|n| !allowed.contains(&n.as_str())).cloned().collect();

    // A vault with an account and a preset to find on the page, a settings file written.
    let secret = Account {
        username: "WallUser77".into(),
        alias: "WallAlias77".into(),
        user_id: 7777,
        cookie: format!("_TEST_WALL_{}", new_guid().simple()),
        ..Default::default()
    };
    let (secret_id, secret_cookie) = (secret.id, secret.cookie.clone());
    let preset = LaunchPreset { name: "WallPreset".into(), account_ids: vec![secret.id], ..Default::default() };
    let preset_id = preset.id;
    {
        let seed = Vault::create(PASSWORD, None).expect("a vault");
        {
            let mut d = seed.data();
            d.accounts.push(secret);
            d.settings.presets.push(preset);
        }
        seed.save().expect("saved");
    }
    prefs::save();

    let notices = Rc::new(Cell::new(0));
    let (app, posts) = quiet_app();
    {
        let notices = notices.clone();
        *app.notify.show_notification.borrow_mut() = Some(Rc::new(move |_: &str, _: &str| {
            notices.set(notices.get() + 1);
            true
        }));
    }
    unlock(&app).await;
    app.controller.page_attached.set(true);
    let vault = app.vault_ref().expect("open");

    for what in [
        "Harness: a dotted path goes through a field (_discordPresence._disposed)",
        "Harness: a name only one of the parts has is found there (_nextTry, in the Discord presence)",
        "Harness: a name the owner has itself is its own (_screenLocked)",
        "Harness: a name nothing has is said clearly",
    ] {
        c.skip(what, "the C# test harness's reflection; the port reaches fields directly");
    }

    app.screen_lock();
    c.check(
        "the lock screen is up, the vault open behind it",
        app.auto_lock.screen_locked.get() && app.vault_ref().is_some_and(|v| std::sync::Arc::ptr_eq(&v, &vault)),
        (),
    );

    let snapshot = || {
        let mut files: Vec<String> = std::fs::read_dir(vault::data_dir())
            .map(|d| d.flatten().filter(|e| e.path().is_file()).map(|e| e.file_name().to_string_lossy().into_owned()).collect())
            .unwrap_or_default();
        files.sort();
        format!(
            "{} | {} | {} | {} | {} | {}",
            hash(&vault::vault_path()),
            hash(&vault::data_dir().join("prefs.json")),
            hash_bytes(&to_net_json(&*vault.data()).unwrap_or_default()),
            files.join(","),
            app.controller.running.borrow().len(),
            app.controller.pending.borrow().len()
        )
    };
    let since = |count: usize| -> Vec<String> { posts.borrow().iter().skip(count).cloned().collect() };

    let before = snapshot();
    let posts_before = posts.borrow().len();

    // Each by itself (an empty answer, whatever the command would have needed).
    let mut not_refused: Vec<String> = Vec::new();
    for name in &walled {
        let r = call(&app, name, json!({})).await;
        match &r {
            Err(crate::core::base::Fail::User(m)) if m == LOCKED => {}
            Ok(_) => not_refused.push(format!("{name}: answered")),
            Err(e) => not_refused.push(format!("{name}: {}", e.message())),
        }
    }
    c.check(
        &format!("behind the lock screen, each of the {} other commands is refused with \"{LOCKED}\"", walled.len()),
        not_refused.is_empty() && walled.len() > 100,
        not_refused.join("; "),
    );
    let none = call(&app, "noSuchCommand", json!({})).await;
    c.check(
        "…and a name that is no command is refused the same way (the wall doesn't tell which names exist)",
        matches!(&none, Err(crate::core::base::Fail::User(m)) if m == LOCKED),
        &none,
    );
    c.check(
        "…the vault file, the settings file, the vault in memory, the files in the folder and the launches are the same afterwards",
        snapshot() == before,
        snapshot(),
    );
    let got = since(posts_before);
    c.check(
        "…and nothing reached the page but the lock screen's own state",
        got.iter().all(|p| lock_screen_state(p)),
        got.iter().filter(|p| !lock_screen_state(p)).map(|p| cut(p, 100)).collect::<Vec<_>>().join(" // "),
    );
    c.check("…and no Windows notification", notices.get() == 0, notices.get());

    // The same through the page's own door: the reply says why.
    let mut wrong_reply: Vec<String> = Vec::new();
    let mut id = 100;
    for name in &walled {
        let at = posts.borrow().len();
        id += 1;
        app.handle_message(&format!("{{\"id\":{id},\"cmd\":\"{name}\",\"args\":{{}}}}")).await;
        let got = since(at);
        let head = format!("{{\"type\":\"reply\",\"id\":{id},");
        let replies: Vec<&String> = got.iter().filter(|p| p.starts_with(&head)).collect();
        let others_ok = got.iter().filter(|p| !p.starts_with(&head)).all(|p| lock_screen_state(p));
        if replies.len() != 1 || !(replies[0].contains("\"ok\":false") && replies[0].contains(LOCKED)) || !others_ok {
            wrong_reply.push(format!("{name}: {}", got.iter().map(|p| cut(p, 80)).collect::<Vec<_>>().join(" // ")));
        }
    }
    c.check(
        &format!("sent through the page's message door, each gets one reply: not ok, \"{LOCKED}\""),
        wrong_reply.is_empty(),
        wrong_reply.join("; "),
    );
    c.check("…and still nothing changed", snapshot() == before, snapshot());
    let answers: String = since(posts_before).concat();
    c.check(
        "…and no account name or cookie is in anything the page got",
        !answers.contains("WallUser77") && !answers.contains("WallAlias77") && !answers.contains(&secret_cookie),
        (),
    );

    // The tray, the mini window and the panels have doors of their own.
    let posts_after_replies = posts.borrow().len();
    {
        let a = app.clone();
        rt::spawn(async move { a.tray_open_account(secret_id).await });
        let a = app.clone();
        rt::spawn(async move { a.tray_run_preset(preset_id).await });
    }
    app.tray_toggle_panel(new_guid());
    rt::sleep(Duration::from_millis(300)).await; // the first two are spawned: a start would show within this
    let later = since(posts_after_replies);
    let only_locked_state = later.iter().all(|p| lock_screen_state(p));
    let same = snapshot() == before;
    c.check(
        "the tray's Open and Run preset do nothing behind the lock screen: no launch, no change, no toast",
        same && notices.get() == 0 && only_locked_state,
        format!(
            "same {same}, notices {}, {} posts, only the locked state {only_locked_state}: {}",
            notices.get(),
            later.len(),
            later.iter().filter(|p| !lock_screen_state(p)).map(|p| cut(p, 120)).collect::<Vec<_>>().join(" // ")
        ),
    );
    let tray = app.tray_state();
    c.check(
        "the tray menu counts as closed, with no presets or clients",
        !tray.open && tray.presets.is_empty(),
        format!("open {}, {} presets", tray.open, tray.presets.len()),
    );
    c.check(
        "…and lists no macros and no running clients",
        app.tray_macros().is_empty() && app.running_clients().is_empty() && !app.unlocked(),
        (),
    );

    // Those it may answer: not the wall's refusal (their own answer, or their own complaint).
    let mut refused_by_wall: Vec<&str> = Vec::new();
    for name in sorted.iter().copied().filter(|a| *a != "lock") {
        if let Err(crate::core::base::Fail::User(m)) = call(&app, name, json!({})).await
            && m == LOCKED
        {
            refused_by_wall.push(name);
        }
    }
    c.check("the lock screen's own commands are not refused by the wall", refused_by_wall.is_empty(), refused_by_wall.join(", "));
    let state = call(&app, "getState", json!({})).await.unwrap_or(Value::Null).to_string();
    c.check(
        "getState behind it names no account and no cookie",
        !state.contains("WallUser77") && !state.contains("WallAlias77") && !state.contains(&secret_cookie),
        (),
    );
}
