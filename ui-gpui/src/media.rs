// The song a music app plays (Spotify, Apple Music, YouTube Music's own app), for the status bar and the profile:
// read from Windows' own media controls (the ones over the volume flyout), so no sign-in, no key, and it works for
// free plans too. Play/pause, next and back go the same way. Only music apps' sessions: a video in a browser tab
// isn't "the song" (a browser's session says only that the browser plays, not which site).
#![cfg_attr(not(any(feature = "core", feature = "shot")), allow(dead_code))]

/// The music app a song comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Player {
    Spotify,
    AppleMusic,
    YouTubeMusic,
}

impl Player {
    /// Which app a media session is, from the app id Windows gives it (lower case). Spotify: Spotify.exe, or the
    /// Microsoft Store one (SpotifyAB.SpotifyMusic_…!Spotify). Apple Music: the Microsoft Store app
    /// (AppleInc.AppleMusicWin_…, in Windows' media controls since its 2023 preview; its exact id not yet seen on a
    /// real PC). YouTube Music has no app of Google's on Windows: its page installed as an app from Chrome or Edge
    /// (Chrome's id for it, cinhimbnkkaeohfgghhklpknlkffjgod, when the browser names the session after it; not seen
    /// on a real PC yet), or th-ch's YouTube Music app (com.github.th-ch.youtube-music).
    pub fn of(id: &str) -> Option<Player> {
        if id.contains("spotify") {
            Some(Player::Spotify)
        } else if id.contains("applemusic") || id.contains("apple music") {
            Some(Player::AppleMusic)
        } else if id.contains("youtube-music") || id.contains("youtubemusic") || id.contains("youtube music") || id.contains("cinhimbnkkaeohfgghhklpknlkffjgod") {
            Some(Player::YouTubeMusic)
        } else {
            None
        }
    }

    /// Its name, as the app calls itself.
    pub fn name(self) -> &'static str {
        match self {
            Player::Spotify => "Spotify",
            Player::AppleMusic => "Apple Music",
            Player::YouTubeMusic => "YouTube Music",
        }
    }

    /// Its mark (assets/icons, from Simple Icons, CC0).
    pub fn icon(self) -> &'static str {
        match self {
            Player::Spotify => "spotify",
            Player::AppleMusic => "appleMusic",
            Player::YouTubeMusic => "youtubeMusic",
        }
    }

    /// Its mark's own colour, as the app shows it (Spotify's green, Apple Music's red, YouTube Music's red).
    pub fn colour(self) -> u32 {
        match self {
            Player::Spotify => 0x1ED760,
            Player::AppleMusic => 0xFA243C,
            Player::YouTubeMusic => 0xFF0033,
        }
    }

    /// Its program's sound session in Windows' mixer (part of its name, lower case), for its loudness.
    #[cfg_attr(not(all(windows, any(feature = "core", feature = "shot"))), allow(dead_code))]
    fn program(self) -> &'static str {
        match self {
            Player::Spotify => "spotify",
            Player::AppleMusic => "applemusic",
            Player::YouTubeMusic => "youtube music",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Playing {
    /// The app it plays in.
    pub player: Player,
    pub title: String,
    pub artist: String,
    pub playing: bool,
    /// The album's picture, saved as a file the picture loader reads (a path), once Windows gave one.
    pub cover: Option<String>,
    /// How loud the music app itself is set in Windows' mixer, 0 to 1 (None when it has no sound session there yet).
    pub volume: Option<f32>,
    /// Where the song was when this was read and how long it is, in ms (None when Spotify says neither), and when
    /// that was: while it plays, where it is now is counted on from then (`at`).
    pub position: Option<(u64, u64)>,
    pub read: std::time::Instant,
}

/// The same song in the same state (when it was read is not part of that).
impl PartialEq for Playing {
    fn eq(&self, o: &Self) -> bool {
        self.player == o.player && self.title == o.title && self.artist == o.artist && self.playing == o.playing && self.cover == o.cover && self.volume == o.volume && self.position == o.position
    }
}

impl Playing {
    /// Where the song is now and how long it is, in ms.
    pub fn at(&self) -> Option<(u64, u64)> {
        let (at, length) = self.position?;
        let on = if self.playing { self.read.elapsed().as_millis() as u64 } else { 0 };
        Some(((at + on).min(length), length))
    }

    /// The song as it will be once Spotify has gone to `ms` (shown at once; Spotify's own answer follows).
    pub fn sought(&mut self, ms: u64) {
        if let Some((_, length)) = self.position {
            self.position = Some((ms.min(length), length));
            self.read = std::time::Instant::now();
        }
    }
}

#[derive(Clone, Copy)]
pub enum Act {
    Toggle,
    Next,
    Back,
}

/// A stand-in for the hidden-window shots: BLOXKEEP_SHOT_MUSIC="Title|Artist[|paused]", its app from
/// BLOXKEEP_SHOT_MUSIC_APP (spotify, applemusic or youtubemusic; Spotify when unset).
pub fn shot_stand_in() -> Option<Playing> {
    let v = std::env::var("BLOXKEEP_SHOT_MUSIC").ok()?;
    let mut parts = v.split('|');
    let title = parts.next()?.to_string();
    let artist = parts.next().unwrap_or_default().to_string();
    let playing = parts.next() != Some("paused");
    let player = std::env::var("BLOXKEEP_SHOT_MUSIC_APP").ok().and_then(|a| Player::of(&a.to_lowercase())).unwrap_or(Player::Spotify);
    Some(Playing { player, title, artist, playing, cover: std::env::var("BLOXKEEP_SHOT_MUSIC_COVER").ok(), volume: Some(0.7), position: Some((63_000, 145_000)), read: std::time::Instant::now() })
}

#[cfg(all(windows, any(feature = "core", feature = "shot")))]
mod win {
    use super::{Act, Player, Playing};
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};
    use windows::Foundation::TypedEventHandler;
    use windows::core::RuntimeType;
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSession as Session, GlobalSystemMediaTransportControlsSessionManager as Manager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
    };
    use windows::Storage::Streams::DataReader;
    use windows::Win32::Media::Audio::Endpoints::IAudioMeterInformation;
    use windows::Win32::Media::Audio::{AudioSessionStateExpired, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator, eMultimedia, eRender};
    use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance, CoTaskMemFree};
    use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
    use windows::core::Interface;

    /// The last song's key and its picture's file, so the picture is read once per song, not every few seconds.
    /// The song's picture saved (None: none read yet) and when it was last asked for.
    static COVER: Mutex<Option<(String, Option<String>, std::time::Instant)>> = Mutex::new(None);
    /// A song without a picture yet is asked again this long after: Spotify names a new song first and gives its
    /// picture a moment later (Windows' own media flyout shows none either meanwhile).
    const COVER_AGAIN: std::time::Duration = std::time::Duration::from_secs(2);
    /// The last failure said, so error.log gets it once and not every poll.
    static SAID: Mutex<String> = Mutex::new(String::new());
    /// The one session manager, kept between polls (see `music`).
    static MANAGER: Mutex<Option<Manager>> = Mutex::new(None);
    /// When the manager was last asked for: asked again at most every `RETRY` after a failure, every `FRESH` when it
    /// finds no music app (each ask leaves a proxy behind, so asking every poll was the leak).
    static ASKED: Mutex<Option<std::time::Instant>> = Mutex::new(None);
    const RETRY: std::time::Duration = std::time::Duration::from_secs(10);
    const FRESH: std::time::Duration = std::time::Duration::from_secs(300);

    // The song is read again only when Windows says something changed (a song, play/pause, a new place, a music app
    // opened or closed), and once every `CHECK` besides (a media service restarted sends nothing). Between those the
    // last read is given (where the song is counted on from when it was read), its loudness read fresh.
    static CHANGED: AtomicBool = AtomicBool::new(true);
    /// The sessions' list changed: the handlers go on the new list at the next read.
    static LISTED: AtomicBool = AtomicBool::new(true);
    /// The sessions watched, with their handlers' tokens (song, playing, place).
    static WATCHED: Mutex<Vec<(Session, i64, i64, i64)>> = Mutex::new(Vec::new());
    static LAST: Mutex<Option<(Option<Playing>, std::time::Instant)>> = Mutex::new(None);
    const CHECK: std::time::Duration = std::time::Duration::from_secs(30);

    fn on_change<S: RuntimeType + 'static, A: RuntimeType + 'static>() -> TypedEventHandler<S, A> {
        TypedEventHandler::new(|_, _| {
            CHANGED.store(true, Ordering::SeqCst);
            Ok(())
        })
    }

    /// Puts the handlers on the music apps' sessions now listed (taken off the ones before).
    fn watch(manager: &Manager) {
        let mut watched = WATCHED.lock().unwrap_or_else(|p| p.into_inner());
        for (s, song, state, place) in watched.drain(..) {
            let _ = s.RemoveMediaPropertiesChanged(song);
            let _ = s.RemovePlaybackInfoChanged(state);
            let _ = s.RemoveTimelinePropertiesChanged(place);
        }
        let Ok(list) = manager.GetSessions() else { return };
        for i in 0..list.Size().unwrap_or(0) {
            let Ok(s) = list.GetAt(i) else { continue };
            if s.SourceAppUserModelId().ok().and_then(|id| Player::of(&id.to_string().to_lowercase())).is_none() {
                continue;
            }
            let tokens = (s.MediaPropertiesChanged(&on_change()), s.PlaybackInfoChanged(&on_change()), s.TimelinePropertiesChanged(&on_change()));
            if let (Ok(song), Ok(state), Ok(place)) = tokens {
                watched.push((s, song, state, place));
            }
        }
    }

    fn say(what: &str, e: &dyn std::fmt::Display) {
        let line = format!("Music app (Windows media controls), {what}: {e}");
        let mut said = SAID.lock().unwrap_or_else(|p| p.into_inner());
        if *said != line {
            #[cfg(feature = "core")]
            crate::core::log::write(&line);
            *said = line;
        }
    }

    /// The music app's session to show: the one playing, else the first paused one (in the order Windows lists
    /// them). Play/pause, next, back and a new place all go to the same one, chosen the same way.
    fn music() -> windows::core::Result<Option<(Session, Player)>> {
        // (The pool's threads: WinRT wants the thread joined to COM; a second call on the same thread is fine.)
        unsafe {
            let _ = RoInitialize(RO_INIT_MULTITHREADED);
        }
        // The manager is asked for once and kept: asking again every second left a proxy behind each time (about
        // 3,000 objects after an hour). It is asked for anew only when a call on it fails, so a restarted media
        // service (0x800706BA, "The RPC server is unavailable") is picked up again.
        // A service restarted without an error on the old manager shows as "no music app": a manager past `FRESH`
        // that finds none is asked for anew too.
        let since = |asked: Option<std::time::Instant>, d| asked.is_none_or(|t| t.elapsed() >= d);
        let asked = *ASKED.lock().unwrap_or_else(|p| p.into_inner());
        let kept = MANAGER.lock().unwrap_or_else(|p| p.into_inner()).clone();
        if let Some(m) = &kept
            && LISTED.swap(false, Ordering::SeqCst)
        {
            watch(m);
        }
        if let Some(m) = kept {
            match pick(&m) {
                Ok(Some(found)) => return Ok(Some(found)),
                Ok(None) if !since(asked, FRESH) => return Ok(None),
                Ok(None) => {}
                Err(e) => {
                    *MANAGER.lock().unwrap_or_else(|p| p.into_inner()) = None;
                    if !since(asked, RETRY) {
                        return Err(e);
                    }
                }
            }
        } else if !since(asked, RETRY) {
            return Ok(None);
        }
        *ASKED.lock().unwrap_or_else(|p| p.into_inner()) = Some(std::time::Instant::now());
        let m = Manager::RequestAsync()?.get()?;
        let found = pick(&m)?;
        let _ = m.SessionsChanged(&TypedEventHandler::new(|_, _| {
            LISTED.store(true, Ordering::SeqCst);
            CHANGED.store(true, Ordering::SeqCst);
            Ok(())
        }));
        watch(&m);
        LISTED.store(false, Ordering::SeqCst);
        *MANAGER.lock().unwrap_or_else(|p| p.into_inner()) = Some(m);
        Ok(found)
    }

    /// The session to show, chosen from the manager's list (playing first, else the first paused one).
    fn pick(manager: &Manager) -> windows::core::Result<Option<(Session, Player)>> {
        let list = manager.GetSessions()?;
        let mut paused = None;
        for i in 0..list.Size()? {
            let s = list.GetAt(i)?;
            let Some(player) = Player::of(&s.SourceAppUserModelId()?.to_string().to_lowercase()) else { continue };
            if s.GetPlaybackInfo().and_then(|p| p.PlaybackStatus()).is_ok_and(|st| st == Status::Playing) {
                return Ok(Some((s, player)));
            }
            if paused.is_none() {
                paused = Some((s, player));
            }
        }
        Ok(paused)
    }

    fn cover(s: &Session, key: &str) -> Option<String> {
        {
            let c = COVER.lock().unwrap_or_else(|p| p.into_inner());
            if let Some((k, file, asked)) = c.as_ref()
                && k == key
                && (file.is_some() || asked.elapsed() < COVER_AGAIN)
            {
                return file.clone();
            }
        }
        let read = || -> windows::core::Result<Option<Vec<u8>>> {
            let props = s.TryGetMediaPropertiesAsync()?.get()?;
            let Ok(thumb) = props.Thumbnail() else { return Ok(None) };
            let stream = thumb.OpenReadAsync()?.get()?;
            let size = stream.Size()? as u32;
            if size == 0 || size > 8 << 20 {
                return Ok(None);
            }
            let reader = DataReader::CreateDataReader(&stream)?;
            reader.LoadAsync(size)?.get()?;
            let mut bytes = vec![0u8; size as usize];
            reader.ReadBytes(&mut bytes)?;
            Ok(Some(bytes))
        };
        let file = match read() {
            Ok(Some(bytes)) => {
                let mut h: u64 = 0xcbf29ce484222325;
                for b in key.bytes() {
                    h = (h ^ b as u64).wrapping_mul(0x100000001b3);
                }
                let path = std::env::temp_dir().join(format!("bloxkeep-song-{h:016x}.jpg"));
                match std::fs::write(&path, &bytes) {
                    Ok(()) => Some(path.to_string_lossy().into_owned()),
                    Err(e) => {
                        say("saving the album picture", &e);
                        None
                    }
                }
            }
            Ok(None) => None,
            Err(e) => {
                say("reading the album picture", &e);
                None
            }
        };
        let mut c = COVER.lock().unwrap_or_else(|p| p.into_inner());
        // The last song's picture file goes when the song changes.
        if let Some((_, Some(old), _)) = c.as_ref()
            && Some(old) != file.as_ref()
        {
            let _ = std::fs::remove_file(old);
        }
        *c = Some((key.to_string(), file.clone(), std::time::Instant::now()));
        file
    }

    pub fn now() -> Option<Playing> {
        let fresh = CHANGED.swap(false, Ordering::SeqCst);
        // (A song still without its picture is read again soon: the picture may come late, without a word.)
        if !fresh
            && let Some((last, at)) = LAST.lock().unwrap_or_else(|p| p.into_inner()).clone()
            && at.elapsed() < if last.as_ref().is_some_and(|p| p.cover.is_none()) { COVER_AGAIN } else { CHECK }
        {
            return last.map(|mut p| {
                p.volume = volume_of(p.player);
                p
            });
        }
        let read = read_now();
        *LAST.lock().unwrap_or_else(|p| p.into_inner()) = Some((read.clone(), std::time::Instant::now()));
        read
    }

    fn read_now() -> Option<Playing> {
        let got = (|| -> windows::core::Result<Option<Playing>> {
            let Some((s, player)) = music()? else { return Ok(None) };
            let props = s.TryGetMediaPropertiesAsync()?.get()?;
            let title = props.Title()?.to_string();
            if title.trim().is_empty() {
                return Ok(None);
            }
            let artist = props.Artist()?.to_string();
            let playing = s.GetPlaybackInfo()?.PlaybackStatus()? == Status::Playing;
            let key = format!("{title}\u{1}{artist}");
            let cover = cover(&s, &key);
            Ok(Some(Playing { player, title, artist, playing, cover, volume: volume_of(player), position: position(&s, playing), read: std::time::Instant::now() }))
        })();
        got.unwrap_or_else(|e| {
            say("reading the song", &e);
            None
        })
    }

    /// Where the song is and how long it is, ms. Spotify says where it was at some moment (not every moment): while
    /// it plays, the time since that moment is counted on. (Measured on the owner's PC, 2026-11-18: a place 0.007 s
    /// in, said 1.4 s before, a length of 2:25.6.)
    fn position(s: &Session, playing: bool) -> Option<(u64, u64)> {
        let read = || -> windows::core::Result<(i64, i64, i64)> {
            let t = s.GetTimelineProperties()?;
            let (start, end) = (t.StartTime()?.Duration, t.EndTime()?.Duration);
            Ok((t.Position()?.Duration - start, end - start, t.LastUpdatedTime()?.UniversalTime))
        };
        // (All in Windows' 100 ns; its clock counts from 1601.)
        let (mut at, length, said) = read().ok()?;
        if length <= 0 {
            return None;
        }
        if playing {
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos() as i64 / 100 + 116_444_736_000_000_000;
            at += (now - said).clamp(0, length);
        }
        Some(((at.clamp(0, length) / 10_000) as u64, (length / 10_000) as u64))
    }

    /// The music app goes to `ms` of its song.
    pub fn seek(ms: u64) {
        let done = (|| -> windows::core::Result<()> {
            let Some((s, _)) = music()? else { return Ok(()) };
            s.TryChangePlaybackPositionAsync(ms as i64 * 10_000)?.get()?;
            Ok(())
        })();
        // (Read afresh next, not waiting for Windows' word of it.)
        CHANGED.store(true, Ordering::SeqCst);
        if let Err(e) = done {
            say("going to another place in the song", &e);
        }
    }

    /// The music app's sound sessions on the default speakers (Windows' volume mixer has one per program that plays):
    /// its loudness is theirs. The apps give no loudness through the media controls.
    fn sessions(program: &str) -> windows::core::Result<Vec<ISimpleAudioVolume>> {
        let mut found = Vec::new();
        unsafe {
            let _ = RoInitialize(RO_INIT_MULTITHREADED);
            let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let manager: IAudioSessionManager2 = devices.GetDefaultAudioEndpoint(eRender, eMultimedia)?.Activate(CLSCTX_ALL, None)?;
            let list = manager.GetSessionEnumerator()?;
            for i in 0..list.GetCount()? {
                let Ok(control) = list.GetSession(i).and_then(|s| s.cast::<IAudioSessionControl2>()) else { continue };
                // (The session's name carries its program's path: …\Spotify.exe%b{…}.)
                let Ok(name) = control.GetSessionInstanceIdentifier() else { continue };
                let said = name.to_string().unwrap_or_default().to_lowercase();
                CoTaskMemFree(Some(name.0 as *const _));
                if said.contains(program)
                    && let Ok(v) = control.cast::<ISimpleAudioVolume>()
                {
                    found.push(v);
                }
            }
        }
        Ok(found)
    }

    /// The program whose sound (with its children's: beat.rs reads a program's whole tree) is the music app's, for the
    /// profile's beat: the top one of the programs with a sound session of the app. (Spotify has a session for itself
    /// and one for the audio service program it started: most songs sound from the audio service, some from Spotify
    /// itself. The first session listed was read, the audio service's, and the profile heard nothing while those
    /// songs played.) Of programs apart, the loudest now. Never an expired session.
    pub fn pid_of(player: Player) -> Option<u32> {
        let mut found: Vec<(u32, f32)> = Vec::new();
        unsafe {
            let _ = RoInitialize(RO_INIT_MULTITHREADED);
            let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
            let manager: IAudioSessionManager2 = devices.GetDefaultAudioEndpoint(eRender, eMultimedia).ok()?.Activate(CLSCTX_ALL, None).ok()?;
            let list = manager.GetSessionEnumerator().ok()?;
            for i in 0..list.GetCount().ok()? {
                let Ok(control) = list.GetSession(i).and_then(|s| s.cast::<IAudioSessionControl2>()) else { continue };
                let Ok(name) = control.GetSessionInstanceIdentifier() else { continue };
                let said = name.to_string().unwrap_or_default().to_lowercase();
                CoTaskMemFree(Some(name.0 as *const _));
                if !said.contains(player.program()) || control.GetState() == Ok(AudioSessionStateExpired) {
                    continue;
                }
                let Ok(pid) = control.GetProcessId() else { continue };
                if pid != 0 {
                    found.push((pid, control.cast::<IAudioMeterInformation>().and_then(|m| m.GetPeakValue()).unwrap_or(0.)));
                }
            }
        }
        if found.len() > 1 {
            let parents = parents();
            let under = |mut pid: u32, top: u32| {
                for _ in 0..16 {
                    match parents.get(&pid) {
                        Some(&p) if p == top => return true,
                        Some(&p) if p != 0 && p != pid => pid = p,
                        _ => return false,
                    }
                }
                false
            };
            let all: Vec<u32> = found.iter().map(|f| f.0).collect();
            found.retain(|&(pid, _)| !all.iter().any(|&top| top != pid && under(pid, top)));
        }
        found.into_iter().max_by(|a, b| a.1.total_cmp(&b.1)).map(|(pid, _)| pid)
    }

    /// Each program's parent, from Windows' list now.
    fn parents() -> std::collections::HashMap<u32, u32> {
        use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS};
        let mut out = std::collections::HashMap::new();
        // SAFETY: the snapshot is closed below; the entry's size is set as Windows asks.
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap == INVALID_HANDLE_VALUE {
                return out;
            }
            let mut e: PROCESSENTRY32W = std::mem::zeroed();
            e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut ok = Process32FirstW(snap, &mut e) != 0;
            while ok {
                out.insert(e.th32ProcessID, e.th32ParentProcessID);
                ok = Process32NextW(snap, &mut e) != 0;
            }
            CloseHandle(snap);
        }
        out
    }

    fn volume_of(player: Player) -> Option<f32> {
        match sessions(player.program()) {
            Ok(list) => list.first().and_then(|v| unsafe { v.GetMasterVolume() }.ok()),
            Err(e) => {
                say("reading its loudness", &e);
                None
            }
        }
    }

    pub fn set_volume(to: f32) {
        let Ok(Some((_, player))) = music() else { return };
        let done = sessions(player.program()).and_then(|list| list.iter().try_for_each(|v| unsafe { v.SetMasterVolume(to.clamp(0., 1.), std::ptr::null()) }));
        if let Err(e) = done {
            say("setting its loudness", &e);
        }
    }

    pub fn act(a: Act) {
        let done = (|| -> windows::core::Result<()> {
            let Some((s, _)) = music()? else { return Ok(()) };
            match a {
                Act::Toggle => s.TryTogglePlayPauseAsync()?.get()?,
                Act::Next => s.TrySkipNextAsync()?.get()?,
                Act::Back => s.TrySkipPreviousAsync()?.get()?,
            };
            Ok(())
        })();
        CHANGED.store(true, Ordering::SeqCst);
        if let Err(e) = done {
            say("play/pause/next", &e);
        }
    }
}

/// What the music app plays now (None when none is open or it plays nothing). Blocks for a moment: call it off the window.
pub fn now() -> Option<Playing> {
    #[cfg(all(windows, any(feature = "core", feature = "shot")))]
    return win::now();
    #[allow(unreachable_code)]
    None
}

/// The program that plays the music app's sound now. Blocks for a moment: call it off the window.
pub fn pid_of(player: Player) -> Option<u32> {
    #[cfg(all(windows, any(feature = "core", feature = "shot")))]
    return win::pid_of(player);
    #[allow(unreachable_code)]
    {
        let _ = player;
        None
    }
}

/// Play/pause, next or back in the music app. Blocks for a moment: call it off the window.
pub fn act(a: Act) {
    #[cfg(all(windows, any(feature = "core", feature = "shot")))]
    win::act(a);
    let _ = a;
}

/// The music app goes to `ms` of its song. Blocks for a moment: call it off the window.
pub fn seek(ms: u64) {
    #[cfg(all(windows, any(feature = "core", feature = "shot")))]
    win::seek(ms);
    let _ = ms;
}

/// Sets how loud the music app is in Windows' mixer, 0 to 1. Blocks for a moment: call it off the window.
pub fn set_volume(to: f32) {
    #[cfg(all(windows, any(feature = "core", feature = "shot")))]
    win::set_volume(to);
    let _ = to;
}
