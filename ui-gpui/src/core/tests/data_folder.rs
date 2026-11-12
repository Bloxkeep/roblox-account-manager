// Tests/Bloxkeep.Tests/DataFolderSuite.cs: the data folder's layout (vault::tidy): a folder from before 1.0.11, with
// everything at the top, has its old vault copies, pictures and caches moved into old-vaults, pictures and cache at
// start; the vaults, their spare copies, settings, the log and anything not Bloxkeep's stay where they are. Then the
// uninstall's clean-ups, error.log's rotation and prefs.json held by another program.
use std::collections::BTreeSet;
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use regex::Regex;

use super::Checks;
use crate::core::program::{self, Opened};
use crate::core::{log, prefs, vault};

fn put(data: &Path, name: &str, text: &str) {
    let p = data.join(name);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).expect("folder");
    }
    std::fs::write(p, text).expect("written");
}

fn at(data: &Path, name: &str) -> bool {
    data.join(name).is_file()
}

fn read(data: &Path, name: &str) -> String {
    std::fs::read_to_string(data.join(name)).unwrap_or_default()
}

/// Held as FileShare.None (another program with the file open).
fn hold(path: &Path) -> std::fs::File {
    std::fs::OpenOptions::new().read(true).write(true).share_mode(0).open(path).expect("held")
}

pub async fn run(c: &mut Checks, _dir: &str) {
    let data = vault::data_dir().to_path_buf();
    let stay = [
        "vault.rmv",
        "vault.rmv.bak",
        "vault.rmv.tmp",
        "vault-a1b2c3d4.rmv",
        "prefs.json",
        "error.log",
        "roblox-settings.json",
        "notes.txt",
        "vault-picture-a1b2c3d4-1.png.txt",
    ];
    for name in stay {
        put(&data, name, "x");
    }
    put(&data, "vault-before-restore-20261023-103313.rmv", "before");
    put(&data, "vault-removed-20261020-090000.rmv", "removed");
    put(&data, "vault-removed-20261020-090000-spare.rmv", "x");
    put(&data, "theme-background-638700000000000000.png", "x");
    put(&data, "vault-picture-a1b2c3d4-639280667182096585.png", "picture");
    put(&data, r"webview\EBWebView\Local State", "cache");
    put(&data, r"captcha\Default\Cookies", "x");
    std::fs::create_dir_all(data.join("roblox-client-paths")).expect("folder");

    vault::tidy();

    c.check(
        "old vault copies move into old-vaults, unchanged",
        read(&data, r"old-vaults\vault-before-restore-20261023-103313.rmv") == "before"
            && read(&data, r"old-vaults\vault-removed-20261020-090000.rmv") == "removed"
            && at(&data, r"old-vaults\vault-removed-20261020-090000-spare.rmv")
            && !at(&data, "vault-before-restore-20261023-103313.rmv"),
        (),
    );
    c.check(
        "pictures move into pictures",
        at(&data, r"pictures\theme-background-638700000000000000.png")
            && at(&data, r"pictures\vault-picture-a1b2c3d4-639280667182096585.png")
            && !at(&data, "theme-background-638700000000000000.png"),
        (),
    );
    c.check(
        "caches move into cache with what's in them",
        read(&data, r"cache\webview\EBWebView\Local State") == "cache"
            && at(&data, r"cache\captcha\Default\Cookies")
            && data.join(r"cache\roblox-client-paths").is_dir()
            && !data.join("webview").is_dir(),
        (),
    );
    let moved: Vec<&str> = stay.iter().copied().filter(|n| !at(&data, n)).collect();
    c.check("the vaults with their .bak and .tmp, settings, the log and others' files stay at the top", moved.is_empty(), moved.join(", "));
    let mut top: Vec<String> = std::fs::read_dir(&data)
        .map(|d| d.flatten().filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    top.sort();
    c.check("the top holds only the three folders", top == ["cache", "old-vaults", "pictures"], top.join(", "));

    // A cache made fresh since (an older build run again): the old one at the top goes, the new one stays.
    put(&data, r"webview\old", "old");
    vault::tidy();
    c.check(
        "an old cache next to a new one is dropped",
        !data.join("webview").is_dir() && at(&data, r"cache\webview\EBWebView\Local State") && !at(&data, r"cache\webview\old"),
        (),
    );
    // An old copy whose name is already taken in old-vaults is left alone, never overwritten.
    put(&data, "vault-before-restore-20261023-103313.rmv", "second");
    vault::tidy();
    c.check(
        "a copy whose name is taken stays at the top, neither one lost",
        read(&data, "vault-before-restore-20261023-103313.rmv") == "second" && read(&data, r"old-vaults\vault-before-restore-20261023-103313.rmv") == "before",
        (),
    );
    let _ = std::fs::remove_file(data.join("vault-before-restore-20261023-103313.rmv"));

    // A prefs.json that isn't JSON any more (this one is "x"): kept aside before the next save writes the defaults over it,
    // with the vault list in it (review, 2026-10-30).
    prefs::load();
    let damaged: Vec<PathBuf> = std::fs::read_dir(&data)
        .map(|d| d.flatten().map(|e| e.path()).filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("prefs.json.damaged-"))).collect())
        .unwrap_or_default();
    c.check(
        "a prefs.json that can't be read is kept aside as it was before the defaults are saved",
        damaged.len() == 1 && std::fs::read_to_string(&damaged[0]).unwrap_or_default() == "x",
        damaged.len(),
    );
    // Held by another program as Bloxkeep starts: not damaged, so this run's defaults aren't saved over it either.
    let prefs_file = data.join("prefs.json");
    {
        let _held = hold(&prefs_file);
        prefs::load();
    }
    prefs::save();
    c.check("a prefs.json held by another program at the start isn't overwritten with defaults", read(&data, "prefs.json") == "x", read(&data, "prefs.json"));
    prefs::lock().unreadable = false;

    // Every uninstall drops the browser caches; in a folder BLOXKEEP_DATA points at (as here), a "webview" folder at the
    // top may be the person's own and stays (review, 2026-10-30: it was deleted, even with "keep my data").
    put(&data, r"webview\mine.txt", "mine");
    put(&data, r"cache\captcha\Default\Cookies", "x");
    vault::delete_caches();
    c.check(
        "uninstall's cache clean-up in a folder BLOXKEEP_DATA points at: Bloxkeep's cache goes, a webview folder at the top stays",
        at(&data, r"webview\mine.txt") && !data.join(r"cache\captcha").is_dir(),
        (),
    );
    let _ = std::fs::remove_dir_all(data.join("webview"));

    // Asked for at uninstall: Bloxkeep's files in the subfolders go too; others' files keep the folder.
    vault::delete_all_data();
    let left = all_entries(&data);
    c.check(
        "delete everything takes the subfolders' files and the emptied folders, leaving others' files",
        left == ["notes.txt", "vault-picture-a1b2c3d4-1.png.txt"],
        left.join(", "),
    );

    every_writer_is_covered(c, &data);
    log_rotates(c, &data);
    prefs_save_failure(c, &data);
    uninstall_guards(c);
}

/// Every file and folder under `data`, relative, sorted (Directory.EnumerateFileSystemEntries, AllDirectories).
fn all_entries(data: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            out.push(p.strip_prefix(root).unwrap_or(&p).to_string_lossy().into_owned());
            if p.is_dir() {
                walk(root, &p, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(data, data, &mut out);
    out.sort();
    out
}

/// Another Bloxkeep started as administrator owns the named handles: a normal start and the uninstall step can't open them
/// (no access), and go on without instead of ending with an error.
fn uninstall_guards(c: &mut Checks) {
    let mine = program::guarded(|| program::create_event(r"Local\Bloxkeep.Tests.Guarded", true));
    c.check("a handle that can be had is returned", mine.is_some(), ());
    c.check("one another user's Bloxkeep owns (no access) is null, not an exception", program::guarded(|| Opened::Denied).is_none(), ());
    // PORT: Opened has no other failure than no access or nothing there; nothing else can come out of `guarded`.
    c.skip("any other failure still shows", "the port's guarded has no other failure to let through (Opened is Handle, Denied or Missing)");

    program::close_running_app_with(|| Opened::Denied, || Opened::Denied);
    c.check("uninstall: a running Bloxkeep it has no access to is skipped, the hook goes on to delete", true, ());
    const NAME: &str = r"Local\Bloxkeep.Tests.Quit";
    let quit = program::create_event(NAME, true);
    program::close_running_app_with(|| program::open_event(NAME), || Opened::Denied);
    let set = match &quit {
        Opened::Handle(q) => q.wait(0).0,
        _ => false,
    };
    c.check("…asked to quit when it can be reached, and the hook goes on if the rest can't be", set, ());
}

/// The names every writer gives its file in the data folder, read out of the program's own source.
// PORT: the Rust port's source (ui-gpui/src), `data_dir().join("name")`, as the C# read RobloxManager's
// `Path.Combine(Vault.DataDir, "name")`; the folders (cache, old-vaults, pictures) are joined the same way here, so only
// names with an extension (files) count.
fn written_names() -> Vec<String> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let pattern = Regex::new(r#"data_dir\(\)\s*\.join\(\s*"([^"{}\\]+)"\s*\)"#).unwrap();
    let mut names: BTreeSet<String> = BTreeSet::new();
    fn walk(dir: &Path, f: &mut dyn FnMut(&Path)) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                // The program's source only, as the C# read RobloxManager's and not the tests' (a check names files too).
                if p.ends_with(Path::new("core").join("tests")) {
                    continue;
                }
                walk(&p, f);
            } else if p.extension().is_some_and(|x| x == "rs") {
                f(&p);
            }
        }
    }
    walk(&source, &mut |p| {
        let text = std::fs::read_to_string(p).unwrap_or_default();
        for m in pattern.captures_iter(&text) {
            let name = m[1].to_string();
            if name.contains('.') {
                names.insert(name);
            }
        }
    });
    names.into_iter().collect()
}

/// "Delete my accounts and settings" leaves no file of Bloxkeep's, whichever part of the program wrote it, and still
/// nothing that isn't Bloxkeep's.
fn every_writer_is_covered(c: &mut Checks, data: &Path) {
    let _ = std::fs::remove_dir_all(data);
    std::fs::create_dir_all(data).expect("folder");
    let written = written_names();
    let has = |n: &str| written.iter().any(|w| w == n);
    c.check(
        "the source names the files Bloxkeep writes into the data folder (the scan isn't empty)",
        written.len() >= 8 && has("game-hours.json") && has("roblox-flags.json") && has("hidden-windows.txt") && has("window-icons.txt"),
        written.join(", "),
    );
    let mut made: Vec<String> = Vec::new();
    let extra = [
        "error.log.1",
        "prefs.json.damaged-20261101-120000",
        "roblox-flags.json.damaged-20261101-120000",
        "roblox-settings.json.damaged-20261101-120000",
        "vault-a1b2c3d4.rmv",
        "vault-a1b2c3d4.rmv.bak",
    ];
    for name in written.iter().map(String::as_str).chain(extra) {
        std::fs::write(data.join(name), "x").expect("written");
        made.push(name.to_string());
        // The temp file a write goes through, and one a crash left behind.
        if has(name) {
            std::fs::write(data.join(format!("{name}.tmp")), "x").expect("written");
            made.push(format!("{name}.tmp"));
        }
    }
    // Not Bloxkeep's, though they look alike (BLOXKEEP_DATA may be a folder of the person's own).
    let mut others = vec!["game-hours.json5", "hidden-windows.txt.bak", "my-error.log", "error.log.2", "roblox-flags.json.old", "window-icons.txt.tmp.old"];
    for name in &others {
        std::fs::write(data.join(name), "mine").expect("written");
    }
    vault::delete_all_data();
    let mut remains: Vec<String> = std::fs::read_dir(data).map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
    remains.sort();
    others.sort();
    c.check(
        "delete everything takes a file of every writer (and its .tmp, error.log.1, the damaged copies) and no other",
        remains == others,
        remains.join(", "),
    );
    for name in &others {
        let _ = std::fs::remove_file(data.join(name));
    }
    for name in &made {
        let _ = std::fs::remove_file(data.join(name));
    }
    for name in made.iter().take(5) {
        let _ = std::fs::write(data.join(name), "x");
    }
    vault::delete_all_data();
    c.check("…and with only Bloxkeep's files in it, the folder goes too", !data.exists(), ());
    std::fs::create_dir_all(data).expect("folder");
}

/// error.log at its limit is kept as error.log.1 (the history a bug report needs), not deleted.
fn log_rotates(c: &mut Checks, data: &Path) {
    let log_file = data.join("error.log");
    let spare = data.join("error.log.1");
    for i in 0..300 {
        log::note(&format!("line{i} {}", "a".repeat(1000)));
    }
    let kept = spare.is_file();
    c.check(
        "an error.log over its limit is kept as error.log.1",
        kept && std::fs::read_to_string(&spare).unwrap_or_default().contains("line0 "),
        kept,
    );
    let text = std::fs::read_to_string(&log_file).unwrap_or_default();
    c.check(
        "…and the log goes on in a new error.log with the newest lines",
        log_file.is_file() && text.contains("line299 ") && !text.contains("line0 ") && std::fs::metadata(&log_file).map(|m| m.len()).unwrap_or(u64::MAX) < 256 * 1024,
        (),
    );
    for i in 300..700 {
        log::note(&format!("line{i} {}", "b".repeat(1000)));
    }
    c.check(
        "a second time the spare is replaced, not added to",
        spare.is_file()
            && std::fs::metadata(&spare).map(|m| m.len()).unwrap_or(u64::MAX) < 300 * 1024
            && !std::fs::read_to_string(&spare).unwrap_or_default().contains("line0 "),
        (),
    );
    let _ = std::fs::remove_file(&log_file);
    let _ = std::fs::remove_file(&spare);

    // A session cookie or a webhook link inside a message never reaches the file.
    log::note("request failed for _|WARNING:-DO-NOT-SHARE-THIS.--Sharing-this-will-allow-someone-to-log-in-as-you.|_ABC123 at https://discord.com/api/webhooks/123/abcDEF done");
    let written = std::fs::read_to_string(&log_file).unwrap_or_default();
    c.check(
        "a cookie or a webhook link in a logged message is taken out",
        !written.contains("ABC123") && !written.contains("abcDEF") && written.contains("request failed for") && written.contains("done"),
        &written,
    );
    let _ = std::fs::remove_file(&log_file);
}

/// prefs.json held by another program (an antivirus, a sync client) for a moment: save tries again, says so once, and
/// tells the caller; it keeps trying on the next save.
fn prefs_save_failure(c: &mut Checks, data: &Path) {
    let file = data.join("prefs.json");
    let log_file = data.join("error.log");
    let lines = || std::fs::read_to_string(&log_file).map(|t| t.lines().filter(|l| l.contains("prefs.json couldn't be saved")).count()).unwrap_or(0);
    prefs::lock().tray_hint_shown = false;
    let ok = prefs::save();
    c.check("a normal Save says it worked", ok && !prefs::lock().save_failed(), ());
    prefs::lock().tray_hint_shown = true;
    {
        let _held = hold(&file);
        let ok = prefs::save();
        c.check("a prefs.json held for good: Save says it failed", !ok && prefs::lock().save_failed(), ());
        c.check("…and writes it in error.log once", lines() == 1, lines());
        prefs::save();
        c.check("…not again for the next failed Save", lines() == 1, lines());
    }
    let ok = prefs::save();
    c.check(
        "let go: the next Save works, with what was changed meanwhile",
        ok && !prefs::lock().save_failed() && std::fs::read_to_string(&file).unwrap_or_default().contains("\"trayHintShown\":true"),
        (),
    );
    // Let go within a moment: the retries get through, and nothing is reported.
    let before = lines();
    prefs::lock().tray_hint_shown = false;
    let held = hold(&file);
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(120));
        drop(held);
    });
    let ok = prefs::save();
    let _ = release.join();
    c.check(
        "held for a moment only: Save tries again and works",
        ok && !prefs::lock().save_failed() && std::fs::read_to_string(&file).unwrap_or_default().contains("\"trayHintShown\":false"),
        (),
    );
    c.check("…and reports nothing", lines() == before, lines());
}
