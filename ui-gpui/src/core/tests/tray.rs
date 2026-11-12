// Tests/Bloxkeep.Tests/TraySuite.cs: Roblox's notification-area copy told by its command line (the C# started ping.exe as
// stand-ins: skipped here, it starts programs), and the tray icon's quick panel (tray_window.rs): what it hands the page,
// what each command the page sends does, and where the panel opens. The panel's window is never made: its page's
// messages are given to it directly, and a hidden panel has nothing to hide.
use std::{cell::RefCell, rc::Rc};

use serde_json::{json, Value};

use super::{unlock, window::quiet_app as app, Checks, PASSWORD};
use crate::core::{
    app::controller::PendingLaunch,
    base::{Guid, Stamp},
    l,
    models::{Account, LaunchPreset},
    multi_instance, roblox_processes, themes,
    tray_window::{place_by, TrayWindow},
    vault::Vault,
    win_kit::{Point, Rect, Size},
};

const MB: i64 = 1024 * 1024;

pub async fn run(c: &mut Checks, _dir: &str) {
    c.skip("a process started with --launch-to-tray is Roblox's notification-area copy", "starts a program");
    c.skip("the same program without it isn't", "starts a program");
    c.skip("asked again, the same answer", "starts a program");
    c.check("a process that isn't there isn't one", !roblox_processes::is_tray_copy(i32::MAX - 7, 1), ());

    // The tray icon's quick panel and menu (TrayWindow, its page ui/src/Tray.tsx): what it hands the page, and what each
    // command the page sends does. PORT: the C# handed the window a made-up TrayInfo; here the app's own state is set up to
    // say the same (a preset, two accounts, one of them running, two Roblox windows using 2800 MB).
    let seed = Vault::create(PASSWORD, None).expect("vault");
    let (farm, alt, main) = {
        let mut d = seed.data();
        let alt = Account { username: "Alt".into(), user_id: 11, last_used: Some(Stamp::utc_now()), ..Default::default() };
        let main = Account { username: "Main".into(), user_id: 12, last_used: Some(Stamp::utc_now().plus_secs(-3600.0)), ..Default::default() };
        let (alt_id, main_id) = (alt.id, main.id);
        d.accounts.push(alt);
        d.accounts.push(main);
        let preset = LaunchPreset { name: "Farm".into(), account_ids: vec![alt_id], ..Default::default() };
        let farm = preset.id;
        d.settings.presets.push(preset);
        d.settings.multi_instance = true;
        (farm, alt_id, main_id)
    };
    seed.save().expect("saved");
    drop(seed);
    let (app, _said) = app();
    unlock(&app).await;
    let launch = PendingLaunch::new(main, Default::default(), Stamp::utc_now(), None, false, None);
    app.controller.running.borrow_mut().insert(main, (4321, Rc::new(launch)));
    app.mini.client_ram.borrow_mut().insert(4321, 1500 * MB);
    app.mini.client_ram.borrow_mut().insert(4322, 1300 * MB);
    app.controller.avatar_urls.borrow_mut().insert(12, "https://tr.rbxcdn.com/main.png".into());

    multi_instance::enable();
    let armed = multi_instance::armed();
    let sent = TrayWindow::message(&app, "panel", 1, true);
    multi_instance::disable();
    let data = &sent["data"];
    let st = &data["state"];
    if armed {
        c.check(
            "the tray's page gets the state: Multi-Roblox, the windows and their RAM, the mini window, presets, accounts and running ones",
            sent["name"] == "tray"
                && st["multi"] == "ready"
                && st["robloxCount"].as_i64() == Some(2)
                && st["ramMb"].as_i64() == Some(2800)
                && st["miniOpen"] == true
                && st["presets"][0]["name"] == "Farm"
                && st["accounts"].as_array().map(Vec::len) == Some(2)
                && st["clients"][0]["pid"].as_i64() == Some(4321),
            &sent,
        );
    } else {
        // Multi-Roblox's lock can't be taken while a Roblox client is open on this PC.
        c.skip(
            "the tray's page gets the state: Multi-Roblox, the windows and their RAM, the mini window, presets, accounts and running ones",
            "needs Roblox closed (Multi-Roblox's lock is held)",
        );
    }
    c.check(
        "…each with its picture's address once known, none otherwise (its initials then)",
        st["clients"][0]["avatar"] == "https://tr.rbxcdn.com/main.png" && st["accounts"][0]["avatar"].is_null(),
        st,
    );
    c.check(
        "…with the theme and the language",
        data["theme"].as_str() == Some(&*themes::current().id) && data["language"].as_str() == Some(&*l::language()),
        data,
    );
    app.controller.running.borrow_mut().clear();
    app.mini.client_ram.borrow_mut().clear();

    let window = TrayWindow::new();
    #[derive(Default)]
    struct Seen {
        ran: Option<Guid>,
        opened: Option<Guid>,
        closed_one: Option<Guid>,
        shown: Option<i32>,
        closed_all: bool,
        fresh: bool,
        quit: bool,
        mini: bool,
        open: bool,
        switched: Option<bool>,
    }
    let seen = Rc::new(RefCell::new(Seen::default()));
    {
        let mut e = window.events.borrow_mut();
        let s = seen.clone();
        e.account_clicked = Some(Box::new(move |id| s.borrow_mut().opened = Some(id)));
        let s = seen.clone();
        e.preset_clicked = Some(Box::new(move |id| s.borrow_mut().ran = Some(id)));
        let s = seen.clone();
        e.new_roblox_clicked = Some(Box::new(move || s.borrow_mut().fresh = true));
        let s = seen.clone();
        e.close_all_clicked = Some(Box::new(move || s.borrow_mut().closed_all = true));
        let s = seen.clone();
        e.close_client_clicked = Some(Box::new(move |id| s.borrow_mut().closed_one = Some(id)));
        let s = seen.clone();
        e.show_client_clicked = Some(Box::new(move |pid| s.borrow_mut().shown = Some(pid)));
        let s = seen.clone();
        e.quit_clicked = Some(Box::new(move || s.borrow_mut().quit = true));
        let s = seen.clone();
        e.mini_clicked = Some(Box::new(move || s.borrow_mut().mini = true));
        let s = seen.clone();
        e.open_clicked = Some(Box::new(move || s.borrow_mut().open = true));
        let s = seen.clone();
        e.multi_clicked = Some(Box::new(move |on| s.borrow_mut().switched = Some(on)));
    }
    let act = |a: Value| window.act(&a);
    act(json!({ "what": "account", "id": alt.to_string() }));
    act(json!({ "what": "preset", "id": farm.to_string() }));
    act(json!({ "what": "newRoblox" }));
    act(json!({ "what": "closeAll" }));
    act(json!({ "what": "closeClient", "id": main.to_string() }));
    act(json!({ "what": "show", "pid": 4321 }));
    act(json!({ "what": "multi", "on": true }));
    act(json!({ "what": "mini" }));
    act(json!({ "what": "open" }));
    act(json!({ "what": "quit" }));
    {
        let s = seen.borrow();
        c.check(
            "its commands: an account, a preset, a new Roblox, close all, close or show one window, Multi-Roblox, the mini window, Bloxkeep, quit",
            s.opened == Some(alt)
                && s.ran == Some(farm)
                && s.fresh
                && s.closed_all
                && s.closed_one == Some(main)
                && s.shown == Some(4321)
                && s.switched == Some(true)
                && s.mini
                && s.open
                && s.quit,
            (),
        );
    }
    seen.borrow_mut().opened = None;
    act(json!({ "what": "account", "id": "not an id" }));
    act(json!({ "what": "nothing" }));
    window.on_message("not json");
    c.check("…anything else it sends does nothing", seen.borrow().opened.is_none(), ());
    window.dispose();

    // Where the panel opens, next to the taskbar Windows says (review 2026-10-30): a 1920 × 1080 screen, a 48 px taskbar,
    // a 300 × 400 panel, 12 px from the edges, the pointer at the clock.
    let place = |work: Rect, bar: Option<(Rect, i32)>, at: Option<Point>| {
        place_by(at.unwrap_or(Point::new(1800, 1060)), Size::new(300, 400), work, Rect::new(0, 0, 1920, 1080), bar, 12)
    };
    let screen = Rect::new(0, 0, 1920, 1080);
    let bottom = Rect::new(0, 1032, 1920, 48);
    let r = place(Rect::new(0, 0, 1920, 1032), Some((bottom, 3)), None);
    c.check("above a bottom taskbar", r == Rect::new(1608, 620, 300, 400), r);
    let r = place(screen, Some((bottom, 3)), None);
    c.check("…also auto-hidden (the work area is the whole screen): above it, not over it", r == Rect::new(1608, 620, 300, 400), r);
    let r = place(Rect::new(200, 0, 1920, 1032), Some((bottom, 3)), Some(Point::new(300, 1060)));
    c.check("…with another app's bar on the left: still above it, not by the side", r == Rect::new(212, 620, 300, 400), r);
    let r = place(Rect::new(48, 0, 1872, 1080), Some((Rect::new(0, 0, 48, 1080), 0)), Some(Point::new(20, 900)));
    c.check("a taskbar on the left: beside it, by the pointer", r == Rect::new(60, 512, 300, 400), r);
    let a = place(Rect::new(0, 48, 1920, 1032), Some((Rect::new(1920, 1032, 1920, 48), 3)), Some(Point::new(1800, 20)));
    let b = place(Rect::new(0, 0, 1920, 1032), None, None);
    c.check(
        "Windows not saying (a taskbar on another screen): guessed from the work area as before",
        a == Rect::new(1608, 60, 300, 400) && b == Rect::new(1608, 620, 300, 400),
        (a, b),
    );
}
