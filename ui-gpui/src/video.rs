// A game's gameplay clip (the video its Roblox page plays first), shown muted and over and over on its tile while the
// mouse rests there (pages/clips.rs).
//
// Roblox keeps it as WebM (VP9, with Opus sound; Adopt Me's: 640×360, 15 frames a second, 32 s, ~10 MB, measured
// 2026-11-13). GPUI has no video, so Windows' own Media Foundation decodes it (its WebM reader and VP9 decoder are part
// of Windows 10 and 11) on a thread of its own, a few frames ahead of the clock, and this view shows each frame when its
// time comes. The decoder only works while the view is drawn in a window that is seen: a moment after it goes (the
// mouse left the tile, or the window is minimized, hidden in the tray or covered) the clock stops, and a while later
// the thread and its frame are let go; the next draw in a seen window starts it again.
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        mpsc::{self, SyncSender, TryRecvError},
    },
    time::{Duration, Instant},
};

use gpui_kit::{prelude::*, AsyncApp, Context, EventEmitter, ImageSource, ObjectFit, RenderImage, Subscription, WeakEntity, Window, div, img};

/// Frames decoded ahead of the one on screen.
const AHEAD: usize = 4;
/// Not drawn for this long: the clock stops (the page went, or the window is hidden).
const PAUSE: Duration = Duration::from_millis(800);
/// Not drawn for this long: the decoder and the last frame are let go.
const STOP: Duration = Duration::from_secs(20);
/// The widest frame kept: a bigger clip is scaled down by Windows while it decodes (the page shows it at most ~600 px).
const MAX_W: u32 = 960;
/// …and for a video behind the whole window (a custom theme's): wider, as it fills the window.
const MAX_W_BEHIND: u32 = 1600;

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
}

/// The clip could not be played (no decoder on this PC, a damaged file): its place in the gallery goes.
pub struct Failed;

pub struct Video {
    path: PathBuf,
    frame: Option<Arc<RenderImage>>,
    drawn: Instant,
    playing: bool,
    failed: bool,
    /// Its window's coming back into sight draws it again (a restored window needn't draw anything otherwise).
    watch: Option<Subscription>,
    widest: u32,
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
        Self { path, frame: None, drawn: Instant::now(), playing: false, failed: false, watch: None, widest: MAX_W }
    }

    /// The same for a video that fills the window behind the page (Settings › Appearance › Custom).
    pub fn behind(path: PathBuf, cx: &mut Context<Self>) -> Self {
        let mut v = Self::new(path, cx);
        v.widest = MAX_W_BEHIND;
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
        let spawned = crate::net::Named::new("video".into()).spawn(move || {
            WIDEST.with(|w| w.set(widest));
            decode(&parts, &tx)
        });
        if spawned.is_err() {
            self.failed = true;
            cx.emit(Failed);
            return;
        }
        self.playing = true;
        cx.spawn(async move |this, cx| play(this, rx, cx).await).detach();
    }
}

/// Shows each frame when its time comes, and stops as `Video` says.
async fn play(this: WeakEntity<Video>, rx: mpsc::Receiver<Frame>, cx: &mut AsyncApp) {
    let mut clock: Option<(Instant, i64)> = None;
    let mut waiting: Option<Frame> = None;
    let mut shown_any = false;
    loop {
        let Ok(idle) = this.update(cx, |v, _| v.drawn.elapsed()) else { return };
        if idle >= STOP {
            let _ = this.update(cx, |v, cx| {
                v.playing = false;
                if let Some(old) = v.frame.take() {
                    cx.drop_image(old, None);
                }
            });
            return;
        }
        if idle >= PAUSE {
            clock = None;
            cx.background_executor().timer(Duration::from_millis(250)).await;
            continue;
        }
        let frame = match waiting.take() {
            Some(f) => f,
            None => match rx.try_recv() {
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
        let Some(pixels) = image::RgbaImage::from_raw(frame.w, frame.h, frame.bgra) else { continue };
        let shown = Arc::new(RenderImage::new(vec![image::Frame::new(pixels)]));
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
        if window.is_visible() || cfg!(feature = "shot") {
            self.drawn = Instant::now();
            if !self.playing && !self.failed {
                self.start(cx);
            }
        }
        div().size_full().when_some(self.frame.clone(), |d, f| d.child(img(ImageSource::Render(f)).size_full().object_fit(ObjectFit::Cover)))
    }
}

/// The frame size kept for a clip of `w`×`h`: as it is, or scaled down to MAX_W wide; even, as the decoder wants.
fn fit(w: u32, h: u32) -> (u32, u32) {
    let (w, h) = (w.max(2), h.max(2));
    let most = WIDEST.with(|m| m.get());
    let (w, h) = if w > most { (most, ((h as u64 * most as u64) / w as u64) as u32) } else { (w, h) };
    ((w & !1).max(2), (h & !1).max(2))
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
        Media::MediaFoundation::{MF_VERSION, MFSTARTUP_LITE, MFShutdown, MFStartup},
        System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize},
    };
    // SAFETY: plain calls into Windows; COM and Media Foundation are started and stopped in pairs on this thread.
    unsafe {
        let com = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        if MFStartup(MF_VERSION, MFSTARTUP_LITE).is_ok() {
            let mut base = 0i64;
            // (A clip that ends before its first frame isn't started again and again.)
            'clip: loop {
                let start = base;
                for part in parts {
                    match pass(part, tx, base) {
                        End::Again(next) if next > base => base = next,
                        End::Again(_) => {}
                        End::Stop => break 'clip,
                    }
                }
                if base <= start {
                    break;
                }
            }
            let _ = MFShutdown();
        }
        if com {
            CoUninitialize();
        }
    }
}

#[cfg(not(all(windows, any(feature = "core", feature = "shot"))))]
fn decode(_: &[PathBuf], _: &SyncSender<Frame>) {}

/// One pass through the file: its frames, their times moved on by `base`.
#[cfg(all(windows, any(feature = "core", feature = "shot")))]
unsafe fn pass(path: &Path, tx: &SyncSender<Frame>, base: i64) -> End {
    use windows::{
        Win32::Media::MediaFoundation::{
            IMFAttributes, IMFSample, IMFSourceReader, MF_MT_FRAME_SIZE, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE, MF_SOURCE_READER_ALL_STREAMS,
            MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, MF_SOURCE_READER_FIRST_VIDEO_STREAM, MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED,
            MF_SOURCE_READERF_ENDOFSTREAM, MF_SOURCE_READERF_ERROR, MFCreateAttributes, MFCreateMediaType, MFCreateSourceReaderFromURL,
            MFMediaType_Video, MFVideoFormat_RGB32,
        },
        core::HSTRING,
    };
    let video = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
    // SAFETY: Media Foundation calls on objects made here; the buffer's bytes are read only between Lock and Unlock, and
    // no further than the length Lock gives.
    unsafe {
        let open = || -> windows::core::Result<(IMFSourceReader, u32, u32)> {
            let mut attributes: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut attributes, 1)?;
            let attributes = attributes.ok_or_else(windows::core::Error::empty)?;
            // (Windows converts to plain pixels and scales: the "advanced" processing.)
            attributes.SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1)?;
            let reader = MFCreateSourceReaderFromURL(&HSTRING::from(path.as_os_str()), &attributes)?;
            reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
            reader.SetStreamSelection(video, true)?;
            let size = reader.GetCurrentMediaType(video)?.GetUINT64(&MF_MT_FRAME_SIZE)?;
            let (w, h) = fit((size >> 32) as u32, size as u32);
            let want = MFCreateMediaType()?;
            want.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            want.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)?;
            want.SetUINT64(&MF_MT_FRAME_SIZE, ((w as u64) << 32) | h as u64)?;
            reader.SetCurrentMediaType(video, None, &want)?;
            Ok((reader, w, h))
        };
        let (reader, mut w, mut h) = match open() {
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
                match reader.GetCurrentMediaType(video).and_then(|t| t.GetUINT64(&MF_MT_FRAME_SIZE)) {
                    Ok(size) => (w, h) = ((size >> 32) as u32, size as u32),
                    Err(_) => return End::Stop,
                }
            }
            let Some(sample) = sample else { continue };
            let Ok(buffer) = sample.ConvertToContiguousBuffer() else { return End::Stop };
            let (mut bytes, mut len) = (std::ptr::null_mut::<u8>(), 0u32);
            if buffer.Lock(&mut bytes, None, Some(&mut len)).is_err() {
                return End::Stop;
            }
            let need = w as usize * h as usize * 4;
            let bgra = (!bytes.is_null() && len as usize >= need).then(|| {
                let mut v = std::slice::from_raw_parts(bytes, need).to_vec();
                // (RGB32's fourth byte is unused, not opacity.)
                for p in v.chunks_exact_mut(4) {
                    p[3] = 255;
                }
                v
            });
            let _ = buffer.Unlock();
            let Some(bgra) = bgra else { continue };
            let at = at - *first.get_or_insert(at);
            if at > last {
                step = (at - last).clamp(1, 10_000_000);
            }
            last = at;
            if tx.send(Frame { at: base + at, w, h, bgra }).is_err() {
                return End::Stop;
            }
        }
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
