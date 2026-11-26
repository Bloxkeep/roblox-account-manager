// Borrowing a Roblox window and giving it back (window_loan.rs): back in its place in the stack (under the nearest
// window still above it, else over the one below), minimised or hidden again (waited for, asked once more), seen again,
// left as it is when someone changed it meanwhile. The decision at give-back is checked on its own, with no window;
// the rest on real windows of the test's own, on a desktop of its own that is never shown (Windows stacks, minimises
// and hides windows there as on the one in view): the suite starts itself again on that desktop and tells this run
// what it found. Nothing here sends a key or a click, and no window of the person's is touched.
use std::{
    ffi::c_void,
    ptr::{null, null_mut},
    time::{Duration, Instant},
};

use windows_sys::Win32::{
    Foundation::{CloseHandle, HWND, RECT},
    System::Threading::{CreateProcessW, GetCurrentThreadId, GetExitCodeProcess, PROCESS_INFORMATION, STARTUPINFOW, WaitForSingleObject},
    UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GWL_EXSTYLE, GW_HWNDNEXT, GW_HWNDPREV, GetForegroundWindow, GetLayeredWindowAttributes, GetMessageW, GetWindow,
        GetWindowLongW, GetWindowRect, IsIconic, IsWindow, IsWindowVisible, IsZoomed, LWA_ALPHA, MSG, PostMessageW, PostThreadMessageW, RegisterClassW, SW_HIDE,
        SW_SHOWMAXIMIZED, SW_SHOWMINNOACTIVE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetForegroundWindow, SetLayeredWindowAttributes,
        SetWindowLongW, SetWindowPos, ShowWindow, WM_APP, WM_CLOSE, WM_QUIT, WNDCLASSW, WS_EX_LAYERED, WS_EX_TRANSPARENT, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
    },
};

use super::Checks;
use crate::core::{
    anti_afk::HIDDEN_ALPHA,
    win_kit::Rect,
    window_loan::{self as wl, Back, How, Kept, Restacked, Seen, Was},
};

#[link(name = "user32")]
unsafe extern "system" {
    fn CreateDesktopW(name: *const u16, device: *const u16, devmode: *const c_void, flags: u32, access: u32, security: *const c_void) -> isize;
    fn CloseDesktop(desktop: isize) -> i32;
    fn GetThreadDesktop(thread: u32) -> isize;
    fn GetUserObjectInformationW(object: isize, index: i32, info: *mut c_void, length: u32, needed: *mut u32) -> i32;
}

/// Posted to a test window: its thread does nothing for `wParam` ms, so what is asked of its windows meanwhile lands late.
const WM_BUSY: u32 = WM_APP + 1;

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
        decisions(c);
        on_a_desktop_of_its_own(c, dir);
        return;
    }
    let mut r = Rec { c, lines: Vec::new() };
    windows(&mut r);
    let _ = std::fs::write(std::path::Path::new(dir).join("results.txt"), r.lines.join("\n"));
}

/// Checks said here and kept for the run that started this one.
struct Rec<'a> {
    c: &'a mut Checks,
    lines: Vec<String>,
}

impl Rec<'_> {
    fn check(&mut self, what: &str, ok: bool, detail: impl std::fmt::Debug) {
        let detail = format!("{detail:?}");
        self.lines.push(if ok { format!("P\t{what}") } else { format!("F\t{what}\t{detail}") });
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
    const WAIT_MS: u32 = 120_000;
    let name = format!("bloxkeep-loan-test-{}", std::process::id());
    let child_dir = format!("{dir}-desk");
    let results = std::path::Path::new(&child_dir).join("results.txt");
    let _ = std::fs::remove_file(&results);
    let ran = "the window checks ran on a desktop of their own";
    // SAFETY: plain calls with strings that outlive them; the handles made are closed below.
    let code = unsafe {
        let desktop = CreateDesktopW(wide(&name).as_ptr(), null(), null(), 0, GENERIC_ALL, null());
        if desktop == 0 {
            c.skip(ran, "Windows made no desktop for them");
            return;
        }
        let exe = std::env::current_exe().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        let mut line = wide(&format!("\"{exe}\" --core-suite window-loan \"{child_dir}\""));
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

// ───────── the decision, without a window ─────────

fn decisions(c: &mut Checks) {
    let minimised = Was { minimised: true, ..Was::default() };
    let hidden = Was { unshown: true, hidden_by_us: true, ..Was::default() };
    let open = Was { to_front: true, ..Was::default() };
    let there = Seen { exists: true, plain: true, ..Seen::default() };
    let open_now = there;
    let down_now = Seen { minimised: true, plain: false, ..there };
    let hidden_now = Seen { unshown: true, hidden_by_us: true, plain: true, ..there };
    let person_in_front = Seen { front: true, person: true, ..there };
    let kept = |why: Kept| Back { kept: Some(why), ..Back::default() };

    c.check("minimised, opened, still open at the end: minimised again", wl::back(&minimised, &open_now) == Back { minimise: true, ..Back::default() }, wl::back(&minimised, &open_now));
    c.check("minimised, and down again already: nothing more", wl::back(&minimised, &down_now) == Back::default(), wl::back(&minimised, &down_now));
    c.check("minimised, now in front with a person's input: left open", wl::back(&minimised, &person_in_front) == kept(Kept::PersonInFront), wl::back(&minimised, &person_in_front));
    let only_front = Seen { front: true, ..there };
    c.check("…in front without a person's input (Bloxkeep's own): minimised again", wl::back(&minimised, &only_front).minimise, wl::back(&minimised, &only_front));
    let only_person = Seen { person: true, ..there };
    c.check("…a person's input but not in front: minimised again", wl::back(&minimised, &only_person).minimise, wl::back(&minimised, &only_person));
    let had = Was { had_front: true, ..minimised };
    c.check("…it had the keyboard while minimised already: being in front says nothing, minimised again", wl::back(&had, &person_in_front).minimise, wl::back(&had, &person_in_front));
    c.check("minimised, hidden meanwhile: never minimised (that would show it)", wl::back(&minimised, &hidden_now) == kept(Kept::HiddenMeanwhile), wl::back(&minimised, &hidden_now));
    // Hidden by Bloxkeep while minimised: both, and it goes back to both (it was left minimised on the taskbar).
    let both = Was { minimised: true, unshown: true, hidden_by_us: true, ..Was::default() };
    let shown_both = Seen { hidden_by_us: true, ..open_now };
    c.check("minimised and hidden, open at the end: minimised and hidden again", wl::back(&both, &shown_both) == Back { minimise: true, hide: true, ..Back::default() }, wl::back(&both, &shown_both));
    let down_shown = Seen { minimised: true, plain: false, hidden_by_us: true, ..there };
    c.check("…minimised again already but on the taskbar: hidden again", wl::back(&both, &down_shown) == Back { hide: true, ..Back::default() }, wl::back(&both, &down_shown));
    let down_hidden = Seen { minimised: true, unshown: true, hidden_by_us: true, plain: false, ..there };
    c.check("…minimised and hidden again already: nothing more", wl::back(&both, &down_hidden) == Back::default(), wl::back(&both, &down_hidden));
    c.check("…hidden but not minimised: left hidden (off the screen and the taskbar, as it was)", wl::back(&both, &hidden_now) == Back::default(), wl::back(&both, &hidden_now));
    c.check("…shown by Bloxkeep's Show windows meanwhile: left shown", wl::back(&both, &open_now) == kept(Kept::ShownMeanwhile), wl::back(&both, &open_now));
    c.check("…a person has it in front: left open", wl::back(&both, &Seen { hidden_by_us: true, ..person_in_front }) == kept(Kept::PersonInFront), ());

    c.check("hidden, shown for the loan, still shown: hidden again", wl::back(&hidden, &Seen { hidden_by_us: true, ..open_now }) == Back { hide: true, ..Back::default() }, ());
    c.check("…shown by Bloxkeep's Show windows meanwhile: left shown", wl::back(&hidden, &open_now) == kept(Kept::ShownMeanwhile), wl::back(&hidden, &open_now));
    c.check("…hidden again already: nothing more", wl::back(&hidden, &hidden_now) == Back::default(), ());
    let hidden_other = Was { unshown: true, ..Was::default() };
    c.check("hidden not by Bloxkeep, shown at the end: hidden again", wl::back(&hidden_other, &open_now).hide, wl::back(&hidden_other, &open_now));
    c.check("…a person has it in front: left shown", wl::back(&hidden_other, &person_in_front) == kept(Kept::PersonInFront), ());

    c.check("open, brought to the front: back in its place in the stack", wl::back(&open, &open_now) == Back { restack: true, ..Back::default() }, wl::back(&open, &open_now));
    c.check("…minimised meanwhile: left minimised, not restacked", wl::back(&open, &down_now) == kept(Kept::MinimisedMeanwhile), wl::back(&open, &down_now));
    c.check("…hidden meanwhile: left hidden", wl::back(&open, &hidden_now) == kept(Kept::HiddenMeanwhile), ());
    c.check("…a person has it in front: left in front", wl::back(&open, &person_in_front) == kept(Kept::PersonInFront), ());
    let looked = Was::default();
    c.check("open, only looked at (not brought forward, not moved): nothing to do", wl::back(&looked, &open_now) == Back::default(), wl::back(&looked, &open_now));

    let moved = Was { moved: true, ..Was::default() };
    c.check("moved for the loan, still an ordinary window: put back where it stood", wl::back(&moved, &open_now) == Back { place_back: true, ..Back::default() }, ());
    let maximised_now = Seen { plain: false, ..there };
    c.check("…maximised meanwhile: left where it is", wl::back(&moved, &maximised_now) == Back::default(), wl::back(&moved, &maximised_now));
    let moved_minimised = Was { moved: true, ..minimised };
    c.check("moved and opened from minimised: put back, then minimised", wl::back(&moved_minimised, &open_now) == Back { place_back: true, minimise: true, ..Back::default() }, ());

    c.check("gone: nothing", wl::back(&minimised, &Seen::default()) == kept(Kept::Gone), ());
    c.check("not answering: nothing (left to recover)", wl::back(&minimised, &Seen { hung: true, ..open_now }) == kept(Kept::NotAnswering), ());
}

// ───────── windows of the test's own ─────────

unsafe extern "system" fn test_proc(h: HWND, m: u32, w: usize, l: isize) -> isize {
    if m == WM_BUSY {
        // The window's thread doing something else for a while (a Roblox busy loading).
        std::thread::sleep(Duration::from_millis(w as u64));
        return 0;
    }
    // SAFETY: the window's own messages, handed on as they came.
    unsafe { DefWindowProcW(h, m, w, l) }
}

/// A thread of `count` ordinary windows that answer as any app's do; gone with the thread, at the end.
struct Windows {
    thread: u32,
    all: Vec<isize>,
}

fn make_windows(count: usize) -> Windows {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let class = wide("BloxkeepLoanTestWindow");
        // SAFETY: a class of this process with a plain procedure; windows made and pumped on this thread only.
        unsafe {
            let mut wc: WNDCLASSW = std::mem::zeroed();
            wc.lpfnWndProc = Some(test_proc);
            wc.lpszClassName = class.as_ptr();
            RegisterClassW(&wc);
            let all: Vec<isize> = (0..count as i32)
                .map(|i| CreateWindowExW(0, class.as_ptr(), class.as_ptr(), WS_OVERLAPPEDWINDOW | WS_VISIBLE, 40 + (i % 8) * 30, 40 + (i % 8) * 30, 420, 320, null_mut(), null_mut(), null_mut(), null()) as isize)
                .collect();
            let _ = tx.send((GetCurrentThreadId(), all));
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
                DispatchMessageW(&msg);
            }
        }
    });
    let (thread, all) = rx.recv().expect("the windows' thread");
    Windows { thread, all }
}

fn h(w: isize) -> HWND {
    w as _
}
fn iconic(w: isize) -> bool {
    // SAFETY: plain query.
    unsafe { IsIconic(h(w)) != 0 }
}
fn visible(w: isize) -> bool {
    // SAFETY: plain query.
    unsafe { IsWindowVisible(h(w)) != 0 }
}
fn zoomed(w: isize) -> bool {
    // SAFETY: plain query.
    unsafe { IsZoomed(h(w)) != 0 }
}
fn alive(w: isize) -> bool {
    // SAFETY: plain query.
    unsafe { IsWindow(h(w)) != 0 }
}
fn show(w: isize, how: i32) {
    // SAFETY: plain call; the window's thread answers it.
    unsafe { ShowWindow(h(w), how) };
}
fn rect_of(w: isize) -> Rect {
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    // SAFETY: one RECT on the stack.
    unsafe { GetWindowRect(h(w), &mut r) };
    Rect::new(r.left, r.top, r.right - r.left, r.bottom - r.top)
}
/// All but invisible, as a loan leaves a window it shows unseen.
fn unseen(w: isize) -> bool {
    // SAFETY: plain queries; three out values on the stack.
    unsafe {
        if GetWindowLongW(h(w), GWL_EXSTYLE) as u32 & WS_EX_LAYERED == 0 {
            return false;
        }
        let (mut key, mut alpha, mut flags) = (0u32, 0u8, 0u32);
        GetLayeredWindowAttributes(h(w), &mut key, &mut alpha, &mut flags) != 0 && flags & LWA_ALPHA != 0 && alpha == HIDDEN_ALPHA
    }
}
/// To the top of the stack, as a nudge brings a window forward (without the keyboard: none is taken here).
fn top(w: isize) {
    // SAFETY: plain call.
    unsafe { SetWindowPos(h(w), null_mut(), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
}
/// Stacked as listed, the first on top.
fn stack(ws: &[isize]) {
    for &w in ws.iter().rev() {
        top(w);
    }
}
/// The nearest shown window above or below `w`.
fn shown_next(w: isize, up: bool) -> isize {
    let mut x = w;
    for _ in 0..5000 {
        // SAFETY: plain query.
        x = unsafe { GetWindow(h(x), if up { GW_HWNDPREV } else { GW_HWNDNEXT }) } as isize;
        if x == 0 || visible(x) {
            return x;
        }
    }
    0
}
/// Each one somewhere over the next.
fn in_order(ws: &[isize]) -> bool {
    ws.windows(2).all(|p| {
        let mut x = p[0];
        for _ in 0..5000 {
            // SAFETY: plain query.
            x = unsafe { GetWindow(h(x), GW_HWNDNEXT) } as isize;
            if x == 0 {
                return false;
            }
            if x == p[1] {
                return true;
            }
        }
        false
    })
}
/// Closed by its own thread; waited for.
fn close(w: isize) {
    // SAFETY: plain call; the window's thread destroys it.
    unsafe { PostMessageW(h(w), WM_CLOSE, 0, 0) };
    let begun = Instant::now();
    while alive(w) && begun.elapsed() < Duration::from_secs(2) {
        std::thread::sleep(Duration::from_millis(10));
    }
}
/// The windows' thread does nothing for `ms`.
fn hold_thread(w: isize, ms: usize) {
    // SAFETY: plain call.
    unsafe { PostMessageW(h(w), WM_BUSY, ms, 0) };
}
/// What a window is asked comes about a moment later, on the window's own thread.
fn settle() {
    std::thread::sleep(Duration::from_millis(300));
}
fn near(a: Rect, b: Rect, slack: i32) -> bool {
    (a.x - b.x).abs() <= slack && (a.y - b.y).abs() <= slack && (a.width - b.width).abs() <= slack && (a.height - b.height).abs() <= slack
}

// ───────── the checks on windows ─────────

fn windows(r: &mut Rec<'_>) {
    let ws = make_windows(40);
    let mut free = ws.all.clone().into_iter();
    let mut next = || free.next().expect("a window left");
    let wait = wl::WAIT;

    // What this desktop allows: the keyboard can't be given to a window on a desktop that isn't the one in view.
    let before = unsafe { GetForegroundWindow() } as isize;
    let probe = next();
    // SAFETY: plain call on a window of the test's own, on its own desktop.
    let set = unsafe { SetForegroundWindow(h(probe)) } != 0;
    let after = unsafe { GetForegroundWindow() } as isize;
    let foreground_works = set && after == probe;

    // ── lent: from borrow to the end of give-back ──
    {
        let w = next();
        let loan = wl::borrow(w, How::PICTURE);
        r.check("lent: true while borrowed", loan.is_some() && wl::is_lent(w), ());
        if let Some(loan) = loan {
            loan.give_back();
        }
        r.check("…false once given back", !wl::is_lent(w), ());
        {
            let _dropped = wl::borrow(w, How::PICTURE);
        }
        r.check("…false once dropped without a give-back (given back on drop)", !wl::is_lent(w), ());
        let (a, b) = (wl::borrow(w, How::VIEW), wl::borrow(w, How::PICTURE));
        if let Some(b) = b {
            b.give_back();
        }
        r.check("two borrowers of one window (the live view and a picture): still lent while one has it", wl::is_lent(w), ());
        drop(a);
        r.check("…not once both gave it back", !wl::is_lent(w), ());
        let (x, y) = (next(), next());
        let round = wl::borrow_round(&[x, y], How::NUDGE);
        r.check("a round: every window lent from its start, before its own turn", wl::is_lent(x) && wl::is_lent(y), ());
        round.give_back();
        r.check("…none once given back (nothing taken: nothing touched)", !wl::is_lent(x) && !wl::is_lent(y), ());
    }

    // ── a round of three keeps the order ──
    {
        let (a1, r1, a2, r2, a3, r3, a4) = (next(), next(), next(), next(), next(), next(), next());
        let all = [a1, r1, a2, r2, a3, r3, a4];
        stack(&all);
        r.check("three windows lent, others between them: stacked as set up", in_order(&all), ());
        let mut round = wl::borrow_round(&[r1, r2, r3], How::NUDGE);
        round.forget_front();
        for &x in &[r1, r2, r3] {
            let lent = round.lend(x);
            top(x);
            r.check("…each taken in turn and made unseen", lent.is_some() && unseen(x), x);
        }
        r.check("…all three brought forward, last on top", in_order(&[r3, r2, r1, a1]), ());
        let back = round.give_back_with(Some(false), wait);
        r.check("…given back: the old order, each between the same windows", in_order(&all), back.iter().map(|b| b.restacked).collect::<Vec<_>>());
        r.check("…seen again, every step held", [r1, r2, r3].iter().all(|&x| !unseen(x)) && back.iter().all(|b| b.held), &back);
    }

    // ── the window it stood under is gone ──
    {
        let (a2, a1, x, b) = (next(), next(), next(), next());
        stack(&[a2, a1, x, b]);
        let mut loan = wl::borrow(x, How::NUDGE).expect("lent");
        loan.forget_front();
        top(x);
        close(a1);
        let back = loan.give_back_with(Some(false), wait);
        r.check(
            "the window it stood under closed meanwhile: under the next one that was above it",
            shown_next(x, true) == a2 && shown_next(x, false) == b && back.restacked == Some(Restacked::Under { at: 1, refused: 0, error: None }),
            (back.restacked, shown_next(x, true) == a2, shown_next(x, false) == b),
        );
        r.check("…which held (a closed window is no refusal)", back.held, back);

        let (a, x, b, c) = (next(), next(), next(), next());
        stack(&[a, x, b]);
        let mut loan = wl::borrow(x, How::NUDGE).expect("lent");
        loan.forget_front();
        top(c); // came up after the window was lent: not one it stood under
        top(x);
        close(a);
        let back = loan.give_back_with(Some(false), wait);
        r.check(
            "every window that was above it gone: over the one that was below it",
            matches!(back.restacked, Some(Restacked::Over { at: 0, second: true, .. })) && shown_next(x, false) == b && shown_next(x, true) == c,
            (back.restacked, shown_next(x, false) == b, shown_next(x, true) == c),
        );
        r.check("…said in error.log as a step that didn't hold", !back.held, back);
    }

    // ── a window above refuses (an elevated one answers error 5) ──
    {
        let (a1, a2, x, b) = (next(), next(), next(), next());
        stack(&[a1, a2, x, b]);
        top(x);
        let first_refuses = |w: isize, after: isize| if after == a1 { Err(5) } else { wl::set_under(w, after) };
        let how = wl::restack(x, &[a1, a2], &[b], &first_refuses);
        r.check(
            "the nearest window above refuses: under the next one above",
            how == Restacked::Under { at: 1, refused: 1, error: Some(5) } && shown_next(x, true) == a2 && shown_next(x, false) == b,
            how,
        );
        top(x);
        let not_a_window: isize = 0x7FFF_FFF0;
        let how = wl::restack(x, &[not_a_window, a2], &[b], &wl::set_under);
        r.check("an anchor that is no longer a window: passed over, not counted as a refusal", !alive(not_a_window) && how == Restacked::Under { at: 1, refused: 0, error: None } && shown_next(x, true) == a2, how);
        top(x);
        let above_refuse = |w: isize, after: isize| if after == a1 || after == a2 { Err(5) } else { wl::set_under(w, after) };
        let how = wl::restack(x, &[a1, a2], &[b], &above_refuse);
        r.check("every window above refuses: over the one below, which needs no word from them", matches!(how, Restacked::Over { at: 0, second: true, .. }) && shown_next(x, false) == b, how);
        top(x);
        // (What stands over it then: the desktop may keep a topmost window of its own over everything.)
        let over_before = shown_next(x, true);
        let all_refuse = |_: isize, _: isize| -> Result<(), u32> { Err(5) };
        let how = wl::restack(x, &[a1], &[b], &all_refuse);
        let over_after = shown_next(x, true);
        r.check(
            "nothing accepts it: it stays where it is, said with Windows' error",
            how == Restacked::Stuck { error: Some(5) } && over_after == over_before,
            format!("{how:?}, over it before {over_before:#x}, after {over_after:#x}"),
        );
        r.check("nothing stood over it: nothing to do", wl::restack(x, &[], &[b], &all_refuse) == Restacked::Top, ());
    }

    // ── minimised: back down, waited for, asked again ──
    {
        let w = next();
        show(w, SW_SHOWMINNOACTIVE);
        let loan = wl::borrow(w, How::PICTURE).expect("lent");
        settle();
        r.check("a minimised window lent for a picture: opened, unseen", !iconic(w) && unseen(w), (iconic(w), unseen(w)));
        let back = loan.give_back_with(Some(false), wait);
        r.check("…given back minimised, asked once, seen again", iconic(w) && back.asked == 1 && back.held && !unseen(w), (iconic(w), back, unseen(w)));

        let loan = wl::borrow(w, How::PICTURE).expect("lent");
        settle();
        hold_thread(w, 1500);
        let begun = Instant::now();
        let back = loan.give_back_with(Some(false), wait);
        r.check(
            "its minimise lands late (the window's thread busy 1.5 s): asked again after a second, and down",
            iconic(w) && back.asked == 2 && back.held && !unseen(w),
            (iconic(w), back, begun.elapsed()),
        );

        let loan = wl::borrow(w, How::PICTURE).expect("lent");
        settle();
        hold_thread(w, 3000);
        let back = loan.give_back_with(Some(false), wait);
        r.check("it doesn't land after two asks: said as not held, and seen again all the same", back.asked == 2 && !back.held && !unseen(w), (back, unseen(w)));
        std::thread::sleep(Duration::from_millis(1200));
        r.check("…(the asks land once its thread answers)", iconic(w), ());
    }

    // ── hidden: shown unseen, hidden again ──
    {
        let w = next();
        show(w, SW_HIDE);
        let loan = wl::borrow(w, How::PICTURE).expect("lent");
        settle();
        r.check("a hidden window lent for a picture: shown, unseen", visible(w) && unseen(w), (visible(w), unseen(w)));
        let back = loan.give_back_with(Some(false), wait);
        r.check("…hidden again, then seen again (its own style back)", !visible(w) && back.asked == 1 && back.held && !unseen(w), (visible(w), back));
    }

    // ── someone changed it meanwhile ──
    {
        let w = next();
        let mut loan = wl::borrow(w, How::NUDGE).expect("lent");
        loan.forget_front();
        top(w);
        show(w, SW_SHOWMINNOACTIVE);
        settle();
        let back = loan.give_back_with(Some(true), wait);
        r.check("open, minimised meanwhile: left minimised, not put back in the stack", iconic(w) && back.kept == Some(Kept::MinimisedMeanwhile) && back.restacked.is_none(), back);
        r.check("…seen again", !unseen(w), ());

        if foreground_works {
            let (w, other) = (next(), next());
            show(w, SW_SHOWMINNOACTIVE);
            // SAFETY: plain call on the test's own window.
            unsafe { SetForegroundWindow(h(other)) };
            let loan = wl::borrow(w, How::PICTURE).expect("lent");
            settle();
            // SAFETY: plain call on the test's own window.
            unsafe { SetForegroundWindow(h(w)) };
            let back = loan.give_back_with(Some(true), wait);
            r.check("minimised, opened unseen, now in front with a person's input: left open", !iconic(w) && back.kept == Some(Kept::PersonInFront), back);
            r.check("…and seen", !unseen(w), ());
            show(w, SW_SHOWMINNOACTIVE);
            unsafe { SetForegroundWindow(h(other)) };
            let loan = wl::borrow(w, How::PICTURE).expect("lent");
            settle();
            unsafe { SetForegroundWindow(h(w)) };
            let back = loan.give_back_with(Some(false), wait);
            r.check("…in front without a person's input: minimised again", iconic(w) && back.kept.is_none(), back);
        } else {
            r.skip(
                "minimised, opened unseen, now in front with a person's input: left open",
                &format!(
                    "the keyboard can't be given on a desktop that isn't in view (SetForegroundWindow said {set}, the window in front was {before:#x} before and {after:#x} after); the decision is checked without windows"
                ),
            );
        }
    }

    // ── the live view: opened in view, moved, given back ──
    {
        let w = next();
        let home = Rect::new(80, 70, 420, 320);
        // SAFETY: plain call.
        unsafe { SetWindowPos(h(w), null_mut(), home.x, home.y, home.width, home.height, SWP_NOZORDER | SWP_NOACTIVATE) };
        let mut loan = wl::borrow(w, How::VIEW).expect("lent");
        loan.move_to(Rect::new(0, 0, 800, 600));
        settle();
        r.check("the live view made it big", near(rect_of(w), Rect::new(0, 0, 800, 600), 2) && !unseen(w), rect_of(w));
        let back = loan.give_back();
        settle();
        r.check("…given back where it stood", back.placed_back && near(rect_of(w), home, 2), (back, rect_of(w)));
        let mut loan = wl::borrow(w, How::VIEW).expect("lent");
        loan.move_to(Rect::new(0, 0, 800, 600));
        settle();
        show(w, SW_SHOWMAXIMIZED);
        settle();
        let back = loan.give_back();
        r.check("…maximised meanwhile: left maximised", zoomed(w) && !back.placed_back, back);
        show(w, SW_SHOWNOACTIVATE);

        let w = next();
        show(w, SW_SHOWMINNOACTIVE);
        let loan = wl::borrow(w, How::VIEW).expect("lent");
        settle();
        r.check("the live view of a minimised window: opened, in view (not unseen)", !iconic(w) && !unseen(w), ());
        let back = loan.give_back_with(Some(false), wait);
        r.check("…minimised again when the view ends", iconic(w) && back.held, back);
    }

    // ── recover: a loan cut short ──
    {
        let w = next();
        // SAFETY: plain calls on the test's own window: left as a cut-short nudge leaves it.
        unsafe {
            let ex = GetWindowLongW(h(w), GWL_EXSTYLE);
            SetWindowLongW(h(w), GWL_EXSTYLE, ex | WS_EX_LAYERED as i32 | WS_EX_TRANSPARENT as i32);
            SetLayeredWindowAttributes(h(w), 0, HIDDEN_ALPHA, LWA_ALPHA);
        }
        let x = next();
        let loan = wl::borrow(x, How::NUDGE).expect("lent");
        r.check("a lent window is unseen on purpose", unseen(x), ());
        wl::recover(&[w, x]);
        r.check("recover: a window left unseen by a cut-short loan is seen again", !unseen(w), ());
        r.check("…one lent now is left alone", unseen(x), ());
        let mut loan = loan;
        loan.forget_front();
        loan.give_back_with(Some(false), wait);
        r.check("…and seen again at its give-back", !unseen(x), ());
    }

    // SAFETY: plain call; the windows end with their thread.
    unsafe { PostThreadMessageW(ws.thread, WM_QUIT, 0, 0) };
}
