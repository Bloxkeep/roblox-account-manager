// Tests/Bloxkeep.Tests/OnlineSuite.cs: Add me to the online count (app/online.rs): a development copy never counts; an
// installed one says it's open with an id made from this PC (the same after a reinstall), its version and language and
// nothing else, locked or not, as often as the bot says (held between a minute and six hours), tries a failed beat again
// sooner but never more often than usual, says goodbye as it closes, and turned off says goodbye, forgets the id and sends
// nothing more. Sent to a little server of the test's own on 127.0.0.1 (feedback.rs's StandInServer), never the bot.
use std::{
    sync::{
        atomic::{AtomicU16, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use regex::Regex;
use serde_json::{json, Value};

use super::{
    call,
    feedback::StandInServer,
    window::quiet_app as app,
    Checks,
};
use crate::core::{
    app::online::{online_id_for_this_pc, ONLINE_URL},
    base::{new_guid, Stamp},
    models::stamp_min,
    prefs, rt,
};

/// HKLM\SOFTWARE\Microsoft\Cryptography's MachineGuid (read only), to check it never goes out as it is.
fn machine_guid() -> String {
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RRF_SUBKEY_WOW6464KEY};
    let key = crate::core::win_kit::wide(r"SOFTWARE\Microsoft\Cryptography");
    let name = crate::core::win_kit::wide("MachineGuid");
    let mut buffer = vec![0u16; 256];
    let mut size = (buffer.len() * 2) as u32;
    // SAFETY: NUL-terminated strings; `buffer` holds `size` bytes and outlives the call.
    let result = unsafe {
        RegGetValueW(HKEY_LOCAL_MACHINE, key.as_ptr(), name.as_ptr(), RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY, std::ptr::null_mut(), buffer.as_mut_ptr().cast(), &mut size)
    };
    if result != 0 {
        return "no-machine-guid".into();
    }
    crate::core::win_kit::from_wide(&buffer)
}

pub async fn run(c: &mut Checks, dir: &str) {
    let (app, _said) = app();
    c.check("on until turned off", prefs::lock().count_online, ());

    // This copy isn't installed and has no server of its own: nothing is sent, not even an id made.
    if app.updates.updater.borrow().supported() {
        // An installed copy would beat the real bot here.
        c.skip("a development copy never counts", "would reach the real bot");
        return;
    }
    app.tick_online(Stamp::utc_now());
    c.check(
        "a development copy never counts",
        !app.online.online_sending.get() && prefs::lock().online_id.is_none() && app.online.next_online.get() == stamp_min(),
        (),
    );

    // The bot: a little server here that answers what each step needs.
    let status = Arc::new(AtomicU16::new(200));
    let answer = Arc::new(Mutex::new(r#"{"next":120}"#.to_string()));
    let (code, said) = (status.clone(), answer.clone());
    let mut server = StandInServer::start(Box::new(move |got| {
        if got.method == "GET" {
            // the sums for an app not in the count (community_now)
            return (200, r#"{"community":{"pcs":3,"alts":9,"games":[],"radar":[],"board":[{"name":"Farm PC","alts":9}],"you":null}}"#.to_string());
        }
        (code.load(Ordering::SeqCst), said.lock().unwrap().clone())
    }));
    // SAFETY: this suite's own process; read each time a beat goes.
    unsafe { std::env::set_var("BLOXKEEP_ONLINE_URL", format!("{}/online/ping/", server.base)) };
    let received = |s: &StandInServer| -> Vec<Value> {
        s.all().iter().filter(|g| g.method != "GET").map(|g| serde_json::from_slice(&g.body).unwrap_or(Value::Null)).collect()
    };
    let count = |s: &StandInServer| received(s).len();
    let last = |s: &StandInServer| received(s).last().cloned().unwrap_or(Value::Null);
    let asked = |s: &StandInServer| s.all().iter().filter(|g| g.method == "GET").count();
    let beat = |at: Stamp, expected: usize| {
        let app = app.clone();
        let server = &server;
        async move {
            app.tick_online(at);
            for _ in 0..100 {
                if count(server) >= expected && !app.online.online_sending.get() {
                    break;
                }
                rt::sleep(Duration::from_millis(50)).await;
            }
            count(server) == expected && !app.online.online_sending.get()
        }
    };
    let next = || app.online.next_online.get();
    let near = |at: Stamp, from: f64| (at.secs_since(Stamp::utc_now().plus_secs(from))).abs() < 10.0;
    let set_answer = |text: &str| *answer.lock().unwrap() = text.to_string();
    let hours = |h: f64| Stamp::utc_now().plus_secs(h * 3600.0);

    // Locked (the vault isn't open): a beat all the same.
    c.check("locked or not: a beat", beat(Stamp::utc_now(), 1).await, count(&server));
    let first = last(&server);
    let id = prefs::lock().online_id.clone().unwrap_or_default();
    let mut names: Vec<String> = first.as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
    names.sort();
    let pc = online_id_for_this_pc();
    // What its accounts play goes too (games, how many run, drops, the alts board's name): none here, no account; on the
    // board from the start under a made-up name; the theme picked, for Popular (Bloxkeep's own only).
    let made_up = Regex::new(r"^[A-Z][a-z]+ [A-Z][a-z]+ \d\d$").unwrap();
    let (board_name, board_chosen) = {
        let p = prefs::lock();
        (p.board_name.clone(), p.board_name_chosen)
    };
    let theme_ok = match first.get("theme") {
        None | Some(Value::Null) => true,
        Some(Value::String(t)) => (1..40).contains(&t.encode_utf16().count()),
        _ => false,
    };
    let pc_ok = pc.as_ref().is_some_and(|pc| pc.len() == 32 && pc.chars().all(|ch| ch.is_ascii_hexdigit()) && *pc == pc.to_lowercase());
    c.check(
        "…with this PC's id (32 hex digits, not Windows' own id), the version and the language, nothing else",
        names.iter().filter(|n| *n != "theme").cloned().collect::<Vec<_>>().join(",") == "age,alts,app,board,drops,first,games,id,language"
            && theme_ok
            && pc_ok
            && first["games"].as_array().is_some_and(|a| a.is_empty())
            && first["alts"].as_i64() == Some(0)
            && first["drops"].as_array().is_some_and(|a| a.is_empty())
            && first["board"].as_str().is_some_and(|made| made_up.is_match(made) && board_name.as_deref() == Some(made))
            && board_chosen
            && first["id"].as_str() == Some(id.as_str())
            && pc.as_deref() == Some(id.as_str())
            && !first.to_string().to_lowercase().contains(&machine_guid().to_lowercase())
            && first["app"].as_str() == Some(app.updates.updater.borrow().current_version())
            && matches!(first["language"].as_str(), Some("en" | "vi")),
        &first,
    );
    let prefs_text = std::fs::read_to_string(std::path::Path::new(dir).join("prefs.json")).unwrap_or_default();
    c.check("…the id kept in prefs.json, outside the vault", prefs_text.contains(&format!("\"onlineId\":\"{id}\"")), &prefs_text);
    c.check("…and the next beat when the bot says (2 minutes here)", near(next(), 120.0), next());
    // How long this PC has had Bloxkeep (its data folder, made just now here): a day at most; and this new install's first
    // open, told once.
    c.check(
        "…how long it has had Bloxkeep, roughly (under a day here), and that it's a new install's first open",
        first["age"] == "new" && first["first"] == true && prefs::lock().first_open_told,
        &first,
    );

    c.check("nothing before then", beat(Stamp::utc_now(), 1).await && count(&server) == 1, count(&server));
    set_answer(r#"{"next":86400}"#);
    c.check(
        "then the same id again, no longer a first open",
        beat(Stamp::utc_now().plus_secs(180.0), 2).await && last(&server)["id"].as_str() == Some(id.as_str()) && last(&server)["first"] == false,
        last(&server),
    );
    c.check("…and a gap longer than six hours is held to six", near(next(), 6.0 * 3600.0), next());
    set_answer(r#"{"next":5}"#);
    beat(hours(7.0), 3).await;
    c.check("…one shorter than a minute to a minute", near(next(), 60.0), next());

    // The bot can't answer: tried again in two minutes, then four… never later than the usual gap.
    set_answer(r#"{"next":600}"#);
    beat(hours(8.0), 4).await;
    status.store(503, Ordering::SeqCst);
    beat(hours(9.0), 5).await;
    c.check("a beat the bot can't take: tried again in two minutes", near(next(), 120.0), next());
    beat(hours(10.0), 6).await;
    c.check("…then four", near(next(), 240.0), next());
    beat(hours(11.0), 7).await;
    beat(hours(12.0), 8).await;
    c.check("…and never later than the usual gap (ten minutes here)", near(next(), 600.0), next());
    status.store(200, Ordering::SeqCst);
    beat(hours(13.0), 9).await;
    beat(hours(14.0), 10).await;
    c.check(
        "answered again: back to the usual gap, the same id",
        near(next(), 600.0) && last(&server)["id"].as_str() == Some(id.as_str()),
        next(),
    );

    // Closing: a goodbye with that id; not for an update in the background, back within seconds.
    *app.relaunch.relaunch_args.borrow_mut() = Some(vec!["--background".to_string()]);
    app.say_offline_on_exit();
    c.check("closing for an update in the background: no goodbye", count(&server) == 10, count(&server));
    *app.relaunch.relaunch_args.borrow_mut() = None;
    app.say_offline_on_exit();
    let bye = last(&server);
    c.check(
        "closing: a goodbye, with the id only",
        count(&server) == 11 && bye["id"].as_str() == Some(id.as_str()) && bye["gone"] == true && bye.as_object().map(|o| o.len()) == Some(2),
        &bye,
    );

    // Turned off: a goodbye, the id forgotten, nothing more.
    let _ = call(&app, "setCountOnline", json!({ "enabled": false })).await;
    for _ in 0..100 {
        if count(&server) >= 12 {
            break;
        }
        rt::sleep(Duration::from_millis(50)).await;
    }
    let bye = last(&server);
    c.check(
        "turned off: a goodbye",
        count(&server) == 12 && bye["gone"] == true && bye["id"].as_str() == Some(id.as_str()),
        &bye,
    );
    let prefs_text = std::fs::read_to_string(std::path::Path::new(dir).join("prefs.json")).unwrap_or_default();
    c.check(
        "…the id forgotten, the choice kept",
        prefs::lock().online_id.is_none() && !prefs_text.contains(&id) && prefs_text.contains("\"countOnline\":false"),
        &prefs_text,
    );
    beat(Stamp::utc_now().plus_secs(86_400.0), 12).await;
    rt::sleep(Duration::from_millis(300)).await;
    c.check("…and nothing more sent", count(&server) == 12, count(&server));

    // Off, with the Games tab or Statistics open: the sums asked for apart (no beat, nothing of this PC), once a minute.
    c.check("off: no sums kept from the count", app.community_dto().is_null(), ());
    let _ = call(&app, "communityNow", json!({})).await;
    for _ in 0..100 {
        if asked(&server) >= 1 && !app.online.community_asking.get() {
            break;
        }
        rt::sleep(Duration::from_millis(50)).await;
    }
    let sums = app.community_dto();
    c.check(
        "…the Games tab or Statistics asks for them apart: no beat, the board shown",
        asked(&server) == 1 && count(&server) == 12 && !sums.is_null() && sums.to_string().contains("Farm PC"),
        asked(&server),
    );
    let _ = call(&app, "communityNow", json!({})).await;
    rt::sleep(Duration::from_millis(300)).await;
    c.check("…not again within the minute", asked(&server) == 1, asked(&server));

    // On again: counted at once, under this PC's id again.
    let _ = call(&app, "setCountOnline", json!({ "enabled": true })).await;
    c.check(
        "on again: a beat at once, this PC's id again",
        beat(Stamp::utc_now(), 13).await && last(&server)["id"].as_str() == Some(id.as_str()) && prefs::lock().online_id.as_deref() == Some(id.as_str()),
        last(&server),
    );

    // The name emptied: off the board, and no made-up one again.
    let _ = call(&app, "setBoardName", json!({ "name": "" })).await;
    c.check(
        "the board name emptied: off the board, no made-up name again",
        beat(Stamp::utc_now(), 14).await && last(&server)["board"].is_null() && prefs::lock().board_name.is_none(),
        last(&server),
    );

    // Reinstalled with its data deleted, or an earlier Bloxkeep's random id in prefs.json: still this PC's id, so it isn't
    // counted as one more.
    prefs::lock().online_id = None;
    app.online.next_online.set(stamp_min());
    c.check(
        "reinstalled, its data deleted: the same id",
        beat(Stamp::utc_now(), 15).await && last(&server)["id"].as_str() == Some(id.as_str()),
        last(&server),
    );
    prefs::lock().online_id = Some(new_guid().simple().to_string());
    app.online.next_online.set(stamp_min());
    c.check(
        "…and an earlier random id gives way to it",
        beat(Stamp::utc_now(), 16).await && last(&server)["id"].as_str() == Some(id.as_str()) && prefs::lock().online_id.as_deref() == Some(id.as_str()),
        last(&server),
    );

    server.stop();
    // SAFETY: as above.
    unsafe { std::env::remove_var("BLOXKEEP_ONLINE_URL") };
    // Without the test's server this copy is a development one again: closing sends nothing to the real bot.
    c.check("a development copy says no goodbye to the real bot", !app.online_allowed(), ());

    // The real address: Bloxkeep's bot (services/DiscordBot/worker), which counts.
    let real = url::Url::parse(ONLINE_URL);
    c.check(
        "the bot's address is set",
        real.as_ref().is_ok_and(|u| u.scheme() == "https" && u.host_str() == Some("bloxkeep-bot.bloxkeep-app.workers.dev") && u.path() == "/online/ping"),
        ONLINE_URL,
    );
}
