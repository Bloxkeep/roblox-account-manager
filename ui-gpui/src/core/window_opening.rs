//! The main window's entrance on start (owner, 2026-11-08: the window itself should come onto the screen with an
//! effect, not just appear). A see-through stand-in, drawn as the window will look (its colour, corners, start logo),
//! grows from the middle to its full size with a little overshoot while fading in. Then the real window, waiting
//! invisible just above it, fades in over it (only its title text and buttons differ) and the stand-in goes
//! (ShellForm.OpenWindowAsync). A stand-in because a real window can fade but not scale, and Windows' own opening
//! animation is off on many PCs (the owner's too). Clicks go through it to the real window.
use std::{ffi::c_void, time::Duration};

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM},
    Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, HDC, HGDIOBJ, ReleaseDC,
        SelectObject,
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, RegisterClassW, SetWindowPos, ShowWindow, TranslateMessage,
        UpdateLayeredWindow, WNDCLASSW,
    },
};

use super::{
    base::{Fail, Res},
    win_kit::{Picture, Rect, Size, hwnd, wide},
};

pub const GROW_MS: f64 = 420.0;
const FROM: f64 = 0.86;

pub struct WindowOpening {
    plate: Picture,
    /// The window's frame with room around it for the overshoot (which was cut off at the window's edges).
    frame: Rect,
    dc: isize,
    dib: isize,
    old: isize,
    bits: *mut u32,
    /// Drawn here, then copied into the surface Windows shows.
    canvas: Picture,
    handle: isize,
}

unsafe extern "system" fn opening_proc(h: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: the default handling of every message (NativeWindow's).
    unsafe { DefWindowProcW(h, msg, wparam, lparam) }
}

impl WindowOpening {
    /// `frame`: where the window shows on screen, without its unseen borders (device pixels).
    /// `plate`: the window as it will look at full size (`plate`).
    pub fn new(frame: Rect, plate: Picture) -> Res<WindowOpening> {
        let frame = frame.inflated((frame.width as f64 * 0.01).ceil() as i32 + 2, (frame.height as f64 * 0.01).ceil() as i32 + 2);
        // SAFETY: a 32-bit top-down DIB section in a memory DC, and a layered popup window; all freed in Drop.
        unsafe {
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: frame.width,
                biHeight: -frame.height, // top down
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
                return Err(Fail::other("No drawing surface for the opening."));
            }
            let old = SelectObject(dc, dib);
            const WS_POPUP: u32 = 0x8000_0000;
            const WS_EX_LAYERED: u32 = 0x80000;
            const WS_EX_TRANSPARENT: u32 = 0x20;
            const WS_EX_TOOLWINDOW: u32 = 0x80;
            const WS_EX_NOACTIVATE: u32 = 0x800_0000;
            let class = wide("BloxkeepOpening");
            let instance = GetModuleHandleW(std::ptr::null());
            let wc = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(opening_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: instance,
                hIcon: std::ptr::null_mut(),
                hCursor: std::ptr::null_mut(),
                hbrBackground: std::ptr::null_mut(),
                lpszMenuName: std::ptr::null(),
                lpszClassName: class.as_ptr(),
            };
            RegisterClassW(&wc); // registered already from an earlier opening: fine
            let handle = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                class.as_ptr(),
                std::ptr::null(),
                WS_POPUP,
                frame.x,
                frame.y,
                frame.width,
                frame.height,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            );
            if handle.is_null() {
                SelectObject(dc, old);
                DeleteObject(dib);
                DeleteDC(dc);
                return Err(Fail::other("No window for the opening."));
            }
            Ok(WindowOpening {
                plate,
                frame,
                dc: dc as isize,
                dib: dib as isize,
                old: old as isize,
                bits: bits as *mut u32,
                canvas: Picture::new(frame.width, frame.height),
                handle: handle as isize,
            })
        }
    }

    /// The window as it will look: its colour, rounded corners and hairline (when `radius` is above 0), and what
    /// `draw_client` puts in the client area (relative to the frame). Colours are 0xAARRGGBB, not premultiplied.
    /// PORT: GDI+'s anti-aliased FillPath/DrawPath are drawn here by each pixel's distance to the rounded edge.
    pub fn plate(size: Size, client: Rect, back: u32, line: u32, radius: i32, draw_client: &dyn Fn(&mut Picture, Rect)) -> Picture {
        let mut plate = Picture::new(size.width, size.height);
        // Rounded(new RectangleF(.5f, .5f, size.Width - 1, size.Height - 1), radius).
        let (rx, ry, rw, rh) = (0.5f64, 0.5f64, (size.width - 1) as f64, (size.height - 1) as f64);
        let (hx, hy) = (rw / 2.0, rh / 2.0);
        let (cx, cy) = (rx + hx, ry + hy);
        let rad = if radius <= 0 { 0.0 } else { (radius as f64).min(hx).min(hy) };
        let back = premul(back);
        let line = premul(line);
        for y in 0..size.height {
            for x in 0..size.width {
                // The distance from the pixel's middle to the rounded rectangle's edge (below 0 inside).
                let (px, py) = (x as f64 + 0.5 - cx, y as f64 + 0.5 - cy);
                let (qx, qy) = (px.abs() - (hx - rad), py.abs() - (hy - rad));
                let d = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - rad;
                let fill = (0.5 - d).clamp(0.0, 1.0);
                let mut out = [back[0] * fill, back[1] * fill, back[2] * fill, back[3] * fill];
                if radius > 0 {
                    let stroke = (1.0 - d.abs()).clamp(0.0, 1.0);
                    let la = line[3] * stroke;
                    for k in 0..4 {
                        out[k] = line[k] * stroke + out[k] * (1.0 - la);
                    }
                }
                plate.set(x, y, pack(out));
            }
        }
        draw_client(&mut plate, client);
        plate
    }

    /// Size (1 = full) and opacity (0–1) at `ms` into the opening.
    pub fn pose(ms: f64) -> (f64, f64) {
        let t = (ms / GROW_MS).clamp(0.0, 1.0);
        // Grows past its size a touch and settles back (an ease-out with overshoot); solid by 60 % of the way.
        const C: f64 = 1.5;
        let back = 1.0 + (C + 1.0) * (t - 1.0).powi(3) + C * (t - 1.0).powi(2);
        let fade = (t / 0.6).min(1.0);
        (FROM + (1.0 - FROM) * back, 1.0 - (1.0 - fade).powi(3))
    }

    /// Draws the plate at `scale` around the middle of `target` (CompositingMode.SourceCopy over a cleared canvas,
    /// bilinear, pixels offset by half).
    pub fn frame(canvas: &mut Picture, plate: &Picture, target: Size, scale: f64) {
        canvas.fill(0);
        let w = plate.width as f64 * scale;
        let h = plate.height as f64 * scale;
        let (left, top) = ((target.width as f64 - w) / 2.0, (target.height as f64 - h) / 2.0);
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let x0 = left.floor().max(0.0) as i32;
        let x1 = ((left + w).ceil() as i32).min(canvas.width);
        let y0 = top.floor().max(0.0) as i32;
        let y1 = ((top + h).ceil() as i32).min(canvas.height);
        for y in y0..y1 {
            let v = (y as f64 + 0.5 - top) / h * plate.height as f64 - 0.5;
            for x in x0..x1 {
                let u = (x as f64 + 0.5 - left) / w * plate.width as f64 - 0.5;
                canvas.set(x, y, bilinear(plate, u, v));
            }
        }
    }

    /// Shows the stand-in just under `window` and plays the opening; done at full size. Blocking, on the thread that
    /// made the stand-in.
    pub fn grow(&mut self, window: isize) {
        // PORT: the WinForms Timer (Interval 1, really every ~15 ms on the window's thread) is a loop on this thread
        // that also hands the stand-in's own messages on.
        let clock = std::time::Instant::now();
        let mut shown = false;
        loop {
            let ms = clock.elapsed().as_secs_f64() * 1000.0;
            let (scale, alpha) = WindowOpening::pose(ms);
            WindowOpening::frame(&mut self.canvas, &self.plate, self.frame.size(), scale);
            // SAFETY: the DIB section holds frame.width × frame.height pixels, as the canvas does.
            unsafe {
                std::ptr::copy_nonoverlapping(self.canvas.pixels.as_ptr(), self.bits, self.canvas.pixels.len());
            }
            self.show((alpha * 255.0).round() as u8);
            if !shown {
                const SW_SHOWNOACTIVATE: i32 = 4;
                const SWP_NOSIZE: u32 = 0x1;
                const SWP_NOMOVE: u32 = 0x2;
                const SWP_NOACTIVATE: u32 = 0x10;
                // SAFETY: plain calls on our own window.
                unsafe {
                    ShowWindow(hwnd(self.handle), SW_SHOWNOACTIVATE);
                    SetWindowPos(hwnd(self.handle), hwnd(window), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE); // under the window
                }
                shown = true;
            }
            if ms >= GROW_MS {
                return;
            }
            // SAFETY: the stand-in's own messages only.
            unsafe {
                let mut msg: MSG = std::mem::zeroed();
                while PeekMessageW(&mut msg, hwnd(self.handle), 0, 0, PM_REMOVE) != 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn show(&self, alpha: u8) {
        const ULW_ALPHA: u32 = 2;
        let at = POINT { x: self.frame.x, y: self.frame.y };
        let size = SIZE { cx: self.frame.width, cy: self.frame.height };
        let from = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION { BlendOp: 0, BlendFlags: 0, SourceConstantAlpha: alpha, AlphaFormat: 1 }; // AC_SRC_OVER, AC_SRC_ALPHA
        // SAFETY: all arguments live on the stack; the DC holds the DIB section.
        unsafe {
            UpdateLayeredWindow(hwnd(self.handle), std::ptr::null_mut(), &at, &size, self.dc as HDC, &from, 0, &blend, ULW_ALPHA);
        }
    }
}

impl Drop for WindowOpening {
    fn drop(&mut self) {
        // SAFETY: each was made in `new` and is freed once.
        unsafe {
            DestroyWindow(hwnd(self.handle));
            SelectObject(self.dc as HDC, self.old as HGDIOBJ);
            DeleteObject(self.dib as HGDIOBJ);
            DeleteDC(self.dc as HDC);
        }
    }
}

/// A straight 0xAARRGGBB colour as premultiplied [r, g, b, a] in 0–1.
fn premul(argb: u32) -> [f64; 4] {
    let a = ((argb >> 24) & 0xFF) as f64 / 255.0;
    [((argb >> 16) & 0xFF) as f64 / 255.0 * a, ((argb >> 8) & 0xFF) as f64 / 255.0 * a, (argb & 0xFF) as f64 / 255.0 * a, a]
}

fn pack(c: [f64; 4]) -> u32 {
    let b = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    (b(c[3]) << 24) | (b(c[0]) << 16) | (b(c[1]) << 8) | b(c[2])
}

/// The plate at a point between pixels: the four around it weighed by distance, transparent outside.
fn bilinear(plate: &Picture, u: f64, v: f64) -> u32 {
    let (x0, y0) = (u.floor() as i32, v.floor() as i32);
    let (fx, fy) = (u - x0 as f64, v - y0 as f64);
    let at = |x: i32, y: i32| -> [f64; 4] {
        if x < 0 || y < 0 || x >= plate.width || y >= plate.height {
            return [0.0; 4];
        }
        let p = plate.get(x, y);
        [((p >> 16) & 0xFF) as f64, ((p >> 8) & 0xFF) as f64, (p & 0xFF) as f64, ((p >> 24) & 0xFF) as f64]
    };
    let (a, b, c, d) = (at(x0, y0), at(x0 + 1, y0), at(x0, y0 + 1), at(x0 + 1, y0 + 1));
    let mut out = [0f64; 4];
    for k in 0..4 {
        let top = a[k] * (1.0 - fx) + b[k] * fx;
        let bottom = c[k] * (1.0 - fx) + d[k] * fx;
        out[k] = (top * (1.0 - fy) + bottom * fy) / 255.0;
    }
    pack(out)
}
