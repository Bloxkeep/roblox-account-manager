// Pictures by address, as the page shows them: Roblox's (https, avatars and game icons), the app's own files (a path),
// and data: addresses (the dev mock's drawn faces). Each is fetched once by worker threads and kept for the session;
// a view asks with `pic(url, cx)` and gets None until it is there, then every window is drawn again.
//
// Made quick (the owner, 2026-11-12: slow everywhere, avatars, items): a picture from the web is kept on this PC too
// (cache/pictures), so the next run draws it at once; the last asked is fetched first (what the page shows now, not
// what scrolled past); and the workers keep their connections to each server open between pictures.
use std::{
    collections::HashMap,
    io::Read,
    path::PathBuf,
    sync::{
        Arc, Condvar, LazyLock, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime},
};

use base64::Engine;
use futures::channel::oneshot;
use gpui_kit::{App, Global, Image, ImageCacheError, ImageFormat, ImageSource, RenderImage, Window};

enum Slot {
    /// On its way: how often it failed before (above 0: still failed() till it comes, so a place shrunk for it
    /// doesn't spring back to full size for the moment the retry takes), and when a draw last asked for it.
    Loading(u32, Arc<AtomicU64>),
    /// The picture, its own colour when it has one (`tone`), when it was last asked for (a draw asks for every
    /// picture it shows, each time), and its size in pixels when it can be made smaller (not an SVG or a GIF).
    Ready(Arc<Image>, Option<u32>, Instant, Option<(u32, u32)>),
    /// Since when, how many times in a row, and whether the server said it isn't there (404, 403): asked again after
    /// FAILED_RETRY, twice that after the next failure and so on (a moment offline, Roblox's CDN busy), not for the rest
    /// of the run; one the server refused only after GONE_RETRY.
    Failed(Instant, u32, bool),
}

const FAILED_RETRY: Duration = Duration::from_secs(60);
const FAILED_MAX: Duration = Duration::from_secs(30 * 60);
const GONE_RETRY: Duration = Duration::from_secs(60 * 60);

/// A picture waiting in the queue that no draw has asked for this long (scrolled away, the page left) is dropped
/// there, not fetched: asked for again, it is queued again.
const STALE: Duration = Duration::from_secs(20);

/// The time since the run started, in ms: when a draw last asked for a picture on its way.
fn ms_now() -> u64 {
    static START: LazyLock<Instant> = LazyLock::new(Instant::now);
    START.elapsed().as_millis() as u64
}

/// How long a picture that failed `n` times in a row waits before it is asked again.
fn wait_after(n: u32, gone: bool) -> Duration {
    if gone {
        return GONE_RETRY;
    }
    FAILED_RETRY.saturating_mul(1 << n.saturating_sub(1).min(5)).min(FAILED_MAX)
}

/// A smaller copy of a picture, made for the pixels it takes on screen (see `pic_at`).
enum CopyState {
    Loading,
    Ready(Arc<RenderImage>),
    Failed,
}

struct Smaller {
    /// Its longer side in pixels (the key: a picture's copies all keep its shape).
    long: u32,
    state: CopyState,
    used: Instant,
}

/// A copy not drawn for this long while another size of the same picture was drawn after it is let go (a window
/// dragged wider makes a new size every few pixels; the old ones are off screen).
const SUPERSEDED: Duration = Duration::from_secs(10);

/// Past this many pictures kept, the ones no draw has asked for in IDLE are let go (their decoded copy in GPUI's cache
/// and their place on each window's atlas too), oldest first, down to KEEP. A picture asked for lately is never let go,
/// however many there are (a page of 650 friends keeps all 650): a picture on screen is asked for by every draw, and the
/// arrival that runs this came from a draw moments before. One let go and wanted again comes back from this PC's copy
/// (cache/pictures) in a few milliseconds. Without this every picture ever shown (a shop browsed for an hour) stayed
/// decoded in memory for the whole run.
const MAX_READY: usize = 600;
const KEEP: usize = 400;
const IDLE: Duration = Duration::from_secs(600);

/// A picture no window has drawn for this long is let go, however few are kept: its bytes, its sized copies, GPUI's
/// decoded copy and its place on the window's atlas. Measured on the owner's PC (2026-11-15, twenty minutes of
/// browsing the Roblox tab on the plain Dark theme): 428 MB, of which 62 atlas textures of 4 MB each and ~380 MB of
/// heap, nearly all pictures of pages left minutes before (the same build showing one page: 98 MB, 2 textures). The
/// count rule above only started past 600 pictures and ten minutes, so nothing was ever let go in a normal sitting.
/// One wanted again comes back from this PC's copy (cache/pictures) in a few milliseconds.
const AWAY: Duration = Duration::from_secs(45);
/// How often the pictures away are looked for, and how long the windows get to draw (and so ask for what they show)
/// before the look.
const SWEEP_EVERY: Duration = Duration::from_secs(10);
const SETTLE: Duration = Duration::from_millis(700);
/// Addresses that failed, kept so they aren't asked again and again; past this many the list starts over.
const MAX_FAILED: usize = 2000;

type Fetched = (Vec<u8>, Option<u32>, Option<(u32, u32)>);

/// How a fetch ended.
enum Got {
    Bytes(Fetched),
    /// Not had; true when the server said it isn't there (a 4xx other than "too many"), not worth asking soon.
    Failed(bool),
    /// Left in the queue too long unasked: not fetched.
    Dropped,
}

type Job = (String, oneshot::Sender<Got>, Arc<AtomicU64>);

/// The pictures waiting: taken newest first, so what was asked last (the page as it is now) comes before what a
/// scroll left behind.
struct Queue {
    jobs: Mutex<Vec<Job>>,
    ready: Condvar,
}

pub struct Images {
    slots: HashMap<String, Slot>,
    /// Each picture's smaller copies, by its address.
    copies: HashMap<String, Vec<Smaller>>,
    queue: Arc<Queue>,
}

impl Global for Images {}

/// How many pictures load at once.
const WORKERS: usize = 12;

pub fn init(cx: &mut App) {
    let queue = Arc::new(Queue { jobs: Mutex::new(Vec::new()), ready: Condvar::new() });
    // Its connections stay open between pictures, one per worker and server: ureq keeps only one per server unless
    // told, so every other worker paid a new TLS handshake for each picture.
    let agent = crate::net::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("Bloxkeep")
        .max_idle_connections(64)
        .max_idle_connections_per_host(WORKERS)
        .build();
    for i in 0..WORKERS {
        let queue = queue.clone();
        let agent = agent.clone();
        let _ = crate::net::Named::new(format!("pictures-{i}")).spawn(move || {
            loop {
                let job = {
                    let Ok(mut jobs) = queue.jobs.lock() else { break };
                    loop {
                        // The draw that asked last goes first (the page as it is now, not what a scroll left behind),
                        // and within one draw the first asked first: taken from the list's end, a long table's
                        // pictures came from its foot up, and the rows on screen got theirs last (the owner,
                        // 2026-11-15: a wiki's item icons "hiện muộn, hiện chậm").
                        let next = (0..jobs.len()).max_by_key(|i| (jobs[*i].2.load(Ordering::Relaxed) / 96, std::cmp::Reverse(*i)));
                        if let Some(i) = next {
                            break jobs.remove(i);
                        }
                        match queue.ready.wait(jobs) {
                            Ok(j) => jobs = j,
                            Err(_) => return,
                        }
                    }
                };
                let (url, done, asked) = job;
                if ms_now().saturating_sub(asked.load(Ordering::Relaxed)) > STALE.as_millis() as u64 {
                    let _ = done.send(Got::Dropped);
                    continue;
                }
                let _ = done.send(match fetch(&agent, &url) {
                    Ok(b) => {
                        let (tone, second) = tones_of(&b);
                        if let (Some(c2), Ok(mut book)) = (second, SECOND.lock()) {
                            book.insert(url.clone(), c2);
                        }
                        let dims = dims_of(&b);
                        Got::Bytes((b, tone, dims))
                    }
                    Err(gone) => {
                        if url.starts_with("http") {
                            crate::net::note_failed("Picture", &url, if gone { "not there or not a picture" } else { "no answer, asked again later" });
                        }
                        Got::Failed(gone)
                    }
                });
            }
        });
    }
    let _ = crate::net::Named::new("pictures-prune".into()).spawn(prune_often);
    cx.set_global(Images { slots: HashMap::new(), copies: HashMap::new(), queue });
    // Every SWEEP_EVERY: when something may be away, every window is drawn once (a window standing still asks for
    // nothing, so its pictures would look away too), and what none of them asked for in AWAY is let go. A window
    // hidden in the tray or minimized doesn't draw: its pictures go too, and come back when it is shown.
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(SWEEP_EVERY).await;
            let asked = Instant::now();
            let some = cx.update(|cx| {
                let some = may_be_away(cx.global::<Images>(), asked);
                if some {
                    cx.refresh_windows();
                }
                some
            });
            if !some {
                continue;
            }
            cx.background_executor().timer(SETTLE).await;
            cx.update(|cx| {
                trim(cx, Some(asked));
                // BLOXKEEP_SHOT_MEM=1: a hidden run says what is kept after each look.
                #[cfg(feature = "shot")]
                if std::env::var("BLOXKEEP_SHOT_MEM").is_ok() {
                    let (n, bytes, copies, pixels) = held(cx);
                    eprintln!("pictures kept: {n} ({} KB as fetched), {copies} sized copies ({} KB of pixels)", bytes / 1024, pixels / 1024);
                }
            });
        }
    })
    .detach();
}

/// Whether a picture or a copy was last asked for more than AWAY before `asked`.
fn may_be_away(images: &Images, asked: Instant) -> bool {
    images.slots.values().any(|s| matches!(s, Slot::Ready(_, _, used, _) if *used + AWAY < asked))
        || images.copies.values().flatten().any(|c| matches!(c.state, CopyState::Ready(_)) && c.used + AWAY < asked)
}

/// What is kept now: the pictures and their bytes as fetched, the sized copies and their pixels' bytes.
pub fn held(cx: &App) -> (usize, u64, usize, u64) {
    let images = cx.global::<Images>();
    let (mut n, mut bytes, mut copies, mut pixels) = (0, 0u64, 0, 0u64);
    for s in images.slots.values() {
        if let Slot::Ready(i, ..) = s {
            n += 1;
            bytes += i.bytes.len() as u64;
        }
    }
    for c in images.copies.values().flatten() {
        if let CopyState::Ready(r) = &c.state {
            copies += 1;
            let size = r.size(0);
            pixels += size.width.0.max(0) as u64 * size.height.0.max(0) as u64 * 4;
        }
    }
    (n, bytes, copies, pixels)
}

/// Where pictures from the web are kept on this PC: Bloxkeep's cache folder (BLOXKEEP_KEPT or BLOXKEEP_DATA: a hidden
/// shot's or a check's own).
static DISK: LazyLock<PathBuf> = LazyLock::new(|| {
    let base = std::env::var("BLOXKEEP_KEPT")
        .or_else(|_| std::env::var("BLOXKEEP_DATA"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into())).join("Bloxkeep"));
    base.join("cache").join("pictures")
});

/// Another folder of that cache (`videos`: the games' clips).
pub fn kept_dir(sub: &str) -> PathBuf {
    DISK.parent().map(|p| p.join(sub)).unwrap_or_else(|| DISK.join(sub))
}

/// The most kept, and how long a picture whose address isn't its content is trusted (Roblox's are: a new picture gets
/// a new address).
const DISK_MAX: u64 = 400 << 20;
const DISK_LIFE: Duration = Duration::from_secs(7 * 24 * 3600);

fn disk_name(url: &str) -> PathBuf {
    // FNV-1a over the address, twice with different seeds: a name that stays the same from build to build.
    let mut a: u64 = 0xcbf29ce484222325;
    let mut b: u64 = 0x84222325cbf29ce4;
    for &x in url.as_bytes() {
        a = (a ^ x as u64).wrapping_mul(0x100000001b3);
        b = (b ^ x as u64).wrapping_mul(0x100000001b3).rotate_left(5);
    }
    DISK.join(format!("{a:016x}{b:016x}"))
}

/// Roblox's picture servers name each picture by its content.
fn lasting(url: &str) -> bool {
    url.starts_with("https://") && url.split('/').nth(2).is_some_and(|h| h.ends_with(".rbxcdn.com"))
}

fn from_disk(url: &str) -> Option<Vec<u8>> {
    let path = disk_name(url);
    if !lasting(url) {
        let age = std::fs::metadata(&path).ok()?.modified().ok()?.elapsed().unwrap_or_default();
        if age > DISK_LIFE {
            return None;
        }
    }
    let b = std::fs::read(&path).ok()?;
    format_of(&b).is_some().then_some(b)
}

fn to_disk(url: &str, b: &[u8]) {
    let path = disk_name(url);
    let _ = std::fs::create_dir_all(&*DISK);
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, b).is_ok() && std::fs::rename(&tmp, &path).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}

/// Once a run, a little after it starts: the pictures kept longest go while the folder is over its size.
fn prune_often() {
    std::thread::sleep(Duration::from_secs(30));
    loop {
        prune();
        // A run kept open in the tray for days stays within DISK_MAX too.
        std::thread::sleep(Duration::from_secs(3600));
    }
}

fn prune() {
    let Ok(dir) = std::fs::read_dir(&*DISK) else { return };
    let mut files: Vec<(SystemTime, u64, PathBuf)> = dir
        .flatten()
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            m.is_file().then(|| (m.modified().unwrap_or(SystemTime::UNIX_EPOCH), m.len(), e.path()))
        })
        .collect();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    if total <= DISK_MAX {
        return;
    }
    files.sort_by_key(|f| f.0);
    for (_, len, path) in files {
        if total <= DISK_MAX * 3 / 4 {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= len;
        }
    }
}

/// The picture's bytes, or whether the server said it isn't there (true) rather than failing a moment (false).
fn fetch(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>, bool> {
    let started = Instant::now();
    let web = url.starts_with("http://") || url.starts_with("https://");
    if web && let Some(b) = from_disk(url) {
        log(url, started, "disk");
        return Ok(b);
    }
    let got = fetch_now(agent, url);
    if web && let Some(b) = got.as_ref().ok().filter(|b| format_of(b).is_some()) {
        to_disk(url, b);
        log(url, started, "web");
    }
    got
}

/// BLOXKEEP_PIC_LOG=1: each picture's time printed (where it came from, how long it took).
fn log(url: &str, started: Instant, from: &str) {
    static ON: LazyLock<bool> = LazyLock::new(|| std::env::var("BLOXKEEP_PIC_LOG").is_ok());
    if *ON {
        eprintln!("[pic] {from} {:>5} ms {}", started.elapsed().as_millis(), url.chars().take(90).collect::<String>());
    }
}

fn fetch_now(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>, bool> {
    if let Some(rest) = url.strip_prefix("data:") {
        let (meta, body) = rest.split_once(',').ok_or(true)?;
        return if meta.ends_with(";base64") {
            base64::engine::general_purpose::STANDARD.decode(body).map_err(|_| true)
        } else {
            Ok(percent_encoding::percent_decode_str(body).collect())
        };
    }
    if url.starts_with("http://") || url.starts_with("https://") {
        // A page of many pictures (an inventory shelf) has some of them refused or cut off now and then: each gets
        // three goes, a little apart. A server that says the picture isn't there (404, 403...) is not asked again.
        for round in 0..3u64 {
            if round > 0 {
                std::thread::sleep(std::time::Duration::from_millis(500 * round));
            }
            let r = match agent.get(url).call() {
                Ok(r) => r,
                Err(ureq::Error::Status(code, _)) if (400..500).contains(&code) && code != 408 && code != 429 => return Err(true),
                Err(_) => continue,
            };
            let mut bytes = Vec::new();
            if r.into_reader().take(16 << 20).read_to_end(&mut bytes).is_ok() && !bytes.is_empty() {
                return Ok(bytes);
            }
        }
        return Err(false);
    }
    std::fs::read(url.strip_prefix("file:///").unwrap_or(url)).map_err(|_| true)
}

fn format_of(b: &[u8]) -> Option<ImageFormat> {
    Some(match b {
        [0x89, b'P', b'N', b'G', ..] => ImageFormat::Png,
        [0xff, 0xd8, ..] => ImageFormat::Jpeg,
        [b'G', b'I', b'F', ..] => ImageFormat::Gif,
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => ImageFormat::Webp,
        [b'B', b'M', ..] => ImageFormat::Bmp,
        _ if b.iter().take(256).collect::<Vec<_>>().windows(4).any(|w| w == [&b'<', &b's', &b'v', &b'g']) => ImageFormat::Svg,
        _ => return None,
    })
}

/// A picture's own colour: the hue most of its vivid pixels share, at a lightness and strength that read on the dark
/// surfaces (a game's icon gives its tiles their base). PNG or WebP (what Roblox is asked for); None for a grey picture.
fn tone_of(b: &[u8]) -> Option<u32> {
    tones_of(b).0
}

/// That colour, and the picture's second one: the next most voted hue at least a sixth of the wheel away, when it has
/// a good share of the votes (a game's logo is often two colours: Blox Fruits' blue and orange). A game's name is
/// written in them (kit::game_name).
fn tones_of(b: &[u8]) -> (Option<u32>, Option<u32>) {
    let format = match format_of(b) {
        Some(ImageFormat::Png) => image::ImageFormat::Png,
        Some(ImageFormat::Webp) => image::ImageFormat::WebP,
        _ => return (None, None),
    };
    let Some(img) = image::load_from_memory_with_format(b, format).ok().map(|i| i.to_rgba8()) else { return (None, None) };
    let (w, h) = img.dimensions();
    let step = (w.max(h) / 48).max(1) as usize;
    // Twelve slices of the colour wheel: each pixel votes for its own with how vivid and bright it is.
    let mut votes = [(0f64, 0f64, 0f64, 0f64); 12];
    for y in (0..h).step_by(step) {
        for x in (0..w).step_by(step) {
            let p = img.get_pixel(x, y).0;
            if p[3] < 128 {
                continue;
            }
            let (r, g, bl) = (p[0] as f64 / 255., p[1] as f64 / 255., p[2] as f64 / 255.);
            let (mx, mn) = (r.max(g).max(bl), r.min(g).min(bl));
            let spread = mx - mn;
            if spread < 0.12 {
                continue;
            }
            let hue = if mx == r { ((g - bl) / spread).rem_euclid(6.) } else if mx == g { (bl - r) / spread + 2. } else { (r - g) / spread + 4. } * 60.;
            let k = spread * spread * mx;
            let v = &mut votes[((hue / 30.) as usize).min(11)];
            *v = (v.0 + k, v.1 + hue.to_radians().cos() * k, v.2 + hue.to_radians().sin() * k, v.3 + spread * k);
        }
    }
    let Some(first) = (0..12).max_by(|a, b| votes[*a].0.total_cmp(&votes[*b].0)).filter(|i| votes[*i].0 > 0.) else { return (None, None) };
    let apart = |i: usize| {
        let d = (i as i32 - first as i32).rem_euclid(12);
        d.min(12 - d) >= 2
    };
    let second = (0..12).filter(|i| apart(*i)).max_by(|a, b| votes[*a].0.total_cmp(&votes[*b].0)).filter(|i| votes[*i].0 >= 0.2 * votes[first].0);
    // A slice's colour: its hue at a fixed lightness and its own strength.
    let paint = |best: (f64, f64, f64, f64)| {
        let hue = best.2.atan2(best.1).to_degrees().rem_euclid(360.);
        let strength = (best.3 / best.0).clamp(0.45, 0.8);
        let light = 0.56;
        let chroma = (1. - (2. * light - 1f64).abs()) * strength;
        let x = chroma * (1. - ((hue / 60.).rem_euclid(2.) - 1.).abs());
        let (r, g, bl) = match (hue / 60.) as u32 {
            0 => (chroma, x, 0.),
            1 => (x, chroma, 0.),
            2 => (0., chroma, x),
            3 => (0., x, chroma),
            4 => (x, 0., chroma),
            _ => (chroma, 0., x),
        };
        let m = light - chroma / 2.;
        let byte = |v: f64| ((v + m).clamp(0., 1.) * 255.).round() as u32;
        byte(r) << 16 | byte(g) << 8 | byte(bl)
    };
    (Some(paint(votes[first])), second.map(|i| paint(votes[i])))
}

/// The second colours of the pictures fetched this run, by address (few and small: kept for the run).
static SECOND: LazyLock<Mutex<HashMap<String, u32>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// The second colour of the picture at an address, once it has loaded (see `tones_of`).
pub fn tone2(url: &str) -> Option<u32> {
    SECOND.lock().ok()?.get(url).copied()
}

/// A picture's size in pixels, read from its header, when a smaller copy can be made of it: not an SVG (GPUI draws
/// those at their size) nor a GIF (a copy would stop it moving).
fn dims_of(b: &[u8]) -> Option<(u32, u32)> {
    if matches!(format_of(b), None | Some(ImageFormat::Svg | ImageFormat::Gif)) {
        return None;
    }
    image::ImageReader::new(std::io::Cursor::new(b)).with_guessed_format().ok()?.into_dimensions().ok().filter(|(w, h)| *w > 0 && *h > 0)
}

/// The picture made `w`×`h` pixels with a filter that weighs every pixel it covers (GPUI's own drawing samples four
/// pixels of the full picture, so a 150 px face shown at 45 px came out jagged): the colours weighed by their alpha, so
/// the see-through edge round an avatar doesn't darken. Kept as GPUI keeps pictures: blue, green, red, alpha, not
/// premultiplied.
pub(crate) fn shrink(bytes: &[u8], w: u32, h: u32, sigma: f32) -> Option<RenderImage> {
    let mut decoder = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format().ok()?.into_decoder().ok()?;
    let orientation = image::ImageDecoder::orientation(&mut decoder).ok()?;
    let mut picture = image::DynamicImage::from_decoder(decoder).ok()?;
    picture.apply_orientation(orientation);
    let mut rgba = picture.into_rgba8();
    let (sw, sh) = rgba.dimensions();
    // A picture turned on its side by its EXIF note: the copy turns with it.
    let (w, h) = if sw != sh && (sw > sh) != (w > h) && w != h { (h, w) } else { (w, h) };
    for p in rgba.pixels_mut() {
        let a = p[3] as u32;
        for c in &mut p.0[..3] {
            *c = ((*c as u32 * a + 127) / 255) as u8;
        }
    }
    // A picture many times bigger is first boxed down to twice the size (each pixel counted once, quick), so the
    // finer filter has little to do.
    if sw > w * 3 && sh > h * 3 {
        rgba = image::imageops::thumbnail(&rgba, w * 2, h * 2);
    }
    let mut out = image::imageops::resize(&rgba, w, h, image::imageops::FilterType::CatmullRom);
    // A blurred copy (sigma above 0): blurred while its colours are still weighed by their alpha.
    if sigma > 0. {
        out = image::imageops::fast_blur(&out, sigma);
    }
    for p in out.pixels_mut() {
        let a = p[3];
        for c in &mut p.0[..3] {
            let v = (*c).min(a);
            *c = if a == 0 { 0 } else { ((v as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8 };
        }
        p.0.swap(0, 2);
    }
    Some(RenderImage::new(vec![image::Frame::new(out)]))
}

/// The longer side, in pixels, of the copy a box of `bw`×`bh` pixels wants of a `w`×`h` picture filling it (as
/// ObjectFit::Cover), or None when the picture is near that size already. Rounded up to a step, so a box that grows
/// a few pixels at a time (a window dragged wider) doesn't ask for a new copy each frame.
fn copy_long(w: u32, h: u32, bw: f32, bh: f32) -> Option<u32> {
    let f = (bw / w as f32).max(bh / h as f32);
    if !(f > 0. && f < 0.8) {
        return None;
    }
    let long = (w.max(h) as f32 * f).ceil().max(1.) as u32;
    let step = if long <= 128 { 8 } else if long <= 256 { 16 } else { 32 };
    Some(long.div_ceil(step) * step).filter(|l| *l < w.max(h))
}

fn copy_dims(w: u32, h: u32, long: u32) -> (u32, u32) {
    let k = long as f64 / w.max(h) as f64;
    (((w as f64 * k).round() as u32).max(1), ((h as f64 * k).round() as u32).max(1))
}

enum Use {
    Copy(Arc<RenderImage>),
    Original,
    Wait,
}

/// The copy of the picture at `url` whose longer side is `long` pixels: made off the UI thread the first time, and
/// meanwhile the nearest size already made (else nothing, for a moment, rather than the jagged original).
fn copy(url: &str, long: u32, original: &Arc<Image>, (w, h): (u32, u32), sigma: f32, cx: &mut App) -> Use {
    let now = Instant::now();
    let images = cx.global_mut::<Images>();
    if !images.copies.contains_key(url) {
        images.copies.insert(url.to_string(), Vec::new());
    }
    let Some(copies) = images.copies.get_mut(url) else { return Use::Original };
    match copies.iter_mut().find(|c| c.long == long) {
        Some(c) => {
            c.used = now;
            match &c.state {
                CopyState::Ready(r) => return Use::Copy(r.clone()),
                CopyState::Failed => return Use::Original,
                CopyState::Loading => {}
            }
        }
        None => {
            copies.push(Smaller { long, state: CopyState::Loading, used: now });
            let bytes = original.clone();
            let (cw, ch) = copy_dims(w, h, long);
            let made = cx.background_executor().spawn(async move { shrink(&bytes.bytes, cw, ch, sigma) });
            let key = url.to_string();
            cx.spawn(async move |cx| {
                let made = made.await;
                let _ = cx.update(|cx| {
                    let images = cx.global_mut::<Images>();
                    // Let go meanwhile (its picture was): dropped with it.
                    let Some(c) = images.copies.get_mut(&key).and_then(|l| l.iter_mut().find(|c| c.long == long)) else { return };
                    c.state = match made {
                        Some(r) => CopyState::Ready(Arc::new(r)),
                        None => CopyState::Failed,
                    };
                    trim(cx, None);
                    cx.refresh_windows();
                });
            })
            .detach();
        }
    }
    let copies = cx.global_mut::<Images>().copies.get_mut(url);
    let nearest = copies.and_then(|l| {
        l.iter_mut().filter(|c| matches!(c.state, CopyState::Ready(_))).min_by_key(|c| c.long.abs_diff(long))
    });
    match nearest {
        Some(c) => {
            c.used = now;
            match &c.state {
                CopyState::Ready(r) => Use::Copy(r.clone()),
                _ => Use::Wait,
            }
        }
        None => Use::Wait,
    }
}

/// The own colour of the picture at an address, once it has loaded (asking starts the load).
pub fn tone(url: &str, cx: &mut App) -> Option<u32> {
    pic(url, cx)?;
    match cx.global::<Images>().slots.get(url) {
        Some(Slot::Ready(_, tone, _, _)) => *tone,
        _ => None,
    }
}

/// Lets go of the pictures not asked for longest once more than MAX_READY are kept, and starts the failed list over
/// past MAX_FAILED; the smaller copies the same way, and also those whose picture went, and those not drawn for
/// SUPERSEDED while another size of theirs was drawn since. Run outside any window's drawing (a picture's arrival), so
/// every window's atlas can be reached.
fn trim(cx: &mut App, asked: Option<Instant>) {
    let images = cx.global_mut::<Images>();
    // (The pictures away: every window drew after `asked`, so one last asked for more than AWAY before it is on none.)
    let away = |used: Instant| asked.is_some_and(|a| used + AWAY < a);
    let failed = images.slots.values().filter(|s| matches!(s, Slot::Failed(..))).count();
    if failed > MAX_FAILED {
        images.slots.retain(|_, s| !matches!(s, Slot::Failed(..)));
    }
    let now = Instant::now();
    let count = images.slots.values().filter(|s| matches!(s, Slot::Ready(..))).count();
    let mut gone: Vec<Arc<Image>> = Vec::new();
    if asked.is_some() {
        let keys: Vec<String> = images.slots.iter().filter_map(|(k, s)| matches!(s, Slot::Ready(_, _, used, _) if away(*used)).then(|| k.clone())).collect();
        for k in keys {
            if let Some(Slot::Ready(i, _, _, _)) = images.slots.remove(&k) {
                gone.push(i);
            }
        }
        let kept: std::collections::HashSet<u64> = images.slots.values().filter_map(|s| if let Slot::Ready(i, _, _, _) = s { Some(i.id) } else { None }).collect();
        gone.retain(|i| !kept.contains(&i.id));
    } else if count > MAX_READY {
        let mut idle: Vec<(Instant, String)> = images
            .slots
            .iter()
            .filter_map(|(k, s)| match s {
                Slot::Ready(_, _, used, _) if now.duration_since(*used) >= IDLE => Some((*used, k.clone())),
                _ => None,
            })
            .collect();
        idle.sort_unstable_by_key(|(used, _)| *used);
        gone = idle
            .iter()
            .take(count.saturating_sub(KEEP))
            .filter_map(|(_, k)| match images.slots.remove(k) {
                Some(Slot::Ready(i, _, _, _)) => Some(i),
                _ => None,
            })
            .collect();
        // Two addresses with the same bytes share one decoded picture in GPUI (its id is the bytes' hash): one still
        // kept keeps it drawn.
        let kept: std::collections::HashSet<u64> = images.slots.values().filter_map(|s| if let Slot::Ready(i, _, _, _) = s { Some(i.id) } else { None }).collect();
        gone.retain(|i| !kept.contains(&i.id));
    }
    // The copies. One whose picture isn't kept can't be drawn (pic_at needs the picture first).
    let mut gone_copies: Vec<Arc<RenderImage>> = Vec::new();
    let slots = &images.slots;
    images.copies.retain(|url, list| {
        let kept = matches!(slots.get(url.as_str()), Some(Slot::Ready(..)));
        let latest = list.iter().map(|c| c.used).max();
        list.retain_mut(|c| {
            if matches!(c.state, CopyState::Loading) {
                return true;
            }
            let idle = now.duration_since(c.used);
            let superseded = idle >= SUPERSEDED && latest.is_some_and(|l| l > c.used);
            if kept && !superseded && !away(c.used) {
                return true;
            }
            if let CopyState::Ready(r) = &c.state {
                gone_copies.push(r.clone());
            }
            false
        });
        !list.is_empty()
    });
    let ready = images.copies.values().flatten().filter(|c| matches!(c.state, CopyState::Ready(_))).count();
    if ready > MAX_READY {
        let mut idle: Vec<(Instant, String, u32)> = images
            .copies
            .iter()
            .flat_map(|(k, l)| l.iter().map(move |c| (k, c)))
            .filter(|(_, c)| matches!(c.state, CopyState::Ready(_)) && now.duration_since(c.used) >= IDLE)
            .map(|(k, c)| (c.used, k.clone(), c.long))
            .collect();
        idle.sort_unstable_by_key(|(used, _, _)| *used);
        for (_, k, long) in idle.into_iter().take(ready.saturating_sub(KEEP)) {
            if let Some(list) = images.copies.get_mut(&k)
                && let Some(at) = list.iter().position(|c| c.long == long)
                && let CopyState::Ready(r) = list.remove(at).state
            {
                gone_copies.push(r);
            }
        }
        images.copies.retain(|_, l| !l.is_empty());
    }
    if gone.is_empty() && gone_copies.is_empty() {
        return;
    }
    for window in cx.windows() {
        let _ = window.update(cx, |_, window, cx| {
            for i in &gone {
                if let Some(drawn) = i.clone().get_render_image(window, cx) {
                    let _ = window.drop_image(drawn);
                }
            }
            for r in &gone_copies {
                let _ = window.drop_image(r.clone());
            }
        });
    }
    for i in gone {
        i.remove_asset(cx);
    }
}

/// The picture at an address for a box of `w`×`h` logical px that it fills (ObjectFit::Cover; a square box for a
/// face or an icon), once it has loaded; asking starts the load. A picture much bigger than the box's pixels on this
/// window's screen is drawn from a copy made at those pixels (see `shrink`), kept per size; one near its size as it is.
pub fn pic_at(url: &str, w: f32, h: f32, cx: &mut App) -> Option<ImageSource> {
    let original = pic(url, cx)?;
    let dims = match cx.global::<Images>().slots.get(url) {
        Some(Slot::Ready(_, _, _, dims)) => *dims,
        _ => None,
    };
    let Some(dims) = dims else { return Some(ImageSource::Image(original)) };
    let key = url.to_string();
    let load = move |window: &mut Window, cx: &mut App| -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
        let scale = window.scale_factor();
        let made = match copy_long(dims.0, dims.1, w * scale, h * scale) {
            Some(long) => copy(&key, long, &original, dims, 0., cx),
            None => Use::Original,
        };
        match made {
            Use::Copy(r) => Some(Ok(r)),
            Use::Original => original.clone().use_render_image(window, cx).map(Ok),
            Use::Wait => None,
        }
    };
    Some(ImageSource::Custom(Arc::new(load)))
}

/// A loaded picture's size in pixels (None while it loads, or for an SVG or a GIF): what a box drawn at the
/// picture's own shape needs.
pub fn dims(url: &str, cx: &App) -> Option<(u32, u32)> {
    match cx.global::<Images>().slots.get(url) {
        Some(Slot::Ready(_, _, _, dims)) => *dims,
        _ => None,
    }
}

/// `pic_at`, with the picture itself (what kit::fade_in keys its fade on).
pub fn pic_pair(url: &str, w: f32, h: f32, cx: &mut App) -> Option<(ImageSource, Arc<Image>)> {
    let original = pic(url, cx)?;
    Some((pic_at(url, w, h, cx)?, original))
}

/// Whether the picture at an address could not be had (offline, gone, not a picture): a place kept for it can
/// shrink to a quiet line instead of standing empty at full size.
pub fn failed(url: &str, cx: &App) -> bool {
    match cx.global::<Images>().slots.get(url) {
        Some(Slot::Failed(..)) => true,
        Some(Slot::Loading(n, _)) => *n > 0,
        _ => false,
    }
}

/// `pic_at` for a square box (a face, an icon).
pub fn pic_sq(url: &str, size: f32, cx: &mut App) -> Option<ImageSource> {
    pic_at(url, size, size, cx)
}

/// The picture at an address, once it has loaded; asking starts the load.
pub fn pic(url: &str, cx: &mut App) -> Option<Arc<Image>> {
    if url.is_empty() {
        return None;
    }
    let images = cx.global_mut::<Images>();
    match images.slots.get_mut(url) {
        Some(Slot::Ready(i, _, used, _)) => {
            *used = Instant::now();
            return Some(i.clone());
        }
        Some(Slot::Loading(_, asked)) => {
            asked.store(ms_now(), Ordering::Relaxed);
            return None;
        }
        Some(Slot::Failed(at, n, gone)) if at.elapsed() < wait_after(*n, *gone) => return None,
        _ => {}
    }
    let fails = match images.slots.get(url) {
        Some(Slot::Failed(_, n, _)) => *n,
        _ => 0,
    };
    let asked = Arc::new(AtomicU64::new(ms_now()));
    images.slots.insert(url.to_string(), Slot::Loading(fails, asked.clone()));
    let (tx, rx) = oneshot::channel();
    if let Ok(mut jobs) = images.queue.jobs.lock() {
        jobs.push((url.to_string(), tx, asked));
    }
    images.queue.ready.notify_one();
    let key = url.to_string();
    cx.spawn(async move |cx| {
        let got = rx.await.unwrap_or(Got::Failed(false));
        let _ = cx.update(|cx| {
            let images = cx.global_mut::<Images>();
            let slot = match got {
                // Not asked for lately: let go, queued again when a draw asks.
                Got::Dropped => {
                    images.slots.remove(&key);
                    return;
                }
                Got::Bytes((b, tone, dims)) => match format_of(&b) {
                    Some(f) => Slot::Ready(Arc::new(Image::from_bytes(f, b)), tone, Instant::now(), dims),
                    None => Slot::Failed(Instant::now(), fails + 1, true),
                },
                Got::Failed(gone) => Slot::Failed(Instant::now(), fails + 1, gone),
            };
            images.slots.insert(key, slot);
            trim(cx, None);
            cx.refresh_windows();
        });
    })
    .detach();
    None
}

static USER: std::sync::OnceLock<Option<std::path::PathBuf>> = std::sync::OnceLock::new();

/// The app's folder of pictures the user chose (--pictures).
pub fn keep_user(dir: Option<std::path::PathBuf>) {
    let _ = USER.set(dir);
}

/// One of those pictures as the app names it in its state ("user/<file>"), when the file is there.
pub fn user(name: &str) -> Option<std::path::PathBuf> {
    let dir = USER.get()?.as_ref()?;
    let file = name.strip_prefix("user/").unwrap_or(name);
    if file.is_empty() || file.contains(['/', '\\']) || file.contains("..") {
        return None;
    }
    let path = dir.join(file);
    path.is_file().then_some(path)
}
