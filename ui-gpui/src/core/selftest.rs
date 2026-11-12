// Checks of the port that need no window, run from the command line with made-up data only (never the real vault):
//   --core-vault-test PATH --password PW   opens the vault at PATH (one the C# app wrote), says what is in it, changes
//                                          a little, saves it: the C# app must then read it back.
//   --core-cmd-test --password PW          the controller with no window: the first commands the interface sends
//                                          (getState, unlock, getState, lock, getState) on the vault in BLOXKEEP_DATA,
//                                          and what it answers and pushes.
use std::{path::PathBuf, rc::Rc, time::Duration};

use serde_json::{Value, json};

use crate::core::{app::App, models::Account, rt, vault::Vault};

pub fn commands(password: &str, script: Option<&str>) -> i32 {
    if std::env::var("BLOXKEEP_DATA").map(|d| d.is_empty()).unwrap_or(true) {
        println!("BLOXKEEP_DATA must point at a scratch folder: this check never runs on the real data.");
        return 9;
    }
    crate::core::prefs::load();
    crate::core::vaults::load();
    let password = password.to_string();
    let said = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let seen = said.clone();
    let seen2 = said.clone();
    let script = script.and_then(|file| std::fs::read_to_string(file).ok());
    let core = std::thread::Builder::new().name("core".into()).spawn(move || {
        rt::run(move || async move {
            let app = App::start();
            app.init(0, Rc::new(move |line: &str| seen.lock().unwrap_or_else(|e| e.into_inner()).push(line.to_string())));
            // The steps: the ones given (--script FILE: a JSON list of [command, arguments]; "$PASSWORD" stands for the
            // password), or the first few the interface sends.
            let steps: Vec<(String, Value)> = match script {
                Some(text) => serde_json::from_str::<Vec<(String, Value)>>(&text.replace("$PASSWORD", &password)).unwrap_or_default(),
                None => [("getState", json!({})), ("unlock", json!({ "password": password })), ("getState", json!({})), ("lock", json!({})), ("getState", json!({}))].into_iter().map(|(c, a)| (c.to_string(), a)).collect(),
            };
            for (i, (cmd, args)) in steps.into_iter().enumerate() {
                seen2.lock().unwrap_or_else(|e| e.into_inner()).push(json!({ "type": "sent", "id": i + 1, "cmd": cmd }).to_string());
                app.handle_message(&json!({ "id": i + 1, "cmd": cmd, "args": args }).to_string()).await;
                rt::sleep(Duration::from_millis(250)).await;
            }
        })
    });
    let ok = core.map(|c| c.join().is_ok()).unwrap_or(false);
    // BLOXKEEP_SELFTEST_RAW=FILE: every line as it came, for a closer look than the summary below.
    if let Ok(file) = std::env::var("BLOXKEEP_SELFTEST_RAW") {
        let _ = std::fs::write(file, said.lock().unwrap_or_else(|e| e.into_inner()).join("
"));
    }
    for line in said.lock().unwrap_or_else(|e| e.into_inner()).iter() {
        let Ok(m) = serde_json::from_str::<Value>(line) else {
            println!("?? {}", &line[..line.len().min(200)]);
            continue;
        };
        let state = |v: &Value| {
            let names: Vec<String> = v.get("accounts").and_then(Value::as_array).map(|l| l.iter().map(|a| a.get("label").and_then(Value::as_str).unwrap_or("?").to_string()).collect()).unwrap_or_default();
            format!("locked={} hasVault={} accounts={:?} keys={}", v.get("locked").map(|x| x.to_string()).unwrap_or_default(), v.get("hasVault").map(|x| x.to_string()).unwrap_or_default(), names, v.as_object().map(|o| o.len()).unwrap_or(0))
        };
        match m.get("type").and_then(Value::as_str) {
            Some("reply") => {
                let r = m.get("result").cloned().unwrap_or(Value::Null);
                let body = if r.get("locked").is_some() { state(&r) } else { let t = r.to_string(); t.chars().take(300).collect() };
                println!("reply {} ok={} {} {}", m["id"], m["ok"], m.get("error").map(|e| e.to_string()).unwrap_or_default(), body);
            }
            Some("event") => {
                let d = m.get("data").cloned().unwrap_or(Value::Null);
                let body = if m["name"] == "state" { state(&d) } else { let t = d.to_string(); t.chars().take(160).collect() };
                println!("event {} {}", m["name"], body);
            }
            Some("sent") => println!("> {} {}", m["id"], m["cmd"]),
            _ => println!("?? {}", &line[..line.len().min(200)]),
        }
    }
    if ok { 0 } else { 3 }
}

pub fn vault(path: &str, password: &str) -> i32 {
    let v = match Vault::open(password, Some(PathBuf::from(path))) {
        Ok(v) => v,
        Err(e) => {
            println!("open failed: {e}");
            return 1;
        }
    };
    {
        let d = v.data();
        println!("accounts={} delay={} presets={} webhook={}", d.accounts.len(), d.settings.launch_delay_seconds, d.settings.presets.len(), d.discord.webhook_url.len());
        for a in &d.accounts {
            println!(
                "  {} {} {} alias=[{}] display=[{}] note=[{}] cookie={} group=[{}] rejoin={} fps={} main={} valid={:?} lastUsed={} created={}",
                a.id,
                a.user_id,
                a.username,
                a.alias,
                a.display_name,
                a.note,
                a.cookie,
                a.group,
                a.auto_rejoin,
                a.fps_cap,
                a.is_main,
                a.cookie_valid,
                if a.last_used.is_some() { "set" } else { "null" },
                a.created_at.to_iso()
            );
        }
        for p in &d.settings.presets {
            println!(
                "  preset [{}] place={} accounts={} sched={:?} days={:?} server={}",
                p.name,
                p.place_input,
                p.account_ids.len(),
                p.schedule.as_ref().map(|s| s.time.clone()),
                p.schedule.as_ref().map(|s| s.days.clone()),
                p.server
            );
        }
    }
    {
        let mut d = v.data();
        if let Some(a) = d.accounts.first_mut() {
            a.note = "rust đã ghi ✓".into();
        }
        d.settings.launch_delay_seconds = 12;
        d.accounts.push(Account { username: "added_by_rust".into(), user_id: 1003, cookie: "TEST-COOKIE-CCCC".into(), ..Default::default() });
    }
    match v.save() {
        Ok(()) => {
            println!("saved");
            0
        }
        Err(e) => {
            println!("save failed: {e}");
            2
        }
    }
}

/// --core-checks TESTS_DIR: parts of the port held to known answers, with no window, no vault and no network:
/// the table of game-box cases the C# suite and the interface share (Tests/game-target-cases.json), the
/// authenticator's codes against the RFCs' own examples, and a date as .NET writes it read and written back.
pub fn checks(tests_dir: &str) -> i32 {
    use crate::core::{base::Stamp, game_target::GameTarget, totp};
    let mut bad = 0;
    let mut check = |name: &str, ok: bool, got: String| {
        if !ok {
            bad += 1;
            println!("FAIL {name}: {got}");
        }
    };

    // The game box.
    let table = std::fs::read_to_string(std::path::Path::new(tests_dir).join("game-target-cases.json")).unwrap_or_default();
    let cases: Vec<Value> = serde_json::from_str(&table).unwrap_or_default();
    let text = |x: &Value, k: &str| x.get(k).and_then(Value::as_str).map(str::to_string);
    for x in &cases {
        let (input, job) = (text(x, "in").unwrap_or_default(), text(x, "job").unwrap_or_default());
        let got = GameTarget::try_parse(&input, &job);
        let what = format!("game box \"{input}\" job \"{job}\"");
        if x.get("ok").and_then(Value::as_bool) == Some(true) {
            let ok = got.as_ref().is_ok_and(|t| Some(t.place_id) == x.get("place").and_then(Value::as_i64) && t.job_id == text(x, "jobId") && t.link_code == text(x, "link") && t.share_code == text(x, "share") && t.access_code == text(x, "access"));
            check(&what, ok, match &got {
                Ok(t) => format!("place {} job {:?} link {:?} share {:?} access {:?}", t.place_id, t.job_id, t.link_code, t.share_code, t.access_code),
                Err(e) => e.clone(),
            });
        } else {
            check(&what, got.as_ref().err() == text(x, "error").as_ref(), match &got {
                Ok(_) => "taken".into(),
                Err(e) => e.clone(),
            });
        }
    }
    check("the game-box table has its cases", cases.len() >= 30, cases.len().to_string());
    println!("game box: {} cases", cases.len());

    // The authenticator: RFC 4226 Appendix D (counters 0–9), RFC 6238 Appendix B (second 59, all three hashes).
    let secret = b"12345678901234567890";
    let hotp = ["755224", "287082", "359152", "969429", "338314", "254676", "287922", "162583", "399871", "520489"];
    let got: Vec<String> = (0..10).map(|n| totp::code_at(secret, n, 6, "SHA1")).collect();
    check("RFC 4226: the ten counter values", got.iter().map(String::as_str).eq(hotp.iter().copied()), got.join(","));
    check("RFC 6238 SHA1 at 59 s", totp::code_at(secret, 1, 8, "SHA1") == "94287082", totp::code_at(secret, 1, 8, "SHA1"));
    check("RFC 6238 SHA256 at 59 s", totp::code_at(b"12345678901234567890123456789012", 1, 8, "SHA256") == "46119246", totp::code_at(b"12345678901234567890123456789012", 1, 8, "SHA256"));
    let long = b"1234567890123456789012345678901234567890123456789012345678901234";
    check("RFC 6238 SHA512 at 59 s", totp::code_at(long, 1, 8, "SHA512") == "90693936", totp::code_at(long, 1, 8, "SHA512"));
    check("RFC 6238 SHA1 at 1111111109 s", totp::code_at(secret, 1111111109 / 30, 8, "SHA1") == "07081804", totp::code_at(secret, 1111111109 / 30, 8, "SHA1"));
    println!("authenticator: 5 checks");

    // Dates as .NET's JSON writes them.
    for iso in ["2026-11-11T20:25:51.7082732+07:00", "2026-11-11T13:25:51Z", "2026-01-02T03:04:05.5", "2025-12-31T23:59:59.0000001-05:30"] {
        let back = Stamp::parse(iso).map(|t| t.to_iso());
        check(&format!("date {iso}"), back.as_deref() == Some(iso), format!("{back:?}"));
    }
    let (a, b) = (Stamp::parse("2026-11-11T20:25:51+07:00"), Stamp::parse("2026-11-11T13:25:51Z"));
    check("the same moment in two zones is equal", a.is_some() && a == b, format!("{a:?} {b:?}"));
    println!("dates: 5 checks");

    println!("{}", if bad == 0 { "all checks passed".to_string() } else { format!("{bad} FAILED") });
    bad
}

/// --core-log-replay FILE: a Roblox log read as the watcher reads it (roblox_log_watcher::replay), one line a change.
pub fn log_replay(file: &str) -> i32 {
    let Ok(bytes) = std::fs::read(file) else {
        println!("can't read {file}");
        return 1;
    };
    for line in crate::core::roblox_log_watcher::replay(&String::from_utf8_lossy(&bytes)) {
        println!("{line}");
    }
    0
}

/// --core-words OUT: every sentence of the app's own table with its Vietnamese, as JSON lines [english, vietnamese].
pub fn words(out: &str) -> i32 {
    let lines: Vec<String> = crate::core::l::keys().into_iter().map(|k| json!([k, crate::core::l::t_in(k, "vi")]).to_string()).collect();
    let _ = std::fs::write(out, lines.join("\n"));
    println!("{} sentences", lines.len());
    0
}

/// --core-expr FILE: each line of FILE is a formula; prints what it comes to (x = 3, coins = 10.5, big = 1e300; any
/// other name is unknown), or why not, then what `problem` and `number_in` say of the same text.
pub fn expr(file: &str) -> i32 {
    use crate::core::macro_expr::{self, MacroExprError};
    let text = std::fs::read_to_string(file).unwrap_or_default();
    for line in text.lines() {
        let mut value = |name: &str| -> Result<f64, MacroExprError> {
            match name {
                "x" => Ok(3.),
                "coins" => Ok(10.5),
                "big" => Ok(1e300),
                other => Err(MacroExprError(format!("unknown {other}"))),
            }
        };
        let got = match macro_expr::eval(line, &mut value, None) {
            Ok(v) => crate::core::models::net_double(v),
            Err(e) => format!("ERR {}", e.0),
        };
        let number = macro_expr::number_in(line).map(crate::core::models::net_double).unwrap_or_else(|| "none".into());
        println!("{got} | {} | {number}", macro_expr::problem(line).unwrap_or_default());
    }
    0
}

/// --core-codes OUT: every number from 0 to 1100 as the core takes it, as JSON lines: Roblox's name, the words in
/// English and Vietnamese, and what the rejoin does with it. Held against Roblox's own table outside (the scratch check).
pub fn codes(out: &str) -> i32 {
    use crate::core::app::rejoin::{lost_connection, roblox_wont_reconnect, signed_in_elsewhere};
    let lines: Vec<String> = (0..=1100)
        .map(|code| {
            let row = crate::roblox_codes::why(code);
            json!({
                "code": code,
                "name": row.map(|c| c.name),
                "key": row.map(|c| c.key),
                "en": crate::core::app::activity::kick_reason(code),
                "vi": row.map(|c| crate::core::l::t_in(c.words, "vi")),
                "noReconnect": roblox_wont_reconnect(Some(code)),
                "lost": lost_connection(code),
                "elsewhere": signed_in_elsewhere(Some(code)),
                "teleport": crate::roblox_codes::teleport_failed(code),
                // the same number taken as a join answer's status, without and with Roblox's message
                "joinWords": crate::core::app::rejoin::join_refusal_words(Some(code), None),
                "joinWordsWithMessage": crate::core::app::rejoin::join_refusal_words(Some(code), Some("Unable to join Game 311")),
            })
            .to_string()
        })
        .collect();
    let _ = std::fs::write(out, lines.join("\n"));
    println!("{} numbers", lines.len());
    0
}
