// The check on real windows (--tool-window-check DIR): the shots draw in GPUI's test window, which can't show whether a
// face draws inside a real window of the core's, at what size, on which thread, or whether a click on it reaches the
// core (the lesson of alpha.39's tray). This makes each kind's window as the core does (a borderless pop-up; the
// dialog's on a thread of its own, the overlay's with no surface of its own), off the screen and under DWM's cloak
// (nothing is seen, nothing takes the keyboard), puts the face in it as the app does, and writes what happened: the face's
// window and place, its sizes against the window's, a picture of the window as Windows composes it (DIR/<kind>.png), and
// what a click or a key on it said to the core. Exit 0 when all hold.
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};


use gpui_kit::{AppContext, WindowKind, WindowOptions};
use serde_json::{Value, json};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::{
        Dwm::{DWMWA_CLOAK, DwmSetWindowAttribute},
        Gdi::{BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject},
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        HiDpi::GetDpiForWindow,
        WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GWL_EXSTYLE, GWL_STYLE, GetClientRect, GetMessageW, GetParent, GetWindowLongPtrW, MSG, PostMessageW, PostQuitMessage,
            RegisterClassExW, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER, SetWindowPos, ShowWindow, TranslateMessage, WM_CLOSE, WM_DESTROY, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN,
            WM_LBUTTONUP, WM_MOUSEMOVE, WNDCLASSEXW, WS_CLIPCHILDREN, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_POPUP,
        },
    },
};

use super::Pages;
use crate::{core::tool_page::{self, FromPage}, i18n::I18n};

#[link(name = "user32")]
unsafe extern "system" {
    fn PrintWindow(hwnd: HWND, hdc: windows_sys::Win32::Graphics::Gdi::HDC, flags: u32) -> i32;
}
const PW_RENDERFULLCONTENT: u32 = 2;

type Inbox = Arc<Mutex<Vec<FromPage>>>;

unsafe extern "system" fn here_proc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    // SAFETY: Windows' own default handling.
    unsafe { DefWindowProcW(h, m, w, l) }
}
unsafe extern "system" fn thread_proc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if m == WM_DESTROY {
        // SAFETY: ends this (the dialog's own) thread's loop.
        unsafe { PostQuitMessage(0) };
        return 0;
    }
    // SAFETY: Windows' own default handling.
    unsafe { DefWindowProcW(h, m, w, l) }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn class(name: &str, proc_: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT) -> Vec<u16> {
    let n = wide(name);
    // SAFETY: a plain class of this process; the name outlives the call (kept by the caller).
    unsafe {
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(proc_),
            hInstance: GetModuleHandleW(std::ptr::null()),
            lpszClassName: n.as_ptr(),
            ..std::mem::zeroed()
        };
        RegisterClassExW(&wc);
    }
    n
}

/// A window as the core makes one: a borderless pop-up off the screen, cloaked, shown without the keyboard.
fn make(cls: &[u16], see_through: bool, w: i32, h: i32) -> isize {
    // SAFETY: plain calls on a window of our own; an int on the stack.
    unsafe {
        let ex = WS_EX_TOOLWINDOW | if see_through { WS_EX_NOREDIRECTIONBITMAP } else { 0 };
        let hwnd = CreateWindowExW(ex, cls.as_ptr(), wide("Bloxkeep check").as_ptr(), WS_POPUP | WS_CLIPCHILDREN, -9000, 100, w, h, std::ptr::null_mut(), std::ptr::null_mut(), GetModuleHandleW(std::ptr::null()), std::ptr::null());
        let cloak: i32 = 1;
        DwmSetWindowAttribute(hwnd, DWMWA_CLOAK as _, &cloak as *const i32 as *const _, 4);
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        hwnd as isize
    }
}

fn client(h: isize) -> (i32, i32) {
    let mut r: RECT = unsafe { std::mem::zeroed() };
    // SAFETY: plain query into a RECT on the stack.
    unsafe { GetClientRect(h as HWND, &mut r) };
    (r.right - r.left, r.bottom - r.top)
}

fn scale(h: isize) -> f32 {
    // SAFETY: plain query.
    unsafe { GetDpiForWindow(h as HWND) }.max(96) as f32 / 96.
}

/// The window as Windows composes it (DirectComposition included), as a picture; and its share of plain black pixels.
fn picture(h: isize, path: &PathBuf) -> Option<(f32, [u8; 4])> {
    let (w, hh) = client(h);
    if w <= 0 || hh <= 0 {
        return None;
    }
    // SAFETY: a DIB of our own, selected into a memory DC for PrintWindow and read back; everything freed after.
    unsafe {
        let screen = GetDC(std::ptr::null_mut());
        let dc = CreateCompatibleDC(screen);
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader = BITMAPINFOHEADER { biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32, biWidth: w, biHeight: -hh, biPlanes: 1, biBitCount: 32, biCompression: BI_RGB, ..std::mem::zeroed() };
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let bmp = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
        let old = SelectObject(dc, bmp);
        PrintWindow(h as HWND, dc, PW_RENDERFULLCONTENT);
        let n = (w * hh) as usize;
        let px = std::slice::from_raw_parts(bits as *const u8, n * 4).to_vec();
        SelectObject(dc, old);
        DeleteObject(bmp);
        DeleteDC(dc);
        ReleaseDC(std::ptr::null_mut(), screen);
        let black = px.chunks(4).filter(|p| p[0] < 4 && p[1] < 4 && p[2] < 4).count() as f32 / n as f32;
        let mid = ((hh / 2) * w + w / 2) as usize * 4;
        let centre = [px[mid + 2], px[mid + 1], px[mid], 255];
        let mut rgba = px;
        for p in rgba.chunks_mut(4) {
            p.swap(0, 2);
            p[3] = 255;
        }
        if let Some(img) = image::RgbaImage::from_raw(w as u32, hh as u32, rgba) {
            let _ = img.save(path);
        }
        Some((black, centre))
    }
}

fn post_click(child: isize, x: f32, y: f32, s: f32) {
    let l = (((y * s) as isize) << 16) | ((x * s) as isize & 0xFFFF);
    // SAFETY: posted to our own face's window.
    unsafe {
        PostMessageW(child as HWND, WM_MOUSEMOVE, 0, l);
        PostMessageW(child as HWND, WM_LBUTTONDOWN, 1, l);
        PostMessageW(child as HWND, WM_LBUTTONUP, 0, l);
    }
}

fn post_key(child: isize, vk: usize) {
    // SAFETY: posted to our own face's window; first, as the core does when its window is activated, the face is told.
    unsafe {
        PostMessageW(child as HWND, windows_sys::Win32::UI::WindowsAndMessaging::WM_ACTIVATE, 1, 0);
        PostMessageW(child as HWND, WM_KEYDOWN, vk, 1);
        PostMessageW(child as HWND, WM_KEYUP, vk, (1 | (3 << 30)) as isize);
    }
}

struct Case {
    kind: &'static str,
    name: &'static str,
    data: Value,
    /// The window's size before the face says its own (the core's first guess), in its pixels.
    size: (f32, f32),
    /// Sized to what it drew (a dialog, a panel), or filling the window it was given (the mini window, the overlay).
    sized: bool,
    other_thread: bool,
    see_through: bool,
    /// A click (x, y in the face's pixels) or a key, and the act the core should hear from it.
    poke: Option<(Poke, &'static str)>,
}

#[derive(Clone, Copy)]
enum Poke {
    Click(f32, f32),
    Key(usize),
}

fn cases() -> Vec<Case> {
    let panel = json!({
        "macro": {
            "id": "m1", "name": "Auto farm", "hotkey": 1, "tabs": ["Main", "Extra"], "areasCount": 2,
            "counters": [{ "name": "kills", "label": "Kills" }],
            "options": [
                { "name": "speed", "label": "Speed", "kind": "number", "tab": "Main", "value": 500, "shown": "0,5 s", "min": 100, "max": 5000, "step": 100, "scale": 1000, "suffix": " s", "minShown": "0,1 s", "maxShown": "5 s" },
                { "name": "loot", "label": "Pick up loot", "kind": "check", "tab": "Main", "value": 1, "shown": "On" },
                { "name": "mode", "label": "Mode", "kind": "choice", "tab": "Main", "value": "Fast", "choices": ["Fast", "Safe", "Quiet"], "shown": "Fast" },
                { "name": "key", "label": "Attack key", "kind": "key", "tab": "Main", "value": "E", "shown": "E" },
                { "name": "colour", "label": "Enemy colour", "kind": "colour", "tab": "Main", "value": "#ff3b30", "shown": "#ff3b30" },
                { "name": "spot", "label": "Shop spot", "kind": "point", "tab": "Main", "value": { "x": 0.25, "y": 0.6 }, "shown": "25 %, 60 %" },
                { "name": "zones", "label": "Zones", "kind": "list", "tab": "Extra", "value": ["A"], "choices": ["A", "B", "C"], "shown": "1 of 3" }
            ]
        },
        "now": { "state": "ready", "runs": [], "counters": { "kills": 12 }, "startedAt": null, "lastError": null, "editingAreas": false, "hotkeysOn": true, "hasGame": true, "hasWindow": true, "recording": null, "undoable": false, "changed": true },
        "pill": false, "language": "en"
    });
    vec![
        Case {
            kind: "dialog",
            name: "dialog",
            data: json!({ "text": "Close every Roblox window? Anything not saved in the games is lost.", "heading": null, "kind": "warning", "choices": [{ "text": "Close all", "danger": true }, { "text": "Cancel" }], "def": 1, "cancel": 1, "footnote": null, "remember": "Don't ask again", "language": "en" }),
            size: (1., 1.),
            sized: true,
            other_thread: true,
            see_through: false,
            poke: Some((Poke::Key(0x1B), "cancel")),
        },
        Case {
            kind: "dialog",
            name: "dialog-click",
            data: json!({ "text": "Delete this preset?", "heading": null, "kind": "info", "choices": [{ "text": "Delete" }, { "text": "Keep" }], "def": 0, "cancel": 1, "footnote": null, "remember": null, "language": "en" }),
            size: (1., 1.),
            sized: true,
            other_thread: true,
            see_through: false,
            // (The main button is the right one: "Delete", the first choice, drawn last.)
            poke: Some((Poke::Click(400., 0.), "pick")),
        },
        Case {
            kind: "mini",
            name: "mini",
            data: json!({ "unlocked": true, "robloxCount": 2, "askCloseAll": true, "language": "en", "clients": [
                { "id": "a1", "name": "player_one", "game": "Blox Fruits", "pid": 11, "ramMb": 1180, "state": "game" },
                { "id": "a2", "name": "second_alt", "game": "Pet Simulator 99", "pid": 12, "ramMb": 960, "state": "menu" }] }),
            size: (300., 360.),
            sized: false,
            other_thread: false,
            see_through: false,
            poke: None,
        },
        Case { kind: "panel", name: "panel", data: panel.clone(), size: (360., 200.), sized: true, other_thread: false, see_through: false, poke: Some((Poke::Click(120., 68.), "run")) },
        Case {
            kind: "panel",
            name: "panel-pill",
            data: {
                let mut p = panel;
                p["pill"] = json!(true);
                p
            },
            size: (360., 200.),
            sized: true,
            other_thread: false,
            see_through: false,
            poke: None,
        },
        Case {
            kind: "overlay",
            name: "overlay",
            data: json!({ "mode": "edit", "name": "Auto farm", "language": "en", "dots": [{ "step": 0, "x": 0.3, "y": 0.4 }, { "step": 1, "x": 0.6, "y": 0.55 }],
                "areas": [{ "name": "hp", "label": "Health bar", "colour": "#22c55e", "rect": { "x": 0.1, "y": 0.7, "w": 0.3, "h": 0.1 } }] }),
            size: (800., 500.),
            sized: false,
            other_thread: false,
            see_through: true,
            poke: Some((Poke::Key(0x1B), "done")),
        },
    ]
}

pub fn window_check(out: PathBuf, assets: PathBuf) {
    let _ = std::fs::create_dir_all(&out);
    gpui_kit::application().with_assets(crate::assets::FsAssets(assets.clone())).run(move |cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(false);
        crate::theme::dress(cx);
        // (BLOXKEEP_CHECK_THEME: in that theme, as the app shows it: "sunset", the one most people have.)
        if let Ok(theme) = std::env::var("BLOXKEEP_CHECK_THEME") {
            crate::theme::show(&theme, None, cx);
        }
        crate::images::init(cx);
        let fonts = ["Regular", "Medium", "SemiBold", "Bold"]
            .iter()
            .filter_map(|w| std::fs::read(assets.join(format!("fonts/BeVietnamPro-{w}.ttf"))).ok())
            .map(std::borrow::Cow::Owned)
            .collect::<Vec<_>>();
        cx.text_system().add_fonts(fonts).ok();
        let words = I18n::from_assets(&assets);
        let here_class = class("BloxkeepToolCheck", here_proc);
        // A hidden window of the interface's own kept open all along (the app's main window, here): GPUI ends its loop
        // when its last window goes, and each face's window goes after its case.
        let keeper = cx.open_window(
            WindowOptions { kind: WindowKind::PopUp, titlebar: None, focus: false, show: false, ..Default::default() },
            |_, cx| cx.new(|_| gpui_kit::EmptyView),
        );
        cx.spawn(async move |cx| {
            let _keeper = keeper;
            let mut pages = Pages::default();
            let mut lines: Vec<String> = Vec::new();
            let mut all_ok = true;
            for case in cases() {
                let mut ok = true;
                let mut say = |ok_now: bool, line: String, lines: &mut Vec<String>| {
                    if !ok_now {
                        ok = false;
                    }
                    lines.push(format!("{}{line}", if ok_now { "  " } else { "  FAIL " }));
                };
                lines.push(format!("── {} ({}{})", case.name, if case.other_thread { "its window on a thread of its own" } else { "its window on the interface's thread" }, if case.see_through { ", no surface of its own" } else { "" }));
                // The core's window.
                let (pw, ph) = (case.size.0.ceil() as i32, case.size.1.ceil() as i32);
                let parent = if case.other_thread {
                    let (tx, rx) = std::sync::mpsc::channel();
                    let _ = crate::threads::own("tool check window", move || {
                        let cls = class("BloxkeepToolCheckThread", thread_proc);
                        let h = make(&cls, false, pw, ph);
                        let _ = tx.send(h);
                        let mut msg: MSG = unsafe { std::mem::zeroed() };
                        // SAFETY: this thread's own loop, until its window goes.
                        unsafe {
                            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                                TranslateMessage(&msg);
                                DispatchMessageW(&msg);
                            }
                        }
                    });
                    rx.recv().unwrap_or(0)
                } else {
                    make(&here_class, case.see_through, pw, ph)
                };
                let s = scale(parent);
                if !case.other_thread {
                    // SAFETY: plain call on our own window (its size at the screen's scale).
                    unsafe { SetWindowPos(parent as HWND, std::ptr::null_mut(), 0, 0, (case.size.0 * s).ceil() as i32, (case.size.1 * s).ceil() as i32, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE) };
                }
                let inbox: Inbox = Arc::new(Mutex::new(Vec::new()));
                let sink = inbox.clone();
                let id = tool_page::register(move |_, m| sink.lock().unwrap_or_else(|e| e.into_inner()).push(m));
                let take = |inbox: &Inbox| std::mem::take(&mut *inbox.lock().unwrap_or_else(|e| e.into_inner()));
                let _ = cx.update(|cx| super::on_event(&mut pages, &words, &json!({ "op": "open", "id": id, "kind": case.kind, "parent": parent }), cx));
                let mut child = 0isize;
                let mut sizes: Vec<(i32, i32, i32, i32)> = Vec::new();
                let mut acts: Vec<String> = Vec::new();
                let read = |inbox: &Inbox, child: &mut isize, sizes: &mut Vec<(i32, i32, i32, i32)>, acts: &mut Vec<String>| {
                    for m in take(inbox) {
                        match m {
                            FromPage::Attached(c) => *child = c,
                            FromPage::Message(v) => match v.get("cmd").and_then(Value::as_str) {
                                Some("size") => {
                                    let (w, h, vw, vh, _) = tool_page::drawn(&v["args"]);
                                    sizes.push((w, h, vw, vh));
                                }
                                Some("act") => acts.push(v["args"]["what"].as_str().unwrap_or("").to_string()),
                                _ => {}
                            },
                            FromPage::Gone => acts.push("(gone)".into()),
                        }
                    }
                };
                read(&inbox, &mut child, &mut sizes, &mut acts);
                say(child != 0, format!("its face's window: {child:#x}"), &mut lines);
                if child == 0 {
                    all_ok = false;
                    continue;
                }
                // SAFETY: plain queries of the face's window.
                let (style, ex, up) = unsafe { (GetWindowLongPtrW(child as HWND, GWL_STYLE) as u32, GetWindowLongPtrW(child as HWND, GWL_EXSTYLE) as u32, GetParent(child as HWND) as isize) };
                say(style & 0x4000_0000 != 0 && up == parent, format!("a child of the core's window: style {style:#010x}, ex {ex:#010x}, parent {up:#x}"), &mut lines);
                say(client(child) == client(parent), format!("fills it: face {:?}, window {:?}", client(child), client(parent)), &mut lines);

                // (What was said before its data came, the empty face, doesn't count.)
                cx.background_executor().timer(Duration::from_millis(100)).await;
                read(&inbox, &mut child, &mut sizes, &mut acts);
                sizes.clear();
                let _ = cx.update(|cx| super::on_event(&mut pages, &words, &json!({ "op": "event", "id": id, "name": case.kind, "data": case.data }), cx));
                let (cw0, ch0) = client(parent);
                let at_window = move |r: &[(i32, i32, i32, i32)]| r.last().is_some_and(|&(w, h, vw, vh)| (vw - (cw0 as f32 / s).round() as i32).abs() <= 1 && (vh - (ch0 as f32 / s).round() as i32).abs() <= 1 && w > 1 && h > 1);
                let mut ms = 0;
                while ms < 3000 && !at_window(&sizes) {
                    cx.background_executor().timer(Duration::from_millis(25)).await;
                    ms += 25;
                    read(&inbox, &mut child, &mut sizes, &mut acts);
                }
                say(at_window(&sizes), format!("its data drawn under the cloak after {ms} ms: {:?} (drawn w, h, window w, h)", sizes.last()), &mut lines);
                if case.sized {
                    if let Some(&(w, h, _, _)) = sizes.last() {
                        let (gw, gh) = ((w as f32 * s).ceil() as i32, (h as f32 * s).ceil() as i32);
                        // The core sizes its window to what was drawn, then the face to the window.
                        // SAFETY: plain call on our own window.
                        unsafe { SetWindowPos(parent as HWND, std::ptr::null_mut(), 0, 0, gw, gh, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE) };
                        tool_page::fit(parent, child, case.other_thread);
                        let fits = |r: &[(i32, i32, i32, i32)]| r.last().is_some_and(|&(w, hh, vw, vh)| (w - vw).abs() <= 1 && (hh - vh).abs() <= 1);
                        let mut ms = 0;
                        while ms < 2000 && !fits(&sizes) {
                            cx.background_executor().timer(Duration::from_millis(25)).await;
                            ms += 25;
                            read(&inbox, &mut child, &mut sizes, &mut acts);
                        }
                        say(fits(&sizes), format!("given {gw}x{gh} (scale {s}); drawn again after {ms} ms: {:?}", sizes.last()), &mut lines);
                        let want = if case.kind == "dialog" { 460 } else { 360 };
                        say(w == want, format!("as wide as its own design ({want}): {w}"), &mut lines);
                    }
                } else {
                    let (cw, chh) = client(parent);
                    let fits = sizes.last().is_some_and(|&(w, h, vw, vh)| (vw - (cw as f32 / s).round() as i32).abs() <= 1 && (vh - (chh as f32 / s).round() as i32).abs() <= 1 && w == vw && h == vh);
                    say(fits, format!("drawn at the window's size {:?} (window {}x{} at scale {s})", sizes.last(), cw, chh), &mut lines);
                }
                cx.background_executor().timer(Duration::from_millis(400)).await;
                let shot = out.join(format!("{}.png", case.name));
                match picture(parent, &shot) {
                    Some((black, centre)) => {
                        let drawn = if case.see_through { centre[0] < 120 && centre[1] < 120 && centre[2] < 120 } else { black < 0.9 };
                        say(drawn, format!("composed picture {}: {:.0} % plain black, centre {:?}", shot.display(), black * 100., centre), &mut lines);
                    }
                    None => say(false, "no picture of the window".into(), &mut lines),
                }
                if let Some((poke, want)) = case.poke {
                    let what = match poke {
                        Poke::Click(x, mut y) => {
                            if y == 0. {
                                // (The dialog's buttons: 30 px from its foot.)
                                y = sizes.last().map(|&(_, h, _, _)| h as f32 - 31.).unwrap_or(0.);
                            }
                            post_click(child, x, y, s);
                            format!("a click at {x}, {y}")
                        }
                        Poke::Key(vk) => {
                            post_key(child, vk);
                            format!("key {vk:#x}")
                        }
                    };
                    let mut ms = 0;
                    while ms < 1500 && acts.is_empty() {
                        cx.background_executor().timer(Duration::from_millis(25)).await;
                        ms += 25;
                        read(&inbox, &mut child, &mut sizes, &mut acts);
                    }
                    say(acts.first().map(String::as_str) == Some(want), format!("{what} said {acts:?} to the core (want {want:?})"), &mut lines);
                }
                if matches!(case.kind, "mini" | "panel") && !case.other_thread {
                    // Hidden, then opened again with its page kept (the core lets it go a minute after hiding): the
                    // owner's "mini window hỏng rồi, ko on/off được" (2026-11-21) was a reopen whose page never said its
                    // size, so the window waited for good. A hidden window isn't drawn: sending it news says nothing;
                    // shown under the cloak and woken, as WebToolWindow::show does, it draws and says its size.
                    // SAFETY: plain calls on our own window.
                    unsafe { ShowWindow(parent as HWND, windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE) };
                    cx.background_executor().timer(Duration::from_millis(150)).await;
                    read(&inbox, &mut child, &mut sizes, &mut acts);
                    sizes.clear();
                    let _ = cx.update(|cx| super::on_event(&mut pages, &words, &json!({ "op": "event", "id": id, "name": case.kind, "data": case.data }), cx));
                    cx.background_executor().timer(Duration::from_millis(700)).await;
                    read(&inbox, &mut child, &mut sizes, &mut acts);
                    lines.push(format!("  (hidden, sent its data again: {} size reports in 700 ms)", sizes.len()));
                    sizes.clear();
                    // SAFETY: as make() shows it: cloaked already, without the keyboard.
                    unsafe { ShowWindow(parent as HWND, SW_SHOWNOACTIVATE) };
                    let _ = cx.update(|cx| super::on_event(&mut pages, &words, &json!({ "op": "wake", "id": id, "seq": 2 }), cx));
                    let mut ms = 0;
                    while ms < 2000 && sizes.is_empty() {
                        cx.background_executor().timer(Duration::from_millis(25)).await;
                        ms += 25;
                        read(&inbox, &mut child, &mut sizes, &mut acts);
                    }
                    say(!sizes.is_empty(), format!("shown again under the cloak with its page kept: its size said after {ms} ms: {:?}", sizes.last()), &mut lines);
                }
                let _ = cx.update(|cx| super::on_event(&mut pages, &words, &json!({ "op": "close", "id": id }), cx));
                tool_page::unregister(id);
                // SAFETY: our own windows; the other thread's goes on its own thread.
                unsafe {
                    if case.other_thread {
                        PostMessageW(parent as HWND, WM_CLOSE, 0, 0);
                    } else {
                        DestroyWindow(parent as HWND);
                    }
                }
                cx.background_executor().timer(Duration::from_millis(150)).await;
                lines.push(format!("  {}", if ok { "pass" } else { "FAILED" }));
                all_ok &= ok;
                let _ = std::fs::write(out.join("result.txt"), lines.join("
") + "
");
            }
            lines.push(if all_ok { "ALL PASS".into() } else { "FAILED".into() });
            let _ = std::fs::write(out.join("result.txt"), lines.join("\n") + "\n");
            let _ = cx.update(|cx| cx.quit());
            if !all_ok {
                std::process::exit(1);
            }
        })
        .detach();
    });
}
