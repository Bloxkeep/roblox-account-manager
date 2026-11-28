// The C# tests (Tests/Bloxkeep.Tests) moved to Rust, suite by suite under the same names: a suite drives the core's
// App from outside as the C# harness drove AppController, on a throwaway data folder, with no window, no Roblox and no
// other program started. `--core-suite NAME DIR` runs one suite in this process; `--core-suites [NAMES]` runs each in a
// child process of its own (as Program.cs did), with a time limit.
use std::{cell::RefCell, rc::Rc, time::Duration};

use serde_json::Value;

use crate::core::{
    app::App,
    base::{Fail, Res},
    rt,
};

pub mod afk;
pub mod restart;
pub mod rejoin;
pub mod rejoin_place;
pub mod window_rules;
pub mod sound;
pub mod same_server;
pub mod limited_deals;
pub mod window_loan;
pub mod window_plan;
pub mod close_plan;
pub mod roblox_words;
pub mod words;
pub mod follow;
pub mod private_server;
pub mod claims;
pub mod preset;
pub mod window;
pub mod arrange;
pub mod watch;
pub mod feedback;
pub mod online;
pub mod remote;
pub mod latest;
pub mod roblox_update;
pub mod game_target;
pub mod played_games;
pub mod game_accounts;
pub mod discord;
pub mod website;
pub mod dialog;
pub mod tray;
pub mod auto_lock;
pub mod vault_save;
pub mod main_browser;
pub mod cores;
pub mod robux;
pub mod saved_servers;
pub mod discord_presence;
pub mod quiet_fix;
pub mod lock_update;
pub mod custom_theme;
pub mod vaults;
pub mod data_folder;
pub mod rejoin_with;
pub mod bot_check;
pub mod server_hop;
pub mod launch_choice;
pub mod hide_windows;
pub mod hidden_desktop;
pub mod ram_limit;
pub mod flags;
pub mod fps_cap;
pub mod roblox_usage;
pub mod game_page;
pub mod window_shot;
pub mod monitor;
pub mod foreign_vault;
pub mod window_icons;
pub mod taskbar_groups;
pub mod home_session;
pub mod sign_ins;
pub mod tick_time;
pub mod schedule;
pub mod play_losses;
pub mod play_heat;
pub mod macros;
pub mod input_turn;
pub mod server_pick;
pub mod adopt;
pub mod delete_accounts;
pub mod shutdown;
pub mod tick;
pub mod totp;
pub mod lock_wall;
pub mod vault_crash;
pub mod stats_icon;

/// PASS/FAIL lines and the count of failures (Harness.cs's Checks).
#[derive(Default)]
pub struct Checks {
    pub failures: i32,
    pub passes: i32,
    /// Checks left out on purpose (a window, Roblox or the network would be needed), said once each.
    pub skipped: i32,
}

impl Checks {
    pub fn check(&mut self, what: &str, ok: bool, detail: impl std::fmt::Debug) {
        if ok {
            self.passes += 1;
            println!("PASS  {what}");
        } else {
            self.failures += 1;
            println!("FAIL  {what}  [{detail:?}]");
        }
    }

    /// A check the C# suite had that can't run here (it opens a window, starts a program or needs the network).
    pub fn skip(&mut self, what: &str, why: &str) {
        self.skipped += 1;
        println!("SKIP  {what}  ({why})");
    }
}

pub const PASSWORD: &str = "harness-pass-1";

/// A fresh data folder; runs before anything reads the data folder or Prefs. Refuses the real one.
pub fn use_data_folder(dir: &str) {
    let real = std::env::var("LOCALAPPDATA").map(|l| std::path::Path::new(&l).join("Bloxkeep")).ok();
    let dir_path = std::path::Path::new(dir);
    assert!(
        real.as_ref().is_none_or(|r| !dir_path.starts_with(r)) && !dir.is_empty(),
        "the tests never run on the real data folder"
    );
    let _ = std::fs::remove_dir_all(dir_path);
    std::fs::create_dir_all(dir_path).expect("data folder");
    // SAFETY: set before any other thread of this process reads the environment (the suite's process is its own).
    unsafe { std::env::set_var("BLOXKEEP_DATA", dir) };
    crate::core::prefs::load();
    crate::core::vaults::load();
}

/// What the App told the interface (each line `emit` sent), kept for checks.
pub type Said = Rc<RefCell<Vec<String>>>;

/// A started App with no window: what it tells the interface is kept in the returned list.
pub fn app() -> (Rc<App>, Said) {
    let said: Said = Rc::new(RefCell::new(Vec::new()));
    let keep = said.clone();
    let app = App::start();
    app.init(0, Rc::new(move |line: &str| keep.borrow_mut().push(line.to_string())));
    (app, said)
}

/// A command as the interface sends it (DispatchAsync): its answer, or the error it was refused with.
pub async fn call(app: &Rc<App>, command: &str, args: Value) -> Res<Value> {
    app.handle(command, &args).await
}

/// A refusal's words, or None when the command went through.
pub fn user_error(result: &Res<Value>) -> Option<String> {
    match result {
        Err(Fail::User(m)) => Some(m.clone()),
        _ => None,
    }
}

/// Unlocks with the harness password (a new vault is made by the first unlock); tried again as a person would when a
/// scanner holds the file a moment.
pub async fn unlock(app: &Rc<App>) {
    for attempt in 1.. {
        match call(app, "unlock", serde_json::json!({ "password": PASSWORD })).await {
            Err(Fail::User(m)) if attempt < 5 && m.contains("another program may be using it") => rt::sleep(Duration::from_millis(500)).await,
            Err(e) => panic!("unlock failed: {e}"),
            Ok(_) => return,
        }
    }
}

/// Every suite, in Program.cs's order, with whether it needs the network (left out unless asked by name).
pub const SUITES: &[(&str, bool)] = &[("afk", false), ("restart", false), ("rejoin", false), ("follow", false), ("private-servers", false), ("claims", false), ("presets", false), ("window", false), ("arrange", false), ("watch", false), ("feedback", false), ("online", false), ("remote", false), ("latest", false), ("roblox-update", false), ("game-target", false), ("played-games", false), ("game-accounts", false), ("discord", false), ("website", false), ("dialog", false), ("tray", false), ("autolock", false), ("vault-save", false), ("main-browser", false), ("cores", false), ("robux", false), ("saved-servers", false), ("discord-presence", false), ("quiet-fixes", false), ("lock-update", false), ("custom-theme", false), ("vaults", false), ("data-folder", false), ("rejoin-with", false), ("rejoin-place", false), ("window-rules", false), ("window-plan", false), ("close-plan", false), ("roblox-words", false), ("words", false), ("bot-check", false), ("server-hop", false), ("launch-choice", false), ("hide-windows", false), ("ram-limit", false), ("flags", false), ("fps-cap", false), ("roblox-usage", false), ("game-page", false), ("window-shot", false), ("monitor", false), ("foreign-vault", false), ("window-icons", false), ("taskbar-groups", false), ("home-session", false), ("sign-ins", false), ("tick-time", false), ("schedule", false), ("play-losses", false), ("play-heat", false), ("macros", false), ("input-turn", false), ("server-pick", false), ("adopt", false), ("delete-accounts", false), ("shutdown", false), ("tick", false), ("totp", false), ("lock-wall", false), ("vault-crash", false), ("window-loan", false), ("sound", false), ("same-server", false), ("stats-icons", true), ("watch-online", true), ("limited-deals", true)];

/// One suite in this process, on the core's thread: the exit code is the count of failures.
pub fn run_one(name: &str, dir: &str) -> i32 {
    if !SUITES.iter().any(|(n, _)| *n == name) {
        println!("No suite named {name}.");
        return 1000;
    }
    use_data_folder(dir);
    let name = name.to_string();
    let dir = dir.to_string();
    let result = std::thread::Builder::new()
        .name("core".into())
        .spawn(move || {
            let out = Rc::new(RefCell::new(Checks::default()));
            let inner = out.clone();
            rt::run(move || async move {
                let mut c = Checks::default();
                run_suite(&name, &mut c, &dir).await;
                *inner.borrow_mut() = c;
            });
            let c = out.take();
            (c.failures, c.passes, c.skipped)
        })
        .expect("core thread")
        .join();
    match result {
        Ok((failures, passes, skipped)) => {
            println!("{} ({passes} passed, {skipped} skipped)", if failures == 0 { "ALL PASS".to_string() } else { format!("{failures} FAILED") });
            failures
        }
        Err(_) => {
            println!("FAIL  the suite ran to its end  [panicked]");
            1
        }
    }
}

async fn run_suite(name: &str, c: &mut Checks, dir: &str) {
    match name {
        "afk" => afk::run(c, dir).await,
        "restart" => restart::run(c, dir).await,
        "rejoin" => rejoin::run(c, dir).await,
        "follow" => follow::run(c, dir).await,
        "private-servers" => private_server::run(c, dir).await,
        "claims" => claims::run(c, dir).await,
        "presets" => preset::run(c, dir).await,
        "window" => window::run(c, dir).await,
        "arrange" => arrange::run(c, dir).await,
        "watch" => watch::run(c, dir).await,
        "feedback" => feedback::run(c, dir).await,
        "online" => online::run(c, dir).await,
        "remote" => remote::run(c, dir).await,
        "latest" => latest::run(c, dir).await,
        "roblox-update" => roblox_update::run(c, dir).await,
        "game-target" => game_target::run(c, dir).await,
        "played-games" => played_games::run(c, dir).await,
        "game-accounts" => game_accounts::run(c, dir).await,
        "discord" => discord::run(c, dir).await,
        "website" => website::run(c, dir).await,
        "dialog" => dialog::run(c, dir).await,
        "tray" => tray::run(c, dir).await,
        "autolock" => auto_lock::run(c, dir).await,
        "vault-save" => vault_save::run(c, dir).await,
        "main-browser" => main_browser::run(c, dir).await,
        "cores" => cores::run(c, dir).await,
        "robux" => robux::run(c, dir).await,
        "saved-servers" => saved_servers::run(c, dir).await,
        "discord-presence" => discord_presence::run(c, dir).await,
        "quiet-fixes" => quiet_fix::run(c, dir).await,
        "lock-update" => lock_update::run(c, dir).await,
        "custom-theme" => custom_theme::run(c, dir).await,
        "vaults" => vaults::run(c, dir).await,
        "data-folder" => data_folder::run(c, dir).await,
        "rejoin-with" => rejoin_with::run(c, dir).await,
        "rejoin-place" => rejoin_place::run(c, dir).await,
        "window-rules" => window_rules::run(c, dir).await,
        "sound" => sound::run(c, dir).await,
        "same-server" => same_server::run(c, dir).await,
        "limited-deals" => limited_deals::run(c, dir).await,
        "window-loan" => window_loan::run(c, dir).await,
        "window-plan" => window_plan::run(c, dir).await,
        "close-plan" => close_plan::run(c, dir).await,
        "roblox-words" => roblox_words::run(c, dir).await,
        "words" => words::run(c, dir).await,
        "bot-check" => bot_check::run(c, dir).await,
        "server-hop" => server_hop::run(c, dir).await,
        "launch-choice" => launch_choice::run(c, dir).await,
        "hide-windows" => hide_windows::run(c, dir).await,
        "ram-limit" => ram_limit::run(c, dir).await,
        "flags" => flags::run(c, dir).await,
        "fps-cap" => fps_cap::run(c, dir).await,
        "roblox-usage" => roblox_usage::run(c, dir).await,
        "game-page" => game_page::run(c, dir).await,
        "window-shot" => window_shot::run(c, dir).await,
        "monitor" => monitor::run(c, dir).await,
        "foreign-vault" => foreign_vault::run(c, dir).await,
        "window-icons" => window_icons::run(c, dir).await,
        "taskbar-groups" => taskbar_groups::run(c, dir).await,
        "home-session" => home_session::run(c, dir).await,
        "sign-ins" => sign_ins::run(c, dir).await,
        "tick-time" => tick_time::run(c, dir).await,
        "schedule" => schedule::run(c, dir).await,
        "play-losses" => play_losses::run(c, dir).await,
        "play-heat" => play_heat::run(c, dir).await,
        "macros" => macros::run(c, dir).await,
        "input-turn" => input_turn::run(c, dir).await,
        "server-pick" => server_pick::run(c, dir).await,
        "adopt" => adopt::run(c, dir).await,
        "delete-accounts" => delete_accounts::run(c, dir).await,
        "shutdown" => shutdown::run(c, dir).await,
        "tick" => tick::run(c, dir).await,
        "totp" => totp::run(c, dir).await,
        "lock-wall" => lock_wall::run(c, dir).await,
        "vault-crash" => vault_crash::run(c, dir).await,
        "stats-icons" => stats_icon::run(c, dir).await,
        "watch-online" => watch::run_online(c, dir).await,
        _ => {}
    }
}

/// Each suite in a child process of its own (a hung one is stopped after `limit`), as Program.cs ran them.
pub fn run_all(wanted: &[String], online: bool, limit: Duration) -> i32 {
    let exe = std::env::current_exe().expect("own path");
    let base = std::env::temp_dir().join("bloxkeep-rust-tests");
    let (mut failed, mut skipped_total) = (0, 0);
    let started = std::time::Instant::now();
    for (name, needs_net) in SUITES {
        if if wanted.is_empty() { *needs_net && !online } else { !wanted.iter().any(|w| w == name) } {
            continue;
        }
        let dir = base.join(name);
        let mut child = match std::process::Command::new(&exe)
            .args(["--core-suite", name, &dir.to_string_lossy()])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                println!("-- {name} --\n  FAIL  started  [{e}]");
                failed += 1;
                continue;
            }
        };
        let mut out = child.stdout.take().expect("piped");
        let mut err = child.stderr.take().expect("piped");
        let reader = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = std::io::Read::read_to_string(&mut out, &mut s);
            s
        });
        let err_reader = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = std::io::Read::read_to_string(&mut err, &mut s);
            s
        });
        let begun = std::time::Instant::now();
        let mut timed_out = false;
        let code = loop {
            if let Ok(Some(status)) = child.try_wait() {
                break status.code().unwrap_or(1);
            }
            if begun.elapsed() > limit {
                timed_out = true;
                let _ = child.kill();
                let _ = child.wait();
                break 1;
            }
            std::thread::sleep(Duration::from_millis(100));
        };
        let text = reader.join().unwrap_or_default() + &err_reader.join().unwrap_or_default();
        println!("-- {name} --");
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            println!("  {}", line.trim_end());
            if line.starts_with("SKIP") || line.starts_with("TODO") {
                skipped_total += 1;
            }
        }
        if timed_out {
            println!("  FAIL  the suite ended within {} minutes  [stopped]", limit.as_secs() / 60);
            failed += 1;
        } else {
            failed += if (0..1000).contains(&code) { code } else { 1 };
        }
    }
    println!(
        "{} ({:.1} s, {skipped_total} skipped or not ported).",
        if failed == 0 { "All checks passed".to_string() } else { format!("{failed} check(s) failed") },
        started.elapsed().as_secs_f64()
    );
    failed
}
