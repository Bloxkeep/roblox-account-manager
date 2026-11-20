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

pub async fn run(c: &mut Checks, dir: &str) {
    // A signed-in ask of Roblox carries a browser tracker id beside the sign-in (Roblox's settings calls answer 400
    // "Request Context BrowserTrackerID is missing or invalid" without one), the same id all run long.
    {
        let (a, b) = (crate::roblox::signed("abc"), crate::roblox::signed("xyz"));
        let id = |t: &str| t.rsplit("browserid=").next().unwrap_or_default().to_string();
        c.check("a signed-in ask carries the sign-in and a browser tracker id", a.starts_with(".ROBLOSECURITY=abc; RBXEventTrackerV2=&browserid=") && id(&a).len() == 12 && id(&a).chars().all(|c| c.is_ascii_digit()), &a.replace("abc", "…"));
        c.check("…the same id for every account, all run long", id(&a) == id(&b) && b.starts_with(".ROBLOSECURITY=xyz; "), ());
    }

    // A call the interface made that failed (net::note_failed): in error.log with ids as "#", no query, no Windows user,
    // and once, not on every poll.
    {
        let user = std::env::var("USERNAME").unwrap_or_default();
        let url = "https://friends.roblox.com/v1/users/123456789/friends/find?userSort=1&cursor=abcdef";
        let why = format!("Roblox 403 for 987654321 in C:\\Users\\{user}\\AppData");
        crate::net::note_failed("Roblox tab", url, &why);
        crate::net::note_failed("Roblox tab", url, &why);
        let text = std::fs::read_to_string(std::path::Path::new(dir).join("error.log")).unwrap_or_default();
        let lines: Vec<&str> = text.lines().filter(|l| l.contains("Roblox tab: https://friends.roblox.com")).collect();
        c.check(
            "a failed call of the interface is in error.log once, ids as #, without its query or the Windows user",
            lines.len() == 1
                && lines[0].contains("/v1/users/#/friends/find failed (Roblox 403 for # in C:\\Users\\[user]\\AppData)")
                && !lines[0].contains("cursor")
                && (user.chars().count() < 3 || !lines[0].contains(&user)),
            &lines,
        );
    }

    // The 3D avatar's picture drawn on every core is the one a single core draws, dot for dot, and sooner.
    if let Some(model) = crate::mesh::blocks() {
        let view = crate::mesh::View { yaw: 0.7, pitch: 0.3, zoom: 0.8, room: 4, t: 1.3, at: [0.2, -0.1], heading: 0.4, stride: 1.1, pace: 0.6, up: 0., rise: 0., act: Default::default(), pack: [0; 4] };
        let time = |cores: usize| {
            let started = std::time::Instant::now();
            let mut last = Vec::new();
            for _ in 0..5 {
                last = model.draw_on(view, 560, cores);
            }
            (last, started.elapsed().as_secs_f64() * 1000. / 5.)
        };
        let (one, one_ms) = time(1);
        let all = std::thread::available_parallelism().map(|c| c.get()).unwrap_or(4);
        let (many, many_ms) = time(all);
        let (odd, _) = time(7);
        c.check("the avatar drawn on every core is the picture one core draws", one == many && one == odd && one.iter().any(|b| *b != 0), format!("{} bytes, {} differ", one.len(), one.iter().zip(&many).filter(|(a, b)| a != b).count()));
        c.check("…and it is drawn sooner", all < 2 || many_ms < one_ms, format!("one core {one_ms:.1} ms, {all} cores {many_ms:.1} ms a picture"));
        println!("avatar draw: one core {one_ms:.1} ms, {all} cores {many_ms:.1} ms a picture (560 dots)");
    }

    // Every theme, as the interface gets it (theme.rs): its words read on its page, and a block stands apart from the
    // page it is on. A third of the themes had blocks within 2.5 L* of their page (measured 2026-11-15).
    {
        use crate::theme::{GROUND, MUTED, PANEL, RAISED, TEXT, contrast, lightness, over, palette_of};
        let mut unreadable: Vec<String> = Vec::new();
        let mut flat: Vec<String> = Vec::new();
        let mut seen = 0;
        for id in crate::theme_table::GROUPS.iter().flat_map(|(_, list)| list.iter()).filter(|id| !matches!(**id, "system" | "custom")) {
            seen += 1;
            let p = palette_of(id);
            let solid = |h: gpui_kit::Hsla| gpui_kit::Hsla { a: 1., ..h };
            let page = solid(over(p.get(PANEL), solid(p.get(GROUND))));
            if contrast(p.get(TEXT), page) < 4.5 || contrast(p.get(MUTED), page) < 3. {
                unreadable.push(format!("{id} {:.1}/{:.1}", contrast(p.get(TEXT), page), contrast(p.get(MUTED), page)));
            }
            let step = (lightness(solid(p.get(RAISED))) - lightness(page)).abs();
            if step < 2.9 {
                flat.push(format!("{id} {step:.1}"));
            }
        }
        c.check("every theme's words read on its page", seen > 90 && unreadable.is_empty(), &unreadable);
        c.check("…and its blocks stand apart from the page", flat.is_empty(), &flat);
    }

    // Sync with Windows: the theme chosen for Windows' light apps or for its dark ones, never Custom or itself.
    {
        use crate::core::{prefs, themes};
        let before = {
            let s = prefs::lock();
            (s.theme.clone(), s.system_light.clone(), s.system_dark.clone())
        };
        {
            let mut s = prefs::lock();
            s.theme = themes::FOLLOW_WINDOWS.to_string();
            s.system_light = "ocean".into();
            s.system_dark = "sakura".into();
        }
        let shown = themes::current().id;
        c.check("Sync with Windows shows the theme chosen for Windows' light or dark apps", shown == "ocean" || shown == "sakura", shown);
        c.check(
            "…and only a theme of its own can be chosen for either",
            themes::plain(Some("custom"), "dark") == "dark" && themes::plain(Some("system"), "light") == "light" && themes::plain(Some("no-such"), "light") == "light" && themes::plain(Some("ocean"), "light") == "ocean",
            (),
        );
        let mut s = prefs::lock();
        (s.theme, s.system_light, s.system_dark) = before;
    }

    // A custom theme's code (Settings › Appearance › Custom): what is copied is what is taken back; anything else isn't.
    {
        use crate::pages::settings::theme_code::{code_of, look_of};
        let look = serde_json::json!({ "background": "#1B2A3A", "accent": "#f59e0b", "fade": 60, "blur": 12, "picture": "user/x.png" });
        let code = code_of(&look);
        let back = look_of(&code);
        c.check(
            "a theme's code holds its two colours, fade and blur, and gives them back",
            code == "bloxkeep:1b2a3a-f59e0b-60-12" && back == Some(serde_json::json!({ "background": "#1b2a3a", "accent": "#f59e0b", "fade": 60, "blur": 12 })),
            (&code, &back),
        );
        let odd = ["", "hello", "bloxkeep:", "bloxkeep:12345-f59e0b", "bloxkeep:zzzzzz-f59e0b-1-1", "other:1b2a3a-f59e0b-60-12"];
        c.check("…words that aren't a code are refused", odd.iter().all(|t| look_of(t).is_none()), odd.iter().filter(|t| look_of(t).is_some()).collect::<Vec<_>>());
        let wild = look_of("bloxkeep:1b2a3a-f59e0b-900-77");
        c.check("…and a fade or blur out of range is brought into it", wild == Some(serde_json::json!({ "background": "#1b2a3a", "accent": "#f59e0b", "fade": 90, "blur": 30 })), &wild);
    }

    // About how many windows the PC runs (capacity.rs): what runs out first sets it, never fewer than are open.
    {
        use crate::core::capacity::{Each, Now, estimate};
        let each = Each { ram_mb: 1500., commit_mb: 2500., cpu: 6., gpu: 8. };
        let roomy = Now { windows: 2, total_mb: 32768., available_mb: 20000., commit_limit_mb: 60000., commit_room_mb: 40000., cpu_busy: Some(20.), gpu_busy: Some(16.) };
        // RAM (20000 - 2621) / 1500 = 11; commit 39000 / 2500 = 15; CPU 70 / 6 = 11; GPU 79 / 8 = 9.
        let fit = estimate(&each, &roomy);
        c.check("the GPU runs out first: two open and nine more", fit.is_some_and(|f| f.max == 11 && f.by == "gpu"), fit);
        let small_page_file = Now { commit_room_mb: 6000., ..roomy };
        let fit = estimate(&each, &small_page_file);
        c.check("a small page file is what runs out first", fit.is_some_and(|f| f.max == 3 && f.by == "commit"), fit);
        let full = Now { available_mb: 900., ..roomy };
        let fit = estimate(&each, &full);
        c.check("a full PC never says fewer than are open", fit.is_some_and(|f| f.max == 2 && f.by == "ram"), fit);
        let unknown = Now { cpu_busy: None, gpu_busy: None, commit_limit_mb: 0., windows: 0, ..roomy };
        let fit = estimate(&each, &unknown);
        c.check("with only RAM known, RAM sets it", fit.is_some_and(|f| f.max == 11 && f.by == "ram"), fit);
        c.check("nothing measured: nothing said", estimate(&Each::default(), &roomy).is_none(), ());
    }

    // A saved file of Bloxkeep's cut short: the list starts empty, and error.log says so; a missing one says nothing.
    {
        let cut = std::path::Path::new(dir).join("watch-cut.json");
        let _ = std::fs::write(&cut, "[{\"id\": 12");
        let read: Option<Vec<serde_json::Value>> = crate::net::saved("Watched items", &cut);
        let none: Option<Vec<serde_json::Value>> = crate::net::saved("Missing list", &std::path::Path::new(dir).join("never-saved.json"));
        let text = std::fs::read_to_string(std::path::Path::new(dir).join("error.log")).unwrap_or_default();
        c.check(
            "a saved file cut short starts empty and is in error.log; a missing one isn't",
            read.is_none() && none.is_none() && text.contains("Watched items: ") && text.contains("isn't readable, started empty") && !text.contains("Missing list"),
            text.lines().filter(|l| l.contains("Watched") || l.contains("Missing")).collect::<Vec<_>>(),
        );
    }

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

    // A game's events (Roblox's virtual-events v3, the shape its own API description gives): what is on or to come,
    // soonest first, by its name in the account's language; one that is over or not for everyone is left out.
    let now = crate::time::iso_secs("2026-11-15T10:00:00Z").unwrap_or(0);
    let answer = serde_json::json!({ "gameEvents": [
        { "id": "3", "title": "Later", "startTime": "2026-11-20T10:00:00Z", "endTime": "2026-11-21T10:00:00Z", "visibility": "public", "userRsvpStatus": "going" },
        { "id": "1", "title": "Over", "startTime": "2026-11-01T10:00:00Z", "endTime": "2026-11-02T10:00:00Z", "visibility": "public" },
        { "id": "2", "title": "Now", "displayTitle": "Đang diễn ra", "subtitle": "Double XP", "startTime": "2026-11-15T09:00:00Z", "endTime": "2026-11-15T12:00:00Z", "visibility": "public", "userRsvpStatus": "none" },
        { "id": "4", "title": "Hidden", "startTime": "2026-11-16T10:00:00Z", "endTime": "2026-11-17T10:00:00Z", "visibility": "private" },
        { "id": "5", "title": "", "startTime": "2026-11-16T10:00:00Z", "endTime": "2026-11-17T10:00:00Z" },
    ], "nextPageToken": null });
    let events = crate::roblox::read_events(&answer, now);
    let names: Vec<&str> = events.iter().map(|e| e.title.as_str()).collect();
    c.check("a game's events: on now first, then to come; over, private and nameless left out", names == ["Đang diễn ra", "Later"], &names);
    c.check("…with the line under its name, and the one the account is going to", events.first().is_some_and(|e| e.sub == "Double XP" && !e.going) && events.last().is_some_and(|e| e.going), &events);
    c.check("…and no events at all from an answer without them", crate::roblox::read_events(&serde_json::json!({ "errors": [{ "code": 0 }] }), now).is_empty(), "");
}
