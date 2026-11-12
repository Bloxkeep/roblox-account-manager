// What a macro sees in a window's picture: a cut-out image found again (normalised cross-correlation on grey levels,
// a coarse pass over a shrunk picture, then the best few places looked at closely), a colour at a point, and whether
// the picture has stopped changing. Text is macro_ocr.rs's. Pictures are the window's client area at full size.
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

use crate::core::win_kit::{Picture as Bitmap, Rect as Rectangle};
use crate::core::log;
use crate::core::macro_model::{MacroRect, net};

/// A grey picture: one byte a pixel, row after row.
#[derive(Clone, Debug, Default)]
pub struct Gray {
    pub w: i32,
    pub h: i32,
    pub p: Vec<u8>,
}

impl Gray {
    pub fn new(w: i32, h: i32, pixels: Vec<u8>) -> Gray {
        Gray { w, h, p: pixels }
    }
    /// `this[x, y]`.
    pub fn at(&self, x: i32, y: i32) -> u8 {
        self.p[(y * self.w + x) as usize]
    }
}

fn rect(x: i32, y: i32, width: i32, height: i32) -> Rectangle {
    Rectangle { x, y, width, height }
}

/// Rectangle.FromLTRB.
fn from_ltrb(l: i32, t: i32, r: i32, b: i32) -> Rectangle {
    rect(l, t, r - l, b - t)
}

/// Rectangle.Intersect: Empty (all 0) when they don't meet.
pub(crate) fn intersect(a: Rectangle, b: Rectangle) -> Rectangle {
    let x1 = a.x.max(b.x);
    let x2 = (a.x + a.width).min(b.x + b.width);
    let y1 = a.y.max(b.y);
    let y2 = (a.y + a.height).min(b.y + b.height);
    if x2 >= x1 && y2 >= y1 { rect(x1, y1, x2 - x1, y2 - y1) } else { rect(0, 0, 0, 0) }
}

/// The picture in grey; `area` (pixels) cuts it first.
pub fn to_gray(bitmap: &Bitmap, area: Option<Rectangle>) -> Gray {
    let whole = rect(0, 0, bitmap.width, bitmap.height);
    let r = intersect(area.unwrap_or(whole), whole);
    if r.width <= 0 || r.height <= 0 {
        return Gray::new(0, 0, Vec::new());
    }
    let mut pixels = vec![0u8; (r.width * r.height) as usize];
    for y in 0..r.height {
        let o = y * r.width;
        for x in 0..r.width {
            // LockBits(Format32bppArgb): B, G, R, A.
            let argb = bitmap.get(r.x + x, r.y + y);
            let (red, green, blue) = ((argb >> 16 & 0xFF) as i32, (argb >> 8 & 0xFF) as i32, (argb & 0xFF) as i32);
            pixels[(o + x) as usize] = ((red * 77 + green * 150 + blue * 29) >> 8) as u8;
        }
    }
    Gray::new(r.width, r.height, pixels)
}

/// An area (0..1 of the client) in pixels of a `w` × `h` picture; all of it when null.
pub fn area(r: Option<&MacroRect>, w: i32, h: i32) -> Rectangle {
    let Some(r) = r else {
        return rect(0, 0, w, h);
    };
    let x = ((r.x * w as f64).floor() as i32).clamp(0, w);
    let y = ((r.y * h as f64).floor() as i32).clamp(0, h);
    let right = (((r.x + r.w) * w as f64).ceil() as i32).clamp(x, w);
    let bottom = (((r.y + r.h) * h as f64).ceil() as i32).clamp(y, h);
    from_ltrb(x, y, right, bottom)
}

/// Pixels back to 0..1 of a `w` × `h` client.
pub fn fraction(r: Rectangle, w: i32, h: i32) -> MacroRect {
    MacroRect { x: r.x as f64 / w as f64, y: r.y as f64 / h as f64, w: r.width as f64 / w as f64, h: r.height as f64 / h as f64 }
}

/// Where `template` best matches in `area` of `picture`, and how well
/// (1 = the same, under 0.5 = not there); null when it doesn't fit in the area.
pub fn find(picture: &Gray, template: &Gray, area: Rectangle) -> Option<(f64, Rectangle)> {
    let area = intersect(area, rect(0, 0, picture.w, picture.h));
    if template.w < 1 || template.h < 1 || area.width < template.w || area.height < template.h {
        return None;
    }
    // Coarse pass: shrunk so the template is about 12 px on its short side, and the work stays bounded.
    let mut step = 1.max(template.w.min(template.h) / 12);
    while step < 16 && (area.width / step) as f64 * (area.height / step) as f64 * (template.w / step) as f64 * (template.h / step) as f64 > 30e6 {
        step += 1;
    }
    let cut = Rc::new(crop(picture, area));
    let template_rc = Rc::new(template.clone());
    // The picture and template at each size looked at, the coarsest first, halving down to full size.
    let mut levels: HashMap<i32, (Rc<Gray>, Rc<Gray>)> = HashMap::new();
    let mut level = |s: i32| -> (Rc<Gray>, Rc<Gray>) {
        levels
            .entry(s)
            .or_insert_with(|| {
                let shrink_rc = |g: &Rc<Gray>| if s <= 1 { g.clone() } else { Rc::new(shrink(g, s)) };
                (shrink_rc(&cut), shrink_rc(&template_rc))
            })
            .clone()
    };
    let mut candidates: Vec<(i32, i32)> = Vec::new();
    let (hay, tpl) = level(step);
    if hay.w >= tpl.w && hay.h >= tpl.h && tpl.w > 0 && tpl.h > 0 {
        let mut scores = correlate(&hay, &tpl, rect(0, 0, hay.w - tpl.w + 1, hay.h - tpl.h + 1));
        let (keep, apart) = (6usize, 1.max(tpl.w.min(tpl.h) / 2));
        // OrderByDescending: stable, NaN last.
        scores.sort_by(|a, b| net_cmp(b.0, a.0));
        for (_, x, y) in scores {
            if candidates.len() >= keep {
                break;
            }
            if candidates.iter().all(|c| (c.0 - x).abs() >= apart || (c.1 - y).abs() >= apart) {
                candidates.push((x, y));
            }
        }
    } else {
        step = 1;
        candidates.push((0, 0));
    }
    // Each followed down the sizes, looking a little around where it was at the size before.
    let mut best = f64::NEG_INFINITY;
    let mut at = (0, 0);
    for (cx, cy) in candidates {
        let (mut x, mut y, mut s) = (cx, cy, step);
        let mut score;
        loop {
            let next = if s == step && step == 1 { 1 } else { 1.max(s / 2) };
            let (h, t) = level(next);
            let reach = if next == 1 && s == 1 { 3 } else { 2 };
            let (x0, y0) = (x * s / next, y * s / next);
            let near = intersect(from_ltrb(x0 - reach, y0 - reach, x0 + reach + 1, y0 + reach + 1), rect(0, 0, h.w - t.w + 1, h.h - t.h + 1));
            score = f64::NEG_INFINITY;
            for (sc, px, py) in correlate(&h, &t, near) {
                if sc > score {
                    (score, x, y) = (sc, px, py);
                }
            }
            s = next;
            if s <= 1 {
                break;
            }
        }
        if score > best {
            (best, at) = (score, (area.x + x, area.y + y));
        }
    }
    if best == f64::NEG_INFINITY { None } else { Some((best.clamp(-1.0, 1.0), rect(at.0, at.1, template.w, template.h))) }
}

/// double.CompareTo: NaN below everything.
fn net_cmp(a: f64, b: f64) -> std::cmp::Ordering {
    match (a.is_nan(), b.is_nan()) {
        (true, true) => std::cmp::Ordering::Equal,
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal),
    }
}

/// Normalised cross-correlation of `t` at each top-left in `places`. A flat
/// template (one colour) is scored by how close and how flat the picture is there instead.
fn correlate(p: &Gray, t: &Gray, places: Rectangle) -> Vec<(f64, i32, i32)> {
    let mut result = Vec::with_capacity(0.max(places.width * places.height) as usize);
    let n = t.w * t.h;
    let (mut sum_t, mut sum_t2): (i64, i64) = (0, 0);
    for &v in &t.p {
        sum_t += v as i64;
        sum_t2 += v as i64 * v as i64;
    }
    let nf = n as f64;
    let mean_t = sum_t as f64 / nf;
    let var_t = sum_t2 as f64 - sum_t as f64 * sum_t as f64 / nf;
    let flat = var_t < nf * 4.0; // under 2 grey levels of spread
    for y in places.y..places.y + places.height {
        for x in places.x..places.x + places.width {
            let (mut sum_i, mut sum_i2, mut cross): (i64, i64, i64) = (0, 0, 0);
            for ty in 0..t.h {
                let po = ((y + ty) * p.w + x) as usize;
                let to = (ty * t.w) as usize;
                for tx in 0..t.w as usize {
                    let i = p.p[po + tx] as i64;
                    sum_i += i;
                    sum_i2 += i * i;
                    cross += i * t.p[to + tx] as i64;
                }
            }
            let var_i = sum_i2 as f64 - sum_i as f64 * sum_i as f64 / nf;
            let score = if flat {
                1.0 - ((sum_i as f64 / nf - mean_t).abs() + (var_i.max(0.0) / nf).sqrt()) / 255.0
            } else if var_i < nf * 4.0 {
                0.0
            } else {
                (cross as f64 - sum_i as f64 * mean_t) / (var_i * var_t).sqrt()
            };
            result.push((score, x, y));
        }
    }
    result
}

pub fn crop(g: &Gray, r: Rectangle) -> Gray {
    let r = intersect(r, rect(0, 0, g.w, g.h));
    if r.x == 0 && r.y == 0 && r.width == g.w && r.height == g.h {
        return g.clone();
    }
    let mut pixels = vec![0u8; (r.width * r.height) as usize];
    for y in 0..r.height {
        let from = ((r.y + y) * g.w + r.x) as usize;
        let to = (y * r.width) as usize;
        pixels[to..to + r.width as usize].copy_from_slice(&g.p[from..from + r.width as usize]);
    }
    Gray::new(r.width, r.height, pixels)
}

/// Each `step` × `step` block averaged into one pixel.
pub fn shrink(g: &Gray, step: i32) -> Gray {
    if step <= 1 {
        return g.clone();
    }
    let (w, h) = (g.w / step, g.h / step);
    let mut pixels = vec![0u8; 0.max(w * h) as usize];
    let area = step * step;
    for y in 0..h {
        for x in 0..w {
            let mut sum = 0i32;
            for dy in 0..step {
                let o = (y * step + dy) * g.w + x * step;
                for dx in 0..step {
                    sum += g.p[(o + dx) as usize] as i32;
                }
            }
            pixels[(y * w + x) as usize] = (sum / area) as u8;
        }
    }
    Gray::new(w, h, pixels)
}

/// Bilinear, to any size: a template cut from a window of another size, scaled to this one.
pub fn resize(g: &Gray, w: i32, h: i32) -> Gray {
    if w == g.w && h == g.h {
        return g.clone();
    }
    let (w, h) = (w.max(1), h.max(1));
    let mut pixels = vec![0u8; (w * h) as usize];
    for y in 0..h {
        // Math.Clamp(v, 0, g.H - 1).
        let sy = ((y as f64 + 0.5) * g.h as f64 / h as f64 - 0.5).max(0.0).min((g.h - 1) as f64);
        let y0 = sy as i32;
        let y1 = (y0 + 1).min(g.h - 1);
        let fy = sy - y0 as f64;
        for x in 0..w {
            let sx = ((x as f64 + 0.5) * g.w as f64 / w as f64 - 0.5).max(0.0).min((g.w - 1) as f64);
            let x0 = sx as i32;
            let x1 = (x0 + 1).min(g.w - 1);
            let fx = sx - x0 as f64;
            let top = g.at(x0, y0) as f64 * (1.0 - fx) + g.at(x1, y0) as f64 * fx;
            let bottom = g.at(x0, y1) as f64 * (1.0 - fx) + g.at(x1, y1) as f64 * fx;
            // Math.Round: halves to even.
            pixels[(y * w + x) as usize] = (top * (1.0 - fy) + bottom * fy).round_ties_even() as u8;
        }
    }
    Gray::new(w, h, pixels)
}

static TEMPLATES: LazyLock<Mutex<HashMap<String, Arc<Gray>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
/// error.log: the length of the last picture that couldn't be read.
static BAD_TEMPLATE: AtomicI32 = AtomicI32::new(-1);

/// A step's picture (PNG, base64) in grey, scaled by `scale` (the window's width over the
/// one it was cut from); null when it can't be read.
pub fn template(base64: &str, scale: f64) -> Option<Arc<Gray>> {
    let cached = TEMPLATES.lock().unwrap_or_else(|e| e.into_inner()).get(base64).cloned();
    let gray = match cached {
        Some(g) => g,
        None => {
            // new Bitmap(new MemoryStream(Convert.FromBase64String(base64))): FormatException for bad base64,
            // ArgumentException (or another ExternalException) when GDI+ can't read the picture.
            let decoded = match net::from_base64(base64) {
                None => Err("FormatException"),
                Some(bytes) => gdip::decode(&bytes).map_err(|status| if status == 2 { "ArgumentException" } else { "ExternalException" }),
            };
            let bitmap = match decoded {
                Ok(b) => b,
                Err(kind) => {
                    let length = net::len16(base64) as i32;
                    // Not cached, so looked at again each look: written once per picture in a row.
                    if BAD_TEMPLATE.swap(length, Ordering::SeqCst) != length {
                        log::note(&format!("Macro: a step's picture ({length} characters) can't be read ({kind}); it is never found."));
                    }
                    return None;
                }
            };
            let gray = Arc::new(to_gray(&bitmap, None));
            let mut templates = TEMPLATES.lock().unwrap_or_else(|e| e.into_inner());
            if templates.len() > 64 {
                templates.clear();
            }
            templates.insert(base64.to_string(), gray.clone());
            gray
        }
    };
    // A few percent off is left as it is: resampling blurs more than it helps.
    if (scale - 1.0).abs() < 0.03 {
        Some(gray)
    } else {
        Some(Arc::new(resize(&gray, (gray.w as f64 * scale).round_ties_even() as i32, (gray.h as f64 * scale).round_ties_even() as i32)))
    }
}

/// The colour at a point (0..1 of the picture) is within `tolerance` of `colour`
/// ("#rrggbb") on every channel.
pub fn colour_at(picture: &Bitmap, x: f64, y: f64, colour: &str, tolerance: i32) -> bool {
    let (w, h) = (picture.width, picture.height);
    let c = picture.get(((x * w as f64) as i32).min(w - 1).max(0), ((y * h as f64) as i32).min(h - 1).max(0));
    // Convert.ToInt32(colour[1..], 16): the colour is checked as "#rrggbb" before it gets here.
    let Ok(want) = i32::from_str_radix(colour.get(1..).unwrap_or(""), 16) else {
        return false;
    };
    let (r, g, b) = ((c >> 16 & 0xFF) as i32, (c >> 8 & 0xFF) as i32, (c & 0xFF) as i32);
    (r - (want >> 16 & 0xFF)).abs() <= tolerance && (g - (want >> 8 & 0xFF)).abs() <= tolerance && (b - (want & 0xFF)).abs() <= tolerance
}

/// A small grey copy of an area, to tell whether the picture is still changing.
pub fn signature(picture: &Gray, area: Rectangle) -> Gray {
    let cut = crop(picture, area);
    let step = 1.max(cut.w / 64);
    shrink(&cut, step)
}

/// The mean difference per pixel of two signatures (0..255); 255 when their sizes differ (the window changed size).
pub fn difference(a: &Gray, b: &Gray) -> f64 {
    if a.w != b.w || a.h != b.h || a.p.is_empty() {
        return 255.0;
    }
    let sum: i64 = a.p.iter().zip(&b.p).map(|(x, y)| (*x as i64 - *y as i64).abs()).sum();
    sum as f64 / a.p.len() as f64
}

/// A signature this close to the last one counts as unchanged (a blinking cursor, video noise).
pub const STILL_DIFFERENCE: f64 = 1.5;

/// GDI+ (System.Drawing's own engine) for what the C# had it do: read a PNG (any picture GDI+ reads) and draw part of
/// a picture at another size (HighQualityBicubic), so both come out as they did.
pub(crate) mod gdip {
    use std::ffi::c_void;
    use std::sync::OnceLock;

    use windows_sys::Win32::Graphics::GdiPlus::{
        BitmapData, GdipBitmapLockBits, GdipBitmapUnlockBits, GdipCreateBitmapFromScan0, GdipCreateBitmapFromStream, GdipDeleteGraphics, GdipDisposeImage,
        GdipDrawImageRectRectI, GdipGetImageGraphicsContext, GdipGetImageHeight, GdipGetImageWidth, GdipSetInterpolationMode, GdiplusStartup, GdiplusStartupInput,
        GpBitmap, GpGraphics, GpImage, Rect,
    };
    use windows_sys::Win32::UI::Shell::SHCreateMemStream;

    use crate::core::win_kit::{Picture as Bitmap, Rect as Rectangle};

    /// PixelFormat32bppARGB.
    const ARGB: i32 = 0x0026_200A;
    const LOCK_READ: u32 = 1;
    const INTERPOLATION_HIGH_QUALITY_BICUBIC: i32 = 7;
    const UNIT_PIXEL: i32 = 2;
    /// GDI+'s Status: Ok.
    const OK: i32 = 0;

    /// GdiplusStartup, once (System.Drawing does it once a process).
    fn start() -> bool {
        static STARTED: OnceLock<bool> = OnceLock::new();
        *STARTED.get_or_init(|| {
            let mut token = 0usize;
            // SAFETY: a zeroed GdiplusStartupInput is valid (no callback); version 1; token and output are ours.
            unsafe {
                let mut input: GdiplusStartupInput = std::mem::zeroed();
                input.GdiplusVersion = 1;
                GdiplusStartup(&mut token, &input, std::ptr::null_mut()) == OK
            }
        })
    }

    /// The whole picture's pixels (LockBits, Format32bppArgb), as ARGB values.
    fn read_pixels(bitmap: *mut GpBitmap, w: i32, h: i32) -> Result<Vec<u32>, i32> {
        let r = Rect { X: 0, Y: 0, Width: w, Height: h };
        // SAFETY: `bitmap` is a live GDI+ bitmap of w × h; the locked rows are read within Stride and unlocked after.
        unsafe {
            let mut data: BitmapData = std::mem::zeroed();
            let status = GdipBitmapLockBits(bitmap, &r, LOCK_READ, ARGB, &mut data);
            if status != OK {
                return Err(status);
            }
            let mut px = Vec::with_capacity(w.max(0) as usize * h.max(0) as usize);
            for y in 0..h {
                let row = (data.Scan0 as *const u8).offset(y as isize * data.Stride as isize) as *const u32;
                for x in 0..w {
                    px.push(std::ptr::read_unaligned(row.add(x as usize)));
                }
            }
            GdipBitmapUnlockBits(bitmap, &mut data);
            Ok(px)
        }
    }

    /// IUnknown::Release on a COM object.
    fn release(obj: *mut c_void) {
        if obj.is_null() {
            return;
        }
        // SAFETY: `obj` is a live COM object we hold one reference to; slot 2 of every COM table is Release.
        unsafe {
            let table = *(obj as *const *const usize);
            let release: unsafe extern "system" fn(*mut c_void) -> u32 = std::mem::transmute(*table.add(2));
            release(obj);
        }
    }

    /// new Bitmap(stream): a picture's bytes read by GDI+; Err(GDI+'s status) when it can't. Its pixels are GDI+'s
    /// Format32bppArgb values as LockBits gives them (straight alpha, as the C# read them), not premultiplied: the
    /// picture is only turned grey (`template`), as in the C#.
    pub(crate) fn decode(bytes: &[u8]) -> Result<Bitmap, i32> {
        if !start() {
            return Err(18); // GdiplusNotInitialized
        }
        // SAFETY: SHCreateMemStream copies `bytes`; the stream is released below, after the bitmap made from it.
        let stream = unsafe { SHCreateMemStream(bytes.as_ptr(), bytes.len() as u32) };
        if stream.is_null() {
            return Err(3); // OutOfMemory
        }
        let mut bitmap: *mut GpBitmap = std::ptr::null_mut();
        // SAFETY: `stream` is a live IStream; `bitmap` receives a new GDI+ bitmap, disposed below.
        let status = unsafe { GdipCreateBitmapFromStream(stream, &mut bitmap) };
        let result = if status != OK {
            Err(status)
        } else {
            let (mut w, mut h) = (0u32, 0u32);
            // SAFETY: `bitmap` is live; width and height are written to our locals.
            unsafe {
                GdipGetImageWidth(bitmap as *mut GpImage, &mut w);
                GdipGetImageHeight(bitmap as *mut GpImage, &mut h);
            }
            // A picture past any screen (an imported file made so): refused, as GDI+'s InvalidParameter, before its
            // w × h could overflow.
            if w == 0 || h == 0 || w > 16_384 || h > 16_384 {
                Err(2)
            } else {
                read_pixels(bitmap, w as i32, h as i32).map(|px| Bitmap { width: w as i32, height: h as i32, pixels: px })
            }
        };
        if !bitmap.is_null() {
            // SAFETY: the bitmap made above, not used after this.
            unsafe { GdipDisposeImage(bitmap as *mut GpImage) };
        }
        release(stream);
        result
    }

    /// `g.DrawImage(picture, new Rectangle(0, 0, w, h), area, GraphicsUnit.Pixel)` into a new w × h Format32bppArgb
    /// bitmap, InterpolationMode.HighQualityBicubic.
    pub(crate) fn draw_scaled(picture: &Bitmap, area: Rectangle, w: i32, h: i32) -> Result<Bitmap, i32> {
        if !start() {
            return Err(18);
        }
        let (pw, ph) = (picture.width, picture.height);
        let mut source_px: Vec<u32> = Vec::with_capacity((pw * ph) as usize);
        for y in 0..ph {
            for x in 0..pw {
                source_px.push(picture.get(x, y));
            }
        }
        let mut source: *mut GpBitmap = std::ptr::null_mut();
        let mut cut: *mut GpBitmap = std::ptr::null_mut();
        let mut graphics: *mut GpGraphics = std::ptr::null_mut();
        // SAFETY: `source` wraps `source_px` (alive until the end of this fn, disposed before); `cut` is GDI+'s own
        // memory; every object made here is deleted below.
        let result = unsafe {
            let mut status = GdipCreateBitmapFromScan0(pw, ph, pw * 4, ARGB, source_px.as_ptr() as *const u8, &mut source);
            if status == OK {
                status = GdipCreateBitmapFromScan0(w, h, 0, ARGB, std::ptr::null(), &mut cut);
            }
            if status == OK {
                status = GdipGetImageGraphicsContext(cut as *mut GpImage, &mut graphics);
            }
            if status == OK {
                status = GdipSetInterpolationMode(graphics, INTERPOLATION_HIGH_QUALITY_BICUBIC);
            }
            if status == OK {
                status = GdipDrawImageRectRectI(
                    graphics,
                    source as *mut GpImage,
                    0,
                    0,
                    w,
                    h,
                    area.x,
                    area.y,
                    area.width,
                    area.height,
                    UNIT_PIXEL,
                    std::ptr::null(),
                    0,
                    std::ptr::null_mut(),
                );
            }
            if !graphics.is_null() {
                GdipDeleteGraphics(graphics);
            }
            if status == OK { read_pixels(cut, w, h).map(|px| Bitmap { width: w, height: h, pixels: px }) } else { Err(status) }
        };
        // SAFETY: both bitmaps were made above and aren't used after this.
        unsafe {
            if !cut.is_null() {
                GdipDisposeImage(cut as *mut GpImage);
            }
            if !source.is_null() {
                GdipDisposeImage(source as *mut GpImage);
            }
        }
        drop(source_px);
        result
    }
}
