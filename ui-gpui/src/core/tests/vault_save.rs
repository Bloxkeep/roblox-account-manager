// Tests/Bloxkeep.Tests/VaultSaveSuite.cs: saves written in the background (Vault::save_in_background, the App's usual
// save): what reaches the disk is the vault as it was when the save was asked for, a newer save wins over one still
// waiting, a save made now or a changed password is never overwritten by an older one still on its way, closing the
// vault finishes the write first, and a file that can't be written is reported and written by the next save.
use std::os::windows::fs::OpenOptionsExt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::{Checks, PASSWORD};
use crate::core::app::persist::is_io_error;
use crate::core::base::Fail;
use crate::core::models::Account;
use crate::core::rt;
use crate::core::vault::{self, Vault};

/// The first account's alias in the file as `password` opens it.
fn alias(password: &str) -> String {
    match Vault::open(password, None) {
        Ok(opened) => opened.data().accounts.first().map(|a| a.alias.clone()).unwrap_or_default(),
        Err(f) if vault::is_invalid_password(&f) => format!("(the file doesn't open with {password})"),
        Err(f) => format!("({})", f.message()),
    }
}

fn set_alias(v: &Vault, text: &str) {
    v.data().accounts[0].alias = text.to_string();
}

fn wait_flag(flag: &AtomicBool, timeout: Duration) -> bool {
    let until = Instant::now() + timeout;
    while !flag.load(Ordering::SeqCst) {
        if Instant::now() >= until {
            return false;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    true
}

/// A background save of the vault as it is now, held once it has taken the vault and before it reaches the file (where
/// a save made now can take the file first), while what comes meanwhile is done; false when it never got there.
fn held_while(v: &Vault, meanwhile: impl FnOnce()) -> bool {
    let reached = Arc::new(AtomicBool::new(false));
    let go = Arc::new(AtomicBool::new(false));
    {
        let (reached, go) = (reached.clone(), go.clone());
        vault::set_before_background_write(Some(Arc::new(move || {
            reached.store(true, Ordering::SeqCst);
            wait_flag(&go, Duration::from_secs(10));
        })));
    }
    let save = v.save_in_background().expect("a background save");
    let held = wait_flag(&reached, Duration::from_secs(10));
    meanwhile();
    go.store(true, Ordering::SeqCst);
    let _ = save.wait();
    vault::set_before_background_write(None);
    held
}

pub async fn run(c: &mut Checks, _dir: &str) {
    let v = Vault::create(PASSWORD, None).expect("a vault");
    v.data().accounts.push(Account { username: "bg_save_user".into(), alias: "first".into(), user_id: 77, ..Default::default() });

    // What the save took, not what the vault became after it.
    let first = v.save_in_background().expect("saved");
    set_alias(&v, "changed after the save was asked for");
    let _ = first.wait();
    c.check("a background save writes the vault as it was when asked for", alias(PASSWORD) == "first", alias(PASSWORD));
    c.check("…and says when it's done", !v.saving_in_background(), ());

    // Many saves in a row: the newest is on disk once they are done.
    let mut saves = Vec::new();
    for i in 1..=40 {
        set_alias(&v, &format!("save {i}"));
        saves.push(v.save_in_background().expect("saved"));
    }
    for s in &saves {
        let _ = s.wait();
    }
    c.check("forty saves in a row: the last one is on disk", alias(PASSWORD) == "save 40", alias(PASSWORD));

    // A save made now, right after one asked for in the background: the older one doesn't come after it.
    set_alias(&v, "older, in the background");
    let older = v.save_in_background().expect("saved");
    set_alias(&v, "newer, saved now");
    v.save().expect("saved now");
    let _ = older.wait();
    rt::sleep(Duration::from_millis(200)).await;
    c.check("a save made now isn't overwritten by an older one from the background", alias(PASSWORD) == "newer, saved now", alias(PASSWORD));

    // The same with the older one already on its way: it had taken the vault but not yet the file when the save made now
    // wrote. Held here, it happens every time.
    set_alias(&v, "older, held on its way");
    let held = held_while(&v, || {
        set_alias(&v, "newer, saved while the older was held");
        v.save().expect("saved now");
    });
    c.check(
        "…nor by an older one that was on its way to the file",
        held && alias(PASSWORD) == "newer, saved while the older was held",
        alias(PASSWORD),
    );

    // A changed password while a save under the old one waits: the file ends under the new password only.
    set_alias(&v, "under the old password");
    let stale = v.save_in_background().expect("saved");
    set_alias(&v, "under the new password");
    v.change_password(Vault::prepare_key("harness-new-pass-2")).expect("changed");
    let _ = stale.wait();
    rt::sleep(Duration::from_millis(200)).await;
    let old_opens = match Vault::open(PASSWORD, None) {
        Ok(_) => true,
        Err(f) if vault::is_invalid_password(&f) => false,
        Err(_) => true,
    };
    c.check("a changed password: the old one no longer opens the file", !old_opens, ());
    c.check("…and the new one opens what was saved under it", alias("harness-new-pass-2") == "under the new password", alias("harness-new-pass-2"));

    // …nor when the save under the old one was already on its way: the new password still opens the file.
    set_alias(&v, "under the second password, on its way");
    let held = held_while(&v, || {
        set_alias(&v, "under the third password");
        v.change_password(Vault::prepare_key("harness-new-pass-3")).expect("changed");
    });
    c.check(
        "…and when a save under the old one was on its way meanwhile",
        held && alias("harness-new-pass-3") == "under the third password",
        alias("harness-new-pass-3"),
    );

    // Closing right after a save was asked for: it is written before the key goes.
    set_alias(&v, "saved just before closing");
    let _closing = v.save_in_background();
    drop(v);
    c.check("closing the vault finishes the save on its way", alias("harness-new-pass-3") == "saved just before closing", alias("harness-new-pass-3"));

    // A file held by another program (an antivirus, a sync client): the save fails and says so; the next one writes.
    let again = Vault::open("harness-new-pass-3", None).expect("opens");
    set_alias(&again, "while the file is held");
    let failure: Option<Fail> = {
        let _held = std::fs::OpenOptions::new().read(true).share_mode(0).open(vault::vault_path()).expect("held");
        again.save_in_background().and_then(|t| t.wait()).err()
    };
    c.check("a file another program holds: the save fails with the reason", failure.as_ref().is_some_and(is_io_error), &failure);
    let _ = again.save_in_background().and_then(|t| t.wait());
    c.check("…and the next save writes it", alias("harness-new-pass-3") == "while the file is held", alias("harness-new-pass-3"));
    again.save().expect("saved"); // the save before is now the .bak, under the same password
    drop(again);

    // The main file's salt damaged (it still looks like a vault): the previous save opens with the typed password
    // (review, 2026-10-30: the .bak was tried only with the main file's salt, so every unlock failed).
    let mut main = std::fs::read(vault::vault_path()).expect("read");
    main[8] ^= 0xFF;
    std::fs::write(vault::vault_path(), &main).expect("written");
    c.check(
        "a main file with a damaged salt: opened from the previous save with the password",
        alias("harness-new-pass-3") == "while the file is held",
        alias("harness-new-pass-3"),
    );
    c.check("…a wrong password still doesn't open it", alias("not the password").starts_with("(the file"), alias("not the password"));
}
