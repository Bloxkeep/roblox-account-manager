// The song's beat, for the profile that moves with its music: the sound the music app (or Bloxkeep's own profile song)
// sends to the speakers, read through Windows' process loopback (only that program and the ones it started, so a game's
// sound never counts), cut into bands on one small thread. Where Windows has no process loopback (before Windows 10
// 2004), what all the speakers play is read instead.
//
// It runs only while a profile with its beat on is drawn and a song plays: the drawing asks (`listen`) each frame, and
// the thread ends 1.5 s after the last ask, letting go of Windows' sound. What it keeps: 4,096 samples, a 1,024-point
// FFT for the bands and a 4,096-point one for the bass's own curve (fine enough there: 12 Hz a step), about 120 KB in
// all; a look about 90 times a second.
#![cfg_attr(not(any(feature = "core", feature = "shot")), allow(dead_code))]

use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::media::Player;
use crate::threads::Hold;

/// The bands, low to high (40 Hz to 14 kHz, evenly on a log scale).
pub const BANDS: usize = 32;

/// The points of the bass's curve, 25 to 160 Hz (what the ring round the picture draws, as Trap Nation's videos do).
pub const LOW: usize = 40;

/// What the song does now, each 0 to 1.
#[derive(Clone, Copy)]
pub struct Beat {
    pub bands: [f32; BANDS],
    /// The bass's curve, low to high: it fills its range quickly, so it leaps.
    pub low: [f32; LOW],
    /// How hard the bass hits now (eased so a soft one still shows): what the picture swells and the sparks fly by.
    pub punch: f32,
    pub bass: f32,
    pub mid: f32,
    pub high: f32,
    pub level: f32,
    /// 1 at a beat (a kick of the bass), falling to 0 in about a third of a second.
    pub kick: f32,
    /// The beats counted so far: a new one starts a burst (and even and odd ones take turns).
    pub beats: u32,
    /// Seconds since the last beat, and the time between beats of late (half a second until it is known).
    pub since: f32,
    pub gap: f32,
    /// How far the song is in a loud part (a chorus, a drop: what osu! calls kiai), 0 to 1: louder than its last
    /// quarter of a minute, eased in and out.
    pub drop: f32,
}

const QUIET: Beat = Beat { bands: [0.; BANDS], low: [0.; LOW], punch: 0., bass: 0., mid: 0., high: 0., level: 0., kick: 0., beats: 0, since: 9., gap: 0.5, drop: 0. };

impl Default for Beat {
    fn default() -> Self {
        QUIET
    }
}

/// Whose sound: the music app's, or Bloxkeep's own (the profile's song file plays in Bloxkeep).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Own,
    App(Player),
}

struct Shared {
    want: Option<(Source, Instant)>,
    running: bool,
    out: Beat,
    beat_at: Option<Instant>,
    gap: f32,
}

static SHARED: Mutex<Shared> = Mutex::new(Shared { want: None, running: false, out: QUIET, beat_at: None, gap: 0.5 });

/// The song's beat now, from `src`'s sound; asked each frame it is drawn (the reading starts on the first ask and stops
/// a moment after the last).
pub fn listen(src: Source) -> Beat {
    #[cfg(feature = "shot")]
    if std::env::var("BLOXKEEP_SHOT_BEAT").is_ok() {
        return made_up();
    }
    let mut s = SHARED.hold();
    s.want = Some((src, Instant::now()));
    if !s.running {
        s.running = true;
        #[cfg(all(windows, any(feature = "core", feature = "shot")))]
        if crate::threads::own("Song beat", win::run).is_err() {
            s.running = false;
        }
    }
    let mut b = s.out;
    b.since = s.beat_at.map_or(9., |t| t.elapsed().as_secs_f32());
    b.kick = (-b.since / 0.16).exp();
    b.gap = s.gap;
    b
}

/// A song made up for the screen shots: 124 beats a minute, the bands rolling.
#[cfg(feature = "shot")]
fn made_up() -> Beat {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| (d.as_millis() % 1_000_000) as f32 / 1000.).unwrap_or(0.);
    let t = std::env::var("BLOXKEEP_SHOT_BEAT").ok().and_then(|v| v.parse::<f32>().ok()).filter(|v| *v > 0.).unwrap_or(t);
    let period = 60. / 124.;
    let since = t.rem_euclid(period);
    let kick = (-since / 0.16).exp();
    let mut bands = [0f32; BANDS];
    for (i, b) in bands.iter_mut().enumerate() {
        let u = i as f32 / BANDS as f32;
        let low = (1. - u * 2.5).max(0.) * (0.45 + 0.55 * kick);
        let roll = 0.5 + 0.5 * (t * 3.1 + i as f32 * 0.7).sin() * (t * 1.7 + i as f32 * 0.31).cos();
        *b = (low + (0.55 - u * 0.35) * roll).clamp(0., 1.);
    }
    let mut low = [0f32; LOW];
    for (i, v) in low.iter_mut().enumerate() {
        let u = i as f32 / LOW as f32;
        *v = ((0.3 + 0.65 * kick) * (1. - u * 0.55) * (0.78 + 0.22 * (t * 5. + i as f32 * 0.45).sin())).clamp(0., 1.);
    }
    let avg = |r: std::ops::Range<usize>| bands[r.clone()].iter().sum::<f32>() / r.len() as f32;
    // (A loud part from the 8th second of each 16.)
    let drop = if t.rem_euclid(16.) >= 8. { 1. } else { 0. };
    Beat { bass: avg(0..6), mid: avg(6..19), high: avg(19..BANDS), level: avg(0..BANDS), kick, beats: (t / period) as u32, bands, punch: punch(&low), low, since, gap: period, drop }
}

/// How hard the bass's curve hits: its mean eased up (a soft bass still shows), 0 to 1.
fn punch(low: &[f32; LOW]) -> f32 {
    let x = low.iter().sum::<f32>() / LOW as f32;
    (5. * (1.2 * x - x.powf(1.2))).clamp(0., 1.).powf(0.8)
}

/// The bands from a frame of sound (`ring`, oldest first, already windowed into `re`): see `Ear`.
struct Ear {
    rate: f32,
    win: Vec<f32>,
    re: Vec<f32>,
    im: Vec<f32>,
    edges: [(usize, usize); BANDS],
    peak: f32,
    smooth: [f32; BANDS],
    history: [f32; 43],
    hist_at: usize,
    last_beat: Option<Instant>,
    big_win: Vec<f32>,
    big_re: Vec<f32>,
    big_im: Vec<f32>,
    low_peak: f32,
    low_smooth: [f32; LOW],
    loud_now: f32,
    loud_long: f32,
    drop: f32,
}

const N: usize = 1024;
/// The samples kept, and the bass's FFT.
const RING: usize = 4096;

impl Ear {
    fn new(rate: f32) -> Ear {
        let win = (0..N).map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / (N - 1) as f32).cos()).collect();
        let bin = rate / N as f32;
        let mut edges = [(1, 2); BANDS];
        let (lo, hi) = (40f32, 14_000f32.min(rate / 2. - bin));
        for (b, e) in edges.iter_mut().enumerate() {
            let f0 = lo * (hi / lo).powf(b as f32 / BANDS as f32);
            let f1 = lo * (hi / lo).powf((b + 1) as f32 / BANDS as f32);
            let a = ((f0 / bin).floor() as usize).max(1);
            *e = (a, ((f1 / bin).ceil() as usize).max(a + 1).min(N / 2));
        }
        let big_win = (0..RING).map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / (RING - 1) as f32).cos()).collect();
        Ear { rate, win, re: vec![0.; N], im: vec![0.; N], edges, peak: 1e-4, smooth: [0.; BANDS], history: [0.; 43], hist_at: 0, last_beat: None, big_win, big_re: vec![0.; RING], big_im: vec![0.; RING], low_peak: 1e-4, low_smooth: [0.; LOW], loud_now: -99., loud_long: -99., drop: 0. }
    }

    /// One look at the samples (`ring`, RING of them, the oldest at `at`): the bands from the last N, the bass's curve
    /// from all, and a beat when the bass jumps.
    fn look(&mut self, ring: &[f32], at: usize) -> (Beat, bool) {
        for i in 0..N {
            self.re[i] = ring[(at + RING - N + i) % RING] * self.win[i];
            self.im[i] = 0.;
        }
        fft(&mut self.re, &mut self.im);
        let mag = |i: usize, re: &[f32], im: &[f32]| (re[i] * re[i] + im[i] * im[i]).sqrt();
        let mut raw = [0f32; BANDS];
        for (b, &(a, z)) in self.edges.iter().enumerate() {
            raw[b] = (a..z).map(|i| mag(i, &self.re, &self.im)).fold(0., f32::max);
        }
        let top = raw.iter().copied().fold(0., f32::max);
        // A loud part: the song's loudness (before it is made to fill the range below) over a second, against the
        // last 15 s; 2.5 dB more is a chorus or a drop. Looks come about 90 times a second.
        let loud = 20. * raw.iter().sum::<f32>().max(1e-9).log10();
        if self.loud_long < -98. {
            (self.loud_now, self.loud_long) = (loud, loud);
        }
        self.loud_now += (loud - self.loud_now) * 0.02;
        self.loud_long += (loud - self.loud_long) * 0.0008;
        let up = if self.loud_now > self.loud_long + 2.5 && top > 1e-3 { 1. } else { 0. };
        self.drop += (up - self.drop) * if up > self.drop { 0.03 } else { 0.008 };
        // (Loud or quiet, the song fills the range: the loudest band of the last few seconds is the top.)
        self.peak = (self.peak * 0.997).max(top).max(1e-4);
        let mut out = Beat::default();
        for b in 0..BANDS {
            let db = 20. * (raw[b] / self.peak).max(1e-6).log10();
            let v = ((db + 48.) / 48.).clamp(0., 1.);
            let s = &mut self.smooth[b];
            *s += (v - *s) * if v > *s { 0.6 } else { 0.14 };
            out.bands[b] = *s;
        }
        // The bass's curve: 25 to 160 Hz, read between the big FFT's steps, measured against its own loudest of late
        // over a 15 dB window (it fills quickly and leaps, as Trap Nation's does), quick to rise and to fall.
        for i in 0..RING {
            self.big_re[i] = ring[(at + i) % RING] * self.big_win[i];
            self.big_im[i] = 0.;
        }
        fft(&mut self.big_re, &mut self.big_im);
        let step = self.rate / RING as f32;
        let mut raw = [0f32; LOW];
        for (p, r) in raw.iter_mut().enumerate() {
            let at_bin = 25. * (160f32 / 25.).powf(p as f32 / (LOW - 1) as f32) / step;
            let (b0, u) = (at_bin.floor() as usize, at_bin.fract());
            *r = mag(b0, &self.big_re, &self.big_im) * (1. - u) + mag(b0 + 1, &self.big_re, &self.big_im) * u;
        }
        let top = raw.iter().copied().fold(0., f32::max);
        self.low_peak = (self.low_peak * 0.998).max(top).max(1e-4);
        for p in 0..LOW {
            let db = 20. * (raw[p] / self.low_peak).max(1e-6).log10();
            let v = ((db + 15.) / 15.).clamp(0., 1.);
            let s = &mut self.low_smooth[p];
            *s += (v - *s) * if v > *s { 0.75 } else { 0.3 };
        }
        for p in 0..LOW {
            let (a, b) = (p.saturating_sub(1), (p + 1).min(LOW - 1));
            out.low[p] = (self.low_smooth[a] + self.low_smooth[p] + self.low_smooth[b]) / 3.;
        }
        out.punch = punch(&out.low);
        out.drop = self.drop;
        let avg = |r: std::ops::Range<usize>| out.bands[r.clone()].iter().sum::<f32>() / r.len() as f32;
        out.bass = avg(0..6);
        out.mid = avg(6..19);
        out.high = avg(19..BANDS);
        out.level = avg(0..BANDS);
        // A beat: the bass's strength well over its last half second's, and not too soon after the last.
        let bin = self.rate / N as f32;
        let energy: f32 = (1..((160. / bin).ceil() as usize).max(2)).map(|i| mag(i, &self.re, &self.im).powi(2)).sum::<f32>() / (self.peak * self.peak);
        let mean = self.history.iter().sum::<f32>() / self.history.len() as f32;
        self.history[self.hist_at] = energy;
        self.hist_at = (self.hist_at + 1) % self.history.len();
        let beat = energy > mean * 1.45 && energy > 0.02 && out.bass > 0.3 && self.last_beat.is_none_or(|t| t.elapsed() > Duration::from_millis(230));
        if beat {
            self.last_beat = Some(Instant::now());
        }
        (out, beat)
    }
}

/// A radix-2 FFT in place (N a power of two).
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -std::f32::consts::TAU / len as f32;
        let (ws, wc) = ang.sin_cos();
        for start in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1f32, 0f32);
            for k in 0..len / 2 {
                let (a, b) = (start + k, start + k + len / 2);
                let (tr, ti) = (re[b] * cr - im[b] * ci, re[b] * ci + im[b] * cr);
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
                (cr, ci) = (cr * wc - ci * ws, cr * ws + ci * wc);
            }
        }
        len <<= 1;
    }
}

/// What the thread is asked for now: the source, or None when nobody asked for 1.5 s (it ends, and says so).
fn wanted() -> Option<Source> {
    let mut s = SHARED.hold();
    match s.want {
        Some((src, at)) if at.elapsed() < Duration::from_millis(1500) => Some(src),
        _ => {
            s.running = false;
            s.out = Beat::default();
            s.beat_at = None;
            s.gap = 0.5;
            None
        }
    }
}

fn give(b: Beat, beat: bool) {
    let mut s = SHARED.hold();
    let beats = s.out.beats + u32::from(beat);
    s.out = Beat { beats, ..b };
    if beat {
        // (The time between beats of late, from ones that come a quarter to one and a half seconds apart.)
        if let Some(d) = s.beat_at.map(|t| t.elapsed().as_secs_f32()).filter(|d| (0.25..1.5).contains(d)) {
            s.gap += (d - s.gap) * 0.2;
        }
        s.beat_at = Some(Instant::now());
    }
}

#[cfg(all(windows, any(feature = "core", feature = "shot")))]
mod win {
    use super::*;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Media::Audio::*;
    use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
    use windows::Win32::System::Com::{BLOB, CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree};
    use windows::Win32::System::Variant::VT_BLOB;
    use windows::core::{Interface, implement};
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{CreateEventW, SetEvent, WaitForSingleObject};

    /// A Windows event, closed when dropped.
    struct Event(windows_sys::Win32::Foundation::HANDLE);
    impl Event {
        fn new() -> Option<Event> {
            let h = unsafe { CreateEventW(std::ptr::null(), 0, 0, std::ptr::null()) };
            (!h.is_null()).then_some(Event(h))
        }
    }
    impl Drop for Event {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.0) };
        }
    }

    /// Told when Windows has made the process loopback's sound client.
    #[implement(IActivateAudioInterfaceCompletionHandler)]
    struct Made(usize);
    impl IActivateAudioInterfaceCompletionHandler_Impl for Made_Impl {
        fn ActivateCompleted(&self, _: windows::core::Ref<'_, IActivateAudioInterfaceAsyncOperation>) -> windows::core::Result<()> {
            unsafe { SetEvent(self.0 as _) };
            Ok(())
        }
    }

    /// A sound client that reads `pid`'s sound (and its children's), as 48 kHz float stereo.
    fn process_client(pid: u32) -> windows::core::Result<(IAudioClient, f32, u16, bool)> {
        let params = AUDIOCLIENT_ACTIVATION_PARAMS {
            ActivationType: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
            Anonymous: AUDIOCLIENT_ACTIVATION_PARAMS_0 { ProcessLoopbackParams: AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS { TargetProcessId: pid, ProcessLoopbackMode: PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE } },
        };
        let mut pv = PROPVARIANT::default();
        // (The blob points at `params`, alive till the end; the PROPVARIANT owns nothing and is never cleared.)
        unsafe {
            let inner = &mut *pv.Anonymous.Anonymous;
            inner.vt = VT_BLOB;
            inner.Anonymous.blob = BLOB { cbSize: std::mem::size_of::<AUDIOCLIENT_ACTIVATION_PARAMS>() as u32, pBlobData: &params as *const _ as *mut u8 };
        }
        let done = Event::new().ok_or_else(windows::core::Error::from_win32)?;
        let handler: IActivateAudioInterfaceCompletionHandler = Made(done.0 as usize).into();
        let op = unsafe { ActivateAudioInterfaceAsync(VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, &IAudioClient::IID, Some(&pv), &handler)? };
        unsafe { WaitForSingleObject(done.0, 5000) };
        let mut hr = windows::core::HRESULT(0);
        let mut made: Option<windows::core::IUnknown> = None;
        unsafe { op.GetActivateResult(&mut hr, &mut made)? };
        hr.ok()?;
        let client: IAudioClient = made.ok_or_else(windows::core::Error::empty)?.cast()?;
        let fmt = WAVEFORMATEX { wFormatTag: 3, nChannels: 2, nSamplesPerSec: 48_000, nAvgBytesPerSec: 48_000 * 8, nBlockAlign: 8, wBitsPerSample: 32, cbSize: 0 };
        unsafe { client.Initialize(AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_EVENTCALLBACK | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM, 200_000, 0, &fmt, None)? };
        Ok((client, 48_000., 2, true))
    }

    /// What all the speakers play (Windows without process loopback): its own mix's format.
    fn speakers_client() -> windows::core::Result<(IAudioClient, f32, u16, bool)> {
        unsafe {
            let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let client: IAudioClient = devices.GetDefaultAudioEndpoint(eRender, eConsole)?.Activate(CLSCTX_ALL, None)?;
            let fmt = client.GetMixFormat()?;
            let (rate, ch, float) = ((*fmt).nSamplesPerSec as f32, (*fmt).nChannels, (*fmt).wBitsPerSample == 32);
            let done = client.Initialize(AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, 200_000, 0, fmt, None);
            CoTaskMemFree(Some(fmt as *const _));
            done?;
            Ok((client, rate, ch, float))
        }
    }

    fn say(what: &str, e: &dyn std::fmt::Display) {
        static SAID: Mutex<String> = Mutex::new(String::new());
        let line = format!("Song beat, {what}: {e}");
        let mut said = SAID.hold();
        if *said != line {
            #[cfg(feature = "core")]
            crate::core::log::write(&line);
            *said = line;
        }
    }

    pub(super) fn run() {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        let mut no_process_loopback = false;
        while let Some(src) = wanted() {
            let pid = match src {
                Source::Own => Some(std::process::id()),
                Source::App(p) => crate::media::pid_of(p),
            };
            let Some(pid) = pid else {
                std::thread::sleep(Duration::from_millis(500));
                continue;
            };
            let client = if no_process_loopback { speakers_client() } else { process_client(pid) };
            let client = match client {
                Ok(c) => c,
                Err(e) if !no_process_loopback => {
                    say("reading the music app's own sound (the speakers' instead)", &e);
                    no_process_loopback = true;
                    continue;
                }
                Err(e) => {
                    say("reading the speakers", &e);
                    std::thread::sleep(Duration::from_secs(2));
                    continue;
                }
            };
            if let Err(e) = read(client, src) {
                say("reading", &e);
                std::thread::sleep(Duration::from_millis(500));
            }
        }
    }

    /// Reads the client's sound until it isn't wanted (or another source is), a look each 512 samples.
    fn read((client, rate, channels, float): (IAudioClient, f32, u16, bool), src: Source) -> windows::core::Result<()> {
        let tick = Event::new().ok_or_else(windows::core::Error::from_win32)?;
        let evented = unsafe { client.SetEventHandle(HANDLE(tick.0)) }.is_ok();
        let capture: IAudioCaptureClient = unsafe { client.GetService()? };
        unsafe { client.Start()? };
        let mut ear = Ear::new(rate);
        let mut ring = vec![0f32; RING];
        let (mut at, mut fresh) = (0usize, 0usize);
        let mut asked = Instant::now();
        let mut heard = Instant::now();
        let channels = channels.max(1) as usize;
        let result = loop {
            if evented {
                unsafe { WaitForSingleObject(tick.0, 60) };
            } else {
                std::thread::sleep(Duration::from_millis(10));
            }
            loop {
                let size = match unsafe { capture.GetNextPacketSize() } {
                    Ok(s) => s,
                    Err(_) => break,
                };
                if size == 0 {
                    break;
                }
                let (mut data, mut frames, mut flags) = (std::ptr::null_mut(), 0u32, 0u32);
                if unsafe { capture.GetBuffer(&mut data, &mut frames, &mut flags, None, None) }.is_err() {
                    break;
                }
                let silent = flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || data.is_null();
                for f in 0..frames as usize {
                    let mut v = 0f32;
                    if !silent {
                        for c in 0..channels {
                            let i = f * channels + c;
                            v += unsafe { if float { *(data as *const f32).add(i) } else { *(data as *const i16).add(i) as f32 / 32768. } };
                        }
                    }
                    ring[at] = v / channels as f32;
                    at = (at + 1) % RING;
                    fresh += 1;
                }
                unsafe { capture.ReleaseBuffer(frames)? };
                heard = Instant::now();
            }
            // (No sound comes while the song is paused: the bands fall as silence would.)
            if heard.elapsed() > Duration::from_millis(40) {
                for _ in 0..(rate * 0.012) as usize {
                    ring[at] = 0.;
                    at = (at + 1) % RING;
                    fresh += 1;
                }
                heard = Instant::now();
            }
            while fresh >= 512 {
                fresh -= 512;
                let (b, beat) = ear.look(&ring, at);
                give(b, beat);
            }
            if asked.elapsed() > Duration::from_millis(100) {
                asked = Instant::now();
                if wanted() != Some(src) {
                    break Ok(());
                }
            }
        };
        unsafe {
            let _ = client.Stop();
        }
        result
    }
}
