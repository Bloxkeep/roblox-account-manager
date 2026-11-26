// assets/i18n.json from RobloxManager/wwwroot/i18n.js (tools/i18n.mjs, with Node). Without Node the last one made stays.
use std::process::Command;

fn main() {
    one_thread_maker();
    one_web_door();
    one_entity_reader();
    no_dead_locks();
    // The window's thread gets 8 MB of stack, not Windows' 1 MB: a deep page is laid out by recursion.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/STACK:8388608");
    }
    // The logo in the exe: without it Explorer, the desktop shortcut and the taskbar show a blank file.
    println!("cargo:rerun-if-changed=app.rc");
    println!("cargo:rerun-if-changed=../RobloxManager/app.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("app.rc", embed_resource::NONE).manifest_optional().expect("app.rc (the exe's icon) didn't compile");
    }
    println!("cargo:rerun-if-changed=../RobloxManager/wwwroot/i18n.js");
    println!("cargo:rerun-if-changed=tools/i18n.mjs");
    match Command::new("node").arg("tools/i18n.mjs").status() {
        Ok(s) if s.success() => {}
        other => {
            if !std::path::Path::new("assets/i18n.json").exists() {
                panic!("assets/i18n.json is missing and tools/i18n.mjs didn't run ({other:?}); install Node");
            }
            println!("cargo:warning=tools/i18n.mjs didn't run ({other:?}); using the last assets/i18n.json");
        }
    }
}

/// Lines of src (its test suites aside, and `except`) whose code (before any `//`) holds one of `bad`.
fn lines_with(bad: &[&str], except: &str) -> Vec<String> {
    fn walk(dir: &std::path::Path, bad: &[&str], except: &str, found: &mut Vec<String>) {
        let Ok(list) = std::fs::read_dir(dir) else { return };
        for entry in list.flatten() {
            let path = entry.path();
            let shown = path.to_string_lossy().replace('\\', "/");
            if path.is_dir() {
                if !shown.ends_with("src/core/tests") {
                    walk(&path, bad, except, found);
                }
                continue;
            }
            if !shown.ends_with(".rs") || (!except.is_empty() && shown.ends_with(except)) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            for (i, line) in text.lines().enumerate() {
                let code = line.split("//").next().unwrap_or("");
                if bad.iter().any(|b| code.contains(b)) {
                    found.push(format!("{shown}:{}: {}", i + 1, line.trim()));
                }
            }
        }
    }
    let mut found = Vec::new();
    walk(std::path::Path::new("src"), bad, except, &mut found);
    found
}

/// Every web call starts from src/net.rs (Windows' proxy and certificates): the build stops on an agent made anywhere
/// else. Two that made their own failed behind an office proxy or an antivirus checking HTTPS while the rest of the app
/// worked (found 2026-11-19).
fn one_web_door() {
    let found = lines_with(&["AgentBuilder::new(", "ureq::get(", "ureq::post(", "ureq::put(", "ureq::delete(", "ureq::request(", "ureq::agent("], "src/net.rs");
    if !found.is_empty() {
        panic!("a web agent made outside src/net.rs (start from crate::net::builder, builder_direct or builder_like_roblox_client):
{}", found.join("
"));
    }
}

/// HTML's character references are read by crate::val::entities only: the build stops on a page reading `&amp;` and
/// its kin with replace() calls of its own. Each such list was short, and none read the numbered ones: Roblox's
/// messages showed "&#224;" where "à" was (found 2026-11-20).
fn one_entity_reader() {
    let found = lines_with(&["replace(\"&amp;\"", "replace(\"&#39;\"", "replace(\"&quot;\"", "replace(\"&lt;\""], "src/val.rs");
    if !found.is_empty() {
        panic!("HTML entities read outside src/val.rs (use crate::val::entities):\n{}", found.join("\n"));
    }
}

/// A lock is taken with crate::threads::Hold (`.hold()`) or `.lock().unwrap_or_else(|e| e.into_inner())`: never in a way
/// that, once some work panicked holding it, makes every later use do nothing or panic too (`.lock().ok()`,
/// `if let Ok(..) = m.lock()`, `.lock().unwrap()`, ...). One picture that panicked a decoder stopped all twelve picture
/// loaders that way (found 2026-11-19).
fn no_dead_locks() {
    let mut found = lines_with(
        &[".lock().ok()", ".lock().unwrap()", ".lock().expect(", ".lock().map(", ".lock().is_ok", ".lock().is_err", ".lock().and_then(", ".lock() else", ".read().ok()", ".write().ok()", ".read().unwrap()", ".write().unwrap()"],
        "src/threads.rs",
    );
    found.extend(lines_with(&["let Ok("], "src/threads.rs").into_iter().filter(|l| l.contains(".lock()") || l.contains(".read()") || l.contains(".write()")));
    if !found.is_empty() {
        panic!("a lock taken so that a panic elsewhere silently stops it (use crate::threads::Hold, `.hold()`):
{}", found.join("
"));
    }
}

/// Threads are made in src/threads.rs only (clippy.toml says the same for whoever runs clippy): the build stops on a
/// std::thread spawn, scope or Builder anywhere else in src (the test suites aside). NVIDIA's driver keeps ~184 bytes
/// for every thread the process ever starts, and threads made per frame, poll or web call grew Bloxkeep past 1 GB
/// (2026-11-19).
fn one_thread_maker() {
    fn walk(dir: &std::path::Path, found: &mut Vec<String>) {
        let Ok(list) = std::fs::read_dir(dir) else { return };
        for entry in list.flatten() {
            let path = entry.path();
            let shown = path.to_string_lossy().replace('\\', "/");
            if path.is_dir() {
                if !shown.ends_with("src/core/tests") {
                    walk(&path, found);
                }
                continue;
            }
            if !shown.ends_with(".rs") || shown.ends_with("src/threads.rs") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            for (i, line) in text.lines().enumerate() {
                let code = line.split("//").next().unwrap_or("");
                if ["thread::spawn", "thread::scope", "thread::Builder"].iter().any(|bad| code.contains(bad)) {
                    found.push(format!("{shown}:{}: {}", i + 1, line.trim()));
                }
            }
        }
    }
    println!("cargo:rerun-if-changed=src");
    let mut found = Vec::new();
    walk(std::path::Path::new("src"), &mut found);
    if !found.is_empty() {
        panic!("a thread made outside src/threads.rs (use crate::threads: run, call, scope, after or own):\n{}", found.join("\n"));
    }
}
