// Tests/Bloxkeep.Tests/DiscordSuite.cs: every Discord message Bloxkeep can send, built with made-up accounts, games and
// play (nothing is sent: the builders and discord_webhook::build are called directly), in English and Vietnamese, and
// checked against docs/discord-style.md: sender, footer and logo, colours, titles, three short fields to a row, names
// hidden, plain text, pings. The C# also wrote the payloads to ui/dev/discord-samples.json for a preview page; the
// checks don't read it and the repository is left alone here, so the pictures go to the suite's own folder only.
use std::{collections::HashMap, sync::Arc};

use serde_json::Value;

use super::{window::quiet_app as app, Checks, PASSWORD};
use crate::core::{
    app::{discord::BoardRow, discord_alerts::DiscordAlert},
    base::{Cancel, Guid, Stamp},
    discord_webhook::{self as webhook, DiscordMessage, DiscordPing},
    l,
    models::{Account, LaunchPreset, PlaySession, PresetSchedule},
    roblox_api::GameInfo,
    summary_card,
    vault::Vault,
};

const HOOK: &str = "https://discord.com/api/webhooks/100000000000000001/abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_abcdef";
const PALETTE: [i32; 5] = [webhook::BLUE, webhook::GREEN, webhook::YELLOW, webhook::RED, webhook::GREY];

struct Sample {
    order: i32,
    label: String,
    language: String,
    payload: Value,
    files: Option<HashMap<String, String>>,
}

/// A stand-in for a picture Roblox would give: a coloured disc or square (the C#'s letters left out: nothing reads them).
fn stand_in(color: (u8, u8, u8), round: bool) -> Vec<u8> {
    let img = image::RgbaImage::from_fn(150, 150, |x, y| {
        let (dx, dy) = (x as f64 - 74.5, y as f64 - 74.5);
        if !round || dx * dx + dy * dy <= 75.0 * 75.0 { image::Rgba([color.0, color.1, color.2, 255]) } else { image::Rgba([0, 0, 0, 0]) }
    });
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(img).write_to(&mut out, image::ImageFormat::Png).expect("a png");
    out.into_inner()
}

/// A small fixed sequence for the made-up play (System.Random(7)'s own numbers don't matter, only that they are fixed).
struct Lcg(u64);
impl Lcg {
    fn next(&mut self, from: i64, to: i64) -> i64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        from + ((self.0 >> 33) as i64).rem_euclid(to - from)
    }
}

pub async fn run(c: &mut Checks, dir: &str) {
    let pictures = std::path::Path::new(dir).join("pictures");
    std::fs::create_dir_all(&pictures).expect("pictures folder");

    // ── Made-up accounts, games, presets and two weeks of play ──
    let accounts = vec![
        Account { username: "NoobFarm2".into(), user_id: 9100000001, ..Default::default() },
        Account { username: "BloxGrinder_07".into(), user_id: 9100000002, ..Default::default() },
        Account { username: "pixelknight".into(), alias: "Pixel*Knight".into(), user_id: 9100000003, ..Default::default() },
        Account { username: "AfkBot_4".into(), user_id: 9100000004, ..Default::default() },
        Account { username: "AfkBot_5".into(), user_id: 9100000005, ..Default::default() },
        Account { username: "SunnyMain".into(), user_id: 9100000006, ..Default::default() },
    ];
    let games: [(i64, i64, &str, (u8, u8, u8)); 5] = [
        (2753915549, 994732206, "Blox Fruits", (0x2B, 0x7B, 0xD6)),
        (8737899170, 3317771874, "Pet Simulator 99", (0xE0, 0x8A, 0x1E)),
        (920587237, 383310974, "Adopt Me!", (0xD9, 0x4F, 0x8A)),
        (6516141723, 2440500124, "DOORS", (0x5B, 0x4A, 0x3A)),
        (4924922222, 1686885941, "Brookhaven RP", (0x3F, 0xA3, 0x5B)),
    ];
    let avatar_url = |i: usize| format!("https://tr.rbxcdn.com/30DAY-AvatarHeadshot-SAMPLE{}/150/150/AvatarHeadshot/Png/noFilter", i + 1);
    let icon_url = |i: usize| format!("https://tr.rbxcdn.com/180DAY-GameIcon-SAMPLE{}/150/150/Image/Png/noFilter", i + 1);
    let avatar_colors = [(0x4E, 0x8C, 0xC9), (0xC9, 0x6B, 0x4E), (0x7A, 0x5C, 0xC4), (0x4E, 0xA8, 0x7A), (0xB8, 0x9A, 0x3C), (0xC4, 0x5C, 0x8E)];
    let mut avatar_bytes: HashMap<String, Vec<u8>> = HashMap::new();
    for (i, color) in avatar_colors.iter().enumerate() {
        let png = stand_in(*color, true);
        let _ = std::fs::write(pictures.join(format!("avatar-{}.png", i + 1)), &png);
        avatar_bytes.insert(avatar_url(i), png);
    }
    for (i, g) in games.iter().enumerate() {
        let _ = std::fs::write(pictures.join(format!("game-{}.png", i + 1)), stand_in(g.3, false));
    }

    let vault = Vault::create(PASSWORD, None).expect("vault");
    let farm = LaunchPreset {
        name: "Farm".into(),
        account_ids: vec![accounts[0].id, accounts[1].id, accounts[2].id],
        place_input: "2753915549".into(),
        game_name: "Blox Fruits".into(),
        schedule: Some(PresetSchedule { enabled: true, time: "20:00".into(), days: vec![0, 1, 2, 3, 4, 5, 6], ..Default::default() }),
        ..Default::default()
    };
    let afk = LaunchPreset {
        name: "AFK night".into(),
        account_ids: vec![accounts[3].id, accounts[4].id],
        place_input: "8737899170".into(),
        game_name: "Pet Simulator 99".into(),
        ..Default::default()
    };
    let now = Stamp::utc_now();
    let mut random = Lcg(7);
    let plays: [&[usize]; 6] = [&[0, 0, 0, 4], &[0, 0, 3], &[0, 0, 0], &[1, 1], &[1, 1], &[2, 3, 4, 2]];
    let preset_of: [Option<Guid>; 6] = [Some(farm.id), Some(farm.id), Some(farm.id), Some(afk.id), Some(afk.id), None];
    let codes = [277, 277, 279, 267, 278];
    let utc_midnight = Stamp::from_unix_secs(now.unix_secs() - now.unix_secs().rem_euclid(86400));
    {
        let mut v = vault.data();
        v.accounts.extend(accounts.iter().cloned());
        v.settings.presets.push(farm.clone());
        v.settings.presets.push(afk.clone());
        for day in (0..=13).rev() {
            for (a, account) in accounts.iter().enumerate() {
                let mut at = utc_midnight.plus_secs((-day as f64) * 86400.0 + (-7 + random.next(0, 6)) as f64 * 3600.0);
                for &g in plays[a] {
                    let length = random.next(25, 170) as f64 * 60.0;
                    if at.plus_secs(length) > now.plus_secs(-300.0) {
                        break;
                    }
                    let dropped = random.next(0, 5) == 0;
                    v.play_sessions.push(PlaySession {
                        account_id: account.id,
                        account_label: Some(account.label()),
                        place_id: games[g].0,
                        universe_id: games[g].1,
                        game_name: games[g].2.into(),
                        started_at: at,
                        ended_at: Some(at.plus_secs(length)),
                        last_seen_at: Some(at.plus_secs(length)),
                        end_reason: Some(if dropped { "disconnected" } else { "left" }.into()),
                        reason_code: if dropped { Some(codes[random.next(0, codes.len() as i64) as usize]) } else { None },
                        preset_id: if g == 2 || g == 4 { None } else { preset_of[a] },
                        ..Default::default()
                    });
                    at = at.plus_secs(length + random.next(2, 40) as f64 * 60.0);
                }
            }
        }
        // Today's run of the scheduled preset, for the summary after it.
        let today = Stamp::now().local();
        let local_midnight = Stamp::from_local(today.year, today.month, today.day, 0, 0, 0).to_utc().plus_secs(60.0);
        for a in [0usize, 1, 2] {
            let start = std::cmp::max(now.plus_secs(-3.0 * 3600.0), local_midnight);
            v.play_sessions.push(PlaySession {
                account_id: accounts[a].id,
                account_label: Some(accounts[a].label()),
                place_id: games[0].0,
                universe_id: games[0].1,
                game_name: games[0].2.into(),
                started_at: start.plus_secs(a as f64 * 240.0),
                ended_at: Some(now.plus_secs(-(2.0 + a as f64) * 60.0)),
                end_reason: Some(if a == 1 { "disconnected" } else { "left" }.into()),
                reason_code: if a == 1 { Some(277) } else { None },
                preset_id: Some(farm.id),
                ..Default::default()
            });
        }
        v.play_sessions.sort_by_key(|s| s.started_at);
        // Never sent: nothing here posts, and the process timer is held off (quiet_app).
        v.discord.webhook_url = HOOK.into();
        v.discord.enabled = true;
        v.discord.events = ["drop", "schedule", "signin", "ram", "launch"].iter().map(|s| s.to_string()).collect();
    }
    let history: Vec<PlaySession> = vault.data().play_sessions.clone(); // each summary sees this, not the alerts' stays

    let (app, _said) = app();
    // The pictures and games Roblox would have given, before anything could ask Roblox; the vault set without an unlock,
    // so nothing starts that asks Roblox's servers.
    for (i, a) in accounts.iter().enumerate().take(accounts.len() - 1) {
        // the last one has no picture: the summary shows its initials
        app.controller.avatar_urls.borrow_mut().insert(a.user_id, avatar_url(i));
    }
    for (i, g) in games.iter().enumerate() {
        app.controller.games.borrow_mut().insert(
            g.0,
            Some(GameInfo { place_id: g.0, universe_id: g.1, name: g.2.into(), creator: "Someone".into(), playing: 1000, icon_url: Some(icon_url(i)) }),
        );
    }
    {
        let mut cache = summary_card::AVATARS.lock().unwrap();
        for (url, png) in &avatar_bytes {
            cache.insert(url.clone(), png.clone());
        }
    }
    *app.controller.vault.borrow_mut() = Some(Arc::new(vault));

    let mut samples: Vec<Sample> = Vec::new();
    for language in ["en", "vi"] {
        let _ = app.vault_mut(|v| {
            v.discord.language = language.into();
            v.discord.hide_names = false;
            v.discord.pings.clear();
        });
        let mut order = 0;
        let d = || app.vault(|v| v.discord.clone()).expect("vault");
        let mut add = |samples: &mut Vec<Sample>, label: &str, message: &DiscordMessage, ping: Option<&DiscordPing>, embed: bool, username: Option<&str>, avatar: Option<&str>, files: Option<HashMap<String, String>>| {
            let ds = d();
            let username = username.map(str::to_string).unwrap_or(ds.username.clone());
            let avatar = avatar.map(str::to_string).unwrap_or(ds.avatar_url.clone());
            let key = files.as_ref().and_then(|f| f.keys().next().cloned());
            let payload = webhook::build(message, ping.unwrap_or(&DiscordPing::none()), embed, Some(&username), Some(&avatar), key.as_deref());
            samples.push(Sample { order, label: label.into(), language: language.into(), payload: Value::Object(payload), files });
            order += 1;
        };
        macro_rules! add {
            ($label:expr, $msg:expr) => { add(&mut samples, $label, &$msg, None, true, None, None, None) };
        }

        // ── Test messages ──
        add!("Test message (Settings › Discord › Send test)", app.test_message(None, false));
        add!("Test message of a preset", app.test_message(Some(&farm), true));

        // ── Alerts, through notify as the app raises them ──
        let name = |a: usize| app.name(&accounts[a]);
        let stay = |a: usize, game: usize, played_min: f64, code: Option<i32>, open: bool| {
            app.stats.playing.borrow_mut().clear();
            let s = PlaySession {
                account_id: accounts[a].id,
                account_label: Some(accounts[a].label()),
                place_id: games[game].0,
                universe_id: games[game].1,
                game_name: games[game].2.into(),
                started_at: Stamp::utc_now().plus_secs(-played_min * 60.0),
                ended_at: if open { None } else { Some(Stamp::utc_now().plus_secs(-20.0)) },
                end_reason: if open { None } else { Some(if code.is_none() { "left" } else { "disconnected" }.into()) },
                reason_code: code,
                ..Default::default()
            };
            if open {
                app.stats.playing.borrow_mut().insert(accounts[a].id, s);
            } else {
                let _ = app.vault_mut(|v| v.play_sessions.push(s));
            }
        };
        // Raises events as notify does (right after the l::f that made the text), then makes the message they go out as.
        let raise = |raisers: Vec<Box<dyn Fn() + '_>>| -> (DiscordMessage, DiscordPing) {
            app.discord.events.borrow_mut().clear();
            for r in &raisers {
                app.discord.notified.borrow_mut().clear();
                r();
            }
            let events: Vec<(String, DiscordAlert, DiscordPing, Option<Guid>)> = app.discord.events.borrow_mut().drain(..).collect();
            let mut ping = DiscordPing::none();
            let mut alerts = Vec::new();
            for e in events {
                ping = ping.merge(&e.2);
                alerts.push(e.1);
            }
            (app.events_message(&alerts), ping)
        };
        let notify = |text: String, kind: &str, subject: Guid| app.notify(&text, "warn", kind, subject, false, None);
        let nil = Guid::nil();
        let id = |a: usize| accounts[a].id;

        stay(0, 0, 72.0, Some(277), false);
        let drop = raise(vec![Box::new(|| notify(l::f("{0} left the game — rejoining in {1}s…", &[&name(0), &5]), "drop", id(0)))]);
        add!("Alert · disconnected, rejoining", drop.0);
        stay(1, 0, 134.0, None, true);
        add!(
            "Alert · disconnected, reason unknown",
            raise(vec![Box::new(|| notify(l::f("{0} dropped out of the game, reason unknown — restarting Roblox…", &[&name(1)]), "drop", id(1)))]).0
        );
        add!(
            "Alert · lost connection, restarting Roblox",
            raise(vec![Box::new(|| notify(l::f("{0} lost connection — restarting Roblox…", &[&name(0)]), "drop", id(0)))]).0
        );
        add!(
            "Alert · couldn't get into the server",
            raise(vec![Box::new(|| notify(l::f("{0} couldn't get into that server (full or closed) — joining another one…", &[&name(1)]), "drop", id(1)))]).0
        );
        stay(2, 0, 3.0, Some(279), false);
        add!(
            "Alert · keeps getting disconnected",
            raise(vec![Box::new(|| notify(l::f("{0} keeps getting disconnected — Bloxkeep keeps rejoining every {1}s.", &[&name(2), &10]), "rejoinSlow", id(2)))]).0
        );
        add!(
            "Alert · couldn't rejoin (an error)",
            raise(vec![Box::new(|| notify(l::f("Couldn't rejoin {0}: {1}", &[&name(2), &"Roblox took too long to answer."]), "rejoinFail", id(2)))]).0
        );
        add!(
            "Alert · couldn't rejoin (Multi-Roblox not ready)",
            raise(vec![Box::new(|| {
                notify(l::f("Didn't rejoin {0}: Multi-Roblox isn't ready, so a new Roblox window would close the one that's open.", &[&name(2)]), "rejoinFail", id(2))
            })])
            .0
        );
        app.stats.playing.borrow_mut().clear();
        add!(
            "Alert · couldn't rejoin (no window)",
            raise(vec![Box::new(|| notify(l::f("{0} didn't rejoin: no Roblox window appeared.", &[&name(2)]), "rejoinFail", id(2)))]).0
        );
        stay(3, 1, 21.0, Some(278), false);
        add!(
            "Alert · kicked for being AFK",
            raise(vec![Box::new(|| {
                notify(
                    l::f("{0} was kicked for being AFK: the PC was in use without a pause of two seconds, so anti-AFK couldn't step in.", &[&name(3)]),
                    "afk",
                    id(3),
                )
            })])
            .0
        );
        add!("Alert · banned", raise(vec![Box::new(|| notify(l::f("{0} is banned on Roblox.", &[&name(4)]), "banned", id(4)))]).0);
        add!(
            "Alert · several banned",
            raise(vec![Box::new(|| {
                let names = app.names(&[accounts[3].clone(), accounts[4].clone()]);
                notify(l::p(2, "{0} account is banned on Roblox: {1}", "{0} accounts are banned on Roblox: {1}", &[&2, &names]), "banned", nil)
            })])
            .0
        );
        add!(
            "Alert · sign-in needed",
            raise(vec![Box::new(|| notify(app.signed_out_text(&[accounts[1].clone(), accounts[2].clone(), accounts[5].clone()]), "signin", nil))]).0
        );
        add!(
            "Alert · low RAM",
            raise(vec![Box::new(|| notify(l::f("Your PC is using {0}% of its RAM. Close some Roblox windows if things get slow.", &[&92]), "ram", nil))]).0
        );
        add!(
            "Alert · launch: accounts didn't open",
            raise(vec![Box::new(|| notify(l::p(2, "{0} account didn't open.", "{0} accounts didn't open.", &[&2]), "launch", nil))]).0
        );
        add!(
            "Alert · Roblox closed windows",
            raise(vec![Box::new(|| {
                notify(
                    l::f("Roblox closed {0} window(s) when another one opened — Roblox may have blocked running several windows. Multi-Roblox may need an update.", &[&2]),
                    "launch",
                    nil,
                )
            })])
            .0
        );
        let fid = farm.id;
        let fname = farm.name.clone();
        add!(
            "Alert · scheduled preset started",
            raise(vec![Box::new(|| {
                notify(l::p(3, "Scheduled preset {0} started: {1} account opened.", "Scheduled preset {0} started: {1} accounts opened.", &[&fname, &3]), "schedule", fid)
            })])
            .0
        );
        add!(
            "Alert · scheduled preset, already open",
            raise(vec![Box::new(|| notify(l::f("Scheduled preset {0}: its accounts were already open.", &[&fname]), "schedule", fid))]).0
        );
        let lang = d().language;
        add!(
            "Alert · scheduled preset didn't run",
            raise(vec![Box::new(|| {
                notify(l::f("Scheduled preset {0} didn't run: {1}", &[&fname, &l::t_in("Wait until Bloxkeep finishes opening Roblox.", &lang)]), "scheduleFail", fid)
            })])
            .0
        );
        add!(
            "Alert · scheduled preset, some didn't open",
            raise(vec![Box::new(|| {
                notify(
                    l::p(
                        1,
                        "Scheduled preset {0}: {1} account didn't open. {2}",
                        "Scheduled preset {0}: {1} accounts didn't open. {2}",
                        &[&fname, &1, &"pixelknight: Roblox didn't open in time."],
                    ),
                    "scheduleFail",
                    fid,
                )
            })])
            .0
        );
        add!(
            "Alert · scheduled preset missed",
            raise(vec![Box::new(|| notify(l::f("{0} didn't run at {1}: the PC was asleep or busy.", &[&fname, &"20:00"]), "scheduleMissed", fid))]).0
        );
        add!(
            "Alert · scheduled preset finished",
            raise(vec![Box::new(|| {
                notify(
                    l::p(3, "Scheduled preset {0} finished: closing {1} account.", "Scheduled preset {0} finished: closing {1} accounts.", &[&fname, &3]),
                    "scheduleStop",
                    fid,
                )
            })])
            .0
        );
        // Several at once: three drops (one message, a field each), and two about one account.
        stay(0, 0, 95.0, Some(277), false);
        stay(1, 0, 95.0, Some(277), false);
        let three = raise(vec![
            Box::new(|| notify(l::f("{0} left the game — rejoining in {1}s…", &[&name(0), &5]), "drop", id(0))),
            Box::new(|| notify(l::f("{0} left the game — rejoining in {1}s…", &[&name(1), &5]), "drop", id(1))),
            Box::new(|| notify(l::f("{0} is banned on Roblox.", &[&name(4)]), "banned", id(4))),
        ]);
        add!("Alerts · three at once", three.0);
        add!(
            "Alerts · two about one account",
            raise(vec![
                Box::new(|| notify(l::f("{0} left the game — rejoining in {1}s…", &[&name(2), &5]), "drop", id(2))),
                Box::new(|| notify(l::f("{0} didn't rejoin: no Roblox window appeared.", &[&name(2)]), "rejoinFail", id(2))),
            ])
            .0
        );
        // A ping chosen for drops: named in the message, and the only one allowed to ping.
        let _ = app.vault_mut(|v| {
            v.discord.pings.insert("drop".into(), "user:123456789012345678".into());
        });
        stay(0, 0, 40.0, Some(277), false);
        let pinged = raise(vec![Box::new(|| notify(l::f("{0} left the game — rejoining in {1}s…", &[&name(0), &5]), "drop", id(0)))]);
        let _ = app.vault_mut(|v| v.discord.pings.clear());
        add(&mut samples, "Alert with a ping", &pinged.0, Some(&pinged.1), true, None, None, None);
        add!("Alert after messages dropped while offline", app.with_dropped(&drop.0, 3));

        // ── The status board ──
        let rows = |rows: &[(usize, Option<usize>, &str, i64, i32, i64)]| -> Vec<BoardRow> {
            rows.iter()
                .map(|r| {
                    let parts: Vec<&str> = r.2.split(':').collect();
                    BoardRow {
                        name: accounts[r.0].label(),
                        place_id: r.1.map(|g| games[g].0),
                        game: r.1.map(|g| games[g].2.to_string()),
                        status: if r.2 == "banned" { "disconnected".into() } else { parts[0].into() },
                        seconds: r.3 * 60,
                        pid: r.4,
                        ram: r.5 * 1024 * 1024,
                        reason: if parts.len() == 2 && parts[0] == "disconnected" { parts[1].parse().ok() } else { None },
                        banned: r.2 == "banned",
                        phase: if parts.len() >= 2 && parts[0] != "disconnected" { Some(parts[1].to_string()) } else { None },
                        queue: if parts.len() == 3 && parts[1] == "full" { parts[2].parse().ok() } else { None },
                    }
                })
                .collect()
        };
        let board = |r: Vec<BoardRow>| app.board_message_of(5, &r, Stamp::utc_now());
        add!(
            "Board · six open, one cut off",
            board(rows(&[
                (0, Some(0), "ingame", 134, 14212, 812),
                (1, Some(0), "ingame", 131, 15880, 790),
                (2, Some(0), "disconnected:267", 128, 9932, 640),
                (3, Some(1), "ingame", 422, 20144, 455),
                (4, Some(1), "joining", 1, 21008, 210),
                (5, None, "menu", 6, 22540, 380),
            ]))
        );
        add!(
            "Board · all in game",
            board(rows(&[(0, Some(0), "ingame", 72, 14212, 812), (1, Some(0), "ingame", 70, 15880, 790), (2, Some(0), "ingame", 69, 9932, 760)]))
        );
        add!(
            "Board · cut off for different reasons",
            board(rows(&[
                (0, Some(0), "disconnected:288", 72, 14212, 812),
                (1, Some(0), "disconnected", 70, 15880, 790),
                (2, Some(1), "banned", 3, 9932, 410),
                (3, Some(1), "disconnected:299", 41, 20144, 455),
            ]))
        );
        add!(
            "Board · on the way in",
            board(rows(&[
                (0, Some(0), "ingame", 72, 14212, 812),
                (1, Some(0), "joining:waiting", 1, 15880, 390),
                (2, Some(1), "joining:teleporting", 41, 9932, 610),
                (3, Some(1), "menu:joinFailed", 2, 20144, 300),
                (4, Some(1), "joining:full:3", 1, 21008, 280),
            ]))
        );
        add!("Board · nobody playing", board(rows(&[])));
        add!("Board · Bloxkeep locked", app.final_board_message(false));
        add!("Board · Bloxkeep closed", app.final_board_message(true));

        // ── Summaries, with their picture ──
        let summary = |samples: &mut Vec<Sample>,
                       add: &mut dyn FnMut(&mut Vec<Sample>, &str, &DiscordMessage, Option<&DiscordPing>, bool, Option<&str>, Option<&str>, Option<HashMap<String, String>>),
                       label: &str,
                       days: i32,
                       due_at: Option<Stamp>,
                       today: bool,
                       preset: Option<&LaunchPreset>,
                       kind: &str,
                       embed: bool| {
            let _ = app.vault_mut(|v| v.play_sessions = history.clone());
            let ds = d();
            let message = match app.summary_message(&ds, days, due_at, Some(Stamp::now()), today, preset, kind) {
                Ok(m) => m,
                Err(e) => {
                    println!("  summary {label}: {e}");
                    return;
                }
            };
            let mut files = None;
            if let Some(card) = &message.card {
                let file = format!("summary-{language}-{}.png", samples.iter().filter(|s| s.language == language).count() + 1);
                if let Ok(png) = summary_card::render(card, &Cancel::new()) {
                    let _ = std::fs::write(pictures.join(&file), png);
                }
                files = Some(HashMap::from([("bloxkeep-summary.png".to_string(), format!("discord-samples/{file}"))]));
            }
            add(samples, label, &message, None, embed, None, None, files);
        };
        let minute_ago = || Some(Stamp::now().plus_secs(-60.0));
        summary(&mut samples, &mut add, "Summary · daily (24 hours)", 1, minute_ago(), false, None, "daily", true);
        summary(&mut samples, &mut add, "Summary · weekly (7 days)", 7, minute_ago(), false, None, "weekly", true);
        summary(&mut samples, &mut add, "Summary · daily and weekly at once", 7, minute_ago(), false, None, "both", true);
        summary(&mut samples, &mut add, "Summary · after the scheduled preset", 1, None, true, Some(&farm), "preset", true);
        summary(&mut samples, &mut add, "Summary · Send summary now (30 days)", 30, None, false, None, "now", true);
        summary(&mut samples, &mut add, "Summary · daily, sent late", 1, Some(Stamp::now().plus_secs(-3.0 * 3600.0)), false, None, "daily", true);
        let _ = app.vault_mut(|v| {
            v.discord.summary_accounts = vec![accounts[5].id];
            v.discord.summary_parts = ["total", "games", "kicks", "accounts"].iter().map(|s| s.to_string()).collect();
        });
        summary(&mut samples, &mut add, "Summary · chosen accounts, fewer parts", 1, minute_ago(), false, None, "daily", true);
        let _ = app.vault_mut(|v| {
            v.discord.summary_accounts = Vec::new();
            v.discord.summary_parts = ["total", "accounts", "games", "sessions", "kicks"].iter().map(|s| s.to_string()).collect();
        });
        summary(&mut samples, &mut add, "Summary · nothing played", 1, None, true, Some(&afk), "preset", true);

        // ── Names hidden, plain text, the person's own sender ──
        let _ = app.vault_mut(|v| v.discord.hide_names = true);
        stay(0, 0, 72.0, Some(277), false);
        let hidden = raise(vec![Box::new(|| notify(l::f("{0} left the game — rejoining in {1}s…", &[&name(0), &5]), "drop", id(0)))]);
        add!("Names hidden · alert", hidden.0);
        summary(&mut samples, &mut add, "Names hidden · summary", 1, minute_ago(), false, None, "daily", true);
        let _ = app.vault_mut(|v| v.discord.hide_names = false);
        add(&mut samples, "Plain text · alert", &drop.0, None, false, None, None, None);
        add(&mut samples, "Plain text · several alerts", &three.0, None, false, None, None, None);
        let plain_board = board(rows(&[(0, Some(0), "ingame", 72, 14212, 812), (2, Some(0), "disconnected", 69, 9932, 760)]));
        add(&mut samples, "Plain text · board", &plain_board, None, false, None, None, None);
        summary(&mut samples, &mut add, "Plain text · summary", 1, minute_ago(), false, None, "daily", false);
        let own = avatar_url(5);
        add(&mut samples, "Own sender name and picture (Settings › Discord)", &drop.0, None, true, Some("Farm watch"), Some(&own), None);
    }
    let _ = app.vault_mut(|v| v.discord.enabled = false);

    verify(c, &samples, &accounts);
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

fn to_text(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

fn verify(c: &mut Checks, samples: &[Sample], accounts: &[Account]) {
    let mut problems: Vec<String> = Vec::new();
    for sm in samples {
        let mut problem = |what: String| problems.push(format!("{} ({}): {what}", sm.label, sm.language));
        let p = &sm.payload;
        let sender = s(&p["username"]);
        if sender.is_empty() || s(&p["avatar_url"]).is_empty() {
            problem("no sender name or picture".into());
        }
        let wrong_sender = if sm.label.starts_with("Own sender") { sender != "Farm watch" } else { sender != "Bloxkeep" || s(&p["avatar_url"]) != webhook::LOGO };
        if wrong_sender {
            problem("sender isn't Bloxkeep with the logo (or the person's own)".into());
        }
        if let Some(embeds) = p["embeds"].as_array() {
            for (i, e) in embeds.iter().enumerate() {
                let title = s(&e["title"]);
                let len16 = title.encode_utf16().count();
                if len16 == 0 || len16 > 70 || title.ends_with('.') || title.chars().any(|ch| ch as u32 > 0xFFFF || ('☀'..='➿').contains(&ch)) {
                    problem(format!("title \"{title}\""));
                }
                if !e["color"].as_i64().is_some_and(|col| PALETTE.contains(&(col as i32))) {
                    problem("a colour that isn't Discord's".into());
                }
                if i == 0 && (!s(&e["footer"]["text"]).starts_with("Bloxkeep · ") || s(&e["footer"]["icon_url"]) != webhook::LOGO || e.get("timestamp").is_none_or(Value::is_null)) {
                    problem("footer, logo or time missing".into());
                }
                // The logo or the subject's picture beside it; not beside the board's accounts (their columns get the width).
                if i == 0 && e.get("thumbnail").is_none_or(Value::is_null) && !(sm.label.starts_with("Board") && e["fields"].is_array()) {
                    problem("no thumbnail".into());
                }
                for url in [&e["thumbnail"]["url"], &e["author"]["icon_url"]].into_iter().filter_map(|v| v.as_str()) {
                    if url != webhook::LOGO && !webhook::is_roblox_picture(Some(url)) {
                        problem(format!("a picture from elsewhere: {url}"));
                    }
                }
                let fields: Vec<&Value> = e["fields"].as_array().map(|f| f.iter().collect()).unwrap_or_default();
                if fields.len() > 25 {
                    problem("more than 25 fields".into());
                }
                // Three short fields to a row: one alone on the last row (not the board's accounts) goes to the small line.
                let inline = fields.iter().filter(|f| f["inline"].as_bool() == Some(true)).count();
                if !sm.label.starts_with("Board") && inline % 3 == 1 {
                    problem(format!("{inline} short fields: one alone on the last row"));
                }
                let text = to_text(e);
                if sm.language == "vi" && text.contains('—') {
                    problem("a dash in the Vietnamese".into());
                }
                if text.contains('⏱') || text.contains('🎮') || text.contains('🏆') {
                    problem("emoji".into());
                }
            }
        } else {
            let content = s(&p["content"]);
            if !content.starts_with("**") || content.contains("](https://") || !content.contains("\n-# Bloxkeep · ") {
                problem("plain text: no bold title, a link, or no footer line".into());
            }
        }
        let all = to_text(p);
        if sm.label.starts_with("Names hidden") {
            let leaked: Vec<String> = accounts.iter().map(Account::label).filter(|n| all.contains(n.as_str())).collect();
            if !leaked.is_empty() || all.contains("AvatarHeadshot") {
                problem(format!("a name or avatar while names are hidden: {}", leaked.join(", ")));
            }
        }
        let parse_empty = p["allowed_mentions"]["parse"].as_array().is_some_and(Vec::is_empty);
        if sm.label == "Alert with a ping"
            && (s(&p["content"]) != "<@123456789012345678>" || s(&p["allowed_mentions"]["users"][0]) != "123456789012345678" || !parse_empty)
        {
            problem("the ping or allowed_mentions".into());
        }
        if sm.label != "Alert with a ping" && !parse_empty {
            problem("mentions not off".into());
        }
    }
    c.check(&format!("{} messages follow docs/discord-style.md", samples.len()), problems.is_empty(), problems.iter().take(12).cloned().collect::<Vec<_>>().join(" | "));

    // Each message reads differently in Vietnamese (its words are translated), and a name typed with "*" stays text.
    let untranslated: Vec<String> = samples
        .iter()
        .filter(|en| en.language == "en" && !en.label.starts_with("Own sender"))
        .filter(|en| samples.iter().find(|v| v.language == "vi" && v.order == en.order).is_some_and(|vi| words(&en.payload) == words(&vi.payload)))
        .map(|en| en.label.clone())
        .collect();
    c.check("every message has its Vietnamese", untranslated.is_empty(), untranslated.join(", "));
    let first = |label: &str| samples.iter().find(|sm| sm.label.starts_with(label) && sm.language == "en");
    let board = first("Board · six").map(|sm| to_text(&sm.payload)).unwrap_or_default();
    c.check("a name with * is escaped", board.contains(r"Pixel\\*Knight"), board.len());
    // A window cut off says why on the board, as on its card: never a lost connection Roblox didn't say.
    let fields_of = |label: &str| -> String {
        first(label)
            .and_then(|sm| sm.payload["embeds"][0]["fields"].as_array().cloned())
            .unwrap_or_default()
            .iter()
            .map(|f| s(&f["value"]).to_string())
            .collect::<Vec<_>>()
            .join("\n")
    };
    let cut = fields_of("Board · cut off");
    c.check(
        "the board says why a window was cut off: the game's kick, a server shut down, a ban, a code, or a reason not known",
        fields_of("Board · six").contains("Kicked by the game")
            && cut.contains("Server shut down")
            && cut.contains("Banned by the game")
            && cut.contains("Disconnected · code 299")
            && cut.contains("Disconnected, reason unknown")
            && !cut.contains("Lost connection"),
        &cut,
    );
    let way = fields_of("Board · on the way in");
    c.check(
        "…and the way in: waiting for a server, teleporting, or left before a server took it",
        way.contains("Waiting for a server") && way.contains("Teleporting") && way.contains("Didn't get into the game") && way.contains("Server full · #3 in line"),
        &way,
    );
    let group = samples.iter().find(|sm| sm.label == "Alerts · three at once" && sm.language == "en").map(|sm| sm.payload.clone()).unwrap_or(Value::Null);
    c.check(
        "events at once: one embed, a field each",
        group["embeds"].as_array().is_some_and(|e| e.len() == 1) && group["embeds"][0]["fields"].as_array().is_some_and(|f| f.len() == 3),
        (),
    );
    c.check(
        "events at once: each account's facts in its field",
        group["embeds"][0]["fields"].as_array().is_some_and(|f| f.len() >= 2 && f.iter().take(2).all(|x| s(&x["value"]).contains("`277`"))),
        to_text(&group),
    );
    let drop = samples
        .iter()
        .find(|sm| sm.label == "Alert · disconnected, rejoining" && sm.language == "en")
        .map(|sm| sm.payload["embeds"][0].clone())
        .unwrap_or(Value::Null);
    c.check(
        "an account's alert: its name and avatar above, the game's icon beside, game, time played and reason",
        s(&drop["author"]["name"]) == "NoobFarm2"
            && !drop["author"]["icon_url"].is_null()
            && s(&drop["thumbnail"]["url"]).contains("GameIcon")
            && drop["fields"].as_array().is_some_and(|f| f.len() == 3)
            && s(&drop["fields"][2]["value"]).contains("277"),
        to_text(&drop),
    );
    let summary = samples.iter().find(|sm| sm.label.starts_with("Summary · daily (") && sm.language == "en");
    c.check(
        "a summary shows its picture",
        summary.is_some_and(|sm| {
            s(&sm.payload["embeds"][0]["image"]["url"]) == "attachment://bloxkeep-summary.png" && sm.payload["attachments"].is_array() && sm.files.is_some()
        }),
        summary.map(|sm| to_text(&sm.payload)),
    );
}

/// The words of a message, without the names and numbers that are the same in both languages.
fn words(payload: &Value) -> String {
    let e = &payload["embeds"][0];
    if e.is_null() {
        s(&payload["content"]).to_string()
    } else {
        format!("{}|{}|{}", s(&e["title"]), s(&e["description"]), s(&e["footer"]["text"]))
    }
}

