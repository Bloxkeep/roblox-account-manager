// Where a Roblox window stands and whether it is open are two things (Settings' "When a Roblox window opens" and
// "When a window joins a game"), and every pair of the two holds together, at a launch and at a rejoin: a window
// minimised as it joins still has its tile or its maximised state as what it opens to. Checked on real windows of the
// test's own, on a desktop of its own that is never shown (Windows moves, minimises and maximises windows there as on
// the one in view): the suite starts itself again on that desktop and tells this run what it found. Arrange is checked
// the same way, carried out by the window desk on those windows: who took which tile, what stayed down, what waited.
use std::{
    collections::{HashMap, HashSet},
    ffi::c_void,
    ptr::{null, null_mut},
    rc::Rc,
    time::{Duration, Instant},
};

use windows_sys::Win32::{
    Foundation::{CloseHandle, HWND, RECT},
    System::Threading::{CreateProcessW, GetCurrentThreadId, GetExitCodeProcess, Sleep, WaitForSingleObject, PROCESS_INFORMATION, STARTUPINFOW},
    UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetWindowLongPtrW, GetWindowRect, IsIconic, IsZoomed, PostMessageW, PostThreadMessageW, RegisterClassW,
        SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow, ShowWindowAsync, GWLP_USERDATA, MSG, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER, SW_MINIMIZE, SW_RESTORE,
        SW_SHOWMAXIMIZED, SW_SHOWMINNOACTIVE, SW_SHOWNOACTIVATE, WM_QUIT, WM_USER, WM_WINDOWPOSCHANGED, WNDCLASSW, WS_OVERLAPPEDWINDOW, WS_POPUP, WS_VISIBLE,
    },
};

use super::{
    Checks,
    rejoin::{no_network, quiet_app, seed_with},
    unlock,
};
use crate::core::{
    app::{App, controller::PendingLaunch},
    base::{Guid, Stamp, new_guid},
    game_target::GameTarget,
    models::{Account, LaunchPreset},
    roblox_log_watcher::{ClientState, Tail},
    roblox_processes as rp, rt,
    win_kit::{self, Rect, Size},
    window_desk::{Arranged, Arrangement, Lift, Took, choose_tiles},
    window_loan::{self, How},
    window_plan::Step,
};

#[link(name = "user32")]
unsafe extern "system" {
    fn CreateDesktopW(name: *const u16, device: *const u16, devmode: *const c_void, flags: u32, access: u32, security: *const c_void) -> isize;
    fn CloseDesktop(desktop: isize) -> i32;
    fn GetThreadDesktop(thread: u32) -> isize;
    fn GetUserObjectInformationW(object: isize, index: i32, info: *mut c_void, length: u32, needed: *mut u32) -> i32;
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// The name of the desktop this thread is on ("Default": the one in view).
fn desktop_name() -> String {
    const UOI_NAME: i32 = 2;
    let mut name = [0u16; 128];
    let mut needed = 0u32;
    // SAFETY: a buffer of the length given; plain queries.
    let ok = unsafe { GetUserObjectInformationW(GetThreadDesktop(GetCurrentThreadId()), UOI_NAME, name.as_mut_ptr().cast(), (name.len() * 2) as u32, &mut needed) };
    if ok == 0 {
        return String::new();
    }
    String::from_utf16_lossy(&name).trim_end_matches('\0').to_string()
}

pub async fn run(c: &mut Checks, dir: &str) {
    if desktop_name() == "Default" {
        on_a_desktop_of_its_own(c, dir);
        return;
    }
    let mut r = Rec { c, lines: Vec::new() };
    checks(&mut r).await;
    let _ = std::fs::write(std::path::Path::new(dir).join("results.txt"), r.lines.join("\n"));
}

/// Checks said here and kept for the run that started this one.
struct Rec<'a> {
    c: &'a mut Checks,
    lines: Vec<String>,
}

impl Rec<'_> {
    fn check(&mut self, what: &str, ok: bool, detail: impl std::fmt::Debug) {
        self.lines.push(if ok { format!("P\t{what}") } else { format!("F\t{what}\t{detail:?}") });
        self.c.check(what, ok, detail);
    }
    fn skip(&mut self, what: &str, why: &str) {
        self.lines.push(format!("S\t{what}\t{why}"));
        self.c.skip(what, why);
    }
}

/// Starts this suite again on a desktop made for it (never switched to: nothing of it shows) and takes its checks.
fn on_a_desktop_of_its_own(c: &mut Checks, dir: &str) {
    const GENERIC_ALL: u32 = 0x1000_0000;
    const WAIT_MS: u32 = 180_000;
    let name = format!("bloxkeep-test-{}", std::process::id());
    let child_dir = format!("{dir}-desk");
    let results = std::path::Path::new(&child_dir).join("results.txt");
    let _ = std::fs::remove_file(&results);
    let ran = "the checks ran on a desktop of their own";
    // SAFETY: plain calls with strings that outlive them; the handles made are closed below.
    let code = unsafe {
        let desktop = CreateDesktopW(wide(&name).as_ptr(), null(), null(), 0, GENERIC_ALL, null());
        if desktop == 0 {
            c.skip(ran, "Windows made no desktop for them");
            return;
        }
        let exe = std::env::current_exe().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        let mut line = wide(&format!("\"{exe}\" --core-suite window-rules \"{child_dir}\""));
        let mut on = wide(&format!("WinSta0\\{name}"));
        let mut si: STARTUPINFOW = std::mem::zeroed();
        si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
        si.lpDesktop = on.as_mut_ptr();
        let mut pi: PROCESS_INFORMATION = std::mem::zeroed();
        let started = CreateProcessW(null(), line.as_mut_ptr(), null(), null(), 0, 0, null(), null(), &si, &mut pi);
        let mut code = u32::MAX;
        if started != 0 {
            WaitForSingleObject(pi.hProcess, WAIT_MS);
            GetExitCodeProcess(pi.hProcess, &mut code);
            CloseHandle(pi.hThread);
            CloseHandle(pi.hProcess);
        }
        CloseDesktop(desktop);
        code
    };
    let Ok(text) = std::fs::read_to_string(&results) else {
        c.check(ran, false, format!("no results; its exit code was {code}"));
        return;
    };
    c.check(ran, true, ());
    for line in text.lines() {
        let mut parts = line.splitn(3, '\t');
        match (parts.next(), parts.next(), parts.next()) {
            (Some("P"), Some(what), _) => c.check(what, true, ()),
            (Some("F"), Some(what), detail) => c.check(what, false, detail.unwrap_or("")),
            (Some("S"), Some(what), why) => c.skip(what, why.unwrap_or("")),
            _ => {}
        }
    }
}

// ───────── windows of the test's own ─────────

/// Asks a stand-in's thread to be busy for this many ms (wParam), as a Roblox window busy loading takes an ask late.
const BUSY: u32 = WM_USER + 1;
/// A stand-in with a least size of its own grows back to it after a move, as Roblox does.
const GROW: u32 = WM_USER + 2;

unsafe extern "system" fn plain_proc(hw: HWND, m: u32, w: usize, l: isize) -> isize {
    // SAFETY: the window's own messages, handed on as they came; plain calls on the window itself.
    unsafe {
        match m {
            BUSY => {
                Sleep(w as u32);
                0
            }
            WM_WINDOWPOSCHANGED => {
                let done = DefWindowProcW(hw, m, w, l);
                if least_of(hw as isize).is_some() {
                    PostMessageW(hw, GROW, 0, 0);
                }
                done
            }
            GROW => {
                grow_back(hw as isize);
                0
            }
            _ => DefWindowProcW(hw, m, w, l),
        }
    }
}

/// The least size a stand-in keeps (`keep_least`), if any.
fn least_of(w: isize) -> Option<(i32, i32)> {
    // SAFETY: plain query.
    let v = unsafe { GetWindowLongPtrW(h(w), GWLP_USERDATA) };
    if v == 0 { None } else { Some((((v >> 16) & 0xFFFF) as i32, (v & 0xFFFF) as i32)) }
}

/// A stand-in that grows back to `least` after any move that makes it smaller (None: it takes any size).
fn keep_least(w: isize, least: Option<(i32, i32)>) {
    let v = least.map_or(0, |(a, b)| ((a as isize) << 16) | b as isize);
    // SAFETY: plain call; the value is read by the window's own procedure.
    unsafe { SetWindowLongPtrW(h(w), GWLP_USERDATA, v) };
}

fn grow_back(w: isize) {
    let Some((lw, lh)) = least_of(w) else { return };
    if iconic(w) || zoomed(w) {
        return;
    }
    let r = rect_of(w);
    if r.width < lw || r.height < lh {
        // SAFETY: plain call on the window's own thread.
        unsafe { SetWindowPos(h(w), null_mut(), 0, 0, r.width.max(lw), r.height.max(lh), SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE) };
    }
}

fn set_title(w: isize, title: &str) {
    // SAFETY: the text outlives the call.
    unsafe { SetWindowTextW(h(w), wide(title).as_ptr()) };
}

/// A thread of windows that answer as any app's do: `count` ordinary ones with a title bar, then two without one (as
/// Roblox's full screen has none). They are gone with the thread, at the end.
struct Windows {
    thread: u32,
    plain: Vec<isize>,
    bare: Vec<isize>,
}

fn make_windows(count: usize) -> Windows {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let class = wide("BloxkeepTestWindow");
        // SAFETY: a class of this process with a plain procedure; windows made and pumped on this thread only.
        unsafe {
            let mut wc: WNDCLASSW = std::mem::zeroed();
            wc.lpfnWndProc = Some(plain_proc);
            wc.lpszClassName = class.as_ptr();
            RegisterClassW(&wc);
            let make = |style: u32, i: i32| CreateWindowExW(0, class.as_ptr(), class.as_ptr(), style | WS_VISIBLE, 40 + i * 30, 40 + i * 30, 500, 400, null_mut(), null_mut(), null_mut(), null()) as isize;
            let plain: Vec<isize> = (0..count as i32).map(|i| make(WS_OVERLAPPEDWINDOW, i)).collect();
            let bare: Vec<isize> = (0..2).map(|i| make(WS_POPUP, i)).collect();
            let _ = tx.send((GetCurrentThreadId(), plain, bare));
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
                DispatchMessageW(&msg);
            }
        }
    });
    let (thread, plain, bare) = rx.recv().expect("the windows' thread");
    Windows { thread, plain, bare }
}

fn h(w: isize) -> windows_sys::Win32::Foundation::HWND {
    w as _
}

fn iconic(w: isize) -> bool {
    // SAFETY: plain query.
    unsafe { IsIconic(h(w)) != 0 }
}

fn zoomed(w: isize) -> bool {
    // SAFETY: plain query.
    unsafe { IsZoomed(h(w)) != 0 }
}

fn rect_of(w: isize) -> Rect {
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    // SAFETY: one RECT on the stack.
    unsafe { GetWindowRect(h(w), &mut r) };
    Rect::new(r.left, r.top, r.right - r.left, r.bottom - r.top)
}

fn show(w: isize, how: i32) {
    // SAFETY: plain call; the window's thread answers it.
    unsafe { ShowWindow(h(w), how) };
}

/// An ordinary open window at `r`.
fn put(w: isize, r: Rect) {
    show(w, SW_RESTORE);
    show(w, SW_SHOWNOACTIVATE);
    // SAFETY: plain call.
    unsafe { SetWindowPos(h(w), null_mut(), r.x, r.y, r.width, r.height, SWP_NOZORDER | SWP_NOACTIVATE) };
}

/// What Bloxkeep asks of a window comes about a moment later, on the window's own thread.
fn settle() {
    std::thread::sleep(std::time::Duration::from_millis(350));
}

fn near(a: Rect, b: Rect, slack: i32) -> bool {
    (a.x - b.x).abs() <= slack && (a.y - b.y).abs() <= slack && (a.right() - b.right()).abs() <= slack && (a.bottom() - b.bottom()).abs() <= slack
}

/// Until `done`, `ms` at most.
fn wait_for(ms: u64, done: impl Fn() -> bool) -> bool {
    let until = Instant::now() + Duration::from_millis(ms);
    loop {
        if done() {
            return true;
        }
        if Instant::now() >= until {
            return false;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// An Arrange of `slots` on `area` (as `choose_tiles` chose them), carried out by the window desk; not lifted over
/// other apps.
async fn arrange(app: &Rc<App>, slots: Vec<(i32, isize)>, open: Vec<isize>, area: Rect, layout: &str, clicked: bool) -> Arranged {
    let a = Arrangement { windows: slots.iter().copied().filter(|s| s.0 != 0).collect(), slots, area, layout: layout.into(), custom: Size::new(0, 0), open, clicked, lift: Lift::No };
    app.desk_arrange(move |_| a).await.ok().expect("the input turn")
}

// ───────── the checks ─────────

async fn checks(r: &mut Rec<'_>) {
    let screens = win_kit::all_screens();
    let Some(screen) = screens.iter().find(|s| s.primary).or(screens.first()).cloned() else {
        r.skip("windows keep their place while minimised", "no screen");
        return;
    };
    let area = screen.working_area;
    let at = |dx: i32, dy: i32, w: i32, hh: i32| Rect::new(area.x + dx, area.y + dy, w, hh);
    let ws = make_windows(64);
    let mut free = ws.plain.clone().into_iter();
    let mut next = || free.next().expect("a window left");

    // ── A window's place while it is minimised (roblox_processes) ──
    {
        let w = next();
        let (home, tile) = (at(60, 60, 640, 480), at(700, 300, 500, 420));
        put(w, home);
        show(w, SW_SHOWMINNOACTIVE);
        let was = rp::resting_place(w);
        r.check("a minimised window's place is where it stood", was == Some(home), (was, home));
        let set = rp::restore_to(w, tile);
        let now = rp::resting_place(w);
        r.check("given another place while minimised: it stays minimised", set && iconic(w), (set, iconic(w)));
        r.check("…and that is where it opens", now == Some(tile), (now, tile));
        r.check("…asked again: nothing to do", !rp::restore_to(w, tile), ());
        show(w, SW_RESTORE);
        r.check("…opened: it stands there, not maximised", rect_of(w) == tile && !zoomed(w), (rect_of(w), zoomed(w)));
        r.check("an open window is given no place this way", !rp::restore_to(w, home) && rect_of(w) == tile && rp::resting_place(w).is_none(), rect_of(w));

        let w = next();
        put(w, home);
        show(w, SW_SHOWMINNOACTIVE);
        let set = rp::restore_maximised(w);
        r.check("a minimised window told to open maximised stays minimised", set && iconic(w), (set, iconic(w)));
        r.check("…it has no place of an ordinary window then", rp::resting_place(w).is_none(), rp::resting_place(w));
        r.check("…asked again: nothing to do", !rp::restore_maximised(w), ());
        show(w, SW_RESTORE);
        r.check("…opened: maximised", zoomed(w), rect_of(w));
        r.check("an open window isn't told so", !rp::restore_maximised(w), ());

        let w = next();
        put(w, home);
        show(w, SW_SHOWMAXIMIZED);
        show(w, SW_MINIMIZE);
        r.check("maximised, then minimised by hand: no place of an ordinary window", rp::resting_place(w).is_none(), rp::resting_place(w));
        let set = rp::restore_to(w, tile);
        show(w, SW_RESTORE);
        r.check("…given a place: it opens there, no longer maximised", set && rect_of(w) == tile && !zoomed(w), (set, rect_of(w), zoomed(w)));

        let bare = ws.bare[0];
        put(bare, screen.bounds);
        show(bare, SW_SHOWMINNOACTIVE);
        r.check("a window without a title bar (Roblox's full screen), minimised: its place is no window's place", rp::resting_place(bare).is_none(), rp::resting_place(bare));

        // Another screen, when there is one: places are kept in each screen's own terms.
        match screens.iter().find(|s| s.device_name != screen.device_name) {
            Some(other) => {
                let w = next();
                let there = Rect::new(other.working_area.x + 80, other.working_area.y + 70, 520, 410);
                put(w, home);
                show(w, SW_SHOWMINNOACTIVE);
                let set = rp::restore_to(w, there);
                let kept = rp::resting_place(w);
                show(w, SW_RESTORE);
                r.check("a place on another screen: kept and opened there", set && kept == Some(there) && rect_of(w) == there, (kept, rect_of(w), there));
            }
            None => r.skip("a place on another screen: kept and opened there", "one screen"),
        }
    }

    // ── The app, whose window desk arranges and keeps the windows ──
    no_network();
    let ids: Vec<Guid> = (0..8).map(|_| new_guid()).collect();
    let (show_preset, minimise_preset, afk_preset) = (new_guid(), new_guid(), new_guid());
    {
        let ids = ids.clone();
        seed_with(
            ids.iter().enumerate().map(|(i, id)| Account { id: *id, username: format!("W{i}"), user_id: 500 + i as i64, cookie: format!("c{i}"), cookie_valid: Some(true), is_main: i == 7, ..Default::default() }).collect(),
            move |d| {
                d.settings.presets.push(LaunchPreset { id: show_preset, name: "Open".into(), on_join: "show".into(), ..Default::default() });
                d.settings.presets.push(LaunchPreset { id: minimise_preset, name: "Away".into(), on_join: "minimise".into(), ..Default::default() });
                d.settings.presets.push(LaunchPreset { id: afk_preset, name: "Light".into(), profile: "afk".into(), ..Default::default() });
            },
        );
    }
    let (app, _said) = quiet_app();
    unlock(&app).await;
    // One tick of the window desk, as on_proc_timer runs it (its entry points share the one pass).
    let tick = |app: &Rc<App>, wins: &HashMap<i32, Vec<isize>>, now: Stamp| {
        app.minimise_afk_windows(now, wins);
        app.keep_rejoin_places(now, wins);
        app.maximise_new_windows(now, wins);
        settle();
    };

    // ── Tiles: windows that stay minimised take theirs all the same ──
    let tiles_of = |n: usize, layout: &str| -> Vec<Rect> {
        let (cols, w, hh) = rp::grid_cells(n as i32, area.size(), layout, win_kit::Size::new(0, 0));
        (0..n as i32).map(|i| Rect::new(area.x + i % cols * w, area.y + i / cols * hh, w, hh)).collect()
    };
    const SLACK: i32 = 24; // a window's unseen borders, where Windows says them
    {
        let four: Vec<isize> = (0..4).map(|_| next()).collect();
        let pids: Vec<i32> = (0..4).map(|i| 930_001 + i).collect();
        let order: Vec<(i32, isize)> = pids.iter().copied().zip(four.iter().copied()).collect();
        for (i, &w) in four.iter().enumerate() {
            put(w, at(30 + 40 * i as i32, 30 + 40 * i as i32, 420, 320));
        }
        show(four[1], SW_SHOWMINNOACTIVE);
        show(four[3], SW_SHOWMINNOACTIVE);
        let (slots, open) = choose_tiles(&order, true, &[pids[1], pids[3]]);
        let laid = arrange(&app, slots, open, area, "2x2", false).await;
        let tiles = tiles_of(4, "2x2");
        r.check("4 windows, 2 of them minimised to stay so: all 4 have a tile", laid.done == 4, laid.done);
        r.check("…the open ones stand in the first and the third", near(rect_of(four[0]), tiles[0], SLACK) && near(rect_of(four[2]), tiles[2], SLACK), (rect_of(four[0]), rect_of(four[2]), tiles[0], tiles[2]));
        r.check("…the minimised ones are still minimised", iconic(four[1]) && iconic(four[3]), (iconic(four[1]), iconic(four[3])));
        let (a, b) = (rp::resting_place(four[1]), rp::resting_place(four[3]));
        r.check(
            "…and open at the second and the fourth",
            a.is_some_and(|p| near(p, tiles[1], SLACK)) && b.is_some_and(|p| near(p, tiles[3], SLACK)),
            (a, b, tiles[1], tiles[3]),
        );
        show(four[1], SW_RESTORE);
        r.check("…opened by hand: in its tile, beside the others", near(rect_of(four[1]), tiles[1], SLACK) && !zoomed(four[1]), (rect_of(four[1]), tiles[1]));

        // Every one of them minimised: nothing comes up, each has its tile.
        for &w in &four {
            put(w, at(50, 50, 400, 300));
            show(w, SW_SHOWMINNOACTIVE);
        }
        let (slots, open) = choose_tiles(&order, true, &pids);
        let laid = arrange(&app, slots, open, area, "2x2", false).await;
        let places: Vec<Option<Rect>> = four.iter().map(|&w| rp::resting_place(w)).collect();
        r.check("all 4 minimised to stay so: none comes up", laid.done == 4 && four.iter().all(|&w| iconic(w)), (laid.done, four.iter().map(|&w| iconic(w)).collect::<Vec<_>>()));
        r.check("…each opens in its own tile", places.iter().zip(&tiles).all(|(p, t)| p.is_some_and(|p| near(p, *t, SLACK))), (&places, &tiles));

        // One of those to stay minimised was maximised when put away: it opens in its tile, as the others.
        put(four[0], at(50, 50, 400, 300));
        show(four[0], SW_SHOWMAXIMIZED);
        show(four[0], SW_MINIMIZE);
        let (slots, open) = choose_tiles(&order, true, &pids);
        let _ = arrange(&app, slots, open, area, "2x2", false).await;
        show(four[0], SW_RESTORE);
        r.check("…one put away maximised: it opens in its tile too", near(rect_of(four[0]), tiles[0], SLACK) && !zoomed(four[0]), (rect_of(four[0]), zoomed(four[0])));

        // Named to stay minimised but still open (its minimise not come about yet): tiled in view, as any open one.
        for &w in &four {
            put(w, at(50, 50, 400, 300));
        }
        let (slots, open) = choose_tiles(&order, true, &[pids[2]]);
        let _ = arrange(&app, slots, open, area, "2x2", false).await;
        r.check("named to stay minimised but open still: tiled as the open ones", !iconic(four[2]) && near(rect_of(four[2]), tiles[2], SLACK), (iconic(four[2]), rect_of(four[2])));

        // Arrange clicked (none stays minimised): minimised and maximised windows come up into their tiles, as before.
        show(four[1], SW_SHOWMINNOACTIVE);
        show(four[2], SW_SHOWMAXIMIZED);
        let (slots, open) = choose_tiles(&order, false, &[]);
        r.check("Arrange clicked: the minimised window is the one it opens", open == vec![four[1]], &open);
        let _ = arrange(&app, slots, open, area, "2x2", true).await;
        r.check(
            "Arrange clicked: minimised and maximised windows come up into their tiles",
            four.iter().zip(&tiles).all(|(&w, t)| !iconic(w) && !zoomed(w) && near(rect_of(w), *t, SLACK)),
            four.iter().map(|&w| (iconic(w), zoomed(w), rect_of(w))).collect::<Vec<_>>(),
        );
        for &w in &four {
            show(w, SW_SHOWMINNOACTIVE);
        }
    }

    // ── Arrange by the window desk: who takes a tile, what stays down, what waits ──
    let mut stand_in = 940_000;
    let mut take = |n: usize| -> Vec<(i32, isize)> {
        (0..n)
            .map(|_| {
                stand_in += 1;
                (stand_in, next())
            })
            .collect()
    };
    let map_of = |list: &[(i32, isize)]| -> HashMap<i32, Vec<isize>> { list.iter().map(|&(p, w)| (p, vec![w])).collect() };
    let lay = |list: &[(i32, isize)]| {
        for (i, &(_, w)) in list.iter().enumerate() {
            put(w, at(40 + 30 * i as i32, 40 + 30 * i as i32, 420, 320));
        }
    };
    let tiles = tiles_of(4, "2x2");

    // After a launch: the open windows each take a tile, and a minimised one is neither tiled nor opened.
    {
        let five = take(5);
        lay(&five);
        show(five[4].1, SW_SHOWMINNOACTIVE);
        let (slots, open) = choose_tiles(&five, true, &[]);
        let laid = arrange(&app, slots.clone(), open.clone(), area, "2x2", false).await;
        let rects: Vec<Rect> = five.iter().map(|&(_, w)| rect_of(w)).collect();
        r.check(
            "after a launch: 4 open windows, each in its tile",
            laid.done == 4 && slots.len() == 4 && (0..4).all(|i| near(rects[i], tiles[i], SLACK)) && laid.took.iter().all(|t| t.3 == Took::Placed),
            (&rects, &laid.took),
        );
        r.check("…the minimised one gets no tile and isn't opened", open.is_empty() && iconic(five[4].1) && !slots.iter().any(|s| s.1 == five[4].1), (iconic(five[4].1), &slots));
        r.check(
            "…each remembers its tile, seen there",
            (0..4).all(|i| app.desk_memory(five[i].0).is_some_and(|m| m.tile.is_some_and(|t| near(t, tiles[i], SLACK)) && matches!(m.tile_given, Step::Done { .. })))
                && app.desk_memory(five[4].0).is_none_or(|m| m.tile.is_none()),
            five.iter().map(|&(p, _)| app.desk_memory(p).map(|m| (m.tile, m.tile_given))).collect::<Vec<_>>(),
        );
        for &(_, w) in &five {
            show(w, SW_SHOWMINNOACTIVE);
        }
    }

    // A minimise that lands while an automatic Arrange runs: the window is busy (its own thread), then put away by
    // someone else; the Arrange's moves reach it after the minimise.
    {
        let slow_set = make_windows(1);
        let slow = slow_set.plain[0];
        let mut four = take(3);
        four.insert(2, (949_999, slow));
        lay(&four);
        // SAFETY: plain calls; the window's thread answers them in turn.
        let posted = unsafe { PostMessageW(h(slow), BUSY, 700, 0) != 0 && ShowWindowAsync(h(slow), SW_SHOWMINNOACTIVE) != 0 };
        let (slots, open) = choose_tiles(&four, true, &[]);
        let laid = arrange(&app, slots, open, area, "2x2", false).await;
        // The tick after: a window that missed its tile in the Arrange is given it now (the place it opens at, if down).
        let map = map_of(&four);
        let soon = Stamp::utc_now().plus_secs(6.);
        tick(&app, &map, soon);
        let _ = wait_for(2000, || !iconic(slow) || rp::resting_place(slow).is_some_and(|p| near(p, tiles[2], SLACK)));
        tick(&app, &map, soon.plus_secs(1.));
        let rest = rp::resting_place(slow);
        r.check("a minimise landing while an Arrange runs: the window stays down", posted && iconic(slow), (posted, laid.took[2], &laid.note));
        r.check("…and keeps its tile as the place it opens at", rest.is_some_and(|p| near(p, tiles[2], SLACK)), (rest, tiles[2]));
        r.check("…the others are in theirs", [0, 1, 3].iter().all(|&i| near(rect_of(four[i].1), tiles[i], SLACK)), four.iter().map(|&(_, w)| rect_of(w)).collect::<Vec<_>>());
        // SAFETY: plain call; the window ends with its thread.
        unsafe { PostThreadMessageW(slow_set.thread, WM_QUIT, 0, 0) };
        for &(_, w) in &four {
            show(w, SW_SHOWMINNOACTIVE);
        }
    }

    // A window Bloxkeep hid takes no tile: the others close up, or, while a launch keeps tiles for windows to come,
    // keep their places. (Hidden as Hide windows does it: the stand-in is the one of this process titled "Roblox".)
    {
        let own = std::process::id() as i32;
        let four = take(4);
        lay(&four);
        let hid = four[1].1;
        set_title(hid, "Roblox stand-in");
        let hidden = rp::hide_windows(&[own]) == 1 && rp::is_hidden(hid);
        settle();
        let before = rect_of(hid);
        let (slots, open) = choose_tiles(&four, true, &[]);
        let _ = arrange(&app, slots.clone(), open, area, "fill", false).await;
        let t3 = tiles_of(3, "fill");
        r.check("a hidden window takes no tile and isn't moved", hidden && slots.len() == 3 && !slots.iter().any(|s| s.1 == hid) && rect_of(hid) == before, (hidden, &slots, rect_of(hid), before));
        r.check(
            "…the others close up",
            near(rect_of(four[0].1), t3[0], SLACK) && near(rect_of(four[2].1), t3[1], SLACK) && near(rect_of(four[3].1), t3[2], SLACK),
            four.iter().map(|&(_, w)| rect_of(w)).collect::<Vec<_>>(),
        );
        let mut planned: Vec<(i32, isize)> = four[..3].to_vec();
        planned.push((0, 0));
        let (slots, open) = choose_tiles(&planned, true, &[]);
        let _ = arrange(&app, slots.clone(), open, area, "fill", false).await;
        let t4 = tiles_of(4, "fill");
        r.check(
            "…while a launch keeps tiles for windows to come: its tile stays empty, the others keep their places",
            slots.len() == 4 && slots[1] == (four[1].0, 0) && near(rect_of(four[0].1), t4[0], SLACK) && near(rect_of(four[2].1), t4[2], SLACK) && rect_of(hid) == before,
            (&slots, four.iter().map(|&(_, w)| rect_of(w)).collect::<Vec<_>>()),
        );
        rp::show_windows(Some(&[own]));
        set_title(hid, "BloxkeepTestWindow");
        for &(_, w) in &four {
            show(w, SW_SHOWMINNOACTIVE);
        }
    }

    // A lent window (a live view, a nudge) is left alone, it alone, and takes its tile on the pass after it is back.
    {
        let four = take(4);
        lay(&four);
        let (p, w) = four[2];
        let before = rect_of(w);
        let loan = window_loan::borrow(w, How::VIEW);
        let (slots, open) = choose_tiles(&four, true, &[]);
        let laid = arrange(&app, slots, open, area, "2x2", false).await;
        r.check("a lent window is left where it is by an Arrange", loan.is_some() && laid.took[2].3 == Took::Skipped && rect_of(w) == before, (laid.took[2], rect_of(w), before));
        r.check("…the others are tiled all the same", [0, 1, 3].iter().all(|&i| near(rect_of(four[i].1), tiles[i], SLACK)), four.iter().map(|&(_, w)| rect_of(w)).collect::<Vec<_>>());
        if let Some(loan) = loan {
            let _ = loan.give_back();
        }
        let map = map_of(&four);
        let mut now = Stamp::utc_now().plus_secs(20.);
        for _ in 0..8 {
            tick(&app, &map, now);
            now = now.plus_secs(1.);
            let _ = wait_for(1500, || near(rect_of(w), tiles[2], SLACK));
            if app.desk_memory(p).is_some_and(|m| m.tile_given.settled()) {
                break;
            }
        }
        r.check(
            "…given back: the next pass puts it in its tile, and it is seen there",
            near(rect_of(w), tiles[2], SLACK) && app.desk_memory(p).is_some_and(|m| matches!(m.tile_given, Step::Done { .. })),
            (rect_of(w), tiles[2], app.desk_memory(p).map(|m| m.tile_given)),
        );
        for &(_, w) in &four {
            show(w, SW_SHOWMINNOACTIVE);
        }
    }

    // Two Arranges asked together: the second waits for the first, so no window is left between two tiles.
    {
        let four = take(4);
        lay(&four);
        let (a_slots, a_open) = choose_tiles(&four, true, &[]);
        let (b_slots, b_open) = choose_tiles(&four, true, &[]);
        let first = arrange(&app, a_slots, a_open, area, "2x2", false);
        let second = async {
            rt::sleep(Duration::from_millis(50)).await;
            arrange(&app, b_slots, b_open, area, "4x1", false).await
        };
        let (la, lb) = futures::future::join(first, second).await;
        let row = tiles_of(4, "4x1");
        r.check("two Arranges asked together: both carried out", la.done == 4 && lb.done == 4, (la.done, lb.done));
        r.check("…one after the other: every window in the second's tile", (0..4).all(|i| near(rect_of(four[i].1), row[i], SLACK)), four.iter().map(|&(_, w)| rect_of(w)).collect::<Vec<_>>());
        r.check(
            "…and that is the tile each remembers",
            (0..4).all(|i| app.desk_memory(four[i].0).and_then(|m| m.tile).is_some_and(|t| near(t, row[i], SLACK))),
            four.iter().map(|&(p, _)| app.desk_memory(p).and_then(|m| m.tile)).collect::<Vec<_>>(),
        );
        for &(_, w) in &four {
            show(w, SW_SHOWMINNOACTIVE);
        }
    }

    // Windows that won't be made smaller than their tiles (as Roblox keeps its least size): laid over each other.
    {
        let four = take(4);
        let least = (area.width * 7 / 10, area.height * 7 / 10);
        for (i, &(_, w)) in four.iter().enumerate() {
            keep_least(w, Some(least));
            put(w, at(20 * i as i32, 20 * i as i32, least.0, least.1));
        }
        let (slots, open) = choose_tiles(&four, true, &[]);
        let laid = arrange(&app, slots, open, area, "2x2", false).await;
        let size = Size::new(least.0, least.1);
        let spots = rp::overlap_spots(4, area, size);
        let rects: Vec<Rect> = four.iter().map(|&(_, w)| rect_of(w)).collect();
        r.check(
            "windows too big for their tiles: laid over each other, as before",
            laid.note.contains("laid over each other") && rects.iter().zip(&spots).all(|(r, p)| near(*r, Rect::at(*p, size), SLACK)),
            (&laid.note, &rects, &spots),
        );
        r.check(
            "…every one on the screen, each at its own spot",
            rects.iter().all(|r| area.inflated(SLACK, SLACK).contains_rect(*r)) && (0..4).all(|i| (0..i).all(|j| rects[i].location() != rects[j].location())),
            &rects,
        );
        r.check(
            "…each remembers its spot as its place, seen there",
            (0..4).all(|i| laid.took[i].3 == Took::Placed && app.desk_memory(four[i].0).and_then(|m| m.tile).is_some_and(|t| near(t, rects[i], SLACK))),
            (&laid.took, four.iter().map(|&(p, _)| app.desk_memory(p).and_then(|m| m.tile)).collect::<Vec<_>>()),
        );
        for &(_, w) in &four {
            keep_least(w, None);
            show(w, SW_SHOWMINNOACTIVE);
        }
    }

    // ── The app's own steps, a tick at a time (as on_proc_timer runs them) ──
    let t0 = Stamp::utc_now();
    let mut pid = 910_000;
    let mut wins: HashMap<i32, Vec<isize>> = HashMap::new();
    // A client of `account` with window `w`: opened just now, joining its game.
    let client = |app: &Rc<App>, wins: &mut HashMap<i32, Vec<isize>>, pid: i32, account: Guid, w: isize, rejoin: bool, preset: Option<Guid>, at: Stamp| {
        let launch = Rc::new(PendingLaunch::new(account, HashSet::new(), at, Some(GameTarget::new(1818, None, None, None)), rejoin, preset));
        app.controller.running.borrow_mut().insert(account, (pid, launch));
        let mut tail = Tail::new("stand-in.log".into(), pid);
        tail.state = Some(ClientState::new("joining", Some(1818), None, None, at));
        app.controller.logs.borrow_mut().tails.insert(pid, tail);
        wins.insert(pid, vec![w]);
    };
    let in_game = |app: &Rc<App>, pid: i32, at: Stamp| {
        if let Some(t) = app.controller.logs.borrow_mut().tails.get_mut(&pid) {
            t.state = Some(ClientState::new("ingame", Some(1818), Some("job".into()), None, at));
        }
    };
    let gone = |app: &Rc<App>, wins: &mut HashMap<i32, Vec<isize>>, pid: i32, account: Guid| {
        app.controller.running.borrow_mut().remove(&account);
        app.controller.logs.borrow_mut().tails.remove(&pid);
        wins.remove(&pid);
    };
    let set = |app: &Rc<App>, on_join: &str, maximise: bool| {
        let _ = app.vault_mut(|d| {
            d.settings.on_join = on_join.into();
            d.settings.maximize_on_open = maximise;
        });
    };
    let home = at(90, 80, 520, 400);
    let s = |secs: f64| t0.plus_secs(secs);
    // A window is left to load this long after its join line before it is minimised.
    let loaded = crate::core::app::presets::ON_JOIN_LOADING.as_secs_f64() + 1.;

    // Left open.
    set(&app, "", false);
    let w = next();
    put(w, home);
    pid += 1;
    client(&app, &mut wins, pid, ids[0], w, false, None, s(0.));
    in_game(&app, pid, s(-120.));
    tick(&app, &wins, s(6.));
    r.check("Leave it open: a window in its game stays open, where it is", !iconic(w) && rect_of(w) == home, (iconic(w), rect_of(w)));
    r.check("…it isn't one to keep minimised in the tiles", !app.resting_clients().contains(&pid), ());

    // Minimised: once, its place kept.
    set(&app, "minimise", false);
    let w2 = next();
    put(w2, home);
    pid += 1;
    let p2 = pid;
    client(&app, &mut wins, p2, ids[1], w2, false, None, s(10.));
    tick(&app, &wins, s(12.));
    r.check("Minimize it: not before it is in its game", !iconic(w2), ());
    in_game(&app, p2, s(15.));
    tick(&app, &wins, s(16.));
    r.check("…not while its game still loads", !iconic(w2), ());
    tick(&app, &wins, s(15. + loaded));
    r.check("…minimised once in its game", iconic(w2), ());
    r.check("…its place is the one it had", rp::resting_place(w2) == Some(home), rp::resting_place(w2));
    r.check("…it is one to keep minimised in the tiles", app.resting_clients().contains(&p2), app.resting_clients());
    r.check("…a window that was in its game long before the choice is left", !iconic(w), ());
    show(w2, SW_RESTORE);
    tick(&app, &wins, s(20.));
    r.check("…opened by hand: it stays open", !iconic(w2), ());

    // A main account's window, a preset that leaves its windows open, one that minimises where Settings don't.
    let (wm, wo) = (next(), next());
    put(wm, home);
    put(wo, home);
    pid += 2;
    let (pm, po) = (pid - 1, pid);
    client(&app, &mut wins, pm, ids[7], wm, false, None, s(20.));
    client(&app, &mut wins, po, ids[2], wo, false, Some(show_preset), s(20.));
    in_game(&app, pm, s(22.));
    in_game(&app, po, s(22.));
    tick(&app, &wins, s(23.));
    r.check("Minimize it: a main account's window is left as it is", !iconic(wm), ());
    r.check("…and isn't one to keep minimised in the tiles", !app.resting_clients().contains(&pm), ());
    r.check("…a preset set to leave its windows open is left open", !iconic(wo) && !app.resting_clients().contains(&po), ());
    set(&app, "", false);
    let wp = next();
    put(wp, home);
    pid += 1;
    let pp = pid;
    client(&app, &mut wins, pp, ids[3], wp, false, Some(minimise_preset), s(24.));
    in_game(&app, pp, s(25.));
    tick(&app, &wins, s(25. + loaded));
    r.check("Leave it open, a preset set to minimise: its window is minimised", iconic(wp) && app.resting_clients().contains(&pp), ());
    let wa = next();
    put(wa, home);
    pid += 1;
    let pa = pid;
    client(&app, &mut wins, pa, ids[4], wa, false, Some(afk_preset), s(24.));
    in_game(&app, pa, s(25.));
    tick(&app, &wins, s(25. + loaded));
    r.check("a light-AFK preset's window is minimised, and gets no tile as before", iconic(wa) && !app.resting_clients().contains(&pa), (iconic(wa), app.resting_clients()));
    for (p, a) in [(pm, ids[7]), (po, ids[2]), (pp, ids[3]), (pa, ids[4])] {
        gone(&app, &mut wins, p, a);
    }

    // Maximise with Minimise: not maximised in view, minimised, and it opens maximised.
    set(&app, "minimise", true);
    let wx = next();
    put(wx, home);
    pid += 1;
    let px = pid;
    client(&app, &mut wins, px, ids[5], wx, false, None, s(30.));
    tick(&app, &wins, s(31.));
    tick(&app, &wins, s(45.));
    r.check("Maximize it with Minimize it: not maximised in view while it loads", !zoomed(wx) && !iconic(wx), (zoomed(wx), iconic(wx)));
    in_game(&app, px, s(50.));
    tick(&app, &wins, s(50.5));
    tick(&app, &wins, s(50. + loaded));
    r.check("…minimised once in its game", iconic(wx), ());
    show(wx, SW_RESTORE);
    r.check("…and it opens maximised", zoomed(wx), rect_of(wx));
    gone(&app, &mut wins, px, ids[5]);

    // Maximise with Leave it open, a window the person put away while it loaded: left away, and it opens maximised.
    set(&app, "", true);
    let wy = next();
    put(wy, home);
    pid += 1;
    let py = pid;
    client(&app, &mut wins, py, ids[5], wy, false, None, s(60.));
    tick(&app, &wins, s(61.));
    show(wy, SW_SHOWMINNOACTIVE);
    in_game(&app, py, s(65.));
    tick(&app, &wins, s(66.));
    r.check("Maximize it, a window put away as it loaded: it stays away", iconic(wy), ());
    show(wy, SW_RESTORE);
    r.check("…and opens maximised", zoomed(wy), rect_of(wy));
    gone(&app, &mut wins, py, ids[5]);

    // ── Rejoins: the window comes back where and as the account's last one was ──
    let tile = at(640, 120, 600, 440);
    // Its last window stood open in a tile.
    set(&app, "", false);
    let (o, n) = (next(), next());
    put(o, tile);
    pid += 1;
    let p_old = pid;
    client(&app, &mut wins, p_old, ids[6], o, false, None, s(100.));
    in_game(&app, p_old, s(101.));
    tick(&app, &wins, s(102.));
    gone(&app, &mut wins, p_old, ids[6]);
    show(o, SW_SHOWMINNOACTIVE);
    put(n, home);
    pid += 1;
    let p_new = pid;
    client(&app, &mut wins, p_new, ids[6], n, true, None, s(110.));
    tick(&app, &wins, s(111.));
    r.check("a rejoin, its account's last window open in a tile: it waits to go back", app.going_back(p_new) && rect_of(n) == home, ());
    in_game(&app, p_new, s(115.));
    tick(&app, &wins, s(115.5));
    tick(&app, &wins, s(119.));
    r.check("…put back in that tile once in its game, open", near(rect_of(n), tile, 0) && !iconic(n), rect_of(n));
    tick(&app, &wins, s(126.));
    r.check("…done going back", !app.going_back(p_new), ());

    // Its last window was minimised in that tile (Leave it open: as it was).
    show(n, SW_SHOWMINNOACTIVE);
    tick(&app, &wins, s(130.));
    gone(&app, &mut wins, p_new, ids[6]);
    let n2 = next();
    put(n2, home);
    pid += 1;
    let p3 = pid;
    client(&app, &mut wins, p3, ids[6], n2, true, None, s(140.));
    tick(&app, &wins, s(141.));
    in_game(&app, p3, s(145.));
    tick(&app, &wins, s(145.5));
    tick(&app, &wins, s(149.));
    r.check("a rejoin, its account's last window minimised in a tile: put in that tile first", near(rect_of(n2), tile, 0), rect_of(n2));
    tick(&app, &wins, s(156.));
    r.check("…not minimised while its game still loads", !iconic(n2), ());
    tick(&app, &wins, s(145. + loaded));
    r.check("…then minimised again, as the last one was", iconic(n2), ());
    r.check("…and it opens in that tile", rp::resting_place(n2) == Some(tile), rp::resting_place(n2));

    // The same under a preset that leaves its windows open: one rule, as the last window was.
    tick(&app, &wins, s(160.));
    gone(&app, &mut wins, p3, ids[6]);
    let n3 = next();
    put(n3, home);
    pid += 1;
    let p4 = pid;
    client(&app, &mut wins, p4, ids[6], n3, true, Some(show_preset), s(170.));
    tick(&app, &wins, s(171.));
    in_game(&app, p4, s(175.));
    tick(&app, &wins, s(175.5));
    tick(&app, &wins, s(179.));
    tick(&app, &wins, s(186.));
    tick(&app, &wins, s(175. + loaded));
    r.check("…a preset that leaves its windows open: its rejoin is minimised again all the same, in that tile", iconic(n3) && rp::resting_place(n3) == Some(tile), (iconic(n3), rp::resting_place(n3)));

    // Minimize it: the rejoin is minimised as it joins, and opens in the tile.
    set(&app, "minimise", false);
    tick(&app, &wins, s(190.));
    gone(&app, &mut wins, p4, ids[6]);
    let n4 = next();
    put(n4, home);
    pid += 1;
    let p5 = pid;
    client(&app, &mut wins, p5, ids[6], n4, true, None, s(200.));
    tick(&app, &wins, s(201.));
    in_game(&app, p5, s(205.));
    tick(&app, &wins, s(205.5));
    tick(&app, &wins, s(209.));
    r.check("Minimize it, a rejoin: not while its game still loads", !iconic(n4), ());
    tick(&app, &wins, s(205. + loaded));
    r.check("…minimised once it has loaded", iconic(n4), ());
    r.check("…still minimised, and it opens in its account's tile", iconic(n4) && rp::resting_place(n4) == Some(tile), (iconic(n4), rp::resting_place(n4)));
    show(n4, SW_RESTORE);
    r.check("…opened by hand: in the tile", near(rect_of(n4), tile, 0) && !zoomed(n4), rect_of(n4));
    tick(&app, &wins, s(212.));
    tick(&app, &wins, s(219.));
    r.check("…and left there, done going back", near(rect_of(n4), tile, 0) && !app.going_back(p5), rect_of(n4));

    // Its last window was put away from Roblox's full screen: no place of a window to go back to.
    set(&app, "", false);
    gone(&app, &mut wins, p5, ids[6]);
    let bare = ws.bare[1];
    put(bare, screen.bounds);
    pid += 1;
    let p6 = pid;
    client(&app, &mut wins, p6, ids[6], bare, false, None, s(230.));
    in_game(&app, p6, s(231.));
    tick(&app, &wins, s(232.));
    show(bare, SW_SHOWMINNOACTIVE);
    tick(&app, &wins, s(234.));
    gone(&app, &mut wins, p6, ids[6]);
    let n5 = next();
    put(n5, home);
    pid += 1;
    let p7 = pid;
    client(&app, &mut wins, p7, ids[6], n5, true, None, s(240.));
    tick(&app, &wins, s(241.));
    r.check("a rejoin, its account's last window put away from full screen: no place to go back to", !app.going_back(p7), ());
    in_game(&app, p7, s(245.));
    tick(&app, &wins, s(249.));
    tick(&app, &wins, s(245. + loaded));
    r.check("…left where Roblox opened it, and minimised again as the last one was", iconic(n5) && rp::resting_place(n5) == Some(home), (iconic(n5), rp::resting_place(n5)));

    // ── What a rule asked holds only once Windows shows it ──
    // Maximize it, a window without a title bar (Roblox's full screen): nothing to maximise, and not marked done.
    set(&app, "", true);
    gone(&app, &mut wins, p7, ids[6]);
    let full = ws.bare[0];
    put(full, screen.bounds);
    pid += 1;
    let p8 = pid;
    client(&app, &mut wins, p8, ids[5], full, false, None, s(300.));
    in_game(&app, p8, s(301.));
    tick(&app, &wins, s(302.));
    tick(&app, &wins, s(320.));
    r.check(
        "Maximize it, a window without a title bar: left as it is, not marked done",
        !zoomed(full) && !app.presets.maximised.borrow().contains_key(&p8),
        (zoomed(full), app.presets.maximised.borrow().get(&p8).cloned()),
    );
    gone(&app, &mut wins, p8, ids[5]);

    // Arrange clicked while a window still loads: the join rule doesn't put it away after.
    set(&app, "minimise", false);
    let wc = next();
    put(wc, home);
    pid += 1;
    let p9 = pid;
    client(&app, &mut wins, p9, ids[5], wc, false, None, s(400.));
    in_game(&app, p9, s(401.));
    tick(&app, &wins, s(405.));
    let (slots, open) = choose_tiles(&[(p9, wc)], false, &[]);
    let _ = arrange(&app, slots, open, area, "fill", true).await;
    tick(&app, &wins, s(401. + loaded));
    tick(&app, &wins, s(440.));
    r.check("Minimize it, Arrange clicked while it loaded: not minimised after", !iconic(wc), ());
    gone(&app, &mut wins, p9, ids[5]);

    // After a launch, a window the join rule put away: it stays minimised, and its tile is where it opens.
    let (wb, wd, we) = (next(), next(), next());
    for w in [wb, wd, we] {
        put(w, home);
    }
    pid += 3;
    let (pb, pd, pe) = (pid - 2, pid - 1, pid);
    client(&app, &mut wins, pb, ids[2], wb, false, None, s(500.));
    client(&app, &mut wins, pd, ids[3], wd, false, Some(show_preset), s(500.));
    client(&app, &mut wins, pe, ids[4], we, false, Some(show_preset), s(500.));
    for p in [pb, pd, pe] {
        in_game(&app, p, s(501.));
    }
    tick(&app, &wins, s(502.));
    tick(&app, &wins, s(501. + loaded));
    tick(&app, &wins, s(503. + loaded));
    let rest = app.resting_clients();
    let (slots, open) = choose_tiles(&[(pb, wb), (pd, wd), (pe, we)], true, &rest);
    let laid = arrange(&app, slots, open, area, "2x2", false).await;
    let tiles = tiles_of(3, "2x2");
    tick(&app, &wins, s(540.));
    tick(&app, &wins, s(560.));
    r.check("after a launch, a window the join rule put away: it stays minimised", iconic(wb) && rest.contains(&pb) && laid.took[0].3 == Took::Resting, (iconic(wb), &rest, laid.took[0]));
    let normal = rp::resting_place(wb);
    r.check("…its normal place (GetWindowPlacement) is its tile", normal.is_some_and(|p| near(p, tiles[0], SLACK)), (normal, tiles[0]));
    r.check("…the open ones are in theirs", near(rect_of(wd), tiles[1], SLACK) && near(rect_of(we), tiles[2], SLACK), (rect_of(wd), rect_of(we)));
    for (p, a) in [(pb, ids[2]), (pd, ids[3]), (pe, ids[4])] {
        gone(&app, &mut wins, p, a);
    }

    // Arrange clicked: minimised windows come up into their tiles (one put away by the join rule, one by hand while it
    // loaded), and the join rule doesn't put them away again.
    let (wf, wg) = (next(), next());
    put(wf, home);
    put(wg, home);
    pid += 2;
    let (pf, pg) = (pid - 1, pid);
    client(&app, &mut wins, pf, ids[5], wf, false, None, s(600.));
    client(&app, &mut wins, pg, ids[6], wg, false, None, s(600.));
    in_game(&app, pf, s(601.));
    tick(&app, &wins, s(601. + loaded));
    in_game(&app, pg, s(640.));
    tick(&app, &wins, s(641.));
    show(wg, SW_SHOWMINNOACTIVE);
    let put_away = iconic(wf) && iconic(wg);
    let (slots, open) = choose_tiles(&[(pf, wf), (pg, wg)], false, &[]);
    let _ = arrange(&app, slots, open.clone(), area, "2x2", true).await;
    let tiles = tiles_of(2, "2x2");
    r.check(
        "Arrange clicked: the minimised windows are opened and tiled",
        put_away && open.len() == 2 && !iconic(wf) && !iconic(wg) && near(rect_of(wf), tiles[0], SLACK) && near(rect_of(wg), tiles[1], SLACK),
        (put_away, open.len(), iconic(wf), iconic(wg), rect_of(wf), rect_of(wg)),
    );
    tick(&app, &wins, s(640. + loaded));
    tick(&app, &wins, s(700.));
    tick(&app, &wins, s(760.));
    r.check("…the join rule doesn't put them away again", !iconic(wf) && !iconic(wg), (iconic(wf), iconic(wg)));
    gone(&app, &mut wins, pf, ids[5]);
    gone(&app, &mut wins, pg, ids[6]);

    // SAFETY: plain call; the windows end with their thread.
    unsafe { PostThreadMessageW(ws.thread, WM_QUIT, 0, 0) };
}
