//! Each account's Roblox window gets the account's picture (or initials on its colour) as its icon: taskbar, Alt+Tab,
//! title bar. Roblox keeps an icon set on its window (seen on 0.743); one that drops it gets it again at its next
//! check (CHECK_EVERY). UI thread only.
use std::{
    collections::{HashMap, HashSet},
    ffi::c_void,
    sync::{LazyLock, Mutex},
};

use windows_sys::Win32::{
    Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateCompatibleDC, CreateDIBSection, CreateSolidBrush, DIB_RGB_COLORS, DeleteDC, DeleteObject, FillRect, GetDC,
        GetTextExtentPoint32W, ReleaseDC, SelectObject, SetBkMode, SetTextColor, TRANSPARENT, TextOutW,
    },
    Foundation::{RECT, SIZE},
    UI::WindowsAndMessaging::{CreateIconIndirect, DestroyIcon, ICONINFO, SMTO_ABORTIFHUNG, SendMessageTimeoutW},
};

use super::{
    clock, log, roblox_processes,
    win_kit::{Picture, hwnd},
};

#[link(name = "gdi32")]
unsafe extern "system" {
    /// Declared here with plain integers: the font's numbers as wingdi.h has them.
    fn CreateFontW(
        height: i32,
        width: i32,
        escapement: i32,
        orientation: i32,
        weight: i32,
        italic: u32,
        underline: u32,
        strike_out: u32,
        char_set: u32,
        out_precision: u32,
        clip_precision: u32,
        quality: u32,
        pitch_and_family: u32,
        face: *const u16,
    ) -> *mut c_void;
}

const FW_NORMAL: i32 = 400;
const DEFAULT_CHARSET: u32 = 1;
const OUT_DEFAULT_PRECIS: u32 = 0;
const CLIP_DEFAULT_PRECIS: u32 = 0;
const ANTIALIASED_QUALITY: u32 = 4;
const DEFAULT_PITCH: u32 = 0;

const WM_GETICON: u32 = 0x7F;
const WM_SETICON: u32 = 0x80;
const ICON_SMALL: usize = 0;
const ICON_BIG: usize = 1;

/// How often a client's windows are asked whether they still have their icon. Roblox puts its own back now and then
/// (a teleport, a reset): every half a minute, its logo stood on the taskbar for up to 30 s. Asking waits for a busy
/// window (up to 300 ms) on the shared pool, not on Bloxkeep's window, so every client due is asked in the same pass.
const CHECK_EVERY_MS: i64 = 3_000;

/// The checks' clock (Tests/Bloxkeep.Tests): when set, it stands in for Environment.TickCount64. The app never sets it.
pub static CLOCK: Mutex<Option<fn() -> i64>> = Mutex::new(None);

fn now() -> i64 {
    match *CLOCK.lock().unwrap_or_else(|e| e.into_inner()) {
        Some(f) => f(),
        None => clock::tick_count64(),
    }
}

/// The icons made for a client, and each of its windows' own icons from before (put back when they go).
struct Made {
    big: isize,
    small: isize,
    key: String,
    own: HashMap<isize, (isize, isize)>,
    /// When its windows were last given the icons or asked whether they still have them (`now`).
    checked: i64,
    /// Its window lost its icon and, busy, didn't take it back: said once, not at every try until it does.
    busy_said: bool,
}

struct Icons {
    by_pid: HashMap<i32, Made>,
    /// What the file holds now ("" for none), so an unchanged list isn't written again at every check.
    saved: Option<String>,
}

static ICONS: LazyLock<Mutex<Icons>> = LazyLock::new(|| Mutex::new(Icons { by_pid: HashMap::new(), saved: None }));

fn icons() -> std::sync::MutexGuard<'static, Icons> {
    ICONS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Set once Bloxkeep gives the icons back at quit: a pass still running on its worker then puts none on again.
static CLOSED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// No more icons from here on (Bloxkeep is closing); `remove` still works.
pub fn close() {
    CLOSED.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// `key`: what the picture shows (the picture's address, or the initials and colour): drawn again when it changes.
/// `check`: its windows are asked whether they still have the icons (`next_to_check`).
pub fn apply(pid: i32, windows: &[isize], key: &str, draw: &dyn Fn() -> Picture, check: bool) {
    let mut s = icons();
    // Asked with the list held: give_icons_back's removal then comes either after this pass or finds it stopped here.
    if CLOSED.load(std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let differs = s.by_pid.get(&pid).is_none_or(|m| m.key != key);
    if differs {
        let picture = draw();
        let own = s.by_pid.get(&pid).map(|m| m.own.clone()).unwrap_or_default();
        let mut next = Made { big: icon_of(&picture, 48), small: icon_of(&picture, 24), key: key.to_string(), own, checked: now(), busy_said: false };
        let mut failed = false;
        for &h in windows {
            failed |= !put(h, &mut next);
        }
        // A window busy loading its game didn't take it: asked again at the next check, not CHECK_EVERY later.
        if failed {
            next.checked = 0;
            log::note(&format!("Window icons: a Roblox window of pid {pid} didn't take its icon (busy); tried again shortly."));
        }
        if let Some(made) = s.by_pid.remove(&pid) {
            destroy(&made);
        }
        s.by_pid.insert(pid, next);
        save(&mut s);
        return;
    }
    if !check {
        return;
    }
    if let Some(made) = s.by_pid.get_mut(&pid) {
        let mut failed = false;
        // The taskbar shows the big one, the title bar the small one: either can be lost.
        for &h in windows {
            if get(h, ICON_BIG) != made.big || get(h, ICON_SMALL) != made.small {
                let ok = put(h, made);
                if ok {
                    redraw_button(h);
                }
                failed |= !ok;
                // A window busy loading is tried again at each check until it takes it: said once, then once put back.
                if ok || !made.busy_said {
                    log::note(&format!(
                        "Window icons: a Roblox window of pid {pid} had lost its icon; {}.",
                        if ok { "put back" } else { "it didn't take it (busy), tried again shortly" }
                    ));
                }
                made.busy_said = !ok;
            }
        }
        if failed {
            made.checked = 0;
        }
    }
    save(&mut s);
}

/// The clients whose windows are due to be asked (CHECK_EVERY_MS since the last time), noted as checked now.
pub fn due_to_check(pids: &[i32]) -> Vec<i32> {
    let now = now();
    let mut s = icons();
    let mut due = Vec::new();
    for &pid in pids {
        if let Some(made) = s.by_pid.get_mut(&pid)
            && now - made.checked >= CHECK_EVERY_MS
        {
            made.checked = now;
            due.push(pid);
        }
    }
    due
}

/// The windows' own icons back (the setting turned off, Bloxkeep closing).
pub fn remove(pid: i32, windows: &[isize]) {
    let mut s = icons();
    let Some(made) = s.by_pid.remove(&pid) else {
        return;
    };
    for &h in windows {
        let own = made.own.get(&h).copied().unwrap_or((0, 0));
        set(h, own.0, own.1);
    }
    destroy(&made);
    save(&mut s);
}

pub fn forget(alive: &HashSet<i32>) {
    let mut s = icons();
    let gone: Vec<i32> = s.by_pid.keys().filter(|pid| !alive.contains(pid)).copied().collect();
    for pid in &gone {
        if let Some(made) = s.by_pid.remove(pid) {
            destroy(&made);
        }
    }
    if !gone.is_empty() {
        save(&mut s);
    }
}

// ───────── icons a run that ended suddenly left ─────────
// The icons are Bloxkeep's: after a crash or End task its windows pointed at icons gone with it (no icon), and the
// next run took those for the windows' own and put them back later (review 2026-10-30). The windows given icons are
// written down (client and window), as hidden windows are, and cleared at the next start.

fn icons_file() -> std::path::PathBuf {
    crate::core::vault::data_dir().join("window-icons.txt")
}

fn save(s: &mut Icons) {
    let mut lines: Vec<String> = s.by_pid.iter().flat_map(|(pid, m)| m.own.keys().map(move |h| format!("{pid} {}", *h as i64))).collect();
    lines.sort(); // Order(StringComparer.Ordinal)
    let text = lines.join("\r\n");
    if s.saved.as_deref() == Some(text.as_str()) {
        return;
    }
    let done = if text.is_empty() {
        match std::fs::remove_file(icons_file()) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    } else {
        std::fs::write(icons_file(), &text)
    };
    match done {
        Ok(()) => s.saved = Some(text),
        Err(ex) => log::write(&ex),
    }
}

/// At start: the windows a run that ended suddenly gave icons (only those still there, of the same process)
/// are left with none of Bloxkeep's, so they show their program's.
pub fn clear_leftovers() -> i32 {
    let mut cleared = 0;
    let path = icons_file();
    if !path.is_file() {
        return 0;
    }
    let done = (|| -> std::io::Result<()> {
        let text = std::fs::read_to_string(&path)?;
        for line in text.lines() {
            let parts: Vec<&str> = line.split(' ').collect();
            if parts.len() != 2 {
                continue;
            }
            let (Ok(pid), Ok(handle)) = (parts[0].trim().parse::<i32>(), parts[1].trim().parse::<i64>()) else {
                continue;
            };
            let h = handle as isize;
            if roblox_processes::still_of(h, pid) {
                set(h, 0, 0);
                cleared += 1;
            }
        }
        std::fs::remove_file(&path)?;
        icons().saved = Some(String::new());
        Ok(())
    })();
    if let Err(ex) = done {
        log::write(&ex);
    }
    cleared
}

pub fn pids() -> Vec<i32> {
    icons().by_pid.keys().copied().collect()
}

/// Puts the made icons on a window, noting first the icons it had of its own (once: later it has ours).
/// False when the window didn't answer in time (it may not have them).
/// The window's taskbar button drawn again with the icon it has now. A new icon alone doesn't redraw it: Roblox puts
/// its own back, the window's title is set (Bloxkeep's "Roblox - name (PID)"), the taskbar reads the icon then, Roblox's,
/// and the account's put back after it ("put back" in the log) left Roblox's logo on the button. The title set again
/// as it is makes the taskbar read the window again.
fn redraw_button(h: isize) {
    const WM_GETTEXT: u32 = 0x0D;
    const WM_SETTEXT: u32 = 0x0C;
    let mut text = [0u16; 512];
    let mut got = 0usize;
    // SAFETY: plain messages with a timeout; the text buffer outlives both and ends in a NUL (its last place kept 0).
    unsafe {
        if SendMessageTimeoutW(hwnd(h), WM_GETTEXT, text.len() - 1, text.as_mut_ptr() as isize, SMTO_ABORTIFHUNG, 300, &mut got) == 0 {
            return;
        }
        text[got.min(text.len() - 1)] = 0;
        SendMessageTimeoutW(hwnd(h), WM_SETTEXT, 0, text.as_ptr() as isize, SMTO_ABORTIFHUNG, 300, &mut got);
    }
}

fn put(h: isize, made: &mut Made) -> bool {
    if !made.own.contains_key(&h) {
        made.own.insert(h, (get(h, ICON_BIG), get(h, ICON_SMALL)));
    }
    set(h, made.big, made.small)
}

fn set(h: isize, big: isize, small: isize) -> bool {
    let mut result = 0usize;
    // SAFETY: plain messages with a timeout; a hung window is skipped.
    unsafe {
        let a = SendMessageTimeoutW(hwnd(h), WM_SETICON, ICON_BIG, big, SMTO_ABORTIFHUNG, 300, &mut result);
        let b = SendMessageTimeoutW(hwnd(h), WM_SETICON, ICON_SMALL, small, SMTO_ABORTIFHUNG, 300, &mut result);
        a != 0 && b != 0
    }
}

fn get(h: isize, which: usize) -> isize {
    let mut icon = 0usize;
    // SAFETY: plain message with a timeout; the answer is the icon's handle.
    let ok = unsafe { SendMessageTimeoutW(hwnd(h), WM_GETICON, which, 0, SMTO_ABORTIFHUNG, 300, &mut icon) };
    if ok != 0 { icon as isize } else { 0 }
}

fn destroy(made: &Made) {
    // SAFETY: the icons were made by icon_of and are destroyed once.
    unsafe {
        DestroyIcon(made.big as _);
        DestroyIcon(made.small as _);
    }
}

/// The picture in a circle, as the app draws avatars, at an icon's size.
/// PORT: GDI+'s HighQualityBicubic DrawImage is the `image` crate's Catmull-Rom; its circle clip (a region, not
/// smoothed) keeps the pixels whose middle is inside the circle.
fn icon_of(picture: &Picture, size: i32) -> isize {
    let rgba = image::RgbaImage::from_raw(picture.width as u32, picture.height as u32, picture.to_rgba()).unwrap_or_default();
    let scaled = image::imageops::resize(&rgba, size as u32, size as u32, image::imageops::FilterType::CatmullRom);
    // circle.AddEllipse(0, 0, size - 1, size - 1), pixels offset by half (HighQuality).
    let r = (size - 1) as f64 / 2.0;
    let mut bgra = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let (dx, dy) = (x as f64 + 0.5 - r, y as f64 + 0.5 - r);
            if dx * dx + dy * dy > r * r {
                continue;
            }
            let p = scaled.get_pixel(x as u32, y as u32).0;
            let i = ((y * size + x) * 4) as usize;
            bgra[i..i + 4].copy_from_slice(&[p[2], p[1], p[0], p[3]]);
        }
    }
    make_icon(&bgra, size)
}

/// Bitmap.GetHicon: a 32-bit icon (straight alpha) of `bgra`, top row first.
fn make_icon(bgra: &[u8], size: i32) -> isize {
    // SAFETY: a DIB section of size×size 32-bit pixels filled from `bgra`, and an empty mask; both are deleted after
    // CreateIconIndirect copied them.
    unsafe {
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size,
            biHeight: -size,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        };
        let screen = GetDC(std::ptr::null_mut());
        let mut bits: *mut c_void = std::ptr::null_mut();
        let color = CreateDIBSection(screen, &info, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
        ReleaseDC(std::ptr::null_mut(), screen);
        if color.is_null() || bits.is_null() {
            return 0;
        }
        std::ptr::copy_nonoverlapping(bgra.as_ptr(), bits as *mut u8, bgra.len());
        let mask_bits = vec![0u8; (((size + 15) / 16 * 2) * size) as usize];
        let mask = CreateBitmap(size, size, 1, 1, mask_bits.as_ptr() as *const c_void);
        let icon_info = ICONINFO { fIcon: 1, xHotspot: 0, yHotspot: 0, hbmMask: mask, hbmColor: color };
        let icon = CreateIconIndirect(&icon_info);
        DeleteObject(color);
        DeleteObject(mask);
        icon as isize
    }
}

/// An account without a picture: up to two letters of its name on its own colour (the page's hue).
/// PORT: GDI+'s AntiAliasGridFit DrawString is GDI's ANTIALIASED_QUALITY TextOut here, centred by GDI's own measure.
pub fn initials(name: &str) -> Picture {
    let letters = initials_of(name);
    let (r, g, b) = name_color(name);
    let size = 96;
    let mut picture = Picture::new(size, size);
    // SAFETY: a 96×96 DIB section in a memory DC; the font, brush, bitmap and DC made here are all freed here.
    unsafe {
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size,
            biHeight: -size,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        };
        let screen = GetDC(std::ptr::null_mut());
        let dc = CreateCompatibleDC(screen);
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
            picture.fill(0xFF00_0000 | ((r as u32) << 16) | ((g as u32) << 8) | b as u32);
            return picture;
        }
        let old_bitmap = SelectObject(dc, dib);
        // g.Clear(NameColor(name)): a COLORREF is 0x00BBGGRR.
        let brush = CreateSolidBrush((r as u32) | ((g as u32) << 8) | ((b as u32) << 16));
        let all = RECT { left: 0, top: 0, right: size, bottom: size };
        FillRect(dc, &all, brush);
        DeleteObject(brush);
        // new Font("Segoe UI Semibold", 34 or 40, GraphicsUnit.Pixel): an em of that many pixels (a negative height).
        let face: Vec<u16> = "Segoe UI Semibold".encode_utf16().chain(std::iter::once(0)).collect();
        let em = if letters.chars().count() > 1 { 34 } else { 40 };
        let font = CreateFontW(-em, 0, 0, 0, FW_NORMAL as i32, 0, 0, 0, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, ANTIALIASED_QUALITY, DEFAULT_PITCH, face.as_ptr());
        let old_font = SelectObject(dc, font);
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, 0x00FF_FFFF);
        let text: Vec<u16> = letters.encode_utf16().collect();
        let mut measured = SIZE { cx: 0, cy: 0 };
        GetTextExtentPoint32W(dc, text.as_ptr(), text.len() as i32, &mut measured);
        TextOutW(dc, (size - measured.cx) / 2, (size - measured.cy) / 2, text.as_ptr(), text.len() as i32);
        let src = std::slice::from_raw_parts(bits as *const u32, (size * size) as usize);
        // GDI leaves the fourth byte at 0: the picture is opaque everywhere.
        for (i, p) in src.iter().enumerate() {
            picture.pixels[i] = p | 0xFF00_0000;
        }
        SelectObject(dc, old_font);
        DeleteObject(font);
        SelectObject(dc, old_bitmap);
        DeleteObject(dib);
        DeleteDC(dc);
    }
    picture
}

/// Up to two letters or digits of a name, "?" when it has none.
pub fn initials_of(name: &str) -> String {
    let letters: String = name.chars().filter(|c| c.is_alphanumeric()).take(2).collect::<String>().to_uppercase();
    if letters.is_empty() { "?".into() } else { letters }
}

/// The colour behind a name's initials, as the page gives it (red, green, blue).
pub fn name_color(name: &str) -> (u8, u8, u8) {
    from_hue(hue(name), 0.45, 0.42)
}

/// The same hue the page gives a name (app.js hue): over the name's UTF-16 units, as C#'s chars.
fn hue(s: &str) -> i32 {
    let mut h: u32 = 7;
    for c in s.encode_utf16() {
        h = h.wrapping_mul(31).wrapping_add(c as u32);
    }
    (h % 360) as i32
}

fn from_hue(hue: i32, s: f64, l: f64) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((hue as f64 / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match hue / 60 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (((r + m) * 255.0) as i32 as u8, ((g + m) * 255.0) as i32 as u8, ((b + m) * 255.0) as i32 as u8)
}
