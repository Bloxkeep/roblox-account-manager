//! A picture of a Roblox window's game, for /pc screenshot (AppController.Remote.cs). A window in view, or behind others,
//! is pictured as it is. A minimized one, or one Bloxkeep hid, has nothing to picture: it is shown for a moment the way
//! anti-AFK shows it (all but invisible, click-through, not given the keyboard: anti_afk::hide), given time
//! to draw, pictured, then minimized or hidden again. Only the game is pictured, not the title bar (an account's name).
//! Tried on the owner's PC (2026-10-29): a minimized window came out whole, in about half a second, nothing seen.
use std::{
    ffi::c_void,
    sync::{Condvar, Mutex, mpsc},
    time::Duration,
};

use windows_sys::Win32::{
    Foundation::{HWND, RECT},
    Graphics::Gdi::{BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject},
    UI::WindowsAndMessaging::{GetClientRect, IsHungAppWindow, IsIconic, IsWindow, IsWindowVisible, ShowWindowAsync},
};

use super::{
    anti_afk,
    win_kit::{Picture, hwnd},
};

/// How long a window shown again is given to draw (as anti-AFK gives a minimized one).
const DRAW_TIME: Duration = Duration::from_millis(400);

/// How long a window shown again may take for its first frame, looked at every FRAME_WAIT; a picture
/// still all one colour by then goes as it is (a game drawing nothing, Windows locked).
/// (3 s wasn't enough on a PC with six windows: four of them still came out white a week after the first report,
/// 2026-11-14. Now 8 s, with a push to draw halfway.)
const FIRST_FRAME_TIME: Duration = Duration::from_secs(8);
const FRAME_WAIT: Duration = Duration::from_millis(200);
/// A window in view whose picture is all one colour is looked at again for this long (it may be mid-load).
const IN_VIEW_TIME: Duration = Duration::from_millis(1500);

/// How long a picture may take: a Roblox that stops answering meanwhile is given up on (it would hold every /pc
/// command after it, which are answered one by one).
const PICTURE_TIME: Duration = Duration::from_secs(5);

const SW_HIDE: i32 = 0;
const SW_SHOWMINNOACTIVE: i32 = 7;
const SW_SHOWNOACTIVATE: i32 = 4;
// The client area only, and what DirectX draws too (without it, a game's window comes out black).
const PW_CLIENTONLY: u32 = 1;
const PW_RENDERFULLCONTENT: u32 = 2;

#[link(name = "user32")]
unsafe extern "system" {
    fn PrintWindow(hwnd: HWND, hdc: *mut c_void, flags: u32) -> i32;
    fn InvalidateRect(hwnd: HWND, rect: *const RECT, erase: i32) -> i32;
}

/// A picture for Discord, and how it was come by (for error.log: without it a white picture can't be explained).
pub struct Shot {
    pub jpeg: Vec<u8>,
    /// All but black (Windows locked, say).
    pub black: bool,
    /// Still all one colour when the wait ended: the game drew nothing.
    pub blank: bool,
}

fn is_window(h: isize) -> bool {
    // SAFETY: plain query of a handle.
    unsafe { IsWindow(hwnd(h)) != 0 }
}
fn is_iconic(h: isize) -> bool {
    // SAFETY: plain query of a handle.
    unsafe { IsIconic(hwnd(h)) != 0 }
}
fn is_visible(h: isize) -> bool {
    // SAFETY: plain query of a handle.
    unsafe { IsWindowVisible(hwnd(h)) != 0 }
}
fn is_hung(h: isize) -> bool {
    // SAFETY: plain query of a handle.
    unsafe { IsHungAppWindow(hwnd(h)) != 0 }
}
fn show_async(h: isize, command: i32) {
    // SAFETY: plain call; posted to the window's thread.
    unsafe {
        ShowWindowAsync(hwnd(h), command);
    }
}

/// The window's game as a JPEG no wider than `max_width`; None when it can't be pictured (gone, not
/// answering, no size). `black` (the second) says the picture came out all but black (Windows locked, say). Blocking.
pub fn capture(h: isize, max_width: i32) -> (Option<Shot>, String) {
    let (full, how) = shown_traced(h);
    let Some(full) = full else { return (None, how) };
    let blank = blank(&full);
    match picture(&full, max_width) {
        Some((jpeg, black)) => (Some(Shot { jpeg, black, blank }), how),
        None => (None, format!("{how}; the picture couldn't be made a JPEG")),
    }
}

/// Preview a visible window without showing, moving, or focusing it.
pub(crate) fn monitor_frame(h: isize) -> Option<Vec<u8>> {
    picture(&monitor_picture(h)?, 960).map(|(jpeg, _)| jpeg)
}

/// The visible client area before resampling or compression.
pub(crate) fn monitor_picture(h: isize) -> Option<Picture> {
    if h == 0 || !is_visible(h) || is_iconic(h) || is_hung(h) {
        return None;
    }
    let full = timed(h)?;
    if blank(&full) || all_but_black(&full) || !is_visible(h) || is_iconic(h) {
        return None;
    }
    Some(full)
}

/// The window's client area at full size, for a macro to look at (MacroPlayer.cs); None when it can't be pictured. A
/// minimized or hidden one is shown unseen for it, unless `shown` (a macro's turn already did). Blocking.
pub fn client(h: isize, shown: bool) -> Option<Picture> {
    if shown { timed(h) } else { shown_now(h) }
}

/// The client area with the window shown: unseen for the moment when it is minimized or hidden.
fn shown_now(h: isize) -> Option<Picture> {
    shown_traced(h).0
}

/// What a picture is, for error.log: its size and, when it is all one colour, that colour.
fn said(p: &Picture) -> String {
    if blank(p) {
        let (r, g, b) = p.rgb(p.width / 80, p.height / 80);
        format!("{}x{} all one colour #{r:02x}{g:02x}{b:02x}", p.width, p.height)
    } else {
        format!("{}x{} drawn", p.width, p.height)
    }
}

/// `shown_now`, with how it went: the window's state, the looks it took and how long, and what came out.
fn shown_traced(h: isize) -> (Option<Picture>, String) {
    // A window that isn't answering would hold whoever asks it to draw.
    if h == 0 || !is_window(h) {
        return (None, "no window".into());
    }
    if is_hung(h) {
        return (None, "window not answering".into());
    }
    let minimized = is_iconic(h);
    let unshown = !is_visible(h);
    let started = std::time::Instant::now();
    let mut looks = 0;
    if !minimized && !unshown {
        // In view (or behind other windows): pictured as it is; all one colour, looked at again for a moment.
        let result = loop {
            looks += 1;
            match timed(h) {
                Some(p) if started.elapsed() < IN_VIEW_TIME && blank(&p) => {}
                other => break other,
            }
            std::thread::sleep(FRAME_WAIT);
        };
        let how = format!("in view, {looks} look(s) in {} ms, {}", started.elapsed().as_millis(), result.as_ref().map(said).unwrap_or_else(|| "no picture (not answering, or no size)".into()));
        return (result, how);
    }
    let before = anti_afk::hide(h, (0, 0), minimized);
    show_async(h, SW_SHOWNOACTIVATE);
    std::thread::sleep(DRAW_TIME);
    // Shown again, Roblox needs a moment for its first frame, longer with many windows open; until then the
    // picture is all one colour (white: of six windows pictured from Discord, five came out white, the one in view whole, 2026-11-07).
    let waited = std::time::Instant::now();
    let (mut pushed, mut shown_again) = (false, 0);
    let result = loop {
        if is_hung(h) {
            break None;
        }
        // It went back by itself (a full-screen Roblox minimizes when it isn't in front): shown once more.
        if (minimized && is_iconic(h) || unshown && !is_visible(h)) && shown_again < 2 {
            shown_again += 1;
            show_async(h, SW_SHOWNOACTIVATE);
            std::thread::sleep(DRAW_TIME);
        }
        looks += 1;
        let picture = timed(h);
        match picture {
            Some(p) if waited.elapsed() < FIRST_FRAME_TIME && blank(&p) => {
                // Halfway and still nothing: asked to draw itself again (posted; never waits for the window).
                if !pushed && waited.elapsed() > FIRST_FRAME_TIME / 3 {
                    pushed = true;
                    // SAFETY: plain call; the whole client area, queued for the window's own thread.
                    unsafe {
                        InvalidateRect(hwnd(h), std::ptr::null(), 0);
                    }
                }
            }
            other => break other,
        }
        std::thread::sleep(FRAME_WAIT);
    };
    let how = format!(
        "{}, shown unseen, {looks} look(s) in {} ms{}{}, {}",
        if minimized { "minimized" } else { "hidden" },
        started.elapsed().as_millis(),
        if pushed { ", asked to draw again" } else { "" },
        if shown_again > 0 { format!(", went back by itself {shown_again} time(s)") } else { String::new() },
        result.as_ref().map(said).unwrap_or_else(|| "no picture (not answering, or no size)".into())
    );
    // finally:
    if minimized {
        show_async(h, SW_SHOWMINNOACTIVE);
    }
    if unshown {
        show_async(h, SW_HIDE);
    }
    // Seen again only once it is minimized or hidden again (it would flash up otherwise).
    let mut i = 0;
    while i < 10 && is_window(h) && (minimized && !is_iconic(h) || unshown && is_visible(h)) {
        std::thread::sleep(Duration::from_millis(50));
        i += 1;
    }
    anti_afk::show(&before);
    (result, how)
}

/// Windows with a picture worker still running. At most one per window: a window that stops answering would
/// otherwise get a new stuck thread (with its DC and DIB) on every look.
static BUSY: Mutex<Vec<isize>> = Mutex::new(Vec::new());
static BUSY_FREED: Condvar = Condvar::new();

/// Takes the window off BUSY when its worker ends, however it ends.
struct Free(isize);

impl Drop for Free {
    fn drop(&mut self) {
        BUSY.lock().unwrap_or_else(|e| e.into_inner()).retain(|&b| b != self.0);
        BUSY_FREED.notify_all();
    }
}

/// The picture, off the calling thread, or None after PICTURE_TIME (left to finish on its own). While the window's
/// last worker still runs, waits for it within PICTURE_TIME instead of starting another.
fn timed(h: isize) -> Option<Picture> {
    // One limit for both waits (the last worker, then this one): PICTURE_TIME in all, not twice it.
    let deadline = std::time::Instant::now() + PICTURE_TIME;
    {
        let busy = BUSY.lock().unwrap_or_else(|e| e.into_inner());
        let (mut busy, _) = BUSY_FREED.wait_timeout_while(busy, PICTURE_TIME, |b| b.contains(&h)).unwrap_or_else(|e| e.into_inner());
        if busy.contains(&h) {
            return None;
        }
        busy.push(h);
    }
    let free = Free(h);
    let (tx, rx) = mpsc::channel();
    let spawned = crate::net::Named::new("Bloxkeep window picture".into()).spawn(move || {
        let _free = free;
        let _ = tx.send(take_client(h));
    });
    if spawned.is_err() {
        return None; // the closure, with `free`, was dropped: the window is free again
    }
    rx.recv_timeout(deadline.saturating_duration_since(std::time::Instant::now())).ok().flatten()
}

/// The client area at full size; None when it can't be pictured.
fn take_client(h: isize) -> Option<Picture> {
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    // SAFETY: plain call with a RECT on the stack.
    if unsafe { GetClientRect(hwnd(h), &mut r) } == 0 || r.right - r.left < 16 || r.bottom - r.top < 16 {
        return None;
    }
    let (width, height) = (r.right - r.left, r.bottom - r.top);
    // SAFETY: a 32-bit top-down DIB section selected into a memory DC; everything made here is freed here, and its
    // pixels are copied out before.
    unsafe {
        let screen = GetDC(std::ptr::null_mut());
        let dc = CreateCompatibleDC(screen);
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let dib = CreateDIBSection(screen, &info, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
        ReleaseDC(std::ptr::null_mut(), screen);
        if dc.is_null() || dib.is_null() || bits.is_null() {
            if !dib.is_null() {
                DeleteObject(dib);
            }
            if !dc.is_null() {
                DeleteDC(dc);
            }
            return None;
        }
        let old = SelectObject(dc, dib);
        let ok = PrintWindow(hwnd(h), dc as *mut c_void, PW_CLIENTONLY | PW_RENDERFULLCONTENT) != 0;
        let result = if ok {
            // Format32bppRgb: the fourth byte means nothing; every pixel is opaque.
            let src = std::slice::from_raw_parts(bits as *const u32, (width * height) as usize);
            Some(Picture { width, height, pixels: src.iter().map(|p| p | 0xFF00_0000).collect() })
        } else {
            None
        };
        SelectObject(dc, old);
        DeleteObject(dib);
        DeleteDC(dc);
        result
    }
}

fn picture(full: &Picture, max_width: i32) -> Option<(Vec<u8>, bool)> {
    let (width, height) = (full.width, full.height);
    let black = all_but_black(full);
    let scale = 1f64.min(max_width as f64 / width as f64);
    let shown = if scale < 1.0 { Some(shrink(full, (width as f64 * scale).round() as i32, (height as f64 * scale).round() as i32)) } else { None };
    Some((jpeg(shown.as_ref().unwrap_or(full))?, black))
}

/// HighQualityBicubic in GDI+; here the `image` crate's Catmull-Rom (bicubic).
fn shrink(source: &Picture, width: i32, height: i32) -> Picture {
    let rgba = image::RgbaImage::from_raw(source.width as u32, source.height as u32, source.to_rgba()).unwrap_or_default();
    let small = image::imageops::resize(&rgba, width.max(1) as u32, height.max(1) as u32, image::imageops::FilterType::CatmullRom);
    Picture::from_rgba(small.width() as i32, small.height() as i32, small.as_raw())
}

/// JPEG at quality 80 (the `image` crate's encoder: needs its "jpeg" feature).
fn jpeg(picture: &Picture) -> Option<Vec<u8>> {
    let rgb: Vec<u8> = picture.pixels.iter().flat_map(|&p| [(p >> 16) as u8, (p >> 8) as u8, p as u8]).collect();
    let mut out = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 80);
    encoder.encode(&rgb, picture.width as u32, picture.height as u32, image::ExtendedColorType::Rgb8).ok()?;
    Some(out)
}

/// Nearly every one of 1,600 points across the picture is the colour of the first: nothing drawn yet.
pub fn blank(picture: &Picture) -> bool {
    let first = picture.rgb(picture.width / 80, picture.height / 80);
    let (mut same, mut seen) = (0, 0);
    for i in 0..40 {
        for j in 0..40 {
            let c = picture.rgb(((i as f64 + 0.5) * picture.width as f64 / 40.0) as i32, ((j as f64 + 0.5) * picture.height as f64 / 40.0) as i32);
            seen += 1;
            if (c.0 as i32 - first.0 as i32).abs() + (c.1 as i32 - first.1 as i32).abs() + (c.2 as i32 - first.2 as i32).abs() <= 24 {
                same += 1;
            }
        }
    }
    same * 50 >= seen * 49
}

/// Nearly every one of 1,600 points across the picture is black.
pub fn all_but_black(picture: &Picture) -> bool {
    let (mut lit, mut seen) = (0, 0);
    for i in 0..40 {
        for j in 0..40 {
            let c = picture.rgb(((i as f64 + 0.5) * picture.width as f64 / 40.0) as i32, ((j as f64 + 0.5) * picture.height as f64 / 40.0) as i32);
            seen += 1;
            if c.0 as i32 + c.1 as i32 + c.2 as i32 > 30 {
                lit += 1;
            }
        }
    }
    lit * 50 < seen
}
