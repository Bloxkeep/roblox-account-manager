// Roblox's own words in error.log (roblox_log_watcher::keep_line, app/roblox_words.rs): which of a Roblox log's lines are
// kept for a window that ends. Made-up lines in the client's format; the edge cases a review found (2026-11-15).
use super::Checks;
use crate::core::roblox_log_watcher::kept_from;

fn line(clock: &str, uptime: &str, text: &str) -> String {
    format!("2026-11-14T{clock}.100Z,{uptime},6f2c,6 {text}\n")
}

fn filler(n: usize) -> String {
    // Each with its own words (lines differing only in numbers count once).
    (0..n).map(|i| line("10:00:30", "30.5", &format!("[FLog::Graphics] pass {} drawn", char::from(b'a' + (i % 26) as u8)))).collect()
}

pub async fn run(c: &mut Checks, _dir: &str) {
    // A teleport's join link carries the account's id and names URL-encoded, past any scrub.
    let teleport = line(
        "10:00:01",
        "1.2",
        r#"[FLog::GameJoinUtil] doTeleport: joinScriptUrl = https://gamejoin.roblox.com/v1/join?ticket={"UserId"%3a123456789%2c"UserName"%3a"Someone"}"#,
    );
    let kept = kept_from(&(teleport.clone() + &filler(8)));
    c.check("a join link's line (ids, names) is never kept", kept.iter().all(|l| !l.contains("joinScriptUrl") && !l.contains("123456789")), &kept);

    // The game's own output names other players.
    let output = line("10:00:02", "2.0", "[FLog::Output] Players.SomeoneElse.PlayerScripts error: nil value");
    let kept = kept_from(&(output + &filler(8)));
    c.check("the game's own output is never kept", kept.iter().all(|l| !l.contains("SomeoneElse")), &kept);

    // Uptime 403.97 s must not read as HTTP 403.
    let uptime = line("10:06:43", "403.979248", "[FLog::Graphics] quality level 5");
    let kept = kept_from(&(uptime + &filler(8)));
    c.check("a line 403 s into the run isn't taken for an HTTP 403", kept.iter().all(|l| !l.contains("quality level")), &kept);

    // A game's missing sounds can't push out the disconnect.
    let mut text = line("10:01:00", "60.0", "[FLog::Network] Sending disconnect with reason: 277");
    for i in 0..40 {
        text += &line("10:01:10", "70.0", &format!("[FLog::Error] Failed to load sound rbxassetid://{i}: HTTP 403"));
    }
    text += &filler(8);
    let kept = kept_from(&text);
    c.check("the disconnect stays under forty failed sounds", kept.iter().any(|l| l.contains("disconnect with reason: 277")), &kept);
    c.check("…and the sounds aren't kept", kept.iter().all(|l| !l.contains("rbxassetid")), &kept);

    // A retry loop's lines differ only in numbers: kept once.
    let mut text = String::new();
    for i in 0..30 {
        text += &line("10:02:00", "120.0", &format!("[FLog::HttpTraceError] HttpError #{i} https://apis.roblox.com/x time: {}ms Timedout", i * 7));
    }
    text += &filler(8);
    let kept = kept_from(&text);
    let loops = kept.iter().filter(|l| l.contains("HttpError")).count();
    c.check("a retry loop's thirty lines are kept as one", loops == 1, &kept);

    // Roblox's sign-in failure at start.
    let web = line("10:03:00", "1.5", "[FLog::WebLoginProtocol] Error performing web login, error code: 403, error message: Authentication ticket was invalid.");
    let kept = kept_from(&(web + &filler(8)));
    c.check("Roblox's sign-in error at start is kept", kept.iter().any(|l| l.contains("Error performing web login")), &kept);
    c.check("…with its time, without the uptime and thread", kept.iter().any(|l| l.starts_with("10:03:00 [FLog::WebLoginProtocol]")), &kept);

    // The redeem's own success line has "authentication-ticket", not a ticket.
    let redeem = line("10:03:01", "1.6", "Warning [FLog::Warning] Set-Cookie RBXEventTrackerV2 detected from https://auth.roblox.com/v1/authentication-ticket/redeem");
    let kept = kept_from(&(redeem + &filler(8)));
    c.check("the ticket redeem's line is kept", kept.iter().any(|l| l.contains("authentication-ticket/redeem")), &kept);

    // Odd lines: no panic, cut to length.
    let odd = "2026-11-14T07:57:0é,x,y,z [FLog::Network] disconnect é\n".to_string() + &"x".repeat(5000) + " error\n";
    let kept = kept_from(&odd);
    c.check("a line with a letter across the clock's end doesn't stop the reader", !kept.is_empty(), &kept);
    c.check("a 5000-letter line is cut", kept.iter().all(|l| l.chars().count() <= 241), kept.iter().map(|l| l.chars().count()).collect::<Vec<_>>());

    // The last lines are kept whatever they say (how the window ended).
    let kept = kept_from(&filler(20));
    c.check("the last six lines are kept", kept.len() == 6 && kept.last().is_some_and(|l| l.contains("pass t drawn")), &kept);

    // Seen in the owner's real logs (2026-11-15): six memory callbacks and four heartbeats as the last lines.
    let mut text = filler(3);
    for i in 0..6 {
        text += &line("10:05:17", &format!("{}.2", 300 + i), "[FLog::SingleSurfaceApp] memoryPrioritizationCallback (level:3)");
    }
    for i in 0..4 {
        text += &line("10:05:20", "310.0", &format!("Info [FLog::SessionL2ValidationHelper] onSessionHeartbeat sc_count 202 sh_count {} cur_l2ts {}.05", 2745 + i, 42313 + 20 * i));
    }
    let kept = kept_from(&text);
    c.check("a line said over and over at the end is kept once", kept.iter().filter(|l| l.contains("memoryPrioritization")).count() == 1 && kept.iter().filter(|l| l.contains("Heartbeat")).count() == 1, &kept);

    // A join's routine steps don't take the places of the lines that say why.
    let mut text = line("10:06:00", "360.0", "Info [DFLog::NetworkClient] Client:Disconnect");
    for i in 0..20 {
        text += &line("10:06:01", "361.0", &format!("[FLog::Network] Replicator created: 00000217D7B8{:04X}", i));
        text += &line("10:06:01", "361.0", &format!("[FLog::Network] Assigned peer id = {i}"));
    }
    text += &filler(8);
    let kept = kept_from(&text);
    c.check("routine join steps aren't kept, the disconnect is", kept.iter().any(|l| l.contains("Client:Disconnect")) && kept.iter().all(|l| !l.contains("Replicator") && !l.contains("peer id")), &kept);

    // Pointers differ in letters too: "#3 0x22b8d282af0" and "#4 0x22b8d2831f0" are the same line.
    let mut text = String::new();
    for p in ["0x22b8d281cf0", "0x22b8d282af0", "0x22b8d2831f0"] {
        text += &line("10:07:00", "420.0", &format!("[DFLog::HttpTraceError] HttpResponse(#1 {p}) status:404 Not Found url:{{ \"https://apis.roblox.com/\" }}"));
    }
    text += &filler(8);
    let kept = kept_from(&text);
    c.check("the same HTTP error with other pointers is kept once", kept.iter().filter(|l| l.contains("HttpResponse")).count() == 1, &kept);
}
