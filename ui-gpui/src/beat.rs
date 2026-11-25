// The song's beat, for the profile that moves with its music: the sound the music app (or Bloxkeep's own profile song)
// sends to the speakers, read through Windows' process loopback (only that program and the ones it started, so a game's
// sound never counts), cut into bands on one small thread. Where Windows has no process loopback (before Windows 10
// 2004), what all the speakers play is read instead.
//
// It runs only while a profile with its beat on is drawn and a song plays: the drawing asks (`listen`) each frame, and
// the thread ends 1.5 s after the last ask, letting go of Windows' sound. What it keeps: 4,096 samples, a 1,024-point
// FFT for the bands and the beat and a 4,096-point one for the bass's own curve (fine enough there: 12 Hz a step), the
// beat's last 5.5 s, about 140 KB in all; a look about 90 times a second. What a look finds is in beat/ear.rs.
#![cfg_attr(not(any(feature = "core", feature = "shot")), allow(dead_code))]

use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::media::Player;
use crate::threads::Hold;

mod ear;

pub use ear::{BANDS, Beat, LOW};
use ear::QUIET;
#[cfg(feature = "shot")]
use ear::punch;

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
    // (The time between beats, while the song has a steady pulse; the last one kept through a soft passage.)
    if b.gap > 0. {
        s.gap = b.gap;
    }
    if beat {
        s.beat_at = Some(Instant::now());
    }
}

#[cfg(all(windows, any(feature = "core", feature = "shot")))]
mod win {
    use super::ear::{Ear, HOP, RING};
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
        let params = Box::new(params);
        // The blob points at `params`, which the PROPVARIANT doesn't own: it is never cleared. (Dropped, the windows
        // crate's PROPVARIANT clears itself, and a blob's clearing frees its data with CoTaskMemFree: memory that
        // was never COM's, and the heap was broken at once. Every Bloxkeep crashed as a profile with its beat on was
        // shown with a song playing.)
        let mut pv = std::mem::ManuallyDrop::new(PROPVARIANT::default());
        unsafe {
            let inner = &mut *pv.Anonymous.Anonymous;
            inner.vt = VT_BLOB;
            inner.Anonymous.blob = BLOB { cbSize: std::mem::size_of::<AUDIOCLIENT_ACTIVATION_PARAMS>() as u32, pBlobData: &*params as *const _ as *mut u8 };
        }
        let done = Event::new().ok_or_else(windows::core::Error::from_win32)?;
        let handler: IActivateAudioInterfaceCompletionHandler = Made(done.0 as usize).into();
        let op = unsafe { ActivateAudioInterfaceAsync(VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, &IAudioClient::IID, Some(&*pv), &handler)? };
        if unsafe { WaitForSingleObject(done.0, 5000) } != 0 {
            // Windows hasn't answered: it may still read the parameters and set the event, so neither goes.
            std::mem::forget(params);
            std::mem::forget(done);
            return Err(windows::core::Error::from_hresult(windows::core::HRESULT(0x8000_000Au32 as i32)));
        }
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

    /// How long after the music app's sound is read the speakers play it: it waits a period for Windows' mix, a period
    /// in the speakers' buffer and a period in the device, and the speakers' own latency as Windows gives it. (Measured
    /// on a screen recording with a 10 ms period, the bars came 17 to 34 ms before the speakers' sound.) The looks are
    /// held that long, so the profile moves when the sound is heard, not before.
    fn speakers_late() -> Duration {
        unsafe {
            let read = || -> windows::core::Result<i64> {
                let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
                let client: IAudioClient = devices.GetDefaultAudioEndpoint(eRender, eConsole)?.Activate(CLSCTX_ALL, None)?;
                let (mut period, mut least) = (0i64, 0i64);
                client.GetDevicePeriod(Some(&mut period), Some(&mut least))?;
                let fmt = client.GetMixFormat()?;
                let done = client.Initialize(AUDCLNT_SHAREMODE_SHARED, 0, 0, 0, fmt, None);
                CoTaskMemFree(Some(fmt as *const _));
                done?;
                Ok(client.GetStreamLatency()? + 3 * period)
            };
            match read() {
                // (In 100 ns steps; never more than a third of a second, whatever a driver says.)
                Ok(t) => Duration::from_nanos((t.max(0) as u64).saturating_mul(100)).min(Duration::from_millis(330)),
                Err(e) => {
                    say("reading the speakers' delay", &e);
                    Duration::ZERO
                }
            }
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
            if let Err(e) = read(client, src, pid) {
                say("reading", &e);
                std::thread::sleep(Duration::from_millis(500));
            }
        }
    }

    /// Reads the client's sound until it isn't wanted (or another source is), a look each HOP samples; or, reading the
    /// music app's program `pid`, until only silence came for 2 s while another of its programs sounds (it is read
    /// from then).
    fn read((made, rate, channels, float): (IAudioClient, f32, u16, bool), src: Source, pid: u32) -> windows::core::Result<()> {
        let tick = Event::new().ok_or_else(windows::core::Error::from_win32)?;
        // (Held after the event, so it is let go before the event is closed, however this ends: Windows signals the
        // event it was given until the client is gone.)
        let client = made;
        let evented = unsafe { client.SetEventHandle(HANDLE(tick.0)) }.is_ok();
        let capture: IAudioCaptureClient = unsafe { client.GetService()? };
        unsafe { client.Start()? };
        let mut ear = Ear::new(rate);
        // The looks are held till the speakers play their sound (the music app's own sound is read before Windows
        // mixes it).
        let late = speakers_late();
        say("held for the speakers", &format_args!("{} ms", late.as_millis()));
        // (About 90 looks a second wait, a few dozen at most.)
        let mut held: std::collections::VecDeque<(Instant, Beat, bool)> = std::collections::VecDeque::with_capacity(32);
        let mut ring = vec![0f32; RING];
        let (mut at, mut fresh) = (0usize, 0usize);
        let mut asked = Instant::now();
        let mut heard = Instant::now();
        let (mut sounded, mut looked) = (Instant::now(), Instant::now());
        let channels = channels.max(1) as usize;
        let result = loop {
            if evented {
                unsafe { WaitForSingleObject(tick.0, 60) };
            } else {
                std::thread::sleep(Duration::from_millis(10));
            }
            if drain(&capture, channels, float, rate, |_, v| {
                if v != 0. {
                    sounded = Instant::now();
                }
                ring[at] = v;
                at = (at + 1) % RING;
                fresh += 1;
            })? {
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
            while fresh >= HOP {
                fresh -= HOP;
                let (b, beat) = ear.look(&ring, at);
                held.push_back((Instant::now(), b, beat));
            }
            while held.front().is_some_and(|(t, _, _)| t.elapsed() >= late) {
                if let Some((_, b, beat)) = held.pop_front() {
                    give(b, beat);
                }
            }
            if asked.elapsed() > Duration::from_millis(100) {
                asked = Instant::now();
                if wanted() != Some(src) {
                    break Ok(());
                }
            }
            // (The song plays, as the profile asks only then: silence this long is another program's sound.)
            if let Source::App(player) = src
                && sounded.elapsed() > Duration::from_secs(2)
                && looked.elapsed() > Duration::from_secs(1)
            {
                looked = Instant::now();
                if crate::media::pid_of(player).is_some_and(|p| p != pid) {
                    break Ok(());
                }
            }
        };
        unsafe {
            let _ = client.Stop();
        }
        result
    }

    /// Takes all the sound waiting: each sample (its channels mixed to one) with when Windows took it (100 ns steps, 0
    /// when it doesn't say). Whether any came.
    fn drain(capture: &IAudioCaptureClient, channels: usize, float: bool, rate: f32, mut each: impl FnMut(u64, f32)) -> windows::core::Result<bool> {
        let mut any = false;
        loop {
            // (An error, as the speakers gone, ends the read: a new client is made.)
            let size = unsafe { capture.GetNextPacketSize()? };
            if size == 0 {
                break;
            }
            let (mut data, mut frames, mut flags, mut taken) = (std::ptr::null_mut(), 0u32, 0u32, 0u64);
            unsafe { capture.GetBuffer(&mut data, &mut frames, &mut flags, None, Some(&mut taken))? };
            let silent = flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || data.is_null();
            for f in 0..frames as usize {
                let mut v = 0f32;
                if !silent {
                    for c in 0..channels {
                        let i = f * channels + c;
                        v += unsafe { if float { *(data as *const f32).add(i) } else { *(data as *const i16).add(i) as f32 / 32768. } };
                    }
                }
                each(if taken == 0 { 0 } else { taken + (f as f64 * 1e7 / rate as f64) as u64 }, v / channels as f32);
            }
            unsafe { capture.ReleaseBuffer(frames)? };
            any = true;
        }
        Ok(any)
    }
}
