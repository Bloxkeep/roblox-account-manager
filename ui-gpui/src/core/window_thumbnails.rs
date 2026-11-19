//! Live pictures of other programs' windows as DWM thumbnails (what the taskbar shows on hover), each laid exactly in a
//! rectangle of a window of ours: Windows draws every frame straight into that window, above its page, so nothing is
//! copied or polled. The game monitor's page draws everything else (frames, names, buttons, menus) and says where each
//! picture goes (`place`); this draws nothing itself. A minimised window has no live frame: its picture is
//! hidden, and `on_changed` tells so the page can say it.
//!
//! A thumbnail exists only while its window has a rectangle from the last `place`; every other path
//! unregisters it, so none leaks or points at a dead window.
use std::{
    collections::HashMap,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::Duration,
};

use windows_sys::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible},
};

use super::{
    log,
    win_kit::{Rect, Size, hwnd},
};

const DWM_TNP_RECTDESTINATION: u32 = 0x1;
const DWM_TNP_RECTSOURCE: u32 = 0x2;
const DWM_TNP_OPACITY: u32 = 0x4;
const DWM_TNP_VISIBLE: u32 = 0x8;
const DWM_TNP_SOURCECLIENTAREAONLY: u32 = 0x10;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct WinRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl WinRect {
    fn of(r: Rect) -> WinRect {
        WinRect { left: r.left(), top: r.top(), right: r.right(), bottom: r.bottom() }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct WinSize {
    cx: i32,
    cy: i32,
}

// BOOLs are 4-byte ints so the struct is 48 bytes, as dwmapi.h lays it out.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct DwmThumbnailProperties {
    dw_flags: u32,
    rc_destination: WinRect,
    rc_source: WinRect,
    opacity: u8,
    f_visible: i32,
    f_source_client_area_only: i32,
}

#[link(name = "dwmapi")]
unsafe extern "system" {
    fn DwmRegisterThumbnail(destination: HWND, source: HWND, thumbnail: *mut isize) -> i32;
    fn DwmUnregisterThumbnail(thumbnail: isize) -> i32;
    fn DwmUpdateThumbnailProperties(thumbnail: isize, properties: *const DwmThumbnailProperties) -> i32;
    fn DwmQueryThumbnailSourceSize(thumbnail: isize, size: *mut WinSize) -> i32;
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
fn source_size(thumb: isize) -> Option<WinSize> {
    let mut s = WinSize::default();
    // SAFETY: one out value on the stack.
    if unsafe { DwmQueryThumbnailSourceSize(thumb, &mut s) } == 0 { Some(s) } else { None }
}
fn update_props(thumb: isize, props: &DwmThumbnailProperties) -> i32 {
    // SAFETY: the properties live on the caller's stack for the call.
    unsafe { DwmUpdateThumbnailProperties(thumb, props) }
}

/// One window's picture. `thumb` is zero while no thumbnail is registered for it.
#[derive(Default)]
struct Picture {
    thumb: isize,
    /// Where the page wants it, in the destination window's client pixels.
    r#box: Rect,
    minimized: bool,
    /// The window's size when its picture was last placed (empty while unknown).
    shown: Size,
    /// A refused picture is logged once per window, not on every retry.
    refused_logged: bool,
}

struct Inner {
    host: isize,
    pictures: HashMap<isize, Picture>,
    /// The part of the host the pictures may show in (the page's monitor area: a picture scrolled half out is cut).
    clip: Rect,
    /// The watch's Enabled.
    watching: bool,
}

type ChangedFn = Box<dyn Fn() + Send + Sync>;

pub struct WindowThumbnails {
    inner: Arc<Mutex<Inner>>,
    /// A window was minimised or opened again: the page says so in its tile (the C# `Changed` event).
    pub on_changed: Arc<Mutex<Option<ChangedFn>>>,
    stop: Arc<AtomicBool>,
    /// Wakes the watch when watching starts or it is to end (it sleeps on `inner`'s lock meanwhile).
    wake: Arc<Condvar>,
    watch: Option<JoinHandle<()>>,
}

/// A picture shows the part of its window that had the window's size when placed: a window that grew (Arrange, brought
/// to the front) showed its top-left corner zoomed in until the next refresh (owner, 2026-11-02). So each window's
/// size and minimising are looked at this often, and the pictures placed again at once.
const WATCH_EVERY: Duration = Duration::from_millis(50);

impl WindowThumbnails {
    /// `host`: our window the pictures are drawn in.
    pub fn new(host: isize) -> WindowThumbnails {
        let inner = Arc::new(Mutex::new(Inner { host, pictures: HashMap::new(), clip: Rect::default(), watching: false }));
        let on_changed: Arc<Mutex<Option<ChangedFn>>> = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let wake = Arc::new(Condvar::new());
        // PORT: the WinForms Timer (50 ms, on the window's thread) is a thread with a stop flag; DWM's thumbnail calls
        // take handles and may come from any thread, and every look at the pictures holds their lock. Not watching
        // (another page shown), it sleeps until woken rather than 20 times a second.
        let (i, c, s, w) = (inner.clone(), on_changed.clone(), stop.clone(), wake.clone());
        // One round a look (threads::keep: one that panics doesn't end the watching for the rest of the run).
        let watch = crate::threads::keep("Bloxkeep thumbnails", move || {
            if s.load(Ordering::SeqCst) {
                return false;
            }
            {
                let mut g = i.lock().unwrap_or_else(|e| e.into_inner());
                while !g.watching && !s.load(Ordering::SeqCst) {
                    g = w.wait(g).unwrap_or_else(|e| e.into_inner());
                }
            }
            std::thread::sleep(WATCH_EVERY);
            let minimized = {
                let mut g = i.lock().unwrap_or_else(|e| e.into_inner());
                if !g.watching || s.load(Ordering::SeqCst) {
                    return true;
                }
                follow(&mut g)
            };
            if minimized {
                if let Some(f) = c.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
                    f();
                }
            }
            true
        })
        .ok();
        WindowThumbnails { inner, on_changed, stop, wake, watch }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A window has no live picture now (minimised or hidden), as of the last look.
    pub fn is_minimized(&self, h: isize) -> bool {
        match self.lock().pictures.get(&h) {
            Some(p) => p.minimized,
            None => is_iconic(h) || !is_visible(h),
        }
    }

    /// Each window's picture in its rectangle (the host's client pixels), cut to `clip`; a window not
    /// given loses its picture (scrolled away, under a menu, closed).
    pub fn place(&self, boxes: &HashMap<isize, Rect>, clip: Rect) {
        let mut g = self.lock();
        g.clip = clip;
        let gone: Vec<isize> = g.pictures.keys().filter(|h| !boxes.contains_key(h)).copied().collect();
        for h in gone {
            if let Some(mut p) = g.pictures.remove(&h) {
                unregister(&mut p);
            }
        }
        for (&h, &b) in boxes {
            let p = g.pictures.entry(h).or_insert_with(|| Picture { minimized: is_iconic(h) || !is_visible(h), ..Picture::default() });
            p.r#box = b;
        }
        update(&mut g);
        g.watching = !g.pictures.is_empty();
        if g.watching {
            self.wake.notify_all();
        }
    }

    /// Every picture taken away (the page shows something else there).
    pub fn clear(&self) {
        let mut g = self.lock();
        g.watching = false;
        for p in g.pictures.values_mut() {
            unregister(p);
        }
        g.pictures.clear();
    }

    /// The largest rectangle of the source's shape inside `b`, centred (a 16:9 game in a square box is letterboxed).
    pub fn fit(source: Size, b: Rect) -> Rect {
        if source.width <= 0 || source.height <= 0 || b.width <= 0 || b.height <= 0 {
            return b;
        }
        let scale = (b.width as f64 / source.width as f64).min(b.height as f64 / source.height as f64);
        let w = 1.max((source.width as f64 * scale) as i32);
        let h = 1.max((source.height as f64 * scale) as i32);
        Rect::new(b.x + (b.width - w) / 2, b.y + (b.height - h) / 2, w, h)
    }

    pub fn dispose(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        {
            // Under the lock: the watch is either before its check (and sees the flag) or asleep (and is woken).
            let _g = self.lock();
            self.wake.notify_all();
        }
        if let Some(t) = self.watch.take() {
            let _ = t.join();
        }
        self.clear();
    }
}

impl Drop for WindowThumbnails {
    fn drop(&mut self) {
        self.dispose();
    }
}

fn register(host: isize, h: isize, p: &mut Picture) {
    if p.thumb != 0 || host == 0 || !is_window(host) || !is_window(h) {
        return;
    }
    let mut thumb: isize = 0;
    // SAFETY: plain call; `thumb` receives the thumbnail's id.
    let hr = unsafe { DwmRegisterThumbnail(hwnd(host), hwnd(h), &mut thumb) };
    // Fails for a window that just closed or one of another desktop session: its tile then shows no picture.
    if hr != 0 || thumb == 0 {
        if !p.refused_logged && is_window(h) {
            let mut pid = 0u32;
            // SAFETY: one out value on the stack.
            unsafe {
                GetWindowThreadProcessId(hwnd(h), &mut pid);
            }
            log::note(&format!("Game monitor: Windows refused a live picture of Roblox window {pid} (0x{:08X}); its tile shows none.", hr as u32));
        }
        p.refused_logged = true;
        return;
    }
    p.thumb = thumb;
    // Until this is set the source size answers for the whole frame (title bar and borders), and the first
    // placing would fit the picture to the wrong shape.
    let props = DwmThumbnailProperties { dw_flags: DWM_TNP_SOURCECLIENTAREAONLY, f_source_client_area_only: 1, ..Default::default() };
    update_props(thumb, &props);
}

fn unregister(p: &mut Picture) {
    if p.thumb == 0 {
        return;
    }
    // Fails harmlessly once the destination window is gone: Windows dropped the thumbnail with it.
    // SAFETY: plain call with the id DwmRegisterThumbnail gave.
    unsafe {
        DwmUnregisterThumbnail(p.thumb);
    }
    p.thumb = 0;
}

/// Fits every picture into its box, hidden while its window is minimised or the box is out of the clip.
fn update(g: &mut Inner) {
    let (host, clip) = (g.host, g.clip);
    for (&h, p) in g.pictures.iter_mut() {
        if !is_window(h) {
            // Gone since the page's last report: never keep a thumbnail of a dead window.
            unregister(p);
            continue;
        }
        // A hidden window has no live frame either (Windows shows a grey placeholder for it).
        p.minimized = is_iconic(h) || !is_visible(h);
        if p.thumb == 0 {
            register(host, h, p);
        }
        if p.thumb == 0 {
            continue;
        }
        let mut props = DwmThumbnailProperties {
            dw_flags: DWM_TNP_RECTDESTINATION | DWM_TNP_VISIBLE | DWM_TNP_OPACITY | DWM_TNP_SOURCECLIENTAREAONLY,
            opacity: 255,
            f_source_client_area_only: 1,
            ..Default::default()
        };
        let size = source_size(p.thumb).filter(|s| s.cx > 0 && s.cy > 0);
        let sized = size.is_some();
        let source = size.map(|s| Size::new(s.cx, s.cy)).unwrap_or(Size::new(16, 9));
        p.shown = if sized { source } else { Size::default() };
        let picture = WindowThumbnails::fit(source, p.r#box);
        let visible = Rect::intersect(picture, clip);
        if p.minimized || p.r#box.width < 24 || p.r#box.height < 24 || visible.is_empty() {
            // Hidden, but still given a real rectangle: an empty one is refused as an argument.
            props.f_visible = 0;
            props.rc_destination = WinRect::of(Rect::new(p.r#box.x, p.r#box.y, 1, 1));
        } else {
            props.f_visible = 1;
            props.rc_destination = WinRect::of(visible);
            // The part of the window shown, given every time since Windows keeps the last one: a picture half scrolled
            // out shows the matching half, not a squashed whole. (Counted from the client area, as the picture is.)
            if sized {
                props.dw_flags |= DWM_TNP_RECTSOURCE;
                let sx = source.width as f64 / picture.width as f64;
                let sy = source.height as f64 / picture.height as f64;
                props.rc_source = if visible.size() == picture.size() {
                    WinRect::of(Rect::new(0, 0, source.width, source.height))
                } else {
                    WinRect::of(Rect::new(
                        ((visible.x - picture.x) as f64 * sx) as i32,
                        ((visible.y - picture.y) as f64 * sy) as i32,
                        1.max((visible.width as f64 * sx) as i32),
                        1.max((visible.height as f64 * sy) as i32),
                    ))
                };
            }
        }
        if update_props(p.thumb, &props) != 0 {
            unregister(p); // the window went away between the two calls; registered again next time if not
        }
    }
}

/// A window resized, minimised or opened again since its picture was placed: placed again now. True when one was
/// minimised or opened again (the caller then calls `on_changed`, outside the lock).
fn follow(g: &mut Inner) -> bool {
    let (mut moved, mut minimized) = (false, false);
    for (&h, p) in g.pictures.iter() {
        if !is_window(h) {
            continue;
        }
        let min = is_iconic(h) || !is_visible(h);
        if min != p.minimized {
            minimized = true;
            continue;
        }
        if min || p.thumb == 0 {
            continue;
        }
        let size = source_size(p.thumb).filter(|s| s.cx > 0 && s.cy > 0).map(|s| Size::new(s.cx, s.cy)).unwrap_or_default();
        if size != p.shown {
            moved = true;
        }
    }
    if moved || minimized {
        update(g);
    }
    minimized
}
