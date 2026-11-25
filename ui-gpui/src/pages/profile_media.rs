use crate::threads::Hold;
use std::{path::Path, time::Duration};

use gpui_kit::{prelude::*, *};

use crate::{images, kit::*, shell::Root, theme::*};

#[derive(Clone, Copy)]
pub(super) enum Kind {
    Banner,
    Song,
    /// Their own picture in place of Roblox's, only in Bloxkeep (a GIF moves).
    Face,
}

impl Kind {
    fn key(self, user: i64) -> String {
        format!("{}:{user}", match self { Self::Banner => "profileBanner", Self::Song => "profileSong", Self::Face => "profileFace" })
    }
}

pub(super) fn path(kind: Kind, user: i64) -> Option<String> {
    if matches!(kind, Kind::Song) {
        let (list, at) = songs(user);
        return list.into_iter().nth(at);
    }
    #[cfg(feature = "shot")]
    if let Ok(path) = std::env::var(match kind { Kind::Banner => "BLOXKEEP_SHOT_BANNER", Kind::Song => "BLOXKEEP_SHOT_SONG", Kind::Face => "BLOXKEEP_SHOT_FACE" }) {
        return Some(path);
    }
    crate::prefs::get(&kind.key(user)).and_then(|v| v.as_str().map(str::to_string))
}

/// The profile's songs, a playlist (the owner, 2026-11-25: "lỡ người ta có hẳn 1 playlist thì sao"), and which one is
/// up. Kept as one path (one song, as before) or a list of them.
pub(super) fn songs(user: i64) -> (Vec<String>, usize) {
    #[cfg(feature = "shot")]
    if let Ok(paths) = std::env::var("BLOXKEEP_SHOT_SONG") {
        return (paths.split('|').map(str::to_string).collect(), 0);
    }
    let list: Vec<String> = match crate::prefs::get(&Kind::Song.key(user)) {
        Some(serde_json::Value::String(s)) => vec![s],
        Some(serde_json::Value::Array(v)) => v.iter().filter_map(|s| s.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    };
    let at = crate::prefs::get(&format!("profileSongAt:{user}")).and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let at = at.min(list.len().saturating_sub(1));
    (list, at)
}

fn set_songs(user: i64, list: &[String], at: usize) {
    let value = match list {
        [] => serde_json::Value::Null,
        [one] => serde_json::json!(one),
        _ => serde_json::json!(list),
    };
    crate::prefs::set(&Kind::Song.key(user), value);
    crate::prefs::set(&format!("profileSongAt:{user}"), serde_json::json!(at.min(list.len().saturating_sub(1))));
}

/// Songs picked from the PC, one or many at once: they take the list's place, or join its end (`add`).
fn choose_songs(r: &mut Root, user: i64, add: bool, cx: &mut Context<Root>) {
    let picked = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: true, prompt: Some(r.tr("pf.media.songPick").into()) });
    cx.spawn(async move |this, cx| {
        let Some(files) = picked.await.ok().and_then(Result::ok).flatten() else { return };
        let _ = this.update(cx, |r, cx| {
            if !r.profile.showing_own(user) || r.locked() {
                return;
            }
            let song = |f: &std::path::PathBuf| f.is_file() && matches!(f.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref(), Some("mp3" | "wav"));
            let good: Vec<String> = files.iter().filter(|f| song(f)).filter_map(|f| f.to_str().map(str::to_string)).collect();
            if good.len() < files.len() {
                r.toast = Some((r.tr("pf.media.badSong"), "bad".into()));
            }
            if !good.is_empty() {
                let (mut list, at) = songs(user);
                if add {
                    list.retain(|s| !good.contains(s));
                    let at = at.min(list.len().saturating_sub(1));
                    list.extend(good);
                    set_songs(user, &list, at);
                } else {
                    r.profile.song = None;
                    set_songs(user, &good, 0);
                }
            }
            cx.notify();
        });
    }).detach();
}

/// The song before or after this one (`step` -1 or 1), playing at once when this one was.
fn skip(r: &mut Root, user: i64, step: isize, cx: &mut Context<Root>) {
    let (list, at) = songs(user);
    if list.len() < 2 {
        return;
    }
    let was_playing = r.profile.song.as_ref().is_some_and(|p| p.user == user && p.playing);
    let next = (at as isize + step).rem_euclid(list.len() as isize) as usize;
    r.profile.song = None;
    set_songs(user, &list, next);
    if was_playing {
        toggle(r, user, &list[next], cx);
    }
    cx.notify();
}

/// Back and Next beside Play, with more than one song.
fn skips(root: &Root, user: i64, size: f32, cx: &mut Context<Root>) -> (Option<IconButton>, Option<IconButton>) {
    if songs(user).0.len() < 2 {
        return (None, None);
    }
    (
        Some(IconButton::new("pf-song-back", "skipBack").size(size).round().tip(root.tr("music.back")).on_click(cx.listener(move |r, _, _, cx| skip(r, user, -1, cx)))),
        Some(IconButton::new("pf-song-next", "skipNext").size(size).round().tip(root.tr("music.next")).on_click(cx.listener(move |r, _, _, cx| skip(r, user, 1, cx)))),
    )
}

/// The line under the song's name: "Profile song", or where it is in the list.
fn song_line(root: &Root, user: i64) -> String {
    let (list, at) = songs(user);
    if list.len() > 1 {
        root.i18n.tv("pf.media.songOf", &[("at", &(at + 1).to_string()), ("all", &list.len().to_string())])
    } else {
        root.tr("pf.media.song")
    }
}

pub(super) fn choose(r: &mut Root, kind: Kind, user: i64, cx: &mut Context<Root>) {
    if matches!(kind, Kind::Song) {
        return choose_songs(r, user, false, cx);
    }
    let prompt = r.tr(match kind { Kind::Banner => "pf.media.banner", Kind::Song => "pf.media.songPick", Kind::Face => "pf.media.face" });
    let picked = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: false, prompt: Some(prompt.into()) });
    cx.spawn(async move |this, cx| {
        let Some(file) = picked.await.ok().and_then(Result::ok).flatten().and_then(|v| v.into_iter().next()) else { return };
        let _ = this.update(cx, |r, cx| {
            if !r.profile.showing_own(user) || r.locked() {
                return;
            }
            let kind_ext = file.extension().and_then(|ext| ext.to_str()).unwrap_or_default().to_ascii_lowercase();
            let valid = match kind {
                Kind::Banner | Kind::Face => matches!(kind_ext.as_str(), "jpg" | "jpeg" | "png" | "webp" | "gif"),
                Kind::Song => matches!(kind_ext.as_str(), "mp3" | "wav"),
            };
            if file.is_file() && valid {
                if matches!(kind, Kind::Song) {
                    r.profile.song = None;
                }
                crate::prefs::set(&kind.key(user), serde_json::json!(file));
            } else {
                let key = match kind { Kind::Banner | Kind::Face => "pf.media.badPicture", Kind::Song => "pf.media.badSong" };
                r.toast = Some((r.tr(key), "bad".into()));
            }
            cx.notify();
        });
    }).detach();
}

/// Their own banner picture laid as they placed it: cut to fill the banner (`bw`×`bh`), the part that shows chosen by
/// where it is held (0 to 1 each way, the middle until moved), drawn at that size so it is sharp. With it: how far it
/// runs past the banner each way (what a drag moves it by), and its own size when that is small for the banner on
/// this screen.
pub(super) struct Placed {
    pub el: AnyElement,
    pub over: (f32, f32),
    pub small: Option<(u32, u32)>,
}

pub(super) fn placed_banner(root: &Root, url: &str, user: i64, bw: f32, bh: f32, cx: &mut Context<Root>) -> Option<Placed> {
    images::pic(url, cx)?;
    let (iw, ih) = images::dims(url, cx).filter(|(w, h)| *w > 0 && *h > 0)?;
    let scale = (bw / iw as f32).max(bh / ih as f32);
    let (w, h) = ((iw as f32 * scale).round().max(bw), (ih as f32 * scale).round().max(bh));
    let [fx, fy] = banner_at(root, user);
    let src = images::pic_at(url, w, h, cx)?;
    let el = img(src).absolute().left(px(-(w - bw) * fx)).top(px(-(h - bh) * fy)).w(px(w)).h(px(h)).into_any_element();
    // (Small: each of its dots spread over a quarter more of the screen's, or more: that is where it reads soft.)
    let small = (scale * crate::kit::screen_scale() > 1.25).then_some((iw, ih));
    Some(Placed { el, over: (w - bw, h - bh), small })
}

/// Where the banner picture is held: while it is being moved, where the mouse has it; else as kept.
pub(super) fn banner_at(root: &Root, user: i64) -> [f32; 2] {
    if let Some(at) = root.profile.banner_at {
        return at;
    }
    let kept = crate::prefs::get(&format!("profileBannerAt:{user}"));
    let part = |i: usize| kept.as_ref().and_then(|v| v.get(i)).and_then(|v| v.as_f64()).map_or(0.5, |v| (v as f32).clamp(0., 1.));
    [part(0), part(1)]
}

/// Moving ends: where it is held is kept.
pub(super) fn keep_banner_at(r: &mut Root, user: i64) {
    r.profile.banner_grab = None;
    if let Some(at) = r.profile.banner_at.take() {
        crate::prefs::set(&format!("profileBannerAt:{user}"), serde_json::json!([at[0], at[1]]));
    }
}

pub(super) fn clear(kind: Kind, user: i64) {
    crate::prefs::set(&kind.key(user), serde_json::Value::Null);
    if matches!(kind, Kind::Song) {
        crate::prefs::set(&format!("profileSongAt:{user}"), serde_json::Value::Null);
    }
}

/// The song on now taken out of the list; the next one takes its place.
fn remove_song(user: i64) {
    let (mut list, at) = songs(user);
    if at < list.len() {
        list.remove(at);
    }
    set_songs(user, &list, at);
}

pub(super) struct Player {
    file: String,
    user: i64,
    id: u64,
    length: u64,
    position: u64,
    playing: bool,
}

impl Player {
    fn open(file: &str, user: i64) -> Option<Self> {
        if file.contains(['"', '\0']) || !Path::new(file).is_file() {
            return None;
        }
        // (As "mpegvideo": the one Windows player that plays MP3 and WAV alike and takes a volume.)
        command(&format!("open \"{file}\" type mpegvideo alias BloxkeepProfileSong"))?;
        let _ = command(&format!("setaudio BloxkeepProfileSong volume to {}", volume() * 10));
        let mut player = Self { file: file.into(), user, id: super::avatar::ticket(), length: 0, position: 0, playing: false };
        command("set BloxkeepProfileSong time format milliseconds")?;
        // Windows' own length of an MP3 can be far off (18 s read as 36 s, 2026-11-17): counted from its frames.
        let mci = command("status BloxkeepProfileSong length").and_then(|v| v.trim().parse().ok());
        player.length = length_of(file).or(mci)?;
        (player.length > 0).then_some(player)
    }
}

/// How loud the song plays, 0 to 100 (kept; 70 until it is set).
fn volume() -> i64 {
    crate::prefs::get("profileSongVolume").and_then(|v| v.as_i64()).unwrap_or(70).clamp(0, 100)
}

fn set_volume(r: &mut Root, v: i64, cx: &mut Context<Root>) {
    crate::prefs::set("profileSongVolume", serde_json::json!(v));
    if r.profile.song.is_some() {
        let _ = command(&format!("setaudio BloxkeepProfileSong volume to {}", v * 10));
    }
    cx.notify();
}

/// A song's length in ms, read from the file itself (so it shows before the song is played); the last one asked is
/// kept, a page is drawn many times a second.
fn length_of(file: &str) -> Option<u64> {
    static LAST: std::sync::Mutex<Option<(String, Option<u64>)>> = std::sync::Mutex::new(None);
    let mut last = LAST.hold();
    if let Some((f, len)) = last.as_ref() {
        if f == file {
            return *len;
        }
    }
    let path = Path::new(file);
    let len = mp3_length(path).or_else(|| wav_length(path));
    *last = Some((file.to_string(), len));
    len
}

/// A WAV's length in ms: its data chunk's size over its bytes a second.
fn wav_length(file: &Path) -> Option<u64> {
    use std::io::Read;
    let mut b = vec![0u8; 8192];
    let n = std::fs::File::open(file).ok()?.read(&mut b).ok()?;
    let b = &b[..n];
    if b.len() < 12 || &b[..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return None;
    }
    let (mut i, mut rate) = (12, 0u64);
    while i + 8 <= b.len() {
        let size = u32::from_le_bytes([b[i + 4], b[i + 5], b[i + 6], b[i + 7]]) as u64;
        match &b[i..i + 4] {
            b"fmt " if i + 20 <= b.len() => rate = u32::from_le_bytes([b[i + 16], b[i + 17], b[i + 18], b[i + 19]]) as u64,
            b"data" if rate > 0 => return Some(size * 1000 / rate),
            _ => {}
        }
        i += 8 + size as usize + (size as usize & 1);
    }
    None
}

/// An MP3's length in ms, from its frames (every frame's samples at its own rate, so a variable bit rate counts
/// right too); None for a file that is not an MP3.
fn mp3_length(file: &Path) -> Option<u64> {
    if !file.extension().is_some_and(|e| e.eq_ignore_ascii_case("mp3")) {
        return None;
    }
    let b = std::fs::read(file).ok()?;
    let mut i = 0;
    // An ID3v2 tag in front: its size is four 7-bit bytes.
    if b.len() > 10 && &b[..3] == b"ID3" {
        i = 10 + ((b[6] as usize & 0x7f) << 21 | (b[7] as usize & 0x7f) << 14 | (b[8] as usize & 0x7f) << 7 | (b[9] as usize & 0x7f));
    }
    const RATE: [[u32; 3]; 3] = [[44100, 48000, 32000], [22050, 24000, 16000], [11025, 12000, 8000]];
    const KBPS1: [u32; 15] = [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320];
    const KBPS2: [u32; 15] = [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160];
    let (mut ms, mut frames) = (0f64, 0u32);
    while i + 4 <= b.len() {
        let h = u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
        let (version, layer, bitrate, rate) = ((h >> 19) & 3, (h >> 17) & 3, (h >> 12) & 15, (h >> 10) & 3);
        // Layer III frames only (MPEG 1, 2 and 2.5); anything else: step on and look again.
        if h >> 21 != 0x7ff || version == 1 || layer != 1 || bitrate == 0 || bitrate == 15 || rate == 3 {
            i += 1;
            continue;
        }
        let v = match version { 3 => 0, 2 => 1, _ => 2 };
        let hz = RATE[v][rate as usize];
        let (kbps, samples) = if v == 0 { (KBPS1[bitrate as usize], 1152) } else { (KBPS2[bitrate as usize], 576) };
        let len = (samples / 8 * kbps * 1000 / hz + (h >> 9 & 1)) as usize;
        if len < 4 {
            i += 1;
            continue;
        }
        ms += samples as f64 * 1000. / hz as f64;
        frames += 1;
        i += len;
    }
    (frames > 0).then_some(ms as u64)
}

impl Drop for Player {
    fn drop(&mut self) {
        let _ = command("close BloxkeepProfileSong");
    }
}

#[cfg(windows)]
fn command(words: &str) -> Option<String> {
    #[link(name = "winmm")]
    unsafe extern "system" {
        fn mciSendStringW(command: *const u16, result: *mut u16, result_len: u32, callback: *mut std::ffi::c_void) -> u32;
    }
    let input: Vec<u16> = words.encode_utf16().chain(Some(0)).collect();
    let mut result = [0u16; 96];
    // MCI reads a nul-terminated command and writes at most result_len UTF-16 units.
    let error = unsafe { mciSendStringW(input.as_ptr(), result.as_mut_ptr(), result.len() as u32, std::ptr::null_mut()) };
    let end = result.iter().position(|&c| c == 0).unwrap_or(result.len());
    (error == 0).then(|| String::from_utf16_lossy(&result[..end]))
}

#[cfg(not(windows))]
fn command(_: &str) -> Option<String> { None }

fn visible(root: &Root, user: i64) -> bool {
    !root.locked() && root.page == crate::pages::Page::Games && root.games.view == super::games::View::Profile
        && root.games.open.is_none() && root.profile.showing_own(user)
}

fn toggle(r: &mut Root, user: i64, file: &str, cx: &mut Context<Root>) {
    if !r.profile.song.as_ref().is_some_and(|p| p.user == user && p.file == file) {
        r.profile.song = None;
        r.profile.song = Player::open(file, user);
    }
    let result = r.profile.song.as_mut().and_then(|p| {
        let was_playing = p.playing;
        if was_playing {
            command("pause BloxkeepProfileSong")?;
            p.position = command("status BloxkeepProfileSong position").and_then(|v| v.trim().parse().ok()).unwrap_or(p.position);
        } else {
            if p.position >= p.length {
                p.position = 0;
            }
            command(&format!("play BloxkeepProfileSong from {}", p.position))?;
        }
        p.playing = !was_playing;
        p.id = super::avatar::ticket();
        Some((p.id, p.playing))
    });
    cx.notify();
    let Some((id, true)) = result else {
        if result.is_none() {
            r.profile.song = None;
            r.toast = Some((r.tr("pf.media.playError"), "bad".into()));
        }
        return;
    };
    cx.spawn(async move |this, cx| loop {
        cx.background_executor().timer(Duration::from_millis(250)).await;
        let mut ended = false;
        let playing = this.update(cx, |r, cx| {
            if !r.profile.song.as_ref().is_some_and(|p| p.id == id && p.playing) {
                return false;
            }
            if !visible(r, user) {
                r.profile.song = None;
                return false;
            }
            let Some(p) = r.profile.song.as_mut() else { return false };
            p.position = command("status BloxkeepProfileSong position").and_then(|v| v.trim().parse().ok()).unwrap_or(p.position);
            p.playing = command("status BloxkeepProfileSong mode").is_some_and(|mode| mode.eq_ignore_ascii_case("playing"));
            if !p.playing {
                p.position = p.length;
                ended = true;
            }
            cx.notify();
            p.playing
        }).unwrap_or(false);
        if ended {
            // The list plays on to its last song.
            let _ = this.update(cx, |r, cx| {
                let (list, at) = songs(user);
                if at + 1 < list.len() {
                    r.profile.song = None;
                    set_songs(user, &list, at + 1);
                    toggle(r, user, &list[at + 1], cx);
                }
            });
        }
        if !playing { break; }
    }).detach();
}

/// To a place in the song (a click on its bar): it plays on from there, or waits there while paused.
fn seek(r: &mut Root, user: i64, file: &str, part: f32, cx: &mut Context<Root>) {
    if !r.profile.song.as_ref().is_some_and(|p| p.user == user && p.file == file) {
        r.profile.song = None;
        r.profile.song = Player::open(file, user);
    }
    if let Some(p) = r.profile.song.as_mut() {
        p.position = (p.length as f32 * part.clamp(0., 1.)) as u64;
        if p.playing {
            let _ = command(&format!("play BloxkeepProfileSong from {}", p.position));
        }
    }
    cx.notify();
}

/// The picture inside an MP3 (its ID3 tag's APIC frame), saved once in the temporary folder; None when it has none.
fn cover(file: &str) -> Option<String> {
    let b = std::fs::read(file).ok()?;
    if b.len() < 10 || &b[..3] != b"ID3" {
        return None;
    }
    let major = b[3];
    let size = |x: &[u8], safe: bool| if safe { (x[0] as usize & 0x7f) << 21 | (x[1] as usize & 0x7f) << 14 | (x[2] as usize & 0x7f) << 7 | (x[3] as usize & 0x7f) } else { u32::from_be_bytes([x[0], x[1], x[2], x[3]]) as usize };
    let end = (10 + size(&b[6..10], true)).min(b.len());
    let mut i = 10;
    while i + 10 <= end && b[i] != 0 {
        let len = size(&b[i + 4..i + 8], major >= 4);
        let body = b.get(i + 10..(i + 10 + len).min(end))?;
        if &b[i..i + 4] == b"APIC" && body.len() > 4 {
            let wide = body[0] == 1 || body[0] == 2;
            let mut j = 1 + body[1..].iter().position(|&c| c == 0)? + 2; // past the mime type's 0 and the picture type
            // The description ends in a 0 (two for UTF-16).
            if wide {
                while j + 1 < body.len() && !(body[j] == 0 && body[j + 1] == 0) { j += 2; }
                j += 2;
            } else {
                while j < body.len() && body[j] != 0 { j += 1; }
                j += 1;
            }
            let pic = body.get(j..)?;
            let ext = if pic.starts_with(b"\x89PNG") { "png" } else { "jpg" };
            let mut h = std::collections::hash_map::DefaultHasher::new();
            std::hash::Hash::hash(&(file, pic.len()), &mut h);
            let out = std::env::temp_dir().join(format!("bloxkeep-song-{:x}.{ext}", std::hash::Hasher::finish(&h)));
            if !out.is_file() {
                std::fs::write(&out, pic).ok()?;
            }
            return out.to_str().map(str::to_string);
        }
        i += 10 + len;
    }
    None
}

/// The song's ⋮: Change and Remove.
fn more(root: &Root, user: i64, size: f32, grounded: bool, cx: &mut Context<Root>) -> Div {
    let open = root.profile.song_menu == Some(user);
    let row = |id: &'static str| hot(id).h(px(32.)).px(px(10.)).rounded(px(R_S)).flex().items_center().gap(px(8.)).text_size(px(T_M)).text_color(c(TEXT_2)).cursor_pointer();
    let menu = div().id("pf-song-menu").occlude().w(px(200.)).p(px(4.)).rounded(px(R_M)).bg(c(RAISED)).border_1().border_color(c(LINE)).flex().flex_col()
        .on_mouse_down_out(cx.listener(|r, _, _, cx| { r.profile.song_menu = None; cx.notify(); }))
        .child(row("pf-song-add").child(icon("plus", 14., c(MUTED))).child(root.tr("pf.media.songAdd")).on_click(cx.listener(move |r, _, _, cx| {
            r.profile.song_menu = None;
            choose_songs(r, user, true, cx);
            cx.notify();
        })))
        .child(row("pf-song-change").child(icon("refresh", 14., c(MUTED))).child(root.tr("pf.media.songChange")).on_click(cx.listener(move |r, _, _, cx| {
            r.profile.song_menu = None;
            choose(r, Kind::Song, user, cx);
            cx.notify();
        })))
        .child(row("pf-song-remove").child(icon("trash", 14., c(MUTED))).child(root.tr("pf.media.songRemove")).on_click(cx.listener(move |r, _, _, cx| {
            r.profile.song_menu = None;
            r.profile.song = None;
            remove_song(user);
            cx.notify();
        })));
    div().relative().flex_none()
        .child({ let b = IconButton::new("pf-song-more", "more").size(size).round(); let b = if grounded { b.filled() } else { b }; if open { b } else { b.tip(root.tr("pf.media.songMore")) } }.on_click(cx.listener(move |r, _, _, cx| {
            r.profile.song_menu = if r.profile.song_menu == Some(user) { None } else { Some(user) };
            cx.notify();
        })))
        .child(popup(PopAt::BelowRight(size + 4.), "pf-song-pop", open, menu))
}

thread_local! {
    /// The song's bar being held (which one, and where along it the mouse is, 0 to 1): shown there at once, the song
    /// sent there when the button lets go.
    static HELD: std::cell::Cell<Option<(&'static str, f32)>> = const { std::cell::Cell::new(None) };
}

/// Where the bar `id` is held now, if it is.
fn held(id: &'static str) -> Option<f32> {
    HELD.with(|h| h.get()).filter(|h| h.0 == id).map(|h| h.1)
}

/// Over a song's bar: a press goes to that place and a drag carries it along (past the bar's ends too, the mouse
/// followed over the whole window while held); the song goes there when the button lets go, so the music app is told
/// once, not at every step.
fn track(id: &'static str, go: impl Fn(&mut Root, f32, &mut Context<Root>) + 'static, cx: &mut Context<Root>) -> Div {
    let this = cx.entity();
    let go = std::rc::Rc::new(go);
    let pad = canvas(|b, window, _| window.insert_hitbox(b, HitboxBehavior::Normal), move |b, hitbox, window, _| {
        window.set_cursor_style(CursorStyle::PointingHand, &hitbox);
        let part = move |x: Pixels| (f32::from(x - b.origin.x) / f32::from(b.size.width).max(1.)).clamp(0., 1.);
        let down = this.clone();
        window.on_mouse_event(move |e: &MouseDownEvent, phase, window, cx| {
            if phase == DispatchPhase::Bubble && e.button == MouseButton::Left && hitbox.is_hovered(window) {
                HELD.with(|h| h.set(Some((id, part(e.position.x)))));
                down.update(cx, |_, cx| cx.notify());
            }
        });
        if held(id).is_none() {
            return;
        }
        let moved = this.clone();
        window.on_mouse_event(move |e: &MouseMoveEvent, phase, _, cx| {
            if phase != DispatchPhase::Bubble || held(id).is_none() {
                return;
            }
            // (Let go outside the window, no up comes: the move without the button ends the hold.)
            if e.pressed_button != Some(MouseButton::Left) {
                HELD.with(|h| h.set(None));
            } else {
                HELD.with(|h| h.set(Some((id, part(e.position.x)))));
            }
            moved.update(cx, |_, cx| cx.notify());
        });
        let (up, go) = (this.clone(), go.clone());
        window.on_mouse_event(move |e: &MouseUpEvent, phase, _, cx| {
            if phase != DispatchPhase::Bubble || e.button != MouseButton::Left {
                return;
            }
            let Some(to) = held(id) else { return };
            HELD.with(|h| h.set(None));
            up.update(cx, |r, cx| go(r, to, cx));
        });
    });
    div().absolute().inset_0().child(pad.size_full())
}

/// What a bar shows: the place played, or where it is held while held; and whether it is held.
fn shown(id: &'static str, at: u64, length: Option<u64>) -> (u64, f32, bool) {
    match (held(id), length) {
        (Some(p), Some(l)) if l > 0 => ((l as f32 * p) as u64, p, true),
        _ => (at, length.map_or(0., |l| (at as f32 / l.max(1) as f32).min(1.)), false),
    }
}

/// The bar's knob while it is held, at the end of the part played.
fn knob(part: f32, top: f32) -> Div {
    div().absolute().left(relative(part)).top(px(top)).ml(px(-6.)).size(px(12.)).rounded_full().bg(c(TEXT)).shadow_sm()
}

/// A song's bar under its two times, as the header's card has it: played so far and its whole length, the bar filled
/// that far; the track (`zones`) lies over the bar and takes the press and drag that go to a place.
fn bar_under(id: &'static str, at: u64, length: Option<u64>, zones: Div) -> Div {
    let (at, part, holding) = shown(id, at, length);
    let stamp = |ms: Option<u64>| div().text_size(px(T_XS)).text_color(c(TEXT_2)).font_features(tnum()).child(ms.map_or("–:––".to_string(), time));
    div().w_full().flex().flex_col()
        .child(div().flex().justify_between().child(stamp(Some(at))).child(stamp(length)))
        .child(div().relative().mt(px(4.)).w_full().h(px(14.))
            .child(div().absolute().left_0().right_0().top(px(5.)).h(px(4.)).rounded_full().bg(c(LINE)))
            .child(div().absolute().left_0().top(px(5.)).h(px(4.)).w(relative(part)).rounded_full().bg(c(TEXT)))
            .when(holding, |d| d.child(knob(part, 1.)))
            .child(zones))
}

/// The same in one line, as the row has it: the time, the bar, the length.
fn bar_between(id: &'static str, at: u64, length: Option<u64>, zones: Div) -> Div {
    let (at, part, holding) = shown(id, at, length);
    let stamp = |ms: Option<u64>| div().flex_none().text_size(px(T_S)).text_color(c(MUTED)).font_features(tnum()).child(ms.map_or("–:––".to_string(), time));
    div().flex().items_center().gap(px(10.))
        .child(stamp(Some(at)))
        .child(div().relative().flex_1().h(px(18.))
            .child(div().absolute().left_0().right_0().top(px(7.)).h(px(4.)).rounded_full().bg(c(LINE)))
            .child(div().absolute().left_0().top(px(7.)).h(px(4.)).w(relative(part)).rounded_full().bg(c(FACE)))
            .when(holding, |d| d.child(knob(part, 3.)))
            .child(zones))
        .child(stamp(length))
}

/// Over the bar of Spotify's song: a press or a drag goes to that place in it (Windows' media controls take it to
/// Spotify), shown at once.
fn spotify_zones(cx: &mut Context<Root>) -> Div {
    track("sp", |r, to, cx| {
        let Some((_, length)) = r.music(cx).and_then(|m| m.at()) else { return };
        let ms = (length as f32 * to) as u64;
        r.change_music(cx, |m| m.sought(ms));
        cx.background_executor().spawn(async move { crate::media::seek(ms) }).detach();
    }, cx)
}

/// Over the bar of the chosen file's song: the same.
fn file_zones(user: i64, file: &str, cx: &mut Context<Root>) -> Div {
    let file = file.to_string();
    track("file", move |r, to, cx| seek(r, user, &file, to, cx), cx)
}

fn time(ms: u64) -> String {
    format!("{}:{:02}", ms / 60000, ms / 1000 % 60)
}

/// The song's loudness, as a music app's bar has it: the speaker (a click mutes, a second gives it back) and a short
/// track beside it to drag.
fn loudness(root: &Root, cx: &mut Context<Root>) -> Div {
    loud_bar(root, ("pf-song-vol", "pf-song-volume"), volume(), "profileSongVolumeBack", set_volume, cx)
}

/// Spotify's own loudness (Windows' mixer), when it has a sound session there.
fn spotify_loudness(root: &Root, m: &crate::media::Playing, cx: &mut Context<Root>) -> Option<Div> {
    let v = (m.volume? * 100.).round() as i64;
    Some(loud_bar(root, ("pf-sp-vol", "pf-sp-volume"), v, "spotifyVolumeBack", |r, v, cx| {
        r.change_music(cx, |m| m.volume = Some(v as f32 / 100.));
        cx.background_executor().spawn(async move { crate::media::set_volume(v as f32 / 100.) }).detach();
    }, cx))
}

/// A loudness control: the speaker (a click mutes, another gives back what it was, kept under `back`) and a bar to
/// press or drag along, `v` of 100 now; `set` takes the new one.
fn loud_bar(root: &Root, ids: (&'static str, &'static str), v: i64, back: &'static str, set: fn(&mut Root, i64, &mut Context<Root>), cx: &mut Context<Root>) -> Div {
    const TRACK: f32 = 92.;
    let mut zones = div().absolute().inset_0().flex();
    for k in 0..=20i64 {
        let to = k * 5;
        zones = zones.child(div().id((ids.0, k as usize)).flex_1().h_full().cursor_pointer()
            .on_mouse_down(MouseButton::Left, cx.listener(move |r, _, _, cx| if to != v { set(r, to, cx) }))
            .on_mouse_move(cx.listener(move |r, e: &MouseMoveEvent, _, cx| if e.pressed_button == Some(MouseButton::Left) && to != v { set(r, to, cx) })));
    }
    let part = v as f32 / 100.;
    div().flex_none().flex().items_center().gap(px(6.))
        .child(IconButton::new(ids.1, if v == 0 { "volumeOff" } else { "volume" }).size(32.).round().tip(root.tr(if v == 0 { "pf.media.unmute" } else { "pf.media.mute" }))
            .on_click(cx.listener(move |r, _, _, cx| {
                // Muting keeps what it was, to give back.
                let was = crate::prefs::get(back).and_then(|b| b.as_i64()).filter(|b| *b > 0).unwrap_or(70);
                if v > 0 {
                    crate::prefs::set(back, serde_json::json!(v));
                }
                set(r, if v > 0 { 0 } else { was }, cx);
            })))
        .child(div().relative().w(px(TRACK)).h(px(22.))
            .child(div().absolute().left_0().right_0().top(px(9.)).h(px(4.)).rounded_full().bg(c(LINE)))
            .child(div().absolute().left_0().top(px(9.)).h(px(4.)).w(px(TRACK * part)).rounded_full().bg(c(TEXT_2)))
            .child(zones))
}

/// The music app's song under its name: who it is by, and which app plays it (Spotify, Apple Music, YouTube Music).
fn spotify_sub(m: &crate::media::Playing) -> String {
    if m.artist.trim().is_empty() { m.player.name().to_string() } else { format!("{} · {}", m.artist, m.player.name()) }
}

/// The music app's mark in the corner of its song's picture, on a dark round.
fn player_badge(m: &crate::media::Playing, size: f32) -> Div {
    div().absolute().right(px(4.)).bottom(px(4.)).size(px(size)).rounded_full().bg(ca(SHADE, 0.62)).flex().items_center().justify_center().child(icon(m.player.icon(), (size * 0.6).round(), c(m.player.colour())))
}

/// The card's ground and edge with a song on it, Spotify's or a chosen file's: its picture's own colour (the owner,
/// 2026-11-18: "màu card phải vibe theo nhạc"); the plain `ground` while the song has no picture.
fn song_tones(pic: Option<&str>, ground: u32, cx: &mut Context<Root>) -> (Hsla, Hsla) {
    match pic.and_then(|p| images::tone(p, cx)) {
        // (The picture's hue kept, dark and still coloured: mixed into the charcoal an orange cover went brown.)
        Some(tone) => {
            let t = c(tone);
            let deep = |l: f32| Hsla { h: t.h, s: t.s.clamp(0.4, 0.62), l, a: 1. };
            (deep(0.17), deep(0.3))
        }
        None => (c(ground), c(LINE)),
    }
}

/// The song the profile moves with, while one plays: whose sound it is (the chosen file plays in Bloxkeep, else the
/// music app's), and its cover's colour (the music app's own colour, or Bloxkeep's gold, when it has none).
pub(super) fn playing_now(root: &Root, user: i64, cx: &mut Context<Root>) -> Option<(crate::beat::Source, u32)> {
    if let Some(p) = root.profile.song.as_ref().filter(|p| p.user == user && p.playing) {
        let tone = cover(&p.file).and_then(|u| images::tone(&u, cx));
        return Some((crate::beat::Source::Own, tone.unwrap_or(0xffc83d)));
    }
    #[cfg(feature = "shot")]
    if std::env::var("BLOXKEEP_SHOT_BEAT").is_ok() {
        let tone = root.music(cx).and_then(|m| m.cover.clone()).and_then(|u| images::tone(&u, cx));
        return Some((crate::beat::Source::Own, tone.unwrap_or(0x1ed760)));
    }
    let m = root.music(cx).filter(|m| m.playing)?;
    let (player, cover) = (m.player, m.cover.clone());
    let tone = cover.and_then(|u| images::tone(&u, cx));
    Some((crate::beat::Source::App(player), tone.unwrap_or(player.colour())))
}

/// The album's picture of Spotify's song, filling a square of `size`.
fn spotify_cover(m: &crate::media::Playing, size: f32, cx: &mut Context<Root>) -> Option<AnyElement> {
    let (src, key) = images::pic_pair(m.cover.as_deref()?, size, size, cx)?;
    Some(fade_in(img(src).size_full().rounded(px(R_L)).object_fit(ObjectFit::Cover), &key).into_any_element())
}

/// The profile's song under the header: the card of 21st.dev's MusicPlayerCard (by Dhileep Kumar GM), which the
/// owner picked on 2026-11-17 ("ăn cắp cái này của họ luôn đi"): a compact card, the cover and the name on top, the
/// two times over a thin bar, a row of plain controls under it. Ours has one song, so Play alone stands where their
/// three do, the loudness beside it, and ⋮ where their last icon is. In the app's charcoal, not their black.
/// art: the picture shown when the song carries none (the profile's banner, else the account's face).
pub(super) fn card(root: &Root, user: i64, w: f32, ground: u32, art: Option<String>, cx: &mut Context<Root>) -> Div {
    const PIC: f32 = 64.;
    let shell = div().w(px(w)).flex_none().max_w_full().rounded(px(24.)).bg(c(ground)).border_1().border_color(c(LINE)).p(px(18.)).flex().flex_col();
    // (`live`: a song is there, playing or paused: its line starts with the mark of a song that plays, kit::playing_bars.)
    // (`mark`: the music app's own mark at the right, in its colour, beside the name: Spotify's song.)
    let head = |pic: AnyElement, title: AnyElement, sub: String, live: Option<bool>, mark: Option<crate::media::Player>| div().flex().items_center().gap(px(12.))
        .child(div().relative().size(px(PIC)).flex_none().rounded(px(R_L)).overflow_hidden().bg(c(RAISED_2)).flex().items_center().justify_center()
            .child(icon("music", 24., c(MUTED))).child(pic))
        .child(div().flex_1().min_w_0().flex().flex_col().gap(px(4.))
            .child(div().w_full().text_size(px(20.)).line_height(px(26.)).font_weight(FontWeight::SEMIBOLD).child(title))
            .child(div().w_full().flex().items_center().gap(px(8.))
                .when_some(live, |d, live| d.child(playing_bars("pf-song-bars", live, c(FACE))))
                .child(div().flex_1().min_w_0().truncate().text_size(px(T_M)).text_color(c(TEXT_2)).child(sub))))
        .when_some(mark, |d, p| d.child(div().flex_none().pl(px(4.)).child(icon(p.icon(), 30., c(p.colour())))));
    let words = |t: String| div().w_full().truncate().child(t).into_any_element();
    let Some(file) = path(Kind::Song, user) else {
        // None chosen and Spotify has a song: that song, with Spotify's own controls; a file can still be chosen.
        if let (Some(m), Some([back, play, next])) = (&root.music(cx), root.music_keys(["pf-sp-back", "pf-sp-play", "pf-sp-next"], cx)) {
            // (The app's mark stands at the right of the name, where the card had room, not over the picture.)
            let pic = div().absolute().inset_0().children(spotify_cover(m, PIC, cx)).into_any_element();
            let (ground, edge) = song_tones(m.cover.as_deref(), ground, cx);
            // (Where the song is, as a file's song has it: Spotify tells Windows, and takes a new place from it.)
            let bar = m.at().map(|(at, length)| bar_under("sp", at, Some(length), spotify_zones(cx)));
            // (Another song: its picture, name and line come in anew, rising into place, kit::line_in.)
            let song = format!("pf-sp|{}|{}", m.title, m.artist);
            return shell.bg(ground).border_color(edge).gap(px(16.))
                .child(line_in(song, 0, head(pic, marquee("pf-sp-title", m.title.clone()).into_any_element(), spotify_sub(m), Some(m.playing), Some(m.player))))
                .children(bar)
                .child(div().flex().items_center().gap(px(6.))
                    .child(back.size(36.)).child(play.size(40.).round()).child(next.size(36.))
                    .children(spotify_loudness(root, m, cx))
                    .child(div().flex_1())
                    .child(Button::new("pf-song-choose", root.tr("pf.media.songPick")).small().quiet().on_click(cx.listener(move |r, _, _, cx| choose(r, Kind::Song, user, cx)))));
        }
        // None yet: the same card, what it is, and the one button that adds one where the controls would be.
        return shell.gap(px(16.))
            .child(head(div().into_any_element(), words(root.tr("pf.media.song")), root.tr("pf.media.songHint"), None, None))
            .child(div().flex().child(Button::new("pf-song-choose", root.tr("pf.media.songPick")).on_click(cx.listener(move |r, _, _, cx| choose(r, Kind::Song, user, cx)))));
    };
    let track = root.profile.song.as_ref().filter(|p| p.file == file && p.user == user);
    let playing = track.is_some_and(|p| p.playing);
    let length = track.map(|t| t.length).or_else(|| length_of(&file));
    let at = track.map_or(0, |t| t.position);
    let title = Path::new(&file).file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
    let pic_url = cover(&file).or(art);
    let (ground, edge) = song_tones(pic_url.as_deref(), ground, cx);
    let pic = pic_url.and_then(|u| images::pic_pair(&u, PIC, PIC, cx))
        .map(|(src, key)| div().absolute().inset_0().child(fade_in(img(src).size_full().rounded(px(R_L)).object_fit(ObjectFit::Cover), &key)).into_any_element())
        .unwrap_or_else(|| div().into_any_element());
    let zones = file_zones(user, &file, cx);
    let (back, next) = skips(root, user, 36., cx);
    shell.bg(ground).border_color(edge)
        .child(head(pic, marquee("pf-song-title", title).into_any_element(), song_line(root, user), Some(playing), None))
        .child(div().pt(px(18.)).child(bar_under("file", at, length, zones)))
        .child(div().pt(px(10.)).flex().items_center().gap(px(10.))
            .children(back)
            .child(IconButton::new("pf-song-play", if playing { "pause" } else { "play" }).size(40.).round()
                .tip(root.tr(if playing { "pf.media.pause" } else { "pf.media.play" }))
                .on_click(cx.listener(move |r, _, _, cx| toggle(r, user, &file, cx))))
            .children(next)
            .child(loudness(root, cx))
            .child(div().flex_1())
            .child(more(root, user, 34., false, cx)))
}

/// The profile's song as a row across the page under the header, where the page is too narrow for the card in the
/// header (the owner's pick of the two, 2026-11-17: "head"). Laid as the Roblox profile add-on the owner showed has it: the cover at the left, a small "PROFILE SONG" tag over the name,
/// and at the right the time played, a short bar, the length, the loudness, ⋮ and the big round Play.
pub(super) fn row(root: &Root, user: i64, inner: f32, art: Option<String>, cx: &mut Context<Root>) -> Div {
    const PIC: f32 = 64.;
    let tag = div().flex_none().h(px(20.)).px(px(8.)).rounded_full().bg(ca(FACE, 0.14)).flex().items_center().gap(px(5.))
        .text_size(px(T_XS)).font_weight(FontWeight::BOLD).text_color(c(FACE)).child(icon("music", 11., c(FACE))).child(song_line(root, user).to_uppercase());
    let shell = div().w_full().rounded(px(R_HERO)).bg(c(RAISED)).border_1().border_color(c(LINE)).p(px(18.)).pr(px(20.)).flex().items_center().gap(px(12.));
    let Some(file) = path(Kind::Song, user) else {
        if let (Some(m), Some([back, play, next])) = (&root.music(cx), root.music_keys(["pf-sp-back", "pf-sp-play", "pf-sp-next"], cx)) {
            let (ground, edge) = song_tones(m.cover.as_deref(), RAISED, cx);
            let bar = m.at().map(|(at, length)| bar_between("sp", at, Some(length), spotify_zones(cx)));
            let (under, beside) = if inner >= 760. { (None, bar) } else { (bar, None) };
            return shell.bg(ground).border_color(edge)
                .child(div().relative().size(px(PIC)).flex_none().rounded(px(R_L)).overflow_hidden().bg(c(RAISED_2)).flex().items_center().justify_center()
                    .child(icon("music", 24., c(MUTED)))
                    .children(spotify_cover(m, PIC, cx).map(|p| div().absolute().inset_0().child(p)))
                    .child(player_badge(m, 22.)))
                .child(div().pl(px(4.)).flex_1().min_w_0().flex().flex_col().items_start().gap(px(6.))
                    .child(div().flex().items_center().gap(px(8.)).child(tag).child(playing_bars("pf-song-bars", m.playing, c(FACE))))
                    .child(line_in(format!("pf-sp-row|{}|{}", m.title, m.artist), 0, div().w_full().flex().flex_col().gap(px(6.))
                        .child(div().w_full().text_size(px(T_H)).font_weight(FontWeight::BOLD).child(marquee("pf-sp-title-row", m.title.clone())))
                        .child(div().w_full().truncate().text_size(px(T_M)).text_color(c(TEXT_2)).child(spotify_sub(m)))))
                    .when_some(under, |d, p| d.child(div().w_full().child(p))))
                .when_some(beside, |d, p| d.child(div().w(px(250.)).mr(px(6.)).flex_none().child(p)))
                .children(spotify_loudness(root, m, cx).filter(|_| inner >= 760.))
                .child(Button::new("pf-song-choose", root.tr("pf.media.songPick")).small().quiet().on_click(cx.listener(move |r, _, _, cx| choose(r, Kind::Song, user, cx))))
                .child(back.size(40.)).child(next.size(40.))
                .child(div().pl(px(2.)).child(play.size(52.).round().primary()));
        }
        return shell
            .child(div().size(px(PIC)).flex_none().rounded(px(R_L)).bg(c(RAISED_2)).flex().items_center().justify_center().child(icon("music", 24., c(MUTED))))
            .child(div().pl(px(4.)).flex_1().min_w_0().flex().flex_col().items_start().gap(px(6.)).child(tag)
                .child(div().w_full().truncate().text_size(px(T_H)).text_color(c(TEXT_2)).child(root.tr("pf.media.songHint"))))
            .child(Button::new("pf-song-choose", root.tr("pf.media.songPick")).on_click(cx.listener(move |r, _, _, cx| choose(r, Kind::Song, user, cx))));
    };
    let track = root.profile.song.as_ref().filter(|p| p.file == file && p.user == user);
    let playing = track.is_some_and(|p| p.playing);
    let length = track.map(|t| t.length).or_else(|| length_of(&file));
    let at = track.map_or(0, |t| t.position);
    let title = Path::new(&file).file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
    let pic_url = cover(&file).or(art);
    let (ground, edge) = song_tones(pic_url.as_deref(), RAISED, cx);
    let pic = pic_url.and_then(|u| images::pic_pair(&u, PIC, PIC, cx));
    let wide = inner >= 760.;
    let zones = file_zones(user, &file, cx);
    let progress = bar_between("file", at, length, zones);
    let (back, next) = skips(root, user, 40., cx);
    let (under, beside) = if wide { (None, Some(progress)) } else { (Some(progress), None) };
    shell.bg(ground).border_color(edge)
        .child(div().relative().size(px(PIC)).flex_none().rounded(px(R_L)).overflow_hidden().bg(c(RAISED_2)).flex().items_center().justify_center()
            .child(icon("music", 24., c(MUTED)))
            .when_some(pic, |d, (src, key)| d.child(div().absolute().inset_0().child(fade_in(img(src).size_full().rounded(px(R_L)).object_fit(ObjectFit::Cover), &key)))))
        .child(div().pl(px(4.)).flex_1().min_w_0().flex().flex_col().items_start().gap(px(6.))
            .child(div().flex().items_center().gap(px(8.)).child(tag).child(playing_bars("pf-song-bars", playing, c(FACE))))
            .child(div().w_full().text_size(px(T_H)).font_weight(FontWeight::BOLD).child(marquee("pf-song-title-row", title)))
            .when_some(under, |d, p| d.child(div().w_full().child(p))))
        .when_some(beside, |d, p| d.child(div().w(px(250.)).mr(px(6.)).flex_none().child(p)))
        .when(wide, |d| d.child(loudness(root, cx)))
        .child(more(root, user, 40., true, cx))
        .children(back)
        .children(next)
        .child(div().pl(px(2.)).child(IconButton::new("pf-song-play", if playing { "pause" } else { "play" }).size(52.).round().primary()
            .tip(root.tr(if playing { "pf.media.pause" } else { "pf.media.play" }))
            .on_click(cx.listener(move |r, _, _, cx| toggle(r, user, &file, cx)))))
}
