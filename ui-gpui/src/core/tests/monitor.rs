// The game monitor's picture of a window (window_thumbnails.rs), on a window of the test's own (no Roblox is touched):
// blue with an orange edge at its right and bottom. Grown, its picture follows within a moment, whole: the orange edge
// still shows. DWM draws the picture into the screen's image only, so it is read back from the screen. Checked on a
// desktop of their own that is never shown (hidden_desktop.rs): when that desktop's screen can't be read back (a desktop
// not in view isn't drawn), the window's own pixels say so, and the checks are left out with what was read.
use std::{
    collections::HashMap,
    ptr::null_mut,
};

use windows_sys::Win32::{
    Graphics::Gdi::{BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SRCCOPY, SelectObject},
    UI::WindowsAndMessaging::{HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SetWindowPos, WS_POPUP},
};

use super::{
    Checks,
    hidden_desktop::{self as hd, Kind, Rec, client_size, h, make_windows, rect_of, set_kind},
};
use crate::core::{win_kit::Rect, window_thumbnails::WindowThumbnails};

pub async fn run(c: &mut Checks, dir: &str) {
    if hd::in_view() {
        hd::run_there(c, dir, "monitor", 60_000);
        return;
    }
    let mut r = Rec::new(c);
    checks(&mut r);
    r.save(dir);
}

/// The orange points read back from the screen in `area`, every second one across and down.
fn orange_in(area: Rect) -> i32 {
    if area.width <= 0 || area.height <= 0 {
        return 0;
    }
    let mut px = vec![0u32; (area.width * area.height) as usize];
    // SAFETY: the screen's DC is released and the bitmap and DC made here are deleted before returning; the buffer holds
    // the 32-bit rows asked for.
    unsafe {
        let screen = GetDC(null_mut());
        let mem = CreateCompatibleDC(screen);
        let bmp = CreateCompatibleBitmap(screen, area.width, area.height);
        let old = SelectObject(mem, bmp);
        BitBlt(mem, 0, 0, area.width, area.height, screen, area.x, area.y, SRCCOPY);
        SelectObject(mem, old);
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = area.width;
        info.bmiHeader.biHeight = -area.height;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        GetDIBits(mem, bmp, 0, area.height as u32, px.as_mut_ptr().cast(), &mut info, DIB_RGB_COLORS);
        DeleteObject(bmp);
        DeleteDC(mem);
        ReleaseDC(null_mut(), screen);
    }
    let mut n = 0;
    for y in (0..area.height).step_by(2) {
        for x in (0..area.width).step_by(2) {
            let p = px[(y * area.width + x) as usize];
            let (red, green, blue) = ((p >> 16) & 0xFF, (p >> 8) & 0xFF, p & 0xFF);
            if red > 220 && green > 130 && green < 190 && blue < 60 {
                n += 1;
            }
        }
    }
    n
}

fn checks(r: &mut Rec<'_>) {
    const WHOLE: &str = "the picture shows the whole window, its orange edge too";
    const GROWN: &str = "grown, the picture follows within a moment and still shows the orange edge";
    let wait = |ms: u64| std::thread::sleep(std::time::Duration::from_millis(ms));
    // Two borderless windows of the test's own: the one pictured, blue with its orange edge, and the one the picture is
    // drawn in, over everything, beside it.
    let ws = make_windows(2, WS_POPUP);
    let (source, host) = (ws.all[0], ws.all[1]);
    set_kind(source, Kind { painted: true, ..Kind::default() });
    // SAFETY: plain calls on the test's own windows.
    unsafe {
        SetWindowPos(h(source), null_mut(), 0, 380, 320, 180, SWP_NOACTIVATE);
        SetWindowPos(h(host), HWND_TOPMOST, 0, 0, 480, 360, SWP_NOACTIVATE);
    }
    wait(300);
    let pictures = WindowThumbnails::new(host);
    // Where the page would say the tile's picture goes: all of the host but a 10 px edge.
    let size = client_size(host);
    let tile = Rect::new(10, 10, size.width - 20, size.height - 20);
    pictures.place(&HashMap::from([(source, tile)]), Rect::new(0, 0, size.width, size.height));
    wait(500);

    // The window itself, drawn by its own thread: if the screen doesn't give it back, it gives no picture back either.
    let drawn = orange_in(rect_of(source));
    let before = orange_in(rect_of(host));
    if drawn <= 50 {
        let why = format!(
            "this desktop isn't in view and Windows doesn't draw it: the screen read back {drawn} orange points where the test's own orange-edged window stands, and {before} where its picture is; the picture is DWM's and is seen on the screen in view only"
        );
        r.skip(WHOLE, &why);
        r.skip(GROWN, &why);
        return;
    }
    r.check(WHOLE, before > 50, before);

    // Twice the size: a stale picture would show the blue top-left quarter only.
    // SAFETY: plain call on the test's own window.
    unsafe { SetWindowPos(h(source), null_mut(), 0, 0, 640, 360, SWP_NOMOVE | SWP_NOACTIVATE) };
    wait(250);
    let grown = orange_in(rect_of(host));
    r.check(GROWN, grown > 50, grown);
    drop(pictures);
}
