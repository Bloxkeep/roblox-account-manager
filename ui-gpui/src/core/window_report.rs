//! Short words for error.log about where windows are and how they look on screen, so a report like "nothing showed" or
//! "all full screen" can be read from the log instead of guessed (three window bugs in a day, each found only from a
//! video). Process ids only, never account names: the log may be sent to us.
use windows_sys::Win32::{
    Foundation::{POINT, RECT},
    UI::WindowsAndMessaging::{GA_ROOT, GetAncestor, GetWindowRect, GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible, IsZoomed, WindowFromPoint},
};

use super::win_kit::{Point, Rect, all_screens, hwnd, process_name, screen_from_rect};

fn window_rect(h: isize) -> Option<Rect> {
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    // SAFETY: plain call with a RECT on the stack.
    if unsafe { GetWindowRect(hwnd(h), &mut r) } != 0 { Some(Rect::from_win(&r)) } else { None }
}

fn is_window(h: isize) -> bool {
    // SAFETY: plain query of a handle.
    unsafe { IsWindow(hwnd(h)) != 0 }
}
fn is_iconic(h: isize) -> bool {
    // SAFETY: plain query of a handle.
    unsafe { IsIconic(hwnd(h)) != 0 }
}
fn is_zoomed(h: isize) -> bool {
    // SAFETY: plain query of a handle.
    unsafe { IsZoomed(hwnd(h)) != 0 }
}
fn is_visible(h: isize) -> bool {
    // SAFETY: plain query of a handle.
    unsafe { IsWindowVisible(hwnd(h)) != 0 }
}

fn class_of(h: isize) -> String {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetClassNameW;
    let mut buf = [0u16; 128];
    // SAFETY: the buffer and its length match.
    let n = unsafe { GetClassNameW(hwnd(h), buf.as_mut_ptr(), buf.len() as i32) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

/// A top-level window's caption: GetWindowText reads it without asking the window (no hang on a busy client).
fn caption_of(h: isize) -> String {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextW;
    let mut buf = [0u16; 256];
    // SAFETY: the buffer and its length match.
    let n = unsafe { GetWindowTextW(hwnd(h), buf.as_mut_ptr(), buf.len() as i32) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

/// A control's words: WM_GETTEXT with a time limit, the only way to read a control of another process (GetWindowText
/// reads captions alone).
fn control_text(h: isize, wait_ms: u32) -> String {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_GETTEXT};
    let mut buf = vec![0u16; 1024];
    let mut copied: usize = 0;
    // SAFETY: WM_GETTEXT writes at most wParam units (NUL included) into the buffer, which outlives the call.
    let ok = unsafe { SendMessageTimeoutW(hwnd(h), WM_GETTEXT, buf.len(), buf.as_mut_ptr() as isize, SMTO_ABORTIFHUNG, wait_ms, &mut copied) };
    if ok == 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..copied.min(buf.len())])
}

fn children(h: isize) -> Vec<isize> {
    use windows_sys::Win32::{Foundation::HWND, UI::WindowsAndMessaging::EnumChildWindows};
    unsafe extern "system" fn each(child: HWND, found: isize) -> i32 {
        // SAFETY: `found` is the Vec passed below, alive for the whole enumeration.
        let list = unsafe { &mut *(found as *mut Vec<isize>) };
        list.push(child as isize);
        (list.len() < 16) as i32
    }
    let mut list: Vec<isize> = Vec::new();
    // SAFETY: the callback only pushes into `list`, which outlives the synchronous enumeration.
    unsafe { EnumChildWindows(hwnd(h), Some(each), &mut list as *mut Vec<isize> as isize) };
    list
}

/// Roblox's game window class.
const GAME_WINDOW_CLASS: &str = "WINDOWSCLIENT";
/// A standard Windows message box.
const DIALOG_CLASS: &str = "#32770";

/// A box in place of a game window (2026-11-14, a user's report: starts showed a 361×166 box, "Error 403" on their
/// screen and nothing of it in error.log): what it says. A standard message box gives its caption and words (asked with
/// WM_GETTEXT only, at most about a second in all); another kind its class and caption. None for Roblox's game window,
/// a minimised one, or one of a game's size.
pub fn small_window_words(h: isize) -> Option<String> {
    if h == 0 || !is_window(h) || is_iconic(h) {
        return None;
    }
    let rect = window_rect(h)?;
    if rect.width >= 640 || rect.height >= 420 {
        return None;
    }
    let class = class_of(h);
    if class.eq_ignore_ascii_case(GAME_WINDOW_CLASS) {
        return None;
    }
    let title = caption_of(h).trim().to_string();
    if class != DIALOG_CLASS {
        return Some(format!("a small window (class {class}, title \"{title}\")"));
    }
    let started = std::time::Instant::now();
    let mut words: Vec<String> = Vec::new();
    for child in children(h) {
        if started.elapsed() > std::time::Duration::from_millis(1000) {
            break;
        }
        if class_of(child).eq_ignore_ascii_case("Button") {
            continue;
        }
        let text = control_text(child, 300).split_whitespace().collect::<Vec<_>>().join(" ");
        if !text.is_empty() {
            words.push(text);
        }
    }
    Some(if words.is_empty() {
        format!("a message box \"{title}\" (its words not readable)")
    } else {
        format!("a message box \"{title}\": \"{}\"", words.join(" "))
    })
}

/// "1224×957 normal, screen 2 of 2" / "maximized" / "full screen" / "minimized" / "gone".
pub fn describe(h: isize) -> String {
    if h == 0 || !is_window(h) {
        return "gone".into();
    }
    if is_iconic(h) {
        return "minimized".into();
    }
    let Some(rect) = window_rect(h) else {
        return "unknown".into();
    };
    let screens = all_screens();
    let screen = screen_from_rect(rect);
    let state = if is_zoomed(h) {
        "maximized"
    } else if rect.contains_rect(screen.bounds) {
        "full screen"
    } else {
        "normal"
    };
    let index = screens.iter().position(|s| s.handle == screen.handle).map(|i| i as i32).unwrap_or(-1);
    format!("{}×{} {state}, screen {} of {}", rect.width, rect.height, index + 1, screens.len()) + if is_visible(h) { "" } else { ", hidden" }
}

/// How many of the windows are in each state: "4 maximized, 2 normal".
pub fn states(windows: &[isize]) -> String {
    // GroupBy keeps the order in which each state first came.
    let mut groups: Vec<(String, usize)> = Vec::new();
    for &h in windows {
        let d = describe(h);
        let s = if d.contains("maximized") {
            "maximized".to_string()
        } else if d.contains("full screen") {
            "full screen".to_string()
        } else if d.contains("normal") {
            "normal".to_string()
        } else {
            d
        };
        match groups.iter_mut().find(|g| g.0 == s) {
            Some(g) => g.1 += 1,
            None => groups.push((s, 1)),
        }
    }
    groups.iter().map(|(s, n)| format!("{n} {s}")).collect::<Vec<_>>().join(", ")
}

/// After an arrange: each window in `area`, restored, and seen at its middle (not under another app);
/// the others named with what's wrong. "5 in place" or "3 in place; 41316 maximized; 52168 under chrome".
pub fn placed(windows: &[(i32, isize)], area: Rect) -> String {
    let mut wrong: Vec<String> = Vec::new();
    let mut ok = 0;
    for &(pid, h) in windows {
        let mut why: Option<String> = None;
        if h == 0 || !is_window(h) {
            why = Some("gone".into());
        } else if is_iconic(h) {
            why = Some("minimized".into());
        } else if !is_visible(h) {
            why = Some("hidden".into());
        } else if is_zoomed(h) {
            why = Some("maximized".into());
        } else if let Some(r) = window_rect(h) {
            let middle = Point::new((r.left() + r.right()) / 2, (r.top() + r.bottom()) / 2);
            if !area.contains_point(middle) {
                why = Some(format!("off the screen asked for ({})", describe(h)));
            } else if let Some(over) = covered_by(h, middle) {
                why = Some(format!("under {over}"));
            }
        }
        match why {
            None => ok += 1,
            Some(why) => wrong.push(format!("{pid} {why}")),
        }
    }
    if wrong.is_empty() { format!("{ok} in place") } else { format!("{ok} in place; ") + &wrong.join("; ") }
}

/// The program whose window shows at `point` instead of `h`; None when it shows, or another Roblox window does
/// (overlapping layouts lay them over each other on purpose).
fn covered_by(h: isize, point: Point) -> Option<String> {
    // SAFETY: plain queries of handles and a point.
    let (root, pid) = unsafe {
        let top = WindowFromPoint(POINT { x: point.x, y: point.y });
        if top.is_null() {
            return None;
        }
        let root = GetAncestor(top, GA_ROOT);
        if root as isize == h || root.is_null() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(root, &mut pid);
        (root, pid)
    };
    let _ = root;
    match process_name(pid as i32) {
        Some(name) => {
            if name.eq_ignore_ascii_case("RobloxPlayerBeta") {
                return None;
            }
            Some(if pid == std::process::id() { "Bloxkeep".to_string() } else { name })
        }
        None => Some("another app".into()),
    }
}
