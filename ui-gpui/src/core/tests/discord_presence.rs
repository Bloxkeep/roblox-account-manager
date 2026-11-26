// Tests/Bloxkeep.Tests/DiscordPresenceSuite.cs: "Playing Bloxkeep" on the Discord profile (discord_presence.rs): against a
// stand-in for the Discord app's pipe, the handshake names the application, the activity says the windows and the game
// with a link and never twice the same, and turning it off clears it. With no Discord running nothing breaks. No real
// Discord is touched: the stand-in's pipe name is set before anything could reach for Discord's own.
use std::{
    ffi::c_void,
    io::{Read, Write},
    os::windows::io::FromRawHandle,
    rc::Rc,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use serde_json::Value;

use super::{unlock, window::quiet_app as app, Checks, PASSWORD};
use crate::core::{
    app::controller::PendingLaunch,
    base::{new_guid, Stamp},
    discord_presence::{Activity, DiscordPresence},
    game_target::GameTarget,
    models::Account,
    roblox_api::GameInfo,
    vault::Vault,
    win_kit::wide,
};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateNamedPipeW(name: *const u16, open_mode: u32, pipe_mode: u32, max: u32, out_size: u32, in_size: u32, timeout: u32, sa: *const c_void) -> *mut c_void;
    fn ConnectNamedPipe(pipe: *mut c_void, overlapped: *mut c_void) -> i32;
}

type Frames = Arc<Mutex<Vec<(i32, String)>>>;

fn read_frame(pipe: &mut std::fs::File) -> Option<(i32, String)> {
    let mut head = [0u8; 8];
    pipe.read_exact(&mut head).ok()?;
    let op = i32::from_le_bytes(head[0..4].try_into().ok()?);
    let len = i32::from_le_bytes(head[4..8].try_into().ok()?) as usize;
    let mut body = vec![0u8; len];
    pipe.read_exact(&mut body).ok()?;
    Some((op, String::from_utf8_lossy(&body).into_owned()))
}

fn write_frame(pipe: &mut std::fs::File, op: i32, json: &str) -> bool {
    let mut frame = Vec::with_capacity(8 + json.len());
    frame.extend_from_slice(&op.to_le_bytes());
    frame.extend_from_slice(&(json.len() as i32).to_le_bytes());
    frame.extend_from_slice(json.as_bytes());
    pipe.write_all(&frame).and_then(|_| pipe.flush()).is_ok()
}

fn json(body: &str) -> Value {
    serde_json::from_str(body).unwrap_or(Value::Null)
}

pub async fn run(c: &mut Checks, dir: &str) {
    let pipe_name = format!("bloxkeep-test-discord-{}", new_guid().simple());
    // SAFETY: this suite's own process, before any thread reads it (the presence reads it once, at its first use).
    unsafe { std::env::set_var("BLOXKEEP_DISCORD_PIPE", &pipe_name) };
    game_shown(c, dir).await;

    // Nothing is listening yet: no Discord on this PC.
    {
        let none = DiscordPresence::new();
        let quiet = Instant::now();
        none.set(
            Some(&Activity { details: "1 Roblox window open".into(), state: None, start_unix: 1, button_label: "Get Bloxkeep".into(), button_url: "https://example.com/".into(), ..Default::default() }),
            "123456",
        );
        let ms = quiet.elapsed().as_millis();
        c.check("with no Discord running nothing breaks, and it doesn't hang", ms < 3000, ms);
        none.dispose();
    }

    let path = wide(&format!(r"\\.\pipe\{pipe_name}"));
    // SAFETY: plain call; the handle goes to the server thread, which owns and closes it.
    let handle = unsafe { CreateNamedPipeW(path.as_ptr(), 3 /* duplex */, 0 /* byte, wait */, 1, 65536, 65536, 0, std::ptr::null()) };
    let frames: Frames = Arc::new(Mutex::new(Vec::new()));
    let keep = frames.clone();
    let handle_value = handle as isize;
    let served = std::thread::spawn(move || {
        let h = handle_value as *mut c_void;
        // SAFETY: the pipe made above; connected (or already connected: ERROR_PIPE_CONNECTED, also fine) and then owned.
        unsafe { ConnectNamedPipe(h, std::ptr::null_mut()) };
        let mut pipe = unsafe { std::fs::File::from_raw_handle(h) };
        while let Some(frame) = read_frame(&mut pipe) {
            let op = frame.0;
            keep.lock().unwrap().push(frame);
            let answer = if op == 0 { r#"{"cmd":"DISPATCH","evt":"READY","data":{}}"# } else { r#"{"cmd":"SET_ACTIVITY","data":null}"# };
            if !write_frame(&mut pipe, 1, answer) {
                return;
            }
        }
    });

    let presence = DiscordPresence::new();
    let activity = Activity {
        details: "2 Roblox windows open".into(),
        state: Some("Blox Fruits".into()),
        start_unix: 1_700_000_000,
        button_label: "Get Bloxkeep".into(),
        button_url: "https://bloxkeep.github.io/roblox-account-manager/".into(),
        ..Default::default()
    };
    presence.set(Some(&activity), "123456");
    let seen = || frames.lock().unwrap().clone();
    let s = seen();
    c.check(
        "the handshake names Bloxkeep's application",
        !s.is_empty() && s[0].0 == 0 && json(&s[0].1)["client_id"] == "123456",
        s.first().map(|f| f.1.clone()),
    );
    let set = s.get(1).map(|f| json(&f.1)).unwrap_or(Value::Null);
    let sent = &set["args"]["activity"];
    c.check(
        "the activity says the windows, the game and since when, with a link to Bloxkeep",
        set["cmd"] == "SET_ACTIVITY"
            && sent["details"] == "2 Roblox windows open"
            && sent["state"] == "Blox Fruits"
            && sent["timestamps"]["start"].as_i64() == Some(1_700_000_000)
            && sent["buttons"][0]["url"] == "https://bloxkeep.github.io/roblox-account-manager/",
        s.get(1).map(|f| f.1.clone()).unwrap_or_else(|| "nothing sent".into()),
    );
    c.check(
        "…without a game picture: Discord shows the application's icon, one button",
        sent["assets"].is_object() && sent["assets"].get("large_image").is_none() && sent["buttons"].as_array().map(Vec::len) == Some(1),
        sent.to_string(),
    );

    presence.set(Some(&activity), "123456");
    c.check("the same activity again isn't sent again", seen().len() == 2, seen().len());

    // In a game: its icon big, Bloxkeep's logo small, the game's page as the first of two buttons.
    let in_game = json(&crate::core::discord_presence::activity_json(&Activity {
        picture: Some("https://tr.rbxcdn.com/icon.png".into()),
        picture_text: Some("Blox Fruits".into()),
        first_label: Some("View game".into()),
        first_url: Some("https://www.roblox.com/games/2753915549".into()),
        ..activity.clone()
    }));
    let assets = &in_game["assets"];
    c.check(
        "in a game: its icon big with its name, Bloxkeep's logo small",
        assets["large_image"] == "https://tr.rbxcdn.com/icon.png"
            && assets["large_text"] == "Blox Fruits"
            && assets["small_image"].as_str().is_some_and(|s| s.ends_with("bloxkeep.png"))
            && assets["small_text"] == "Bloxkeep",
        in_game.to_string(),
    );
    c.check(
        "…and two buttons, the game's page first",
        in_game["buttons"].as_array().map(Vec::len) == Some(2)
            && in_game["buttons"][0]["url"] == "https://www.roblox.com/games/2753915549"
            && in_game["buttons"][1]["label"] == "Get Bloxkeep",
        in_game.to_string(),
    );

    presence.set(None, "123456");
    let s = seen();
    let last = s.last().map(|f| f.1.clone()).unwrap_or_default();
    c.check(
        "turned off, the activity is cleared",
        s.len() == 3 && json(&last)["args"].get("activity").is_some_and(Value::is_null),
        &last,
    );

    presence.dispose();
    drop(presence);
    let begun = Instant::now();
    while !served.is_finished() && begun.elapsed() < Duration::from_secs(3) {
        crate::core::rt::sleep(Duration::from_millis(50)).await;
    }
}

/// `yyyy-MM-ddTHH:mm:ss.fffZ,1.0,abcd,6 text`, a Roblox log line.
fn line(at: Stamp, text: &str) -> String {
    let c = at.utc();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z,1.0,abcd,6 {text}\n",
        c.year,
        c.month,
        c.day,
        c.hour,
        c.minute,
        c.second,
        at.unix_ms().rem_euclid(1000)
    )
}

/// The game on the profile is the one a window is in: after leaving it for Roblox's home screen the window still counts
/// but its game no longer shows. A made-up Roblox log held by this process stands in for the window's.
async fn game_shown(c: &mut Checks, dir: &str) {
    let logs = std::path::Path::new(dir).join("roblox-logs");
    std::fs::create_dir_all(&logs).expect("logs folder");
    // SAFETY: this suite's own process, before any thread reads it.
    unsafe { std::env::set_var("BLOXKEEP_ROBLOX_LOGS", &logs) };
    let me = std::process::id() as i32;
    const PLACE: i64 = 121165298854655;

    let seed = Vault::create(PASSWORD, None).expect("vault");
    seed.data().accounts.push(Account { username: "Player".into(), user_id: 31, ..Default::default() });
    seed.save().expect("saved");
    drop(seed);
    let (app, _said) = app();
    unlock(&app).await;
    let account = app.vault(|d| d.accounts[0].clone()).expect("unlocked");
    const ICON: &str = "https://tr.rbxcdn.com/monster.png";
    app.controller.games.borrow_mut().insert(
        PLACE,
        Some(GameInfo { place_id: PLACE, universe_id: 1, name: "Monster Game".into(), creator: "Maker".into(), playing: 0, icon_url: Some(ICON.into()) }),
    );
    let launch = PendingLaunch::new(account.id, Default::default(), Stamp::utc_now().plus_secs(-120.0), Some(GameTarget::new(PLACE, None, None, None)), false, None);
    app.controller.running.borrow_mut().insert(account.id, (me, Rc::new(launch)));
    // The game sits on the top line (details), with a button to its page; without one the top line is a count.
    let game = || Some(app.presence_activity()).filter(|a| a.first_url.is_some()).map(|a| a.details);
    c.check("a window just opened shows the game it was opened with", game().as_deref() == Some("Monster Game"), game());
    let first = app.presence_activity();
    c.check(
        "…with its icon, its page, and the windows open",
        first.picture.as_deref() == Some(ICON)
            && first.first_url.as_deref() == Some(&*format!("https://www.roblox.com/games/{PLACE}"))
            && first.state.as_deref().is_some_and(|s| s.contains("Roblox window")),
        format!("{first:?}"),
    );

    // A second account in another game: each game takes its turn on the profile.
    const OTHER_PLACE: i64 = 2753915549;
    app.controller.games.borrow_mut().insert(
        OTHER_PLACE,
        Some(GameInfo { place_id: OTHER_PLACE, universe_id: 2, name: "Fruit Game".into(), creator: "Maker".into(), playing: 0, icon_url: None }),
    );
    let second = Account { username: "Second".into(), user_id: 32, ..Default::default() };
    let second_id = second.id;
    let _ = app.vault_mut(|d| d.accounts.push(second));
    let other = PendingLaunch::new(second_id, Default::default(), Stamp::utc_now().plus_secs(-120.0), Some(GameTarget::new(OTHER_PLACE, None, None, None)), false, None);
    app.controller.running.borrow_mut().insert(second_id, (-1, Rc::new(other)));
    let turns: Vec<Activity> = (0..4).map(|_| app.presence_activity()).collect();
    let details: std::collections::HashSet<&str> = turns.iter().map(|t| t.details.as_str()).collect();
    c.check(
        "two games: each takes its turn, saying both are being played",
        details.len() == 2 && turns[0].details != turns[1].details && turns.iter().all(|t| t.state.as_deref().is_some_and(|s| s.ends_with("· 2 games"))),
        turns.iter().map(|t| format!("{} / {:?}", t.details, t.state)).collect::<Vec<_>>().join(" | "),
    );
    c.check("…a game without an icon shows Bloxkeep's", turns.iter().find(|t| t.details == "Fruit Game").is_some_and(|t| t.picture.is_none()), ());
    app.controller.running.borrow_mut().shift_remove(&second_id);

    let stamp = {
        let c = Stamp::utc_now().utc();
        format!("{:04}{:02}{:02}T{:02}{:02}{:02}", c.year, c.month, c.day, c.hour, c.minute, c.second)
    };
    let mut log = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .read(true)
        .write(true)
        .open(logs.join(format!("0.743.0.7431134_{stamp}Z_Player_PRESENCE_last.log")))
        .expect("a log");
    let mut write = |text: String| {
        use std::io::Seek;
        let _ = log.seek(std::io::SeekFrom::End(0));
        let _ = log.write_all(text.as_bytes());
        let _ = log.flush();
    };
    let poll = |status: &str| {
        let app = app.clone();
        let status = status.to_string();
        async move {
            let begun = Instant::now();
            while begun.elapsed() < Duration::from_secs(15) && app.controller.logs.borrow().state_of(me).map(|s| s.status) != Some(status.clone()) {
                app.controller.logs.borrow_mut().poll(&[me]);
                crate::core::rt::sleep(Duration::from_millis(200)).await;
            }
            app.controller.logs.borrow().state_of(me)
        }
    };

    let at = Stamp::utc_now().plus_secs(-20.0);
    write(line(at, "[FLog::SingleSurfaceApp] launchUGCGameInternal"));
    write(line(at.plus_secs(1.0), &format!("[FLog::Output] ! Joining game '11111111-2222-3333-4444-555555555555' place {PLACE} at 10.0.0.1")));
    write(line(at.plus_secs(2.0), "[FLog::Network] serverId: 10.0.0.1|5000"));
    let state = poll("ingame").await;
    c.check(
        "in the game: the profile shows it",
        state.as_ref().is_some_and(|s| s.status == "ingame") && game().as_deref() == Some("Monster Game"),
        (&state, game()),
    );
    // Discord's clock under the game's name counts from when the window got into it, not from when Bloxkeep opened
    // (17 h on a game just joined).
    app.relaunch.started.set(Stamp::utc_now().plus_secs(-17.0 * 3600.0));
    let start = app.presence_activity().start_unix;
    let joined = at.unix_secs();
    c.check("…its clock counts from when the window got into the game, not from Bloxkeep's start", (joined..=joined + 3).contains(&start), (start, joined));

    write(line(at.plus_secs(10.0), "[FLog::SingleSurfaceApp] leaveUGCGameInternal"));
    write(line(at.plus_secs(10.1), "[FLog::Network] Time to disconnect replication data: 0.24"));
    let state = poll("left").await;
    c.check(
        "left for Roblox's home screen: the game no longer shows",
        state.as_ref().is_some_and(|s| s.status == "left") && game().is_none(),
        (&state, game()),
    );
    app.controller.running.borrow_mut().clear();
}
