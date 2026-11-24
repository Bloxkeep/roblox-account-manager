// A game's gameplay clip (the video its Roblox page plays first), shown over and over on its tile while the mouse
// rests there (pages/clips.rs), with its sound (the owner, 2026-11-19: "some Roblox game demos have sound"); a button
// on the clip turns the sound off for every clip, kept between runs.
//
// Roblox keeps it as WebM (VP9, with Opus sound; Adopt Me's: 640Ã—360, 15 frames a second, 32 s, ~10 MB, measured
// 2026-11-13). GPUI has no video, so Windows' own Media Foundation decodes it (its WebM reader and VP9 decoder are part
// of Windows 10 and 11) on a thread of its own, a few frames ahead of the clock, and this view shows each frame when its
// time comes. The decoder only works while the view is drawn in a window that is seen: a moment after it goes (the
// mouse left the tile, or the window is minimized, hidden in the tray or covered) the clock stops and the decoder is let
// go, and a while later its last frame; the next draw in a seen window starts it again.
use std::{
    cell::Cell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender, TryRecvError},
    },
    time::{Duration, Instant},
};

use gpui_kit::{prelude::*, AnyWindowHandle, AsyncApp, Context, EventEmitter, ImageSource, ObjectFit, RenderImage, Subscription, WeakEntity, Window, div, img};

/// Frames decoded ahead of the one on screen: two (each up to 960 or 1600 pixels wide, 2-6 MB; four held 12 MB for
/// a clip, the owner's error.log 2026-11-20); a frame decodes in a few ms, far within one frame's time.
const AHEAD: usize = 2;
/// Not drawn for this long: the clock stops and the decoder is let go (the page went, or the window is hidden); drawn
/// again, the clip starts over.
const PAUSE: Duration = Duration::from_millis(800);
/// Not drawn for this long: the last frame is let go too.
const STOP: Duration = Duration::from_secs(20);
/// The widest frame kept: a bigger clip is scaled down by Windows while it decodes (the page shows it at most ~600 px).
const MAX_W: u32 = 960;
/// â€¦and for a video behind the whole window (a custom theme's): wider, as it fills the window.
const MAX_W_BEHIND: u32 = 1600;

/// Clips' sound on (ui-prefs "clipSound", on unless turned off).
static SOUND: std::sync::LazyLock<AtomicBool> =
    std::sync::LazyLock::new(|| AtomicBool::new(crate::prefs::get("clipSound").and_then(|v| v.as_bool()).unwrap_or(true)));
/// The clip whose sound is heard: the one started last (two playing at once never sound over each other).
static TURN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// How loud a clip is, of its own loudness: a preview under the mouse, not a film.
const LOUDNESS: f32 = 0.7;
/// Not drawn for this long: the clip's sound goes quiet at once (its frames stop only at PAUSE).
const HUSH_MS: u64 = 200;

pub fn sound_on() -> bool {
    SOUND.load(Ordering::Relaxed)
}

pub fn set_sound(on: bool) {
    SOUND.store(on, Ordering::Relaxed);
    crate::prefs::set("clipSound", serde_json::Value::Bool(on));
}

/// A clock for "drawn when" shared with the sound's thread (ms since the first call).
fn now_ms() -> u64 {
    static BASE: std::sync::LazyLock<Instant> = std::sync::LazyLock::new(Instant::now);
    BASE.elapsed().as_millis() as u64
}

thread_local! {
    /// The widest frame this decoder thread keeps (set as the thread starts).
    static WIDEST: std::cell::Cell<u32> = const { std::cell::Cell::new(MAX_W) };
}

/// One decoded picture: when it is due (in 100 ns, counting on across the loops), and its pixels in GPUI's order (BGRA).
struct Frame {
    at: i64,
    w: u32,
    h: u32,
    bgra: Vec<u8>,
    /// A live picture kept on the GPU instead (window_capture.rs â€º run_shared): the shared texture's handle, `bgra`
    /// empty; 0 for pixels.
    shared: isize,
    /// A frame decoded on the graphics card, left there (`bgra` empty): copied on the card into the clip's texture.
    card: Option<OnCard>,
}

/// A decoded frame on the graphics card (NV12): its texture (on GPUI's device) and slice, held with its sample so the
/// decoder doesn't write over it before it is converted.
#[cfg(all(windows, any(feature = "core", feature = "shot")))]
struct OnCard {
    texture: windows::Win32::Graphics::Direct3D11::ID3D11Texture2D,
    sub: u32,
    _sample: windows::Win32::Media::MediaFoundation::IMFSample,
}

// SAFETY: Direct3D 11 resources and Media Foundation samples are free-threaded (the device is guarded for use from many
// threads, gpui-pre-windows â€º directx_devices); the frame is only handed from the decoder's thread to the window's.
#[cfg(all(windows, any(feature = "core", feature = "shot")))]
unsafe impl Send for OnCard {}

#[cfg(not(all(windows, any(feature = "core", feature = "shot"))))]
struct OnCard;

/// The clip could not be played (no decoder on this PC, a damaged file): its place in the gallery goes.
pub struct Failed;

pub struct Video {
    path: PathBuf,
    frame: Option<Arc<RenderImage>>,
    /// When it was last drawn in a seen window: by its view, or where it lies at each frame (`in_place`).
    drawn: Rc<Cell<Instant>>,
    /// A game's clip: its texture painted where it lies at every frame the window draws (motion::here), under what lies
    /// over it in its tile (the sound button, the chosen one's ring), its part kept as drawn; each new frame into the
    /// texture asks only for the window to be shown again, not for the part to be made over (88 frames a second at
    /// 6 ms each for a 60 fps clip on Home, made over inside the app). A theme's video behind the page is drawn in the
    /// theme's own layer, as before.
    kept: bool,
    /// Drawn that way now (in the main window, its frames on the card), and the texture painted there.
    in_place: bool,
    on_card: Rc<Cell<Option<(isize, u32, u32)>>>,
    window: Option<AnyWindowHandle>,
    playing: bool,
    failed: bool,
    /// Its window's coming back into sight draws it again (a restored window needn't draw anything otherwise).
    watch: Option<Subscription>,
    widest: u32,
    /// Instead of a file, a window's live picture (window_capture.rs): Wallpaper Engine's wallpaper in its own window.
    live: Option<isize>,
    /// The live picture on the GPU (its shared texture's handle and size), drawn as GPUI draws a frame.
    shared: Option<(isize, u32, u32)>,
    /// A decoded clip's frames written over one another in one texture on the GPU (window_capture::SharedFrame), drawn as
    /// `shared` is; None before the first frame, and for good (`gpu_failed`) where it can't be made: pictures then.
    #[cfg(all(windows, any(feature = "core", feature = "shot")))]
    gpu: Option<crate::window_capture::SharedFrame>,
    gpu_failed: bool,
    /// A game's clip plays its sound (not a theme's video); whether the clip has any (its sound thread found some), and
    /// when it was last drawn (now_ms), for the sound to go quiet the moment it isn't.
    sound: bool,
    heard: Arc<AtomicBool>,
    seen: Arc<std::sync::atomic::AtomicU64>,
}

impl EventEmitter<Failed> for Video {}

impl Video {
    pub fn new(path: PathBuf, cx: &mut Context<Self>) -> Self {
        // The frame on screen is a texture in every window's atlas: given back with the view.
        cx.on_release(|v: &mut Video, cx| {
            if let Some(old) = v.frame.take() {
                cx.drop_image(old, None);
            }
        })
        .detach();
        Self {
            path,
            frame: None,
            drawn: Rc::new(Cell::new(Instant::now())),
            kept: true,
            in_place: false,
            on_card: Rc::new(Cell::new(None)),
            window: None,
            playing: false,
            failed: false,
            watch: None,
            widest: MAX_W,
            live: None,
            shared: None,
            #[cfg(all(windows, any(feature = "core", feature = "shot")))]
            gpu: None,
            gpu_failed: false,
            sound: true,
            heard: Arc::new(AtomicBool::new(false)),
            seen: Arc::new(std::sync::atomic::AtomicU64::new(now_ms())),
        }
    }

    /// The clip has sound to turn off or on (a hidden shot shows the button as if it had).
    pub fn has_sound(&self) -> bool {
        self.sound && (self.heard.load(Ordering::Relaxed) || cfg!(feature = "shot"))
    }

    /// The same for a video that fills the window behind the page (Settings â€º Appearance â€º Custom).
    pub fn behind(path: PathBuf, cx: &mut Context<Self>) -> Self {
        let mut v = Self::new(path, cx);
        v.widest = MAX_W_BEHIND;
        v.sound = false;
        v.kept = false;
        v
    }

    /// A clip shown without sound at a page's width (What's new's demos): drawn as a game's clip is, never heard.
    pub fn quiet(path: PathBuf, cx: &mut Context<Self>) -> Self {
        let mut v = Self::new(path, cx);
        v.sound = false;
        v
    }

    /// A window's live picture behind the page (Wallpaper Engine's, wallpaper_engine.rs), drawn as a video is.
    pub fn live(hwnd: isize, cx: &mut Context<Self>) -> Self {
        let mut v = Self::behind(PathBuf::new(), cx);
        v.live = Some(hwnd);
        // (Played at most this wide already: kept whole, not thinned to every other pixel.)
        v.widest = crate::wallpaper_engine::LIVE_W;
        v
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        if self.playing || self.failed {
            return;
        }
        let (tx, rx) = mpsc::sync_channel::<Frame>(AHEAD);
        let parts = parts(&self.path);
        // (The thread ends by itself once `rx` is gone: its next frame finds nobody to take it.)
        let widest = self.widest;
        let live = self.live;
        // Set when `play` ends, for a capture that is waiting for a frame that never comes (a still wallpaper: Windows
        // sends a frame only when the window changes), which would otherwise never find its receiver gone and would
        // keep its thread, its Direct3D device and its capture session for good, one more each time the window is
        // hidden and shown again.
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        // Its sound on a thread of its own, from the same files, started with the frames (both from the clip's start).
        #[cfg(all(windows, feature = "core"))]
        if self.sound && live.is_none() {
            let (parts, stop, heard, seen) = (parts.clone(), stop.clone(), self.heard.clone(), self.seen.clone());
            let turn = TURN.fetch_add(1, Ordering::Relaxed) + 1;
            crate::threads::run(move || sound(&parts, &stop, &heard, &seen, turn));
        }
        // On the shared pool (threads.rs), not a thread per clip: a game's clip starts on every mouse rest.
        crate::threads::run(move || {
            WIDEST.with(|w| w.set(widest));
            match live {
                #[cfg(all(windows, any(feature = "core", feature = "shot")))]
                Some(hwnd) => {
                    // On the GPU: each frame copied into a texture GPUI draws, only a word sent here. Through memory
                    // when this GPU can't share one (Windows' software GPU, an old driver) before any frame came.
                    let begun = Instant::now();
                    let mut sent = false;
                    let on_gpu = crate::window_capture::run_shared(hwnd, &stopped, |handle, w, h| {
                        sent = true;
                        tx.send(Frame { at: (begun.elapsed().as_nanos() / 100) as i64, w, h, bgra: Vec::new(), shared: handle, card: None }).is_ok()
                    });
                    match on_gpu {
                        Err(why) if !sent => {
                            crate::net::note_failed("Wallpaper Engine live picture on the GPU (drawn through memory instead)", "", &why);
                            if let Err(why) = crate::window_capture::run(hwnd, widest, &stopped, |at, w, h, bgra| tx.send(Frame { at, w, h, bgra, shared: 0, card: None }).is_ok()) {
                                crate::net::note_failed("Wallpaper Engine live picture", "", &why);
                            }
                        }
                        Err(why) => crate::net::note_failed("Wallpaper Engine live picture", "", &why),
                        Ok(()) => {}
                    }
                }
                _ => decode(&parts, &tx),
            }
        });
        self.playing = true;
        cx.spawn(async move |this, cx| play(this, rx, StopOnDrop(stop), cx).await).detach();
    }
}

/// Tells the capture thread to stop when `play` ends, whichever way it ends.
struct StopOnDrop(Arc<AtomicBool>);

impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

impl Video {
    /// A frame now in the texture on the card (`handle`): the view is told when it changes what it draws (the first
    /// frame, a new texture, not painted in place); otherwise the window to draw again, which keeps the app as it was
    /// drawn and paints the texture anew where it lies (motion::here).
    fn show_on_card(&mut self, handle: (isize, u32, u32), cx: &mut Context<Self>) -> Option<AnyWindowHandle> {
        let changed = self.shared != Some(handle);
        self.shared = Some(handle);
        self.on_card.set(Some(handle));
        if let Some(old) = self.frame.take() {
            cx.drop_image(old, None);
        }
        if changed || !self.in_place {
            cx.notify();
            return None;
        }
        self.window
    }
}

/// The window drawn again for a clip's new frame (Video::show_on_card).
fn redraw(window: Option<AnyWindowHandle>, cx: &mut AsyncApp) {
    if let Some(w) = window {
        let _ = cx.update(|cx| w.update(cx, |_, window, _| window.redraw()));
    }
}

/// Shows each frame when its time comes, and stops as `Video` says.
async fn play(this: WeakEntity<Video>, rx: mpsc::Receiver<Frame>, stop: StopOnDrop, cx: &mut AsyncApp) {
    let mut clock: Option<(Instant, i64)> = None;
    let mut waiting: Option<Frame> = None;
    let mut shown_any = false;
    // Both let go once the clip isn't drawn: the decoder ends at its next frame (a 1280x720 clip's holds ~19 MB and its
    // threads), the capture at once. The last frame stays on screen until STOP.
    let (mut rx, mut stop) = (Some(rx), Some(stop));
    loop {
        let Ok(idle) = this.update(cx, |v, _| v.drawn.get().elapsed()) else { return };
        if idle >= STOP {
            let _ = this.update(cx, |v, cx| {
                v.playing = false;
                v.shared = None;
                v.on_card.set(None);
                #[cfg(all(windows, any(feature = "core", feature = "shot")))]
                {
                    v.gpu = None;
                }
                if let Some(old) = v.frame.take() {
                    cx.drop_image(old, None);
                }
            });
            return;
        }
        if idle >= PAUSE {
            clock = None;
            if rx.take().is_some() {
                waiting = None;
                stop = None;
            }
            cx.background_executor().timer(Duration::from_millis(250)).await;
            continue;
        }
        let Some(frames) = rx.as_ref() else {
            // Drawn again after its decoder went: a new one starts the clip over (the last frame shown until its first).
            let _ = this.update(cx, |v, cx| {
                v.playing = false;
                v.start(cx);
            });
            drop(stop);
            return;
        };
        let frame = match waiting.take() {
            Some(f) => f,
            None => match frames.try_recv() {
                Ok(f) => f,
                Err(TryRecvError::Empty) => {
                    cx.background_executor().timer(Duration::from_millis(10)).await;
                    continue;
                }
                Err(TryRecvError::Disconnected) => {
                    let _ = this.update(cx, |v, cx| {
                        v.playing = false;
                        // The decoder loops until nobody takes its frames: ending while they are still taken means the
                        // file went or broke mid-way. Not started again (it failed again and again, a few seconds
                        // apart); a clip that showed frames keeps its last one.
                        v.failed = true;
                        if !shown_any {
                            cx.emit(Failed);
                        }
                        cx.notify();
                    });
                    return;
                }
            },
        };
        // (Its waits to each frame's time come on Windows' fine tick while it plays, not 15.6 ms late: frame_step.)
        let _ = cx.update(|cx| gpui_kit::fine_timers(cx));
        if frame.shared != 0 {
            // On the GPU: the texture already holds this frame; the window only needs drawing again.
            shown_any = true;
            let Ok(again) = this.update(cx, |v, cx| v.show_on_card((frame.shared, frame.w, frame.h), cx)) else { return };
            redraw(again, cx);
            continue;
        }
        let now = Instant::now();
        let (t0, a0) = *clock.get_or_insert((now, frame.at));
        let due = t0 + Duration::from_micros(((frame.at - a0).max(0) / 10) as u64);
        if due > now + Duration::from_millis(2) {
            // (In steps of at most a fifth of a second, so a pause is seen soon.)
            cx.background_executor().timer((due - now).min(Duration::from_millis(200))).await;
            waiting = Some(frame);
            continue;
        }
        if now.duration_since(due) > Duration::from_millis(250) {
            // Fell behind (the window was busy): the clock starts again from this frame rather than racing to catch up.
            clock = Some((now, frame.at));
        }
        // Decoded on the card: turned there into the clip's one texture by the card's video processor. Where it can't,
        // the decoder is told to go on by the processor (CARD_BROKEN), its next frames coming as pixels.
        #[cfg(all(windows, any(feature = "core", feature = "shot")))]
        if let Some(card) = frame.card.as_ref() {
            let put = this.update(cx, |v, cx| {
                if v.gpu.as_ref().is_none_or(|g| g.size() != (frame.w, frame.h) || g.stale()) {
                    v.gpu = crate::window_capture::SharedFrame::new(frame.w, frame.h).map_err(|why| card_broken(&why)).ok();
                }
                let g = v.gpu.as_ref()?;
                match g.convert_from(&card.texture, card.sub, frame.w, frame.h) {
                    Ok(true) => {}
                    Ok(false) => return Some(None),
                    Err(why) => {
                        card_broken(&why);
                        return None;
                    }
                }
                let handle = g.handle();
                Some(v.show_on_card(handle, cx))
            });
            match put {
                Ok(Some(again)) => {
                    shown_any = true;
                    redraw(again, cx);
                }
                Ok(None) => {}
                Err(_) => return,
            }
            continue;
        }
        // On the GPU: written over the last frame in the clip's one texture (no new picture each frame).
        #[cfg(all(windows, any(feature = "core", feature = "shot")))]
        {
            let put = this.update(cx, |v, cx| {
                if v.gpu_failed || v.live.is_some() {
                    return None;
                }
                if v.gpu.as_ref().is_none_or(|g| g.size() != (frame.w, frame.h) || g.stale()) {
                    match crate::window_capture::SharedFrame::new(frame.w, frame.h) {
                        Ok(g) => v.gpu = Some(g),
                        Err(why) => {
                            crate::net::note_failed("Video frames on the GPU (drawn as pictures instead)", "", &why);
                            v.gpu_failed = true;
                            v.gpu = None;
                            return None;
                        }
                    }
                }
                let g = v.gpu.as_ref()?;
                if !g.put(&frame.bgra) {
                    return Some(None);
                }
                let handle = g.handle();
                Some(v.show_on_card(handle, cx))
            });
            match put {
                Ok(Some(again)) => {
                    shown_any = true;
                    redraw(again, cx);
                    continue;
                }
                Ok(None) => {}
                Err(_) => return,
            }
        }
        let Some(pixels) = image::RgbaImage::from_raw(frame.w, frame.h, frame.bgra) else { continue };
        let shown = Arc::new(RenderImage::new(vec![image::Frame::new(pixels)]));
        gpui_kit::label_picture(shown.id, "video frame");
        shown_any = true;
        let alive = this.update(cx, |v, cx| {
            if let Some(old) = v.frame.replace(shown) {
                cx.drop_image(old, None);
            }
            cx.notify();
        });
        if alive.is_err() {
            return;
        }
    }
}

impl Render for Video {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.watch.is_none() {
            let me = cx.weak_entity();
            self.watch = Some(window.observe_window_visibility(move |_, window, cx| {
                if window.is_visible() {
                    let _ = me.update(cx, |_, cx| cx.notify());
                }
            }));
        }
        // (A minimized or covered window may still be drawn: that isn't watching. A hidden shot's window counts as seen.)
        self.window = Some(window.window_handle());
        if window.is_visible() || cfg!(feature = "shot") {
            self.drawn.set(Instant::now());
            self.seen.store(now_ms(), Ordering::Relaxed);
            if !self.playing && !self.failed {
                self.start(cx);
            }
        }
        // A game's clip in the main window: painted where it lies (see `kept`), from the texture it reads each time;
        // painted where nothing covers it (a dialog's backdrop over it, as the motion layer hid it), it is still seen.
        self.in_place = self.kept && self.shared.is_some() && crate::motion::is_main(window);
        #[cfg(windows)]
        if self.in_place {
            let (on_card, drawn, seen) = (self.on_card.clone(), self.drawn.clone(), self.seen.clone());
            return div()
                .size_full()
                .overflow_hidden()
                .child(
                    crate::motion::here(move |bounds, open, window, _| {
                        if open && (window.is_visible() || cfg!(feature = "shot")) {
                            drawn.set(Instant::now());
                            seen.store(now_ms(), Ordering::Relaxed);
                        }
                        if let Some((handle, w, h)) = on_card.get() {
                            paint_cover(window, bounds, handle, w, h);
                        }
                        false
                    })
                    .size_full(),
                )
                .into_any_element();
        }
        #[cfg(windows)]
        if let Some((handle, w, h)) = self.shared {
            // Filling the box as ObjectFit::Cover does, the overflow cut off.
            return div()
                .size_full()
                .overflow_hidden()
                .child(
                    gpui_kit::canvas(
                        |_, _, _| (),
                        move |bounds, _, window, _| paint_cover(window, bounds, handle, w, h),
                    )
                    .size_full(),
                )
                .into_any_element();
        }
        div().size_full().when_some(self.frame.clone(), |d, f| d.child(img(ImageSource::Render(f)).size_full().object_fit(ObjectFit::Cover))).into_any_element()
    }
}

/// The texture `handle` (`w`×`h`) filling `bounds` as ObjectFit::Cover does, the overflow cut off by the box's clip.
#[cfg(windows)]
fn paint_cover(window: &mut Window, bounds: gpui_kit::Bounds<gpui_kit::Pixels>, handle: isize, w: u32, h: u32) {
    let (bw, bh) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    let k = (bw / w.max(1) as f32).max(bh / h.max(1) as f32);
    let (sw, sh) = (w as f32 * k, h as f32 * k);
    let at = gpui_kit::point(bounds.origin.x + gpui_kit::px((bw - sw) / 2.), bounds.origin.y + gpui_kit::px((bh - sh) / 2.));
    window.with_content_mask(Some(gpui_kit::ContentMask { bounds }), |window| {
        window.paint_shared_texture(gpui_kit::Bounds { origin: at, size: gpui_kit::size(gpui_kit::px(sw), gpui_kit::px(sh)) }, handle, w, h)
    });
}

/// The frame size kept for a clip of `w`Ã—`h`: as it is, or scaled down to MAX_W wide; even, as the decoder wants.
fn fit(w: u32, h: u32) -> (u32, u32) {
    let (w, h) = (w.max(2), h.max(2));
    let most = WIDEST.with(|m| m.get());
    let (w, h) = if w > most { (most, ((h as u64 * most as u64) / w as u64) as u32) } else { (w, h) };
    ((w & !1).max(2), (h & !1).max(2))
}

/// A decoded frame (`sw`x`sh` RGB32, rows `pitch` bytes apart, the bottom row first when `flip`) as `w`x`h` BGRA no
/// bigger, fully opaque (RGB32's fourth byte is unused, not opacity). Each pixel is the average of the frame's pixels
/// it covers, so a smaller picture keeps its detail without shimmering.
#[cfg(all(windows, any(feature = "core", feature = "shot")))]
fn shrink(src: &[u8], pitch: usize, flip: bool, sw: u32, sh: u32, w: u32, h: u32) -> Vec<u8> {
    let (sw, sh, w, h) = (sw as usize, sh as usize, (w as usize).clamp(1, sw.max(1) as usize), (h as usize).clamp(1, sh.max(1) as usize));
    let row = |y: usize| {
        let y = if flip { sh - 1 - y } else { y };
        &src[y * pitch..y * pitch + sw * 4]
    };
    let mut out = vec![255u8; w * h * 4];
    if (w, h) == (sw, sh) {
        for (y, line) in out.chunks_exact_mut(w * 4).enumerate() {
            for (o, i) in line.chunks_exact_mut(4).zip(row(y).chunks_exact(4)) {
                o[..3].copy_from_slice(&i[..3]);
            }
        }
        return out;
    }
    // Each output column's first source column (the next one's first ends it).
    let cols: Vec<usize> = (0..=w).map(|x| x * sw / w).collect();
    let mut sums = vec![0u32; sw * 3];
    for (y, line) in out.chunks_exact_mut(w * 4).enumerate() {
        let (y0, y1) = (y * sh / h, ((y + 1) * sh / h).max(y * sh / h + 1));
        sums.fill(0);
        for sy in y0..y1 {
            for (s, p) in sums.chunks_exact_mut(3).zip(row(sy).chunks_exact(4)) {
                s[0] += p[0] as u32;
                s[1] += p[1] as u32;
                s[2] += p[2] as u32;
            }
        }
        for (x, o) in line.chunks_exact_mut(4).enumerate() {
            let (x0, x1) = (cols[x], cols[x + 1].max(cols[x] + 1));
            let n = ((x1 - x0) * (y1 - y0)) as u32;
            let mut c = [0u32; 3];
            for s in sums[x0 * 3..x1 * 3].chunks_exact(3) {
                c[0] += s[0];
                c[1] += s[1];
                c[2] += s[2];
            }
            o[0] = (c[0] / n) as u8;
            o[1] = (c[1] / n) as u8;
            o[2] = (c[2] / n) as u8;
        }
    }
    out
}

/// How one pass through the file ended.
enum End {
    /// At the end of the clip, its last frame at this time: it starts over after it.
    Again(i64),
    /// Nobody takes the frames any more, or the file can't be read.
    Stop,
}

/// The decoder's thread: the clip over and over, each pass's times following on from the last, until nobody takes its
/// frames.
#[cfg(all(windows, any(feature = "core", feature = "shot")))]
fn decode(parts: &[PathBuf], tx: &SyncSender<Frame>) {
    use windows::Win32::{
        System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize},
    };
    // SAFETY: plain calls into Windows; COM is started and stopped in pairs on this thread.
    unsafe {
        let com = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        if mf_started() {
            let mut card = on_card();
            let mut base = 0i64;
            // (A clip that ends before its first frame isn't started again and again.)
            'clip: loop {
                let start = base;
                for part in parts {
                    match pass(part, tx, base, &mut card) {
                        End::Again(next) if next > base => base = next,
                        End::Again(_) => {}
                        End::Stop => break 'clip,
                    }
                }
                if base <= start {
                    break;
                }
            }
        }
        if com {
            CoUninitialize();
        }
    }
}

#[cfg(not(all(windows, any(feature = "core", feature = "shot"))))]
fn decode(_: &[PathBuf], _: &SyncSender<Frame>) {}

/// Decoding on the card failed to start (said once a run): clips are decoded by the processor instead.
#[cfg(all(windows, any(feature = "core", feature = "shot")))]
fn note_card_failed(why: &str) {
    static SAID: AtomicBool = AtomicBool::new(false);
    if !SAID.swap(true, Ordering::Relaxed) {
        crate::net::note_failed("Game clip decoding on the graphics card (decoded by the processor instead)", "", why);
    }
}

/// The card couldn't show a frame decoded on it (no video processor for it, a lost device): the clips go on decoded by
/// the processor, for the rest of the run.
#[cfg(all(windows, any(feature = "core", feature = "shot")))]
static CARD_BROKEN: AtomicBool = AtomicBool::new(false);

#[cfg(all(windows, any(feature = "core", feature = "shot")))]
fn card_broken(why: &str) {
    CARD_BROKEN.store(true, Ordering::Relaxed);
    note_card_failed(why);
}

/// GPUI's device handed to Media Foundation, so a clip is decoded on the graphics card (DXVA) and its frames stay
/// there; None where the device has no video decoder (gpui-pre-windows makes it with one where it can), outside GPUI's
/// process (--clip-check), or when the manager can't be made: decoded by the processor then, as before.
#[cfg(all(windows, any(feature = "core", feature = "shot")))]
fn on_card() -> Option<windows::Win32::Media::MediaFoundation::IMFDXGIDeviceManager> {
    use windows::{
        Win32::{Graphics::Direct3D11::ID3D11VideoDevice, Media::MediaFoundation::MFCreateDXGIDeviceManager},
        core::Interface,
    };
    if CARD_BROKEN.load(Ordering::Relaxed) {
        return None;
    }
    let device = match crate::window_capture::SharedFrame::gpui_device() {
        Some(d) => d,
        None => check_device()?,
    };
    device.cast::<ID3D11VideoDevice>().ok()?;
    // SAFETY: plain Media Foundation calls with valid out-pointers; the device is GPUI's, guarded for many threads.
    unsafe {
        let (mut token, mut manager) = (0u32, None);
        MFCreateDXGIDeviceManager(&mut token, &mut manager).ok()?;
        let manager = manager?;
        manager.ResetDevice(&device, token).ok()?;
        Some(manager)
    }
}

/// --clip-check only, with BLOXKEEP_CLIP_CARD set: a device of its own with a video decoder, to measure decoding on the
/// card outside GPUI.
#[cfg(all(windows, any(feature = "core", feature = "shot")))]
fn check_device() -> Option<windows::Win32::Graphics::Direct3D11::ID3D11Device> {
    use windows::{
        Win32::{
            Foundation::HMODULE,
            Graphics::{
                Direct3D::D3D_DRIVER_TYPE_HARDWARE,
                Direct3D11::{D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_CREATE_DEVICE_VIDEO_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Multithread},
            },
        },
        core::Interface,
    };
    std::env::var("BLOXKEEP_CLIP_CARD").ok()?;
    let (mut device, mut context) = (None, None);
    // SAFETY: plain Direct3D calls with valid out-pointers.
    unsafe {
        D3D11CreateDevice(None, D3D_DRIVER_TYPE_HARDWARE, HMODULE::default(), D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT, None, D3D11_SDK_VERSION, Some(&mut device), None, Some(&mut context)).ok()?;
        let _ = context?.cast::<ID3D11Multithread>().ok()?.SetMultithreadProtected(true);
    }
    device
}

/// Media Foundation, started once for the app and never stopped: stopping it with each clip ended its work threads, and
/// the next clip made them again (NVIDIA's driver keeps ~184 bytes for every thread ever made, threads.rs).
#[cfg(all(windows, any(feature = "core", feature = "shot")))]
fn mf_started() -> bool {
    use windows::Win32::Media::MediaFoundation::{MF_VERSION, MFSTARTUP_LITE, MFStartup};
    static STARTED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    // SAFETY: a plain call into Windows, once.
    *STARTED.get_or_init(|| unsafe { MFStartup(MF_VERSION, MFSTARTUP_LITE).is_ok() })
}

/// The sound's thread: the clip's sound over and over, decoded by Media Foundation (Opus in Roblox's WebM) into the
/// default speakers through Windows' shared audio (it converts the rate), until `stop` (the frames stopped). Silence
/// is written instead while sound is off, the clip isn't drawn, or a newer clip started; a clip without sound ends
/// the thread at once.
#[cfg(all(windows, feature = "core"))]
fn sound(parts: &[PathBuf], stop: &AtomicBool, heard: &AtomicBool, seen: &std::sync::atomic::AtomicU64, turn: u64) {
    use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
    // SAFETY: COM is started and stopped in pairs on this thread; everything else is in `sound_passes`.
    unsafe {
        let com = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        if mf_started()
            && let Err(why) = sound_passes(parts, stop, heard, seen, turn)
        {
            crate::net::note_failed("Game clip sound", &parts.first().map(|p| p.to_string_lossy().to_string()).unwrap_or_default(), &format!("{why}"));
        }
        if com {
            CoUninitialize();
        }
    }
}

#[cfg(all(windows, feature = "core"))]
unsafe fn sound_passes(parts: &[PathBuf], stop: &AtomicBool, heard: &AtomicBool, seen: &std::sync::atomic::AtomicU64, turn: u64) -> windows::core::Result<()> {
    use windows::{
        Win32::{
            Media::{
                Audio::{
                    AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM, AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
                    IAudioClient, IAudioRenderClient, IMMDeviceEnumerator, MMDeviceEnumerator, WAVEFORMATEX, eConsole, eRender,
                },
                MediaFoundation::{
                    IMFSample, MF_MT_AUDIO_NUM_CHANNELS, MF_MT_AUDIO_SAMPLES_PER_SECOND, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE, MF_SOURCE_READER_ALL_STREAMS,
                    MF_SOURCE_READER_FIRST_AUDIO_STREAM, MF_SOURCE_READERF_ENDOFSTREAM, MF_SOURCE_READERF_ERROR, MFAudioFormat_Float, MFCreateMediaType,
                    MFCreateSourceReaderFromURL, MFMediaType_Audio,
                },
            },
            System::Com::{CLSCTX_ALL, CoCreateInstance},
        },
        core::HSTRING,
    };
    // The speakers' stream, made for the clip's channels and rate (made again only if a part differs).
    struct Out {
        client: IAudioClient,
        render: IAudioRenderClient,
        size: u32,
        channels: u32,
        rate: u32,
    }
    // SAFETY: Windows calls on objects made here; the buffers are written only within the frames GetBuffer gave, and
    // the decoded bytes read only between Lock and Unlock, no further than Lock's length.
    unsafe {
        let open = |channels: u32, rate: u32| -> windows::core::Result<Out> {
            let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let client: IAudioClient = devices.GetDefaultAudioEndpoint(eRender, eConsole)?.Activate(CLSCTX_ALL, None)?;
            let format = WAVEFORMATEX {
                // (WAVE_FORMAT_IEEE_FLOAT: 32-bit floats, as the decoder gives them.)
                wFormatTag: 3,
                nChannels: channels as u16,
                nSamplesPerSec: rate,
                nAvgBytesPerSec: rate * channels * 4,
                nBlockAlign: (channels * 4) as u16,
                wBitsPerSample: 32,
                cbSize: 0,
            };
            // A fifth of a second written ahead: little enough that Mute is heard at once.
            client.Initialize(AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, 2_000_000, 0, &format, None)?;
            let size = client.GetBufferSize()?;
            let render: IAudioRenderClient = client.GetService()?;
            client.Start()?;
            Ok(Out { client, render, size, channels, rate })
        };
        let mut out: Option<Out> = None;
        let audio = MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32;
        loop {
            let mut played = false;
            for part in parts {
                if stop.load(Ordering::Relaxed) {
                    return Ok(());
                }
                let reader = MFCreateSourceReaderFromURL(&HSTRING::from(part.as_os_str()), None)?;
                reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
                // No sound in this clip: nothing to play.
                if reader.SetStreamSelection(audio, true).is_err() {
                    return Ok(());
                }
                let want = MFCreateMediaType()?;
                want.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
                want.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_Float)?;
                reader.SetCurrentMediaType(audio, None, &want)?;
                let kind = reader.GetCurrentMediaType(audio)?;
                let (channels, rate) = (kind.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS)?.max(1), kind.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND)?);
                if out.as_ref().is_none_or(|o| o.channels != channels || o.rate != rate) {
                    // (The old stream let go before the new one is made.)
                    drop(out.take());
                    out = Some(open(channels, rate)?);
                }
                let Some(o) = out.as_ref() else { return Ok(()) };
                heard.store(true, Ordering::Relaxed);
                loop {
                    let (mut flags, mut sample) = (0u32, None::<IMFSample>);
                    reader.ReadSample(audio, 0, None, Some(&mut flags), None, Some(&mut sample))?;
                    if flags & MF_SOURCE_READERF_ERROR.0 as u32 != 0 {
                        return Ok(());
                    }
                    if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                        break;
                    }
                    let Some(sample) = sample else { continue };
                    let buffer = sample.ConvertToContiguousBuffer()?;
                    let (mut bytes, mut len) = (std::ptr::null_mut::<u8>(), 0u32);
                    buffer.Lock(&mut bytes, None, Some(&mut len))?;
                    let floats: Vec<f32> = if bytes.is_null() {
                        Vec::new()
                    } else {
                        std::slice::from_raw_parts(bytes as *const f32, len as usize / 4).to_vec()
                    };
                    let _ = buffer.Unlock();
                    let frames = floats.len() / channels as usize;
                    // (Counted for --clip-sound-check: what was decoded, and its loudest sample.)
                    SOUND_FRAMES.fetch_add(frames as u64, Ordering::Relaxed);
                    let peak = floats.iter().fold(0f32, |m, f| m.max(f.abs()));
                    SOUND_PEAK.fetch_max(peak.to_bits(), Ordering::Relaxed);
                    let mut done = 0usize;
                    while done < frames {
                        if stop.load(Ordering::Relaxed) {
                            return Ok(());
                        }
                        let room = (o.size - o.client.GetCurrentPadding()?) as usize;
                        if room == 0 {
                            std::thread::sleep(Duration::from_millis(8));
                            continue;
                        }
                        let n = room.min(frames - done);
                        let to = o.render.GetBuffer(n as u32)?;
                        let quiet = !SOUND.load(Ordering::Relaxed)
                            || TURN.load(Ordering::Relaxed) != turn
                            || now_ms().saturating_sub(seen.load(Ordering::Relaxed)) > HUSH_MS;
                        if quiet || to.is_null() {
                            o.render.ReleaseBuffer(n as u32, AUDCLNT_BUFFERFLAGS_SILENT.0 as u32)?;
                        } else {
                            let from = &floats[done * channels as usize..(done + n) * channels as usize];
                            let to = std::slice::from_raw_parts_mut(to as *mut f32, from.len());
                            for (t, f) in to.iter_mut().zip(from) {
                                *t = f * LOUDNESS;
                            }
                            o.render.ReleaseBuffer(n as u32, 0)?;
                        }
                        done += n;
                        played = true;
                    }
                }
            }
            if !played {
                return Ok(());
            }
        }
    }
}

/// One pass through the file: its frames, their times moved on by `base`.
#[cfg(all(windows, any(feature = "core", feature = "shot")))]
unsafe fn pass(path: &Path, tx: &SyncSender<Frame>, base: i64, card: &mut Option<windows::Win32::Media::MediaFoundation::IMFDXGIDeviceManager>) -> End {
    use windows::{
        Win32::Media::MediaFoundation::{
            IMFAttributes, IMFDXGIBuffer, IMFDXGIDeviceManager, IMFSample, IMFSourceReader, MF_MT_DEFAULT_STRIDE, MF_MT_FRAME_SIZE, MF_MT_MAJOR_TYPE,
            MF_MT_SUBTYPE, MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, MF_SOURCE_READER_ALL_STREAMS, MF_SOURCE_READER_D3D_MANAGER,
            MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, MF_SOURCE_READER_FIRST_VIDEO_STREAM,
            MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED, MF_SOURCE_READERF_ENDOFSTREAM, MF_SOURCE_READERF_ERROR, MFCreateAttributes,
            MFCreateMediaType, MFCreateSourceReaderFromURL, MFMediaType_Video, MFVideoFormat_NV12, MFVideoFormat_RGB32,
        },
        Win32::Graphics::Direct3D11::{D3D11_TEXTURE2D_DESC, ID3D11Texture2D},
        core::{HSTRING, Interface},
    };
    let video = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
    // The decoded frame's size and the bytes from one row to the next (below 0: the rows come bottom first).
    let layout = |reader: &IMFSourceReader| -> windows::core::Result<(u32, u32, i32)> {
        // SAFETY: reading the reader's current type; nothing is kept from it.
        unsafe {
            let kind = reader.GetCurrentMediaType(video)?;
            let size = kind.GetUINT64(&MF_MT_FRAME_SIZE)?;
            let (w, h) = ((size >> 32) as u32, size as u32);
            let stride = kind.GetUINT32(&MF_MT_DEFAULT_STRIDE).map(|s| s as i32).unwrap_or((w * 4) as i32);
            Ok((w, h, stride))
        }
    };
    // SAFETY: Media Foundation calls on objects made here; the buffer's bytes are read only between Lock and Unlock, and
    // no further than the length Lock gives.
    unsafe {
        let open = |card: Option<&IMFDXGIDeviceManager>| -> windows::core::Result<(IMFSourceReader, u32, u32, i32)> {
            let mut attributes: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut attributes, 3)?;
            let attributes = attributes.ok_or_else(windows::core::Error::empty)?;
            if let Some(manager) = card {
                // On the graphics card: decoded there (DXVA), the frames left there as the decoder gives them (NV12) and
                // turned into pixels by the card's video processor as they are shown (SharedFrame::convert_from), as a
                // browser plays Roblox's clips. Not through Media Foundation's converter (its "advanced" processing):
                // it gave only every other frame of a 60 fps clip and kept 25 MB more (--clip-check, 2026-11-21).
                attributes.SetUnknown(&MF_SOURCE_READER_D3D_MANAGER, manager)?;
                attributes.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)?;
            } else {
                // Windows only converts to plain pixels; the frame is made smaller here (`shrink`). Its "advanced"
                // processing, which scales too, cost 27 MB and 10 threads more for each 1280x720 clip than the decoder
                // itself (19 MB) when done by the processor, measured 2026-11-19: the home page's clip grew Bloxkeep by
                // tens of MB for each clip playing.
                attributes.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)?;
            }
            let reader = MFCreateSourceReaderFromURL(&HSTRING::from(path.as_os_str()), &attributes)?;
            reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
            reader.SetStreamSelection(video, true)?;
            let want = MFCreateMediaType()?;
            want.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            // (By the processor: RGB32, its fourth byte unused and made opaque by `shrink`.)
            want.SetGUID(&MF_MT_SUBTYPE, if card.is_some() { &MFVideoFormat_NV12 } else { &MFVideoFormat_RGB32 })?;
            reader.SetCurrentMediaType(video, None, &want)?;
            let (w, h, stride) = layout(&reader)?;
            Ok((reader, w, h, stride))
        };
        // (Not on the card where it can't be opened so: by the processor, and so for the rest of this clip.)
        let mut opened = card.as_ref().map(|m| open(Some(m)));
        if let Some(Err(e)) = &opened {
            note_card_failed(&format!("{e}"));
            *card = None;
            opened = None;
        }
        let (reader, mut sw, mut sh, mut stride) = match opened.unwrap_or_else(|| open(None)) {
            Ok(x) => x,
            Err(e) => {
                // Said, as the tile then shows its still picture only (a Windows without the WebM decoder, a bad file).
                crate::net::note_failed("Game clip playback", &path.to_string_lossy(), &format!("{e}"));
                return End::Stop;
            }
        };
        let (mut last, mut step) = (0i64, 666_667i64);
        // (A part of an HLS clip may count on from the parts before it: its times are taken from its first frame.)
        let mut first: Option<i64> = None;
        loop {
            let (mut flags, mut at, mut sample) = (0u32, 0i64, None::<IMFSample>);
            if reader.ReadSample(video, 0, None, Some(&mut flags), Some(&mut at), Some(&mut sample)).is_err() || flags & MF_SOURCE_READERF_ERROR.0 as u32 != 0 {
                return End::Stop;
            }
            if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                return End::Again(base + last + step);
            }
            if flags & MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0 as u32 != 0 {
                match layout(&reader) {
                    Ok(x) => (sw, sh, stride) = x,
                    Err(_) => return End::Stop,
                }
            }
            let Some(sample) = sample else { continue };
            if card.is_some() && CARD_BROKEN.load(Ordering::Relaxed) {
                // (The card can't show its frames: the rest by the processor, from the next part or the clip's start.)
                *card = None;
                return End::Again(base + last + step);
            }
            // Left on the card: its texture handed on with the sample, copied on the card when its time comes.
            if card.is_some()
                && let Ok(buffer) = sample.GetBufferByIndex(0)
                && let Ok(on) = buffer.cast::<IMFDXGIBuffer>()
            {
                let mut texture: Option<ID3D11Texture2D> = None;
                let got = on.GetResource(&ID3D11Texture2D::IID, &mut texture as *mut _ as *mut _).is_ok();
                if let (true, Some(texture), Ok(sub)) = (got, texture, on.GetSubresourceIndex()) {
                    let mut desc = D3D11_TEXTURE2D_DESC::default();
                    texture.GetDesc(&mut desc);
                    let (w, h) = (sw.min(desc.Width), sh.min(desc.Height));
                    let at = at - *first.get_or_insert(at);
                    if at > last {
                        step = (at - last).clamp(1, 10_000_000);
                    }
                    last = at;
                    let frame = OnCard { texture, sub, _sample: sample };
                    if tx.send(Frame { at: base + at, w, h, bgra: Vec::new(), shared: 0, card: Some(frame) }).is_err() {
                        return End::Stop;
                    }
                    continue;
                }
            }
            let Ok(buffer) = sample.ConvertToContiguousBuffer() else { return End::Stop };
            let (mut bytes, mut len) = (std::ptr::null_mut::<u8>(), 0u32);
            if buffer.Lock(&mut bytes, None, Some(&mut len)).is_err() {
                return End::Stop;
            }
            let pitch = (stride.unsigned_abs() as usize).max(sw as usize * 4);
            let need = pitch * sh as usize;
            let (w, h) = fit(sw, sh);
            let bgra = (!bytes.is_null() && sw > 0 && sh > 0 && len as usize >= need)
                .then(|| shrink(std::slice::from_raw_parts(bytes, need), pitch, stride < 0, sw, sh, w, h));
            let _ = buffer.Unlock();
            let Some(bgra) = bgra else { continue };
            let at = at - *first.get_or_insert(at);
            if at > last {
                step = (at - last).clamp(1, 10_000_000);
            }
            last = at;
            if tx.send(Frame { at: base + at, w, h, bgra, shared: 0, card: None }).is_err() {
                return End::Stop;
            }
        }
    }
}

/// --clip-check FILE [--seconds N]: a game clip decoded as a tile plays it (its frames taken as they come due), for
/// measuring from outside what decoding one costs: the threads and memory it brings. Prints the frames.
#[cfg(any(feature = "core", feature = "shot"))]
pub fn clip_check(file: &Path, seconds: u64) {
    let (tx, rx) = mpsc::sync_channel::<Frame>(AHEAD);
    let parts = parts(file);
    crate::threads::run(move || decode(&parts, &tx));
    let begun = Instant::now();
    let mut frames = 0u64;
    while begun.elapsed() < Duration::from_secs(seconds) {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(f) => {
                frames += 1;
                // (Taken when due, as the tile takes them.)
                let due = Duration::from_nanos(f.at.max(0) as u64 * 100);
                if let Some(wait) = due.checked_sub(begun.elapsed()) {
                    std::thread::sleep(wait.min(Duration::from_millis(200)));
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    drop(rx);
    println!("clip-check: {frames} frames in {:.1} s", begun.elapsed().as_secs_f32());
    // (The decoder lets go once it finds nobody takes its frames: a moment for that, measured from outside too.)
    std::thread::sleep(Duration::from_secs(3));
}

#[cfg(all(windows, feature = "core"))]
static SOUND_FRAMES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
#[cfg(all(windows, feature = "core"))]
static SOUND_PEAK: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// --clip-sound-check FILE [--seconds N]: the clip's sound decoded and fed to the speakers as a tile's is, but as
/// silence (a turn that is never the current one), so nothing is heard: prints what was decoded and its loudest sample.
#[cfg(all(windows, feature = "core"))]
pub fn clip_sound_check(file: &Path, seconds: u64) {
    let parts = parts(file);
    let (stop, heard, seen) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)), Arc::new(std::sync::atomic::AtomicU64::new(now_ms())));
    let (s, h) = (stop.clone(), heard.clone());
    let done = Arc::new(AtomicBool::new(false));
    let d = done.clone();
    crate::threads::run(move || {
        sound(&parts, &s, &h, &seen, u64::MAX);
        d.store(true, Ordering::Relaxed);
    });
    let begun = Instant::now();
    while begun.elapsed() < Duration::from_secs(seconds) && !done.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(100));
    }
    stop.store(true, Ordering::Relaxed);
    std::thread::sleep(Duration::from_millis(300));
    println!(
        "clip-sound-check: sound {}, {} frames decoded in {:.1} s, loudest {:.3}, thread ended by itself: {}",
        heard.load(Ordering::Relaxed),
        SOUND_FRAMES.load(Ordering::Relaxed),
        begun.elapsed().as_secs_f32(),
        f32::from_bits(SOUND_PEAK.load(Ordering::Relaxed)),
        done.load(Ordering::Relaxed)
    );
}

/// --clip-window-check FILE [--seconds N]: the clip played as a tile plays it, in a real GPUI window off the screen
/// and under DWM's cloak (nothing is seen), for measuring from outside what showing one costs: whether its frames make
/// a Direct3D device of their own (NVIDIA's driver threads), the memory. Without FILE's clip ("-"): the window alone;
/// "~": the window alone, drawn again every frame (what drawing 60 times a second costs, without a clip).
#[cfg(feature = "core")]
pub fn clip_window_check(file: String, seconds: u64) {
    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);
        let bounds = gpui_kit::Bounds { origin: gpui_kit::point(gpui_kit::px(-6000.), gpui_kit::px(-6000.)), size: gpui_kit::size(gpui_kit::px(640.), gpui_kit::px(360.)) };
        let options = gpui_kit::WindowOptions {
            kind: gpui_kit::WindowKind::PopUp,
            titlebar: None,
            focus: false,
            show: true,
            window_bounds: Some(gpui_kit::WindowBounds::Windowed(bounds)),
            ..Default::default()
        };
        let window = cx.open_window(options, move |window, cx| {
            if let Some(h) = crate::hwnd_of(window) {
                let on: i32 = 1;
                // SAFETY: a window of ours, cloaked so nothing of the check is seen.
                unsafe { windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(h as _, windows_sys::Win32::Graphics::Dwm::DWMWA_CLOAK as u32, (&on as *const i32).cast(), 4) };
            }
            let spin = file == "~";
            cx.new(|cx| ClipCheck { video: (file != "-" && !spin).then(|| cx.new(|cx| Video::new(PathBuf::from(&file), cx))), spin })
        });
        let Ok(window) = window else {
            println!("clip-window-check: no window");
            cx.quit();
            return;
        };
        cx.spawn(async move |cx| {
            cx.background_executor().timer(Duration::from_secs(seconds)).await;
            let how = cx
                .update(|cx| {
                    window.update(cx, |root, _, cx| match &root.video {
                        Some(v) => {
                            let v = v.read(cx);
                            format!("gpu frame {}, gpu failed {}", v.gpu.is_some(), v.gpu_failed)
                        }
                        None => "no clip".into(),
                    })
                })
                .unwrap_or_default();
            println!("clip-window-check: done ({how})");
            let _ = cx.update(|cx| cx.quit());
        })
        .detach();
    });
}

#[cfg(feature = "core")]
struct ClipCheck {
    video: Option<gpui_kit::Entity<Video>>,
    spin: bool,
}

#[cfg(feature = "core")]
impl Render for ClipCheck {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        if self.spin {
            window.request_animation_frame();
        }
        div().size_full().bg(gpui_kit::rgb(0x202020)).children(self.video.clone())
    }
}

/// The files to play in turn: the clip itself, or for an HLS clip (roblox.rs's `{asset}.parts`) the parts it lists.
fn parts(path: &Path) -> Vec<PathBuf> {
    if path.extension().is_some_and(|e| e == "parts") {
        let dir = path.parent().unwrap_or(Path::new("."));
        if let Ok(names) = std::fs::read_to_string(path) {
            return names.lines().filter(|n| !n.trim().is_empty()).map(|n| dir.join(n.trim())).collect();
        }
    }
    vec![path.to_path_buf()]
}
