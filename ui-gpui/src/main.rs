#![recursion_limit = "512"]
// Bloxkeep (1.1): its window drawn with GPUI, and with --features core the app itself (core/, moved from the C# app it
// was before 1.1) in the same process. Without the core, a host starts it with --pipe NAME and talks to it over two
// named pipes, the same JSON messages (link.rs): ui-gpui/dev/mockhost.mjs, the made-up app behind the screenshots.
//
//   --pipe NAME     a host's pipes (without, and without the core: an empty window, for looking at the frame)
//   --shot PNG      draw the window hidden, save a picture of it and quit (built with --features shot)
//   --size WxH      the window's size, 1280x800 by default
//   --wait MS       for --shot: how long the app's state and pictures get first, 2500 by default
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// A build without the core (the shot harness, a UI-only check) leaves the parts only the core calls unused by design:
// their warnings would hide the real ones, so only the core build (what ships) warns about unused code.
#![cfg_attr(not(feature = "core"), allow(dead_code, unused_variables, unused_imports))]

mod ask;
mod assets;
mod calendar;
// The C# app moved to Rust (src/core/), compiled only with `--features core` until it is joined to the window.
#[cfg(feature = "core")]
mod core;
mod countries;
mod crisp;
mod duo_icons;
mod frame;
mod frame_tally;
mod heap;
mod mem_regions;
mod mem_tally;
mod history;
#[cfg(all(test, feature = "shot"))]
mod hover_test;
mod i18n;
mod lang;
mod images;
mod kit;
mod link;
mod media;
mod wallpaper_engine;
mod window_capture;
mod mesh;
mod rbxm;
mod motion;
mod itemwatch;
mod modstore;
mod launcher_icons;
mod net;
mod prefs;
mod pages;
mod roblox;
mod roblox_codes;
mod scene;
mod live;
mod near;
mod scrolled;
mod shell;
mod shot;
mod target;
mod theme;
mod theme_table;
mod threads;
mod time;
mod toolpage;
mod tray;
mod val;
mod video;
mod wikihtml;
mod wikitext;

use std::{borrow::Cow, path::PathBuf, time::Duration};

use gpui_kit::*;

/// Rust's heap, counted for error.log's memory line (heap.rs).
#[global_allocator]
static ALLOC: heap::Counted = heap::Counted;

fn main() {
    // (Where the heap goes, in error.log: always in the app, in a shot when asked.)
    #[cfg(feature = "core")]
    heap::sites::start();
    #[cfg(all(feature = "shot", not(feature = "core")))]
    if std::env::var_os("BLOXKEEP_SHOT_SITES").is_some() {
        heap::sites::start();
    }
    let args: Vec<String> = std::env::args().collect();
    #[cfg(feature = "core")]
    if args.get(1).is_some_and(|arg| arg == "--core-monitor-frames") {
        if let Err(error) = core::monitor_frames::run() {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    let arg = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let pipe = arg("--pipe");
    // A build with the `core` feature is the app itself (src/core, the C# app moved to Rust): no C# app, no pipes,
    // also started bare (the installed shortcut, Velopack's install and update steps); `--core` is still taken. Only
    // `--pipe` (the dev mock host) makes it the page alone. It then says where the cache and the pictures are
    // (core::gpui_host::start).
    let in_process = cfg!(feature = "core") && pipe.is_none();
    if !in_process {
        // --cache DIR: the app's cache folder, where what Roblox said may be kept between runs (roblox::keep_in).
        roblox::keep_in(arg("--cache").map(PathBuf::from));
        // --pictures DIR: where the app keeps the pictures the user chose (a vault's face): the page's "user/…" files.
        images::keep_user(arg("--pictures").map(PathBuf::from));
    }
    let shot = arg("--shot").map(PathBuf::from);
    // BLOXKEEP_SHOT_WE=<file>: what Follow Wallpaper Engine reads from this PC's Wallpaper Engine (its config, the
    // wallpaper on show, its picture and colour), written to that file: the reading checked on a real install with
    // nothing asked of Wallpaper Engine itself (no live copy played).
    #[cfg(feature = "shot")]
    if let (Some(_), Some(out)) = (&shot, std::env::var_os("BLOXKEEP_SHOT_WE")) {
        let config = wallpaper_engine::config();
        let got = wallpaper_engine::check(None, None);
        let _ = std::fs::write(out, format!("config: {config:?}\nchecked: {got:?}\n"));
    }
    let (w, h) = arg("--size")
        .and_then(|s| s.split_once('x').and_then(|(a, b)| Some((a.parse::<f32>().ok()?, b.parse::<f32>().ok()?))))
        .unwrap_or((1280., 800.));
    let wait = Duration::from_millis(arg("--wait").and_then(|s| s.parse().ok()).unwrap_or(2500));
    let base = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())).unwrap_or_default();
    let assets = std::env::var("BLOXKEEP_UI_ASSETS").map(PathBuf::from).unwrap_or_else(|_| base.join("assets"));
    // The core's sentences in the languages after Vietnamese (assets/lang/<code>.json).
    #[cfg(feature = "core")]
    core::l::load_more(&assets);

    // --tray-window-check OUT: the tray's real window, cloaked, checked on this PC (tray.rs › window_check).
    #[cfg(feature = "core")]
    if let Some(out) = arg("--tray-window-check") {
        tray::window_check(PathBuf::from(out), assets);
        return;
    }
    // --codes-check NAME UNIVERSE: a game's codes as its page gets them (core/game_codes.rs), printed.
    #[cfg(feature = "core")]
    if let Some(name) = arg("--codes-check") {
        let universe = arg("--universe").and_then(|s| s.parse().ok()).unwrap_or(0);
        match core::game_codes::codes(&name, universe, 0, &core::base::Cancel::new()) {
            Ok(v) => println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default()),
            Err(e) => println!("codes-check failed: {}", e.message()),
        }
        return;
    }
    // --clip-sound-check FILE [--seconds N]: that clip's sound decoded and fed as silence, to prove it plays (video.rs).
    #[cfg(feature = "core")]
    if let Some(file) = arg("--clip-sound-check") {
        video::clip_sound_check(std::path::Path::new(&file), arg("--seconds").and_then(|s| s.parse().ok()).unwrap_or(10));
        return;
    }
    // --clip-window-check FILE [--seconds N]: that clip shown in a cloaked real window, to measure (video.rs).
    #[cfg(feature = "core")]
    if let Some(file) = arg("--clip-window-check") {
        video::clip_window_check(file, arg("--seconds").and_then(|s| s.parse().ok()).unwrap_or(30));
        return;
    }
    // --clip-check FILE [--seconds N]: a game clip decoded as a tile plays it, to measure (video.rs).
    #[cfg(any(feature = "core", feature = "shot"))]
    if let Some(file) = arg("--clip-check") {
        video::clip_check(std::path::Path::new(&file), arg("--seconds").and_then(|s| s.parse().ok()).unwrap_or(30));
        return;
    }
    // --tool-window-check DIR: the small windows' faces in real windows, cloaked, checked on this PC (toolpage/check.rs).
    #[cfg(feature = "core")]
    if let Some(out) = arg("--tool-window-check") {
        toolpage::window_check(PathBuf::from(out), assets);
        return;
    }
    // --core-vault-test PATH --password PW: the port's own check of the vault file, on made-up data (core/selftest.rs).
    #[cfg(feature = "core")]
    if let (Some(path), Some(password)) = (arg("--core-vault-test"), arg("--password")) {
        std::process::exit(core::selftest::vault(&path, &password));
    }
    // --core-log-replay FILE: a Roblox log read as the watcher reads it, one line a change of state.
    #[cfg(feature = "core")]
    if let Some(file) = arg("--core-log-replay") {
        std::process::exit(core::selftest::log_replay(&file));
    }
    // --core-log-words FILE: the lines of a Roblox log that would go to error.log when its window ends.
    #[cfg(feature = "core")]
    if let Some(file) = arg("--core-log-words") {
        std::process::exit(core::selftest::log_words(&file));
    }
    // --core-update-try MARK: the updater's own steps for real (check, download, install on exit, start again), on an
    // installed copy made for the test; the copy started again writes its version to MARK (--core-version-mark).
    #[cfg(feature = "core")]
    if let Some(mark) = arg("--core-update-try") {
        std::process::exit(core::selftest::update_try(&mark));
    }
    #[cfg(feature = "core")]
    if let Some(mark) = arg("--core-version-mark") {
        std::process::exit(core::selftest::version_mark(&mark));
    }
    // --core-suite NAME DIR: one of the C# tests moved to Rust (core/tests), on the data folder DIR; --core-suites [NAME..]
    // [--online] [--timeout=MIN]: each in a child process, as Tests/Bloxkeep.Tests ran them.
    #[cfg(feature = "core")]
    {
        let all: Vec<String> = std::env::args().collect();
        if let Some(at) = all.iter().position(|a| a == "--core-suite") {
            let name = all.get(at + 1).cloned().unwrap_or_default();
            let dir = all.get(at + 2).cloned().unwrap_or_default();
            std::process::exit(core::tests::run_one(&name, &dir));
        }
        if let Some(at) = all.iter().position(|a| a == "--core-suites") {
            let rest = &all[at + 1..];
            let wanted: Vec<String> = rest.iter().filter(|a| !a.starts_with("--")).cloned().collect();
            let minutes = rest.iter().find_map(|a| a.strip_prefix("--timeout=")).and_then(|m| m.parse::<u64>().ok()).unwrap_or(10);
            let online = rest.iter().any(|a| a == "--online");
            std::process::exit(core::tests::run_all(&wanted, online, std::time::Duration::from_secs(minutes * 60)));
        }
    }
    // --core-codes OUT: what the core says and does for every Roblox connection error code.
    #[cfg(feature = "core")]
    if let Some(out) = arg("--core-codes") {
        std::process::exit(core::selftest::codes(&out));
    }
    // --core-words OUT, --core-expr FILE: the table of sentences and the macro formulas, for a side-by-side check with the C#.
    #[cfg(feature = "core")]
    if let Some(out) = arg("--core-words") {
        std::process::exit(core::selftest::words(&out));
    }
    #[cfg(feature = "core")]
    if let Some(file) = arg("--core-expr") {
        std::process::exit(core::selftest::expr(&file));
    }
    // --core-checks TESTS_DIR: parts of the port held to known answers (core/selftest.rs).
    #[cfg(feature = "core")]
    if let Some(dir) = arg("--core-checks") {
        std::process::exit(core::selftest::checks(&dir));
    }
    // --core-cmd-test --password PW: the controller with no window, on the scratch vault in BLOXKEEP_DATA.
    #[cfg(feature = "core")]
    if let (true, Some(password)) = (args.iter().any(|a| a == "--core-cmd-test"), arg("--password")) {
        std::process::exit(core::selftest::commands(&password, arg("--script").as_deref()));
    }
    #[cfg(feature = "core")]
    if in_process && shot.is_none() {
        // The window is the core's to show: its shell places it, then shows it (core::shell_form), and it has a title
        // bar of its own now that no C# window is around it.
        core::gpui_host::set_runner(move || run_ui(Some(link::local()), true, false, None, w, h, wait, assets));
        let rest: Vec<String> = args.iter().skip(1).filter(|a| a.as_str() != "--core").cloned().collect();
        core::program::main_core(&rest);
        return;
    }
    // The line first: when the app isn't there, there's nothing to show.
    let link = match pipe.as_deref().map(link::connect) {
        Some(Ok(l)) => Some(l),
        Some(Err(e)) => {
            eprintln!("can't reach the app: {e}");
            std::process::exit(2);
        }
        None => None,
    };
    // Inside the C# app's window it has no frame of its own, and the app shows it once it's in place.
    let embedded = link.is_some() && shot.is_none();
    run_ui(link, !embedded, shot.is_none() && !embedded, shot, w, h, wait, assets);
}

/// The window, to its end. `frame`: it has a title bar of its own; `shown`: it shows and takes the keyboard by itself.
#[allow(clippy::too_many_arguments)]
fn run_ui(link: Option<(link::Link, futures::channel::mpsc::UnboundedReceiver<link::Event>)>, frame: bool, shown: bool, shot: Option<PathBuf>, w: f32, h: f32, wait: Duration, assets: PathBuf) {
    gpui_kit::application().with_assets(assets::FsAssets(assets.clone())).run(move |cx| {
        gpui_kit::init(cx);
        // Motion is Bloxkeep's own, as the web page's was. GPUI Base reads Windows' "Animation effects" switch at init
        // and, with it off, GPUI draws every animation and spring already finished: the owner's PC has it off, so
        // nothing moved in the real app (found 2026-11-11). The app takes the flag over here; Base leaves it alone after.
        cx.set_reduce_motion(false);
        crate::theme::dress(cx);
        images::init(cx);
        frame_tally::start(cx);
        mem_tally::start(cx);
        // The keys: Ctrl+K to the search, Tab and Shift+Tab through the controls, Alt+arrows for Back and Forward.
        cx.bind_keys([
            KeyBinding::new("ctrl-k", shell::FocusSearch, None),
            KeyBinding::new("tab", shell::FocusNext, Some("Bloxkeep")),
            KeyBinding::new("shift-tab", shell::FocusPrev, Some("Bloxkeep")),
            KeyBinding::new("alt-left", shell::GoBack, Some("Bloxkeep")),
            KeyBinding::new("alt-right", shell::GoForward, Some("Bloxkeep")),
            // Accounts' keys, as the old page had them (each passes on when it doesn't apply).
            KeyBinding::new("ctrl-f", shell::FindHere, Some("Bloxkeep")),
            KeyBinding::new("ctrl-l", shell::FocusPlace, Some("Bloxkeep")),
            KeyBinding::new("/", shell::FocusPlace, Some("Bloxkeep && !Input")),
            KeyBinding::new("ctrl-a", shell::PickAll, Some("Bloxkeep")),
            KeyBinding::new("delete", shell::DeletePicked, Some("Bloxkeep")),
            KeyBinding::new("f2", shell::EditPicked, Some("Bloxkeep")),
            KeyBinding::new("enter", shell::PlayPicked, Some("Bloxkeep")),
            KeyBinding::new("escape", shell::ClearPicked, Some("Bloxkeep")),
            KeyBinding::new("left", shell::CardLeft, Some("Bloxkeep && !Input")),
            KeyBinding::new("right", shell::CardRight, Some("Bloxkeep && !Input")),
            KeyBinding::new("up", shell::CardUp, Some("Bloxkeep && !Input")),
            KeyBinding::new("down", shell::CardDown, Some("Bloxkeep && !Input")),
            KeyBinding::new("home", shell::CardFirst, Some("Bloxkeep && !Input")),
            KeyBinding::new("end", shell::CardLast, Some("Bloxkeep && !Input")),
            KeyBinding::new("space", shell::CardPick, Some("Bloxkeep && !Input")),
        ]);
        pages::palette::bind(cx);
        let fonts = ["Regular", "Medium", "SemiBold", "Bold"]
            .iter()
            .filter_map(|w| std::fs::read(assets.join(format!("fonts/BeVietnamPro-{w}.ttf"))).ok())
            .map(Cow::Owned)
            .collect::<Vec<_>>();
        cx.text_system().add_fonts(fonts).ok();
        let words = i18n::I18n::from_assets(&assets);

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(120.), px(60.)), size: size(px(w), px(h)) })),
            // Windows' own title bar hidden (appears_transparent): the app's top bar is the title bar, with its own
            // buttons and moving; GPUI keeps the edges for resizing. (A native caption over it gave two title bars,
            // owner's screenshot 2026-11-12.)
            titlebar: if frame { Some(TitlebarOptions { title: Some("Bloxkeep".into()), appears_transparent: true, ..Default::default() }) } else { None },
            focus: shown,
            show: shown,
            // (500 high: a 1080p screen at 200 % leaves 516 above the taskbar; 600 put the window's foot under it.)
            window_min_size: Some(size(px(860.), px(500.))),
            ..Default::default()
        };
        let shot2 = shot.clone();
        let main = gpui_kit::open_window(options, cx, move |window, cx| {
            let hwnd = hwnd_of(window);
            if let (Some((l, _)), Some(h)) = (&link, hwnd) {
                l.hello(h);
            }
            let root = cx.new(|cx| {
                let mut r = shell::Root::new(words, link, window, cx);
                if let Some(out) = shot2 {
                    let me = cx.entity().downgrade();
                    shot::schedule(me, out, w, h, wait, cx).detach();
                }
                r.page = shell_page_from_env(r.page);
                r.hwnd = hwnd;
                r
            });
            root
        })
        .expect("open the window");
        // The main window closed: Bloxkeep ends, whatever else is still made (the tray's menu and panel, the mini
        // window, an overlay: GPUI windows since 2026-11-19, put away hidden). GPUI on Windows ends only when its last
        // window closes, so with one of those hidden the process stayed on after Quit, still holding the
        // single-instance mutex: every start after just asked it to come forward, and nothing came (the owner,
        // 2026-11-19 17:10 and 18:46: "bloxkeep mở không lên nữa").
        let main_id = main.0.window_id();
        cx.on_window_closed(move |cx, id| {
            if id == main_id {
                cx.quit();
            }
        })
        .detach();
    });
}

/// BLOXKEEP_UI_PAGE=accounts…: which place a shot starts on.
fn shell_page_from_env(default: pages::Page) -> pages::Page {
    std::env::var("BLOXKEEP_UI_PAGE").ok().and_then(|k| pages::Page::from_key(&k)).unwrap_or(default)
}

pub fn hwnd_of(window: &Window) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match HasWindowHandle::window_handle(window).ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
        _ => None,
    }
}
