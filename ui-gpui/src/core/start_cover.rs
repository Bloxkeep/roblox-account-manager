// StartCover.cs: the window's start logo while the interface loads (shell_form::show_start_cover): still while the
// window opens (it grows in with it, window_opening), then it hops in place, squashing as it lands, until the interface is
// ready (owner, 2026-11-08: the moment before the password card should move too). Drawn here because nothing else can
// be yet. It comes off only between hops (`at_rest`), where it sits as the interface's own logo starts, so one logo moves
// all the way, without a jump.
//
// A child window filling the main window's client area (the C# Control docked to fill), on the windows thread.
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    time::Instant,
};

use futures::channel::oneshot;
use windows_sys::Win32::{
    Foundation::RECT,
    Graphics::Gdi::{BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BeginPaint, DIB_RGB_COLORS, EndPaint, PAINTSTRUCT, SetDIBitsToDevice},
    UI::WindowsAndMessaging::{GetClientRect, HWND_TOP, SWP_NOACTIVATE, SetWindowPos, WM_ERASEBKGND, WM_PAINT, WS_CHILD, WS_CLIPSIBLINGS, WS_VISIBLE},
};

use super::{
    base::Res,
    themes::Color,
    win_form::{self, Timer},
    win_kit::{Picture, Rect, hwnd},
};

const HOP_MS: f64 = 760.0;

/// One hop: (time 0–1, lift in logical px, width scale, height scale), standing on its bottom edge. The last stretch is
/// still: the moments it may come off.
const HOP: [(f64, f64, f64, f64); 7] = [(0.0, 0.0, 1.0, 1.0), (0.12, 0.0, 1.1, 0.88), (0.4, 18.0, 0.95, 1.06), (0.62, 0.0, 1.08, 0.92), (0.76, 4.0, 0.99, 1.01), (0.88, 0.0, 1.0, 1.0), (1.0, 0.0, 1.0, 1.0)];
const REST_FROM: f64 = 0.88;

pub struct StartCover {
    logo: Picture,
    unit: f32,
    back: Color,
    /// Runs from `start`.
    clock: Cell<Option<Instant>>,
    timer: Timer,
    rest: RefCell<Vec<oneshot::Sender<()>>>,
    handle: Cell<isize>,
}

impl StartCover {
    /// `logo`: the logo at its size on screen (72 logical px). `unit`: device pixels per logical pixel.
    pub fn new(logo: Picture, unit: f32, back: Color) -> Rc<StartCover> {
        let me = Rc::new(StartCover { logo, unit, back, clock: Cell::new(None), timer: Timer::new(15), rest: RefCell::new(Vec::new()), handle: Cell::new(0) });
        let w = Rc::downgrade(&me);
        me.timer.on_tick(move || {
            let Some(c) = w.upgrade() else { return };
            if c.clock.get().is_some() {
                win_form::invalidate(c.handle.get());
            }
            if !c.rest.borrow().is_empty() && c.at_rest_now() {
                for tx in c.rest.borrow_mut().drain(..) {
                    let _ = tx.send(());
                }
            }
        });
        me.timer.start();
        me
    }

    /// Controls.Add + BringToFront: a child filling `parent`'s client area, above its other children.
    pub fn attach(self: &Rc<Self>, parent: isize) -> Res<isize> {
        let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        // SAFETY: plain query.
        unsafe { GetClientRect(hwnd(parent), &mut r) };
        let weak: Weak<StartCover> = Rc::downgrade(self);
        let handler: win_form::Handler = Rc::new(move |h, msg, _wparam, _lparam| {
            let me = weak.upgrade()?;
            match msg {
                WM_ERASEBKGND => Some(1),
                WM_PAINT => {
                    me.paint(h);
                    Some(0)
                }
                _ => None,
            }
        });
        let h = win_form::create_window("Bloxkeep.StartCover", 0, WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS, 0, Rect::from_win(&r), parent, "", handler)?;
        // SAFETY: plain call on our own child.
        unsafe { SetWindowPos(hwnd(h), HWND_TOP, 0, 0, 0, 0, 0x1 | 0x2 | SWP_NOACTIVATE) };
        self.handle.set(h);
        Ok(h)
    }

    pub fn handle(&self) -> isize {
        self.handle.get()
    }

    /// The parent was resized (Dock = Fill, ResizeRedraw).
    pub fn fit(&self, parent: isize) {
        let h = self.handle.get();
        if h == 0 {
            return;
        }
        win_form::set_bounds(h, Rect::at(Default::default(), win_form::client_size(parent)));
        win_form::invalidate(h);
    }

    /// The window is open: the hops begin.
    pub fn start(&self) {
        if self.clock.get().is_none() {
            self.clock.set(Some(Instant::now()));
        }
    }

    /// Done once the logo stands still in the middle, as the interface's logo starts: between hops, a hop at most.
    pub async fn at_rest(&self) {
        if self.at_rest_now() {
            return;
        }
        let (tx, rx) = oneshot::channel();
        self.rest.borrow_mut().push(tx);
        let _ = rx.await;
    }

    fn elapsed_ms(&self) -> f64 {
        self.clock.get().map(|c| c.elapsed().as_secs_f64() * 1000.0).unwrap_or(0.0)
    }

    fn at_rest_now(&self) -> bool {
        self.clock.get().is_none() || self.elapsed_ms() % HOP_MS / HOP_MS >= REST_FROM
    }

    /// Lift and scale at `ms` since the cover went up.
    fn pose(ms: f64) -> (f64, f64, f64) {
        let p = ms % HOP_MS / HOP_MS;
        for i in 1..HOP.len() {
            if p > HOP[i].0 {
                continue;
            }
            let (a, b) = (HOP[i - 1], HOP[i]);
            let mut u = (p - a.0) / (b.0 - a.0);
            u = u * u * (3.0 - 2.0 * u); // eased at both ends
            return (a.1 + (b.1 - a.1) * u, a.2 + (b.2 - a.2) * u, a.3 + (b.3 - a.3) * u);
        }
        (0.0, 1.0, 1.0)
    }

    fn paint(&self, h: isize) {
        let size = win_form::client_size(h);
        let mut canvas = Picture::new(size.width.max(1), size.height.max(1));
        canvas.fill(0xFF00_0000 | (self.back.r as u32) << 16 | (self.back.g as u32) << 8 | self.back.b as u32);
        self.draw_logo(&mut canvas, Rect::new(0, 0, size.width, size.height));
        // SAFETY: BeginPaint/EndPaint pair; the DIB header describes `canvas`' pixels, read during the call.
        unsafe {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let dc = BeginPaint(hwnd(h), &mut ps);
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: canvas.width,
                biHeight: -canvas.height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..std::mem::zeroed()
            };
            SetDIBitsToDevice(dc, 0, 0, canvas.width as u32, canvas.height as u32, 0, 0, 0, canvas.height as u32, canvas.pixels.as_ptr() as *const _, &info, DIB_RGB_COLORS);
            EndPaint(hwnd(h), &ps);
        }
    }

    /// The logo as it is now, in the middle of `area` (also the window's stand-in, window_opening).
    /// PORT: GDI+'s HighQualityBicubic DrawImage is a bilinear scale drawn over the canvas (source-over, premultiplied).
    pub fn draw_logo(&self, g: &mut Picture, area: Rect) {
        let (lift, sx, sy) = StartCover::pose(self.elapsed_ms());
        let w = (self.logo.width as f64 * sx) as f32;
        let h = (self.logo.height as f64 * sy) as f32;
        // Centred as the still logo was, standing on its bottom edge: a squash stays on the ground.
        let bottom = area.top() as f32 + (area.height + self.logo.height) as f32 / 2.0 - (lift * self.unit as f64) as f32;
        let left = area.left() as f32 + (area.width as f32 - w) / 2.0;
        draw_scaled(g, &self.logo, left, bottom - h, w, h);
    }

    pub fn dispose(&self) {
        self.timer.dispose();
        for tx in self.rest.borrow_mut().drain(..) {
            let _ = tx.send(());
        }
        let h = self.handle.replace(0);
        win_form::destroy(h);
    }
}

/// `src` stretched into (x, y, w, h) of `dst`, bilinear, over what is there.
fn draw_scaled(dst: &mut Picture, src: &Picture, x: f32, y: f32, w: f32, h: f32) {
    if w <= 0.0 || h <= 0.0 || src.width <= 0 || src.height <= 0 {
        return;
    }
    let (x0, y0) = (x.floor().max(0.0) as i32, y.floor().max(0.0) as i32);
    let (x1, y1) = (((x + w).ceil() as i32).min(dst.width), ((y + h).ceil() as i32).min(dst.height));
    let sample = |sx: f32, sy: f32| -> [f32; 4] {
        let fx = (sx - 0.5).clamp(0.0, (src.width - 1) as f32);
        let fy = (sy - 0.5).clamp(0.0, (src.height - 1) as f32);
        let (ix, iy) = (fx as i32, fy as i32);
        let (nx, ny) = ((ix + 1).min(src.width - 1), (iy + 1).min(src.height - 1));
        let (tx, ty) = (fx - ix as f32, fy - iy as f32);
        let ch = |p: u32| [(p >> 24) as f32, ((p >> 16) & 255) as f32, ((p >> 8) & 255) as f32, (p & 255) as f32];
        let (a, b, c, d) = (ch(src.get(ix, iy)), ch(src.get(nx, iy)), ch(src.get(ix, ny)), ch(src.get(nx, ny)));
        let mut out = [0f32; 4];
        for k in 0..4 {
            out[k] = (a[k] * (1.0 - tx) + b[k] * tx) * (1.0 - ty) + (c[k] * (1.0 - tx) + d[k] * tx) * ty;
        }
        out
    };
    for py in y0..y1 {
        for px in x0..x1 {
            let (cx, cy) = (px as f32 + 0.5, py as f32 + 0.5);
            if cx < x || cx >= x + w || cy < y || cy >= y + h {
                continue;
            }
            let s = sample((cx - x) / w * src.width as f32, (cy - y) / h * src.height as f32);
            let d = dst.get(px, py);
            let inv = 1.0 - s[0] / 255.0;
            let blend = |sv: f32, dv: u32| (sv + dv as f32 * inv).round().clamp(0.0, 255.0) as u32;
            let out = blend(s[0], d >> 24) << 24 | blend(s[1], (d >> 16) & 255) << 16 | blend(s[2], (d >> 8) & 255) << 8 | blend(s[3], d & 255);
            dst.set(px, py, out);
        }
    }
}
