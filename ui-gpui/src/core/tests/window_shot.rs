// Tests/Bloxkeep.Tests/WindowShotSuite.cs: /pc screenshot's picture of a window (window_shot.rs). The C# pictured a red
// window of its own in view, minimized and hidden: those need a window and are skipped here. What runs: a picture all
// white is taken for nothing drawn yet (one like that red window, with its blue spot, isn't), and no window gives no
// picture.
use super::Checks;
use crate::core::{win_kit::Picture, window_shot};

/// Six windows of the checks' own, never shown: red, a blue spot at 20,40, drawn by their own thread as a game draws
/// its window. Hidden, so nothing comes up on the desktop: the picture code shows each unseen for its picture, as it
/// does a Roblox window Bloxkeep hid. The handles, and the word that ends their thread.
/// (`slow_ms`: each draws white until that long after it is shown, then its red: a game slow with its first frame.)
static SHOWN_AT: std::sync::Mutex<Vec<(isize, std::time::Instant)>> = std::sync::Mutex::new(Vec::new());
static SLOW_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn hidden_windows(n: usize, slow_ms: u64) -> Option<(Vec<isize>, std::sync::mpsc::Sender<()>)> {
    SLOW_MS.store(slow_ms, std::sync::atomic::Ordering::SeqCst);
    SHOWN_AT.lock().unwrap_or_else(|e| e.into_inner()).clear();
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, PAINTSTRUCT},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect, MSG, PM_REMOVE, PeekMessageW, RegisterClassW, SetTimer, TranslateMessage,
            WM_ERASEBKGND, WM_PAINT, WM_SHOWWINDOW, WM_TIMER, WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
        },
    };
    unsafe extern "system" fn proc(h: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        // SAFETY: plain GDI drawing into this window's own paint DC.
        unsafe {
            match msg {
                WM_ERASEBKGND => 1,
                // Shown: the moment is kept, and it draws again five times a second from then on, as a game does.
                WM_SHOWWINDOW if w != 0 => {
                    let mut shown = SHOWN_AT.lock().unwrap_or_else(|e| e.into_inner());
                    shown.retain(|(win, _)| *win != h as isize);
                    shown.push((h as isize, std::time::Instant::now()));
                    SetTimer(h, 1, 200, None);
                    DefWindowProcW(h, msg, w, l)
                }
                WM_TIMER => {
                    windows_sys::Win32::Graphics::Gdi::InvalidateRect(h, std::ptr::null(), 0);
                    0
                }
                WM_PAINT => {
                    let slow = SLOW_MS.load(std::sync::atomic::Ordering::SeqCst);
                    let since = SHOWN_AT.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(win, _)| *win == h as isize).map(|(_, at)| at.elapsed().as_millis() as u64);
                    let ready = slow == 0 || since.is_some_and(|ms| ms >= slow);
                    let mut ps: PAINTSTRUCT = std::mem::zeroed();
                    let dc = BeginPaint(h, &mut ps);
                    let mut all = RECT { left: 0, top: 0, right: 0, bottom: 0 };
                    GetClientRect(h, &mut all);
                    let ground = CreateSolidBrush(if ready { 0x0000_00FF } else { 0x00FF_FFFF });
                    FillRect(dc, &all, ground);
                    DeleteObject(ground);
                    if ready {
                        let blue = CreateSolidBrush(0x00FF_0000);
                        FillRect(dc, &RECT { left: 20, top: 40, right: 80, bottom: 100 }, blue);
                        DeleteObject(blue);
                    }
                    EndPaint(h, &ps);
                    0
                }
                _ => DefWindowProcW(h, msg, w, l),
            }
        }
    }
    let (made_tx, made_rx) = std::sync::mpsc::channel::<Vec<isize>>();
    let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
    std::thread::Builder::new()
        .name("shot-test-windows".into())
        .spawn(move || {
            // SAFETY: a window class and windows of this thread's own, pumped here and destroyed before it ends.
            unsafe {
                let class: Vec<u16> = "BloxkeepShotTest\0".encode_utf16().collect();
                let instance = GetModuleHandleW(std::ptr::null());
                let wc = WNDCLASSW { lpfnWndProc: Some(proc), hInstance: instance, lpszClassName: class.as_ptr(), ..std::mem::zeroed() };
                RegisterClassW(&wc);
                let mut made = Vec::new();
                for i in 0..n {
                    let h = CreateWindowExW(WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE, class.as_ptr(), std::ptr::null(), WS_POPUP, 120 + i as i32 * 24, 120 + i as i32 * 24, 400, 300, std::ptr::null_mut(), std::ptr::null_mut(), instance, std::ptr::null());
                    if !h.is_null() {
                        made.push(h as isize);
                    }
                }
                let _ = made_tx.send(made.clone());
                let mut msg: MSG = std::mem::zeroed();
                while stop_rx.try_recv().is_err() {
                    while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                for h in made {
                    DestroyWindow(h as HWND);
                }
            }
        })
        .ok()?;
    let made = made_rx.recv_timeout(std::time::Duration::from_secs(5)).ok()?;
    (made.len() == n).then_some((made, stop_tx))
}

pub async fn run(c: &mut Checks, _dir: &str) {
    for what in ["a window in view: pictured whole, its game only (no title bar)", "a minimized window: pictured whole all the same", "…minimized again after, with its own look back (not unseen, not click-through)"] {
        c.skip(what, "needs a window seen on the desktop");
    }
    // Six hidden windows pictured one after another, as /pc screenshot does for six accounts (a user's report: the
    // first whole, the next four white, 2026-11-14): each must come out drawn, and be hidden again with its own look.
    match hidden_windows(6, 0) {
        None => c.check("six hidden windows of the checks' own were made", false, ()),
        Some((windows, stop)) => {
            use windows_sys::Win32::UI::WindowsAndMessaging::{GWL_EXSTYLE, GetWindowLongW, IsWindowVisible, WS_EX_LAYERED, WS_EX_TRANSPARENT};
            let mut said: Vec<String> = Vec::new();
            let mut whole = 0;
            let mut put_back = 0;
            for (i, &h) in windows.iter().enumerate() {
                let (shot, how) = crate::core::rt::blocking(move || window_shot::capture(h, 1280)).await;
                let drawn = shot.as_ref().is_some_and(|s| {
                    !s.blank
                        && image::load_from_memory(&s.jpeg).ok().map(|p| p.to_rgb8()).is_some_and(|p| {
                            let (red, blue) = (p.get_pixel(200, 150).0, p.get_pixel(50, 70).0);
                            p.width() == 400 && p.height() == 300 && red[0] > 200 && red[2] < 60 && blue[2] > 200 && blue[0] < 60
                        })
                });
                whole += drawn as usize;
                // SAFETY: plain queries of the checks' own window.
                let (seen, ex) = unsafe { (IsWindowVisible(h as _) != 0, GetWindowLongW(h as _, GWL_EXSTYLE) as u32) };
                put_back += (!seen && ex & (WS_EX_LAYERED | WS_EX_TRANSPARENT) == 0) as usize;
                said.push(format!("{}: {}{}", i + 1, how, if drawn { "" } else { " — NOT DRAWN" }));
            }
            let _ = stop.send(());
            c.check("six hidden windows pictured in a row: every one comes out drawn (red, its blue spot), none white", whole == 6, &said);
            c.check("…each hidden again after, with its own look back (not see-through, not click-through)", put_back == 6, put_back);
            c.check("…and how each picture was come by is said for error.log (its state, the looks, the time)", said.iter().all(|s| s.contains("hidden, shown unseen") && s.contains(" ms")), &said);
        }
    }
    // A window slow with its first frame: white for 4.5 s after it is shown, as a Roblox window can be on a PC with many
    // of them. The wait was 3 s, and its picture went out white; it is 8 s now.
    std::thread::sleep(std::time::Duration::from_millis(300));
    match hidden_windows(1, 4500) {
        None => c.check("a slow window of the checks' own was made", false, ()),
        Some((windows, stop)) => {
            let h = windows[0];
            let began = std::time::Instant::now();
            let (shot, how) = crate::core::rt::blocking(move || window_shot::capture(h, 1280)).await;
            let took = began.elapsed().as_millis();
            let _ = stop.send(());
            c.check(
                "a window white for 4.5 s after it is shown is waited for and comes out drawn, not white",
                shot.as_ref().is_some_and(|s| !s.blank) && (4300..8600).contains(&took),
                format!("{how} ({took} ms)"),
            );
        }
    }

    let mut white = Picture::new(400, 300);
    white.pixels.iter_mut().for_each(|p| *p = 0xFFFF_FFFF);
    let white_blank = window_shot::blank(&white);
    // PORT: the C# used its picture of the red window; one drawn the same way stands in (red, a blue spot at 20,40).
    let mut drawn = Picture::new(400, 300);
    for y in 0..300 {
        for x in 0..400 {
            drawn.pixels[(y * 400 + x) as usize] = if (20..80).contains(&x) && (40..100).contains(&y) { 0xFF00_00FF } else { 0xFFFF_0000 };
        }
    }
    let drawn_blank = window_shot::blank(&drawn);
    c.check(
        "a picture all white is taken for nothing drawn yet, one with something on it isn't",
        white_blank && !drawn_blank,
        format!("white {white_blank}, drawn {drawn_blank}"),
    );

    c.check("no window: no picture", window_shot::capture(0, 1920).0.is_none(), ());
}
