#![recursion_limit = "512"]
// Bloxkeep's window, drawn with GPUI (1.1). The C# app (RobloxManager) starts it with --pipe NAME and talks to it over
// two named pipes, the same JSON messages the WebView2 page has (link.rs); the window then says its handle and the app
// lays it inside its own window, which keeps the tray, the hotkeys and the title bar.
//
//   --pipe NAME     the app's pipes (without: an empty window, for looking at the frame)
//   --shot PNG      draw the window hidden, save a picture of it and quit (built with --features shot)
//   --size WxH      the window's size, 1280x800 by default
//   --wait MS       for --shot: how long the app's state and pictures get first, 2500 by default
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod assets;
mod calendar;
// The C# app moved to Rust (src/core/), compiled only with `--features core` until it is joined to the window.
#[cfg(feature = "core")]
mod core;
mod crisp;
mod frame;
mod history;
mod i18n;
mod images;
mod kit;
mod link;
mod mesh;
mod itemwatch;
mod modstore;
mod net;
mod prefs;
mod pages;
mod roblox;
mod roblox_codes;
mod scene;
mod near;
mod scrolled;
mod shell;
mod shot;
mod target;
mod theme;
mod theme_table;
mod time;
mod video;
mod wikihtml;
mod wikitext;

use std::{borrow::Cow, path::PathBuf, time::Duration};

use gpui_kit::*;

fn main() {
    let args: Vec<String> = std::env::args().collect();
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
    let (w, h) = arg("--size")
        .and_then(|s| s.split_once('x').and_then(|(a, b)| Some((a.parse::<f32>().ok()?, b.parse::<f32>().ok()?))))
        .unwrap_or((1280., 800.));
    let wait = Duration::from_millis(arg("--wait").and_then(|s| s.parse().ok()).unwrap_or(2500));
    let base = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())).unwrap_or_default();
    let assets = std::env::var("BLOXKEEP_UI_ASSETS").map(PathBuf::from).unwrap_or_else(|_| base.join("assets"));

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
        let mut words = i18n::I18n::load(&std::fs::read_to_string(assets.join("i18n.json")).unwrap_or_default());
        words.add(&std::fs::read_to_string(assets.join("words.json")).unwrap_or_default());

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(120.), px(60.)), size: size(px(w), px(h)) })),
            // Windows' own title bar hidden (appears_transparent): the app's top bar is the title bar, with its own
            // buttons and moving; GPUI keeps the edges for resizing. (A native caption over it gave two title bars,
            // owner's screenshot 2026-11-12.)
            titlebar: if frame { Some(TitlebarOptions { title: Some("Bloxkeep".into()), appears_transparent: true, ..Default::default() }) } else { None },
            focus: shown,
            show: shown,
            window_min_size: Some(size(px(860.), px(600.))),
            ..Default::default()
        };
        let shot2 = shot.clone();
        gpui_kit::open_window(options, cx, move |window, cx| {
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
    });
}

/// BLOXKEEP_UI_PAGE=accounts…: which place a shot starts on.
fn shell_page_from_env(default: pages::Page) -> pages::Page {
    std::env::var("BLOXKEEP_UI_PAGE").ok().and_then(|k| pages::Page::from_key(&k)).unwrap_or(default)
}

fn hwnd_of(window: &Window) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match HasWindowHandle::window_handle(window).ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
        _ => None,
    }
}
