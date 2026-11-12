// Tests/Bloxkeep.Tests/RemoteSuite.cs: Control from Discord (app/remote.rs, remote_link.rs): off until turned on; on,
// Bloxkeep connects to the bot (a little WebSocket server of the test's own on 127.0.0.1 here, never the bot) with a secret
// kept encrypted, says only the PC's, accounts' and presets' names, asks for a code and shows it, shows who it's linked
// to; answers the bot's commands (the status card; a preset, an account that doesn't exist; nothing open to close), and
// only "locked" while locked; connects again after a drop; turned off, tells the bot to unlink and closes.
use std::{
    net::{TcpListener, TcpStream},
    rc::Rc,
    sync::{
        atomic::{AtomicBool, AtomicI32, Ordering},
        mpsc, Arc, Mutex,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use serde_json::{json, Value};
use tungstenite::{
    handshake::server::{Request, Response},
    protocol::{frame::coding::CloseCode, CloseFrame},
    Message,
};

use super::{
    call,
    feedback::{read_request, write_response},
    unlock,
    window::quiet_app as app,
    Checks, PASSWORD,
};
use crate::core::{
    app::{
        controller::{LaunchRef, PendingLaunch, RejoinRequest},
        remote::machine_name,
        App,
    },
    base::{new_guid, Cancel, Stamp},
    game_target::GameTarget,
    models::{Account, LaunchPreset},
    prefs,
    remote_link::{is_bot_reply, RemoteCommand},
    roblox_api::{self, HttpRequest, HttpResponse},
    rt,
    vault::Vault,
    vaults,
};

/// What came to /webhooks/ (Discord's answer link, here).
#[derive(Clone, Debug)]
struct Reply {
    method: String,
    body: String,
    authorization: Option<String>,
}

#[derive(Default)]
struct BotState {
    heard: Mutex<Vec<Value>>,
    replies: Mutex<Vec<Reply>>,
    last_secret: Mutex<Option<String>>,
    connections: AtomicI32,
    open: AtomicI32,
    close_at_once: AtomicBool,
    /// The open socket: what to send down it, and its stream (to abort it).
    current: Mutex<Option<(mpsc::Sender<String>, TcpStream)>>,
    stop: AtomicBool,
}

/// The bot's /remote/connect, here: takes connections, keeps what they say, says what the test wants.
struct StandInBot {
    url: String,
    reply_url: String,
    s: Arc<BotState>,
    thread: Option<JoinHandle<()>>,
}

impl StandInBot {
    fn new() -> StandInBot {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
        listener.set_nonblocking(true).expect("non-blocking");
        let port = listener.local_addr().expect("port").port();
        let s = Arc::new(BotState::default());
        let state = s.clone();
        let thread = std::thread::spawn(move || {
            while !state.stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let state = state.clone();
                        std::thread::spawn(move || serve(stream, &state));
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(10)),
                }
            }
        });
        StandInBot {
            url: format!("ws://127.0.0.1:{port}/remote/connect/"),
            reply_url: format!("http://127.0.0.1:{port}/webhooks/app1/tok1/messages/@original"),
            s,
            thread: Some(thread),
        }
    }
    fn connections(&self) -> i32 {
        self.s.connections.load(Ordering::SeqCst)
    }
    fn open(&self) -> i32 {
        self.s.open.load(Ordering::SeqCst)
    }
    fn heard(&self) -> Vec<Value> {
        self.s.heard.lock().unwrap().clone()
    }
    fn last(&self, t: &str) -> Option<Value> {
        self.heard().into_iter().rev().find(|m| m["t"] == t)
    }
    fn count(&self, t: &str) -> usize {
        self.heard().iter().filter(|m| m["t"] == t).count()
    }
    fn last_secret(&self) -> Option<String> {
        self.s.last_secret.lock().unwrap().clone()
    }
    fn replies(&self) -> Vec<Reply> {
        self.s.replies.lock().unwrap().clone()
    }
    async fn send(&self, m: Value) {
        if let Some((tx, _)) = self.s.current.lock().unwrap().as_ref() {
            let _ = tx.send(m.to_string());
        }
        // The socket's thread sends it within its next look (C#'s SendAsync was awaited).
        rt::sleep(Duration::from_millis(80)).await;
    }
    fn drop_socket(&self) {
        if let Some((_, stream)) = self.s.current.lock().unwrap().take() {
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
    }
}

impl Drop for StandInBot {
    fn drop(&mut self) {
        self.drop_socket();
        self.s.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn serve(mut stream: TcpStream, s: &Arc<BotState>) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    // The request's head, looked at without taking it (the WebSocket handshake reads it again).
    let mut peeked = vec![0u8; 8192];
    let began = Instant::now();
    let head = loop {
        let Ok(n) = stream.peek(&mut peeked) else { return };
        let text = String::from_utf8_lossy(&peeked[..n]).into_owned();
        if text.contains("\r\n\r\n") || n == peeked.len() {
            break text;
        }
        if n == 0 || began.elapsed() > Duration::from_secs(5) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let path = head.split(' ').nth(1).unwrap_or("").to_string();
    if path.starts_with("/webhooks/") {
        if let Some(got) = read_request(&mut stream) {
            let reply = Reply { method: got.method.clone(), body: got.latin1(), authorization: got.header("Authorization").map(str::to_string) };
            s.replies.lock().unwrap().push(reply);
        }
        write_response(&mut stream, 200, "");
        return;
    }
    let state = s.clone();
    let accepted = tungstenite::accept_hdr(stream, move |req: &Request, res: Response| {
        let secret = req.headers().get("Authorization").and_then(|v| v.to_str().ok()).map(|v| v.replace("Bearer ", ""));
        *state.last_secret.lock().unwrap() = secret;
        Ok(res)
    });
    let Ok(mut ws) = accepted else { return };
    s.connections.fetch_add(1, Ordering::SeqCst);
    if s.close_at_once.load(Ordering::SeqCst) {
        let _ = ws.close(Some(CloseFrame { code: CloseCode::from(4000), reason: "connected again".into() }));
        let _ = ws.flush();
        // Kept until the other side has closed too (or a moment has passed).
        let _ = ws.get_ref().set_read_timeout(Some(Duration::from_secs(2)));
        while ws.read().is_ok() {}
        return;
    }
    let (tx, rx) = mpsc::channel::<String>();
    let Ok(abort) = ws.get_ref().try_clone() else { return };
    *s.current.lock().unwrap() = Some((tx, abort));
    s.open.fetch_add(1, Ordering::SeqCst);
    let _ = ws.get_ref().set_read_timeout(Some(Duration::from_millis(30)));
    'reading: loop {
        while let Ok(text) = rx.try_recv() {
            if ws.send(Message::text(text)).is_err() {
                break 'reading;
            }
        }
        match ws.read() {
            Ok(Message::Text(t)) => {
                let text = t.to_string();
                if text != "ping"
                    && let Ok(m) = serde_json::from_str::<Value>(&text)
                    && m.is_object()
                {
                    s.heard.lock().unwrap().push(m);
                }
            }
            Ok(Message::Close(_)) => {
                let _ = ws.flush();
                break;
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            Err(_) => break,
        }
    }
    s.open.fetch_sub(1, Ordering::SeqCst);
}

fn names(hello: &Value, list: &str) -> String {
    hello[list].as_array().map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(",")).unwrap_or_default()
}

/// Waits for `done`, ticking the link as the app's timer would.
async fn until(app: &Rc<App>, done: impl Fn() -> bool, ms: u64) -> bool {
    let begun = Instant::now();
    while !done() && begun.elapsed() < Duration::from_millis(ms) {
        app.tick_remote();
        rt::sleep(Duration::from_millis(50)).await;
    }
    done()
}

fn prefs_text(dir: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(dir).join("prefs.json")).unwrap_or_default()
}

pub async fn run(c: &mut Checks, dir: &str) {
    // Roblox isn't asked anything for real: every question gets "not found".
    *roblox_api::STAND_IN.write().unwrap() =
        Some(Box::new(|_req: &HttpRequest| Ok(HttpResponse { status: 404, headers: Vec::new(), body: b"{}".to_vec() })));
    let seed = Vault::create(PASSWORD, None).expect("vault");
    let (alpha_id, beta_id) = {
        let mut d = seed.data();
        d.accounts.push(Account { username: "Alpha".into(), user_id: 3001, cookie: "_TEST_remote_A".into(), cookie_valid: Some(true), ..Default::default() });
        d.accounts.push(Account { username: "Beta".into(), user_id: 3002, cookie: "_TEST_remote_B".into(), cookie_valid: Some(true), ..Default::default() });
        let alpha = d.accounts[0].id;
        d.settings.presets.push(LaunchPreset { name: "Farm".into(), account_ids: vec![alpha], ..Default::default() });
        (alpha, d.accounts[1].id)
    };
    let _ = alpha_id;
    seed.save().expect("saved");
    drop(seed);

    let bot = StandInBot::new();
    // SAFETY: this suite's own process; read each time the link is made.
    unsafe { std::env::set_var("BLOXKEEP_REMOTE_URL", &bot.url) };
    let (app, _said) = app();
    unlock(&app).await;
    let field = |name: &str| app.remote_dto()[name].clone();

    c.check(
        "off until turned on: nothing connects",
        !prefs::lock().remote_control() && !until(&app, || bot.connections() > 0, 600).await,
        (),
    );

    let _ = call(&app, "setRemote", json!({ "enabled": true })).await;
    c.check("turned on: it connects to the bot", until(&app, || bot.connections() == 1 && field("connected") == true, 8000).await, bot.connections());
    let secret = bot.last_secret().unwrap_or_default();
    let key = prefs::lock().remote_key();
    c.check(
        "…with a secret of its own (long and random), kept in prefs.json encrypted, not as it is",
        secret.len() >= 40 && key.as_ref().is_some_and(|k| !k.is_empty() && !k.contains(&secret)) && !prefs_text(dir).contains(&secret),
        secret.len(),
    );
    let hello_ok = until(&app, || bot.last("hello").is_some(), 8000).await;
    let hello = bot.last("hello").unwrap_or(Value::Null);
    c.check(
        "…says the PC's, accounts' and presets' names, nothing else",
        hello_ok
            && hello["name"].as_str() == Some(machine_name().as_str())
            && names(&hello, "accounts") == "Alpha,Beta"
            && names(&hello, "presets") == "Farm"
            && !bot.heard().iter().any(|m| m.to_string().contains("_TEST_remote_")),
        hello.to_string(),
    );
    c.check("…and asks for a code", until(&app, || bot.last("code").is_some(), 8000).await, ());
    bot.send(json!({ "t": "link", "linked": false })).await;
    bot.send(json!({ "t": "code", "code": "ABC234", "until": Stamp::utc_now().plus_secs(600.0).unix_ms() })).await;
    c.check(
        "the code the bot gives is shown, with when it stops working",
        until(&app, || field("code") == "ABC234", 8000).await
            && field("codeUntil").as_i64().is_some_and(|u| u > Stamp::utc_now().plus_secs(540.0).unix_ms())
            && field("linked") == false,
        app.remote_dto(),
    );
    // The bot keeps codes in memory: connected again (after a drop, or the bot's restart) with a code shown, it's asked
    // whether it still knows it (else a new one), never left showing a code that may be dead.
    let connections = bot.connections();
    bot.drop_socket();
    c.check(
        "connected again with a code shown: the bot is asked to keep it, or give a new one",
        until(&app, || bot.connections() > connections && bot.last("code").is_some_and(|m| m["keep"] == "ABC234"), 10_000).await,
        bot.last("code"),
    );
    bot.send(json!({ "t": "link", "linked": true, "user": "Five" })).await;
    c.check(
        "linked: who to is shown, the code gone",
        until(&app, || field("linked") == true, 8000).await && field("user") == "Five" && field("code").is_null(),
        app.remote_dto(),
    );

    // Someone typed the code into /pc link: nothing is linked until it's answered here.
    let asking = || app.remote_dto()["ask"].clone();
    let ask_field = |name: &str| asking()[name].as_str().map(str::to_string);
    bot.send(json!({ "t": "ask", "user": "Stranger", "replacing": "Five" })).await;
    c.check(
        "asked to allow a link: who asks and whom it replaces are shown",
        until(&app, || !asking().is_null(), 8000).await && ask_field("user").as_deref() == Some("Stranger") && ask_field("replacing").as_deref() == Some("Five"),
        asking(),
    );
    let _ = call(&app, "remoteAllow", json!({ "allow": false })).await;
    c.check(
        "…Don't allow: the bot is told no, the question gone",
        until(&app, || bot.last("deny").is_some(), 8000).await && asking().is_null() && bot.last("allow").is_none(),
        (),
    );
    // Another /pc link while the box is up: a click on the box as it was answers nobody (2026-10-30 review: it allowed
    // whoever asked last).
    bot.send(json!({ "t": "ask", "user": "@stranger", "id": "8" })).await;
    until(&app, || ask_field("id").as_deref() == Some("8"), 8000).await;
    bot.send(json!({ "t": "ask", "user": "@other", "id": "9" })).await;
    until(&app, || ask_field("id").as_deref() == Some("9"), 8000).await;
    let _ = call(&app, "remoteAllow", json!({ "allow": true, "id": "8" })).await;
    rt::sleep(Duration::from_millis(300)).await;
    c.check(
        "an Allow pressed on a question someone else's replaced: nothing sent, the new question stays",
        bot.last("allow").is_none() && ask_field("id").as_deref() == Some("9") && ask_field("user").as_deref() == Some("@other"),
        asking(),
    );
    let _ = call(&app, "remoteAllow", json!({ "allow": false, "id": "9" })).await;
    c.check(
        "…an answer says whose question it was",
        until(&app, || bot.last("deny").is_some_and(|m| m["user"] == "9"), 8000).await && asking().is_null(),
        bot.last("deny"),
    );
    bot.send(json!({ "t": "ask", "user": "Five", "id": "5" })).await;
    until(&app, || !asking().is_null(), 8000).await;
    let _ = call(&app, "lock", json!({})).await;
    c.check("…while locked, nothing is asked (no name on the lock screen)", asking().is_null(), asking());
    unlock(&app).await;
    c.check(
        "…unlocked, the question is back",
        until(&app, || !asking().is_null(), 8000).await && ask_field("user").as_deref() == Some("Five"),
        asking(),
    );
    let _ = call(&app, "remoteAllow", json!({ "allow": true, "id": "5" })).await;
    c.check(
        "…Allow: the bot is told yes, to that question",
        until(&app, || bot.last("allow").is_some(), 8000).await && asking().is_null() && bot.last("allow").is_some_and(|m| m["user"] == "5"),
        bot.last("allow"),
    );
    bot.send(json!({ "t": "link", "linked": true, "user": "Five" })).await;
    until(&app, || field("linked") == true, 8000).await;

    // ── commands ──
    async fn ask(app: &Rc<App>, bot: &StandInBot, command: &str, value: Option<&str>) -> Option<Value> {
        let id = new_guid().simple().to_string();
        bot.send(json!({ "t": "command", "id": id, "command": command, "value": value })).await;
        until(app, || bot.heard().iter().any(|m| m["t"] == "answer" && m["id"] == id.as_str()), 8000).await;
        bot.heard().into_iter().rev().find(|m| m["id"] == id.as_str())
    }
    fn text(answer: &Option<Value>) -> String {
        answer.as_ref().and_then(|a| a["content"].as_str()).unwrap_or("").to_string()
    }
    let status = ask(&app, &bot, "status", None).await.unwrap_or(Value::Null);
    c.check(
        "/pc status: the status card, named after this PC",
        status["embeds"][0]["title"] == "Nobody is playing" && status["embeds"][0]["footer"]["text"].as_str() == Some(format!("Bloxkeep · {}", machine_name()).as_str()),
        status.to_string(),
    );
    let said = text(&ask(&app, &bot, "preset", Some("Nope")).await);
    c.check("/pc preset with a name it doesn't have: said so", said == "No preset is called Nope.", &said);
    let said = text(&ask(&app, &bot, "close", Some("All accounts")).await);
    c.check("/pc close All accounts with nothing open: said so", said == "No Roblox window is open on this PC.", &said);
    let said = text(&ask(&app, &bot, "close", Some("beta")).await);
    c.check("/pc close an account with no window: said so", said == "Beta has no Roblox window open.", &said);
    // Its window late (Roblox updating on a slow link) or its rejoin under way: /pc close stops it, as Close on the account
    // does. Before, Discord was told it had no window open, and the window came later.
    let late_beta = Rc::new(PendingLaunch::new(beta_id, Default::default(), Stamp::utc_now().plus_secs(-150.0), Some(GameTarget::new(1818, None, None, None)), false, None));
    app.launching.late.borrow_mut().push(late_beta.clone());
    let close_said = text(&ask(&app, &bot, "close", Some("beta")).await);
    c.check(
        "/pc close an account whose window is late: closes it once it comes",
        close_said == "Closing Beta…" && app.controller.close_when_claimed.borrow().contains(&LaunchRef(late_beta.clone())),
        &close_said,
    );
    app.launching.late.borrow_mut().clear();
    app.controller.close_when_claimed.borrow_mut().clear();
    let rejoin_cts = Cancel::new();
    *app.rejoin.rejoin_in_flight.borrow_mut() = Some(Rc::new(RejoinRequest::new(beta_id, GameTarget::new(1818, None, None, None), Stamp::utc_now())));
    *app.controller.rejoin_cts.borrow_mut() = Some(rejoin_cts.clone());
    let close_said = text(&ask(&app, &bot, "close", Some("beta")).await);
    c.check("…its rejoin under way: called off", close_said == "Closing Beta…" && rejoin_cts.is_cancelled(), &close_said);
    *app.rejoin.rejoin_in_flight.borrow_mut() = None;
    *app.controller.rejoin_cts.borrow_mut() = None;
    let said = text(&ask(&app, &bot, "switch", Some("Ghost")).await);
    c.check("/pc switch an account that doesn't exist: said so", said == "No account is called Ghost.", &said);
    let said = text(&ask(&app, &bot, "switch", Some("Alpha")).await);
    c.check("…and one that isn't playing: said why it can't", said == "Only accounts playing in a public server can switch to another one.", &said);
    let said = text(&ask(&app, &bot, "switch", Some("All accounts")).await);
    c.check("/pc switch All accounts with nothing open: said so", said == "No Roblox window is open on this PC.", &said);
    c.check("the bot is told this app answers /pc screenshot", bot.last("hello").is_some_and(|h| h["shots"] == true), ());
    let said = text(&ask(&app, &bot, "screenshot", Some("All accounts")).await);
    c.check("/pc screenshot All accounts with nothing open: said so", said == "No Roblox window is open on this PC.", &said);
    let said = text(&ask(&app, &bot, "screenshot", Some("alpha")).await);
    c.check("/pc screenshot an account with no window: said so", said == "Alpha has no Roblox window open.", &said);
    let said = text(&ask(&app, &bot, "screenshot", Some("Ghost")).await);
    c.check("…one that doesn't exist: said so", said == "No account is called Ghost.", &said);
    // Asked again and again: one at a time, 15 s apart (anti-AFK keeps its turns).
    app.remote.picturing.set(true);
    let said = text(&ask(&app, &bot, "screenshot", Some("All accounts")).await);
    c.check("/pc screenshot while one is being taken: said so", said == "Already taking a screenshot. Try again in a moment.", &said);
    app.remote.picturing.set(false);
    app.remote.last_shot.set(Stamp::utc_now().plus_secs(-5.0));
    let said = text(&ask(&app, &bot, "screenshot", Some("All accounts")).await);
    c.check("…within 15 s of the last one: told how long to wait", said == "Wait 10 s, then try again.", &said);
    app.remote.last_shot.set(crate::core::models::stamp_min());
    // The pictures go to the answer link the bot handed (Discord's), not to the bot; then the bot is told it's done.
    let link = app.remote.remote.borrow().clone().expect("the link");
    let shot_id = new_guid().simple().to_string();
    let command = RemoteCommand { id: shot_id.clone(), name: "screenshot".into(), value: Some("Alpha".into()), reply: Some(bot.reply_url.clone()) };
    let sent = {
        let link = link.clone();
        rt::blocking(move || link.shot(&command, "Alpha", &[vec![0xFF, 0xD8, 0xFF, 0xD9]])).await
    };
    let patch = bot.replies().last().cloned();
    c.check(
        "the pictures are sent to the answer link handed with the command: a PATCH with the line and a JPEG, no mentions",
        sent && patch.as_ref().is_some_and(|p| {
            p.method == "PATCH"
                && p.body.contains("name=payload_json")
                && p.body.contains("\"content\":\"Alpha\"")
                && p.body.contains("\"parse\":[]")
                && p.body.contains("filename=screenshot-1.jpg")
                && p.body.contains("image/jpeg")
                && p.authorization.is_none()
        }),
        patch.as_ref().map(|p| p.body.len()),
    );
    c.check(
        "…and the bot is told it's done (so it doesn't say \"no answer\" over them)",
        until(&app, || bot.heard().iter().any(|m| m["t"] == "shown" && m["id"] == shot_id.as_str()), 8000).await,
        (),
    );
    let none = {
        let link = link.clone();
        let command = RemoteCommand { id: "x".into(), name: "screenshot".into(), value: Some("Alpha".into()), reply: None };
        rt::blocking(move || link.shot(&command, "Alpha", &[vec![0xFF]])).await
    };
    c.check("…a command handed no link: nothing sent", !none, ());
    // The real bot's links: its own application's answer link only, not any webhook on Discord (2026-10-30 review).
    const BOT_APP: &str = "1561788269933498408"; // the bot's application (DISCORD_APPLICATION_ID)
    let token = "aW50ZXJhY3Rpb246MTIzNDU2Nzg5MDEyMzQ1Njc4OnRva2Vu_-.x";
    c.check(
        "pictures go only to Bloxkeep's bot's own answer link on Discord",
        is_bot_reply(&format!("https://discord.com/api/v10/webhooks/{BOT_APP}/{token}/messages/@original"))
            && !is_bot_reply(&format!("https://discord.com/api/webhooks/123456789012345678/{token}"))
            && !is_bot_reply(&format!("https://discord.com/api/v10/webhooks/123456789012345678/{token}/messages/@original"))
            && !is_bot_reply(&format!("https://discord.com/api/v10/webhooks/{BOT_APP}/{token}/messages/@original?wait=true"))
            && !is_bot_reply(&format!("https://discord.com.evil.example/api/v10/webhooks/{BOT_APP}/{token}/messages/@original"))
            && !is_bot_reply(&format!("http://discord.com/api/v10/webhooks/{BOT_APP}/{token}/messages/@original")),
        (),
    );

    // Very many names: what's sent stays under what the bot takes (it closed a bigger one at every connection).
    // PORT: the C# read the link's private _hello; here the hello the stand-in bot got is measured (the link is connected).
    let many: Vec<String> = (0..300).map(|n| format!("Tài khoản số {n} của tôi, với một cái tên dài thật là dài")).collect();
    let hellos = bot.count("hello");
    link.hello(&machine_name(), &many, &many, "vi");
    until(&app, || bot.count("hello") > hellos, 5000).await;
    let big = bot.last("hello").unwrap_or(Value::Null);
    let big_text = crate::core::models::to_net_json_string(&big).unwrap_or_default();
    let big_len = big_text.encode_utf16().count();
    c.check(
        "very many accounts and presets: what's said of them stays under the bot's 16,000 characters, some of each kept",
        big_len < 16_000 && big["presets"].as_array().is_some_and(|a| a.len() > 10) && big["accounts"].as_array().is_some_and(|a| a.len() > 10),
        big_len,
    );

    // A command that takes long (ten windows pictured, about a minute): nothing else waits behind it.
    // PORT: the C# swapped the link's private command for one held open; here the core's thread (which answers commands)
    // is held instead, without yielding, while the link's own threads must still take the question.
    {
        // Both sent without the core's thread getting a turn in between.
        let tx = bot.s.current.lock().unwrap().as_ref().map(|(tx, _)| tx.clone()).expect("open");
        let _ = tx.send(json!({ "t": "command", "id": "slow", "command": "status" }).to_string());
        let _ = tx.send(json!({ "t": "ask", "user": "@later", "id": "10" }).to_string());
    }
    let held = Instant::now();
    let mut shown = false;
    while held.elapsed() < Duration::from_secs(3) {
        if app.remote.remote.borrow().as_ref().and_then(|r| r.asking()).is_some_and(|a| a.id.as_deref() == Some("10")) {
            shown = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let answered_meanwhile = bot.heard().iter().any(|m| m["t"] == "answer" && m["id"] == "slow");
    c.check("a command still running holds nothing back: a question to allow shows meanwhile", shown && !answered_meanwhile, (shown, answered_meanwhile));
    c.check(
        "…and the command's answer goes once it's done",
        until(&app, || bot.heard().iter().any(|m| m["t"] == "answer" && m["id"] == "slow"), 8000).await,
        (),
    );
    let _ = call(&app, "remoteAllow", json!({ "allow": false, "id": "10" })).await;

    // Discord's own Hide account names: text the app wrote for itself (a launch error) says no name in Discord either.
    let set_hide = |on: bool| {
        if let Some(v) = app.controller.vault.borrow().as_ref() {
            v.data().discord.hide_names = on;
        }
    };
    set_hide(true);
    let said = app.remote_say("Alpha: Roblox didn't open in time.");
    c.check(
        "Discord's Hide account names on: an answer's text names no account",
        said["content"] == "Account 1: Roblox didn't open in time.",
        said.to_string(),
    );
    set_hide(false);

    // Locked: nothing but that, and no names to suggest.
    let _ = call(&app, "lock", json!({})).await;
    let said = text(&ask(&app, &bot, "status", None).await);
    c.check("locked: a command is answered only that it is locked", said == "Bloxkeep is locked on this PC. Unlock it there first.", &said);
    c.check(
        "…and Discord is sent no names to suggest",
        until(&app, || bot.last("hello").is_some_and(|h| names(&h, "accounts").is_empty() && names(&h, "presets").is_empty()), 8000).await,
        bot.last("hello"),
    );
    unlock(&app).await;

    // A drop: connected again, the names said again.
    let (before, hellos) = (bot.connections(), bot.count("hello"));
    bot.drop_socket();
    c.check(
        "dropped: it connects again and says its names again",
        until(&app, || bot.connections() == before + 1 && field("connected") == true && bot.count("hello") > hellos, 10_000).await,
        format!("{} {}", bot.connections(), bot.count("hello")),
    );
    c.check("…with the same secret (still linked)", bot.last_secret().as_deref() == Some(secret.as_str()), ());

    // Turned off: the bot told to unlink, the connection closed, nothing more.
    let _ = call(&app, "setRemote", json!({ "enabled": false })).await;
    c.check(
        "turned off: the bot is told to unlink, and the connection closes",
        until(&app, || bot.last("unlink").is_some() && bot.open() == 0, 8000).await,
        (bot.open(), bot.count("unlink")),
    );
    c.check("…and stays closed", !until(&app, || bot.open() > 0, 2500).await && field("on") == false, ());
    // Its secret goes too: the unlink may not have reached the bot (offline), and with the same secret the PC came back
    // still linked, without asking (2026-10-30 review).
    c.check("…and its secret is forgotten", prefs::lock().remote_key().is_none(), ());
    // The unlink may not have got through: told again on a connection of its own until the bot says it's done.
    let gone = prefs::lock().remote_gone.len();
    c.check("…kept to tell the bot again", gone == 1 && !prefs_text(dir).contains(&secret), gone);
    let forget_now = || {
        app.remote.next_remote_forget.set(crate::core::models::stamp_min());
        app.tick_remote();
    };
    let unlinks = bot.count("unlink");
    forget_now();
    c.check(
        "…told on its own connection, with the old secret",
        until(&app, || bot.count("unlink") == unlinks + 1 && bot.last_secret().as_deref() == Some(secret.as_str()), 8000).await,
        bot.count("unlink"),
    );
    bot.drop_socket();
    c.check(
        "…the bot gone before it answers: still kept",
        until(&app, || !app.remote.remote_forgetting.get(), 8000).await && prefs::lock().remote_gone.len() == 1,
        prefs::lock().remote_gone.len(),
    );
    forget_now();
    until(&app, || bot.count("unlink") == unlinks + 2, 8000).await;
    bot.send(json!({ "t": "link", "linked": false })).await;
    c.check(
        "…the bot says it's linked to nobody: done, and that's saved",
        until(&app, || prefs::lock().remote_gone.is_empty() && !app.remote.remote_forgetting.get(), 8000).await
            && !prefs_text(dir).contains("\"remoteGone\":[\""),
        prefs::lock().remote_gone.len(),
    );
    until(&app, || bot.open() == 0, 8000).await;
    let _ = call(&app, "setRemote", json!({ "enabled": true })).await;
    c.check(
        "turned on again: a new secret (a new PC for the bot, linked to nobody)",
        until(&app, || bot.open() == 1 && field("connected") == true, 8000).await
            && bot.last_secret().is_some_and(|again| again.len() >= 40 && again != secret),
        (),
    );
    let first = bot.last_secret().unwrap_or_default();

    // Each vault its own: vaults are for different people, and one person's /pc must not reach another's accounts. A
    // switch from before (one for the whole PC) goes to the vault last used.
    prefs::save();
    prefs::lock().vault_list.clear();
    prefs::load();
    vaults::load();
    let mine = vaults::current();
    let mine_ok = mine.as_ref().is_some_and(|m| m.remote_control && m.remote_key.as_ref().is_some_and(|k| !k.is_empty())) && prefs::lock().remote_control();
    c.check(
        "from before each vault had its own: Control from Discord and its secret go to the vault last used",
        mine_ok && until(&app, || field("connected") == true, 8000).await && bot.last_secret().as_deref() == Some(first.as_str()),
        format!("{:?} {}", mine.as_ref().map(|m| m.remote_control), bot.last_secret().as_deref() == Some(first.as_str())),
    );
    let _ = call(&app, "addVault", json!({})).await;
    let _ = call(&app, "createVault", json!({ "password": "another vault's password", "name": "Other" })).await;
    c.check(
        "another vault open: this one's link closes, and it's off there",
        until(&app, || bot.open() == 0, 8000).await && field("on") == false && vaults::current().map(|v| v.id) != mine.as_ref().map(|m| m.id.clone()),
        (),
    );
    let _ = call(&app, "setRemote", json!({ "enabled": true })).await;
    c.check(
        "…turned on there: a PC of its own for the bot, its own names only",
        until(
            &app,
            || {
                bot.open() == 1
                    && bot.last_secret().as_deref() != Some(first.as_str())
                    && bot.last("hello").is_some_and(|h| names(&h, "accounts").is_empty() && names(&h, "presets").is_empty())
            },
            8000,
        )
        .await,
        bot.last_secret().as_deref() == Some(first.as_str()),
    );
    let _ = call(&app, "selectVault", json!({ "id": mine.as_ref().map(|m| m.id.clone()).unwrap_or_default() })).await;
    let back = until(&app, || bot.open() == 1 && bot.last_secret().as_deref() == Some(first.as_str()), 8000).await;
    let said = text(&ask(&app, &bot, "status", None).await);
    c.check(
        "the first vault offered again: its own link, answering only that it's locked",
        back && said == "Bloxkeep is locked on this PC. Unlock it there first.",
        &said,
    );
    unlock(&app).await;
    c.check(
        "…unlocked: its own accounts",
        until(&app, || bot.last("hello").is_some_and(|h| names(&h, "accounts") == "Alpha,Beta"), 8000).await,
        bot.last("hello"),
    );

    // A bot that closes the connection at once (another Bloxkeep with this secret, a hello it won't take): connected again
    // later and later, not every 2-3 s forever (2026-10-30 review).
    bot.s.close_at_once.store(true, Ordering::SeqCst);
    let tries = bot.connections();
    bot.drop_socket();
    until(&app, || false, 9500).await;
    c.check("closed at once by the bot each time: it waits longer each time", bot.connections() - tries <= 2, bot.connections() - tries);
    app.stop_remote();
    drop(bot);
    // SAFETY: as above.
    unsafe { std::env::remove_var("BLOXKEEP_REMOTE_URL") };
}
